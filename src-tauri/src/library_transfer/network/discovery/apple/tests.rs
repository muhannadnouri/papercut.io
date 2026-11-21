use super::*;
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::sync::{
    atomic::{AtomicI32, AtomicUsize},
    Mutex,
};

static TEST: Mutex<()> = Mutex::new(());
static FD: AtomicI32 = AtomicI32::new(-1);
static HANDLES: AtomicUsize = AtomicUsize::new(0);
static PUBLICATION: Mutex<Option<(u32, u16, Vec<u8>)>> = Mutex::new(None);
unsafe fn allocated(out: *mut Ref, index: u32) -> i32 {
    assert_eq!(index, 42);
    HANDLES.fetch_add(1, Ordering::SeqCst);
    unsafe {
        *out = Box::into_raw(Box::new(0u8)).cast();
    }
    0
}
unsafe extern "C" fn browse(
    out: *mut Ref,
    flags: u32,
    index: u32,
    kind: *const c_char,
    domain: *const c_char,
    _: BrowseCallback,
    _: *mut c_void,
) -> i32 {
    assert_eq!(flags, 0);
    assert_eq!(unsafe { text(kind, 64) }.unwrap(), SERVICE);
    assert_eq!(unsafe { text(domain, 64) }.unwrap(), "local.");
    unsafe { allocated(out, index) }
}
unsafe extern "C" fn resolve(
    out: *mut Ref,
    _: u32,
    index: u32,
    _: *const c_char,
    _: *const c_char,
    _: *const c_char,
    _: ResolveCallback,
    _: *mut c_void,
) -> i32 {
    unsafe { allocated(out, index) }
}
unsafe extern "C" fn address(
    out: *mut Ref,
    _: u32,
    index: u32,
    protocol: u32,
    host: *const c_char,
    _: AddressCallback,
    _: *mut c_void,
) -> i32 {
    assert_eq!(protocol, 3);
    assert!(unsafe { text(host, 253) }.unwrap().ends_with(".local."));
    unsafe { allocated(out, index) }
}
unsafe extern "C" fn register(
    out: *mut Ref,
    _: u32,
    index: u32,
    _: *const c_char,
    _: *const c_char,
    _: *const c_char,
    host: *const c_char,
    port: u16,
    len: u16,
    txt: *const c_void,
    _: RegisterCallback,
    _: *mut c_void,
) -> i32 {
    assert!(host.is_null());
    *PUBLICATION.lock().unwrap() = Some((
        index,
        u16::from_be(port),
        unsafe { std::slice::from_raw_parts(txt.cast(), len as usize) }.to_vec(),
    ));
    unsafe { allocated(out, index) }
}
unsafe extern "C" fn fd(_: Ref) -> i32 {
    FD.load(Ordering::SeqCst)
}
unsafe extern "C" fn process(_: Ref) -> i32 {
    0
}
unsafe extern "C" fn free(raw: Ref) {
    drop(unsafe { Box::from_raw(raw.cast::<u8>()) });
    HANDLES.fetch_sub(1, Ordering::SeqCst);
}
const API: Api = Api {
    browse,
    resolve,
    address,
    register,
    fd,
    process,
    free,
};
fn adapter() -> (Bonjour, (UnixStream, UnixStream)) {
    let pair = UnixStream::pair().unwrap();
    FD.store(pair.0.as_raw_fd(), Ordering::SeqCst);
    (
        Bonjour::with_api(&super::super::tests::task().config, API).unwrap(),
        pair,
    )
}
fn context(handle: &Handle) -> *mut c_void {
    ptr::from_ref(handle.context.as_ref()).cast_mut().cast()
}
fn found(adapter: &Bonjour, add: bool) {
    unsafe {
        browsed(
            ptr::null_mut(),
            if add { 2 } else { 0 },
            42,
            0,
            c"Papercut-peer".as_ptr(),
            c"_papercut-xfer._tcp".as_ptr(),
            c"local.".as_ptr(),
            context(adapter.browser.as_ref().unwrap()),
        );
    }
}
fn resolution(adapter: &Bonjour, host: &std::ffi::CStr) {
    let bytes = b"\x03v=1\x0ap=PCLAN004\x06m=push";
    unsafe {
        resolved(
            ptr::null_mut(),
            0,
            42,
            0,
            ptr::null(),
            host.as_ptr(),
            53318u16.to_be(),
            bytes.len() as u16,
            bytes.as_ptr(),
            context(&adapter.peers["Papercut-peer"].resolve),
        );
    }
}

