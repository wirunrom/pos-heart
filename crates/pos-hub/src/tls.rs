//! TLS for the shop LAN. The hub has its own self-signed certificate (kept
//! beside the database as a device file). A device learns the certificate's
//! SHA-256 fingerprint when it pairs and afterwards accepts only that
//! certificate (pinning), so no public CA or internet is involved.
use rustls::{
    ClientConfig, DigitallySignedStruct, ServerConfig, SignatureScheme,
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    crypto::{WebPkiSupportedAlgorithms, ring},
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime},
};
use sha2::{Digest, Sha256};
use std::{path::Path, sync::Arc};

const CERT_FILE: &str = "hub-cert.der";
const KEY_FILE: &str = "hub-key.der";

/// The hub's certificate and private key (DER).
#[derive(Clone)]
pub struct Identity {
    cert: Vec<u8>,
    key: Vec<u8>,
}
impl Identity {
    /// Loads the hub identity from `dir`, creating it on first use.
    pub fn load_or_create(dir: &Path) -> std::io::Result<Self> {
        let cert_path = dir.join(CERT_FILE);
        let key_path = dir.join(KEY_FILE);
        if let (Ok(cert), Ok(key)) = (std::fs::read(&cert_path), std::fs::read(&key_path)) {
            return Ok(Self { cert, key });
        }
        let identity = Self::generate()?;
        std::fs::write(&key_path, &identity.key)?;
        std::fs::write(&cert_path, &identity.cert)?;
        Ok(identity)
    }
    /// A new self-signed identity (also used by tests).
    pub fn generate() -> std::io::Result<Self> {
        let made = rcgen::generate_simple_self_signed(vec!["pos-heart-hub".to_owned()])
            .map_err(std::io::Error::other)?;
        Ok(Self {
            cert: made.cert.der().to_vec(),
            key: made.key_pair.serialize_der(),
        })
    }
    /// SHA-256 of the certificate: what devices pin.
    pub fn fingerprint(&self) -> [u8; 32] {
        fingerprint(&self.cert)
    }
    pub fn server_config(&self) -> std::io::Result<Arc<ServerConfig>> {
        let config = ServerConfig::builder_with_provider(Arc::new(ring::default_provider()))
            .with_safe_default_protocol_versions()
            .map_err(std::io::Error::other)?
            .with_no_client_auth()
            .with_single_cert(
                vec![CertificateDer::from(self.cert.clone())],
                PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(self.key.clone())),
            )
            .map_err(std::io::Error::other)?;
        Ok(Arc::new(config))
    }
}
pub fn fingerprint(cert: &[u8]) -> [u8; 32] {
    Sha256::digest(cert).into()
}
/// Short form people compare on both screens when pairing.
pub fn short(fingerprint: &[u8; 32]) -> String {
    fingerprint[..4]
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Accepts exactly the pinned hub certificate, or (while pairing, `pin`
/// None) any certificate whose fingerprint the caller then stores.
#[derive(Debug)]
struct Pinned {
    pin: Option<[u8; 32]>,
    algorithms: WebPkiSupportedAlgorithms,
}
impl ServerCertVerifier for Pinned {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        match self.pin {
            Some(pin) if fingerprint(end_entity) != pin => Err(rustls::Error::General(
                "hub certificate changed; pair again".into(),
            )),
            _ => Ok(ServerCertVerified::assertion()),
        }
    }
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.algorithms)
    }
    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.algorithms)
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algorithms.supported_schemes()
    }
}
pub fn client_config(pin: Option<[u8; 32]>) -> std::io::Result<Arc<ClientConfig>> {
    let provider = Arc::new(ring::default_provider());
    let algorithms = provider.signature_verification_algorithms;
    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(std::io::Error::other)?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(Pinned { pin, algorithms }))
        .with_no_client_auth();
    Ok(Arc::new(config))
}
pub fn server_name() -> ServerName<'static> {
    ServerName::try_from("pos-heart-hub").unwrap_or(ServerName::IpAddress(
        std::net::IpAddr::from([127, 0, 0, 1]).into(),
    ))
}
