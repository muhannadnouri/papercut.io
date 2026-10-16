//! Foreground library transfer: explicit endpoints, native session ownership and one-use pairing.
use std::fs;
use std::io;
use std::net::TcpStream;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rustls::ServerConfig;
use serde::{Deserialize, Serialize};
use tauri::State;

use super::diagnostics::{Diagnostics, Trace};
use super::{
    build_library_package, create_transfer_temp, emit_transfer_progress, import_library_package,
    LibraryTransferError, LibraryTransferExportRequest, LibraryTransferImportResult,
    LibraryTransferProgress, LibraryTransferResult,
};

mod endpoints;
mod security;
mod session;
mod transport;
use endpoints::{ConnectionOptions, TransferInterface, DEFAULT_PORT};
use security::{display_code, new_pairing_code, normalize_code, server_tls_config};
use session::{NetworkWatch, TransferControl};
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
    receive: Arc<Mutex<Option<ReceiveSession>>>,
    next_id: Arc<AtomicU64>,
    backgrounded: Arc<Mutex<bool>>,
    visibility_known: Arc<AtomicBool>,
    visibility_epoch: Arc<AtomicU64>,
    send_preparing: Arc<AtomicBool>,
    restoring: Arc<AtomicBool>,
}

impl LibraryTransferState {
    fn begin_send_preparation(&self) -> LibraryTransferResult<TransferOperationReservation> {
        reserve_operation(
            &self.send_preparing,
            "A library send is already being prepared",
        )
    }
    pub(super) fn begin_restore(&self) -> LibraryTransferResult<TransferOperationReservation> {
        reserve_operation(
            &self.restoring,
            "Another library transfer import is already running",
        )
    }
    fn check_foreground(&self) -> LibraryTransferResult<u64> {
        if cfg!(mobile) && !self.visibility_known.load(Ordering::Acquire) {
            return Err(
                "App visibility tracking is unavailable. Restart Papercut or use a transfer file."
                    .into(),
            );
        }
        let backgrounded = lock(&self.backgrounded)?;
        if *backgrounded {
            return Err("Open Papercut to start a transfer.".into());
        }
        Ok(self.visibility_epoch.load(Ordering::Acquire))
    }

