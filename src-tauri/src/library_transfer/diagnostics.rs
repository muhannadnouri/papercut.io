//! Bounded process-local diagnostics. Only fixed labels and numeric facts cross this boundary.
use std::collections::{BTreeMap, VecDeque};
use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;

use super::{LibraryTransferError, LibraryTransferImportResult, LibraryTransferProgress};

const LIMIT: usize = 200;

#[derive(Clone, Default)]
pub(super) struct Diagnostics(Arc<Mutex<History>>);

#[derive(Default)]
struct History {
    next_attempt: u64,
    dropped: u64,
    events: VecDeque<DiagnosticEvent>,
    attempts: BTreeMap<&'static str, Attempt>,
}

struct Attempt {
    id: u64,
    started: Instant,
    phase: &'static str,
    sample: Instant,
    last_progress: Option<(Option<u64>, Option<u64>, Option<usize>, Option<usize>)>,
    finished: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticEvent {
    attempt: u64,
    role: &'static str,
    timestamp_ms: u64,
    elapsed_ms: u64,
    phase: &'static str,
    level: &'static str,
    code: &'static str,
    bytes: Option<u64>,
    total_bytes: Option<u64>,
    items: Option<usize>,
    total_items: Option<usize>,
    io_kind: Option<String>,
    os_code: Option<i32>,
    port: Option<u16>,
    address_scope: Option<&'static str>,
    required_bytes: Option<u64>,
    available_bytes: Option<u64>,
    imported: Option<usize>,
    skipped: Option<usize>,
    failed: Option<usize>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticSnapshot {
    app_version: &'static str,
    build: Option<&'static str>,
    os: &'static str,
    arch: &'static str,
    protocol: &'static str,
    dropped: u64,
    events: Vec<DiagnosticEvent>,
}

enum Facts<'a> {
    Progress(&'a LibraryTransferProgress),
    Io(&'a io::Error),
    Endpoint(SocketAddr),
    Failure(&'a LibraryTransferError),
    ImportResult(&'a LibraryTransferImportResult),
}

#[derive(Clone)]
pub(super) struct Trace {
    diagnostics: Diagnostics,
    role: &'static str,
    id: u64,
}

impl Diagnostics {
    pub fn begin(&self, role: &'static str) -> Trace {
        let mut id = 0;
        if let Ok(mut history) = self.0.lock() {
            history.next_attempt += 1;
            id = history.next_attempt;
            history.attempts.insert(
                role,
                Attempt {
                    id,
                    started: Instant::now(),
                    phase: "preparation",
                    sample: Instant::now(),
                    last_progress: None,
                    finished: false,
                },
            );
        }
        let trace = Trace {
            diagnostics: self.clone(),
            role,
            id,
        };
        trace.stage("preparation");
        trace
    }

    pub fn snapshot(&self) -> Result<DiagnosticSnapshot, String> {
        let history = self
            .0
            .lock()
            .map_err(|_| "Transfer diagnostics are unavailable")?;
        Ok(DiagnosticSnapshot {
            app_version: env!("CARGO_PKG_VERSION"),
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
            protocol: super::network::protocol_version(),
            build: option_env!("GITHUB_SHA")
                .filter(|sha| sha.len() == 40 && sha.bytes().all(|byte| byte.is_ascii_hexdigit())),
            dropped: history.dropped,
            events: history.events.iter().cloned().collect(),
        })
    }

    pub fn progress(&self, progress: &LibraryTransferProgress) {
        let role = match progress.operation.as_str() {
            "receive" => "receive",
            "import" => "import",
            _ => return,
        };
        let Ok(history) = self.0.lock() else { return };
        let Some(attempt) = history.attempts.get(role) else {
            return;
        };
        let id = attempt.id;
        drop(history);
        Trace {
            diagnostics: self.clone(),
            role,
            id,
        }
        .progress(progress);
    }
}

impl Trace {
    pub fn progress(&self, progress: &LibraryTransferProgress) {
        let phase = match progress.phase.as_str() {
            "connecting" => return,
            "receiving" => "package-transfer",
            "verifying" => "verification",
            "importingDocuments" => "document-import",
            "restoringAudiobooks" => "audiobook-import",
            _ => return,
        };
        self.record(phase, "info", "progress", Some(Facts::Progress(progress)));
    }
    pub fn bytes(&self, bytes: u64, total: u64) {
        self.record(
            "package-transfer",
            "info",
            "progress",
            Some(Facts::Progress(&LibraryTransferProgress {
                operation: self.role.into(),
                phase: "receiving".into(),
                bytes_processed: Some(bytes),
                bytes_total: Some(total),
                items_processed: None,
                items_total: None,
                item: None,
            })),
        );
    }
    pub fn import_result(&self, result: &LibraryTransferImportResult) {
        let partial = result.failed > 0 || result.audiobooks_failed > 0;
        self.record(
            "restore-summary",
            if partial { "warning" } else { "info" },
            if partial {
                "partial-import"
            } else {
                "import-complete"
            },
            Some(Facts::ImportResult(result)),
        );
    }
    pub fn endpoint(&self, address: SocketAddr) {
        self.record(
            "",
            "info",
            "endpoint-redacted",
            Some(Facts::Endpoint(address)),
        );
    }
    pub fn stage(&self, phase: &'static str) {
        self.record(phase, "info", "started", None);
    }
    pub fn note(&self, phase: &'static str, code: &'static str) {
        self.record(phase, "warning", code, None);
    }
    pub fn io_error(&self, error: &io::Error) {
        self.record("", "error", "io-error", Some(Facts::Io(error)));
    }
    pub fn failure(&self, code: &'static str) {
        self.record("", "error", code, None);
    }

    pub fn finish(&self, error: Option<&LibraryTransferError>) {
        if let Some(error) = error {
            self.record(
                "",
                "error",
                if error.is_retryable() {
                    "insufficient-space"
                } else {
                    "failed"
                },
                Some(Facts::Failure(error)),
            );
        } else {
            self.record("completion", "info", "complete", None);
        }
        self.close();
    }

    pub fn cancelled(&self) {
        self.record("completion", "info", "cancelled", None);
        self.close();
    }

    fn close(&self) {
        if let Ok(mut history) = self.diagnostics.0.lock() {
            if let Some(attempt) = history
                .attempts
                .get_mut(self.role)
                .filter(|attempt| attempt.id == self.id)
            {
                attempt.finished = true;
            }
        }
    }

    fn record(
        &self,
        phase: &'static str,
        level: &'static str,
        code: &'static str,
        facts: Option<Facts<'_>>,
    ) {
        let Ok(mut history) = self.diagnostics.0.lock() else {
            return;
        };
        let Some(attempt) = history
            .attempts
            .get_mut(self.role)
            .filter(|a| a.id == self.id && !a.finished)
        else {
            return;
        };
        let phase = if phase.is_empty() {
            attempt.phase
        } else {
            phase
        };
        if let Some(Facts::Progress(progress)) = &facts {
            let counts = (
                progress.bytes_processed,
                progress.bytes_total,
                progress.items_processed,
                progress.items_total,
            );
            let complete = progress.bytes_total.is_some()
                && progress.bytes_processed == progress.bytes_total
                || progress.items_total.is_some()
                    && progress.items_processed == progress.items_total;
            if phase == attempt.phase
                && attempt.last_progress.is_some()
                && (attempt.last_progress == Some(counts)
                    || !complete && attempt.sample.elapsed() < Duration::from_secs(1))
            {
                return;
            }
            attempt.last_progress = Some(counts);
        } else {
            attempt.last_progress = None;
        }
        attempt.phase = phase;
        attempt.sample = Instant::now();
        let progress = match &facts {
            Some(Facts::Progress(progress)) => Some(*progress),
            _ => None,
        };
        let error = match &facts {
            Some(Facts::Io(error)) => Some(*error),
            _ => None,
        };
        let endpoint = match &facts {
            Some(Facts::Endpoint(address)) => Some(*address),
            _ => None,
        };
        let failure = match &facts {
            Some(Facts::Failure(error)) => Some(*error),
            _ => None,
        };
        let result = match &facts {
            Some(Facts::ImportResult(result)) => Some(*result),
            _ => None,
        };
        let event = DiagnosticEvent {
            attempt: self.id,
            role: self.role,
            timestamp_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            elapsed_ms: attempt.started.elapsed().as_millis() as u64,
            phase,
            level,
            code,
            bytes: progress.and_then(|p| p.bytes_processed),
            total_bytes: progress.and_then(|p| p.bytes_total),
            items: progress.and_then(|p| p.items_processed),
            total_items: progress.and_then(|p| p.items_total),
            // Error display strings may contain paths, peer input or credentials. Never record them.
            io_kind: error.map(|e| format!("{:?}", e.kind())),
            os_code: error.and_then(io::Error::raw_os_error),
            port: endpoint.map(|address| address.port()),
            address_scope: endpoint.map(|address| {
                if address.ip().is_loopback() {
                    "loopback"
                } else if matches!(address.ip(), std::net::IpAddr::V4(ip) if ip.is_link_local())
                    || matches!(address.ip(), std::net::IpAddr::V6(ip) if ip.is_unicast_link_local()) {
                    "link-local"
                } else {
                    "private"
                }
            }),
            required_bytes: failure.and_then(|error| error.required_bytes),
            available_bytes: failure.and_then(|error| error.available_bytes),
            imported: result.map(|result| result.imported + result.audiobooks_imported),
            skipped: result.map(|result| result.skipped + result.audiobooks_skipped),
            failed: result.map(|result| result.failed + result.audiobooks_failed),
        };
        if history.events.len() == LIMIT {
            history.events.pop_front();
            history.dropped += 1;
        }
        history.events.push_back(event);
    }
}

#[tauri::command]
pub fn library_transfer_diagnostics(
    state: tauri::State<'_, super::LibraryTransferState>,
) -> Result<DiagnosticSnapshot, String> {
    state.diagnostics.snapshot()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_is_bounded_and_never_records_error_strings_or_item_names() {
        let diagnostics = Diagnostics::default();
        let trace = diagnostics.begin("receive");
        trace.stage("tcp-connect");
        trace.io_error(&io::Error::other(
            "192.168.1.4:1234 ABCD-EFGH-JKLM /home/private/book.pdf secret title",
        ));
        let progress = LibraryTransferProgress {
            operation: "receive".into(),
            phase: "verifying".into(),
            bytes_processed: Some(10),
            bytes_total: Some(20),
            items_processed: None,
            items_total: None,
            item: Some("secret title".into()),
        };
        diagnostics.progress(&progress);
        for _ in 0..1000 {
            diagnostics.progress(&progress);
        }
        assert_eq!(diagnostics.snapshot().unwrap().events.len(), 4);
        let json = serde_json::to_string(&diagnostics.snapshot().unwrap()).unwrap();
        for secret in ["192.168", "ABCD", "/home", "secret title"] {
            assert!(!json.contains(secret));
        }
        assert!(json.contains("Other"));
        for _ in 0..300 {
            trace.note("waiting", "unauthenticated-connection");
        }
        let snapshot = diagnostics.snapshot().unwrap();
        assert_eq!(snapshot.events.len(), LIMIT);
        assert_eq!(snapshot.dropped, 104);
    }

    #[test]
    fn attempts_retain_failure_and_are_isolated_from_late_and_untrusted_progress() {
        let diagnostics = Diagnostics::default();
        let old = diagnostics.begin("receive");
        old.stage("tls-handshake");
        old.finish(Some(&LibraryTransferError::message("secret error")));
        let next = diagnostics.begin("receive");
        let before = diagnostics.snapshot().unwrap().events.len();
        old.failure("late");
        diagnostics.progress(&LibraryTransferProgress {
            operation: "receive".into(),
            phase: "secret phase".into(),
            bytes_processed: None,
            bytes_total: None,
            items_processed: None,
            items_total: None,
            item: None,
        });
        assert_eq!(diagnostics.snapshot().unwrap().events.len(), before);
        next.finish(None);
        let events = diagnostics.snapshot().unwrap().events;
        assert_eq!(events[2].phase, "tls-handshake");
        assert_eq!(events[2].code, "failed");
        assert_ne!(events[2].attempt, events.last().unwrap().attempt);
        assert_eq!(events.last().unwrap().code, "complete");
    }

    #[test]
    fn sampling_retains_first_and_final_counts_and_redacts_endpoint_and_storage_errors() {
        let diagnostics = Diagnostics::default();
        let trace = diagnostics.begin("send");
        trace.stage("package-transfer");
        trace.bytes(0, 100);
        trace.bytes(10, 100);
        trace.bytes(100, 100);
        trace.bytes(100, 100);
        let events = diagnostics.snapshot().unwrap().events;
        let counts: Vec<_> = events.iter().filter_map(|event| event.bytes).collect();
        assert_eq!(counts, vec![0, 100]);
        trace.endpoint("192.168.1.42:50123".parse().unwrap());
        trace.finish(Some(&LibraryTransferError::insufficient_space(1024, 512)));
        let report = serde_json::to_string(&diagnostics.snapshot().unwrap()).unwrap();
        assert!(!report.contains("192.168.1.42"));
        assert!(report.contains("50123"));
        assert!(report.contains("private"));
        assert!(report.contains("\"requiredBytes\":1024"));
        assert!(report.contains("\"availableBytes\":512"));
        assert!(report.contains("insufficient-space"));
        let cancelled = diagnostics.begin("export");
        cancelled.cancelled();
        assert_eq!(
            diagnostics.snapshot().unwrap().events.last().unwrap().code,
            "cancelled"
        );
    }

    #[test]
    fn ipv6_endpoints_retain_only_port_and_scope() {
        for (address, scope) in [
            ("[fe80::abcd%27]:53318", "link-local"),
            ("[fd12::abcd]:53318", "private"),
            ("[::1]:53318", "loopback"),
        ] {
            let diagnostics = Diagnostics::default();
            let trace = diagnostics.begin("receive");
            trace.endpoint(address.parse().unwrap());
            let report = serde_json::to_value(diagnostics.snapshot().unwrap()).unwrap();
            let event = report["events"].as_array().unwrap().last().unwrap();
            assert_eq!(event["port"], 53318);
            assert_eq!(event["addressScope"], scope);
            assert!(!report.to_string().contains("abcd"));
            assert!(!report.to_string().contains("%27"));
        }
    }

    #[test]
    fn partial_restore_summary_keeps_counts_without_item_failures_or_content() {
        let diagnostics = Diagnostics::default();
        let trace = diagnostics.begin("import");
        trace.import_result(&LibraryTransferImportResult {
            selected: 3,
            imported: 1,
            skipped: 1,
            failed: 1,
            folders_created: 1,
            audiobooks_selected: 0,
            audiobooks_imported: 0,
            audiobooks_skipped: 0,
            audiobooks_failed: 0,
            imported_audiobooks: vec![],
            failures: vec![crate::library_transfer::LibraryTransferFailure {
                item: "private title".into(),
                error: "/private/path content".into(),
            }],
        });
        let events = diagnostics.snapshot().unwrap().events;
        let event = events.last().unwrap();
        assert_eq!(event.code, "partial-import");
        assert_eq!(
            (event.imported, event.skipped, event.failed),
            (Some(1), Some(1), Some(1))
        );
        assert!(!serde_json::to_string(&events).unwrap().contains("private"));
    }
}
