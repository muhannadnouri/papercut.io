//! Windows DNS-SD callbacks use opaque registry IDs, never pointers into a dropped adapter.
use super::{Adapter, Config, Event, Mode, Status, EVENTS, PEERS};
use std::cell::UnsafeCell;
use std::collections::BTreeMap;
use std::ffi::c_void;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::ptr;
use std::sync::{
    atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering},
    mpsc, Arc, Mutex, OnceLock,
};
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{DNS_REQUEST_PENDING, ERROR_CANCELLED};
use windows_sys::Win32::NetworkManagement::Dns::*;

#[derive(Clone, Copy)]
struct Api {
    browse: unsafe extern "system" fn(
        *const DNS_SERVICE_BROWSE_REQUEST,
        *mut DNS_SERVICE_CANCEL,
    ) -> i32,
    browse_cancel: unsafe extern "system" fn(*const DNS_SERVICE_CANCEL) -> i32,
    resolve: unsafe extern "system" fn(
        *const DNS_SERVICE_RESOLVE_REQUEST,
        *mut DNS_SERVICE_CANCEL,
    ) -> i32,
    resolve_cancel: unsafe extern "system" fn(*const DNS_SERVICE_CANCEL) -> i32,
    register: unsafe extern "system" fn(
        *const DNS_SERVICE_REGISTER_REQUEST,
        *mut DNS_SERVICE_CANCEL,
    ) -> u32,
    register_cancel: unsafe extern "system" fn(*const DNS_SERVICE_CANCEL) -> u32,
    deregister: unsafe extern "system" fn(
        *const DNS_SERVICE_REGISTER_REQUEST,
        *mut DNS_SERVICE_CANCEL,
    ) -> u32,
    construct: unsafe extern "system" fn(
        *const u16,
        *const u16,
        *const u32,
        *const IP6_ADDRESS,
        u16,
        u16,
        u16,
        u32,
        *const *const u16,
        *const *const u16,
    ) -> *mut DNS_SERVICE_INSTANCE,
    free_instance: unsafe extern "system" fn(*mut DNS_SERVICE_INSTANCE),
    free_records: unsafe extern "system" fn(*const c_void, DNS_FREE_TYPE),
    _module: usize,
}
#[cfg(windows)]
fn api() -> Result<&'static Api, Status> {
    use windows_sys::Win32::{Foundation::FreeLibrary, System::LibraryLoader::*};
    static API: OnceLock<Result<Api, Status>> = OnceLock::new();
    API.get_or_init(|| unsafe {
        let module = LoadLibraryExW(
            wide("dnsapi.dll").as_ptr(),
            ptr::null_mut(),
            LOAD_LIBRARY_SEARCH_SYSTEM32,
        );
        if module.is_null() {
            return Err(Status::Unsupported);
        }
        macro_rules! load {
            ($name:literal) => {
                std::mem::transmute(
                    GetProcAddress(module, concat!($name, "\0").as_ptr())
                        .ok_or(Status::Unsupported)?,
                )
            };
        }
        let result = (|| {
            Ok(Api {
                browse: load!("DnsServiceBrowse"),
                browse_cancel: load!("DnsServiceBrowseCancel"),
                resolve: load!("DnsServiceResolve"),
                resolve_cancel: load!("DnsServiceResolveCancel"),
                register: load!("DnsServiceRegister"),
                register_cancel: load!("DnsServiceRegisterCancel"),
                deregister: load!("DnsServiceDeRegister"),
                construct: load!("DnsServiceConstructInstance"),
                free_instance: load!("DnsServiceFreeInstance"),
                free_records: load!("DnsFree"),
                _module: module as usize,
            })
        })();
        // The successfully loaded module stays process-owned so even late cleanup callbacks can free results.
        if result.is_err() {
            FreeLibrary(module);
        }
        result
    })
    .as_ref()
    .map_err(|state| *state)
}
#[cfg(not(windows))]
fn api() -> Result<&'static Api, Status> {
    Err(Status::Unsupported)
}

