//! Ephemeral TLS and one-use pairing for local library transfer.

use std::io::{self, Read};
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::sync::Arc;

use hmac::{Hmac, Mac};
use rcgen::generate_simple_self_signed;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{verify_tls12_signature, verify_tls13_signature, WebPkiSupportedAlgorithms};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::{
    ClientConfig, ClientConnection, DigitallySignedStruct, Error as TlsError, ServerConfig,
    ServerConnection, SignatureScheme,
};

pub(super) const PROTOCOL_MAGIC: &[u8; 8] = b"PCLAN002";
pub(super) const RECEIVER_ROLE: &[u8] = b"receiver";
pub(super) const SENDER_ROLE: &[u8] = b"sender";

const TLS_EXPORTER_LABEL: &[u8] = b"papercut-library-transfer-v1";
const PROOF_BYTES: usize = 32;
const CODE_ALPHABET: &[u8; 32] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";
const CODE_CHARS: usize = 12;

type HmacSha256 = Hmac<sha2::Sha256>;

/// Separate unrelated/incomplete traffic from a complete wrong-code proof so
/// only the latter consumes the source's one-use pairing session.
#[derive(Debug)]
pub(super) enum PairingProofError {
    Unauthenticated(String),
    IncorrectCode,
}

impl std::fmt::Display for PairingProofError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unauthenticated(message) => formatter.write_str(message),
            Self::IncorrectCode => {
                formatter.write_str("Pairing code did not match the source device")
            }
        }
    }
}

/// Bind knowledge of the displayed code to this exact TLS channel and peer
/// role, preventing a proof captured from one connection from being replayed.
pub(super) fn pairing_proof(
    code: &str,
    exporter: &[u8],
    role: &[u8],
) -> io::Result<[u8; PROOF_BYTES]> {
    let mut mac = HmacSha256::new_from_slice(code.as_bytes())
        .map_err(|_| io::Error::other("Invalid library transfer code"))?;
    mac.update(PROTOCOL_MAGIC);
    mac.update(exporter);
    mac.update(role);
    Ok(mac.finalize().into_bytes().into())
}

/// Reject peers that do not present both the Papercut protocol marker and the
/// role-specific HMAC expected for this TLS session.
pub(super) fn read_and_verify_proof<R: Read>(
    reader: &mut R,
    code: &str,
    exporter: &[u8],
    role: &[u8],
) -> Result<(), PairingProofError> {
    let mut magic = [0u8; PROTOCOL_MAGIC.len()];
    let mut proof = [0u8; PROOF_BYTES];
    reader
        .read_exact(&mut magic)
        .and_then(|_| reader.read_exact(&mut proof))
        .map_err(|err| {
            PairingProofError::Unauthenticated(format!(
                "Failed to read library pairing proof: {err}"
            ))
        })?;
    if &magic != PROTOCOL_MAGIC {
        return Err(PairingProofError::Unauthenticated(
            "The other device is not using a compatible Papercut transfer".into(),
        ));
    }
    let mut mac = HmacSha256::new_from_slice(code.as_bytes())
        .map_err(|_| PairingProofError::Unauthenticated("Invalid library transfer code".into()))?;
    mac.update(PROTOCOL_MAGIC);
    mac.update(exporter);
    mac.update(role);
    mac.verify_slice(&proof)
        .map_err(|_| PairingProofError::IncorrectCode)
}

/// Derive shared channel-specific bytes from the completed TLS handshake for
/// the pairing proof without exposing TLS keys.
pub(super) fn tls_exporter<C>(connection: &C) -> Result<[u8; PROOF_BYTES], String>
where
    C: ExportKeyingMaterial,
{
    connection
        .export([0u8; PROOF_BYTES], TLS_EXPORTER_LABEL)
        .map_err(|err| format!("Failed to authenticate secure library transfer: {err}"))
}

