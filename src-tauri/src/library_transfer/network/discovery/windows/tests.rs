use super::*;
static TEST: Mutex<()> = Mutex::new(());
static CAPTURE: Mutex<Vec<(String, usize, u32)>> = Mutex::new(Vec::new());
static INSTANCES: AtomicUsize = AtomicUsize::new(0);
static RECORDS: AtomicUsize = AtomicUsize::new(0);
unsafe extern "system" fn browse(
    request: *const DNS_SERVICE_BROWSE_REQUEST,
    cancel: *mut DNS_SERVICE_CANCEL,
) -> i32 {
    let request = unsafe { &*request };
    assert_eq!(request.Version, 1);
    assert_eq!(request.InterfaceIndex, 42);
    assert_eq!(
        unsafe { text(request.QueryName, 253) }.unwrap(),
        "_papercut-xfer._tcp.local"
    );
    unsafe {
        (*cancel).reserved = request.pQueryContext;
    }
    CAPTURE.lock().unwrap().push((
        "browse".into(),
        request.pQueryContext as usize,
        request.InterfaceIndex,
    ));
    DNS_REQUEST_PENDING
}
unsafe extern "system" fn resolve(
    request: *const DNS_SERVICE_RESOLVE_REQUEST,
    cancel: *mut DNS_SERVICE_CANCEL,
) -> i32 {
    let request = unsafe { &*request };
    assert_eq!(request.InterfaceIndex, 42);
    unsafe {
        (*cancel).reserved = request.pQueryContext;
    }
    CAPTURE.lock().unwrap().push((
        "resolve".into(),
        request.pQueryContext as usize,
        request.InterfaceIndex,
    ));
    DNS_REQUEST_PENDING
}
unsafe extern "system" fn register(
    request: *const DNS_SERVICE_REGISTER_REQUEST,
    cancel: *mut DNS_SERVICE_CANCEL,
) -> u32 {
    let request = unsafe { &*request };
    assert_eq!(request.InterfaceIndex, 42);
    assert_eq!(request.unicastEnabled, 0);
    assert_eq!(unsafe { (*request.pServiceInstance).dwInterfaceIndex }, 42);
    unsafe {
        (*cancel).reserved = request.pQueryContext;
    }
    CAPTURE.lock().unwrap().push((
        "register".into(),
        request.pQueryContext as usize,
        request.InterfaceIndex,
    ));
    DNS_REQUEST_PENDING as u32
}
unsafe extern "system" fn deregister(
    request: *const DNS_SERVICE_REGISTER_REQUEST,
    cancel: *mut DNS_SERVICE_CANCEL,
) -> u32 {
    assert!(cancel.is_null());
    let request = unsafe { &*request };
    CAPTURE.lock().unwrap().push((
        "deregister".into(),
        request.pQueryContext as usize,
        request.InterfaceIndex,
    ));
    DNS_REQUEST_PENDING as u32
}
unsafe extern "system" fn cancel(_: *const DNS_SERVICE_CANCEL) -> i32 {
    0
}
unsafe extern "system" fn register_cancel(_: *const DNS_SERVICE_CANCEL) -> u32 {
    0
}
#[repr(C)]
struct Instance {
    value: DNS_SERVICE_INSTANCE,
    _name: Vec<u16>,
    _host: Vec<u16>,
    _keys: Vec<Vec<u16>>,
    _values: Vec<Vec<u16>>,
    _key_ptrs: Vec<*mut u16>,
    _value_ptrs: Vec<*mut u16>,
    _v4: Option<Box<u32>>,
    _v6: Option<Box<IP6_ADDRESS>>,
}
unsafe extern "system" fn construct(
    name: *const u16,
    host: *const u16,
    v4: *const u32,
    v6: *const IP6_ADDRESS,
    port: u16,
    _: u16,
    _: u16,
    count: u32,
    keys: *const *const u16,
    values: *const *const u16,
) -> *mut DNS_SERVICE_INSTANCE {
    let name = wide(&unsafe { text(name, 253) }.unwrap());
    let host = wide(&unsafe { text(host, 253) }.unwrap());
    let keys: Vec<_> = (0..count as usize)
        .map(|i| wide(&unsafe { text(*keys.add(i), 63) }.unwrap()))
        .collect();
    let values: Vec<_> = (0..count as usize)
        .map(|i| wide(&unsafe { text(*values.add(i), 512) }.unwrap()))
        .collect();
    let key_ptrs: Vec<_> = keys.iter().map(|v| v.as_ptr().cast_mut()).collect();
    let value_ptrs: Vec<_> = values.iter().map(|v| v.as_ptr().cast_mut()).collect();
    let v4 = (!v4.is_null()).then(|| Box::new(unsafe { *v4 }));
    let v6 = (!v6.is_null()).then(|| Box::new(unsafe { *v6 }));
    let value = DNS_SERVICE_INSTANCE {
        pszInstanceName: name.as_ptr().cast_mut(),
        pszHostName: host.as_ptr().cast_mut(),
        ip4Address: v4
            .as_deref()
            .map_or(ptr::null_mut(), |v| ptr::from_ref(v).cast_mut()),
        ip6Address: v6
            .as_deref()
            .map_or(ptr::null_mut(), |v| ptr::from_ref(v).cast_mut()),
        wPort: port,
        dwPropertyCount: count,
        keys: key_ptrs.as_ptr().cast_mut(),
        values: value_ptrs.as_ptr().cast_mut(),
        ..Default::default()
    };
    INSTANCES.fetch_add(1, Ordering::SeqCst);
    Box::into_raw(Box::new(Instance {
        value,
        _name: name,
        _host: host,
        _keys: keys,
        _values: values,
        _key_ptrs: key_ptrs,
        _value_ptrs: value_ptrs,
        _v4: v4,
        _v6: v6,
    }))
    .cast()
}
unsafe extern "system" fn free_instance(value: *mut DNS_SERVICE_INSTANCE) {
    drop(unsafe { Box::from_raw(value.cast::<Instance>()) });
    INSTANCES.fetch_sub(1, Ordering::SeqCst);
}
unsafe extern "system" fn free_records(mut value: *const c_void, kind: DNS_FREE_TYPE) {
    assert_eq!(kind, DnsFreeRecordList);
    while !value.is_null() {
        let row = unsafe { Box::from_raw(value.cast_mut().cast::<DNS_RECORDW>()) };
        value = row.pNext.cast();
        RECORDS.fetch_sub(1, Ordering::SeqCst);
    }
}
static API: Api = Api {
    browse,
    browse_cancel: cancel,
    resolve,
    resolve_cancel: cancel,
    register,
    register_cancel,
    deregister,
    construct,
    free_instance,
    free_records,
    _module: 0,
};
fn adapter() -> Windows {
    Windows::with_api(&super::super::tests::task().config, &API).unwrap()
}
fn result(name: &str, index: u32) -> *mut DNS_SERVICE_INSTANCE {
    let keys = [wide("v"), wide("p"), wide("m")];
    let values = [wide("1"), wide("PCLAN004"), wide("push")];
    let v4 = u32::from_ne_bytes([192, 168, 1, 3]);
    let value = unsafe {
        construct(
            wide(&format!("{name}._papercut-xfer._tcp.local")).as_ptr(),
            wide("peer.local").as_ptr(),
            &v4,
            ptr::null(),
            53318,
            0,
            0,
            3,
            keys.each_ref().map(|v| v.as_ptr()).as_ptr(),
            values.each_ref().map(|v| v.as_ptr()).as_ptr(),
        )
    };
    unsafe {
        (*value).dwInterfaceIndex = index;
    }
    value
}
fn found(adapter: &Windows, name: &str, ttl: u32) {
    let owner = wide("_papercut-xfer._tcp.local");
    let query = wide(&format!("{name}._papercut-xfer._tcp.local"));
    let row = Box::new(DNS_RECORDW {
        pName: owner.as_ptr().cast_mut(),
        wType: DNS_TYPE_PTR,
        dwTtl: ttl,
        Data: DNS_RECORDW_1 {
            PTR: DNS_PTR_DATAW {
                pNameHost: query.as_ptr().cast_mut(),
            },
        },
        ..Default::default()
    });
    RECORDS.fetch_add(1, Ordering::SeqCst);
    unsafe {
        browsed(
            0,
            adapter.browser.as_ref().unwrap().id as *const c_void,
            Box::into_raw(row),
        );
    }
}
fn complete_register(id: usize) {
    unsafe {
        registered(0, id as *const c_void, result("Papercut-test", 42));
    }
}
fn cleanup(adapter: Windows) {
    drop(adapter);
    let pending: Vec<_> = calls().values().cloned().collect();
    for call in pending {
        unsafe {
            match &call.request {
                Request::Browse(_) => {
                    browsed(ERROR_CANCELLED, call.id as *const c_void, ptr::null())
                }
                Request::Resolve(_) => {
                    resolved(ERROR_CANCELLED, call.id as *const c_void, ptr::null())
                }
                Request::Register(_) => {
                    if call.state.load(Ordering::Acquire) == 0 {
                        complete_register(call.id);
                    }
                    if call.state.load(Ordering::Acquire) == 2 {
                        complete_register(call.id);
                    }
                }
            }
        }
    }
    assert!(calls().is_empty());
    assert_eq!(INSTANCES.load(Ordering::SeqCst), 0);
    assert_eq!(RECORDS.load(Ordering::SeqCst), 0);
}