    fn foreground_guard(
        &self,
        epoch: u64,
    ) -> LibraryTransferResult<std::sync::MutexGuard<'_, bool>> {
        let backgrounded = lock(&self.backgrounded)?;
        if *backgrounded || self.visibility_epoch.load(Ordering::Acquire) != epoch {
            return Err("App visibility changed. Start a new transfer.".into());
        }
        Ok(backgrounded)
    }

    pub(super) fn receive_session_id(&self) -> Option<u64> {
        self.receive
            .lock()
            .ok()?
            .as_ref()?
            .status
            .lock()
            .ok()
            .map(|s| s.id)
    }
    pub(crate) fn set_backgrounded(&self, backgrounded: bool) {
        let Ok(mut current) = self.backgrounded.lock() else {
            return;
        };
        *current = backgrounded;
        self.visibility_known.store(true, Ordering::Release);
        if backgrounded {
            self.cancel_network_sessions("backgrounded");
        }
    }

    pub(crate) fn cancel_network_sessions(&self, reason: &'static str) {
        self.visibility_epoch.fetch_add(1, Ordering::AcqRel);
        if let Ok(send) = self.send.lock() {
            if let Some(s) = send.as_ref() {
                s.control.cancel(reason);
            }
        }
        if let Ok(receive) = self.receive.lock() {
            if let Some(s) = receive.as_ref() {
                s.control.cancel(reason);
            }
        }
    }

    fn cancel_send(&self, id: Option<u64>) -> LibraryTransferResult<()> {
        if let Some(session) = lock(&self.send)?.as_ref() {
            if id.is_none_or(|id| lock(&session.status).is_ok_and(|s| s.id == id)) {
                session.control.cancel("cancelled");
            }
        }
        Ok(())
    }

    fn cancel_receive(&self, id: u64) -> LibraryTransferResult<bool> {
        if let Some(session) = lock(&self.receive)?.as_ref() {
            if lock(&session.status)?.id == id {
                return Ok(session.control.cancel("cancelled"));
            }
        }
        Ok(false)
    }
}

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
    control: Arc<TransferControl>,
    status: Arc<Mutex<LibraryTransferSendStatus>>,
}
#[derive(Clone)]
struct ReceiveSession {
    control: Arc<TransferControl>,
    status: Arc<Mutex<LibraryTransferReceiveStatus>>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryTransferSendStatus {
    id: u64,
    state: LibraryTransferSendState,
    address: String,
    addresses: Vec<String>,
    code: String,
    listener_ready: bool,
    expires_at_ms: Option<u64>,
    reason: Option<&'static str>,
    documents: usize,
    audiobooks: usize,
    package_bytes: u64,
    bytes_transferred: u64,
    receiver_progress: Option<LibraryTransferProgress>,
    error: Option<String>,
}
#[derive(Clone, Copy, Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum LibraryTransferSendState {
    Preparing,
    Binding,
    #[default]
    Waiting,
    Sending,
    Complete,
    Cancelled,
    Failed,
}
impl LibraryTransferSendState {
    fn active(self) -> bool {
        matches!(
            self,
            Self::Preparing | Self::Binding | Self::Waiting | Self::Sending
        )
    }
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryTransferReceiveStatus {
    id: u64,
    phase: &'static str,
    cancellable: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryTransferReceiveRequest {
    address: String,
    code: String,
    interface_id: Option<String>,
}

#[tauri::command]
pub fn library_transfer_interfaces() -> LibraryTransferResult<Vec<TransferInterface>> {
    endpoints::interfaces().map_err(Into::into)
}

#[tauri::command]
pub async fn library_transfer_send_start(
    app: tauri::AppHandle,
    state: State<'_, LibraryTransferState>,
    request: Option<LibraryTransferExportRequest>,
    connection: Option<ConnectionOptions>,
) -> LibraryTransferResult<LibraryTransferSendStatus> {
    let state = state.inner().clone();
    let visibility_epoch = state.check_foreground()?;
    let reservation = state.begin_send_preparation()?;
    let connection = connection.unwrap_or_default();
    let network = endpoints::select_interface(
        &endpoints::interfaces()?,
        connection.interface_id.as_deref(),
    )?;
    let port = connection.port.unwrap_or(DEFAULT_PORT);
    let session = SendSession {
        control: Arc::new(TransferControl::default()),
        status: Arc::new(Mutex::new(LibraryTransferSendStatus {
            id: state.next_id.fetch_add(1, Ordering::Relaxed) + 1,
            state: LibraryTransferSendState::Preparing,
            ..Default::default()
        })),
    };
    {
        let _foreground = state.foreground_guard(visibility_epoch)?;
        let mut current = lock(&state.send)?;
        if current
            .as_ref()
            .is_some_and(|s| s.status.lock().map(|s| s.state.active()).unwrap_or(true))
        {
            return Err("A library send is already active".into());
        }
        *current = Some(session.clone());
    }
    let trace = state.diagnostics.begin("send");
    let worker_trace = trace.clone();
    let worker_session = session.clone();
    let request = request.unwrap_or_default();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _reservation = reservation;
        let watch = NetworkWatch::start(worker_session.control.clone(), network.clone());
        start_send(
            app,
            request,
            network,
            port,
            worker_session,
            worker_trace,
            watch,
        )
    })
    .await
    .map_err(|err| LibraryTransferError::message(format!("Library send task failed: {err}")))
    .and_then(|r| r);
    if let Err(error) = &result {
        if session.control.check().is_err() {
            trace.note("cleanup", session.control.reason());
            trace.cancelled();
            set_send_status(&session.status, LibraryTransferSendState::Cancelled, None);
            return Err(cancelled_error());
        } else {
            trace.finish(Some(error));
            set_send_status(
                &session.status,
                LibraryTransferSendState::Failed,
                Some(error.to_string()),
            );
        }
    }
    result
}

