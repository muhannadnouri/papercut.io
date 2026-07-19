//! Foreground, one-use library transfer over the local network.
//!
//! High-level flow:
//! 1. The source builds the same checked `.papercut-library` package used by
//!    file export, binds an IPv4 listener, and displays its address plus a
//!    random one-use code.
//! 2. The target enters those values. Both peers establish ephemeral TLS, then
//!    prove knowledge of the code with HMACs bound to that exact TLS session.
//!    This authenticates the self-signed connection without a permanent key or
//!    certificate warning.
//! 3. The source streams the package once and destroys the temporary file. The
//!    target stages it in app cache and invokes the normal package importer, so
//!    network transfer cannot bypass checksums, sanitization, merge rules, or
//!    search-index rebuilding.
//!
//! Sessions stay foreground-only, expire after ten minutes, and consume the
//! code after the first connection attempt to prevent online guessing. Device
//! discovery, background transfer, and resumable byte ranges intentionally live
//! outside this module until the manual pairing path has proven reliable.

use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream, UdpSocket};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use hmac::{Hmac, Mac};
use rcgen::generate_simple_self_signed;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{verify_tls12_signature, verify_tls13_signature, WebPkiSupportedAlgorithms};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::{
    ClientConfig, ClientConnection, DigitallySignedStruct, Error as TlsError, ServerConfig,
    ServerConnection, SignatureScheme,
};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use tauri::State;

use super::package::MAX_PACKAGE_BYTES;
use super::{
    build_library_package, emit_transfer_progress, ensure_available_space, import_library_package,
    transfer_temp_path, LibraryTransferExportRequest, LibraryTransferImportResult,
};

const PROTOCOL_MAGIC: &[u8; 8] = b"PCLAN001";
const TLS_EXPORTER_LABEL: &[u8] = b"papercut-library-transfer-v1";
const RECEIVER_ROLE: &[u8] = b"receiver";
const SENDER_ROLE: &[u8] = b"sender";
const PROOF_BYTES: usize = 32;
const CODE_ALPHABET: &[u8; 32] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";
const CODE_CHARS: usize = 8;
const SESSION_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const SOCKET_TIMEOUT: Duration = Duration::from_secs(30);
const COPY_BUFFER_BYTES: usize = 64 * 1024;
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Default)]
pub struct LibraryTransferState {
    send: Arc<Mutex<Option<SendSession>>>,
}

#[derive(Clone)]
struct SendSession {
    cancel: Arc<AtomicBool>,
    status: Arc<Mutex<LibraryTransferSendStatus>>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryTransferSendStatus {
    state: LibraryTransferSendState,
    address: String,
    code: String,
    documents: usize,
    audiobooks: usize,
    package_bytes: u64,
    bytes_transferred: u64,
    error: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum LibraryTransferSendState {
    Waiting,
    Sending,
    Complete,
    Cancelled,
    Failed,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryTransferReceiveRequest {
    address: String,
    code: String,
}

/// Prepare one package, bind a foreground listener, and return the address and
/// one-use code before a receiver connects. A second active send is rejected so
/// two callers cannot replace each other's package or session credentials.
#[tauri::command]
pub async fn library_transfer_send_start(
    app: tauri::AppHandle,
    state: State<'_, LibraryTransferState>,
    request: Option<LibraryTransferExportRequest>,
) -> Result<LibraryTransferSendStatus, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        start_send(app, state, request.unwrap_or_default().include_audiobooks)
    })
    .await
    .map_err(|err| format!("Library send task failed: {err}"))?
}

#[tauri::command]
pub fn library_transfer_send_status(
    state: State<'_, LibraryTransferState>,
) -> Result<Option<LibraryTransferSendStatus>, String> {
    let send = lock(&state.send)?;
    send.as_ref()
        .map(|session| lock(&session.status).map(|status| status.clone()))
        .transpose()
}

