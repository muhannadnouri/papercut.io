use super::*;
use crate::library_transfer::diagnostics::Diagnostics;
use std::sync::atomic::AtomicUsize;

fn book() -> Book {
    Book {
        state: Status::Searching,
        name: None,
        records: BTreeMap::new(),
    }
}
fn peer(name: &str, source: &str, address: &str) -> Event {
    Event::Peer {
        name: name.into(),
        source: source.into(),
        addresses: vec![address.into()],
        port: 53318,
        txt: vec![b"v=1".to_vec(), b"p=PCLAN004".to_vec(), b"m=push".to_vec()],
    }
}
pub(super) fn task() -> Task {
    Task {
        network: endpoints::discovery_test_interface(),
        config: Config {
            id: 1,
            interface_name: "test".into(),
            interface_index: 42,
            addresses: vec!["192.168.1.2".into()],
            name: "Papercut-test".into(),
            port: 53318,
            mode: Mode::Push,
        },
        stop: AtomicBool::new(false),
        book: Mutex::new(book()),
        protocol: None,
        trace: Diagnostics::default().begin("discovery"),
    }
}

#[test]
fn discovery_hints_cannot_change_mode_protocol_or_endpoint_policy() {
    let network = endpoints::discovery_test_interface();
    let mut book = book();
    let now = Instant::now();
    for address in [
        "8.8.8.8",
        "224.0.0.1",
        "0.0.0.0",
        "192.168.1.255",
        "127.0.0.1",
        "::",
        "ff02::1",
        "2001:4860:4860::8888",
    ] {
        book.apply(
            peer("untrusted", "source", address),
            &network,
            Mode::Push,
            now,
        )
        .unwrap();
    }
    assert!(book.snapshot(1, now).peers.is_empty());
    for txt in [
        vec![],
        vec![b"v=1".to_vec(), b"p=PCLAN003".to_vec(), b"m=push".to_vec()],
        vec![b"v=1".to_vec(), b"p=PCLAN004".to_vec(), b"m=auto".to_vec()],
        vec![b"v=1".to_vec(), b"v=1".to_vec(), b"m=push".to_vec()],
        vec![vec![0; 513]],
    ] {
        let mut invalid = peer("invalid", "source", "192.168.1.3");
        if let Event::Peer { txt: field, .. } = &mut invalid {
            *field = txt;
        }
        book.apply(invalid, &network, Mode::Push, now).unwrap();
        assert!(book.snapshot(1, now).peers.is_empty());
    }
    book.apply(
        peer("pull", "source", "192.168.1.3"),
        &network,
        Mode::Pull,
        now,
    )
    .unwrap();
    assert!(book.snapshot(1, now).peers.is_empty());
    book.apply(
        peer("nearby", "v4", "192.168.1.3"),
        &network,
        Mode::Push,
        now,
    )
    .unwrap();
    book.apply(
        peer("nearby", "v6", "fe80::3%999"),
        &network,
        Mode::Push,
        now,
    )
    .unwrap();
    let snapshot = book.snapshot(1, now);
    assert_eq!(snapshot.peers.len(), 1);
    assert_eq!(
        snapshot.peers[0].addresses,
        vec!["192.168.1.3:53318", "[fe80::3%42]:53318"]
    );
}

#[test]
fn updates_and_losses_remove_only_their_source_and_retention_never_exceeds_sixty_seconds() {
    let network = endpoints::discovery_test_interface();
    let now = Instant::now();
    let mut book = book();
    book.apply(
        peer("session", "v4", "192.168.1.3"),
        &network,
        Mode::Push,
        now,
    )
    .unwrap();
    book.apply(peer("session", "v6", "fe80::3"), &network, Mode::Push, now)
        .unwrap();
    book.apply(
        Event::Lost {
            name: "session".into(),
            source: "v4".into(),
        },
        &network,
        Mode::Push,
        now,
    )
    .unwrap();
    assert_eq!(
        book.snapshot(1, now).peers[0].addresses,
        vec!["[fe80::3%42]:53318"]
    );
    book.apply(peer("session", "v6", "8.8.8.8"), &network, Mode::Push, now)
        .unwrap();
    assert!(book.snapshot(1, now).peers.is_empty());
    book.apply(
        peer("session", "v4", "192.168.1.3"),
        &network,
        Mode::Push,
        now,
    )
    .unwrap();
    assert_eq!(
        book.snapshot(1, now + RETAIN - Duration::from_nanos(1))
            .peers
            .len(),
        1
    );
    assert!(book.snapshot(1, now + RETAIN).peers.is_empty());
}

