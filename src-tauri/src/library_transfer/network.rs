//! Foreground, one-use library transfer over the local network.
//!
//! High-level flow:
//! 1. The source builds the same checked `.papercut-library` package used by
//!    file export, binds an IPv4 listener, and displays its address plus a
//!    random one-use code.
//! 2. The target enters those values. Both peers establish ephemeral TLS, then
//!    prove knowledge of the code with HMACs bound to that exact TLS session.
//!    This authenticates the self-signed connection without a permanent key or
//!    certificate warning.
//! 3. The target reports how many bytes of the authenticated session it already
//!    has, then retains that partial package if the connection drops. Retrying
//!    with the same address and code continues from that byte offset.
//! 4. Once all bytes arrive, the target invokes the normal package importer and
//!    forwards its verification and restore progress to the source. The source
//!    reports completion only after the target confirms that import finished.
//!
//! Sessions stay foreground-only and expire after ten minutes. Failed
//! authentication consumes the code to prevent online guessing; an authenticated
//! interruption may reconnect with the same code. Device discovery and
//! background transfer remain outside this module.

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, Read, Seek, SeekFrom, Write};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream, UdpSocket};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use hmac::{Hmac, Mac};
use rcgen::generate_simple_self_signed;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{verify_tls12_signature, verify_tls13_signature, WebPkiSupportedAlgorithms};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::{
    ClientConfig, ClientConnection, DigitallySignedStruct, Error as TlsError, ServerConfig,
    ServerConnection, SignatureScheme, StreamOwned,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::State;

use super::package::MAX_PACKAGE_BYTES;
use super::{
    build_library_package, emit_transfer_progress, ensure_available_space, import_library_package,
    transfer_cache_dir, transfer_temp_path, LibraryTransferExportRequest,
    LibraryTransferImportResult, LibraryTransferProgress, INSUFFICIENT_SPACE_CODE,
};

const PROTOCOL_MAGIC: &[u8; 8] = b"PCLAN002";
const TLS_EXPORTER_LABEL: &[u8] = b"papercut-library-transfer-v1";
const RECEIVER_ROLE: &[u8] = b"receiver";
const SENDER_ROLE: &[u8] = b"sender";
const PROOF_BYTES: usize = 32;
const CODE_ALPHABET: &[u8; 32] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";
const CODE_CHARS: usize = 8;
const SESSION_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const SOCKET_TIMEOUT: Duration = Duration::from_secs(30);
const COPY_BUFFER_BYTES: usize = 64 * 1024;
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);
const RECEIVER_MESSAGE_MAX_BYTES: usize = 64 * 1024;
const RECEIVE_PART_PREFIX: &str = "library-transfer-lan-receive-";

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Default)]
pub struct LibraryTransferState {
    send: Arc<Mutex<Option<SendSession>>>,
}

