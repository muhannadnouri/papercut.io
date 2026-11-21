//! Native v4 stream boundary, independent of which content role connects.
//! Production orchestration supplies native staging and device-wide reservations.

mod auth;
mod codec;
#[cfg(test)]
mod tests;

use std::io::{self, Read, Seek, SeekFrom, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::security::{new_pairing_code, normalize_code, server_tls_config};
use super::transport::ReceiverMessage;
use crate::library_transfer::{LibraryTransferError, LibraryTransferProgress};
use auth::{Credentials, SecureStream};
pub(super) use codec::Offer;
use codec::{Expected, Frame};

const IO_TIMEOUT: Duration = Duration::from_secs(30);
const WINDOW: Duration = Duration::from_secs(600);
const AUTH_TIMEOUT: Duration = Duration::from_secs(5);
const BUFFER: usize = 64 * 1024;
pub(super) fn protocol_version() -> &'static str {
    std::str::from_utf8(auth::MAGIC).expect("fixed ASCII protocol marker")
}

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub(super) enum Error {
    Io(io::Error),
    Protocol(&'static str),
    Incompatible,
    WrongProof,
    Expired,
    Cancelled,
    ChangedSession,
    ChangedPackage,
    DigestMismatch,
    Remote(LibraryTransferError),
    OutcomeUnknown,
}
impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[repr(u8)]
pub(super) enum Mode {
    Push = 1,
    Pull = 2,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Side {
    Connector,
    Listener,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum Content {
    Sender,
    Receiver,
}

impl Mode {
    pub fn side(self, content: Content) -> Side {
        if self.content(Side::Connector) == content {
            Side::Connector
        } else {
            Side::Listener
        }
    }

    fn content(self, side: Side) -> Content {
        if matches!(
            (self, side),
            (Self::Push, Side::Connector) | (Self::Pull, Side::Listener)
        ) {
            Content::Sender
        } else {
            Content::Receiver
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Phase {
    Selecting,
    Waiting,
    Connecting,
    Authenticating,
    Offer,
    Ready,
    Streaming,
    Verifying,
    FinishingImport,
    AwaitingResult,
    WaitingResume,
    Complete,
    Failed,
    Cancelled,
    Expired,
    OutcomeUnknown,
}
impl Phase {
    pub fn terminal(self) -> bool {
        matches!(
            self,
            Self::Complete | Self::Failed | Self::Cancelled | Self::Expired | Self::OutcomeUnknown
        )
    }
}

// No Debug/Serialize: neither session credentials nor resume identity belongs in diagnostics.
pub(super) struct Session {
    generation: u64,
    mode: Mode,
    side: Side,
    endpoints: Vec<SocketAddr>,
    inner: Mutex<Inner>,
}

struct Inner {
    phase: Phase,
    deadline: Option<Instant>,
    code: Option<String>,
    listener: Option<Credentials>,
    pinned_id: Option<[u8; 16]>,
    pinned_endpoint: Option<SocketAddr>,
    offer: Option<Offer>,
    offset: Option<u64>,
    copied: u64,
    total: u64,
    verified: bool,
    socket: Option<TcpStream>,
    network_closed: bool,
    active: bool,
}

impl Inner {
    fn close_network(&mut self) {
        if let Some(socket) = self.socket.take() {
            let _ = socket.shutdown(Shutdown::Both);
        }
    }
    fn end(&mut self, phase: Phase) {
        if self.phase.terminal() {
            return;
        }
        self.close_network();
        self.phase = phase;
        self.deadline = None;
        self.code = None;
        self.listener = None;
        self.pinned_id = None;
        self.pinned_endpoint = None;
        self.offer = None;
        self.offset = None;
        self.verified = false;
        self.network_closed = true;
    }
    fn eligible(&mut self, now: Instant) -> Result<Instant> {
        if self.network_closed || self.phase.terminal() {
            return Err(Error::Cancelled);
        }
        let deadline = self
            .deadline
            .ok_or(Error::Protocol("Connect budget not started"))?;
        if deadline <= now {
            self.end(Phase::Expired);
            return Err(Error::Expired);
        }
        Ok(deadline)
    }

    fn disconnected(&mut self) {
        self.close_network();
        if self.phase.terminal() || self.phase == Phase::FinishingImport {
            return;
        }
        if self.phase == Phase::AwaitingResult {
            self.end(Phase::OutcomeUnknown);
        } else if self.phase == Phase::Verifying {
            self.end(Phase::Failed);
        } else if self.eligible(Instant::now()).is_ok() {
            self.phase = Phase::WaitingResume;
        }
    }
}

impl Session {
    #[cfg(test)]
    pub fn expire_for_test(&self) {
        self.lock().unwrap().deadline = Some(Instant::now());
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn bytes(&self) -> Result<(u64, u64)> {
        let inner = self.lock()?;
        Ok((inner.copied, inner.total))
    }
    pub fn deadline(&self) -> Result<Option<Instant>> {
        Ok(self.lock()?.deadline)
    }
    pub fn is_listener(&self) -> bool {
        self.side == Side::Listener
    }
    pub fn admit_import(&self) -> Result<()> {
        let mut inner = self.lock()?;
        if inner.network_closed {
            return Err(Error::Cancelled);
        }
        if self.mode.content(self.side) != Content::Receiver
            || inner.phase != Phase::Verifying
            || !inner.verified
        {
            return Err(Error::Protocol("Unverified archive cannot be imported"));
        }
        inner.phase = Phase::FinishingImport;
        Ok(())
    }

    pub fn listener(generation: u64, mode: Mode, endpoints: Vec<SocketAddr>) -> Result<Self> {
        if endpoints.is_empty()
            || endpoints
                .iter()
                .any(|endpoint| endpoint.ip().is_unspecified() || endpoint.port() == 0)
        {
            return Err(Error::Protocol("Missing bound listener endpoints"));
        }
        let mut id = [0; 16];
        getrandom::fill(&mut id).map_err(|_| Error::Protocol("Session randomness unavailable"))?;
        let code =
            new_pairing_code().map_err(|_| Error::Protocol("Pairing randomness unavailable"))?;
        let tls = server_tls_config().map_err(|_| Error::Protocol("TLS configuration failed"))?;
        let session = Self::new(generation, mode, Side::Listener, endpoints, None);
        {
            let mut inner = session.lock()?;
            inner.listener = Some(Credentials { code, id, tls });
            inner.deadline = Some(Instant::now() + WINDOW);
        }
        Ok(session)
    }

    pub fn connector(
        generation: u64,
        mode: Mode,
        endpoint: SocketAddr,
        code: &str,
    ) -> Result<Self> {
        let code = normalize_code(code).map_err(|_| Error::Protocol("Invalid pairing code"))?;
        Ok(Self::new(
            generation,
            mode,
            Side::Connector,
            vec![endpoint],
            Some(code),
        ))
    }

    fn new(
        generation: u64,
        mode: Mode,
        side: Side,
        endpoints: Vec<SocketAddr>,
        code: Option<String>,
    ) -> Self {
        Self {
            generation,
            mode,
            side,
            endpoints,
            inner: Mutex::new(Inner {
                phase: if side == Side::Listener {
                    Phase::Waiting
                } else {
                    Phase::Selecting
                },
                deadline: None,
                code,
                listener: None,
                pinned_id: None,
                pinned_endpoint: None,
                offer: None,
                offset: None,
                copied: 0,
                total: 0,
                verified: false,
                socket: None,
                network_closed: false,
                active: false,
            }),
        }
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Inner>> {
        self.inner
            .lock()
            .map_err(|_| Error::Protocol("Session state unavailable"))
    }

    pub fn phase(&self) -> Result<Phase> {
        let mut inner = self.lock()?;
        if matches!(
            inner.phase,
            Phase::Waiting | Phase::WaitingResume | Phase::Connecting | Phase::Authenticating
        ) && inner
            .deadline
            .is_some_and(|deadline| deadline <= Instant::now())
        {
            inner.end(Phase::Expired);
        }
        Ok(inner.phase)
    }

    // Called only after the whole bind set is ready; caller keeps the listener set private.
    pub fn pairing_code(&self) -> Result<String> {
        let mut inner = self.lock()?;
        if inner.active || !matches!(inner.phase, Phase::Waiting | Phase::WaitingResume) {
            return Err(Error::Protocol("Listener is not ready"));
        }
        inner.eligible(Instant::now())?;
        Ok(inner
            .listener
            .as_ref()
            .ok_or(Error::Protocol("Not a listener"))?
            .code
            .clone())
    }

    // Must precede TCP connect, including retries. No socket is held for human code entry.
    pub fn begin_connect(&self) -> Result<Instant> {
        self.begin_connect_at(Instant::now())
    }
    fn begin_connect_at(&self, now: Instant) -> Result<Instant> {
        let mut inner = self.lock()?;
        if inner.active
            || self.side != Side::Connector
            || !matches!(inner.phase, Phase::Selecting | Phase::WaitingResume)
        {
            return Err(Error::Protocol("Connection not allowed"));
        }
        inner.deadline.get_or_insert(now + WINDOW);
        let deadline = inner.eligible(now)?;
        inner.phase = Phase::Connecting;
        Ok(deadline.min(now + IO_TIMEOUT))
    }

    pub fn authenticate(&self, socket: TcpStream) -> Result<Channel<'_>> {
        let endpoint = if self.side == Side::Connector {
            socket.peer_addr()?
        } else {
            socket.local_addr()?
        };
        if !self.endpoints.contains(&endpoint) {
            return Err(Error::Protocol("Socket does not match selected endpoint"));
        }
        let (deadline, code, id, tls) = {
            let mut inner = self.lock()?;
            if inner.active
                || !matches!(
                    (self.side, inner.phase),
                    (Side::Listener, Phase::Waiting | Phase::WaitingResume)
                        | (Side::Connector, Phase::Connecting)
                )
            {
                return Err(Error::Protocol(
                    "Another connection or restore owns this generation",
                ));
            }
            let deadline = inner
                .eligible(Instant::now())?
                .min(Instant::now() + AUTH_TIMEOUT);
            let (code, id, tls) = if let Some(credentials) = &inner.listener {
                (
                    credentials.code.clone(),
                    Some(credentials.id),
                    Some(credentials.tls.clone()),
                )
            } else {
                (inner.code.clone().ok_or(Error::Cancelled)?, None, None)
            };
            inner.socket = Some(socket.try_clone()?);
            inner.phase = Phase::Authenticating;
            inner.active = true;
            (deadline, code, id, tls)
        };
        let result = (|| {
            let mut stream = SecureStream::tls(socket, self.side, tls, deadline)?;
            let id = auth::pair(&mut stream, self.side, self.mode, &code, id)?;
            if Instant::now() >= deadline {
                return Err(Error::Io(io::ErrorKind::TimedOut.into()));
            }
            let mut inner = self.lock()?;
            inner.eligible(Instant::now())?;
            if inner.pinned_id.is_some_and(|pinned| pinned != id)
                || inner
                    .pinned_endpoint
                    .is_some_and(|pinned| pinned != endpoint)
            {
                return Err(Error::ChangedSession);
            }
            inner.pinned_id = Some(id);
            inner.pinned_endpoint = Some(endpoint);
            inner.phase = Phase::Offer;
            inner.verified = false;
            stream.limits(IO_TIMEOUT, None);
            Ok(Channel {
                session: self,
                stream,
                forwarding: true,
            })
        })();
        if let Err(error) = &result {
            let mut inner = self.lock()?;
            inner.active = false;
            if !inner.phase.terminal() {
                inner.close_network();
                if matches!(
                    error,
                    Error::WrongProof | Error::ChangedSession | Error::Incompatible
                ) && (self.side == Side::Connector || matches!(error, Error::WrongProof))
                {
                    inner.end(Phase::Failed);
                } else if inner.eligible(Instant::now()).is_ok() {
                    inner.phase = if self.side == Side::Listener {
                        Phase::Waiting
                    } else {
                        Phase::WaitingResume
                    };
                }
            }
        }
        result
    }

    pub fn cancel(&self) -> Result<()> {
        let mut inner = self.lock()?;
        inner.close_network();
        inner.network_closed = true;
        if inner.phase == Phase::FinishingImport {
            inner.code = None;
            inner.listener = None;
            inner.pinned_id = None;
            inner.pinned_endpoint = None;
            inner.deadline = None;
        } else if !inner.phase.terminal() {
            let phase = if inner.phase == Phase::AwaitingResult {
                Phase::OutcomeUnknown
            } else {
                Phase::Cancelled
            };
            inner.end(phase);
        }
        Ok(())
    }

    fn check(&self, phase: Phase, content: Content) -> Result<()> {
        let inner = self.lock()?;
        if inner.network_closed {
            return Err(Error::Cancelled);
        }
        if inner.phase != phase || self.mode.content(self.side) != content {
            return Err(Error::Protocol("Operation not allowed in this state"));
        }
        Ok(())
    }

    fn pin_offer(&self, offer: Offer) -> Result<()> {
        let mut inner = self.lock()?;
        if inner.phase != Phase::Offer || inner.network_closed {
            return Err(Error::Cancelled);
        }
        offer.validate()?;
        if inner.offer.is_some_and(|pinned| pinned != offer) {
            inner.end(Phase::Failed);
            return Err(Error::ChangedPackage);
        }
        inner.offer = Some(offer);
        inner.total = offer.size;
        inner.offset = None;
        inner.phase = Phase::Ready;
        Ok(())
    }

    fn transition(&self, from: Phase, to: Phase) -> Result<()> {
        let mut inner = self.lock()?;
        if inner.network_closed {
            return Err(Error::Cancelled);
        }
        if inner.phase != from {
            return Err(Error::Protocol("Unexpected session state"));
        }
        inner.phase = to;
        Ok(())
    }

    pub fn connect_failed(&self) -> Result<()> {
        let mut inner = self.lock()?;
        if inner.active || inner.phase != Phase::Connecting {
            return Err(Error::Protocol("No pending TCP connection"));
        }
        inner.disconnected();
        Ok(())
    }

    fn connection_lost(&self) {
        if let Ok(mut inner) = self.lock() {
            inner.disconnected();
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.cancel();
    }
}

pub(super) struct Channel<'a> {
    session: &'a Session,
    stream: SecureStream,
    forwarding: bool,
}

impl Channel<'_> {
    pub fn reject(&mut self, error: LibraryTransferError) -> Result<()> {
        self.session.check(Phase::Ready, Content::Receiver)?;
        let retryable = error.is_retryable();
        let result = self.write_frame(Frame::Status(ReceiverMessage::Failed(error)));
        if retryable {
            self.session.connection_lost();
        } else {
            self.session.lock()?.end(Phase::Failed);
        }
        result
    }
    fn read_frame(&mut self, expected: Expected, wait: Duration) -> Result<Frame> {
        let outer = Instant::now() + wait;
        self.stream.limits(wait, Some(outer));
        let mut kind = [0];
        self.stream.read_exact(&mut kind)?;
        self.stream
            .limits(IO_TIMEOUT, Some(outer.min(Instant::now() + IO_TIMEOUT)));
        let result = codec::read_body(&mut self.stream, kind[0], expected).map_err(|error| {
            if matches!(&error,Error::Io(io) if io.kind()==io::ErrorKind::UnexpectedEof) {
                Error::Protocol("Truncated control frame")
            } else {
                error
            }
        });
        self.stream.limits(IO_TIMEOUT, None);
        result
    }

    fn write_frame(&mut self, frame: Frame) -> Result<()> {
        self.stream
            .limits(IO_TIMEOUT, Some(Instant::now() + IO_TIMEOUT));
        let result = codec::write_frame(&mut self.stream, &frame);
        self.stream.limits(IO_TIMEOUT, None);
        result
    }

    pub fn send_offer(&mut self, offer: Offer) -> Result<()> {
        self.session.check(Phase::Offer, Content::Sender)?;
        self.session.pin_offer(offer)?;
        self.write_frame(Frame::Offer(offer))
            .inspect_err(|_| self.session.connection_lost())
    }

    pub fn receive_offer(&mut self) -> Result<Offer> {
        self.session.check(Phase::Offer, Content::Receiver)?;
        let frame = self.read_frame(Expected::Offer, IO_TIMEOUT);
        match frame {
            Ok(Frame::Offer(offer)) => {
                self.session.pin_offer(offer)?;
                Ok(offer)
            }
            Err(error) => {
                self.fail_control(&error)?;
                Err(error)
            }
            _ => unreachable!(),
        }
    }

    fn fail_control(&self, error: &Error) -> Result<()> {
        if matches!(error, Error::Io(_)) {
            self.session.connection_lost();
        } else {
            self.session.lock()?.end(Phase::Failed);
        }
        Ok(())
    }

    pub fn wait_ready(&mut self) -> Result<u64> {
        self.session.check(Phase::Ready, Content::Sender)?;
        let size = self
            .session
            .lock()?
            .offer
            .ok_or(Error::Protocol("Missing offer"))?
            .size;
        match self.read_frame(Expected::Ready { size }, WINDOW) {
            Ok(Frame::Ready(offset)) => {
                let mut inner = self.session.lock()?;
                if inner.network_closed || inner.phase != Phase::Ready {
                    return Err(Error::Cancelled);
                }
                inner.offset = Some(offset);
                inner.phase = Phase::Streaming;
                Ok(offset)
            }
            Ok(Frame::Status(ReceiverMessage::Failed(error))) => {
                if error.is_retryable() {
                    self.session.connection_lost();
                } else {
                    self.session.lock()?.end(Phase::Failed);
                }
                Err(Error::Remote(error))
            }
            Err(error) => {
                self.fail_control(&error)?;
                Err(error)
            }
            _ => unreachable!(),
        }
    }

    pub fn send_archive(&mut self, archive: &mut (impl Read + Seek)) -> Result<()> {
        self.session.check(Phase::Streaming, Content::Sender)?;
        let offer = self
            .session
            .lock()?
            .offer
            .ok_or(Error::Protocol("Missing offer"))?;
        let offset = self
            .session
            .lock()?
            .offset
            .ok_or(Error::Protocol("Missing Ready offset"))?;
        if offset > offer.size {
            self.session.lock()?.end(Phase::Failed);
            return Err(Error::Protocol("Invalid resume offset"));
        }
        if archive.seek(SeekFrom::End(0))? != offer.size {
            self.session.lock()?.end(Phase::Failed);
            return Err(Error::ChangedPackage);
        }
        archive.seek(SeekFrom::Start(offset))?;
        self.session.lock()?.copied = offset;
        let mut left = offer.size - offset;
        let mut buffer = [0; BUFFER];
        while left > 0 {
            self.session.check(Phase::Streaming, Content::Sender)?;
            let n = (left.min(BUFFER as u64)) as usize;
            if let Err(error) = archive.read_exact(&mut buffer[..n]) {
                self.session.lock()?.end(Phase::Failed);
                return Err(error.into());
            }
            // A failed final write/flush cannot prove that the receiver got no complete archive.
            if left == n as u64 {
                self.session
                    .transition(Phase::Streaming, Phase::AwaitingResult)?;
            }
            if let Err(error) = self.stream.write_all(&buffer[..n]) {
                self.session.connection_lost();
                return Err(if left == n as u64 {
                    Error::OutcomeUnknown
                } else {
                    error.into()
                });
            }
            left -= n as u64;
            self.session.lock()?.copied = offer.size - left;
        }
        if offer.size == offset {
            self.session
                .transition(Phase::Streaming, Phase::AwaitingResult)?;
        }
        if self.stream.flush().is_err() {
            self.session.connection_lost();
            return Err(Error::OutcomeUnknown);
        }
        Ok(())
    }

    // Staging is a native-owned private file supplied by orchestration, never a peer path.
    // A DigestMismatch makes it unusable; the owner must delete it on terminal cleanup.
    pub fn receive_archive(
        &mut self,
        staging: &mut (impl Read + Write + Seek),
        available: u64,
    ) -> Result<()> {
        self.session.check(Phase::Ready, Content::Receiver)?;
        let offer = self
            .session
            .lock()?
            .offer
            .ok_or(Error::Protocol("Missing offer"))?;
        let offset = staging.seek(SeekFrom::End(0))?;
        if offset > offer.size {
            self.session.lock()?.end(Phase::Failed);
            return Err(Error::Protocol("Partial exceeds offered archive"));
        }
        if offer.size - offset > available {
            let error = LibraryTransferError::insufficient_space(offer.size - offset, available);
            let _ = self.write_frame(Frame::Status(ReceiverMessage::Failed(error.clone())));
            self.session.connection_lost();
            return Err(Error::Remote(error));
        }
        self.write_frame(Frame::Ready(offset))
            .inspect_err(|_| self.session.connection_lost())?;
        self.session.transition(Phase::Ready, Phase::Streaming)?;
        self.session.lock()?.copied = offset;
        let result = (|| {
            let mut left = offer.size - offset;
            let mut buffer = [0; BUFFER];
            while left > 0 {
                self.session.check(Phase::Streaming, Content::Receiver)?;
                let n = (left.min(BUFFER as u64)) as usize;
                self.stream.read_exact(&mut buffer[..n])?;
                if left == n as u64 {
                    self.session
                        .transition(Phase::Streaming, Phase::Verifying)?;
                }
                staging.write_all(&buffer[..n])?;
                left -= n as u64;
                self.session.lock()?.copied = offer.size - left;
            }
            if offset == offer.size {
                self.session
                    .transition(Phase::Streaming, Phase::Verifying)?;
            }
            staging.flush()?;
            staging.seek(SeekFrom::Start(0))?;
            let actual = hash_archive(staging, || {
                self.session.check(Phase::Verifying, Content::Receiver)
            })?;
            if actual != offer {
                return Err(Error::DigestMismatch);
            }
            let mut inner = self.session.lock()?;
            if inner.phase != Phase::Verifying || inner.network_closed {
                return Err(Error::Cancelled);
            }
            inner.verified = true;
            Ok(())
        })();
        if let Err(error) = &result {
            // No retry after local copy completion or invalid/corrupt staging.
            if self.session.phase()? == Phase::Verifying || !matches!(error, Error::Io(_)) {
                self.session.lock()?.end(Phase::Failed);
            } else {
                self.session.connection_lost();
            }
        }
        result
    }

    pub fn progress(&mut self, progress: LibraryTransferProgress) -> Result<()> {
        let inner = self.session.lock()?;
        if self.session.mode.content(self.session.side) != Content::Receiver
            || !matches!(inner.phase, Phase::Verifying | Phase::FinishingImport)
        {
            return Err(Error::Protocol("Progress is not allowed in this state"));
        }
        let open = !inner.network_closed;
        drop(inner);
        if open
            && self.forwarding
            && self
                .write_frame(Frame::Status(ReceiverMessage::Progress(progress)))
                .is_err()
        {
            self.forwarding = false;
        }
        Ok(())
    }

    pub fn finish_import(
        &mut self,
        result: std::result::Result<(), LibraryTransferError>,
    ) -> Result<()> {
        if self.session.phase()? != Phase::FinishingImport {
            return Err(Error::Protocol("Import was not admitted"));
        }
        let phase = if result.is_ok() {
            Phase::Complete
        } else {
            Phase::Failed
        };
        let message = match result {
            Ok(()) => ReceiverMessage::Complete,
            Err(error) => ReceiverMessage::Failed(error),
        };
        if self.forwarding && !self.session.lock()?.network_closed {
            let _ = self.write_frame(Frame::Status(message));
        }
        self.session.lock()?.end(phase);
        Ok(())
    }

    pub fn await_result(
        &mut self,
        mut progress: impl FnMut(LibraryTransferProgress),
    ) -> Result<()> {
        if self.session.phase()? == Phase::OutcomeUnknown {
            return Err(Error::OutcomeUnknown);
        }
        self.session.check(Phase::AwaitingResult, Content::Sender)?;
        loop {
            let frame = self.read_frame(Expected::Result, WINDOW);
            let mut inner = self.session.lock()?;
            if inner.phase != Phase::AwaitingResult || inner.network_closed {
                return Err(Error::OutcomeUnknown);
            }
            match frame {
                Ok(Frame::Status(ReceiverMessage::Progress(value))) => {
                    drop(inner);
                    progress(value);
                }
                Ok(Frame::Status(ReceiverMessage::Complete)) => {
                    inner.end(Phase::Complete);
                    return Ok(());
                }
                Ok(Frame::Status(ReceiverMessage::Failed(error))) => {
                    inner.end(Phase::Failed);
                    return Err(Error::Remote(error));
                }
                _ => {
                    inner.end(Phase::OutcomeUnknown);
                    return Err(Error::OutcomeUnknown);
                }
            }
        }
    }
}

impl Drop for Channel<'_> {
    fn drop(&mut self) {
        if let Ok(mut inner) = self.session.lock() {
            inner.disconnected();
            inner.active = false;
        }
    }
}

// Sender hashes the immutable prepared archive before connecting. Receiver uses
// the same bounded pass, including when a resume already contains all N bytes.
pub(super) fn hash_archive(
    reader: &mut impl Read,
    mut check: impl FnMut() -> Result<()>,
) -> Result<Offer> {
    let mut hash = Sha256::new();
    let mut size = 0u64;
    let mut buffer = [0; BUFFER];
    loop {
        check()?;
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        size = size
            .checked_add(n as u64)
            .ok_or(Error::Protocol("Archive size overflow"))?;
        Offer {
            size,
            digest: [0; 32],
        }
        .validate()?;
        hash.update(&buffer[..n]);
    }
    check()?;
    Offer {
        size,
        digest: hash.finalize().into(),
    }
    .validate()
}