#[test]
fn peer_and_address_limits_fail_closed_without_publishing_oversized_lists() {
    let network = endpoints::discovery_test_interface();
    let now = Instant::now();
    let mut peers = book();
    for i in 0..PEERS {
        peers
            .apply(
                peer(&format!("session-{i}"), "v4", "192.168.1.3"),
                &network,
                Mode::Push,
                now,
            )
            .unwrap();
    }
    assert_eq!(
        peers.apply(
            peer("overflow", "v4", "192.168.1.3"),
            &network,
            Mode::Push,
            now
        ),
        Err(Status::Limited)
    );
    assert_eq!(peers.snapshot(1, now).peers.len(), PEERS);
    let mut addresses = book();
    for i in 0..ADDRESSES {
        addresses
            .apply(
                peer(
                    "session",
                    &format!("source-{i}"),
                    &format!("192.168.1.{}", i + 3),
                ),
                &network,
                Mode::Push,
                now,
            )
            .unwrap();
    }
    assert_eq!(
        addresses.apply(
            peer("session", "overflow", "192.168.1.99"),
            &network,
            Mode::Push,
            now
        ),
        Err(Status::Limited)
    );
    assert_eq!(
        addresses.snapshot(1, now).peers[0].addresses.len(),
        ADDRESSES
    );
    assert!(!label(&"x".repeat(64)));
    assert!(!label("bad\nname"));
    assert!(label("<b>plain text</b>"));
}