#[test]
fn scoped_bonjour_resolves_updates_loses_and_rejects_replaced_callbacks() {
    let _guard = TEST.lock().unwrap();
    let (mut adapter, _sockets) = adapter();
    adapter.browse().unwrap();
    found(&adapter, true);
    adapter.poll().unwrap();
    resolution(&adapter, c"outside.example.");
    assert!(matches!(
        adapter.poll().unwrap().as_slice(),
        [Event::Lost { .. }]
    ));
    assert_eq!(HANDLES.load(Ordering::SeqCst), 2);
    resolution(&adapter, c"host.local.");
    adapter.poll().unwrap();
    let token = adapter.peers["Papercut-peer"]
        .address
        .as_ref()
        .unwrap()
        .context
        .token;
    let ipv4 = libc::sockaddr_in {
        sin_family: libc::AF_INET as _,
        sin_addr: libc::in_addr {
            s_addr: u32::from_ne_bytes([192, 168, 1, 3]),
        },
        ..unsafe { std::mem::zeroed() }
    };
    let ctx = context(adapter.peers["Papercut-peer"].address.as_ref().unwrap());
    unsafe {
        addressed(
            ptr::null_mut(),
            2,
            999,
            0,
            ptr::null(),
            ptr::from_ref(&ipv4).cast(),
            120,
            ctx,
        );
    }
    assert!(adapter.poll().unwrap().is_empty());
    unsafe {
        addressed(
            ptr::null_mut(),
            2,
            42,
            0,
            ptr::null(),
            ptr::from_ref(&ipv4).cast(),
            120,
            ctx,
        );
    }
    assert!(
        matches!(adapter.poll().unwrap().as_slice(), [Event::Peer { port:53318, addresses, txt, .. }] if addresses == &["192.168.1.3"] && super::super::metadata(txt, super::super::Mode::Push))
    );
    resolution(&adapter, c"replacement.local.");
    adapter.poll().unwrap();
    assert!(adapter
        .send
        .try_send(Signal::Address {
            name: "Papercut-peer".into(),
            token,
            ip: "192.168.1.99".parse().unwrap(),
            add: true
        })
        .is_ok());
    assert!(adapter.poll().unwrap().is_empty());
    found(&adapter, false);
    assert!(matches!(
        adapter.poll().unwrap().as_slice(),
        [Event::Lost { .. }]
    ));
    assert_eq!(HANDLES.load(Ordering::SeqCst), 1);
    found(&adapter, true);
    adapter.poll().unwrap();
    assert!(adapter
        .send
        .try_send(Signal::Resolved {
            name: "Papercut-peer".into(),
            token: 1,
            host: "stale.local.".into(),
            port: 1,
            txt: vec![]
        })
        .is_ok());
    assert!(adapter.poll().unwrap().is_empty());
    assert!(adapter
        .send
        .try_send(Signal::Error {
            name: "Papercut-peer".into(),
            token: 1,
            state: Status::Denied
        })
        .is_ok());
    assert!(adapter.poll().unwrap().is_empty());
    drop(adapter);
    assert_eq!(HANDLES.load(Ordering::SeqCst), 0);
}

#[test]
fn publication_is_scoped_confirmed_and_disposed_before_restart() {
    let _guard = TEST.lock().unwrap();
    let (mut adapter, _sockets) = adapter();
    adapter.publish(true).unwrap();
    let (index, port, txt) = PUBLICATION.lock().unwrap().take().unwrap();
    assert_eq!((index, port), (42, 53318));
    assert!(super::super::metadata(
        &parse_txt(&txt).unwrap(),
        super::super::Mode::Push
    ));
    let old_token = adapter.publication.as_ref().unwrap().context.token;
    adapter.publish(false).unwrap();
    assert_eq!(HANDLES.load(Ordering::SeqCst), 0);
    adapter.publish(true).unwrap();
    assert!(adapter
        .send
        .try_send(Signal::Ready {
            token: old_token,
            name: "old".into()
        })
        .is_ok());
    assert!(adapter.poll().unwrap().is_empty());
    unsafe {
        registered(
            ptr::null_mut(),
            2,
            0,
            c"Papercut-renamed".as_ptr(),
            ptr::null(),
            ptr::null(),
            context(adapter.publication.as_ref().unwrap()),
        );
    }
    assert!(
        matches!(adapter.poll().unwrap().as_slice(), [Event::Ready { name }] if name == "Papercut-renamed")
    );
    drop(adapter);
    assert_eq!(HANDLES.load(Ordering::SeqCst), 0);
}

