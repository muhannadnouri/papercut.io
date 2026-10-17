//! Ephemeral TLS 1.3 and random one-use codes for local library transfer.
use rcgen::generate_simple_self_signed;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{verify_tls12_signature, verify_tls13_signature, WebPkiSupportedAlgorithms};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::{
    ClientConfig, DigitallySignedStruct, Error as TlsError, ServerConfig, SignatureScheme,
};
use std::sync::Arc;
const CODE_ALPHABET: &[u8; 32] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";
const CODE_CHARS: usize = 12;
const TLS13_ONLY: &[&rustls::SupportedProtocolVersion] = &[&rustls::version::TLS13];

pub(super) fn server_tls_config() -> Result<Arc<ServerConfig>, String> {
    let certified = generate_simple_self_signed(["papercut.local".to_string()])
        .map_err(|err| format!("Failed to create secure transfer certificate: {err}"))?;
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(certified.key_pair.serialize_der()));
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut config = ServerConfig::builder_with_provider(provider)
        .with_protocol_versions(TLS13_ONLY)
        .map_err(|err| format!("Failed to configure secure library transfer: {err}"))?
        .with_no_client_auth()
        .with_single_cert(vec![certified.cert.der().clone()], key)
        .map_err(|err| format!("Failed to configure secure transfer certificate: {err}"))?;
    config.send_tls13_tickets = 0;
    config.session_storage = Arc::new(rustls::server::NoServerSessionStorage {});
    config.max_early_data_size = 0;
    Ok(Arc::new(config))
}

/// Accept the listener's ephemeral certificate chain while retaining TLS signature
/// verification; the following channel-bound PAKE confirmation establishes trust.
pub(super) fn client_tls_config() -> Result<Arc<ClientConfig>, String> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let verifier = Arc::new(PairingCertificateVerifier {
        supported: provider.signature_verification_algorithms,
    });
    let mut config = ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(TLS13_ONLY)
        .map_err(|err| format!("Failed to configure secure library transfer: {err}"))?
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();
    config.resumption = rustls::client::Resumption::disabled();
    config.enable_early_data = false;
    Ok(Arc::new(config))
}

/// Certificate-chain trust is intentionally deferred to the channel-bound
/// one-use PAKE confirmation; handshake signatures are still cryptographically
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
        return Err("Enter the 12-character pairing code shown on the listening device".into());
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
    fn pairing_codes_keep_the_full_random_alphabet_and_normalization_contract() {
        assert_eq!(normalize_code("2345-abcd-ghjk").unwrap(), "2345ABCDGHJK");
        assert_eq!(display_code("2345ABCDGHJK"), "2345-ABCD-GHJK");
        let generated = new_pairing_code().unwrap();
        assert_eq!(generated.len(), CODE_CHARS);
        assert!(generated.bytes().all(|byte| CODE_ALPHABET.contains(&byte)));
        for invalid in ["2345-abcd", "0000-1111-OOOO", "2345ABCDGHJKM"] {
            assert!(normalize_code(invalid).is_err());
        }
    }
}
