//! One foreground v4 session; connection and content roles are independent.
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::State;

use super::diagnostics::{Diagnostics, Trace};
use super::{
    LibraryTransferError, LibraryTransferExportRequest, LibraryTransferImportResult,
    LibraryTransferProgress, LibraryTransferResult,
};
pub(crate) mod discovery;
mod endpoints;
mod operation;
mod security;
mod session;
mod transport;
mod v4;
use endpoints::{TransferInterface, DEFAULT_PORT};
use session::TransferControl;
use v4::{Content, Mode, Phase, Side};

pub(super) fn protocol_version() -> &'static str {
    v4::protocol_version()
}

#[derive(Clone, Default)]
pub struct LibraryTransferState {
    pub(super) diagnostics: Diagnostics,
    current: Arc<Mutex<Option<Arc<Operation>>>>,
    next_id: Arc<AtomicU64>,
    active: Arc<AtomicBool>,
    backgrounded: Arc<Mutex<bool>>,
    visibility_known: Arc<AtomicBool>,
    visibility_epoch: Arc<AtomicU64>,
    discovery: Arc<Mutex<Option<discovery::Job>>>,
}

impl LibraryTransferState {
    pub(super) fn begin_restore(&self) -> LibraryTransferResult<TransferOperationReservation> {
        reserve_operation(&self.active)
    }
    fn check_foreground(&self) -> LibraryTransferResult<u64> {
        if cfg!(mobile) && !self.visibility_known.load(Ordering::Acquire) {
            return Err(
                "App visibility tracking is unavailable. Restart Papercut or use a transfer file."
                    .into(),
            );
        }
        if *lock(&self.backgrounded)? {
            return Err("Open Papercut to start a transfer.".into());
        }
        Ok(self.visibility_epoch.load(Ordering::Acquire))
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
        self.stop_discovery();
        if let Ok(current) = self.current.lock() {
            if let Some(operation) = current.as_ref() {
                operation.control.cancel(reason);
            }
        }
    }
    fn stop_discovery(&self) {
        if let Ok(mut job) = self.discovery.lock() {
            job.take();
        }
    }
    pub(super) fn receive_session_id(&self) -> Option<u64> {
        let current = self.current.lock().ok()?;
        let status = current.as_ref()?.status.lock().ok()?;
        (status.role == Content::Receiver && status.state.active()).then_some(status.id)
    }
    fn install(&self, operation: Arc<Operation>, epoch: u64) -> LibraryTransferResult<()> {
        let foreground = lock(&self.backgrounded)?;
        if *foreground || self.visibility_epoch.load(Ordering::Acquire) != epoch {
            return Err("App visibility changed. Start a new transfer.".into());
        }
        let mut current = lock(&self.current)?;
        if current
            .as_ref()
            .is_some_and(|o| o.status.lock().map(|s| s.state.active()).unwrap_or(true))
        {
            return Err("A library transfer is already active".into());
        }
        *current = Some(operation);
        Ok(())
    }
    fn operation(&self, id: u64) -> LibraryTransferResult<Arc<Operation>> {
        lock(&self.current)?
            .as_ref()
            .filter(|o| o.id == id)
            .cloned()
            .ok_or_else(|| "This transfer is no longer active".into())
    }
    fn dismiss(&self, id: u64) -> LibraryTransferResult<()> {
        let mut current = lock(&self.current)?;
        if let Some(operation) = current.as_ref().filter(|operation| operation.id == id) {
            if lock(&operation.status)?.state.active() {
                return Err("An active transfer cannot be dismissed".into());
            }
            current.take();
        }
        Ok(())
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
fn reserve_operation(
    active: &Arc<AtomicBool>,
) -> LibraryTransferResult<TransferOperationReservation> {
    if active.swap(true, Ordering::AcqRel) {
        return Err("Another library transfer is already running".into());
    }
    Ok(TransferOperationReservation {
        active: active.clone(),
    })
}

struct Operation {
    id: u64,
    role: Content,
    mode: Mode,
    control: Arc<TransferControl>,
    retry: AtomicBool,
    status: Mutex<LibraryTransferNetworkStatus>,
    trace: Trace,
    reservation: Mutex<Option<TransferOperationReservation>>,
    publication: Mutex<Option<discovery::View>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
enum NetworkState {
    Preparing,
    Binding,
    Waiting,
    Connecting,
    Authenticating,
    Sending,
    Receiving,
    Verifying,
    FinishingImport,
    AwaitingResult,
    WaitingResume,
    Complete,
    Cancelled,
    Expired,
    Failed,
    OutcomeUnknown,
}
impl NetworkState {
    fn active(self) -> bool {
        !matches!(
            self,
            Self::Complete | Self::Cancelled | Self::Expired | Self::Failed | Self::OutcomeUnknown
        )
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryTransferNetworkStatus {
    id: u64,
    role: Content,
    mode: Mode,
    state: NetworkState,
    listener_ready: bool,
    addresses: Vec<String>,
    code: String,
    expires_at_ms: Option<u64>,
    cancellable: bool,
    can_retry: bool,
    documents: usize,
    audiobooks: usize,
    package_bytes: u64,
    bytes_transferred: u64,
    receiver_progress: Option<LibraryTransferProgress>,
    error: Option<LibraryTransferError>,
    result: Option<LibraryTransferImportResult>,
    discovery: Option<discovery::Snapshot>,
}

// Pairing credentials are accepted only for an explicit connector; never Debug/log this request.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LibraryTransferNetworkRequest {
    role: Content,
    mode: Mode,
    interface_id: String,
    port: Option<u16>,
    address: Option<String>,
    code: Option<String>,
    selection: Option<LibraryTransferExportRequest>,
}

#[tauri::command]
pub fn library_transfer_interfaces() -> LibraryTransferResult<Vec<TransferInterface>> {
    endpoints::interfaces().map_err(Into::into)
}

#[tauri::command]
pub fn library_transfer_network_start(
    app: tauri::AppHandle,
    state: State<'_, LibraryTransferState>,
    request: LibraryTransferNetworkRequest,
) -> LibraryTransferResult<LibraryTransferNetworkStatus> {
    let epoch = state.check_foreground()?;
    let reservation = state.begin_restore()?;
    let trace = state.diagnostics.begin(if request.role == Content::Sender {
        "send"
    } else {
        "receive"
    });
    let validated: LibraryTransferResult<_> = (|| {
        trace.stage("address-selection");
        let network =
            endpoints::select_interface(&endpoints::interfaces()?, Some(&request.interface_id))?;
        let side = request.mode.side(request.role);
        let peer = if side == Side::Connector {
            trace.stage("address-and-code-validation");
            Some((
                endpoints::parse_endpoint(
                    request.address.as_deref().unwrap_or_default(),
                    &network,
                )?,
                security::normalize_code(request.code.as_deref().unwrap_or_default())?,
            ))
        } else {
            if request.address.is_some() || request.code.is_some() {
                return Err("A listening transfer does not accept peer credentials".into());
            }
            None
        };
        let port = request.port.unwrap_or(DEFAULT_PORT);
        if side == Side::Listener && !(1024..=65535).contains(&port) {
            return Err("Choose a TCP port from 1024 to 65535".into());
        }
        if side == Side::Connector && request.port.is_some() {
            return Err("Use the port in the listening device address".into());
        }
        if request.role == Content::Receiver && request.selection.is_some() {
            return Err("Receiving does not accept a content selection".into());
        }
        Ok((network, port, peer))
    })();
    let (network, port, peer) = validated.inspect_err(|error| trace.finish(Some(error)))?;
    state.stop_discovery();
    let operation = Arc::new(Operation::new(
        state.next_id.fetch_add(1, Ordering::Relaxed) + 1,
        request.role,
        request.mode,
        trace.clone(),
    ));
    *lock(&operation.reservation)? = Some(reservation);
    state
        .install(operation.clone(), epoch)
        .inspect_err(|error| trace.finish(Some(error)))?;
    let initial = operation.snapshot()?;
    tauri::async_runtime::spawn(async move {
        let worker = operation.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            let _watch = session::NetworkWatch::start(worker.control.clone(), network.clone());
            operation::run(
                &app, &worker, network, port, peer, request.selection.unwrap_or_default(),
            )
        })
        .await
        .unwrap_or_else(|_| {
            Err(LibraryTransferError::message(
                "The transfer task stopped unexpectedly. Check the receiving device before trying again.",
            ))
        });
        operation.finish(result);
    });
    Ok(initial)
}

#[tauri::command]
pub fn library_transfer_network_status(
    state: State<'_, LibraryTransferState>,
    id: Option<u64>,
) -> LibraryTransferResult<Option<LibraryTransferNetworkStatus>> {
    lock(&state.current)?
        .as_ref()
        .filter(|o| id.is_none_or(|id| id == o.id))
        .map(|o| o.snapshot())
        .transpose()
}
#[tauri::command]
pub fn library_transfer_network_cancel(
    state: State<'_, LibraryTransferState>,
    id: u64,
) -> LibraryTransferResult<bool> {
    Ok(state.operation(id)?.control.cancel("cancelled"))
}
#[tauri::command]
pub fn library_transfer_network_retry(
    state: State<'_, LibraryTransferState>,
    id: u64,
) -> LibraryTransferResult<LibraryTransferNetworkStatus> {
    let operation = state.operation(id)?;
    if !operation.snapshot()?.can_retry {
        return Err("This transfer cannot be resumed. Start a new transfer.".into());
    }
    operation
        .retry
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| LibraryTransferError::message("A retry is already pending"))?;
    operation.snapshot()
}

#[tauri::command]
pub fn library_transfer_network_dismiss(
    state: State<'_, LibraryTransferState>,
    id: u64,
) -> LibraryTransferResult<()> {
    state.dismiss(id)
}

impl Operation {
    fn new(id: u64, role: Content, mode: Mode, trace: Trace) -> Self {
        Self {
            id,
            role,
            mode,
            control: Arc::new(TransferControl::default()),
            retry: AtomicBool::new(false),
            trace,
            reservation: Mutex::new(None),
            publication: Mutex::new(None),
            status: Mutex::new(LibraryTransferNetworkStatus {
                id,
                role,
                mode,
                state: NetworkState::Preparing,
                listener_ready: false,
                addresses: vec![],
                code: String::new(),
                expires_at_ms: None,
                cancellable: true,
                can_retry: false,
                documents: 0,
                audiobooks: 0,
                package_bytes: 0,
                bytes_transferred: 0,
                receiver_progress: None,
                error: None,
                result: None,
                discovery: None,
            }),
        }
    }
    fn snapshot(&self) -> LibraryTransferResult<LibraryTransferNetworkStatus> {
        let protocol = self.control.protocol();
        let mut status = lock(&self.status)?.clone();
        status.discovery = lock(&self.publication)?
            .as_ref()
            .map(discovery::View::snapshot)
            .transpose()?;
        if status.state.active() {
            if let Some(protocol) = protocol.filter(|p| p.generation() == self.id) {
                if let Ok(phase) = protocol.phase() {
                    // Only the worker publishes terminal states, after all guards/listeners close.
                    status.state = match phase {
                        Phase::Waiting => NetworkState::Waiting,
                        Phase::Connecting => NetworkState::Connecting,
                        Phase::Authenticating => NetworkState::Authenticating,
                        Phase::Offer | Phase::Ready | Phase::Streaming => {
                            if self.role == Content::Sender {
                                NetworkState::Sending
                            } else {
                                NetworkState::Receiving
                            }
                        }
                        Phase::Verifying => NetworkState::Verifying,
                        Phase::FinishingImport => NetworkState::FinishingImport,
                        Phase::AwaitingResult => NetworkState::AwaitingResult,
                        Phase::WaitingResume => NetworkState::WaitingResume,
                        _ => status.state,
                    };
                    status.cancellable = !phase.terminal()
                        && phase != Phase::FinishingImport
                        && status.state.active();
                    status.can_retry = phase == Phase::WaitingResume
                        && !protocol.is_listener()
                        && !self.retry.load(Ordering::Acquire);
                    status.listener_ready = matches!(phase, Phase::Waiting | Phase::WaitingResume)
                        && protocol.is_listener()
                        && status.listener_ready
                        && !status.addresses.is_empty()
                        && !status.code.is_empty()
                        && protocol.pairing_code().is_ok();
                    if !status.listener_ready {
                        status.clear_pairing();
                    }
                }
                if let Ok((bytes, total)) = protocol.bytes() {
                    status.bytes_transferred = bytes;
                    if total > 0 {
                        status.package_bytes = total;
                        self.trace.bytes(bytes, total);
                    }
                }
            } else {
                status.clear_pairing();
            }
        }
        Ok(status)
    }
    fn finish(&self, result: LibraryTransferResult<Option<LibraryTransferImportResult>>) {
        let protocol = self.control.protocol();
        let phase = protocol.as_ref().and_then(|p| p.phase().ok());
        let counts = protocol.as_ref().and_then(|p| p.bytes().ok());
        if let Some((bytes, total)) = counts.filter(|(_, total)| *total > 0) {
            self.trace.bytes(bytes, total);
        }
        self.control.release();
        let mut status = match self.status.lock() {
            Ok(status) => status,
            Err(_) => return,
        };
        status.state = if phase == Some(Phase::OutcomeUnknown)
            || result
                .as_ref()
                .err()
                .is_some_and(|e| e.code == "TRANSFER_OUTCOME_UNKNOWN")
        {
            NetworkState::OutcomeUnknown
        } else if phase == Some(Phase::Complete) && result.is_ok() {
            NetworkState::Complete
        } else if phase == Some(Phase::Expired) {
            NetworkState::Expired
        } else if phase == Some(Phase::Failed) {
            NetworkState::Failed
        } else if self.control.check().is_err() {
            NetworkState::Cancelled
        } else if result.is_ok() {
            NetworkState::Complete
        } else {
            NetworkState::Failed
        };
        status.error = result.as_ref().err().cloned();
        if status.state == NetworkState::OutcomeUnknown {
            status.error = Some(operation::unknown_outcome());
            self.trace.failure("outcome-unknown");
        }
        if status.state == NetworkState::Cancelled {
            status.error = None;
            self.trace.note("cleanup", self.control.reason());
            self.trace.cancelled();
        } else {
            self.trace.finish(status.error.as_ref());
        }
        status.result = result.ok().flatten();
        if let Some((bytes, total)) = counts {
            status.bytes_transferred = bytes;
            if total > 0 {
                status.package_bytes = total;
            }
        }
        status.clear_pairing();
        status.cancellable = false;
        status.can_retry = false;
        // Release the native reservation before a terminal snapshot becomes observable.
        if let Ok(mut reservation) = self.reservation.lock() {
            reservation.take();
        }
    }
}
impl LibraryTransferNetworkStatus {
    fn clear_pairing(&mut self) {
        self.listener_ready = false;
        self.addresses.clear();
        self.code.clear();
        self.expires_at_ms = None;
    }
}
fn expires_at(deadline: Instant) -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
        + deadline
            .saturating_duration_since(Instant::now())
            .as_millis() as u64
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
    fn native_reservation_serializes_network_generations_and_file_imports() {
        let state = LibraryTransferState::default();
        let active = state.begin_restore().unwrap();
        assert!(state.begin_restore().is_err());
        drop(active);
        assert!(state.begin_restore().is_ok());
    }
    #[test]
    fn stale_commands_and_backgrounded_preparation_cannot_change_the_next_generation() {
        let state = LibraryTransferState::default();
        state.set_backgrounded(false);
        let epoch = state.check_foreground().unwrap();
        let first = Arc::new(Operation::new(
            1,
            Content::Sender,
            Mode::Push,
            state.diagnostics.begin("send"),
        ));
        state.install(first.clone(), epoch).unwrap();
        assert!(state.operation(999).is_err());
        assert!(first.control.check().is_ok());
        state.cancel_network_sessions("closed");
        first.finish(Err(LibraryTransferError::message("cancelled")));
        let next = Arc::new(Operation::new(
            2,
            Content::Receiver,
            Mode::Push,
            state.diagnostics.begin("receive"),
        ));
        assert!(state.install(next.clone(), epoch).is_err());
        state
            .install(next.clone(), state.check_foreground().unwrap())
            .unwrap();
        assert!(state.operation(1).is_err());
        assert!(next.control.check().is_ok());
        state.set_backgrounded(true);
        assert!(next.control.check().is_err());
        assert!(state.check_foreground().is_err());
    }
    #[test]
    fn request_contract_rejects_legacy_or_automatic_mode_negotiation() {
        for mode in ["auto", "PCLAN003", "", "Push"] {
            assert!(serde_json::from_value::<LibraryTransferNetworkRequest>(
                serde_json::json!({"role":"sender","mode":mode,"interfaceId":"selected"})
            )
            .is_err());
        }
        assert!(serde_json::from_value::<LibraryTransferNetworkRequest>(serde_json::json!({"role":"sender","mode":"push","interfaceId":"selected","callbackAddress":"private"})).is_err());
        assert_eq!(protocol_version(), "PCLAN004");
    }

    #[test]
    fn terminal_publication_releases_the_reservation_and_stale_dismiss_cannot_clear_a_new_owner() {
        let state = LibraryTransferState::default();
        let operation = Arc::new(Operation::new(
            1,
            Content::Sender,
            Mode::Push,
            state.diagnostics.begin("send"),
        ));
        *operation.reservation.lock().unwrap() = Some(state.begin_restore().unwrap());
        state
            .install(operation.clone(), state.check_foreground().unwrap())
            .unwrap();
        assert!(state.dismiss(1).is_err());
        assert!(state.begin_restore().is_err());
        operation.control.cancel("cancelled");
        operation.finish(Err(LibraryTransferError::message("cancelled")));
        assert!(!operation.snapshot().unwrap().state.active());
        assert!(state.begin_restore().is_ok());
        state.dismiss(1).unwrap();
        let next = Arc::new(Operation::new(
            2,
            Content::Receiver,
            Mode::Pull,
            state.diagnostics.begin("receive"),
        ));
        state
            .install(next, state.check_foreground().unwrap())
            .unwrap();
        state.dismiss(1).unwrap();
        assert!(state.operation(2).is_ok());
    }
}
