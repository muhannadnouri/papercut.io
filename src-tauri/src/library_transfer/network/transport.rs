//! Resumable package streaming and receiver progress framing.

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, Read, Seek, SeekFrom, Write};
use std::net::{SocketAddr, SocketAddrV4, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rustls::pki_types::ServerName;
use rustls::{ClientConnection, ServerConfig, ServerConnection, StreamOwned};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::security::{
    client_tls_config, pairing_proof, read_and_verify_proof, tls_exporter, PROTOCOL_MAGIC,
    RECEIVER_ROLE, SENDER_ROLE,
};
use super::LibraryTransferSendStatus;
use crate::library_transfer::package::MAX_PACKAGE_BYTES;
use crate::library_transfer::{
    ensure_available_space, transfer_cache_dir, LibraryTransferError, LibraryTransferProgress,
    LibraryTransferResult,
};

const SOCKET_TIMEOUT: Duration = Duration::from_secs(30);
const SESSION_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const COPY_BUFFER_BYTES: usize = 64 * 1024;
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);
const RECEIVER_MESSAGE_MAX_BYTES: usize = 64 * 1024;
const RECEIVE_PART_PREFIX: &str = "library-transfer-lan-receive-";

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub(super) enum ReceiverMessage {
    Progress(LibraryTransferProgress),
    Complete,
    Failed(LibraryTransferError),
}

#[derive(Debug)]
pub(super) enum SendAttemptError {
    Fatal(String),
    Retryable(String),
}

/// Authenticate one receiver, resume from its announced offset, then keep the
/// TLS stream open until that receiver reports target-side import completion.
pub(super) fn send_package(
    mut socket: TcpStream,
    tls_config: Arc<ServerConfig>,
    code: &str,
    package_path: &Path,
    status: &Arc<Mutex<LibraryTransferSendStatus>>,
    cancel: &AtomicBool,
) -> Result<(), SendAttemptError> {
    configure_socket(&socket).map_err(SendAttemptError::Fatal)?;
    let mut connection = ServerConnection::new(tls_config).map_err(|err| {
        SendAttemptError::Fatal(format!(
            "Failed to initialize secure library transfer: {err}"
        ))
    })?;
    connection.complete_io(&mut socket).map_err(|err| {
        SendAttemptError::Fatal(format!("Secure library transfer handshake failed: {err}"))
    })?;
    let exporter = tls_exporter(&connection).map_err(SendAttemptError::Fatal)?;
    let mut stream = StreamOwned::new(connection, socket);
    read_and_verify_proof(&mut stream, code, &exporter, RECEIVER_ROLE)
        .map_err(SendAttemptError::Fatal)?;
    stream
        .write_all(PROTOCOL_MAGIC)
        .and_then(|_| stream.write_all(&pairing_proof(code, &exporter, SENDER_ROLE)?))
        .map_err(|err| {
            SendAttemptError::Retryable(format!("Failed to authenticate library sender: {err}"))
        })?;

    let mut package = File::open(package_path).map_err(|err| {
        SendAttemptError::Fatal(format!("Failed to open prepared library package: {err}"))
    })?;
    let package_bytes = package
        .metadata()
        .map_err(|err| {
            SendAttemptError::Fatal(format!("Failed to inspect prepared library package: {err}"))
        })?
        .len();
    stream
        .write_all(&package_bytes.to_be_bytes())
        .and_then(|_| stream.flush())
        .map_err(|err| {
            SendAttemptError::Retryable(format!("Failed to send library package header: {err}"))
        })?;
    let mut offset = [0u8; 8];
    stream.read_exact(&mut offset).map_err(|err| {
        SendAttemptError::Retryable(format!("Failed to read library resume position: {err}"))
    })?;
    let offset = u64::from_be_bytes(offset);
    if offset > package_bytes {
        return Err(SendAttemptError::Fatal(
            "Receiver reported an invalid library resume position".into(),
        ));
    }
    package.seek(SeekFrom::Start(offset)).map_err(|err| {
        SendAttemptError::Fatal(format!("Failed to resume prepared library package: {err}"))
    })?;
    if let Ok(mut current) = status.lock() {
        current.bytes_transferred = offset;
    }
    let mut last_progress = Instant::now() - PROGRESS_INTERVAL;
    let copied = copy_exact_with_progress(
        &mut BufReader::new(package),
        &mut stream,
        package_bytes - offset,
        || cancel.load(Ordering::Relaxed),
        |processed| {
            let total_processed = offset + processed;
            if total_processed == package_bytes || last_progress.elapsed() >= PROGRESS_INTERVAL {
                if let Ok(mut current) = status.lock() {
                    current.bytes_transferred = total_processed;
                }
                last_progress = Instant::now();
            }
        },
    )
    .map_err(|err| SendAttemptError::Retryable(format!("Failed to send library package: {err}")))?;
    if copied != package_bytes - offset {
        return Err(SendAttemptError::Retryable(
            "Receiver disconnected before the library package was complete".into(),
        ));
    }
    stream.flush().map_err(|err| {
        SendAttemptError::Retryable(format!("Failed to finish library send: {err}"))
    })?;
    stream
        .sock
        .set_read_timeout(Some(SESSION_TIMEOUT))
        .map_err(|err| {
            SendAttemptError::Retryable(format!(
                "Failed to wait for receiving-device import progress: {err}"
            ))
        })?;

    loop {
        match read_receiver_message(&mut stream).map_err(SendAttemptError::Retryable)? {
            ReceiverMessage::Progress(progress) => {
                if let Ok(mut current) = status.lock() {
                    current.receiver_progress = Some(progress);
                }
            }
            ReceiverMessage::Complete => return Ok(()),
            ReceiverMessage::Failed(error) => {
                let retryable = error.is_retryable();
                let error = format!("The receiving device could not finish the import: {error}");
                return Err(if retryable {
                    SendAttemptError::Retryable(error)
                } else {
                    SendAttemptError::Fatal(error)
                });
            }
        }
    }
}