struct Fake {
    dropped: Arc<AtomicUsize>,
    overflow: bool,
    task: Arc<Task>,
}
impl Drop for Fake {
    fn drop(&mut self) {
        self.dropped.fetch_add(1, Ordering::SeqCst);
    }
}
impl Adapter for Fake {
    fn browse(&mut self) -> Result<(), Status> {
        Ok(())
    }
    fn publish(&mut self, _: bool) -> Result<(), Status> {
        Ok(())
    }
    fn poll(&mut self) -> Result<Vec<Event>, Status> {
        if self.overflow {
            Ok((0..=EVENTS)
                .map(|_| Event::Ready {
                    name: "test".into(),
                })
                .collect())
        } else {
            self.task.stop.store(true, Ordering::Release);
            Ok(vec![])
        }
    }
}
#[test]
fn browse_deadlines_overflow_and_cancellation_dispose_the_adapter_before_completion() {
    for (overflow, expired, expected) in [
        (false, true, Ok(Status::Finished)),
        (true, false, Err(Status::Limited)),
        (false, false, Ok(Status::Idle)),
    ] {
        let task = Arc::new(task());
        let dropped = Arc::new(AtomicUsize::new(0));
        let adapter = Fake {
            task: task.clone(),
            dropped: dropped.clone(),
            overflow,
        };
        let deadline = Instant::now() + if expired { Duration::ZERO } else { BROWSE };
        assert_eq!(run(adapter, &task, deadline), expected);
        assert_eq!(dropped.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn background_stops_the_owned_browser_without_starting_transfer_work() {
    let state = LibraryTransferState::default();
    let task = Arc::new(task());
    *state.discovery.lock().unwrap() = Some(Job {
        task: task.clone(),
        thread: None,
    });
    assert!(!state.active.load(Ordering::Acquire));
    state.set_backgrounded(true);
    assert!(state.discovery.lock().unwrap().is_none());
    assert!(task.stop.load(Ordering::Acquire));
    assert!(!state.active.load(Ordering::Acquire));
}

#[test]
fn stale_stop_commands_cannot_dispose_the_next_browser_generation() {
    use tauri::Manager;
    let state = LibraryTransferState::default();
    let mut next = task();
    next.config.id = 2;
    let next = Arc::new(next);
    *state.discovery.lock().unwrap() = Some(Job {
        task: next.clone(),
        thread: None,
    });
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    library_transfer_discovery_stop(app.state(), 1).unwrap();
    assert!(!next.stop.load(Ordering::Acquire));
    assert_eq!(
        library_transfer_discovery_status(app.state(), 2)
            .unwrap()
            .unwrap()
            .id,
        2
    );
    library_transfer_discovery_stop(app.state(), 2).unwrap();
    assert!(next.stop.load(Ordering::Acquire));
    assert!(library_transfer_discovery_status(app.state(), 2)
        .unwrap()
        .is_none());
}

#[test]
fn cancelling_a_blocked_browser_is_nonblocking_and_its_worker_remains_bounded() {
    let slot = WorkerSlot::acquire().unwrap();
    let second = WorkerSlot::acquire().unwrap();
    let (release, wait) = std::sync::mpsc::channel();
    let thread = thread::spawn(move || {
        let _slot = slot;
        let _ = wait.recv();
    });
    let task = Arc::new(task());
    let job = Job {
        task: task.clone(),
        thread: Some(thread),
    };
    let now = Instant::now();
    drop(job);
    assert!(now.elapsed() < Duration::from_secs(1));
    assert!(task.stop.load(Ordering::Acquire));
    assert!(WorkerSlot::acquire().is_err());
    release.send(()).unwrap();
    drop(second);
    let until = Instant::now() + Duration::from_secs(2);
    while WORKERS.load(Ordering::Acquire) != 0 && Instant::now() < until {
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(WORKERS.load(Ordering::Acquire), 0);
}

#[test]
fn optional_discovery_failure_preserves_manual_pairing_but_network_loss_cancels_it() {
    struct Failure {
        lost: bool,
    }
    impl Adapter for Failure {
        fn browse(&mut self) -> Result<(), Status> {
            Ok(())
        }
        fn publish(&mut self, _: bool) -> Result<(), Status> {
            if self.lost {
                Ok(())
            } else {
                Err(Status::Denied)
            }
        }
        fn poll(&mut self) -> Result<Vec<Event>, Status> {
            Ok(vec![Event::NetworkChanged])
        }
    }
    for lost in [false, true] {
        let mut task = task();
        let protocol = Arc::new(
            v4::Session::listener(9, Mode::Push, vec!["127.0.0.1:53318".parse().unwrap()]).unwrap(),
        );
        task.protocol = Some(protocol.clone());
        assert_eq!(
            run(Failure { lost }, &task, Instant::now() + BROWSE),
            Err(if lost {
                Status::Unavailable
            } else {
                Status::Denied
            })
        );
        assert_eq!(
            protocol.phase().unwrap(),
            if lost {
                Phase::Cancelled
            } else {
                Phase::Waiting
            }
        );
        if !lost {
            assert!(protocol.pairing_code().is_ok());
        }
    }
}

#[test]
fn successful_pairing_withdraws_publication_and_eligible_disconnect_republishes() {
    use std::net::{TcpListener, TcpStream};
    use std::sync::mpsc;
    struct Publisher {
        visible: Arc<AtomicBool>,
        transitions: Arc<Mutex<Vec<bool>>>,
    }
    impl Adapter for Publisher {
        fn browse(&mut self) -> Result<(), Status> {
            panic!("A listener must not browse")
        }
        fn publish(&mut self, enabled: bool) -> Result<(), Status> {
            self.visible.store(enabled, Ordering::Release);
            self.transitions.lock().unwrap().push(enabled);
            Ok(())
        }
        fn poll(&mut self) -> Result<Vec<Event>, Status> {
            Ok(vec![])
        }
    }
    impl Drop for Publisher {
        fn drop(&mut self) {
            let _ = self.publish(false);
        }
    }
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let l = Arc::new(v4::Session::listener(9, Mode::Push, vec![address]).unwrap());
    let code = l.pairing_code().unwrap();
    let c = v4::Session::connector(10, Mode::Push, address, &code).unwrap();
    let mut setup = task();
    setup.protocol = Some(l.clone());
    setup.network = endpoints::interfaces()
        .unwrap()
        .into_iter()
        .find(|i| i.addresses.iter().any(|ip| ip.is_loopback()))
        .unwrap();
    let task = Arc::new(setup);
    let worker_task = task.clone();
    let visible = Arc::new(AtomicBool::new(false));
    let transitions = Arc::new(Mutex::new(vec![]));
    let adapter = Publisher {
        visible: visible.clone(),
        transitions: transitions.clone(),
    };
    let worker = thread::spawn(move || run(adapter, &worker_task, Instant::now() + BROWSE));
    let wait = |expected: bool| {
        let until = Instant::now() + Duration::from_secs(2);
        while visible.load(Ordering::Acquire) != expected && Instant::now() < until {
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(visible.load(Ordering::Acquire), expected);
    };
    wait(true);
    let (release, finish) = mpsc::channel();
    let paired = l.clone();
    let handshake = thread::spawn(move || {
        let channel = paired.authenticate(listener.accept().unwrap().0).unwrap();
        let _ = finish.recv();
        drop(channel);
    });
    c.begin_connect().unwrap();
    let _connector = c
        .authenticate(TcpStream::connect(address).unwrap())
        .unwrap();
    wait(false);
    release.send(()).unwrap();
    handshake.join().unwrap();
    wait(true);
    assert_eq!(l.phase().unwrap(), Phase::WaitingResume);
    task.stop.store(true, Ordering::Release);
    assert_eq!(worker.join().unwrap(), Ok(Status::Idle));
    assert_eq!(*transitions.lock().unwrap(), vec![true, false, true, false]);
}