pub(super) trait ExportKeyingMaterial {
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

/// Create a fresh self-signed certificate for one foreground send session; the
/// pairing proof, rather than a persistent certificate, authenticates the peer.
pub(super) fn server_tls_config() -> Result<Arc<ServerConfig>, String> {
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

/// Accept the source's ephemeral certificate chain while retaining TLS signature
/// verification; the following channel-bound pairing proof establishes trust.
pub(super) fn client_tls_config() -> Result<Arc<ClientConfig>, String> {
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

/// Ask the routing table which IPv4 interface would carry a packet. UDP
/// `connect` does not send traffic, so address discovery remains offline.
pub(super) fn local_ipv4() -> Result<Ipv4Addr, String> {
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
        std::net::IpAddr::V4(ip) if is_local_network_ipv4(ip) => Ok(ip),
        _ => Err("Could not find this device's local IPv4 address".into()),
    }
}

/// Keep source advertisement and receiver validation on the same deliberately
/// narrow address policy so Papercut never displays an address it later rejects.
fn is_local_network_ipv4(ip: Ipv4Addr) -> bool {
    ip.is_private() || ip.is_link_local() || ip.is_loopback()
}

pub(super) fn parse_local_address(value: &str) -> Result<SocketAddrV4, String> {
    let address = value
        .trim()
        .parse::<SocketAddrV4>()
        .map_err(|_| "Enter the source address exactly as shown on the other device".to_string())?;
    let ip = address.ip();
    if !is_local_network_ipv4(*ip) {
        return Err("The source address must be on the local network".into());
    }
    Ok(address)
}

/// Generate 12 independent base-32 symbols for 60 bits of pairing entropy.
/// Masking is unbiased because a byte's 256 values divide evenly across 32 symbols.
pub(super) fn new_pairing_code() -> Result<String, String> {
    let mut random = [0u8; CODE_CHARS];
    getrandom::fill(&mut random)
        .map_err(|err| format!("Failed to create a secure pairing code: {err}"))?;
    Ok(random
        .into_iter()
        .map(|value| CODE_ALPHABET[(value & 31) as usize] as char)
        .collect())
}

/// Accept pasted grouped or ungrouped codes while requiring the complete
/// 12-symbol alphabet before any network connection is attempted.
pub(super) fn normalize_code(value: &str) -> Result<String, String> {
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
        return Err("Enter the 12-character pairing code shown on the source device".into());
    }
    Ok(code)
}

pub(super) fn display_code(code: &str) -> String {
    format!("{}-{}-{}", &code[..4], &code[4..8], &code[8..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairing_proof_is_bound_to_channel_and_role() {
        let code = "2345ABCDGHJK";
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
        let mut incompatible = [0u8; PROTOCOL_MAGIC.len() + PROOF_BYTES].as_slice();
        assert!(matches!(
            read_and_verify_proof(&mut incompatible, code, b"first channel", RECEIVER_ROLE),
            Err(PairingProofError::Unauthenticated(_))
        ));
        let mut incorrect = Vec::from(PROTOCOL_MAGIC.as_slice());
        incorrect.extend_from_slice(
            &pairing_proof("other-code", b"first channel", RECEIVER_ROLE).unwrap(),
        );
        assert!(matches!(
            read_and_verify_proof(
                &mut incorrect.as_slice(),
                code,
                b"first channel",
                RECEIVER_ROLE,
            ),
            Err(PairingProofError::IncorrectCode)
        ));
        assert_eq!(normalize_code("2345-abcd-ghjk").unwrap(), code);
        assert_eq!(display_code(code), "2345-ABCD-GHJK");
        let generated = new_pairing_code().unwrap();
        assert_eq!(generated.len(), CODE_CHARS);
        assert!(generated.bytes().all(|byte| CODE_ALPHABET.contains(&byte)));
        assert!(normalize_code("2345-abcd").is_err());
    }

    #[test]
    fn source_and_receiver_share_the_local_address_policy() {
        assert!(is_local_network_ipv4(Ipv4Addr::new(192, 168, 1, 20)));
        assert!(is_local_network_ipv4(Ipv4Addr::new(169, 254, 1, 20)));
        assert!(is_local_network_ipv4(Ipv4Addr::LOCALHOST));
        assert!(!is_local_network_ipv4(Ipv4Addr::new(203, 0, 113, 20)));
        assert!(!is_local_network_ipv4(Ipv4Addr::UNSPECIFIED));
    }
}
