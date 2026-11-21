//! TLS connection roles, four-flight PAKE and absolute I/O budgets.

use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::{Duration, Instant};

use hmac::{Hmac, Mac};
use rustls::pki_types::ServerName;
use rustls::{ClientConnection, ServerConfig, ServerConnection, StreamOwned};
use spake2::{Ed25519Group, Identity, Password, Spake2};

use super::super::security::client_tls_config;
use super::{Error, Mode, Result, Side, IO_TIMEOUT};

pub(super) const MAGIC: &[u8; 8] = b"PCLAN004";
const CONNECTOR_ID: &[u8] = b"papercut-library-transfer-v4-connector";
const LISTENER_ID: &[u8] = b"papercut-library-transfer-v4-listener";
const EXPORTER: &[u8] = b"EXPORTER-papercut-library-transfer-v4";
type Tag = Hmac<sha2::Sha256>;

// No Debug/Serialize: credentials and channel/session identifiers stay native.
pub(super) struct Credentials {
    pub code: String,
    pub id: [u8; 16],
    pub tls: Arc<ServerConfig>,
}

pub(super) struct BudgetSocket {
    socket: TcpStream,
    pub deadline: Option<Instant>,
    pub inactivity: Duration,
}

impl BudgetSocket {
    fn timeout(&self) -> io::Result<Duration> {
        let timeout = self.deadline.map_or(self.inactivity, |deadline| {
            self.inactivity
                .min(deadline.saturating_duration_since(Instant::now()))
        });
        if timeout.is_zero() {
            return Err(io::ErrorKind::TimedOut.into());
        }
        Ok(timeout)
    }
}

impl Read for BudgetSocket {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.socket.set_read_timeout(Some(self.timeout()?))?;
        self.socket.read(bytes)
    }
}

impl Write for BudgetSocket {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.socket.set_write_timeout(Some(self.timeout()?))?;
        self.socket.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.socket.flush()
    }
}

pub(super) enum SecureStream {
    Connector(StreamOwned<ClientConnection, BudgetSocket>),
    Listener(StreamOwned<ServerConnection, BudgetSocket>),
}

impl SecureStream {
    pub fn tls(
        socket: TcpStream,
        side: Side,
        config: Option<Arc<ServerConfig>>,
        deadline: Instant,
    ) -> Result<Self> {
        socket.set_nonblocking(false)?;
        socket.set_nodelay(true)?;
        let mut socket = BudgetSocket {
            socket,
            deadline: Some(deadline),
            inactivity: IO_TIMEOUT,
        };
        let stream = match side {
            Side::Connector => {
                let mut connection = ClientConnection::new(
                    client_tls_config().map_err(|_| Error::Protocol("TLS configuration failed"))?,
                    ServerName::try_from("papercut.local").unwrap().to_owned(),
                )
                .map_err(|_| Error::Protocol("TLS initialization failed"))?;
                connection.complete_io(&mut socket)?;
                Self::Connector(StreamOwned::new(connection, socket))
            }
            Side::Listener => {
                let mut connection = ServerConnection::new(
                    config.ok_or(Error::Protocol("Missing listener TLS configuration"))?,
                )
                .map_err(|_| Error::Protocol("TLS initialization failed"))?;
                connection.complete_io(&mut socket)?;
                Self::Listener(StreamOwned::new(connection, socket))
            }
        };
        stream.socket().timeout()?;
        Ok(stream)
    }

    fn socket(&self) -> &BudgetSocket {
        match self {
            Self::Connector(s) => &s.sock,
            Self::Listener(s) => &s.sock,
        }
    }

    pub fn limits(&mut self, inactivity: Duration, deadline: Option<Instant>) {
        let socket = match self {
            Self::Connector(s) => &mut s.sock,
            Self::Listener(s) => &mut s.sock,
        };
        socket.inactivity = inactivity;
        socket.deadline = deadline;
    }

