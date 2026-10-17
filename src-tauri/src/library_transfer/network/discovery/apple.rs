//! Bonjour references and callbacks stay on one discovery worker; no dispatch queue or shared refs.
use std::collections::BTreeMap;
use std::ffi::{c_char, c_void, CString};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::ptr;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};

use super::{Adapter, Config, Event, Status, ADDRESSES, EVENTS, PEERS, SERVICE};

type Ref = *mut c_void;
type BrowseCallback = unsafe extern "C" fn(
    Ref,
    u32,
    u32,
    i32,
    *const c_char,
    *const c_char,
    *const c_char,
    *mut c_void,
);
type ResolveCallback = unsafe extern "C" fn(
    Ref,
    u32,
    u32,
    i32,
    *const c_char,
    *const c_char,
    u16,
    u16,
    *const u8,
    *mut c_void,
);
type AddressCallback = unsafe extern "C" fn(
    Ref,
    u32,
    u32,
    i32,
    *const c_char,
    *const libc::sockaddr,
    u32,
    *mut c_void,
);
type RegisterCallback =
    unsafe extern "C" fn(Ref, u32, i32, *const c_char, *const c_char, *const c_char, *mut c_void);
#[derive(Clone, Copy)]
struct Api {
    browse: unsafe extern "C" fn(
        *mut Ref,
        u32,
        u32,
        *const c_char,
        *const c_char,
        BrowseCallback,
        *mut c_void,
    ) -> i32,
    resolve: unsafe extern "C" fn(
        *mut Ref,
        u32,
        u32,
        *const c_char,
        *const c_char,
        *const c_char,
        ResolveCallback,
        *mut c_void,
    ) -> i32,
    address: unsafe extern "C" fn(
        *mut Ref,
        u32,
        u32,
        u32,
        *const c_char,
        AddressCallback,
        *mut c_void,
    ) -> i32,
    register: unsafe extern "C" fn(
        *mut Ref,
        u32,
        u32,
        *const c_char,
        *const c_char,
        *const c_char,
        *const c_char,
        u16,
        u16,
        *const c_void,
        RegisterCallback,
        *mut c_void,
    ) -> i32,
    fd: unsafe extern "C" fn(Ref) -> i32,
    process: unsafe extern "C" fn(Ref) -> i32,
    free: unsafe extern "C" fn(Ref),
}
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod native {
    use super::*;
    #[link(name = "dns_sd")]
    unsafe extern "C" {
        fn DNSServiceBrowse(
            r: *mut Ref,
            flags: u32,
            index: u32,
            kind: *const c_char,
            domain: *const c_char,
            cb: BrowseCallback,
            ctx: *mut c_void,
        ) -> i32;
        fn DNSServiceResolve(
            r: *mut Ref,
            flags: u32,
            index: u32,
            name: *const c_char,
            kind: *const c_char,
            domain: *const c_char,
            cb: ResolveCallback,
            ctx: *mut c_void,
        ) -> i32;
        fn DNSServiceGetAddrInfo(
            r: *mut Ref,
            flags: u32,
            index: u32,
            protocol: u32,
            host: *const c_char,
            cb: AddressCallback,
            ctx: *mut c_void,
        ) -> i32;
        fn DNSServiceRegister(
            r: *mut Ref,
            flags: u32,
            index: u32,
            name: *const c_char,
            kind: *const c_char,
            domain: *const c_char,
            host: *const c_char,
            port: u16,
            txt_len: u16,
            txt: *const c_void,
            cb: RegisterCallback,
            ctx: *mut c_void,
        ) -> i32;
        fn DNSServiceRefSockFD(r: Ref) -> i32;
        fn DNSServiceProcessResult(r: Ref) -> i32;
        fn DNSServiceRefDeallocate(r: Ref);
    }
    pub(super) const API: Api = Api {
        browse: DNSServiceBrowse,
        resolve: DNSServiceResolve,
        address: DNSServiceGetAddrInfo,
        register: DNSServiceRegister,
        fd: DNSServiceRefSockFD,
        process: DNSServiceProcessResult,
        free: DNSServiceRefDeallocate,
    };
}

