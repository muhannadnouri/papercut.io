//! Foreground, one-use library transfer over the local network.
//!
//! High-level flow:
//! 1. The source builds the same checked `.papercut-library` package used by
//!    file export, binds an IPv4 listener, and displays its address plus a
//!    random one-use code.
//! 2. The target enters those values. Both peers establish ephemeral TLS, then
//!    use SPAKE2 to derive a shared key from the code and mutually confirm it
//!    against that exact TLS session. This authenticates the self-signed
//!    connection without a permanent key or certificate warning.
//! 3. The target reports how many bytes of the authenticated session it already
//!    has, then retains that partial package if the connection drops. Retrying
//!    with the same address and code continues from that byte offset.
//! 4. Once all bytes arrive, the target invokes the normal package importer and
//!    forwards its verification and restore progress to the source. The source
//!    reports completion only after the target confirms that import finished.
//!
//! Sessions stay foreground-only and expire after ten minutes. Malformed traffic
//! before key confirmation leaves the sender waiting, while an incorrect
//! confirmation consumes the code to prevent online guessing. An authenticated
//! interruption may reconnect with the same code. Background transfer remains
//! outside this module.

use std::fs;
use std::io;
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use rustls::ServerConfig;
use serde::{Deserialize, Serialize};
use tauri::State;

use super::diagnostics::{Diagnostics, Trace};
use super::{
    build_library_package, create_transfer_temp, emit_transfer_progress, import_library_package,
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

pub(super) fn protocol_version() -> &'static str {
    std::str::from_utf8(security::PROTOCOL_MAGIC).unwrap_or("unknown")
}

const SESSION_TIMEOUT: Duration = Duration::from_secs(10 * 60);

#[derive(Clone, Default)]
pub struct LibraryTransferState {
    pub(super) diagnostics: Diagnostics,
    send: Arc<Mutex<Option<SendSession>>>,
    send_preparing: Arc<AtomicBool>,
    restoring: Arc<AtomicBool>,
}

impl LibraryTransferState {
    /// Reserve package preparation before entering the blocking pool so two
    /// native callers cannot both pass the active-session check.
    fn begin_send_preparation(&self) -> LibraryTransferResult<TransferOperationReservation> {
        reserve_operation(
            &self.send_preparing,
            "A library send is already being prepared",
        )
    }

    /// Serialize file import and LAN receive at the Rust boundary; the UI busy
    /// state remains feedback rather than the authority protecting library writes.
    /// ponytail: one slot covers staging and restore; split those phases only if
    /// concurrent transfer intake becomes a real product requirement.
    pub(super) fn begin_restore(&self) -> LibraryTransferResult<TransferOperationReservation> {
        reserve_operation(
            &self.restoring,
            "Another library transfer import is already running",
        )
    }
}

/// Releases a native operation slot on success, error, cancellation, or panic.
pub(super) struct TransferOperationReservation {
    active: Arc<AtomicBool>,
}

impl Drop for TransferOperationReservation {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
    }
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
    let reservation = state.begin_send_preparation()?;
    let request = request.unwrap_or_default();
    let trace = state.diagnostics.begin("send");
    let worker_trace = trace.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _reservation = reservation;
        start_send(
            app,
            state,
            request.document_ids,
            request.audiobook_ids,
            worker_trace,
        )
    })
    .await
    .map_err(|err| LibraryTransferError::message(format!("Library send task failed: {err}")))
    .and_then(|result| result);
    if result.is_err() {
        trace.finish(result.as_ref().err());
    }
    result
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
    state: State<'_, LibraryTransferState>,
    request: LibraryTransferReceiveRequest,
) -> LibraryTransferResult<LibraryTransferImportResult> {
    let reservation = state.begin_restore()?;
    let trace = state.diagnostics.begin("receive");
    let worker_trace = trace.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _reservation = reservation;
        receive_library(app, request, &worker_trace)
    })
    .await
    .map_err(|err| LibraryTransferError::message(format!("Library receive task failed: {err}")))
    .and_then(|result| result);
    trace.finish(result.as_ref().err());
    result
}