#[tauri::command]
pub fn library_transfer_send_status(
    state: State<'_, LibraryTransferState>,
) -> LibraryTransferResult<Option<LibraryTransferSendStatus>> {
    lock(&state.send)?
        .as_ref()
        .map(|s| lock(&s.status).map(|s| s.clone()))
        .transpose()
}

#[tauri::command]
pub fn library_transfer_send_cancel(
    state: State<'_, LibraryTransferState>,
    id: Option<u64>,
) -> LibraryTransferResult<()> {
    state.cancel_send(id)
}

#[tauri::command]
pub fn library_transfer_receive_status(
    state: State<'_, LibraryTransferState>,
) -> LibraryTransferResult<Option<LibraryTransferReceiveStatus>> {
    lock(&state.receive)?
        .as_ref()
        .map(|s| lock(&s.status).map(|s| s.clone()))
        .transpose()
}

#[tauri::command]
pub fn library_transfer_receive_cancel(
    state: State<'_, LibraryTransferState>,
    id: u64,
) -> LibraryTransferResult<bool> {
    state.cancel_receive(id)
}

#[tauri::command]
pub async fn library_transfer_receive(
    app: tauri::AppHandle,
    state: State<'_, LibraryTransferState>,
    request: LibraryTransferReceiveRequest,
) -> LibraryTransferResult<LibraryTransferImportResult> {
    let visibility_epoch = state.check_foreground()?;
    let reservation = state.begin_restore()?;
    let trace = state.diagnostics.begin("receive");
    let validated: LibraryTransferResult<_> = (|| {
        trace.stage("address-selection");
        let network = endpoints::select_interface(
            &endpoints::interfaces()?,
            request.interface_id.as_deref(),
        )?;
        trace.stage("address-validation");
        let address = endpoints::parse_endpoint(&request.address, &network)?;
        trace.stage("code-validation");
        let code = normalize_code(&request.code)?;
        Ok((network, address, code))
    })();
    let (network, address, code) = validated.inspect_err(|error| trace.finish(Some(error)))?;
    let session = ReceiveSession {
        control: Arc::new(TransferControl::default()),
        status: Arc::new(Mutex::new(LibraryTransferReceiveStatus {
            id: state.next_id.fetch_add(1, Ordering::Relaxed) + 1,
            phase: "connecting",
            cancellable: true,
        })),
    };
    {
        let _foreground = state
            .foreground_guard(visibility_epoch)
            .inspect_err(|error| trace.finish(Some(error)))?;
        *lock(&state.receive)? = Some(session.clone());
    }
    let worker_trace = trace.clone();
    let worker_session = session.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _reservation = reservation;
        let _watch = NetworkWatch::start(worker_session.control.clone(), network.clone());
        receive_library(app, address, code, &network, &worker_session, &worker_trace)
    })
    .await
    .map_err(|err| LibraryTransferError::message(format!("Library receive task failed: {err}")))
    .and_then(|r| r);
    let result = if session.control.check().is_err() {
        trace.note("cleanup", session.control.reason());
        trace.cancelled();
        Err(cancelled_error())
    } else {
        trace.finish(result.as_ref().err());
        result
    };
    session.control.clear_socket();
    if let Ok(mut s) = session.status.lock() {
        s.phase = if result.is_ok() {
            "complete"
        } else if session.control.check().is_err() {
            "cancelled"
        } else {
            "failed"
        };
        s.cancellable = false;
    }
    result
}