#[test]
fn windows_scopes_native_requests_validates_metadata_and_ignores_replaced_resolutions() {
    let _guard = TEST.lock().unwrap();
    CAPTURE.lock().unwrap().clear();
    let mut adapter = adapter();
    adapter.browse().unwrap();
    found(&adapter, "Papercut-peer", 120);
    adapter.poll().unwrap();
    let old = adapter.peers["papercut-peer"].call.id;
    found(&adapter, "Papercut-peer", 0);
    assert!(matches!(
        adapter.poll().unwrap().as_slice(),
        [Event::Lost { .. }]
    ));
    found(&adapter, "Papercut-peer", 120);
    adapter.poll().unwrap();
    let current = adapter.peers["papercut-peer"].call.id;
    unsafe {
        resolved(0, old as *const c_void, result("Papercut-peer", 42));
    }
    assert!(adapter
        .context
        .send
        .try_send(Signal::Error {
            id: old,
            state: Status::Denied
        })
        .is_ok());
    assert!(adapter.poll().unwrap().is_empty());
    unsafe {
        resolved(0, current as *const c_void, result("Papercut-peer", 42));
    }
    assert!(
        matches!(adapter.poll().unwrap().as_slice(),[Event::Peer { port:53318, addresses, txt, .. }] if addresses==&["192.168.1.3"] && super::super::metadata(txt,Mode::Push))
    );
    found(&adapter, "Papercut-peer", 120);
    adapter.poll().unwrap();
    let current = adapter.peers["papercut-peer"].call.id;
    unsafe {
        resolved(0, current as *const c_void, result("Papercut-peer", 999));
    }
    assert!(matches!(
        adapter.poll().unwrap().as_slice(),
        [Event::Lost { .. }]
    ));
    assert!(CAPTURE
        .lock()
        .unwrap()
        .iter()
        .all(|(_, _, index)| *index == 42));
    cleanup(adapter);
}

