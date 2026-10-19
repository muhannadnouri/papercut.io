//! Cancellable, selected-interface TCP connection setup for the v4 channel.
use super::endpoints::{source_address, TransferInterface};
use super::session::TransferControl;
use crate::library_transfer::{LibraryTransferError, LibraryTransferProgress};
use serde::{Deserialize, Serialize};
use std::io;
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub(super) enum ReceiverMessage {
    Progress(LibraryTransferProgress),
    Complete,
    Failed(LibraryTransferError),
}

/// Initiate one nonblocking connect; polling does not restart TCP's SYN timer.
pub(super) fn connect_cancellable(
    address: SocketAddr,
    network: Option<&TransferInterface>,
    control: &TransferControl,
    deadline: Instant,
) -> io::Result<TcpStream> {
    use socket2::{Domain, Protocol, Socket, Type};
    control.check()?;
    let socket = Socket::new(
        Domain::for_address(address),
        Type::STREAM,
        Some(Protocol::TCP),
    )?;
    if let Some(network) = network {
        socket.bind(
            &source_address(network, address)
                .map_err(io::Error::other)?
                .into(),
        )?;
    }
    socket.set_nonblocking(true)?;
    if let Err(error) = socket.connect(&address.into()) {
        #[cfg(unix)]
        let pending = error.raw_os_error() == Some(libc::EINPROGRESS);
        // Winsock reports WSAEWOULDBLOCK for nonblocking connect.
        #[cfg(windows)]
        let pending = error.raw_os_error() == Some(10035);
        if !pending && error.kind() != io::ErrorKind::WouldBlock {
            return Err(error);
        }
    }
    let stream: TcpStream = socket.into();
    control.track(&stream)?;
    wait_for_connection(&stream, control, deadline)?;
    stream.set_nonblocking(false)?;
    Ok(stream)
}

fn wait_for_connection(
    stream: &TcpStream,
    control: &TransferControl,
    deadline: Instant,
) -> io::Result<()> {
    loop {
        control.check()?;
        if Instant::now() >= deadline {
            return Err(io::ErrorKind::TimedOut.into());
        }
        if let Some(error) = stream.take_error()? {
            return Err(error);
        }
        if stream.peer_addr().is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, TcpListener};
    use std::sync::Arc;
    use std::thread;
    #[test]
    fn pending_connect_has_one_deadline_and_cancels_without_waiting_for_it() {
        use socket2::{Domain, Protocol, Socket, Type};
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let connected = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        assert_eq!(
            wait_for_connection(&connected, &TransferControl::default(), Instant::now())
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
        let socket: TcpStream = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))
            .unwrap()
            .into();
        let control = TransferControl::default();
        assert_eq!(
            wait_for_connection(&socket, &control, Instant::now())
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
        let control = Arc::new(control);
        let worker_control = control.clone();
        let (started, ready) = std::sync::mpsc::channel();
        let worker = thread::spawn(move || {
            started.send(()).unwrap();
            wait_for_connection(
                &socket,
                &worker_control,
                Instant::now() + Duration::from_secs(30),
            )
        });
        ready.recv_timeout(Duration::from_secs(2)).unwrap();
        let now = Instant::now();
        control.cancel("cancelled");
        assert_eq!(
            worker.join().unwrap().unwrap_err().kind(),
            io::ErrorKind::Interrupted
        );
        assert!(now.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn loopback_refusal_retains_os_error_facts_without_address_or_credentials() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let endpoint = listener.local_addr().unwrap();
        drop(listener);
        let error = connect_cancellable(
            endpoint,
            None,
            &TransferControl::default(),
            Instant::now() + Duration::from_secs(1),
        )
        .unwrap_err();
        let diagnostics = crate::library_transfer::diagnostics::Diagnostics::default();
        let trace = diagnostics.begin("receive");
        trace.stage("tcp-connect");
        trace.endpoint(endpoint);
        trace.io_error(&error);
        let report = serde_json::to_string(&diagnostics.snapshot().unwrap()).unwrap();
        assert!(report.contains("ConnectionRefused") && report.contains("tcp-connect"));
        assert!(!report.contains("127.0.0.1"));
    }
}