/// Authenticate the source, resume its package into app cache, and return the
/// live TLS stream so target-side import progress can travel back to the sender.
pub(super) fn receive_package<P: FnMut(u64, u64)>(
    address: SocketAddrV4,
    code: &str,
    output_path: &Path,
    mut progress: P,
) -> LibraryTransferResult<StreamOwned<ClientConnection, TcpStream>> {
    let mut socket = TcpStream::connect_timeout(&SocketAddr::V4(address), SOCKET_TIMEOUT)
        .map_err(|err| format!("Could not connect to the source device: {err}"))?;
    configure_socket(&socket)?;
    let config = client_tls_config()?;
    let server_name = ServerName::IpAddress((*address.ip()).into());
    let mut connection = ClientConnection::new(config, server_name)
        .map_err(|err| format!("Failed to initialize secure library transfer: {err}"))?;
    connection
        .complete_io(&mut socket)
        .map_err(|err| format!("Secure library transfer handshake failed: {err}"))?;
    let exporter = tls_exporter(&connection)?;
    let mut stream = StreamOwned::new(connection, socket);
    stream
        .write_all(PROTOCOL_MAGIC)
        .and_then(|_| stream.write_all(&pairing_proof(code, &exporter, RECEIVER_ROLE)?))
        .and_then(|_| stream.flush())
        .map_err(|err| format!("Failed to authenticate library receiver: {err}"))?;
    read_and_verify_proof(&mut stream, code, &exporter, SENDER_ROLE)?;

    let mut size = [0u8; 8];
    stream
        .read_exact(&mut size)
        .map_err(|err| format!("Failed to read library package header: {err}"))?;
    let size = u64::from_be_bytes(size);
    if size == 0 || size > MAX_PACKAGE_BYTES {
        return Err("Source device reported an unsupported library package size".into());
    }
    remove_other_receive_parts(output_path);
    let mut output = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(output_path)
        .map_err(|err| format!("Failed to open partial library package: {err}"))?;
    let mut offset = output
        .metadata()
        .map_err(|err| format!("Failed to inspect partial library package: {err}"))?
        .len();
    if offset > size {
        output
            .set_len(0)
            .map_err(|err| format!("Failed to reset partial library package: {err}"))?;
        offset = 0;
    }
    let staging_dir = output_path
        .parent()
        .ok_or_else(|| "Partial library package path has no parent".to_string())?;
    ensure_available_space(staging_dir, size - offset)?;
    stream
        .write_all(&offset.to_be_bytes())
        .and_then(|_| stream.flush())
        .map_err(|err| format!("Failed to send library resume position: {err}"))?;
    output
        .seek(SeekFrom::Start(offset))
        .map_err(|err| format!("Failed to resume partial library package: {err}"))?;
    progress(offset, size);
    let mut last_progress = Instant::now() - PROGRESS_INTERVAL;
    let copied = copy_exact_with_progress(
        &mut stream,
        &mut output,
        size - offset,
        || false,
        |processed| {
            let total_processed = offset + processed;
            if total_processed == size || last_progress.elapsed() >= PROGRESS_INTERVAL {
                progress(total_processed, size);
                last_progress = Instant::now();
            }
        },
    )
    .map_err(|err| format!("Failed to receive library package: {err}"))?;
    if copied != size - offset {
        return Err("Source device disconnected before the library package was complete".into());
    }
    output
        .flush()
        .map_err(|err| format!("Failed to finish partial library package: {err}"))?;
    Ok(stream)
}