/// Stop a waiting or active foreground sender. The worker owns package cleanup
/// so cancellation cannot remove a file while another thread is reading it.
#[tauri::command]
pub fn library_transfer_send_cancel(state: State<'_, LibraryTransferState>) -> Result<(), String> {
    let send = lock(&state.send)?;
    if let Some(session) = send.as_ref() {
        session.cancel.store(true, Ordering::Relaxed);
    }
    Ok(())
}

/// Receive exactly one authenticated package into app cache, then hand it to
/// the normal package importer. Network bytes never bypass archive validation,
/// checksums, HTML sanitization, or target-side index rebuilding.
#[tauri::command]
pub async fn library_transfer_receive(
    app: tauri::AppHandle,
    request: LibraryTransferReceiveRequest,
) -> Result<LibraryTransferImportResult, String> {
    tauri::async_runtime::spawn_blocking(move || receive_library(app, request))
        .await
        .map_err(|err| format!("Library receive task failed: {err}"))?
}

fn start_send(
    app: tauri::AppHandle,
    state: LibraryTransferState,
    include_audiobooks: bool,
) -> Result<LibraryTransferSendStatus, String> {
    {
        let send = lock(&state.send)?;
        if send.as_ref().is_some_and(|session| {
            lock(&session.status)
                .map(|status| {
                    matches!(
                        status.state,
                        LibraryTransferSendState::Waiting | LibraryTransferSendState::Sending
                    )
                })
                .unwrap_or(true)
        }) {
            return Err("A library send is already active".into());
        }
    }

    let package_path = transfer_temp_path(&app, "lan-send")?;
    let prepared = build_library_package(&app, &package_path, include_audiobooks);
    let prepared = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            let _ = fs::remove_file(&package_path);
            return Err(error);
        }
    };
    let package_bytes = fs::metadata(&package_path)
        .map_err(|err| format!("Failed to inspect prepared library package: {err}"))?
        .len();
    let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0))
        .map_err(|err| format!("Failed to start local library transfer: {err}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|err| format!("Failed to configure local library transfer: {err}"))?;
    let port = listener
        .local_addr()
        .map_err(|err| format!("Failed to read local library transfer address: {err}"))?
        .port();
    let address = SocketAddrV4::new(local_ipv4()?, port).to_string();
    let code = new_pairing_code()?;
    let tls_config = server_tls_config()?;
    let status = Arc::new(Mutex::new(LibraryTransferSendStatus {
        state: LibraryTransferSendState::Waiting,
        address,
        code: display_code(&code),
        documents: prepared.documents,
        audiobooks: prepared.audiobooks,
        package_bytes,
        bytes_transferred: 0,
        error: None,
    }));
    let cancel = Arc::new(AtomicBool::new(false));
    let session = SendSession {
        cancel: Arc::clone(&cancel),
        status: Arc::clone(&status),
    };
    *lock(&state.send)? = Some(session);

    let thread_status = Arc::clone(&status);
    thread::spawn(move || {
        run_sender(
            listener,
            tls_config,
            &code,
            &package_path,
            &thread_status,
            &cancel,
        );
        let _ = fs::remove_file(package_path);
    });
    lock(&status).map(|status| status.clone())
}

/// Accept one connection only. A failed authentication consumes the session,
/// preventing online guessing against the short code; restarting creates fresh
/// credentials and a fresh ephemeral certificate.
fn run_sender(
    listener: TcpListener,
    tls_config: Arc<ServerConfig>,
    code: &str,
    package_path: &Path,
    status: &Arc<Mutex<LibraryTransferSendStatus>>,
    cancel: &AtomicBool,
) {
    let deadline = Instant::now() + SESSION_TIMEOUT;
    loop {
        if cancel.load(Ordering::Relaxed) {
            set_send_status(status, LibraryTransferSendState::Cancelled, None);
            return;
        }
        if Instant::now() >= deadline {
            set_send_status(
                status,
                LibraryTransferSendState::Failed,
                Some("Library send timed out".into()),
            );
            return;
        }
        match listener.accept() {
            Ok((stream, _)) => {
                set_send_status(status, LibraryTransferSendState::Sending, None);
                let result = send_package(stream, tls_config, code, package_path, status, cancel);
                if cancel.load(Ordering::Relaxed) {
                    set_send_status(status, LibraryTransferSendState::Cancelled, None);
                    return;
                }
                match result {
                    Ok(()) => set_send_status(status, LibraryTransferSendState::Complete, None),
                    Err(error) => {
                        set_send_status(status, LibraryTransferSendState::Failed, Some(error))
                    }
                }
                return;
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(100));
            }
            Err(error) => {
                set_send_status(
                    status,
                    LibraryTransferSendState::Failed,
                    Some(format!("Local library transfer failed: {error}")),
                );
                return;
            }
        }
    }
}