#[test]
fn windows_publication_retains_cancelled_requests_until_ack_and_restarts_only_after_deregistration()
{
    let _guard = TEST.lock().unwrap();
    CAPTURE.lock().unwrap().clear();
    let mut adapter = adapter();
    adapter.publish(true).unwrap();
    let old = adapter.publication.as_ref().unwrap().id;
    let decoded = unsafe {
        instance(
            adapter.publication.as_ref().unwrap().instance,
            adapter.publication.as_ref().unwrap(),
        )
    }
    .unwrap();
    assert_eq!(decoded.port, 53318);
    assert!(super::super::metadata(&decoded.txt, Mode::Push));
    adapter.publish(false).unwrap();
    adapter.publish(true).unwrap();
    assert!(calls().contains_key(&old));
    assert_eq!(CAPTURE.lock().unwrap().len(), 1);
    complete_register(old); // A registration may succeed while its cancellation is in flight.
    assert_eq!(CAPTURE.lock().unwrap().last().unwrap().0, "deregister");
    assert!(adapter.poll().unwrap().is_empty());
    assert_eq!(adapter.publication.as_ref().unwrap().id, old);
    complete_register(old);
    adapter.poll().unwrap();
    let current = adapter.publication.as_ref().unwrap().id;
    assert_ne!(old, current);
    assert!(adapter
        .context
        .send
        .try_send(Signal::Ready {
            id: old,
            name: "stale".into()
        })
        .is_ok());
    assert!(adapter.poll().unwrap().is_empty());
    complete_register(current);
    assert!(
        matches!(adapter.poll().unwrap().as_slice(),[Event::Ready { name }] if name=="Papercut-test")
    );
    cleanup(adapter);
}

