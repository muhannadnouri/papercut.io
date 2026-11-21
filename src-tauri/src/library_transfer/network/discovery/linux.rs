//! Avahi over a private system-bus connection. Never activate or launch a daemon.
use std::collections::BTreeMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};
use std::time::Duration;

use super::{Adapter, Config, Event, Status, EVENTS, PEERS, SERVICE};
use dbus::{
    arg::{AppendAll, ReadAll},
    blocking::{BlockingSender, Connection},
    message::MatchRule,
    Message, MessageType, Path,
};

const SERVER: &str = "org.freedesktop.Avahi.Server";
const BROWSER: &str = "org.freedesktop.Avahi.ServiceBrowser";
const RESOLVER: &str = "org.freedesktop.Avahi.ServiceResolver";
const GROUP: &str = "org.freedesktop.Avahi.EntryGroup";
type Service = (i32, i32, String, String, String, u32);
type Found = (
    i32,
    i32,
    String,
    String,
    String,
    String,
    i32,
    String,
    u16,
    Vec<Vec<u8>>,
    u32,
);
enum Signal {
    New(String, Service),
    Lost(String, Service),
    Found(String, Found),
    Invalid(String),
    State(String, i32),
    Failed(String),
}

pub(super) struct Avahi {
    bus: Connection,
    owner: String,
    config: Config,
    browser: Option<Path<'static>>,
    group: Option<Path<'static>>,
    resolvers: BTreeMap<(String, i32), Path<'static>>,
    events: mpsc::Receiver<Signal>,
    overflow: Arc<AtomicBool>,
}
impl Avahi {
    pub fn new(config: &Config) -> Result<Self, Status> {
        if config.interface_index == 0 {
            return Err(Status::Unsupported);
        }
        let bus = Connection::new_system().map_err(classify)?;
        Self::connect(bus, config)
    }
    fn connect(bus: Connection, config: &Config) -> Result<Self, Status> {
        // GetNameOwner does not activate Avahi; all subsequent calls use its unique owner.
        let (owner,): (String,) = call(
            &bus,
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "GetNameOwner",
            ("org.freedesktop.Avahi",),
        )?;
        // Two workers can overlap while cancellation finishes; their queues share the 128-event cap.
        let (send, events) = mpsc::sync_channel(EVENTS / 2);
        let overflow = Arc::new(AtomicBool::new(false));
        let exceeded = overflow.clone();
        let mut rule = MatchRule::new();
        rule.msg_type = Some(MessageType::Signal);
        rule.sender = Some(owner.clone().into());
        let index = config.interface_index;
        bus.add_match::<(), _>(rule, move |_, _, message| {
            let path = message.path().map(|p| p.to_string()).unwrap_or_default();
            let interface = message
                .interface()
                .map(|p| p.to_string())
                .unwrap_or_default();
            let member = message.member().map(|p| p.to_string()).unwrap_or_default();
            let signal = match (interface.as_str(), member.as_str()) {
                (BROWSER, "ItemNew") => message
                    .read_all::<Service>()
                    .ok()
                    .filter(|service| valid_service(service, index))
                    .map(|value| Signal::New(path, value)),
                (BROWSER, "ItemRemove") => message
                    .read_all::<Service>()
                    .ok()
                    .filter(|service| valid_service(service, index))
                    .map(|value| Signal::Lost(path, value)),
                (RESOLVER, "Found") => message.read_all::<Found>().ok().map(|found| {
                    if super::label(&found.2)
                        && found.3 == SERVICE
                        && found.4 == "local"
                        && found.5.len() <= 253
                        && found.7.len() <= 128
                        && found.9.len() <= 3
                        && found.9.iter().map(Vec::len).sum::<usize>() <= 512
                    {
                        Signal::Found(path, found)
                    } else {
                        Signal::Invalid(path)
                    }
                }),
                (GROUP, "StateChanged") => message
                    .read_all::<(i32, String)>()
                    .ok()
                    .map(|(state, _)| Signal::State(path, state)),
                (BROWSER | RESOLVER, "Failure") => Some(Signal::Failed(path)),
                _ => None,
            };
            if let Some(signal) = signal {
                if send.try_send(signal).is_err() {
                    exceeded.store(true, Ordering::Release);
                }
            }
            true
        })
        .map_err(classify)?;
        Ok(Self {
            bus,
            owner,
            config: config.clone(),
            browser: None,
            group: None,
            resolvers: BTreeMap::new(),
            events,
            overflow,
        })
    }
    fn call<A: AppendAll, R: ReadAll>(
        &self,
        path: &str,
        interface: &str,
        method: &str,
        args: A,
    ) -> Result<R, Status> {
        call(&self.bus, &self.owner, path, interface, method, args)
    }
    fn service(&self, service: &Service) -> bool {
        valid_service(service, self.config.interface_index)
    }
}
fn valid_service(service: &Service, index: u32) -> bool {
    service.0 == index as i32
        && matches!(service.1, 0 | 1)
        && super::label(&service.2)
        && service.3 == SERVICE
        && service.4 == "local"
}
fn call<A: AppendAll, R: ReadAll>(
    bus: &Connection,
    dest: &str,
    path: &str,
    interface: &str,
    method: &str,
    args: A,
) -> Result<R, Status> {
    let mut message =
        Message::new_method_call(dest, path, interface, method).map_err(|_| Status::Unavailable)?;
    message.set_auto_start(false);
    message.append_all(args);
    bus.send_with_reply_and_block(message, Duration::from_millis(500))
        .map_err(classify)?
        .read_all()
        .map_err(|_| Status::Unavailable)
}
fn classify(error: dbus::Error) -> Status {
    match error.name().unwrap_or_default() {
        "org.freedesktop.DBus.Error.AccessDenied"
        | "org.freedesktop.Avahi.AccessDenied"
        | "org.freedesktop.Avahi.NotPermitted" => Status::Denied,
        _ => Status::Unavailable,
    }
}
impl Adapter for Avahi {
    fn browse(&mut self) -> Result<(), Status> {
        let (path,): (Path<'static>,) = self.call(
            "/",
            SERVER,
            "ServiceBrowserNew",
            (
                self.config.interface_index as i32,
                -1i32,
                SERVICE,
                "local",
                0u32,
            ),
        )?;
        self.browser = Some(path);
        Ok(())
    }
    fn publish(&mut self, enabled: bool) -> Result<(), Status> {
        if let Some(path) = self.group.take() {
            self.call::<_, ()>(&path, GROUP, "Free", ())?;
        }
        if enabled {
            let (path,): (Path<'static>,) = self.call("/", SERVER, "EntryGroupNew", ())?;
            self.group = Some(path.clone());
            let txt = vec![
                b"v=1".to_vec(),
                b"p=PCLAN004".to_vec(),
                if self.config.mode == super::Mode::Push {
                    b"m=push".to_vec()
                } else {
                    b"m=pull".to_vec()
                },
            ];
            self.call::<_, ()>(
                &path,
                GROUP,
                "AddService",
                (
                    self.config.interface_index as i32,
                    -1i32,
                    0u32,
                    &self.config.name,
                    SERVICE,
                    "local",
                    "",
                    self.config.port,
                    txt,
                ),
            )?;
            self.call::<_, ()>(&path, GROUP, "Commit", ())?;
        }
        Ok(())
    }
    fn poll(&mut self) -> Result<Vec<Event>, Status> {
        for _ in 0..EVENTS {
            if !self.bus.process(Duration::ZERO).map_err(classify)? {
                break;
            }
        }
        if self.overflow.load(Ordering::Acquire) {
            return Err(Status::Limited);
        }
        let signals = self.events.try_iter().take(EVENTS).collect::<Vec<_>>();
        let mut events = Vec::new();
        for signal in signals {
            match signal {
                Signal::New(path, service)
                    if self.service(&service)
                        && self.browser.as_ref().is_some_and(|p| p.to_string() == path) =>
                {
                    let key = (service.2.clone(), service.1);
                    if self.resolvers.contains_key(&key) {
                        continue;
                    }
                    if self.resolvers.len() == PEERS {
                        return Err(Status::Limited);
                    }
                    let (path,): (Path<'static>,) = self.call(
                        "/",
                        SERVER,
                        "ServiceResolverNew",
                        (
                            service.0, service.1, &service.2, SERVICE, "local", service.1, 0u32,
                        ),
                    )?;
                    self.resolvers.insert(key, path);
                }
                Signal::Lost(path, service)
                    if self.service(&service)
                        && self.browser.as_ref().is_some_and(|p| p.to_string() == path) =>
                {
                    let key = (service.2.clone(), service.1);
                    if let Some(path) = self.resolvers.remove(&key) {
                        let _ = self.call::<_, ()>(&path, RESOLVER, "Free", ());
                    }
                    events.push(Event::Lost {
                        name: service.2,
                        source: service.1.to_string(),
                    });
                }
                Signal::Found(
                    path,
                    (index, protocol, name, kind, domain, _, _, address, port, txt, _),
                ) if index == self.config.interface_index as i32
                    && kind == SERVICE
                    && domain == "local"
                    && self
                        .resolvers
                        .get(&(name.clone(), protocol))
                        .is_some_and(|p| p.to_string() == path) =>
                {
                    if txt.len() <= 3
                        && txt.iter().map(Vec::len).sum::<usize>() <= 512
                        && address.len() <= 128
                    {
                        events.push(Event::Peer {
                            name,
                            source: protocol.to_string(),
                            addresses: vec![address],
                            port,
                            txt,
                        });
                    } else {
                        events.push(Event::Lost {
                            name,
                            source: protocol.to_string(),
                        });
                    }
                }
                Signal::State(path, 2)
                    if self.group.as_ref().is_some_and(|p| p.to_string() == path) =>
                {
                    events.push(Event::Ready {
                        name: self.config.name.clone(),
                    })
                }
                Signal::Invalid(path) => {
                    if let Some(((name, protocol), _)) = self
                        .resolvers
                        .iter()
                        .find(|(_, value)| value.to_string() == path)
                    {
                        events.push(Event::Lost {
                            name: name.clone(),
                            source: protocol.to_string(),
                        });
                    }
                }
                Signal::State(path, 3 | 4)
                    if self.group.as_ref().is_some_and(|p| p.to_string() == path) =>
                {
                    return Err(Status::Unavailable)
                }
                Signal::Failed(path)
                    if self.browser.as_ref().is_some_and(|p| p.to_string() == path)
                        || self.resolvers.values().any(|p| p.to_string() == path) =>
                {
                    return Err(Status::Unavailable)
                }
                _ => {}
            }
        }
        Ok(events)
    }
}
impl Drop for Avahi {
    fn drop(&mut self) {
        if let Some(path) = self.group.take() {
            let _ = self.call::<_, ()>(&path, GROUP, "Free", ());
        }
        // Closing this private connection disposes all client-owned browser/resolver objects together.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dbus::channel::{MatchingReceiver, Sender};
    use std::io::BufRead;
    use std::process::{Child, Command, Stdio};
    type ServiceRegistration = (
        i32,
        i32,
        u32,
        String,
        String,
        String,
        String,
        u16,
        Vec<Vec<u8>>,
    );
    struct Bus(Child);
    impl Drop for Bus {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    fn signal(
        conn: &Connection,
        request: &Message,
        path: &str,
        interface: &str,
        member: &str,
        args: impl AppendAll,
    ) {
        let mut signal = Message::signal(&path.into(), &interface.into(), &member.into());
        signal.set_destination(request.sender());
        signal.append_all(args);
        conn.send(signal).unwrap();
    }
    #[test]
    fn avahi_adapter_scopes_calls_never_activates_and_handles_resolve_publish_and_loss() {
        let mut process = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("dbus-daemon is required for the isolated adapter regression");
        let mut address = String::new();
        std::io::BufReader::new(process.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        let _bus = Bus(process);
        let server = Connection::new_address(address.trim()).unwrap();
        server
            .request_name("org.freedesktop.Avahi", false, false, false)
            .unwrap();
        let done = Arc::new(AtomicBool::new(false));
        let stop = done.clone();
        let freed = Arc::new(AtomicBool::new(false));
        let saw_free = freed.clone();
        server.start_receive(
            MatchRule::new_method_call(),
            Box::new(move |request, conn| {
                assert!(
                    !request.get_auto_start(),
                    "Discovery must not activate Avahi"
                );
                let member = request.member().unwrap().to_string();
                match member.as_str() {
                    "ServiceBrowserNew" => {
                        let (index, protocol, kind, domain, flags): (
                            i32,
                            i32,
                            String,
                            String,
                            u32,
                        ) = request.read_all().unwrap();
                        assert_eq!(
                            (index, protocol, kind.as_str(), domain.as_str(), flags),
                            (42, -1, SERVICE, "local", 0)
                        );
                        conn.send(
                            request
                                .method_return()
                                .append1(Path::new("/browser").unwrap()),
                        )
                        .unwrap();
                        signal(
                            conn,
                            &request,
                            "/browser",
                            BROWSER,
                            "ItemNew",
                            (42i32, 0i32, "Papercut-test", SERVICE, "local", 0u32),
                        );
                    }
                    "ServiceResolverNew" => {
                        let (index, protocol, name, kind, domain, aprotocol, flags): (
                            i32,
                            i32,
                            String,
                            String,
                            String,
                            i32,
                            u32,
                        ) = request.read_all().unwrap();
                        assert_eq!((index, protocol, aprotocol, flags), (42, 0, 0, 0));
                        assert_eq!(
                            (name.as_str(), kind.as_str(), domain.as_str()),
                            ("Papercut-test", SERVICE, "local")
                        );
                        conn.send(
                            request
                                .method_return()
                                .append1(Path::new("/resolver").unwrap()),
                        )
                        .unwrap();
                        signal(
                            conn,
                            &request,
                            "/resolver",
                            RESOLVER,
                            "Found",
                            (
                                42i32,
                                0i32,
                                "Papercut-test",
                                SERVICE,
                                "local",
                                "host.local",
                                0i32,
                                "192.168.1.3",
                                53318u16,
                                vec![b"v=1".to_vec(), b"p=PCLAN004".to_vec(), b"m=push".to_vec()],
                                0u32,
                            ),
                        );
                    }
                    "EntryGroupNew" => {
                        conn.send(
                            request
                                .method_return()
                                .append1(Path::new("/group").unwrap()),
                        )
                        .unwrap();
                    }
                    "AddService" => {
                        let (index, protocol, flags, name, kind, domain, host, port, txt): ServiceRegistration = request.read_all().unwrap();
                        assert_eq!((index, protocol, flags, port), (42, -1, 0, 53318));
                        assert_eq!(
                            (name.as_str(), kind.as_str(), domain.as_str(), host.as_str()),
                            ("Papercut-test", SERVICE, "local", "")
                        );
                        assert!(super::super::metadata(&txt, super::super::Mode::Push));
                        conn.send(request.method_return()).unwrap();
                    }
                    "Commit" => {
                        conn.send(request.method_return()).unwrap();
                        signal(conn, &request, "/group", GROUP, "StateChanged", (2i32, ""));
                        signal(
                            conn,
                            &request,
                            "/browser",
                            BROWSER,
                            "ItemRemove",
                            (42i32, 0i32, "Papercut-test", SERVICE, "local", 0u32),
                        );
                    }
                    "Free" => {
                        if request.path().as_deref() == Some("/group") {
                            saw_free.store(true, Ordering::Release);
                        }
                        conn.send(request.method_return()).unwrap();
                    }
                    _ => panic!("Unexpected fake Avahi method: {member}"),
                }
                true
            }),
        );
        let worker = std::thread::spawn(move || {
            while !stop.load(Ordering::Acquire) {
                server.process(Duration::from_millis(10)).unwrap();
            }
        });
        let config = super::super::tests::task().config;
        let mut adapter =
            Avahi::connect(Connection::new_address(address.trim()).unwrap(), &config).unwrap();
        adapter.browse().unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        let mut found = false;
        while !found && std::time::Instant::now() < deadline {
            found = adapter.poll().unwrap().iter().any(
                |event| matches!(event,Event::Peer{addresses,..} if addresses==&["192.168.1.3"]),
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(found);
        adapter.publish(true).unwrap();
        let mut ready = false;
        let mut lost = false;
        while !(ready && lost) && std::time::Instant::now() < deadline {
            for event in adapter.poll().unwrap() {
                match event {
                    Event::Ready { .. } => ready = true,
                    Event::Lost { .. } => lost = true,
                    _ => {}
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(ready && lost);
        drop(adapter);
        assert!(freed.load(Ordering::Acquire));
        done.store(true, Ordering::Release);
        worker.join().unwrap();
    }
}