fn receive_library(
    app: tauri::AppHandle,
    request: LibraryTransferReceiveRequest,
) -> Result<LibraryTransferImportResult, String> {
    emit_transfer_progress(&app, "receive", "connecting", None, None, None);
    let address = parse_local_address(&request.address)?;
    let code = normalize_code(&request.code)?;
    let temp_path = transfer_temp_path(&app, "lan-receive")?;
    let result = (|| {
        let cache = temp_path
            .parent()
            .ok_or_else(|| "Temporary library package path has no parent".to_string())?;
        let mut output = BufWriter::new(
            File::create(&temp_path)
                .map_err(|err| format!("Failed to create received library package: {err}"))?,
        );
        receive_package(address, &code, cache, &mut output, |processed, total| {
            emit_transfer_progress(
                &app,
                "receive",
                "receiving",
                Some((processed, total)),
                None,
                None,
            );
        })?;
        output
            .flush()
            .map_err(|err| format!("Failed to finish received library package: {err}"))?;
        drop(output);
        import_library_package(&app, &temp_path, "receive")
    })();
    let _ = fs::remove_file(&temp_path);
    result
}

fn send_package(
    mut socket: TcpStream,
    tls_config: Arc<ServerConfig>,
    code: &str,
    package_path: &Path,
    status: &Arc<Mutex<LibraryTransferSendStatus>>,
    cancel: &AtomicBool,
) -> Result<(), String> {
    configure_socket(&socket)?;
    let mut connection = ServerConnection::new(tls_config)
        .map_err(|err| format!("Failed to initialize secure library transfer: {err}"))?;
    connection
        .complete_io(&mut socket)
        .map_err(|err| format!("Secure library transfer handshake failed: {err}"))?;
    let exporter = tls_exporter(&connection)?;
    let mut stream = rustls::Stream::new(&mut connection, &mut socket);
    read_and_verify_proof(&mut stream, code, &exporter, RECEIVER_ROLE)?;
    stream
        .write_all(PROTOCOL_MAGIC)
        .and_then(|_| stream.write_all(&pairing_proof(code, &exporter, SENDER_ROLE)?))
        .map_err(|err| format!("Failed to authenticate library sender: {err}"))?;

    let package = File::open(package_path)
        .map_err(|err| format!("Failed to open prepared library package: {err}"))?;
    let package_bytes = package
        .metadata()
        .map_err(|err| format!("Failed to inspect prepared library package: {err}"))?
        .len();
    stream
        .write_all(&package_bytes.to_be_bytes())
        .map_err(|err| format!("Failed to send library package header: {err}"))?;
    let mut last_progress = Instant::now() - PROGRESS_INTERVAL;
    let copied = copy_exact_with_progress(
        &mut BufReader::new(package),
        &mut stream,
        package_bytes,
        || cancel.load(Ordering::Relaxed),
        |processed| {
            if processed == package_bytes || last_progress.elapsed() >= PROGRESS_INTERVAL {
                if let Ok(mut current) = status.lock() {
                    current.bytes_transferred = processed;
                }
                last_progress = Instant::now();
            }
        },
    )
    .map_err(|err| format!("Failed to send library package: {err}"))?;
    if copied != package_bytes {
        return Err("Prepared library package changed before sending completed".into());
    }
    stream
        .flush()
        .map_err(|err| format!("Failed to finish library send: {err}"))
}

