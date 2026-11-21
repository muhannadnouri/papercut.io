//! Production v4 orchestration. All guards live until the foreground generation ends.
use std::fs::File;
use std::io::{self, Seek, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use super::{
    endpoints, expires_at, lock, security, transport, v4, Content, Mode, NetworkState, Operation,
    Phase, Side,
};
use crate::library_transfer::{
    build_library_package, create_transfer_temp, ensure_available_space, import_library_package,
    LibraryTransferError, LibraryTransferExportRequest, LibraryTransferImportResult,
    LibraryTransferProgress, LibraryTransferResult, TransferTempFile,
};

struct Prepared {
    reader: File,
    _guard: TransferTempFile,
    offer: v4::Offer,
}

pub(super) fn run(
    app: &tauri::AppHandle,
    operation: &Operation,
    network: endpoints::TransferInterface,
    port: u16,
    peer: Option<(SocketAddr, String)>,
    selection: LibraryTransferExportRequest,
) -> LibraryTransferResult<Option<LibraryTransferImportResult>> {
    let check = || operation.control.check().map_err(|_| cancelled());
    check()?;
    operation.trace.stage(
        match (operation.mode.side(operation.role), operation.mode) {
            (Side::Connector, Mode::Push) => "connector-push",
            (Side::Connector, Mode::Pull) => "connector-pull",
            (Side::Listener, Mode::Push) => "listener-push",
            (Side::Listener, Mode::Pull) => "listener-pull",
        },
    );
    let mut prepared = if operation.role == Content::Sender {
        operation.trace.stage("package-preparation");
        let mut guard = create_transfer_temp(app, "lan-send")?;
        let path = guard.path().to_path_buf();
        // shortcut: packaging is synchronous; thread cancellation through it if large-library latency warrants it.
        let result = build_library_package(
            app,
            guard.file_mut(),
            &path,
            selection.document_ids.as_deref(),
            &selection.audiobook_ids,
        )?;
        check()?;
        if result.documents + result.audiobooks == 0 {
            return Err("Choose at least one document or audiobook.".into());
        }
        guard.file_mut().flush().map_err(|e| e.to_string())?;
        guard.file.take();
        let mut reader = File::open(&path).map_err(|e| e.to_string())?;
        operation.trace.stage("archive-digest");
        let offer = v4::hash_archive(&mut reader, || {
            operation.control.check().map_err(v4::Error::Io)
        })
        .map_err(|e| protocol_error(e, &operation.trace))?;
        reader.rewind().map_err(|e| e.to_string())?;
        let mut status = lock(&operation.status)?;
        status.documents = result.documents;
        status.audiobooks = result.audiobooks;
        status.package_bytes = offer.size;
        Some(Prepared {
            _guard: guard,
            reader,
            offer,
        })
    } else {
        None
    };
    // One unique private file is retained only by this native generation, never by address/code.
    let mut staging = if operation.role == Content::Receiver {
        Some(create_transfer_temp(app, "lan-receive")?)
    } else {
        None
    };
    check()?;
    if !endpoints::still_available(&network) {
        operation.control.cancel("network-changed");
        return Err(cancelled());
    }
    let side = operation.mode.side(operation.role);
    let listeners = if side == Side::Listener {
        lock(&operation.status)?.state = NetworkState::Binding;
        operation.trace.stage("listener-bind");
        endpoints::bind_listeners(&network, port).map_err(|error| {
            operation.trace.io_error(&error);
            failure(
                if error.kind() == io::ErrorKind::AddrInUse {
                    "TRANSFER_PORT_IN_USE"
                } else {
                    "TRANSFER_BIND_FAILED"
                },
                format!("Could not bind the selected network port: {error}"),
            )
        })?
    } else {
        vec![]
    };
    let protocol = if side == Side::Listener {
        let addresses = listeners
            .iter()
            .map(TcpListener::local_addr)
            .collect::<io::Result<Vec<_>>>()
            .map_err(|e| e.to_string())?;
        for address in &addresses {
            operation.trace.endpoint(*address);
        }
        operation.trace.stage("certificate-setup");
        Arc::new(
            v4::Session::listener(operation.id, operation.mode, addresses)
                .map_err(|e| protocol_error(e, &operation.trace))?,
        )
    } else {
        let (address, code) = peer.as_ref().ok_or("Missing explicit peer details")?;
        Arc::new(
            v4::Session::connector(operation.id, operation.mode, *address, code)
                .map_err(|e| protocol_error(e, &operation.trace))?,
        )
    };
    operation
        .control
        .install(protocol.clone())
        .map_err(|_| cancelled())?;
    let _publication = if side == Side::Listener {
        match super::discovery::Job::start(
            app.clone(),
            operation.id,
            network.clone(),
            operation.mode,
            Some(protocol.clone()),
            port,
            operation.trace.clone(),
        ) {
            Ok(job) => {
                *lock(&operation.publication)? = Some(job.view());
                Some(job)
            }
            Err(_) => {
                operation.trace.note("discovery", "unavailable");
                None
            }
        }
    } else {
        None
    };
    let mut interrupted = None;
    loop {
        check()?;
        let phase = protocol
            .phase()
            .map_err(|e| protocol_error(e, &operation.trace))?;
        if phase == Phase::Expired {
            return Err(protocol_error(v4::Error::Expired, &operation.trace));
        }
        if side == Side::Listener {
            expose_pairing(operation, &protocol, &listeners, interrupted.take())?;
        } else if phase == Phase::WaitingResume {
            lock(&operation.status)?.state = NetworkState::WaitingResume;
            if !operation.retry.load(Ordering::Acquire) {
                thread::sleep(Duration::from_millis(50));
                continue;
            }
        }
        let socket = if side == Side::Listener {
            accept(&listeners, &protocol, &operation.control, &operation.trace)?
        } else {
            let deadline = protocol
                .begin_connect()
                .map_err(|e| protocol_error(e, &operation.trace))?;
            operation.retry.store(false, Ordering::Release);
            operation.trace.stage("tcp-connect");
            let address = peer.as_ref().ok_or("Missing peer")?.0;
            operation.trace.endpoint(address);
            match transport::connect_cancellable(
                address,
                Some(&network),
                &operation.control,
                deadline,
            ) {
                Ok(socket) => socket,
                Err(error) => {
                    operation.control.clear_socket();
                    let _ = protocol.connect_failed();
                    operation.trace.io_error(&error);
                    let error = failure(
                        "TRANSFER_CONNECT_FAILED",
                        format!("Could not connect to the listening device: {error}"),
                    );
                    lock(&operation.status)?.error = Some(error);
                    continue;
                }
            }
        };
        {
            let mut status = lock(&operation.status)?;
            status.clear_pairing();
            status.error = None;
            status.receiver_progress = None;
        }
        operation.trace.stage("tls-and-pairing");
        let attempt = transfer(
            operation,
            &protocol,
            socket,
            prepared.as_mut().map(|p| (&mut p.reader, p.offer)),
            staging.as_mut(),
            |path, forward| import_library_package(app, path, "receive", Some(forward)),
        );
        operation.control.clear_socket();
        match attempt {
            Ok(result) => return Ok(result),
            Err(error) => {
                let phase = protocol
                    .phase()
                    .map_err(|e| protocol_error(e, &operation.trace))?;
                if matches!(phase, Phase::Waiting | Phase::WaitingResume) {
                    operation.trace.note("waiting", "retryable-interruption");
                    lock(&operation.status)?.error = Some(error.clone());
                    interrupted = Some(error);
                } else {
                    return Err(error);
                }
            }
        }
    }
}

fn expose_pairing(
    operation: &Operation,
    protocol: &v4::Session,
    listeners: &[TcpListener],
    error: Option<LibraryTransferError>,
) -> LibraryTransferResult<()> {
    let mut status = lock(&operation.status)?;
    status.state = if protocol
        .phase()
        .map_err(|e| protocol_error(e, &operation.trace))?
        == Phase::WaitingResume
    {
        NetworkState::WaitingResume
    } else {
        NetworkState::Waiting
    };
    status.addresses = listeners
        .iter()
        .map(|l| l.local_addr().map(|a| a.to_string()))
        .collect::<io::Result<_>>()
        .map_err(|e| e.to_string())?;
    status.code = security::display_code(
        &protocol
            .pairing_code()
            .map_err(|e| protocol_error(e, &operation.trace))?,
    );
    status.expires_at_ms = protocol
        .deadline()
        .map_err(|e| protocol_error(e, &operation.trace))?
        .map(expires_at);
    status.listener_ready = true;
    status.error = error;
    operation.trace.stage("waiting");
    Ok(())
}

fn accept(
    listeners: &[TcpListener],
    protocol: &v4::Session,
    control: &super::TransferControl,
    trace: &super::Trace,
) -> LibraryTransferResult<TcpStream> {
    loop {
        control.check().map_err(|_| cancelled())?;
        if protocol.phase().map_err(|e| protocol_error(e, trace))? == Phase::Expired {
            return Err(protocol_error(v4::Error::Expired, trace));
        }
        for listener in listeners {
            match listener.accept() {
                Ok((socket, _)) => {
                    trace.stage("tcp-accepted");
                    return Ok(socket);
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                Err(error) => {
                    trace.io_error(&error);
                    return Err(failure(
                        "TRANSFER_LISTENER_FAILED",
                        format!("The local transfer listener failed: {error}"),
                    ));
                }
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
}

// The same adapter is used by the real importer and the file/socket regression harness.
fn transfer(
    operation: &Operation,
    protocol: &v4::Session,
    socket: TcpStream,
    prepared: Option<(&mut File, v4::Offer)>,
    mut staging: Option<&mut TransferTempFile>,
    import: impl FnOnce(
        &Path,
        &mut dyn FnMut(&LibraryTransferProgress),
    ) -> LibraryTransferResult<LibraryTransferImportResult>,
) -> LibraryTransferResult<Option<LibraryTransferImportResult>> {
    let mut channel = protocol.authenticate(socket).map_err(|error| {
        let incomplete = matches!(&error, v4::Error::Io(_));
        let mut error = protocol_error(error, &operation.trace);
        if incomplete {
            error.message.push_str(" Check the listening device and update Papercut on both devices, or use a transfer file.");
        }
        error
    })?;
    operation.trace.stage("package-offer");
    if operation.role == Content::Sender {
        let (archive, offer) = prepared.ok_or("Missing prepared package")?;
        channel
            .send_offer(offer)
            .map_err(|e| protocol_error(e, &operation.trace))?;
        channel
            .wait_ready()
            .map_err(|e| protocol_error(e, &operation.trace))?;
        operation.trace.stage("package-transfer");
        channel
            .send_archive(archive)
            .map_err(|e| protocol_error(e, &operation.trace))?;
        operation.trace.bytes(offer.size, offer.size);
        operation.trace.stage("acknowledgement");
        channel
            .await_result(|progress| {
                operation.trace.progress(&progress);
                if let Ok(mut status) = operation.status.lock() {
                    status.receiver_progress = Some(progress);
                }
            })
            .map_err(|e| protocol_error(e, &operation.trace))?;
        Ok(None)
    } else {
        let offer = channel
            .receive_offer()
            .map_err(|e| protocol_error(e, &operation.trace))?;
        let staging = staging.as_mut().ok_or("Missing private staging file")?;
        let path = staging.path().to_path_buf();
        let offset = staging
            .file_mut()
            .metadata()
            .map_err(|e| e.to_string())?
            .len();
        if let Some(remaining) = offer.size.checked_sub(offset) {
            if let Err(error) =
                ensure_available_space(path.parent().ok_or("Missing staging directory")?, remaining)
            {
                let _ = channel.reject(error.clone());
                return Err(error);
            }
        }
        operation.trace.stage("package-transfer");
        channel
            .receive_archive(staging.file_mut(), u64::MAX)
            .map_err(|e| protocol_error(e, &operation.trace))?;
        operation.trace.bytes(offer.size, offer.size);
        operation.trace.stage("verification");
        operation.control.begin_import().map_err(|_| cancelled())?;
        let result = import(&path, &mut |progress| {
            operation.trace.progress(progress);
            if let Ok(mut status) = operation.status.lock() {
                status.receiver_progress = Some(progress.clone());
            }
            let _ = channel.progress(progress.clone());
        });
        operation.trace.stage("acknowledgement");
        let _ = channel.finish_import(result.as_ref().map(|_| ()).map_err(Clone::clone));
        if let Ok(result) = &result {
            operation.trace.import_result(result);
        }
        result.map(Some)
    }
}

fn cancelled() -> LibraryTransferError {
    failure("TRANSFER_CANCELLED", "Library transfer cancelled")
}
pub(super) fn unknown_outcome() -> LibraryTransferError {
    failure(
        "TRANSFER_OUTCOME_UNKNOWN",
        "The receiving device may have imported this transfer. Check that device before sending again.",
    )
}
fn failure(code: &str, message: impl Into<String>) -> LibraryTransferError {
    let mut error = LibraryTransferError::message(message);
    error.code = code.into();
    error
}
fn protocol_error(error: v4::Error, trace: &super::Trace) -> LibraryTransferError {
    let (code, message) = match error {
        v4::Error::Io(error) => {
            trace.io_error(&error);
            ("TRANSFER_IO_FAILED", format!("The local transfer connection stopped: {error}"))
        }
        v4::Error::Protocol(message) => ("TRANSFER_PROTOCOL_ERROR", message.into()),
        v4::Error::Incompatible => (
            "TRANSFER_INCOMPATIBLE",
            "This device uses a different transfer protocol. Update Papercut on both devices or use a transfer file.".into(),
        ),
        v4::Error::WrongProof => (
            "TRANSFER_PAIRING_FAILED",
            "The pairing code did not match the listening device. Start a new transfer.".into(),
        ),
        v4::Error::Expired => (
            "TRANSFER_EXPIRED",
            "The pairing session expired. Start a new transfer.".into(),
        ),
        v4::Error::Cancelled => return cancelled(),
        v4::Error::ChangedSession | v4::Error::ChangedPackage => (
            "TRANSFER_IDENTITY_CHANGED",
            "The selected session or package changed. Start a new transfer.".into(),
        ),
        v4::Error::DigestMismatch => (
            "TRANSFER_DIGEST_MISMATCH",
            "The received package failed verification. Nothing was imported from this attempt.".into(),
        ),
        v4::Error::Remote(error) => return error,
        v4::Error::OutcomeUnknown => (
            "TRANSFER_OUTCOME_UNKNOWN",
            "The receiving device may have imported this transfer. Check that device before sending again.".into(),
        ),
    };
    trace.failure(match code {
        "TRANSFER_INCOMPATIBLE" => "protocol-mismatch",
        "TRANSFER_PAIRING_FAILED" => "pairing-mismatch",
        "TRANSFER_EXPIRED" => "session-expired",
        "TRANSFER_IDENTITY_CHANGED" => "identity-changed",
        "TRANSFER_DIGEST_MISMATCH" => "archive-digest-mismatch",
        "TRANSFER_OUTCOME_UNKNOWN" => "outcome-unknown",
        _ => "connection-failed",
    });
    failure(code, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library_transfer::diagnostics::Diagnostics;
    use crate::library_transfer::{create_private_directory, create_transfer_temp_in};
    use std::sync::atomic::AtomicUsize;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn result() -> LibraryTransferImportResult {
        LibraryTransferImportResult {
            selected: 2,
            imported: 1,
            skipped: 0,
            failed: 1,
            folders_created: 0,
            audiobooks_selected: 0,
            audiobooks_imported: 0,
            audiobooks_skipped: 0,
            audiobooks_failed: 0,
            imported_audiobooks: vec![],
            failures: vec![],
        }
    }
    #[derive(Clone, Copy)]
    enum Scenario {
        Complete,
        Corrupt,
        LostResult,
        ImportFailure,
    }

    #[test]
    fn production_adapter_uses_private_files_and_restores_once_in_both_connection_modes() {
        for mode in [Mode::Push, Mode::Pull] {
            for scenario in [
                Scenario::Complete,
                Scenario::Corrupt,
                Scenario::LostResult,
                Scenario::ImportFailure,
            ] {
                let root = std::env::temp_dir().join(format!(
                    "papercut-v4-adapter-{}-{}",
                    std::process::id(),
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                ));
                create_private_directory(&root).unwrap();
                let listener = TcpListener::bind("127.0.0.1:0").unwrap();
                let endpoint = listener.local_addr().unwrap();
                let l = Arc::new(v4::Session::listener(1, mode, vec![endpoint]).unwrap());
                let c = Arc::new(
                    v4::Session::connector(2, mode, endpoint, &l.pairing_code().unwrap()).unwrap(),
                );
                let imports = AtomicUsize::new(0);
                let code = l.pairing_code().unwrap();
                let exercise = |protocol: Arc<v4::Session>, side: Side, socket: TcpStream| {
                    let role = if mode.side(Content::Sender) == side {
                        Content::Sender
                    } else {
                        Content::Receiver
                    };
                    let diagnostics = Diagnostics::default();
                    let operation = Operation::new(
                        protocol.generation(),
                        role,
                        mode,
                        diagnostics.begin(if role == Content::Sender {
                            "send"
                        } else {
                            "receive"
                        }),
                    );
                    operation.control.install(protocol.clone()).unwrap();
                    let source = b"native adapter test archive";
                    let mut guard = create_transfer_temp_in(&root, "adapter").unwrap();
                    let path = guard.path().to_path_buf();
                    let mut reader = None;
                    let offer = v4::hash_archive(&mut &source[..], || Ok(())).unwrap();
                    if role == Content::Sender {
                        guard.file_mut().write_all(source).unwrap();
                        guard.file_mut().flush().unwrap();
                        guard.file.take();
                        reader = Some(File::open(&path).unwrap());
                    } else if matches!(scenario, Scenario::Corrupt) {
                        guard
                            .file_mut()
                            .write_all(&vec![b'x'; source.len()])
                            .unwrap();
                    }
                    let outcome = transfer(
                        &operation,
                        &protocol,
                        socket,
                        reader.as_mut().map(|r| (r, offer)),
                        if role == Content::Receiver {
                            Some(&mut guard)
                        } else {
                            None
                        },
                        |staged, _| {
                            assert_eq!(protocol.phase().unwrap(), Phase::FinishingImport);
                            assert_eq!(std::fs::read(staged).unwrap(), source);
                            #[cfg(unix)]
                            {
                                use std::os::unix::fs::PermissionsExt;
                                assert_eq!(
                                    std::fs::metadata(staged).unwrap().permissions().mode() & 0o777,
                                    0o600
                                );
                            }
                            imports.fetch_add(1, Ordering::SeqCst);
                            if matches!(scenario, Scenario::LostResult) {
                                assert!(!operation.control.cancel("backgrounded"));
                                assert!(operation.control.check().is_ok());
                            }
                            if matches!(scenario, Scenario::ImportFailure) {
                                Err(LibraryTransferError::insufficient_space(1000, 1))
                            } else {
                                Ok(result())
                            }
                        },
                    );
                    drop(reader);
                    drop(guard);
                    assert!(!path.exists());
                    if matches!(scenario, Scenario::Complete) {
                        assert!(!operation.control.cancel("cancelled"));
                        assert!(operation.control.check().is_ok());
                        assert!(!operation.snapshot().unwrap().cancellable);
                    }
                    // A core terminal transition is not published as UI completion before cleanup.
                    assert!(operation.snapshot().unwrap().state.active());
                    operation.finish(outcome);
                    let status = operation.snapshot().unwrap();
                    assert!(
                        !status.listener_ready
                            && status.code.is_empty()
                            && status.addresses.is_empty()
                            && !status.can_retry
                    );
                    match (role, scenario) {
                        (Content::Sender, Scenario::LostResult | Scenario::Corrupt) => {
                            assert_eq!(status.state, NetworkState::OutcomeUnknown)
                        }
                        (Content::Receiver, Scenario::Corrupt) | (_, Scenario::ImportFailure) => {
                            assert_eq!(status.state, NetworkState::Failed)
                        }
                        _ => assert_eq!(status.state, NetworkState::Complete),
                    }
                    if role == Content::Receiver
                        && matches!(scenario, Scenario::Complete | Scenario::LostResult)
                    {
                        assert_eq!(status.result.unwrap().failed, 1);
                    }
                    let report = serde_json::to_string(&diagnostics.snapshot().unwrap()).unwrap();
                    let path_text = path.to_string_lossy();
                    for secret in [
                        code.as_str(),
                        path_text.as_ref(),
                        "native adapter test archive",
                    ] {
                        assert!(!report.contains(secret));
                    }
                };
                thread::scope(|scope| {
                    let server =
                        scope.spawn(|| exercise(l, Side::Listener, listener.accept().unwrap().0));
                    c.begin_connect().unwrap();
                    exercise(c, Side::Connector, TcpStream::connect(endpoint).unwrap());
                    server.join().unwrap();
                });
                assert_eq!(
                    imports.load(Ordering::SeqCst),
                    if matches!(scenario, Scenario::Corrupt) {
                        0
                    } else {
                        1
                    }
                );
                std::fs::remove_dir(root).unwrap();
            }
        }
    }

    #[test]
    fn cancellation_and_expiry_close_the_owned_listener_before_terminal_publication() {
        for expire in [false, true] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let endpoint = listener.local_addr().unwrap();
            let protocol = Arc::new(v4::Session::listener(1, Mode::Push, vec![endpoint]).unwrap());
            let operation = Operation::new(
                1,
                Content::Receiver,
                Mode::Push,
                Diagnostics::default().begin("receive"),
            );
            operation.control.install(protocol.clone()).unwrap();
            let listeners = vec![listener];
            expose_pairing(&operation, &protocol, &listeners, None).unwrap();
            if expire {
                // A short eligibility deadline exercises the actual waiting-loop expiry path.
                protocol.expire_for_test();
            } else {
                operation.control.cancel("cancelled");
            }
            assert!(accept(&listeners, &protocol, &operation.control, &operation.trace).is_err());
            assert!(operation.snapshot().unwrap().state.active());
            drop(listeners);
            assert!(TcpListener::bind(endpoint).is_ok());
            operation.finish(Err(if expire {
                protocol_error(v4::Error::Expired, &operation.trace)
            } else {
                cancelled()
            }));
            let status = operation.snapshot().unwrap();
            assert_eq!(
                status.state,
                if expire {
                    NetworkState::Expired
                } else {
                    NetworkState::Cancelled
                }
            );
            assert!(
                status.code.is_empty()
                    && status.addresses.is_empty()
                    && status.expires_at_ms.is_none()
            );
        }
    }

    #[test]
    fn safe_error_classification_retains_io_facts_and_excludes_peer_strings() {
        let diagnostics = Diagnostics::default();
        let trace = diagnostics.begin("send");
        trace.stage("tcp-connect");
        let error = io::Error::new(
            io::ErrorKind::PermissionDenied,
            "private-address-and-secret-code",
        );
        protocol_error(v4::Error::Io(error), &trace);
        protocol_error(v4::Error::Incompatible, &trace);
        let report = serde_json::to_string(&diagnostics.snapshot().unwrap()).unwrap();
        assert!(report.contains("PermissionDenied") && report.contains("protocol-mismatch"));
        assert!(!report.contains("private-address-and-secret-code"));
    }
}