fn reserve_operation(
    active: &Arc<AtomicBool>,
    conflict: &str,
) -> LibraryTransferResult<TransferOperationReservation> {
    if active.swap(true, Ordering::AcqRel) {
        return Err(conflict.into());
    }
    Ok(TransferOperationReservation {
        active: Arc::clone(active),
    })
}

/// Build the reusable package before exposing session credentials, then hand
/// listener ownership to the background sender so command return is immediate.
fn start_send(
    app: tauri::AppHandle,
    state: LibraryTransferState,
    document_ids: Option<Vec<String>>,
    audiobook_ids: Vec<String>,
    trace: Trace,
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

    trace.stage("package-preparation");
    let mut package = create_transfer_temp(&app, "lan-send")?;
    let package_path = package.path().to_path_buf();
    let prepared = build_library_package(
        &app,
        package.file_mut(),
        &package_path,
        document_ids.as_deref(),
        &audiobook_ids,
    )?;
    let setup = (|| {
        let package_bytes = fs::metadata(&package_path)
            .map_err(|err| format!("Failed to inspect prepared library package: {err}"))?
            .len();
        trace.stage("address-selection");
        let address = local_ipv4()?;
        trace.stage("listener-bind");
        let listener = TcpListener::bind((address, 0)).map_err(|err| {
            trace.io_error(&err);
            format!("Failed to start local library transfer: {err}")
        })?;
        listener
            .set_nonblocking(true)
            .map_err(|err| format!("Failed to configure local library transfer: {err}"))?;
        let local_address = listener.local_addr().map_err(|error| {
            trace.io_error(&error);
            format!("Failed to read local library transfer address: {error}")
        })?;
        if let std::net::SocketAddr::V4(address) = local_address {
            trace.endpoint(address);
        }
        let address = local_address.to_string();
        let code = new_pairing_code()?;
        trace.stage("certificate-setup");
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
    let (listener, tls_config, code, status, cancel, active_socket) = setup?;

    trace.stage("waiting");
    let thread_status = Arc::clone(&status);
    thread::spawn(move || {
        run_sender(
            listener,
            tls_config,
            &code,
            package.path(),
            &thread_status,
            &cancel,
            &active_socket,
            &trace,
        );
    });
    Ok(lock(&status).map(|status| status.clone())?)
}

/// Keep one package/code alive for authenticated resume attempts until import
/// completes, cancellation, or expiry. Pre-authentication transport noise is
/// ignored, but a complete incorrect proof consumes the one-use session.
fn run_sender(
    listener: TcpListener,
    tls_config: Arc<ServerConfig>,
    code: &str,
    package_path: &Path,
    status: &Arc<Mutex<LibraryTransferSendStatus>>,
    cancel: &AtomicBool,
    active_socket: &Mutex<Option<TcpStream>>,
    trace: &Trace,
) {
    let deadline = Instant::now() + SESSION_TIMEOUT;
    loop {
        if cancel.load(Ordering::Relaxed) {
            trace.cancelled();
            set_send_status(status, LibraryTransferSendState::Cancelled, None);
            return;
        }
        if Instant::now() >= deadline {
            trace.failure("session-expired");
            trace.finish(Some(&LibraryTransferError::message("expired")));
            set_send_status(
                status,
                LibraryTransferSendState::Failed,
                Some("Library send timed out".into()),
            );
            return;
        }
        match listener.accept() {
            Ok((stream, _)) => {
                trace.stage("tcp-accepted");
                if let Err(error) = track_active_socket(&stream, active_socket) {
                    trace.finish(Some(&LibraryTransferError::message(&error)));
                    set_send_status(status, LibraryTransferSendState::Failed, Some(error));
                    return;
                }
                if cancel.load(Ordering::Relaxed) {
                    if let Some(socket) = take_active_socket(active_socket) {
                        let _ = socket.shutdown(Shutdown::Both);
                    }
                    trace.cancelled();
                    set_send_status(status, LibraryTransferSendState::Cancelled, None);
                    return;
                }
                let result = send_package(
                    stream,
                    Arc::clone(&tls_config),
                    code,
                    package_path,
                    status,
                    cancel,
                    trace,
                );
                let _ = take_active_socket(active_socket);
                if cancel.load(Ordering::Relaxed) {
                    trace.cancelled();
                    set_send_status(status, LibraryTransferSendState::Cancelled, None);
                    return;
                }
                match result {
                    Ok(()) => {
                        trace.finish(None);
                        set_send_status(status, LibraryTransferSendState::Complete, None);
                        return;
                    }
                    Err(SendAttemptError::Unauthenticated) => {
                        trace.note("waiting", "unauthenticated-connection");
                        set_send_waiting(status, None);
                    }
                    Err(SendAttemptError::Fatal(error)) => {
                        trace.finish(Some(&LibraryTransferError::message(&error)));
                        set_send_status(status, LibraryTransferSendState::Failed, Some(error));
                        return;
                    }
                    Err(SendAttemptError::Retryable(error)) => {
                        trace.note("waiting", "retryable-interruption");
                        set_send_waiting(status, Some(error));
                    }
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(100));
            }
            Err(error) => {
                trace.io_error(&error);
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
    trace: &Trace,
) -> LibraryTransferResult<LibraryTransferImportResult> {
    trace.stage("address-validation");
    emit_transfer_progress(&app, "receive", "connecting", None, None, None);
    let address = parse_local_address(&request.address)?;
    trace.stage("code-validation");
    let code = normalize_code(&request.code)?;
    trace.stage("staging");
    let temp_path = receive_resume_path(&app, address, &code)?;
    (|| {
        let mut stream = receive_package(
            address,
            &code,
            &temp_path,
            |processed, total| {
                emit_transfer_progress(
                    &app,
                    "receive",
                    "receiving",
                    Some((processed, total)),
                    None,
                    None,
                );
            },
            trace,
        )?;

        trace.stage("verification");
        let import_result = {
            let mut forward = |progress: &LibraryTransferProgress| {
                if let Err(error) = write_receiver_message(
                    &mut stream,
                    &ReceiverMessage::Progress(progress.clone()),
                ) {
                    trace.io_error(&error);
                }
            };
            import_library_package(&app, &temp_path, "receive", Some(&mut forward))
        };
        match import_result {
            Ok(result) => {
                trace.import_result(&result);
                trace.stage("acknowledgement");
                if let Err(error) = write_receiver_message(&mut stream, &ReceiverMessage::Complete)
                {
                    trace.io_error(&error);
                }
                trace.stage("cleanup");
                if let Err(error) = fs::remove_file(&temp_path) {
                    trace.io_error(&error);
                }
                Ok(result)
            }
            Err(error) => {
                let retryable = error.is_retryable();
                if let Err(error) =
                    write_receiver_message(&mut stream, &ReceiverMessage::Failed(error.clone()))
                {
                    trace.io_error(&error);
                }
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

/// Return a failed attempt to the pairing screen. Only authenticated transfer
/// interruptions surface an error; unrelated pre-authentication traffic does not.
fn set_send_waiting(status: &Mutex<LibraryTransferSendStatus>, error: Option<String>) {
    if let Ok(mut status) = status.lock() {
        status.state = LibraryTransferSendState::Waiting;
        status.receiver_progress = None;
        status.error = error;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_operation_reservations_release_after_every_owner() {
        let state = LibraryTransferState::default();
        let send = state
            .begin_send_preparation()
            .expect("first send preparation");
        assert!(state.begin_send_preparation().is_err());
        drop(send);
        assert!(state.begin_send_preparation().is_ok());

        let restore = state.begin_restore().expect("first restore");
        assert!(state.begin_restore().is_err());
        drop(restore);
        assert!(state.begin_restore().is_ok());
    }
}