enum Signal {
    Browse {
        name: String,
        add: bool,
    },
    Resolved {
        name: String,
        token: u64,
        host: String,
        port: u16,
        txt: Vec<Vec<u8>>,
    },
    Address {
        name: String,
        token: u64,
        ip: IpAddr,
        add: bool,
    },
    Invalid {
        name: String,
        token: u64,
    },
    Ready {
        token: u64,
        name: String,
    },
    Error {
        name: String,
        token: u64,
        state: Status,
    },
}
struct Context {
    index: u32,
    name: String,
    token: u64,
    send: mpsc::SyncSender<Signal>,
    overflow: Arc<AtomicBool>,
}
impl Context {
    fn push(&self, signal: Signal) {
        if self.send.try_send(signal).is_err() {
            self.overflow.store(true, Ordering::Release);
        }
    }
    fn fail(&self, state: Status) {
        self.push(Signal::Error {
            name: self.name.clone(),
            token: self.token,
            state,
        });
    }
    fn error(&self, code: i32) -> bool {
        if code == 0 {
            return false;
        }
        self.fail(classify(code));
        true
    }
}
struct Handle {
    raw: Ref,
    context: Box<Context>,
    api: Api,
}
impl Drop for Handle {
    fn drop(&mut self) {
        // ProcessResult, callbacks and disposal all run on this worker, before the context is freed.
        if !self.raw.is_null() {
            unsafe { (self.api.free)(self.raw) };
        }
    }
}
struct Peer {
    token: u64,
    resolve: Handle,
    address: Option<Handle>,
    host: String,
    port: u16,
    txt: Vec<Vec<u8>>,
    addresses: Vec<IpAddr>,
}
pub(super) struct Bonjour {
    config: Config,
    api: Api,
    send: mpsc::SyncSender<Signal>,
    events: mpsc::Receiver<Signal>,
    overflow: Arc<AtomicBool>,
    browser: Option<Handle>,
    publication: Option<Handle>,
    peers: BTreeMap<String, Peer>,
    next: u64,
}
impl Bonjour {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    pub fn new(config: &Config) -> Result<Self, Status> {
        Self::with_api(config, native::API)
    }
    fn with_api(config: &Config, api: Api) -> Result<Self, Status> {
        if config.interface_index == 0 {
            return Err(Status::Unsupported);
        }
        let (send, events) = mpsc::sync_channel(EVENTS / 2);
        Ok(Self {
            config: config.clone(),
            api,
            send,
            events,
            overflow: Arc::new(AtomicBool::new(false)),
            browser: None,
            publication: None,
            peers: BTreeMap::new(),
            next: 0,
        })
    }
    fn handle(&self, name: String, token: u64) -> Handle {
        Handle {
            raw: ptr::null_mut(),
            api: self.api,
            context: Box::new(Context {
                index: self.config.interface_index,
                name,
                token,
                send: self.send.clone(),
                overflow: self.overflow.clone(),
            }),
        }
    }
    fn peer_event(name: String, peer: &Peer) -> Event {
        if peer.addresses.is_empty() {
            return Event::Lost {
                name,
                source: "bonjour".into(),
            };
        }
        Event::Peer {
            name,
            source: "bonjour".into(),
            addresses: peer.addresses.iter().map(ToString::to_string).collect(),
            port: peer.port,
            txt: peer.txt.clone(),
        }
    }
    fn signals(&mut self) -> Result<Vec<Event>, Status> {
        if self.overflow.load(Ordering::Acquire) {
            return Err(Status::Limited);
        }
        let mut result = Vec::new();
        for signal in self.events.try_iter().take(EVENTS).collect::<Vec<_>>() {
            match signal {
                Signal::Browse { name, add: false } => {
                    self.peers.remove(&name);
                    result.push(Event::Lost {
                        name,
                        source: "bonjour".into(),
                    });
                }
                Signal::Browse { name, add: true } => {
                    if self.peers.contains_key(&name) {
                        continue;
                    }
                    if self.peers.len() == PEERS {
                        return Err(Status::Limited);
                    }
                    self.next += 1;
                    let mut resolve = self.handle(name.clone(), self.next);
                    let c_name = CString::new(name.as_str()).map_err(|_| Status::Unavailable)?;
                    check(unsafe {
                        (self.api.resolve)(
                            &mut resolve.raw,
                            0,
                            self.config.interface_index,
                            c_name.as_ptr(),
                            c"_papercut-xfer._tcp".as_ptr(),
                            c"local.".as_ptr(),
                            resolved,
                            ptr::from_mut(resolve.context.as_mut()).cast(),
                        )
                    })?;
                    self.peers.insert(
                        name,
                        Peer {
                            token: self.next,
                            resolve,
                            address: None,
                            host: String::new(),
                            port: 0,
                            txt: vec![],
                            addresses: vec![],
                        },
                    );
                }
                Signal::Resolved {
                    name,
                    token,
                    host,
                    port,
                    txt,
                } => {
                    let Some(peer) = self.peers.get(&name).filter(|p| p.token == token) else {
                        continue;
                    };
                    let address = if peer.host != host {
                        self.next += 1;
                        let mut address = self.handle(name.clone(), self.next);
                        let c_host =
                            CString::new(host.as_str()).map_err(|_| Status::Unavailable)?;
                        check(unsafe {
                            (self.api.address)(
                                &mut address.raw,
                                0,
                                self.config.interface_index,
                                3,
                                c_host.as_ptr(),
                                addressed,
                                ptr::from_mut(address.context.as_mut()).cast(),
                            )
                        })?;
                        Some(address)
                    } else {
                        None
                    };
                    let peer = self.peers.get_mut(&name).ok_or(Status::Unavailable)?;
                    if let Some(address) = address {
                        peer.address = Some(address);
                        peer.addresses.clear();
                        peer.host = host;
                    }
                    peer.port = port;
                    peer.txt = txt;
                    result.push(Self::peer_event(name, peer));
                }
                Signal::Address {
                    name,
                    token,
                    ip,
                    add,
                } => {
                    let Some(peer) = self
                        .peers
                        .get_mut(&name)
                        .filter(|p| p.address.as_ref().is_some_and(|a| a.context.token == token))
                    else {
                        continue;
                    };
                    if add && !peer.addresses.contains(&ip) {
                        if peer.addresses.len() == ADDRESSES {
                            return Err(Status::Limited);
                        }
                        peer.addresses.push(ip);
                    } else if !add {
                        peer.addresses.retain(|value| *value != ip);
                    }
                    result.push(Self::peer_event(name, peer));
                }
                Signal::Invalid { name, token } => {
                    if let Some(peer) = self.peers.get_mut(&name).filter(|p| p.token == token) {
                        peer.address = None;
                        peer.addresses.clear();
                        peer.txt.clear();
                        peer.host.clear();
                        result.push(Event::Lost {
                            name,
                            source: "bonjour".into(),
                        });
                    }
                }
                Signal::Ready { token, name }
                    if self
                        .publication
                        .as_ref()
                        .is_some_and(|p| p.context.token == token) =>
                {
                    result.push(Event::Ready { name })
                }
                Signal::Error { name, token, state } => {
                    let owned = (token == 0 && self.browser.is_some())
                        || self
                            .publication
                            .as_ref()
                            .is_some_and(|p| p.context.token == token)
                        || self.peers.get(&name).is_some_and(|p| {
                            p.token == token
                                || p.address.as_ref().is_some_and(|a| a.context.token == token)
                        });
                    if owned {
                        return Err(state);
                    }
                }
                _ => {}
            }
        }
        Ok(result)
    }
}
impl Adapter for Bonjour {
    fn browse(&mut self) -> Result<(), Status> {
        let mut handle = self.handle(String::new(), 0);
        check(unsafe {
            (self.api.browse)(
                &mut handle.raw,
                0,
                self.config.interface_index,
                c"_papercut-xfer._tcp".as_ptr(),
                c"local.".as_ptr(),
                browsed,
                ptr::from_mut(handle.context.as_mut()).cast(),
            )
        })?;
        self.browser = Some(handle);
        Ok(())
    }
    fn publish(&mut self, enabled: bool) -> Result<(), Status> {
        self.publication = None;
        if enabled {
            self.next += 1;
            let mut handle = self.handle(self.config.name.clone(), self.next);
            let name = CString::new(self.config.name.as_str()).map_err(|_| Status::Unavailable)?;
            let txt: Vec<u8> = [
                b"v=1".as_slice(),
                b"p=PCLAN004",
                if self.config.mode == super::Mode::Push {
                    b"m=push"
                } else {
                    b"m=pull"
                },
            ]
            .into_iter()
            .flat_map(|v| std::iter::once(v.len() as u8).chain(v.iter().copied()))
            .collect();
            check(unsafe {
                (self.api.register)(
                    &mut handle.raw,
                    0,
                    self.config.interface_index,
                    name.as_ptr(),
                    c"_papercut-xfer._tcp".as_ptr(),
                    c"local.".as_ptr(),
                    ptr::null(),
                    self.config.port.to_be(),
                    txt.len() as u16,
                    txt.as_ptr().cast(),
                    registered,
                    ptr::from_mut(handle.context.as_mut()).cast(),
                )
            })?;
            self.publication = Some(handle);
        }
        Ok(())
    }
    fn poll(&mut self) -> Result<Vec<Event>, Status> {
        let refs = self.browser.iter().chain(self.publication.iter()).chain(
            self.peers
                .values()
                .flat_map(|p| std::iter::once(&p.resolve).chain(p.address.iter())),
        );
        for handle in refs {
            let fd = unsafe { (self.api.fd)(handle.raw) };
            if fd < 0 {
                return Err(Status::Unavailable);
            }
            let mut ready = libc::pollfd {
                fd,
                events: libc::POLLIN,
                revents: 0,
            };
            let count = unsafe { libc::poll(&mut ready, 1, 0) };
            if count < 0 || ready.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
                return Err(Status::Unavailable);
            }
            if ready.revents & libc::POLLIN != 0 {
                check(unsafe { (self.api.process)(handle.raw) })?;
            }
        }
        self.signals()
    }
}
fn classify(code: i32) -> Status {
    if matches!(code, -65555 | -65570 | -65571) {
        Status::Denied
    } else {
        Status::Unavailable
    }
}
fn check(code: i32) -> Result<(), Status> {
    if code == 0 {
        Ok(())
    } else {
        Err(classify(code))
    }
}
// The system API supplies NUL-terminated strings; cap the copy before allocating untrusted data.
unsafe fn text(value: *const c_char, limit: usize) -> Option<String> {
    if value.is_null() {
        return None;
    }
    let length = (0..=limit).find(|n| unsafe { *value.add(*n) } == 0)?;
    std::str::from_utf8(unsafe { std::slice::from_raw_parts(value.cast::<u8>(), length) })
        .ok()
        .map(str::to_owned)
}
unsafe extern "C" fn browsed(
    _: Ref,
    flags: u32,
    index: u32,
    error: i32,
    name: *const c_char,
    kind: *const c_char,
    domain: *const c_char,
    ctx: *mut c_void,
) {
    let ctx = unsafe { &*ctx.cast::<Context>() };
    if ctx.error(error) || index != ctx.index {
        return;
    }
    if unsafe { text(kind, 64) }
        .as_deref()
        .map(|v| v.trim_end_matches('.'))
        != Some(SERVICE)
        || unsafe { text(domain, 64) }.as_deref() != Some("local.")
    {
        return;
    }
    if let Some(name) = unsafe { text(name, 63) }.filter(|n| super::label(n)) {
        ctx.push(Signal::Browse {
            name,
            add: flags & 2 != 0,
        });
    }
}
unsafe extern "C" fn resolved(
    _: Ref,
    _: u32,
    index: u32,
    error: i32,
    _: *const c_char,
    host: *const c_char,
    port: u16,
    len: u16,
    txt: *const u8,
    ctx: *mut c_void,
) {
    let ctx = unsafe { &*ctx.cast::<Context>() };
    if ctx.error(error) || index != ctx.index {
        return;
    }
    let host = unsafe { text(host, 253) }.filter(|h| h.to_ascii_lowercase().ends_with(".local."));
    let fields = if len <= 512 && (len == 0 || !txt.is_null()) {
        parse_txt(if len == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(txt, len as usize) }
        })
    } else {
        None
    };
    if let (Some(host), Some(txt)) = (host, fields) {
        ctx.push(Signal::Resolved {
            name: ctx.name.clone(),
            token: ctx.token,
            host,
            port: u16::from_be(port),
            txt,
        });
    } else {
        ctx.push(Signal::Invalid {
            name: ctx.name.clone(),
            token: ctx.token,
        });
    }
}
fn parse_txt(mut bytes: &[u8]) -> Option<Vec<Vec<u8>>> {
    let mut fields = Vec::new();
    while let Some((&length, rest)) = bytes.split_first() {
        if fields.len() == 3 || length as usize > rest.len() {
            return None;
        }
        fields.push(rest[..length as usize].to_vec());
        bytes = &rest[length as usize..];
    }
    Some(fields)
}
unsafe extern "C" fn addressed(
    _: Ref,
    flags: u32,
    index: u32,
    error: i32,
    _: *const c_char,
    address: *const libc::sockaddr,
    ttl: u32,
    ctx: *mut c_void,
) {
    let ctx = unsafe { &*ctx.cast::<Context>() };
    if ctx.error(error) || index != ctx.index || address.is_null() {
        return;
    }
    let ip = match unsafe { (*address).sa_family } as i32 {
        libc::AF_INET => IpAddr::V4(Ipv4Addr::from(
            unsafe { (*address.cast::<libc::sockaddr_in>()).sin_addr.s_addr }.to_ne_bytes(),
        )),
        libc::AF_INET6 => IpAddr::V6(Ipv6Addr::from(unsafe {
            (*address.cast::<libc::sockaddr_in6>()).sin6_addr.s6_addr
        })),
        _ => return,
    };
    ctx.push(Signal::Address {
        name: ctx.name.clone(),
        token: ctx.token,
        ip,
        add: flags & 2 != 0 && ttl != 0,
    });
}
unsafe extern "C" fn registered(
    _: Ref,
    _: u32,
    error: i32,
    name: *const c_char,
    _: *const c_char,
    _: *const c_char,
    ctx: *mut c_void,
) {
    let ctx = unsafe { &*ctx.cast::<Context>() };
    if ctx.error(error) {
        return;
    }
    if let Some(name) = unsafe { text(name, 63) }.filter(|n| super::label(n)) {
        ctx.push(Signal::Ready {
            token: ctx.token,
            name,
        });
    } else {
        ctx.fail(Status::Unavailable);
    }
}

#[cfg(test)]
mod tests;