fn receive_package<W: Write, P: FnMut(u64, u64)>(
    address: SocketAddrV4,
    code: &str,
    staging_dir: &Path,
    output: &mut W,
    mut progress: P,
) -> Result<(), String> {
    let mut socket = TcpStream::connect_timeout(&SocketAddr::V4(address), SOCKET_TIMEOUT)
        .map_err(|err| format!("Could not connect to the source device: {err}"))?;
    configure_socket(&socket)?;
    let config = client_tls_config()?;
    let server_name = ServerName::IpAddress((*address.ip()).into());
    let mut connection = ClientConnection::new(config, server_name)
        .map_err(|err| format!("Failed to initialize secure library transfer: {err}"))?;
    connection
        .complete_io(&mut socket)
        .map_err(|err| format!("Secure library transfer handshake failed: {err}"))?;
    let exporter = tls_exporter(&connection)?;
    let mut stream = rustls::Stream::new(&mut connection, &mut socket);
    stream
        .write_all(PROTOCOL_MAGIC)
        .and_then(|_| stream.write_all(&pairing_proof(code, &exporter, RECEIVER_ROLE)?))
        .and_then(|_| stream.flush())
        .map_err(|err| format!("Failed to authenticate library receiver: {err}"))?;
    read_and_verify_proof(&mut stream, code, &exporter, SENDER_ROLE)?;

    let mut size = [0u8; 8];
    stream
        .read_exact(&mut size)
        .map_err(|err| format!("Failed to read library package header: {err}"))?;
    let size = u64::from_be_bytes(size);
    if size == 0 || size > MAX_PACKAGE_BYTES {
        return Err("Source device reported an unsupported library package size".into());
    }
    ensure_available_space(staging_dir, size)?;
    let mut last_progress = Instant::now() - PROGRESS_INTERVAL;
    let copied = copy_exact_with_progress(
        &mut stream,
        output,
        size,
        || false,
        |processed| {
            if processed == size || last_progress.elapsed() >= PROGRESS_INTERVAL {
                progress(processed, size);
                last_progress = Instant::now();
            }
        },
    )
    .map_err(|err| format!("Failed to receive library package: {err}"))?;
    if copied != size {
        return Err("Source device disconnected before the library package was complete".into());
    }
    Ok(())
}

/// Copy an announced byte count without buffering the whole package. The
/// callback is throttled by callers, while cancellation is checked between
/// chunks so stopping a sender also interrupts an active transfer.
fn copy_exact_with_progress<R, W, C, P>(
    reader: &mut R,
    writer: &mut W,
    total: u64,
    mut cancelled: C,
    mut progress: P,
) -> io::Result<u64>
where
    R: Read,
    W: Write,
    C: FnMut() -> bool,
    P: FnMut(u64),
{
    let mut buffer = [0u8; COPY_BUFFER_BYTES];
    let mut copied = 0u64;
    while copied < total {
        if cancelled() {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "library transfer cancelled",
            ));
        }
        let remaining = usize::try_from((total - copied).min(COPY_BUFFER_BYTES as u64))
            .unwrap_or(COPY_BUFFER_BYTES);
        let read = reader.read(&mut buffer[..remaining])?;
        if read == 0 {
            break;
        }
        writer.write_all(&buffer[..read])?;
        copied += read as u64;
        progress(copied);
    }
    Ok(copied)
}

/// The ephemeral certificate encrypts TLS, while the HMAC below authenticates
/// the exact TLS channel with the displayed one-use code. This avoids shipping
/// a long-lived private key or asking users to trust a local certificate.
fn pairing_proof(code: &str, exporter: &[u8], role: &[u8]) -> io::Result<[u8; PROOF_BYTES]> {
    let mut mac = HmacSha256::new_from_slice(code.as_bytes())
        .map_err(|_| io::Error::other("Invalid library transfer code"))?;
    mac.update(PROTOCOL_MAGIC);
    mac.update(exporter);
    mac.update(role);
    Ok(mac.finalize().into_bytes().into())
}