#[derive(Clone)]
struct SendSession {
    cancel: Arc<AtomicBool>,
    status: Arc<Mutex<LibraryTransferSendStatus>>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryTransferSendStatus {
    state: LibraryTransferSendState,
    address: String,
    code: String,
    documents: usize,
    audiobooks: usize,
    package_bytes: u64,
    bytes_transferred: u64,
    receiver_progress: Option<LibraryTransferProgress>,
    error: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum LibraryTransferSendState {
    Waiting,
    Sending,
    Complete,
    Cancelled,
    Failed,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryTransferReceiveRequest {
    address: String,
    code: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
enum ReceiverMessage {
    Progress(LibraryTransferProgress),
    Complete,
    Failed(String),
}

#[derive(Debug)]
enum SendAttemptError {
    Fatal(String),
    Retryable(String),
}

/// Prepare one package, bind a foreground listener, and return the address and
/// one-use code before a receiver connects. A second active send is rejected so
/// two callers cannot replace each other's package or session credentials.
#[tauri::command]
pub async fn library_transfer_send_start(
    app: tauri::AppHandle,
    state: State<'_, LibraryTransferState>,
    request: Option<LibraryTransferExportRequest>,
) -> Result<LibraryTransferSendStatus, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        start_send(app, state, request.unwrap_or_default().include_audiobooks)
    })
    .await
    .map_err(|err| format!("Library send task failed: {err}"))?
}

#[tauri::command]
pub fn library_transfer_send_status(
    state: State<'_, LibraryTransferState>,
) -> Result<Option<LibraryTransferSendStatus>, String> {
    let send = lock(&state.send)?;
    send.as_ref()
        .map(|session| lock(&session.status).map(|status| status.clone()))
        .transpose()
}

/// Stop a waiting or active foreground sender. The worker owns package cleanup
/// so cancellation cannot remove a file while another thread is reading it.
#[tauri::command]
pub fn library_transfer_send_cancel(state: State<'_, LibraryTransferState>) -> Result<(), String> {
    let send = lock(&state.send)?;
    if let Some(session) = send.as_ref() {
        session.cancel.store(true, Ordering::Relaxed);
    }
    Ok(())
}

/// Receive exactly one authenticated package into app cache, then hand it to
/// the normal package importer. Network bytes never bypass archive validation,
/// checksums, HTML sanitization, or target-side index rebuilding.
#[tauri::command]
pub async fn library_transfer_receive(
    app: tauri::AppHandle,
    request: LibraryTransferReceiveRequest,
) -> Result<LibraryTransferImportResult, String> {
    tauri::async_runtime::spawn_blocking(move || receive_library(app, request))
        .await
        .map_err(|err| format!("Library receive task failed: {err}"))?
}

fn start_send(
    app: tauri::AppHandle,
    state: LibraryTransferState,
    include_audiobooks: bool,
) -> Result<LibraryTransferSendStatus, String> {
    {
        let send = lock(&state.send)?;
        if send.as_ref().is_some_and(|session| {
            lock(&session.status)
                .map(|status| {
                    matches!(
                        status.state,
                        LibraryTransferSendState::Waiting | LibraryTransferSendState::Sending
                    )
                })
                .unwrap_or(true)
        }) {
            return Err("A library send is already active".into());
        }
    }

    let package_path = transfer_temp_path(&app, "lan-send")?;
    let prepared = build_library_package(&app, &package_path, include_audiobooks);
    let prepared = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            let _ = fs::remove_file(&package_path);
            return Err(error);
        }
    };
    let package_bytes = fs::metadata(&package_path)
        .map_err(|err| format!("Failed to inspect prepared library package: {err}"))?
        .len();
    let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0))
        .map_err(|err| format!("Failed to start local library transfer: {err}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|err| format!("Failed to configure local library transfer: {err}"))?;
    let port = listener
        .local_addr()
        .map_err(|err| format!("Failed to read local library transfer address: {err}"))?
        .port();
    let address = SocketAddrV4::new(local_ipv4()?, port).to_string();
    let code = new_pairing_code()?;
    let tls_config = server_tls_config()?;
    let status = Arc::new(Mutex::new(LibraryTransferSendStatus {
        state: LibraryTransferSendState::Waiting,
        address,
        code: display_code(&code),
        documents: prepared.documents,
        audiobooks: prepared.audiobooks,
        package_bytes,
        bytes_transferred: 0,
        receiver_progress: None,
        error: None,
    }));
    let cancel = Arc::new(AtomicBool::new(false));
    let session = SendSession {
        cancel: Arc::clone(&cancel),
        status: Arc::clone(&status),
    };
    *lock(&state.send)? = Some(session);

    let thread_status = Arc::clone(&status);
    thread::spawn(move || {
        run_sender(
            listener,
            tls_config,
            &code,
            &package_path,
            &thread_status,
            &cancel,
        );
        let _ = fs::remove_file(package_path);
    });
    lock(&status).map(|status| status.clone())
}

/// Keep one package/code alive for authenticated resume attempts until import
/// completes, cancellation, or expiry. An unauthenticated failure still consumes
/// the session so reconnect support cannot become an online guessing oracle.
fn run_sender(
    listener: TcpListener,
    tls_config: Arc<ServerConfig>,
    code: &str,
    package_path: &Path,
    status: &Arc<Mutex<LibraryTransferSendStatus>>,
    cancel: &AtomicBool,
) {
    let deadline = Instant::now() + SESSION_TIMEOUT;
    loop {
        if cancel.load(Ordering::Relaxed) {
            set_send_status(status, LibraryTransferSendState::Cancelled, None);
            return;
        }
        if Instant::now() >= deadline {
            set_send_status(
                status,
                LibraryTransferSendState::Failed,
                Some("Library send timed out".into()),
            );
            return;
        }
        match listener.accept() {
            Ok((stream, _)) => {
                set_send_active(status);
                let result = send_package(
                    stream,
                    Arc::clone(&tls_config),
                    code,
                    package_path,
                    status,
                    cancel,
                );
                if cancel.load(Ordering::Relaxed) {
                    set_send_status(status, LibraryTransferSendState::Cancelled, None);
                    return;
                }
                match result {
                    Ok(()) => {
                        set_send_status(status, LibraryTransferSendState::Complete, None);
                        return;
                    }
                    Err(SendAttemptError::Fatal(error)) => {
                        set_send_status(status, LibraryTransferSendState::Failed, Some(error));
                        return;
                    }
                    Err(SendAttemptError::Retryable(error)) => {
                        set_send_retry_waiting(status, error);
                    }
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(100));
            }
            Err(error) => {
                set_send_status(
                    status,
                    LibraryTransferSendState::Failed,
                    Some(format!("Local library transfer failed: {error}")),
                );
                return;
            }
        }
    }
}

fn receive_library(
    app: tauri::AppHandle,
    request: LibraryTransferReceiveRequest,
) -> Result<LibraryTransferImportResult, String> {
    emit_transfer_progress(&app, "receive", "connecting", None, None, None);
    let address = parse_local_address(&request.address)?;
    let code = normalize_code(&request.code)?;
    let temp_path = receive_resume_path(&app, address, &code)?;
    (|| {
        let mut stream = receive_package(address, &code, &temp_path, |processed, total| {
            emit_transfer_progress(
                &app,
                "receive",
                "receiving",
                Some((processed, total)),
                None,
                None,
            );
        })?;

        let import_result = {
            let mut forward = |progress: &LibraryTransferProgress| {
                let _ = write_receiver_message(
                    &mut stream,
                    &ReceiverMessage::Progress(progress.clone()),
                );
            };
            import_library_package(&app, &temp_path, "receive", Some(&mut forward))
        };
        match import_result {
            Ok(result) => {
                let _ = write_receiver_message(&mut stream, &ReceiverMessage::Complete);
                let _ = fs::remove_file(&temp_path);
                Ok(result)
            }
            Err(error) => {
                let _ =
                    write_receiver_message(&mut stream, &ReceiverMessage::Failed(error.clone()));
                if !error.contains(INSUFFICIENT_SPACE_CODE) {
                    let _ = fs::remove_file(&temp_path);
                }
                Err(error)
            }
        }
    })()
}

fn send_package(
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
                return Err(SendAttemptError::Retryable(format!(
                    "The receiving device could not finish the import: {error}"
                )));
            }
        }
    }
}