    fn exporter(&self) -> Result<[u8; 32]> {
        let result = match self {
            Self::Connector(s) => s.conn.export_keying_material([0; 32], EXPORTER, None),
            Self::Listener(s) => s.conn.export_keying_material([0; 32], EXPORTER, None),
        };
        result.map_err(|_| Error::Protocol("TLS exporter failed"))
    }
}

impl Read for SecureStream {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.socket().timeout()?;
        let result = match self {
            Self::Connector(s) => s.read(bytes),
            Self::Listener(s) => s.read(bytes),
        };
        self.socket().timeout()?;
        result
    }
}

impl Write for SecureStream {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.socket().timeout()?;
        let result = match self {
            Self::Connector(s) => s.write(bytes),
            Self::Listener(s) => s.write(bytes),
        };
        self.socket().timeout()?;
        result
    }
    fn flush(&mut self) -> io::Result<()> {
        self.socket().timeout()?;
        let result = match self {
            Self::Connector(s) => s.flush(),
            Self::Listener(s) => s.flush(),
        };
        self.socket().timeout()?;
        result
    }
}

fn hello<const N: usize>(mode: Mode, id: Option<[u8; 16]>, message: &[u8]) -> [u8; N] {
    let mut bytes = [0; N];
    bytes[..8].copy_from_slice(MAGIC);
    bytes[8] = mode as u8;
    if let Some(id) = id {
        bytes[9..25].copy_from_slice(&id);
    }
    bytes[N - 33..].copy_from_slice(message);
    bytes
}

fn read_hello<const N: usize>(reader: &mut impl Read, mode: Mode) -> Result<[u8; N]> {
    let mut bytes = [0; N];
    reader.read_exact(&mut bytes[..8])?;
    if &bytes[..8] != MAGIC {
        return Err(Error::Incompatible);
    }
    reader.read_exact(&mut bytes[8..9])?;
    if bytes[8] != mode as u8 {
        return Err(Error::Protocol("Unexpected transfer mode"));
    }
    reader.read_exact(&mut bytes[9..])?;
    Ok(bytes)
}

fn confirmation(key: &[u8], c: &[u8; 42], l: &[u8; 58], exporter: &[u8; 32], side: Side) -> Tag {
    let mut mac = Tag::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(b"papercut-library-transfer-v4-confirm\0");
    mac.update(c);
    mac.update(l);
    mac.update(exporter);
    mac.update(&[match side {
        Side::Connector => 1,
        Side::Listener => 2,
    }]);
    mac
}

pub(super) fn pair(
    stream: &mut SecureStream,
    side: Side,
    mode: Mode,
    code: &str,
    id: Option<[u8; 16]>,
) -> Result<[u8; 16]> {
    let exporter = stream.exporter()?;
    let password = Password::new(code);
    let a = Identity::new(CONNECTOR_ID);
    let b = Identity::new(LISTENER_ID);
    let (pake, message) = match side {
        Side::Connector => Spake2::<Ed25519Group>::start_a(&password, &a, &b),
        Side::Listener => Spake2::<Ed25519Group>::start_b(&password, &a, &b),
    };
    let (c, l) = match side {
        Side::Connector => {
            let c = hello::<42>(mode, None, &message);
            stream.write_all(&c)?;
            stream.flush()?;
            (c, read_hello::<58>(stream, mode)?)
        }
        Side::Listener => {
            let c = read_hello::<42>(stream, mode)?;
            let l = hello::<58>(
                mode,
                Some(id.ok_or(Error::Protocol("Missing listener session"))?),
                &message,
            );
            // Validate the group before replying to malformed unauthenticated traffic.
            let key = pake
                .finish(&c[9..])
                .map_err(|_| Error::Protocol("Invalid PAKE message"))?;
            stream.write_all(&l)?;
            stream.flush()?;
            verify_proof(
                stream,
                confirmation(&key, &c, &l, &exporter, Side::Connector),
            )?;
            stream.write_all(
                &confirmation(&key, &c, &l, &exporter, Side::Listener)
                    .finalize()
                    .into_bytes(),
            )?;
            stream.flush()?;
            return Ok(l[9..25].try_into().unwrap());
        }
    };
    let key = pake
        .finish(&l[25..])
        .map_err(|_| Error::Protocol("Invalid PAKE message"))?;
    stream.write_all(
        &confirmation(&key, &c, &l, &exporter, Side::Connector)
            .finalize()
            .into_bytes(),
    )?;
    stream.flush()?;
    verify_proof(
        stream,
        confirmation(&key, &c, &l, &exporter, Side::Listener),
    )?;
    Ok(l[9..25].try_into().unwrap())
}