#[test]
fn malformed_txt_permission_and_queue_bounds_fail_closed() {
    let _guard = TEST.lock().unwrap();
    let (mut adapter, _sockets) = adapter();
    adapter.browse().unwrap();
    assert!(parse_txt(b"\x08short").is_none());
    assert!(parse_txt(b"\x00\x00\x00\x00").is_none());
    assert_eq!(classify(-65570), Status::Denied);
    assert_eq!(classify(-65571), Status::Denied);
    let ctx = adapter.browser.as_ref().unwrap().context.as_ref();
    for _ in 0..=EVENTS / 2 {
        ctx.push(Signal::Browse {
            name: "bounded".into(),
            add: true,
        });
    }
    assert!(matches!(adapter.poll(), Err(Status::Limited)));
    drop(adapter);
    assert_eq!(HANDLES.load(Ordering::SeqCst), 0);
}

#[test]
fn pull_publication_and_ipv6_address_loss_use_the_same_scoped_contract() {
    let _guard = TEST.lock().unwrap();
    let (mut adapter, _sockets) = adapter();
    adapter.config.mode = super::super::Mode::Pull;
    adapter.publish(true).unwrap();
    let (_, _, txt) = PUBLICATION.lock().unwrap().take().unwrap();
    assert!(super::super::metadata(
        &parse_txt(&txt).unwrap(),
        super::super::Mode::Pull
    ));
    adapter.browse().unwrap();
    found(&adapter, true);
    adapter.poll().unwrap();
    resolution(&adapter, c"host.local.");
    adapter.poll().unwrap();
    let address = libc::sockaddr_in6 {
        sin6_family: libc::AF_INET6 as _,
        sin6_addr: libc::in6_addr {
            s6_addr: "fe80::3".parse::<Ipv6Addr>().unwrap().octets(),
        },
        ..unsafe { std::mem::zeroed() }
    };
    let ctx = context(adapter.peers["Papercut-peer"].address.as_ref().unwrap());
    unsafe {
        addressed(
            ptr::null_mut(),
            2,
            42,
            0,
            ptr::null(),
            ptr::from_ref(&address).cast(),
            120,
            ctx,
        );
    }
    assert!(
        matches!(adapter.poll().unwrap().as_slice(), [Event::Peer { addresses,.. }] if addresses == &["fe80::3"])
    );
    unsafe {
        addressed(
            ptr::null_mut(),
            2,
            42,
            0,
            ptr::null(),
            ptr::from_ref(&address).cast(),
            0,
            ctx,
        );
    }
    assert!(matches!(
        adapter.poll().unwrap().as_slice(),
        [Event::Lost { .. }]
    ));
    drop(adapter);
    assert_eq!(HANDLES.load(Ordering::SeqCst), 0);
}

#[test]
fn bonjour_peer_limit_and_permission_failure_dispose_only_owned_references() {
    let _guard = TEST.lock().unwrap();
    let (mut adapter, _sockets) = adapter();
    adapter.browse().unwrap();
    for i in 0..PEERS {
        adapter
            .browser
            .as_ref()
            .unwrap()
            .context
            .push(Signal::Browse {
                name: format!("peer-{i}"),
                add: true,
            });
    }
    adapter.poll().unwrap();
    assert_eq!(HANDLES.load(Ordering::SeqCst), PEERS + 1);
    adapter
        .browser
        .as_ref()
        .unwrap()
        .context
        .push(Signal::Browse {
            name: "overflow".into(),
            add: true,
        });
    assert!(matches!(adapter.poll(), Err(Status::Limited)));
    drop(adapter);
    assert_eq!(HANDLES.load(Ordering::SeqCst), 0);
    unsafe extern "C" fn denied(
        _: *mut Ref,
        _: u32,
        _: u32,
        _: *const c_char,
        _: *const c_char,
        _: BrowseCallback,
        _: *mut c_void,
    ) -> i32 {
        -65570
    }
    let api = Api {
        browse: denied,
        ..API
    };
    let mut adapter = Bonjour::with_api(&super::super::tests::task().config, api).unwrap();
    assert_eq!(adapter.browse(), Err(Status::Denied));
    drop(adapter);
    assert_eq!(HANDLES.load(Ordering::SeqCst), 0);
}
