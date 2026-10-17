//! Optional, bounded system DNS-SD. Discovery is a hint; v4 pairing remains the trust boundary.
use std::collections::BTreeMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::State;

use super::{endpoints, lock, v4, LibraryTransferState, Mode, Phase, TransferInterface};
use crate::library_transfer::{diagnostics::Trace, LibraryTransferResult};

#[cfg(target_os = "android")]
mod android;
#[cfg(any(target_os = "macos", target_os = "ios", all(test, unix)))]
mod apple;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(any(windows, test))]
mod windows;

const SERVICE: &str = "_papercut-xfer._tcp";
const PEERS: usize = 64;
const ADDRESSES: usize = 8;
const EVENTS: usize = 128;
const BROWSE: Duration = Duration::from_secs(30);
const RETAIN: Duration = Duration::from_secs(60);
static WORKERS: AtomicUsize = AtomicUsize::new(0);
struct WorkerSlot;
impl WorkerSlot {
    fn acquire() -> LibraryTransferResult<Self> {
        WORKERS
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < 2).then_some(n + 1)
            })
            .map_err(|_| {
                "Discovery is still stopping. Retry shortly or enter the address manually."
            })?;
        Ok(Self)
    }
}
impl Drop for WorkerSlot {
    fn drop(&mut self) {
        WORKERS.fetch_sub(1, Ordering::AcqRel);
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Starting,
    Searching,
    Finished,
    Advertising,
    Idle,
    Unavailable,
    Denied,
    Unsupported,
    Limited,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Peer {
    pub id: String,
    pub name: String,
    pub addresses: Vec<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub id: u64,
    pub state: Status,
    pub name: Option<String>,
    pub peers: Vec<Peer>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum Event {
    Peer {
        name: String,
        source: String,
        addresses: Vec<String>,
        port: u16,
        txt: Vec<Vec<u8>>,
    },
    Lost {
        name: String,
        source: String,
    },
    Ready {
        name: String,
    },
    Error {
        state: Status,
    },
    NetworkChanged,
}

struct Record {
    sources: BTreeMap<String, Vec<SocketAddr>>,
    seen: Instant,
}
struct Book {
    state: Status,
    name: Option<String>,
    records: BTreeMap<String, Record>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Config {
    id: u64,
    interface_name: String,
    interface_index: u32,
    addresses: Vec<String>,
    name: String,
    port: u16,
    mode: Mode,
}

trait Adapter {
    fn browse(&mut self) -> Result<(), Status>;
    fn publish(&mut self, enabled: bool) -> Result<(), Status>;
    fn poll(&mut self) -> Result<Vec<Event>, Status>;
}

struct Task {
    config: Config,
    network: TransferInterface,
    stop: AtomicBool,
    book: Mutex<Book>,
    protocol: Option<Arc<v4::Session>>,
    trace: Trace,
}

pub(super) struct Job {
    task: Arc<Task>,
    thread: Option<JoinHandle<()>>,
}
pub(super) struct View(Arc<Task>);
impl View {
    pub fn snapshot(&self) -> LibraryTransferResult<Snapshot> {
        Ok(lock(&self.0.book)?.snapshot(self.0.config.id, Instant::now()))
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        self.task.stop.store(true, Ordering::Release);
        if let Ok(mut book) = self.task.book.lock() {
            book.records.clear();
            book.name = None;
            book.state = Status::Idle;
        }
        if let Some(thread) = self.thread.take() {
            // Browser cleanup must not block native UI callbacks on D-Bus or a pending plugin call.
            if self.task.protocol.is_none() {
                return;
            }
            let _ = thread.join();
        }
    }
}

impl Book {
    fn prune(&mut self, now: Instant) {
        self.records
            .retain(|_, r| now.duration_since(r.seen) < RETAIN);
    }
    fn apply(
        &mut self,
        event: Event,
        network: &TransferInterface,
        mode: Mode,
        now: Instant,
    ) -> Result<(), Status> {
        match event {
            Event::Peer {
                name,
                source,
                addresses,
                port,
                txt,
            } => {
                if !label(&name) || source.len() > 128 {
                    return Ok(());
                }
                if addresses.len() > ADDRESSES || !metadata(&txt, mode) {
                    return self.apply(Event::Lost { name, source }, network, mode, now);
                }
                let mut valid = addresses
                    .into_iter()
                    .filter_map(|ip| {
                        // An IPv6 scope belongs to the local selected interface, not the remote host.
                        let ip = ip.split('%').next()?.parse::<IpAddr>().ok()?;
                        endpoints::parse_endpoint(&SocketAddr::new(ip, port).to_string(), network)
                            .ok()
                    })
                    .collect::<Vec<_>>();
                valid.sort();
                valid.dedup();
                if valid.is_empty() {
                    return self.apply(Event::Lost { name, source }, network, mode, now);
                }
                self.prune(now);
                if !self.records.contains_key(&name) && self.records.len() == PEERS {
                    return Err(Status::Limited);
                }
                let record = self.records.entry(name).or_insert_with(|| Record {
                    sources: BTreeMap::new(),
                    seen: now,
                });
                if !record.sources.contains_key(&source) && record.sources.len() == ADDRESSES {
                    return Err(Status::Limited);
                }
                if record
                    .sources
                    .iter()
                    .filter(|(key, _)| **key != source)
                    .flat_map(|(_, values)| values.iter())
                    .chain(valid.iter())
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    > ADDRESSES
                {
                    return Err(Status::Limited);
                }
                record.sources.insert(source, valid);
                record.seen = now;
            }
            Event::Lost { name, source } => {
                if let Some(record) = self.records.get_mut(&name) {
                    record.sources.remove(&source);
                    if record.sources.is_empty() {
                        self.records.remove(&name);
                    }
                }
            }
            Event::Ready { name } if label(&name) => {
                self.name = Some(name);
                self.state = Status::Advertising;
            }
            Event::Error { state } => {
                return Err(match state {
                    Status::Denied | Status::Unsupported | Status::Limited => state,
                    _ => Status::Unavailable,
                })
            }
            _ => {}
        }
        Ok(())
    }
    fn snapshot(&mut self, id: u64, now: Instant) -> Snapshot {
        self.prune(now);
        Snapshot {
            id,
            state: self.state,
            name: self.name.clone(),
            peers: self
                .records
                .iter()
                .map(|(name, r)| Peer {
                    id: name.clone(),
                    name: name.clone(),
                    addresses: r
                        .sources
                        .values()
                        .flatten()
                        .copied()
                        .collect::<std::collections::BTreeSet<_>>()
                        .into_iter()
                        .map(|a| a.to_string())
                        .collect(),
                })
                .collect(),
        }
    }
}
fn label(name: &str) -> bool {
    !name.is_empty() && name.len() <= 63 && !name.chars().any(char::is_control)
}
fn metadata(txt: &[Vec<u8>], mode: Mode) -> bool {
    if txt.len() != 3 || txt.iter().map(|v| v.len() + 1).sum::<usize>() > 512 {
        return false;
    }
    let expected = [
        b"v=1".as_slice(),
        b"p=PCLAN004",
        if mode == Mode::Push {
            b"m=push"
        } else {
            b"m=pull"
        },
    ];
    expected.iter().all(|value| {
        txt.iter()
            .filter(|field| field.as_slice() == *value)
            .count()
            == 1
    })
}

impl Job {
    pub fn view(&self) -> View {
        View(self.task.clone())
    }
    pub fn start(
        app: tauri::AppHandle,
        id: u64,
        network: TransferInterface,
        mode: Mode,
        protocol: Option<Arc<v4::Session>>,
        port: u16,
        trace: Trace,
    ) -> LibraryTransferResult<Self> {
        // Bound cancelled workers still waiting for a system API as well as active discovery.
        let slot =
            WorkerSlot::acquire().inspect_err(|_| trace.note("discovery", "resource-limit"))?;
        let mut random = [0u8; 8];
        getrandom::fill(&mut random).map_err(|_| "Could not create a discovery name")?;
        let name = format!(
            "Papercut-{}",
            random
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        );
        let task = Arc::new(Task {
            config: Config {
                id,
                interface_name: network.name.clone(),
                interface_index: network.index,
                addresses: network.addresses.iter().map(ToString::to_string).collect(),
                name,
                port,
                mode,
            },
            network,
            protocol,
            trace,
            stop: AtomicBool::new(false),
            book: Mutex::new(Book {
                state: Status::Starting,
                name: None,
                records: BTreeMap::new(),
            }),
        });
        let worker = task.clone();
        let thread = thread::Builder::new()
            .name("transfer-discovery".into())
            .spawn(move || {
                let _slot = slot;
                #[cfg(target_os = "linux")]
                let result = linux::Avahi::new(&worker.config)
                    .and_then(|adapter| run(adapter, &worker, Instant::now() + BROWSE));
                #[cfg(target_os = "android")]
                let result = android::Nsd::new(&app, &worker.config)
                    .and_then(|adapter| run(adapter, &worker, Instant::now() + BROWSE));
                #[cfg(any(target_os = "macos", target_os = "ios"))]
                let result = apple::Bonjour::new(&worker.config)
                    .and_then(|adapter| run(adapter, &worker, Instant::now() + BROWSE));
                #[cfg(windows)]
                let result = windows::Windows::new(&worker.config)
                    .and_then(|adapter| run(adapter, &worker, Instant::now() + BROWSE));
                #[cfg(not(any(
                    target_os = "linux",
                    target_os = "android",
                    target_os = "macos",
                    target_os = "ios",
                    windows
                )))]
                let result: Result<Status, Status> = Err(Status::Unsupported);
                let _ = app;
                let state = result.unwrap_or_else(|state| state);
                if let Ok(mut book) = worker.book.lock() {
                    book.state = state;
                    if !matches!(state, Status::Finished) {
                        book.records.clear();
                    }
                    book.name = None;
                }
                worker.trace.note(
                    "discovery",
                    match state {
                        Status::Denied => "permission-denied",
                        Status::Unsupported => "unsupported",
                        Status::Limited => "resource-limit",
                        Status::Unavailable => "unavailable",
                        _ => "stopped",
                    },
                );
            })
            .map_err(|_| "Could not start discovery")?;
        Ok(Self {
            task,
            thread: Some(thread),
        })
    }
    pub fn snapshot(&self) -> LibraryTransferResult<Snapshot> {
        Ok(lock(&self.task.book)?.snapshot(self.task.config.id, Instant::now()))
    }
}