fn read_and_verify_proof<R: Read>(
    reader: &mut R,
    code: &str,
    exporter: &[u8],
    role: &[u8],
) -> Result<(), String> {
    let mut magic = [0u8; PROTOCOL_MAGIC.len()];
    let mut proof = [0u8; PROOF_BYTES];
    reader
        .read_exact(&mut magic)
        .and_then(|_| reader.read_exact(&mut proof))
        .map_err(|err| format!("Failed to read library pairing proof: {err}"))?;
    if &magic != PROTOCOL_MAGIC {
        return Err("The other device is not using a compatible Papercut transfer".into());
    }
    let mut mac = HmacSha256::new_from_slice(code.as_bytes())
        .map_err(|_| "Invalid library transfer code".to_string())?;
    mac.update(PROTOCOL_MAGIC);
    mac.update(exporter);
    mac.update(role);
    mac.verify_slice(&proof)
        .map_err(|_| "Pairing code did not match the source device".to_string())
}

fn tls_exporter<C>(connection: &C) -> Result<[u8; PROOF_BYTES], String>
where
    C: ExportKeyingMaterial,
{
    connection
        .export([0u8; PROOF_BYTES], TLS_EXPORTER_LABEL)
        .map_err(|err| format!("Failed to authenticate secure library transfer: {err}"))
}

trait ExportKeyingMaterial {
    fn export<const N: usize>(&self, output: [u8; N], label: &[u8]) -> Result<[u8; N], TlsError>;
}

impl ExportKeyingMaterial for ServerConnection {
    fn export<const N: usize>(&self, output: [u8; N], label: &[u8]) -> Result<[u8; N], TlsError> {
        self.export_keying_material(output, label, None)
    }
}

impl ExportKeyingMaterial for ClientConnection {
    fn export<const N: usize>(&self, output: [u8; N], label: &[u8]) -> Result<[u8; N], TlsError> {
        self.export_keying_material(output, label, None)
    }
}

fn server_tls_config() -> Result<Arc<ServerConfig>, String> {
    let certified = generate_simple_self_signed(["papercut.local".to_string()])
        .map_err(|err| format!("Failed to create secure transfer certificate: {err}"))?;
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(certified.key_pair.serialize_der()));
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|err| format!("Failed to configure secure library transfer: {err}"))?
        .with_no_client_auth()
        .with_single_cert(vec![certified.cert.der().clone()], key)
        .map_err(|err| format!("Failed to configure secure transfer certificate: {err}"))?;
    Ok(Arc::new(config))
}

fn client_tls_config() -> Result<Arc<ClientConfig>, String> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let verifier = Arc::new(PairingCertificateVerifier {
        supported: provider.signature_verification_algorithms,
    });
    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|err| format!("Failed to configure secure library transfer: {err}"))?
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();
    Ok(Arc::new(config))
}

/// Certificate-chain trust is intentionally deferred to the channel-bound
/// one-use pairing proof; handshake signatures are still cryptographically
/// verified so an invalid or corrupted ephemeral certificate is rejected.
#[derive(Debug)]
struct PairingCertificateVerifier {
    supported: WebPkiSupportedAlgorithms,
}

impl ServerCertVerifier for PairingCertificateVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        verify_tls12_signature(message, cert, dss, &self.supported)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        verify_tls13_signature(message, cert, dss, &self.supported)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.supported.supported_schemes()
    }
}

fn configure_socket(socket: &TcpStream) -> Result<(), String> {
    socket
        .set_read_timeout(Some(SOCKET_TIMEOUT))
        .and_then(|_| socket.set_write_timeout(Some(SOCKET_TIMEOUT)))
        .map_err(|err| format!("Failed to configure local library connection: {err}"))
}

/// Ask the routing table which IPv4 interface would carry a packet. UDP
/// `connect` does not send traffic, so address discovery remains offline.
fn local_ipv4() -> Result<Ipv4Addr, String> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))
        .map_err(|err| format!("Failed to inspect local network: {err}"))?;
    socket
        .connect((Ipv4Addr::new(192, 0, 2, 1), 9))
        .map_err(|err| format!("Connect both devices to the same local network: {err}"))?;
    match socket
        .local_addr()
        .map_err(|err| format!("Failed to inspect local network address: {err}"))?
        .ip()
    {
        std::net::IpAddr::V4(ip) if !ip.is_unspecified() && !ip.is_loopback() => Ok(ip),
        _ => Err("Could not find this device's local IPv4 address".into()),
    }
}