/// Keep one opaque partial file per source session. The address and random code
/// make retries converge on the same file without exposing credentials in its
/// name; starting another authenticated receive discards older unusable parts.
pub(super) fn receive_resume_path(
    app: &tauri::AppHandle,
    address: SocketAddrV4,
    code: &str,
) -> Result<PathBuf, String> {
    let cache = transfer_cache_dir(app)?;
    let key = format!("{:x}", Sha256::digest(format!("{address}|{code}")));
    Ok(cache.join(format!("{RECEIVE_PART_PREFIX}{key}.part")))
}

/// Best-effort cleanup prevents abandoned sessions from accumulating multi-GB
/// cache files; the current authenticated session is always retained.
fn remove_other_receive_parts(current: &Path) {
    let Some(parent) = current.parent() else {
        return;
    };
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_receive_part = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(RECEIVE_PART_PREFIX) && name.ends_with(".part"));
        if is_receive_part && path != current {
            let _ = fs::remove_file(path);
        }
    }
}

/// Frame target-side progress on the existing TLS stream. A bounded JSON body
/// keeps the protocol inspectable without introducing a second transport.
pub(super) fn write_receiver_message<W: Write>(
    writer: &mut W,
    message: &ReceiverMessage,
) -> io::Result<()> {
    let body = serde_json::to_vec(message).map_err(io::Error::other)?;
    if body.len() > RECEIVER_MESSAGE_MAX_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "library transfer progress message is too large",
        ));
    }
    writer.write_all(&(body.len() as u32).to_be_bytes())?;
    writer.write_all(&body)?;
    writer.flush()
}

/// Decode one bounded target-side status frame after package upload completes.
fn read_receiver_message<R: Read>(reader: &mut R) -> Result<ReceiverMessage, String> {
    let mut size = [0u8; 4];
    reader
        .read_exact(&mut size)
        .map_err(|err| format!("Failed to read receiving-device progress: {err}"))?;
    let size = u32::from_be_bytes(size) as usize;
    if size == 0 || size > RECEIVER_MESSAGE_MAX_BYTES {
        return Err("Receiving device sent an invalid progress message".into());
    }
    let mut body = vec![0u8; size];
    reader
        .read_exact(&mut body)
        .map_err(|err| format!("Failed to read receiving-device progress: {err}"))?;
    serde_json::from_slice(&body)
        .map_err(|err| format!("Receiving device sent invalid progress data: {err}"))
}