fn receive_package<P: FnMut(u64, u64)>(
    address: SocketAddrV4,
    code: &str,
    output_path: &Path,
    mut progress: P,
) -> Result<StreamOwned<ClientConnection, TcpStream>, String> {
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
fn receive_resume_path(
    app: &tauri::AppHandle,
    address: SocketAddrV4,
    code: &str,
) -> Result<std::path::PathBuf, String> {
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
fn write_receiver_message<W: Write>(writer: &mut W, message: &ReceiverMessage) -> io::Result<()> {
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

/// The ephemeral certificate encrypts TLS, while the HMAC below authenticates
/// the exact TLS channel with the displayed one-use code. This avoids shipping
/// a long-lived private key or asking users to trust a local certificate.
fn pairing_proof(code: &str, exporter: &[u8], role: &[u8]) -> io::Result<[u8; PROOF_BYTES]> {
    let mut mac = HmacSha256::new_from_slice(code.as_bytes())
        .map_err(|_| io::Error::other("Invalid library transfer code"))?;
    mac.update(PROTOCOL_MAGIC);
    mac.update(exporter);
    mac.update(role);
    Ok(mac.finalize().into_bytes().into())
}

fn read_and_verify_proof<R: Read>(
    reader: &mut R,
    code: &str,
    exporter: &[u8],
    role: &[u8],
) -> Result<(), String> {
    let mut magic = [0u8; PROTOCOL_MAGIC.len()];
    let mut proof = [0u8; PROOF_BYTES];
    reader
        .read_exact(&mut magic)
        .and_then(|_| reader.read_exact(&mut proof))
        .map_err(|err| format!("Failed to read library pairing proof: {err}"))?;
    if &magic != PROTOCOL_MAGIC {
        return Err("The other device is not using a compatible Papercut transfer".into());
    }
    let mut mac = HmacSha256::new_from_slice(code.as_bytes())
        .map_err(|_| "Invalid library transfer code".to_string())?;
    mac.update(PROTOCOL_MAGIC);
    mac.update(exporter);
    mac.update(role);
    mac.verify_slice(&proof)
        .map_err(|_| "Pairing code did not match the source device".to_string())
}

fn tls_exporter<C>(connection: &C) -> Result<[u8; PROOF_BYTES], String>
where
    C: ExportKeyingMaterial,
{
    connection
        .export([0u8; PROOF_BYTES], TLS_EXPORTER_LABEL)
        .map_err(|err| format!("Failed to authenticate secure library transfer: {err}"))
}

trait ExportKeyingMaterial {
    fn export<const N: usize>(&self, output: [u8; N], label: &[u8]) -> Result<[u8; N], TlsError>;
}

impl ExportKeyingMaterial for ServerConnection {
    fn export<const N: usize>(&self, output: [u8; N], label: &[u8]) -> Result<[u8; N], TlsError> {
        self.export_keying_material(output, label, None)
    }
}

impl ExportKeyingMaterial for ClientConnection {
    fn export<const N: usize>(&self, output: [u8; N], label: &[u8]) -> Result<[u8; N], TlsError> {
        self.export_keying_material(output, label, None)
    }
}

fn server_tls_config() -> Result<Arc<ServerConfig>, String> {
    let certified = generate_simple_self_signed(["papercut.local".to_string()])
        .map_err(|err| format!("Failed to create secure transfer certificate: {err}"))?;
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(certified.key_pair.serialize_der()));
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|err| format!("Failed to configure secure library transfer: {err}"))?
        .with_no_client_auth()
        .with_single_cert(vec![certified.cert.der().clone()], key)
        .map_err(|err| format!("Failed to configure secure transfer certificate: {err}"))?;
    Ok(Arc::new(config))
}

fn client_tls_config() -> Result<Arc<ClientConfig>, String> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let verifier = Arc::new(PairingCertificateVerifier {
        supported: provider.signature_verification_algorithms,
    });
    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|err| format!("Failed to configure secure library transfer: {err}"))?
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();
    Ok(Arc::new(config))
}