fn parse_local_address(value: &str) -> Result<SocketAddrV4, String> {
    let address = value
        .trim()
        .parse::<SocketAddrV4>()
        .map_err(|_| "Enter the source address exactly as shown on the other device".to_string())?;
    let ip = address.ip();
    if !(ip.is_private() || ip.is_link_local() || ip.is_loopback()) {
        return Err("The source address must be on the local network".into());
    }
    Ok(address)
}

fn new_pairing_code() -> Result<String, String> {
    let mut random = [0u8; CODE_CHARS];
    getrandom::fill(&mut random)
        .map_err(|err| format!("Failed to create a secure pairing code: {err}"))?;
    Ok(random
        .into_iter()
        .map(|value| CODE_ALPHABET[(value & 31) as usize] as char)
        .collect())
}

fn normalize_code(value: &str) -> Result<String, String> {
    let code: String = value
        .chars()
        .filter(|character| !character.is_ascii_whitespace() && *character != '-')
        .flat_map(char::to_uppercase)
        .collect();
    if code.len() != CODE_CHARS
        || !code
            .bytes()
            .all(|character| CODE_ALPHABET.contains(&character))
    {
        return Err("Enter the 8-character pairing code shown on the source device".into());
    }
    Ok(code)
}

fn display_code(code: &str) -> String {
    format!("{}-{}", &code[..4], &code[4..])
}

fn set_send_status(
    status: &Mutex<LibraryTransferSendStatus>,
    state: LibraryTransferSendState,
    error: Option<String>,
) {
    if let Ok(mut status) = status.lock() {
        status.state = state;
        status.error = error;
    }
}

fn lock<T>(mutex: &Mutex<T>) -> Result<std::sync::MutexGuard<'_, T>, String> {
    mutex
        .lock()
        .map_err(|_| "Library transfer state is unavailable".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn pairing_proof_is_bound_to_channel_and_role() {
        let code = "2345ABCD";
        let first = pairing_proof(code, b"first channel", SENDER_ROLE).unwrap();

        assert_eq!(
            first,
            pairing_proof(code, b"first channel", SENDER_ROLE).unwrap()
        );
        assert_ne!(
            first,
            pairing_proof(code, b"other channel", SENDER_ROLE).unwrap()
        );
        assert_ne!(
            first,
            pairing_proof(code, b"first channel", RECEIVER_ROLE).unwrap()
        );
        assert_eq!(normalize_code("2345-abcd").unwrap(), code);
    }

    #[test]
    fn tls_round_trip_streams_the_exact_package() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = match listener.local_addr().unwrap() {
            SocketAddr::V4(address) => address,
            _ => unreachable!(),
        };
        let package_path = std::env::temp_dir().join(format!(
            "papercut-lan-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let package = b"test papercut library package";
        fs::write(&package_path, package).unwrap();
        let sender_path = package_path.clone();
        let sender = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let status = Arc::new(Mutex::new(LibraryTransferSendStatus {
                state: LibraryTransferSendState::Sending,
                address: address.to_string(),
                code: "2345-ABCD".into(),
                documents: 1,
                audiobooks: 0,
                package_bytes: package.len() as u64,
                bytes_transferred: 0,
                error: None,
            }));
            send_package(
                stream,
                server_tls_config().unwrap(),
                "2345ABCD",
                &sender_path,
                &status,
                &AtomicBool::new(false),
            )
        });

        let mut received = Vec::new();
        receive_package(
            address,
            "2345ABCD",
            package_path.parent().unwrap(),
            &mut received,
            |_, _| {},
        )
        .unwrap();
        sender.join().unwrap().unwrap();
        let _ = fs::remove_file(package_path);

        assert_eq!(received, package);
    }
}