#[test]
fn windows_closed_callbacks_keep_buffers_alive_but_cannot_publish_and_bounds_fail_closed() {
    let _guard = TEST.lock().unwrap();
    let mut adapter = adapter();
    adapter.browse().unwrap();
    let browser = adapter.browser.as_ref().unwrap().id;
    adapter.context.closed.store(true, Ordering::Release);
    adapter.browser.as_ref().unwrap().close();
    assert!(calls().contains_key(&browser));
    {
        let call = calls().get(&browser).cloned().unwrap();
        if let Request::Browse(r) = &call.request {
            assert_eq!(
                unsafe { text(r.QueryName, 253) }.unwrap(),
                "_papercut-xfer._tcp.local"
            );
        }
    }
    found(&adapter, "Papercut-peer", 120);
    assert!(adapter.poll().unwrap().is_empty());
    for _ in 0..=EVENTS / 2 {
        let _ = adapter
            .context
            .send
            .try_send(Signal::Lost("bounded".into()));
    }
    adapter.context.closed.store(false, Ordering::Release);
    adapter.context.push(Signal::Lost("overflow".into()));
    assert!(matches!(adapter.poll(), Err(Status::Limited)));
    cleanup(adapter);
}

#[test]
fn windows_pending_call_limit_and_cleanup_timeout_do_not_allocate_more_native_handles() {
    let _guard = TEST.lock().unwrap();
    let mut adapter = adapter();
    let mut pending = Vec::new();
    for i in 0..2 * (PEERS + 1) {
        pending.push(
            adapter
                .call(
                    format!("peer-{i}"),
                    format!("peer-{i}._papercut-xfer._tcp.local"),
                    false,
                )
                .unwrap(),
        );
    }
    assert!(matches!(
        adapter.call(
            "overflow".into(),
            "overflow._papercut-xfer._tcp.local".into(),
            false
        ),
        Err(Status::Limited)
    ));
    for call in &pending {
        call.close();
    }
    drop(pending);
    cleanup(adapter);
    adapter = super::Windows::with_api(&super::super::tests::task().config, &API).unwrap();
    adapter.publish(true).unwrap();
    adapter.publish(false).unwrap();
    adapter.wanted = true;
    adapter.cleanup_deadline = Some(Instant::now());
    assert!(matches!(adapter.poll(), Err(Status::Unavailable)));
    cleanup(adapter);
}