enum Signal {
    Found {
        name: String,
        query: String,
    },
    Lost(String),
    Resolved {
        id: usize,
        name: String,
        value: Option<Resolved>,
    },
    Ready {
        id: usize,
        name: String,
    },
    Error {
        id: usize,
        state: Status,
    },
}
struct Resolved {
    addresses: Vec<String>,
    port: u16,
    txt: Vec<Vec<u8>>,
}
struct Context {
    index: u32,
    send: mpsc::SyncSender<Signal>,
    overflow: AtomicBool,
    closed: AtomicBool,
}
impl Context {
    fn push(&self, signal: Signal) {
        if !self.closed.load(Ordering::Acquire) && self.send.try_send(signal).is_err() {
            self.overflow.store(true, Ordering::Release);
        }
    }
}
enum Request {
    Browse(Box<DNS_SERVICE_BROWSE_REQUEST>),
    Resolve(Box<DNS_SERVICE_RESOLVE_REQUEST>),
    Register(Box<DNS_SERVICE_REGISTER_REQUEST>),
}
struct Call {
    id: usize,
    api: &'static Api,
    context: Arc<Context>,
    name: String,
    _query: Vec<u16>,
    request: Request,
    cancel: UnsafeCell<DNS_SERVICE_CANCEL>,
    instance: *mut DNS_SERVICE_INSTANCE,
    state: AtomicU8, // 0 pending, 1 registered, 2 deregistering, 3 complete.
    closing: AtomicBool,
}
// Request buffers never move or change after submission; only the worker uses the OS cancel handle.
// Callbacks use atomics and the registry, and keep an Arc alive through inline or concurrent completion.
unsafe impl Send for Call {}
unsafe impl Sync for Call {}
static CALLS: OnceLock<Mutex<BTreeMap<usize, Arc<Call>>>> = OnceLock::new();
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn calls() -> std::sync::MutexGuard<'static, BTreeMap<usize, Arc<Call>>> {
    CALLS
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}
fn finished(call: &Call) {
    call.state.store(3, Ordering::Release);
    let removed = { calls().remove(&call.id) };
    drop(removed);
}
impl Call {
    fn submit(self) -> Result<Arc<Self>, Status> {
        let call = Arc::new(self);
        {
            let mut entries = calls();
            // Cancelled OS calls retain their buffers until final acknowledgement, even after worker exit.
            if entries.len() >= 2 * (PEERS + 1) {
                return Err(Status::Limited);
            }
            entries.insert(call.id, call.clone());
        }
        let status = unsafe {
            match &call.request {
                Request::Browse(request) => {
                    (call.api.browse)(request.as_ref(), call.cancel.get()) as u32
                }
                Request::Resolve(request) => {
                    (call.api.resolve)(request.as_ref(), call.cancel.get()) as u32
                }
                Request::Register(request) => {
                    (call.api.register)(request.as_ref(), call.cancel.get())
                }
            }
        };
        if status != DNS_REQUEST_PENDING as u32 {
            finished(&call);
            return Err(classify(status));
        }
        Ok(call)
    }
    fn close(&self) {
        if self.closing.swap(true, Ordering::AcqRel) {
            return;
        }
        let status = unsafe {
            match &self.request {
                Request::Browse(_) if self.state.load(Ordering::Acquire) != 3 => {
                    Some((self.api.browse_cancel)(self.cancel.get()) as u32)
                }
                Request::Resolve(_) if self.state.load(Ordering::Acquire) != 3 => {
                    Some((self.api.resolve_cancel)(self.cancel.get()) as u32)
                }
                Request::Register(_) if self.state.load(Ordering::Acquire) == 0 => {
                    Some((self.api.register_cancel)(self.cancel.get()))
                }
                Request::Register(_) => {
                    self.deregister();
                    None
                }
                _ => None,
            }
        };
        // Completion can race cancellation; its final callback remains the cleanup authority.
        if let Some(status) = status.filter(|status| *status == 5) {
            self.context.push(Signal::Error {
                id: self.id,
                state: classify(status),
            });
        }
    }
    fn deregister(&self) {
        if self
            .state
            .compare_exchange(1, 2, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            if let Request::Register(request) = &self.request {
                let status = unsafe { (self.api.deregister)(request.as_ref(), ptr::null_mut()) };
                if status != DNS_REQUEST_PENDING as u32 {
                    self.context.push(Signal::Error {
                        id: self.id,
                        state: classify(status),
                    });
                }
            }
        }
    }
}
impl Drop for Call {
    fn drop(&mut self) {
        if !self.instance.is_null() {
            unsafe { (self.api.free_instance)(self.instance) };
        }
    }
}
struct Peer {
    name: String,
    call: Arc<Call>,
}
pub(super) struct Windows {
    config: Config,
    api: &'static Api,
    context: Arc<Context>,
    events: mpsc::Receiver<Signal>,
    browser: Option<Arc<Call>>,
    publication: Option<Arc<Call>>,
    wanted: bool,
    cleanup_deadline: Option<Instant>,
    peers: BTreeMap<String, Peer>,
}
impl Windows {
    #[cfg(windows)]
    pub fn new(config: &Config) -> Result<Self, Status> {
        Self::with_api(config, api()?)
    }
    fn with_api(config: &Config, api: &'static Api) -> Result<Self, Status> {
        if config.interface_index == 0 {
            return Err(Status::Unsupported);
        }
        let (send, events) = mpsc::sync_channel(EVENTS / 2);
        Ok(Self {
            config: config.clone(),
            api,
            context: Arc::new(Context {
                index: config.interface_index,
                send,
                overflow: AtomicBool::new(false),
                closed: AtomicBool::new(false),
            }),
            events,
            browser: None,
            publication: None,
            wanted: false,
            cleanup_deadline: None,
            peers: BTreeMap::new(),
        })
    }
    fn call(&self, name: String, query: String, register: bool) -> Result<Arc<Call>, Status> {
        let id = NEXT.fetch_add(1, Ordering::Relaxed) + 1;
        let query = wide(&query);
        let ctx = id as *mut c_void;
        let mut instance = ptr::null_mut();
        let request = if register {
            let ip4 = self
                .config
                .addresses
                .iter()
                .filter_map(|ip| ip.parse::<Ipv4Addr>().ok())
                .next()
                .map(|ip| u32::from_ne_bytes(ip.octets()));
            let ip6 = self
                .config
                .addresses
                .iter()
                .filter_map(|ip| ip.parse::<Ipv6Addr>().ok())
                .next()
                .map(|ip| IP6_ADDRESS {
                    IP6Byte: ip.octets(),
                });
            let host = wide(&format!("{}.local", self.config.name));
            let keys = [wide("v"), wide("p"), wide("m")];
            let values = [
                wide("1"),
                wide("PCLAN004"),
                wide(if self.config.mode == Mode::Push {
                    "push"
                } else {
                    "pull"
                }),
            ];
            let key_ptrs = keys.each_ref().map(|v| v.as_ptr());
            let value_ptrs = values.each_ref().map(|v| v.as_ptr());
            instance = unsafe {
                (self.api.construct)(
                    query.as_ptr(),
                    host.as_ptr(),
                    ip4.as_ref().map_or(ptr::null(), ptr::from_ref),
                    ip6.as_ref().map_or(ptr::null(), ptr::from_ref),
                    self.config.port,
                    0,
                    0,
                    3,
                    key_ptrs.as_ptr(),
                    value_ptrs.as_ptr(),
                )
            };
            if instance.is_null() {
                return Err(Status::Unavailable);
            }
            unsafe {
                (*instance).dwInterfaceIndex = self.config.interface_index;
            }
            Request::Register(Box::new(DNS_SERVICE_REGISTER_REQUEST {
                Version: 1,
                InterfaceIndex: self.config.interface_index,
                pServiceInstance: instance,
                pRegisterCompletionCallback: Some(registered),
                pQueryContext: ctx,
                hCredentials: ptr::null_mut(),
                unicastEnabled: 0,
            }))
        } else if name.is_empty() {
            Request::Browse(Box::new(DNS_SERVICE_BROWSE_REQUEST {
                Version: 1,
                InterfaceIndex: self.config.interface_index,
                QueryName: query.as_ptr(),
                Anonymous: DNS_SERVICE_BROWSE_REQUEST_0 {
                    pBrowseCallback: Some(browsed),
                },
                pQueryContext: ctx,
            }))
        } else {
            Request::Resolve(Box::new(DNS_SERVICE_RESOLVE_REQUEST {
                Version: 1,
                InterfaceIndex: self.config.interface_index,
                QueryName: query.as_ptr().cast_mut(),
                pResolveCompletionCallback: Some(resolved),
                pQueryContext: ctx,
            }))
        };
        Call {
            id,
            api: self.api,
            context: self.context.clone(),
            name,
            _query: query,
            request,
            cancel: UnsafeCell::new(DNS_SERVICE_CANCEL::default()),
            instance,
            state: AtomicU8::new(0),
            closing: AtomicBool::new(false),
        }
        .submit()
    }
    fn publication(&mut self) -> Result<(), Status> {
        if !self.wanted {
            return Ok(());
        }
        if self.publication.as_ref().is_some_and(|p| {
            p.closing.load(Ordering::Acquire) && p.state.load(Ordering::Acquire) != 3
        }) {
            if self
                .cleanup_deadline
                .is_some_and(|deadline| Instant::now() >= deadline)
            {
                return Err(Status::Unavailable);
            }
            return Ok(());
        }
        if self
            .publication
            .as_ref()
            .is_none_or(|p| p.state.load(Ordering::Acquire) == 3)
        {
            self.publication = Some(self.call(
                self.config.name.clone(),
                format!("{}._papercut-xfer._tcp.local", self.config.name),
                true,
            )?);
            self.cleanup_deadline = None;
        }
        Ok(())
    }
}
impl Adapter for Windows {
    fn browse(&mut self) -> Result<(), Status> {
        self.browser = Some(self.call(String::new(), "_papercut-xfer._tcp.local".into(), false)?);
        Ok(())
    }
    fn publish(&mut self, enabled: bool) -> Result<(), Status> {
        self.wanted = enabled;
        if !enabled {
            if let Some(call) = &self.publication {
                call.close();
                self.cleanup_deadline = Some(Instant::now() + Duration::from_secs(5));
            }
        }
        self.publication()
    }
    fn poll(&mut self) -> Result<Vec<Event>, Status> {
        self.publication()?;
        if self.context.overflow.load(Ordering::Acquire) {
            return Err(Status::Limited);
        }
        let mut result = Vec::new();
        for signal in self.events.try_iter().take(EVENTS).collect::<Vec<_>>() {
            match signal {
                Signal::Found { name, query } => {
                    let key = name.to_ascii_lowercase();
                    if self
                        .peers
                        .get(&key)
                        .is_some_and(|p| p.call.state.load(Ordering::Acquire) != 3)
                    {
                        continue;
                    }
                    if !self.peers.contains_key(&key) && self.peers.len() == PEERS {
                        return Err(Status::Limited);
                    }
                    let call = self.call(name.clone(), query, false)?;
                    self.peers.insert(key, Peer { name, call });
                }
                Signal::Lost(name) => {
                    let name = if let Some(peer) = self.peers.remove(&name.to_ascii_lowercase()) {
                        peer.call.close();
                        peer.name
                    } else {
                        name
                    };
                    result.push(Event::Lost {
                        name,
                        source: "windows".into(),
                    });
                }
                Signal::Resolved { id, name, value } => {
                    let Some(peer) = self
                        .peers
                        .get(&name.to_ascii_lowercase())
                        .filter(|p| p.call.id == id)
                    else {
                        continue;
                    };
                    result.push(match value {
                        Some(value) => Event::Peer {
                            name: peer.name.clone(),
                            source: "windows".into(),
                            addresses: value.addresses,
                            port: value.port,
                            txt: value.txt,
                        },
                        None => Event::Lost {
                            name: peer.name.clone(),
                            source: "windows".into(),
                        },
                    });
                }
                Signal::Ready { id, name }
                    if self.wanted
                        && self
                            .publication
                            .as_ref()
                            .is_some_and(|p| p.id == id && !p.closing.load(Ordering::Acquire)) =>
                {
                    result.push(Event::Ready { name })
                }
                Signal::Error { id, state }
                    if self.browser.as_ref().is_some_and(|p| p.id == id)
                        || self.publication.as_ref().is_some_and(|p| p.id == id)
                        || self.peers.values().any(|p| p.call.id == id) =>
                {
                    return Err(state)
                }
                _ => {}
            }
        }
        Ok(result)
    }
}
impl Drop for Windows {
    fn drop(&mut self) {
        self.context.closed.store(true, Ordering::Release);
        for call in self
            .browser
            .iter()
            .chain(self.publication.iter())
            .chain(self.peers.values().map(|p| &p.call))
        {
            call.close();
        }
        while self.events.try_recv().is_ok() {}
    }
}
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
unsafe fn text(value: *const u16, limit: usize) -> Option<String> {
    if value.is_null() {
        return None;
    }
    let length = (0..=limit).find(|n| unsafe { *value.add(*n) } == 0)?;
    String::from_utf16(unsafe { std::slice::from_raw_parts(value, length) }).ok()
}
fn label(full: &str) -> Option<String> {
    let name = full.trim_end_matches('.');
    let suffix = "._papercut-xfer._tcp.local";
    if !name.to_ascii_lowercase().ends_with(suffix) {
        return None;
    }
    let prefix = name.get(..name.len().checked_sub(suffix.len())?)?;
    super::label(prefix).then(|| prefix.to_owned())
}
fn classify(status: u32) -> Status {
    if status == 5 {
        Status::Denied
    } else {
        Status::Unavailable
    }
}
fn lookup(ctx: *const c_void) -> Option<Arc<Call>> {
    calls().get(&(ctx as usize)).cloned()
}
unsafe extern "system" fn browsed(status: u32, ctx: *const c_void, records: *const DNS_RECORDW) {
    let call = lookup(ctx);
    let api = call.as_ref().map(|c| c.api).or_else(|| api().ok());
    if let Some(call) = call {
        if status == ERROR_CANCELLED {
            if !call.closing.load(Ordering::Acquire) {
                call.context.push(Signal::Error {
                    id: call.id,
                    state: Status::Unavailable,
                });
            }
            finished(&call);
        } else if !call.closing.load(Ordering::Acquire) {
            if status != 0 {
                call.context.push(Signal::Error {
                    id: call.id,
                    state: classify(status),
                });
            } else {
                let mut record = ptr::NonNull::new(records.cast_mut());
                for _ in 0..EVENTS / 2 {
                    let Some(current) = record else { break };
                    // SAFETY: Windows owns the non-null list until this callback frees it below.
                    let row = unsafe { current.as_ref() };
                    if row.wType == DNS_TYPE_PTR
                        && unsafe { text(row.pName, 253) }.is_some_and(|v| {
                            v.trim_end_matches('.')
                                .eq_ignore_ascii_case("_papercut-xfer._tcp.local")
                        })
                    {
                        if let Some(query) = unsafe { text(row.Data.PTR.pNameHost, 253) } {
                            if let Some(name) = label(&query) {
                                call.context.push(if row.dwTtl == 0 {
                                    Signal::Lost(name)
                                } else {
                                    Signal::Found { name, query }
                                });
                            }
                        }
                    }
                    record = ptr::NonNull::new(row.pNext);
                }
                if record.is_some() {
                    call.context.overflow.store(true, Ordering::Release);
                }
            }
        }
    }
    if !records.is_null() {
        if let Some(api) = api {
            unsafe { (api.free_records)(records.cast(), DnsFreeRecordList) };
        }
    }
}
unsafe fn instance(value: *const DNS_SERVICE_INSTANCE, call: &Call) -> Option<Resolved> {
    if value.is_null() {
        return None;
    }
    let value = unsafe { &*value };
    if value.dwInterfaceIndex != call.context.index
        || value.dwPropertyCount != 3
        || value.keys.is_null()
        || value.values.is_null()
    {
        return None;
    }
    if !label(&unsafe { text(value.pszInstanceName, 253) }?)?.eq_ignore_ascii_case(&call.name)
        || !unsafe { text(value.pszHostName, 253) }?
            .trim_end_matches('.')
            .to_ascii_lowercase()
            .ends_with(".local")
    {
        return None;
    }
    let mut txt = Vec::new();
    let mut size = 0;
    for i in 0..3 {
        let key = unsafe { text(*value.keys.add(i), 63) }?;
        let field = format!("{key}={}", unsafe { text(*value.values.add(i), 512) }?).into_bytes();
        size += field.len() + 1;
        if size > 512 {
            return None;
        }
        txt.push(field);
    }
    let mut addresses = Vec::new();
    if !value.ip4Address.is_null() {
        addresses.push(Ipv4Addr::from(unsafe { *value.ip4Address }.to_ne_bytes()).to_string());
    }
    if !value.ip6Address.is_null() {
        addresses.push(Ipv6Addr::from(unsafe { (*value.ip6Address).IP6Byte }).to_string());
    }
    Some(Resolved {
        addresses,
        port: value.wPort,
        txt,
    })
}
unsafe extern "system" fn resolved(
    status: u32,
    ctx: *const c_void,
    value: *const DNS_SERVICE_INSTANCE,
) {
    let call = lookup(ctx);
    let api = call.as_ref().map(|c| c.api).or_else(|| api().ok());
    if let Some(call) = call {
        if !call.closing.load(Ordering::Acquire) {
            if status == 5 {
                call.context.push(Signal::Error {
                    id: call.id,
                    state: Status::Denied,
                });
            } else {
                call.context.push(Signal::Resolved {
                    id: call.id,
                    name: call.name.clone(),
                    value: if status == 0 {
                        unsafe { instance(value, &call) }
                    } else {
                        None
                    },
                });
            }
        }
        finished(&call);
    }
    if !value.is_null() {
        if let Some(api) = api {
            unsafe { (api.free_instance)(value.cast_mut()) };
        }
    }
}
unsafe extern "system" fn registered(
    status: u32,
    ctx: *const c_void,
    value: *const DNS_SERVICE_INSTANCE,
) {
    let call = lookup(ctx);
    let api = call.as_ref().map(|c| c.api).or_else(|| api().ok());
    if let Some(call) = call {
        if call.state.load(Ordering::Acquire) == 2 || status != 0 {
            if status != 0 && !call.closing.load(Ordering::Acquire) {
                call.context.push(Signal::Error {
                    id: call.id,
                    state: classify(status),
                });
            }
            finished(&call);
        } else {
            call.state.store(1, Ordering::Release);
            if call.closing.load(Ordering::Acquire) {
                call.deregister();
            } else if let Some(name) = unsafe {
                text(
                    if value.is_null() {
                        ptr::null()
                    } else {
                        (*value).pszInstanceName
                    },
                    253,
                )
            }
            .and_then(|name| label(&name))
            {
                call.context.push(Signal::Ready { id: call.id, name });
            } else {
                call.context.push(Signal::Error {
                    id: call.id,
                    state: Status::Unavailable,
                });
            }
        }
    }
    if !value.is_null() {
        if let Some(api) = api {
            unsafe { (api.free_instance)(value.cast_mut()) };
        }
    }
}

#[cfg(test)]
mod tests;