/// Copy an announced byte count without buffering the whole package. The
/// callback is throttled by callers, while cancellation is checked between
/// chunks so stopping a sender also interrupts an active transfer.
fn copy_exact_with_progress<R, W, C, P>(
    reader: &mut R,
    writer: &mut W,
    total: u64,
    mut cancelled: C,
    mut progress: P,
) -> io::Result<u64>
where
    R: Read,
    W: Write,
    C: FnMut() -> bool,
    P: FnMut(u64),
{
    let mut buffer = [0u8; COPY_BUFFER_BYTES];
    let mut copied = 0u64;
    while copied < total {
        if cancelled() {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "library transfer cancelled",
            ));
        }
        let remaining = usize::try_from((total - copied).min(COPY_BUFFER_BYTES as u64))
            .unwrap_or(COPY_BUFFER_BYTES);
        let read = reader.read(&mut buffer[..remaining])?;
        if read == 0 {
            break;
        }
        writer.write_all(&buffer[..read])?;
        copied += read as u64;
        progress(copied);
    }
    Ok(copied)
}

fn configure_socket(socket: &TcpStream) -> Result<(), String> {
    socket
        .set_read_timeout(Some(SOCKET_TIMEOUT))
        .and_then(|_| socket.set_write_timeout(Some(SOCKET_TIMEOUT)))
        .map_err(|err| format!("Failed to configure local library connection: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library_transfer::network::{security::server_tls_config, LibraryTransferSendState};
    use std::net::{Ipv4Addr, TcpListener};
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn tls_round_trip_resumes_and_waits_for_receiver_completion() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = match listener.local_addr().unwrap() {
            SocketAddr::V4(address) => address,
            _ => unreachable!(),
        };
        let test_key = format!(
            "papercut-lan-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let package_path = std::env::temp_dir().join(format!("{test_key}-source"));
        let received_path = std::env::temp_dir().join(format!("{test_key}-received.part"));
        let package = b"test papercut library package";
        fs::write(&package_path, package).unwrap();
        fs::write(&received_path, &package[..8]).unwrap();
        let sender_path = package_path.clone();
        let sender = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let status = Arc::new(Mutex::new(LibraryTransferSendStatus {
                state: LibraryTransferSendState::Sending,
                address: address.to_string(),
                code: "2345-ABCD".into(),
                documents: 1,
                audiobooks: 0,
                package_bytes: package.len() as u64,
                bytes_transferred: 0,
                receiver_progress: None,
                error: None,
            }));
            let result = send_package(
                stream,
                server_tls_config().unwrap(),
                "2345ABCD",
                &sender_path,
                &status,
                &AtomicBool::new(false),
            );
            let final_status = status.lock().unwrap().clone();
            (result, final_status)
        });

        let mut stream = receive_package(address, "2345ABCD", &received_path, |_, _| {}).unwrap();
        let progress = LibraryTransferProgress {
            operation: "receive".into(),
            phase: "verifying".into(),
            bytes_processed: None,
            bytes_total: None,
            items_processed: None,
            items_total: None,
            item: None,
        };
        write_receiver_message(&mut stream, &ReceiverMessage::Progress(progress.clone())).unwrap();
        write_receiver_message(&mut stream, &ReceiverMessage::Complete).unwrap();
        let (result, status) = sender.join().unwrap();
        result.unwrap();

        assert_eq!(fs::read(&received_path).unwrap(), package);
        assert_eq!(status.bytes_transferred, package.len() as u64);
        assert_eq!(status.receiver_progress.unwrap().phase, progress.phase);
        let _ = fs::remove_file(package_path);
        let _ = fs::remove_file(received_path);
    }
}
