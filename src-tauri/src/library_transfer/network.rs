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
//! interruption may reconnect with the same code. Background transfer remains
//! outside this module.

use std::fs;
use std::io;
use std::net::{Ipv4Addr, Shutdown, SocketAddrV4, TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use rustls::ServerConfig;
use serde::{Deserialize, Serialize};
use tauri::State;

use super::{
    build_library_package, emit_transfer_progress, import_library_package, transfer_temp_path,
    LibraryTransferError, LibraryTransferExportRequest, LibraryTransferImportResult,
    LibraryTransferProgress, LibraryTransferResult,
};

mod security;
mod transport;

use security::{
    display_code, local_ipv4, new_pairing_code, normalize_code, parse_local_address,
    server_tls_config,
};
use transport::{
    receive_package, receive_resume_path, send_package, write_receiver_message, ReceiverMessage,
    SendAttemptError,
};

const SESSION_TIMEOUT: Duration = Duration::from_secs(10 * 60);

#[derive(Clone, Default)]
pub struct LibraryTransferState {
    send: Arc<Mutex<Option<SendSession>>>,
}

#[derive(Clone)]
struct SendSession {
    cancel: Arc<AtomicBool>,
    active_socket: Arc<Mutex<Option<TcpStream>>>,
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

/// Prepare one package, bind a foreground listener, and return the address and
/// one-use code before a receiver connects. A second active send is rejected so
/// two callers cannot replace each other's package or session credentials.
#[tauri::command]
pub async fn library_transfer_send_start(
    app: tauri::AppHandle,
    state: State<'_, LibraryTransferState>,
    request: Option<LibraryTransferExportRequest>,
) -> LibraryTransferResult<LibraryTransferSendStatus> {
    let state = state.inner().clone();
    let request = request.unwrap_or_default();
    tauri::async_runtime::spawn_blocking(move || {
        start_send(app, state, request.document_ids, request.audiobook_ids)
    })
    .await
    .map_err(|err| format!("Library send task failed: {err}"))?
}

#[tauri::command]
pub fn library_transfer_send_status(
    state: State<'_, LibraryTransferState>,
) -> LibraryTransferResult<Option<LibraryTransferSendStatus>> {
    let send = lock(&state.send)?;
    Ok(send
        .as_ref()
        .map(|session| lock(&session.status).map(|status| status.clone()))
        .transpose()?)
}

/// Stop a waiting or active foreground sender. The worker owns package cleanup
/// so cancellation cannot remove a file while another thread is reading it.
#[tauri::command]
pub fn library_transfer_send_cancel(
    state: State<'_, LibraryTransferState>,
) -> LibraryTransferResult<()> {
    let send = lock(&state.send)?;
    if let Some(session) = send.as_ref() {
        session.cancel.store(true, Ordering::Relaxed);
        if let Some(socket) = take_active_socket(&session.active_socket) {
            let _ = socket.shutdown(Shutdown::Both);
        }
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
) -> LibraryTransferResult<LibraryTransferImportResult> {
    tauri::async_runtime::spawn_blocking(move || receive_library(app, request))
        .await
        .map_err(|err| format!("Library receive task failed: {err}"))?
}

/// Build the reusable package before exposing session credentials, then hand
/// listener ownership to the background sender so command return is immediate.
fn start_send(
    app: tauri::AppHandle,
    state: LibraryTransferState,
    document_ids: Option<Vec<String>>,
    audiobook_ids: Vec<String>,
) -> LibraryTransferResult<LibraryTransferSendStatus> {
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
    let prepared =
        build_library_package(&app, &package_path, document_ids.as_deref(), &audiobook_ids);
    let prepared = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            let _ = fs::remove_file(&package_path);
            return Err(error);
        }
    };
    let setup = (|| {
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
        let active_socket = Arc::new(Mutex::new(None));
        *lock(&state.send)? = Some(SendSession {
            cancel: Arc::clone(&cancel),
            active_socket: Arc::clone(&active_socket),
            status: Arc::clone(&status),
        });
        Ok::<_, LibraryTransferError>((listener, tls_config, code, status, cancel, active_socket))
    })();
    let (listener, tls_config, code, status, cancel, active_socket) = match setup {
        Ok(setup) => setup,
        Err(error) => {
            let _ = fs::remove_file(&package_path);
            return Err(error);
        }
    };

    let thread_status = Arc::clone(&status);
    thread::spawn(move || {
        run_sender(
            listener,
            tls_config,
            &code,
            &package_path,
            &thread_status,
            &cancel,
            &active_socket,
        );
        let _ = fs::remove_file(package_path);
    });
    Ok(lock(&status).map(|status| status.clone())?)
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
    active_socket: &Mutex<Option<TcpStream>>,
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
                if let Err(error) = track_active_socket(&stream, active_socket) {
                    set_send_status(status, LibraryTransferSendState::Failed, Some(error));
                    return;
                }
                if cancel.load(Ordering::Relaxed) {
                    if let Some(socket) = take_active_socket(active_socket) {
                        let _ = socket.shutdown(Shutdown::Both);
                    }
                    set_send_status(status, LibraryTransferSendState::Cancelled, None);
                    return;
                }
                set_send_active(status);
                let result = send_package(
                    stream,
                    Arc::clone(&tls_config),
                    code,
                    package_path,
                    status,
                    cancel,
                );
                let _ = take_active_socket(active_socket);
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

/// Keep the authenticated stream open across package import so every target-side
/// restore phase, final success, or failure reaches the waiting source device.
fn receive_library(
    app: tauri::AppHandle,
    request: LibraryTransferReceiveRequest,
) -> LibraryTransferResult<LibraryTransferImportResult> {
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
                let retryable = error.is_retryable();
                let _ =
                    write_receiver_message(&mut stream, &ReceiverMessage::Failed(error.clone()));
                if !retryable {
                    let _ = fs::remove_file(&temp_path);
                }
                Err(error)
            }
        }
    })()
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

/// Keep a clone solely so the command thread can interrupt blocking TLS I/O
/// without taking ownership from the sender worker.
fn track_active_socket(
    socket: &TcpStream,
    active_socket: &Mutex<Option<TcpStream>>,
) -> Result<(), String> {
    let cancellation_socket = socket
        .try_clone()
        .map_err(|error| format!("Failed to prepare library send cancellation: {error}"))?;
    *lock(active_socket)? = Some(cancellation_socket);
    Ok(())
}

/// Take the cancellation-only socket clone so shutting it down wakes whichever
/// blocking TLS phase currently owns the original stream.
fn take_active_socket(active_socket: &Mutex<Option<TcpStream>>) -> Option<TcpStream> {
    active_socket.lock().ok()?.take()
}

fn lock<T>(mutex: &Mutex<T>) -> Result<std::sync::MutexGuard<'_, T>, String> {
    mutex
        .lock()
        .map_err(|_| "Library transfer state is unavailable".to_string())
}