fn verify_proof(reader: &mut impl Read, expected: Tag) -> Result<()> {
    let mut proof = [0; 32];
    reader.read_exact(&mut proof)?;
    expected.verify_slice(&proof).map_err(|_| Error::WrongProof)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_transcripts_and_role_channel_binding() {
        let key = std::array::from_fn::<_, 32, _>(|i| i as u8);
        let exporter = key.map(|b| b + 32);
        let mut a = [0; 33];
        a[0] = b'A';
        a[1..].copy_from_slice(&key);
        let mut b = [0; 33];
        b[0] = b'B';
        b[1..].copy_from_slice(&exporter);
        for (mode, ct, lt) in [
            (
                Mode::Push,
                "72d53e91684808e350f4b0482158d9abe7cc4449b55580af7ebc964445814013",
                "defef37eecaf9d9d5fb7b61a1cd227433cf1941073499979abd1fb2fe36b632a",
            ),
            (
                Mode::Pull,
                "409b69ad2a0ecb8995db9ae0506c93d5ab1e890100f01dcafe0ac4ce8016bfe0",
                "8267d4fb112bf47405cc095dc773fd1e904e36256860856467882880039dc836",
            ),
        ] {
            let c = hello::<42>(mode, None, &a);
            let l = hello::<58>(mode, Some(key[..16].try_into().unwrap()), &b);
            for (side, expected) in [(Side::Connector, ct), (Side::Listener, lt)] {
                let proof = confirmation(&key, &c, &l, &exporter, side)
                    .finalize()
                    .into_bytes();
                assert_eq!(
                    proof.iter().map(|v| format!("{v:02x}")).collect::<String>(),
                    expected
                );
                for i in 0..c.len() {
                    let mut changed = c;
                    changed[i] ^= 1;
                    assert!(confirmation(&key, &changed, &l, &exporter, side)
                        .verify_slice(&proof)
                        .is_err());
                }
                for i in 0..l.len() {
                    let mut changed = l;
                    changed[i] ^= 1;
                    assert!(confirmation(&key, &c, &changed, &exporter, side)
                        .verify_slice(&proof)
                        .is_err());
                }
                for i in 0..exporter.len() {
                    let mut changed = exporter;
                    changed[i] ^= 1;
                    assert!(confirmation(&key, &c, &l, &changed, side)
                        .verify_slice(&proof)
                        .is_err());
                }
                assert!(confirmation(
                    &key,
                    &c,
                    &l,
                    &exporter,
                    if side == Side::Connector {
                        Side::Listener
                    } else {
                        Side::Connector
                    }
                )
                .verify_slice(&proof)
                .is_err());
            }
        }
    }

    #[test]
    fn old_magic_and_wrong_mode_are_rejected_without_waiting_for_payload() {
        assert!(matches!(
            read_hello::<42>(&mut &b"PCLAN003"[..], Mode::Push),
            Err(Error::Incompatible)
        ));
        assert!(matches!(
            read_hello::<42>(&mut &b"PCLAN004\x02"[..], Mode::Push),
            Err(Error::Protocol(_))
        ));
    }
}
