//! Cancellation owns the socket; restore admission is atomic with cancellation.
use std::io;
use std::net::{Shutdown, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use super::endpoints::{still_available, TransferInterface};

#[derive(Default)]
pub(super) struct TransferControl {
    pub cancelled: AtomicBool,
    inner: Mutex<ControlState>,
}

#[derive(Default)]
struct ControlState {
    socket: Option<TcpStream>,
    importing: bool,
    reason: Option<&'static str>,
}

impl TransferControl {
    pub fn cancel(&self, reason: &'static str) -> bool {
        let Ok(mut inner) = self.inner.lock() else {
            return false;
        };
        if inner.importing {
            return false;
        }
        if !self.cancelled.swap(true, Ordering::AcqRel) {
            inner.reason = Some(reason);
        }
        if let Some(socket) = inner.socket.take() {
            let _ = socket.shutdown(Shutdown::Both);
        }
        true
    }

    pub fn reason(&self) -> &'static str {
        self.inner
            .lock()
            .ok()
            .and_then(|s| s.reason)
            .unwrap_or("cancelled")
    }

    pub fn check(&self) -> io::Result<()> {
        if self.cancelled.load(Ordering::Acquire) {
            Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Library transfer cancelled",
            ))
        } else {
            Ok(())
        }
    }

    pub fn track(&self, socket: &TcpStream) -> io::Result<()> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| io::Error::other("Transfer state unavailable"))?;
        self.check()?;
        inner.socket = Some(socket.try_clone()?);
        Ok(())
    }

    pub fn clear_socket(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.socket = None;
        }
    }

    pub fn begin_import(&self) -> io::Result<()> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| io::Error::other("Transfer state unavailable"))?;
        self.check()?;
        inner.importing = true;
        inner.socket = None;
        Ok(())
    }
}

pub(super) struct NetworkWatch {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl NetworkWatch {
    // shortcut: identical-address network changes need OS identity notifications with the discovery adapters.
    pub fn start(control: Arc<TransferControl>, network: TransferInterface) -> Self {
        Self::observe(control, move || still_available(&network))
    }

    fn observe(
        control: Arc<TransferControl>,
        mut available: impl FnMut() -> bool + Send + 'static,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let done = stop.clone();
        let thread = thread::spawn(move || {
            while !done.load(Ordering::Acquire) && control.check().is_ok() {
                if !available() {
                    control.cancel("network-changed");
                    break;
                }
                thread::sleep(Duration::from_millis(250));
            }
        });
        Self {
            stop,
            thread: Some(thread),
        }
    }
}

impl Drop for NetworkWatch {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::net::TcpListener;
    use std::sync::Barrier;
    use std::time::Instant;

    #[test]
    fn cancellation_interrupts_blocked_io_and_cannot_cross_restore_admission() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (_server, _) = listener.accept().unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(30)))
            .unwrap();
        let control = Arc::new(TransferControl::default());
        control.track(&client).unwrap();
        let barrier = Arc::new(Barrier::new(2));
        let started = barrier.clone();
        let reader = thread::spawn(move || {
            started.wait();
            client.read(&mut [0])
        });
        barrier.wait();
        let now = Instant::now();
        assert!(control.cancel("cancelled"));
        assert!(matches!(reader.join().unwrap(), Ok(0) | Err(_)));
        assert!(now.elapsed() < Duration::from_secs(2));
        assert!(control.begin_import().is_err());
        let restoring = TransferControl::default();
        restoring.begin_import().unwrap();
        assert!(!restoring.cancel("backgrounded"));
        assert!(restoring.check().is_ok());
    }

    #[test]
    fn losing_the_selected_network_cancels_only_its_owned_session() {
        let old = Arc::new(TransferControl::default());
        let next = TransferControl::default();
        let watcher = NetworkWatch::observe(old.clone(), || false);
        let deadline = Instant::now() + Duration::from_secs(2);
        while old.check().is_ok() && Instant::now() < deadline {
            thread::yield_now();
        }
        drop(watcher);
        assert!(old.check().is_err());
        assert_eq!(old.reason(), "network-changed");
        assert!(next.check().is_ok());
    }
}