#[test]
fn windows_pull_publication_and_null_metadata_are_bounded_native_values() {
    let _guard = TEST.lock().unwrap();
    let mut adapter = adapter();
    adapter.config.mode = Mode::Pull;
    adapter.publish(true).unwrap();
    let call = adapter.publication.as_ref().unwrap();
    let value = unsafe { instance(call.instance, call) }.unwrap();
    assert!(super::super::metadata(&value.txt, Mode::Pull));
    adapter.browse().unwrap();
    found(&adapter, "Papercut-peer", 120);
    adapter.poll().unwrap();
    let id = adapter.peers["papercut-peer"].call.id;
    let malformed = result("Papercut-peer", 42);
    unsafe {
        *(*malformed).values.add(1) = ptr::null_mut();
        resolved(0, id as *const c_void, malformed);
    }
    assert!(matches!(
        adapter.poll().unwrap().as_slice(),
        [Event::Lost { .. }]
    ));
    cleanup(adapter);
}

#[test]
fn windows_permission_failures_surface_without_releasing_unacknowledged_buffers() {
    unsafe extern "system" fn denied_browse(
        _: *const DNS_SERVICE_BROWSE_REQUEST,
        _: *mut DNS_SERVICE_CANCEL,
    ) -> i32 {
        5
    }
    unsafe extern "system" fn denied_cancel(_: *const DNS_SERVICE_CANCEL) -> i32 {
        5
    }
    static START_DENIED: Api = Api {
        browse: denied_browse,
        ..API
    };
    static STOP_DENIED: Api = Api {
        browse_cancel: denied_cancel,
        ..API
    };
    let _guard = TEST.lock().unwrap();
    let mut adapter =
        Windows::with_api(&super::super::tests::task().config, &START_DENIED).unwrap();
    assert!(matches!(adapter.browse(), Err(Status::Denied)));
    assert!(calls().is_empty());
    drop(adapter);
    let mut adapter = Windows::with_api(&super::super::tests::task().config, &STOP_DENIED).unwrap();
    adapter.browse().unwrap();
    let id = adapter.browser.as_ref().unwrap().id;
    adapter.browser.as_ref().unwrap().close();
    assert!(matches!(adapter.poll(), Err(Status::Denied)));
    assert!(calls().contains_key(&id));
    cleanup(adapter);
}

#[test]
fn windows_cancel_completion_race_is_not_a_false_permission_failure() {
    unsafe extern "system" fn already_completed(_: *const DNS_SERVICE_CANCEL) -> i32 {
        87
    }
    static API_RACE: Api = Api {
        browse_cancel: already_completed,
        ..API
    };
    let _guard = TEST.lock().unwrap();
    let mut adapter = Windows::with_api(&super::super::tests::task().config, &API_RACE).unwrap();
    adapter.browse().unwrap();
    let id = adapter.browser.as_ref().unwrap().id;
    adapter.browser.as_ref().unwrap().close();
    assert!(adapter.poll().unwrap().is_empty());
    assert!(calls().contains_key(&id));
    cleanup(adapter);
    unsafe extern "system" fn inline_cancel(cancel: *const DNS_SERVICE_CANCEL) -> i32 {
        unsafe {
            browsed(ERROR_CANCELLED, (*cancel).reserved, ptr::null());
        }
        0
    }
    static INLINE: Api = Api {
        browse_cancel: inline_cancel,
        ..API
    };
    let mut inline = Windows::with_api(&super::super::tests::task().config, &INLINE).unwrap();
    inline.browse().unwrap();
    let id = inline.browser.as_ref().unwrap().id;
    inline.browser.as_ref().unwrap().close();
    assert!(!calls().contains_key(&id));
    assert!(inline.poll().unwrap().is_empty());
    cleanup(inline);
    let mut adapter = super::Windows::with_api(&super::super::tests::task().config, &API).unwrap();
    adapter.browse().unwrap();
    let id = adapter.browser.as_ref().unwrap().id;
    unsafe {
        browsed(ERROR_CANCELLED, id as *const c_void, ptr::null());
    }
    assert!(matches!(adapter.poll(), Err(Status::Unavailable)));
    cleanup(adapter);
}