fn cancelled_error() -> LibraryTransferError {
    let mut error = LibraryTransferError::message("Library transfer cancelled");
    error.code = "TRANSFER_CANCELLED".into();
    error
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

fn start_send(
    app: tauri::AppHandle,
    request: LibraryTransferExportRequest,
    network: TransferInterface,
    port: u16,
    session: SendSession,
    trace: Trace,
    watch: NetworkWatch,
) -> LibraryTransferResult<LibraryTransferSendStatus> {
    let control = &session.control;
    control.check().map_err(|e| e.to_string())?;
    trace.stage("package-preparation");
    let mut package = create_transfer_temp(&app, "lan-send")?;
    let path = package.path().to_path_buf();
    // shortcut: preparation cancellation waits for this synchronous build; thread cancellation through packaging if large-library latency warrants it.
    let prepared = build_library_package(
        &app,
        package.file_mut(),
        &path,
        request.document_ids.as_deref(),
        &request.audiobook_ids,
    )?;
    control.check().map_err(|e| e.to_string())?;
    trace.stage("address-selection");
    if !endpoints::still_available(&network) {
        return Err("Selected network changed. Start a new transfer.".into());
    }
    lock(&session.status)?.state = LibraryTransferSendState::Binding;
    trace.stage("listener-bind");
    let listeners = endpoints::bind_listeners(&network, port).map_err(|e| {
        trace.io_error(&e);
        let mut error =
            LibraryTransferError::message(format!("Could not bind the selected network port: {e}"));
        if e.kind() == io::ErrorKind::AddrInUse {
            error.code = "TRANSFER_PORT_IN_USE".into();
        }
        error
    })?;
    let addresses = listeners
        .iter()
        .map(|l| l.local_addr().map(|a| a.to_string()))
        .collect::<io::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    for listener in &listeners {
        trace.endpoint(listener.local_addr().map_err(|e| e.to_string())?);
    }
    trace.stage("certificate-setup");
    let tls = server_tls_config()?;
    let code = new_pairing_code()?;
    control.check().map_err(|e| e.to_string())?;
    let deadline = Instant::now() + SESSION_TIMEOUT;
    {
        let mut s = lock(&session.status)?;
        s.state = LibraryTransferSendState::Waiting;
        s.address = addresses[0].clone();
        s.addresses = addresses;
        s.code = display_code(&code);
        s.listener_ready = true;
        s.expires_at_ms = Some(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64
                + SESSION_TIMEOUT.as_millis() as u64,
        );
        s.documents = prepared.documents;
        s.audiobooks = prepared.audiobooks;
        s.package_bytes = fs::metadata(&path).map_err(|e| e.to_string())?.len();
    }
    trace.stage("waiting");
    let thread_session = session.clone();
    thread::spawn(move || {
        let _watch = watch;
        run_sender(
            move || {
                for listener in &listeners {
                    match listener.accept() {
                        Ok((socket, _)) => return Ok(socket),
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                        Err(e) => return Err(e),
                    }
                }
                Err(io::ErrorKind::WouldBlock.into())
            },
            tls,
            &code,
            package.path(),
            &thread_session.status,
            &thread_session.control,
            &trace,
            deadline,
        );
    });
    Ok(lock(&session.status)?.clone())
}

fn run_sender(
    mut accept: impl FnMut() -> io::Result<TcpStream>,
    tls_config: Arc<ServerConfig>,
    code: &str,
    package_path: &Path,
    status: &Arc<Mutex<LibraryTransferSendStatus>>,
    control: &TransferControl,
    trace: &Trace,
    deadline: Instant,
) {
    loop {
        if control.check().is_err() {
            if let Ok(mut s) = status.lock() {
                s.reason = Some(control.reason());
            }
            trace.note("cleanup", control.reason());
            trace.cancelled();
            drop(accept);
            set_send_status(status, LibraryTransferSendState::Cancelled, None);
            return;
        }
        if Instant::now() >= deadline {
            trace.failure("session-expired");
            trace.finish(Some(&LibraryTransferError::message("expired")));
            if let Ok(mut s) = status.lock() {
                s.reason = Some("session-expired");
            }
            drop(accept);
            set_send_status(
                status,
                LibraryTransferSendState::Failed,
                Some("Library send timed out".into()),
            );
            return;
        }
        match accept() {
            Ok(stream) => {
                trace.stage("tcp-accepted");
                if let Err(e) = control.track(&stream) {
                    if control.check().is_err() {
                        continue;
                    }
                    trace.finish(Some(&LibraryTransferError::message(e.to_string())));
                    drop(accept);
                    set_send_status(
                        status,
                        LibraryTransferSendState::Failed,
                        Some(e.to_string()),
                    );
                    return;
                }
                let result = send_package(
                    stream,
                    Arc::clone(&tls_config),
                    code,
                    package_path,
                    status,
                    &control.cancelled,
                    trace,
                );
                control.clear_socket();
                if control.check().is_err() {
                    continue;
                }
                match result {
                    Ok(()) => {
                        trace.finish(None);
                        drop(accept);
                        set_send_status(status, LibraryTransferSendState::Complete, None);
                        return;
                    }
                    Err(SendAttemptError::Unauthenticated) => {
                        trace.note("waiting", "unauthenticated-connection");
                        set_send_waiting(status, None);
                    }
                    Err(SendAttemptError::Fatal(e)) => {
                        trace.finish(Some(&LibraryTransferError::message(&e)));
                        drop(accept);
                        set_send_status(status, LibraryTransferSendState::Failed, Some(e));
                        return;
                    }
                    Err(SendAttemptError::Retryable(e)) => {
                        trace.note("waiting", "retryable-interruption");
                        set_send_waiting(status, Some(e));
                    }
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(100))
            }
            Err(e) => {
                trace.io_error(&e);
                trace.finish(Some(&LibraryTransferError::message("listener failed")));
                drop(accept);
                set_send_status(
                    status,
                    LibraryTransferSendState::Failed,
                    Some(format!("Local library transfer failed: {e}")),
                );
                return;
            }
        }
    }
}

fn receive_library(
    app: tauri::AppHandle,
    address: std::net::SocketAddr,
    code: String,
    network: &TransferInterface,
    session: &ReceiveSession,
    trace: &Trace,
) -> LibraryTransferResult<LibraryTransferImportResult> {
    if !endpoints::still_available(network) {
        session.control.cancel("network-changed");
    }
    session.control.check().map_err(|e| e.to_string())?;
    emit_transfer_progress(&app, "receive", "connecting", None, None, None);
    let temp_path = receive_resume_path(&app, address, &code)?;
    let mut stream = receive_package(
        address,
        &code,
        &temp_path,
        |processed, total| {
            if let Ok(mut s) = session.status.lock() {
                s.phase = "receiving";
            }
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
        &session.control,
        Some(network),
    )?;
    session.control.begin_import().map_err(|e| e.to_string())?;
    {
        let mut s = lock(&session.status)?;
        s.phase = "finishingImport";
        s.cancellable = false;
    }
    trace.stage("verification");
    let result = {
        let mut forward = |p: &LibraryTransferProgress| {
            if let Err(e) =
                write_receiver_message(&mut stream, &ReceiverMessage::Progress(p.clone()))
            {
                trace.io_error(&e);
            }
        };
        import_library_package(&app, &temp_path, "receive", Some(&mut forward))
    };
    match result {
        Ok(result) => {
            trace.import_result(&result);
            trace.stage("acknowledgement");
            if let Err(e) = write_receiver_message(&mut stream, &ReceiverMessage::Complete) {
                trace.io_error(&e);
            }
            trace.stage("cleanup");
            if let Err(e) = fs::remove_file(&temp_path) {
                trace.io_error(&e);
            }
            Ok(result)
        }
        Err(e) => {
            if let Err(io) =
                write_receiver_message(&mut stream, &ReceiverMessage::Failed(e.clone()))
            {
                trace.io_error(&io);
            }
            if !e.is_retryable() {
                let _ = fs::remove_file(&temp_path);
            }
            Err(e)
        }
    }
}

fn set_send_status(
    status: &Mutex<LibraryTransferSendStatus>,
    state: LibraryTransferSendState,
    error: Option<String>,
) {
    if let Ok(mut s) = status.lock() {
        s.state = state;
        s.error = error;
        if !state.active() {
            s.listener_ready = false;
            s.address.clear();
            s.addresses.clear();
            s.code.clear();
            s.expires_at_ms = None;
        }
    }
}
fn set_send_active(status: &Mutex<LibraryTransferSendStatus>) {
    if let Ok(mut s) = status.lock() {
        s.state = LibraryTransferSendState::Sending;
        s.receiver_progress = None;
        s.error = None;
    }
}
fn set_send_waiting(status: &Mutex<LibraryTransferSendStatus>, error: Option<String>) {
    if let Ok(mut s) = status.lock() {
        s.state = LibraryTransferSendState::Waiting;
        s.receiver_progress = None;
        s.error = error;
    }
}
fn lock<T>(mutex: &Mutex<T>) -> LibraryTransferResult<std::sync::MutexGuard<'_, T>> {
    mutex
        .lock()
        .map_err(|_| LibraryTransferError::message("Library transfer state is unavailable"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listener_failure_finishes_send_trace_and_preserves_safe_io_facts() {
        for error in [
            io::Error::from_raw_os_error(13),
            io::Error::new(io::ErrorKind::PermissionDenied, "private listener details"),
        ] {
            let kind = format!("{:?}", error.kind());
            let code = error.raw_os_error();
            let diagnostics = Diagnostics::default();
            let trace = diagnostics.begin("send");
            trace.stage("waiting");
            let status = Arc::new(Mutex::new(LibraryTransferSendStatus {
                state: LibraryTransferSendState::Waiting,
                address: String::new(),
                code: String::new(),
                documents: 0,
                audiobooks: 0,
                package_bytes: 0,
                bytes_transferred: 0,
                receiver_progress: None,
                error: None,
                ..Default::default()
            }));
            let mut calls = 0;
            let mut error = Some(error);
            run_sender(
                || {
                    calls += 1;
                    if calls == 1 {
                        Err(io::ErrorKind::WouldBlock.into())
                    } else {
                        Err(error.take().unwrap())
                    }
                },
                server_tls_config().unwrap(),
                "unused-code",
                Path::new("unused-package"),
                &status,
                &TransferControl::default(),
                &trace,
                Instant::now() + SESSION_TIMEOUT,
            );
            assert_eq!(calls, 2);
            let status = status.lock().unwrap();
            assert_eq!(status.state, LibraryTransferSendState::Failed);
            assert!(status
                .error
                .as_deref()
                .unwrap()
                .starts_with("Local library transfer failed:"));
            let snapshot = serde_json::to_value(diagnostics.snapshot().unwrap()).unwrap();
            let events = snapshot["events"].as_array().unwrap();
            assert_eq!(events.len(), 4);
            assert_eq!(events[2]["code"], "io-error");
            assert_eq!(events[2]["ioKind"], kind);
            assert_eq!(events[2]["osCode"], serde_json::json!(code));
            assert_eq!(events[3]["phase"], "waiting");
            assert_eq!(events[3]["code"], "failed");
            assert!(!snapshot.to_string().contains("private listener details"));
            trace.note("waiting", "late-event");
            assert_eq!(
                serde_json::to_value(diagnostics.snapshot().unwrap()).unwrap(),
                snapshot
            );
        }
    }

    fn sender(id: u64) -> SendSession {
        SendSession {
            control: Arc::new(TransferControl::default()),
            status: Arc::new(Mutex::new(LibraryTransferSendStatus {
                id,
                state: LibraryTransferSendState::Waiting,
                listener_ready: true,
                address: "127.0.0.1:53318".into(),
                addresses: vec!["127.0.0.1:53318".into()],
                code: "2345-ABCD-GHJK".into(),
                expires_at_ms: Some(100),
                ..Default::default()
            })),
        }
    }

    fn receiver(id: u64) -> ReceiveSession {
        ReceiveSession {
            control: Arc::new(TransferControl::default()),
            status: Arc::new(Mutex::new(LibraryTransferReceiveStatus {
                id,
                phase: "connecting",
                cancellable: true,
            })),
        }
    }

    #[test]
    fn stale_cancel_and_late_completion_cannot_change_the_next_generation() {
        let state = LibraryTransferState::default();
        let old = sender(1);
        let next = sender(2);
        *state.send.lock().unwrap() = Some(next.clone());
        state.cancel_send(Some(1)).unwrap();
        set_send_status(
            &old.status,
            LibraryTransferSendState::Failed,
            Some("late failure".into()),
        );
        assert!(next.control.check().is_ok());
        assert!(next.status.lock().unwrap().listener_ready);
        state.cancel_send(Some(2)).unwrap();
        assert!(next.control.check().is_err());
        let receive = receiver(4);
        *state.receive.lock().unwrap() = Some(receive.clone());
        assert!(!state.cancel_receive(3).unwrap());
        assert!(receive.control.check().is_ok());
        receive.control.begin_import().unwrap();
        assert!(!state.cancel_receive(4).unwrap());
        assert!(receive.control.check().is_ok());
    }

    #[test]
    fn backgrounding_cancels_network_work_but_does_not_cancel_restore_or_restart_sharing() {
        let state = LibraryTransferState::default();
        let send = sender(1);
        let receive = receiver(2);
        *state.send.lock().unwrap() = Some(send.clone());
        *state.receive.lock().unwrap() = Some(receive.clone());
        let epoch = state.check_foreground().unwrap();
        state.set_backgrounded(true);
        assert!(state.foreground_guard(epoch).is_err());
        assert!(state.check_foreground().is_err());
        assert_eq!(send.control.reason(), "backgrounded");
        assert!(send.control.check().is_err());
        assert!(receive.control.check().is_err());
        state.set_backgrounded(false);
        assert!(state.check_foreground().is_ok());
        assert!(state.foreground_guard(epoch).is_err());
        let fresh = state.check_foreground().unwrap();
        assert!(state.foreground_guard(fresh).is_ok());
        state.cancel_network_sessions("closed");
        assert!(state.foreground_guard(fresh).is_err());
        assert!(send.control.check().is_err());
        let importing = receiver(3);
        importing.control.begin_import().unwrap();
        *state.receive.lock().unwrap() = Some(importing.clone());
        state.set_backgrounded(true);
        assert!(importing.control.check().is_ok());
    }

    #[test]
    fn expiry_and_cancellation_release_listener_and_clear_every_pairing_field() {
        use std::net::TcpListener;
        for expired in [false, true] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let session = sender(1);
            if !expired {
                session.control.cancel("backgrounded");
            }
            let diagnostics = Diagnostics::default();
            let trace = diagnostics.begin("send");
            run_sender(
                move || listener.accept().map(|p| p.0),
                server_tls_config().unwrap(),
                "unused",
                Path::new("unused"),
                &session.status,
                &session.control,
                &trace,
                if expired {
                    Instant::now()
                } else {
                    Instant::now() + SESSION_TIMEOUT
                },
            );
            assert!(TcpListener::bind(address).is_ok());
            let status = session.status.lock().unwrap();
            assert!(!status.listener_ready);
            assert!(
                status.address.is_empty() && status.addresses.is_empty() && status.code.is_empty()
            );
            assert!(status.expires_at_ms.is_none());
            assert_eq!(
                status.reason,
                Some(if expired {
                    "session-expired"
                } else {
                    "backgrounded"
                })
            );
            assert_eq!(
                status.state,
                if expired {
                    LibraryTransferSendState::Failed
                } else {
                    LibraryTransferSendState::Cancelled
                }
            );
            assert!(serde_json::to_string(&diagnostics.snapshot().unwrap())
                .unwrap()
                .contains(status.reason.unwrap()));
        }
    }

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