/// Certificate-chain trust is intentionally deferred to the channel-bound
/// one-use pairing proof; handshake signatures are still cryptographically
/// verified so an invalid or corrupted ephemeral certificate is rejected.
#[derive(Debug)]
struct PairingCertificateVerifier {
    supported: WebPkiSupportedAlgorithms,
}

impl ServerCertVerifier for PairingCertificateVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        verify_tls12_signature(message, cert, dss, &self.supported)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        verify_tls13_signature(message, cert, dss, &self.supported)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.supported.supported_schemes()
    }
}

fn configure_socket(socket: &TcpStream) -> Result<(), String> {
    socket
        .set_read_timeout(Some(SOCKET_TIMEOUT))
        .and_then(|_| socket.set_write_timeout(Some(SOCKET_TIMEOUT)))
        .map_err(|err| format!("Failed to configure local library connection: {err}"))
}

/// Ask the routing table which IPv4 interface would carry a packet. UDP
/// `connect` does not send traffic, so address discovery remains offline.
fn local_ipv4() -> Result<Ipv4Addr, String> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))
        .map_err(|err| format!("Failed to inspect local network: {err}"))?;
    socket
        .connect((Ipv4Addr::new(192, 0, 2, 1), 9))
        .map_err(|err| format!("Connect both devices to the same local network: {err}"))?;
    match socket
        .local_addr()
        .map_err(|err| format!("Failed to inspect local network address: {err}"))?
        .ip()
    {
        std::net::IpAddr::V4(ip) if !ip.is_unspecified() && !ip.is_loopback() => Ok(ip),
        _ => Err("Could not find this device's local IPv4 address".into()),
    }
}