fn run(mut adapter: impl Adapter, task: &Task, deadline: Instant) -> Result<Status, Status> {
    if task.stop.load(Ordering::Acquire) {
        return Ok(Status::Idle);
    }
    let mut advertised = false;
    if task.protocol.is_none() {
        adapter.browse()?;
        task.book.lock().map_err(|_| Status::Unavailable)?.state = Status::Searching;
    }
    let mut checked = Instant::now();
    loop {
        if task.stop.load(Ordering::Acquire) {
            return Ok(Status::Idle);
        }
        if checked.elapsed() >= Duration::from_millis(250) {
            if !endpoints::still_available(&task.network) {
                return Err(Status::Unavailable);
            }
            checked = Instant::now();
        }
        if let Some(protocol) = &task.protocol {
            let phase = protocol.phase().map_err(|_| Status::Unavailable)?;
            if phase.terminal() {
                return Ok(Status::Idle);
            }
            let waiting = matches!(
                phase,
                Phase::Waiting | Phase::WaitingResume | Phase::Authenticating
            );
            if waiting != advertised {
                adapter.publish(waiting)?;
                advertised = waiting;
                task.book.lock().map_err(|_| Status::Unavailable)?.state = if waiting {
                    Status::Starting
                } else {
                    Status::Idle
                };
            }
        } else if Instant::now() >= deadline {
            return Ok(Status::Finished);
        }
        let events = adapter.poll()?;
        if task.stop.load(Ordering::Acquire) {
            return Ok(Status::Idle);
        }
        if events.len() > EVENTS {
            return Err(Status::Limited);
        }
        let mut book = task.book.lock().map_err(|_| Status::Unavailable)?;
        for event in events {
            if matches!(event, Event::NetworkChanged) {
                if let Some(protocol) = &task.protocol {
                    let _ = protocol.cancel();
                }
                return Err(Status::Unavailable);
            }
            if matches!(event, Event::Ready { .. }) && !advertised {
                continue;
            }
            book.apply(event, &task.network, task.config.mode, Instant::now())?;
        }
        drop(book);
        thread::sleep(Duration::from_millis(50));
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BrowseRequest {
    interface_id: String,
    mode: Mode,
}

#[tauri::command]
pub fn library_transfer_discovery_start(
    app: tauri::AppHandle,
    state: State<'_, LibraryTransferState>,
    request: BrowseRequest,
) -> LibraryTransferResult<Snapshot> {
    let epoch = state.check_foreground()?;
    let network =
        endpoints::select_interface(&endpoints::interfaces()?, Some(&request.interface_id))?;
    let foreground = lock(&state.backgrounded)?;
    if *foreground || state.visibility_epoch.load(Ordering::Acquire) != epoch {
        return Err("App visibility changed. Retry discovery.".into());
    }
    let mut current = lock(&state.discovery)?;
    if state.active.load(Ordering::Acquire) {
        return Err("A library transfer is already active".into());
    }
    current.take();
    let id = state.next_id.fetch_add(1, Ordering::Relaxed) + 1;
    let trace = state.diagnostics.begin("discovery");
    trace.stage("discovery");
    let job = Job::start(app, id, network, request.mode, None, 0, trace)?;
    let snapshot = job.snapshot()?;
    *current = Some(job);
    Ok(snapshot)
}
#[tauri::command]
pub fn library_transfer_discovery_status(
    state: State<'_, LibraryTransferState>,
    id: u64,
) -> LibraryTransferResult<Option<Snapshot>> {
    lock(&state.discovery)?
        .as_ref()
        .filter(|job| job.task.config.id == id)
        .map(Job::snapshot)
        .transpose()
}
#[tauri::command]
pub fn library_transfer_discovery_stop(
    state: State<'_, LibraryTransferState>,
    id: u64,
) -> LibraryTransferResult<()> {
    let mut current = lock(&state.discovery)?;
    if current.as_ref().is_some_and(|job| job.task.config.id == id) {
        current.take();
    }
    Ok(())
}
#[tauri::command]
pub fn library_transfer_discovery_select(
    state: State<'_, LibraryTransferState>,
    id: u64,
    peer: String,
) -> LibraryTransferResult<String> {
    state.check_foreground()?;
    let current = lock(&state.discovery)?;
    let job = current
        .as_ref()
        .filter(|job| job.task.config.id == id)
        .ok_or("Discovery expired. Refresh nearby sessions.")?;
    if !endpoints::still_available(&job.task.network) {
        return Err("The selected network changed. Refresh nearby sessions.".into());
    }
    let snapshot = job.snapshot()?;
    let value = snapshot
        .peers
        .iter()
        .find(|p| p.id == peer)
        .and_then(|p| p.addresses.first())
        .ok_or("This session is no longer available. Refresh nearby sessions.")?;
    Ok(endpoints::parse_endpoint(value, &job.task.network)?.to_string())
}

#[cfg(test)]
mod tests;