fn parse_local_address(value: &str) -> Result<SocketAddrV4, String> {
    let address = value
        .trim()
        .parse::<SocketAddrV4>()
        .map_err(|_| "Enter the source address exactly as shown on the other device".to_string())?;
    let ip = address.ip();
    if !(ip.is_private() || ip.is_link_local() || ip.is_loopback()) {
        return Err("The source address must be on the local network".into());
    }
    Ok(address)
}

fn new_pairing_code() -> Result<String, String> {
    let mut random = [0u8; CODE_CHARS];
    getrandom::fill(&mut random)
        .map_err(|err| format!("Failed to create a secure pairing code: {err}"))?;
    Ok(random
        .into_iter()
        .map(|value| CODE_ALPHABET[(value & 31) as usize] as char)
        .collect())
}

fn normalize_code(value: &str) -> Result<String, String> {
    let code: String = value
        .chars()
        .filter(|character| !character.is_ascii_whitespace() && *character != '-')
        .flat_map(char::to_uppercase)
        .collect();
    if code.len() != CODE_CHARS
        || !code
            .bytes()
            .all(|character| CODE_ALPHABET.contains(&character))
    {
        return Err("Enter the 8-character pairing code shown on the source device".into());
    }
    Ok(code)
}

fn display_code(code: &str) -> String {
    format!("{}-{}", &code[..4], &code[4..])
}

fn set_send_status(
    status: &Mutex<LibraryTransferSendStatus>,
    state: LibraryTransferSendState,
    error: Option<String>,
) {
    if let Ok(mut status) = status.lock() {
        status.state = state;
        status.error = error;
    }
}

fn set_send_active(status: &Mutex<LibraryTransferSendStatus>) {
    if let Ok(mut status) = status.lock() {
        status.state = LibraryTransferSendState::Sending;
        status.receiver_progress = None;
        status.error = None;
    }
}

/// Return an authenticated interrupted session to its pairing screen while
/// retaining the source package and target partial for an explicit retry.
fn set_send_retry_waiting(status: &Mutex<LibraryTransferSendStatus>, error: String) {
    if let Ok(mut status) = status.lock() {
        status.state = LibraryTransferSendState::Waiting;
        status.receiver_progress = None;
        status.error = Some(error);
    }
}

fn lock<T>(mutex: &Mutex<T>) -> Result<std::sync::MutexGuard<'_, T>, String> {
    mutex
        .lock()
        .map_err(|_| "Library transfer state is unavailable".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn pairing_proof_is_bound_to_channel_and_role() {
        let code = "2345ABCD";
        let first = pairing_proof(code, b"first channel", SENDER_ROLE).unwrap();

        assert_eq!(
            first,
            pairing_proof(code, b"first channel", SENDER_ROLE).unwrap()
        );
        assert_ne!(
            first,
            pairing_proof(code, b"other channel", SENDER_ROLE).unwrap()
        );
        assert_ne!(
            first,
            pairing_proof(code, b"first channel", RECEIVER_ROLE).unwrap()
        );
        assert_eq!(normalize_code("2345-abcd").unwrap(), code);
    }

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
