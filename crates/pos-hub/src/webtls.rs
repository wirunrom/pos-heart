//! Certificates for the iPad web app. Safari only trusts a root certificate
//! the iPad has installed (configuration profile) and checks the server's
//! IP/name, so the hub keeps a small shop CA and signs a short leaf for its
//! current LAN addresses on every start. The CA key never leaves this
//! computer; desktop devices keep using the pinned identity in `tls`.
use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, ExtendedKeyUsagePurpose, IsCa,
    KeyPair, KeyUsagePurpose, SanType,
};
use rustls::{
    ServerConfig,
    crypto::ring,
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
};
use std::{net::IpAddr, path::Path, sync::Arc};

const CA_CERT: &str = "web-ca.der";
const CA_KEY: &str = "web-ca-key.der";

/// The shop CA. Its parameters are fixed, so the certificate rebuilt from
/// the stored key has the same subject and key as the installed one.
fn ca_params() -> Result<CertificateParams, rcgen::Error> {
    let mut params = CertificateParams::new(Vec::<String>::new())?;
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, "POS Heart shop CA");
    dn.push(DnType::OrganizationName, "POS Heart");
    params.distinguished_name = dn;
    params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
    params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
    params.not_before = rcgen::date_time_ymd(2024, 1, 1);
    params.not_after = rcgen::date_time_ymd(2044, 1, 1);
    Ok(params)
}

pub struct WebIdentity {
    ca_cert: Vec<u8>,
    ca_key: Vec<u8>,
}
impl WebIdentity {
    /// Loads the shop CA from `dir`, creating it on first use.
    pub fn load_or_create(dir: &Path) -> std::io::Result<Self> {
        if let (Ok(ca_cert), Ok(ca_key)) = (
            std::fs::read(dir.join(CA_CERT)),
            std::fs::read(dir.join(CA_KEY)),
        ) {
            return Ok(Self { ca_cert, ca_key });
        }
        let identity = Self::generate()?;
        std::fs::write(dir.join(CA_KEY), &identity.ca_key)?;
        std::fs::write(dir.join(CA_CERT), &identity.ca_cert)?;
        Ok(identity)
    }
    pub fn generate() -> std::io::Result<Self> {
        let key = KeyPair::generate().map_err(std::io::Error::other)?;
        let cert = ca_params()
            .and_then(|p| p.self_signed(&key))
            .map_err(std::io::Error::other)?;
        Ok(Self {
            ca_cert: cert.der().to_vec(),
            ca_key: key.serialize_der(),
        })
    }
    /// The CA certificate (DER) the iPad installs.
    pub fn ca_der(&self) -> &[u8] {
        &self.ca_cert
    }
    /// A server config for this computer's addresses (loopback is always
    /// included), with a fresh leaf valid for 800 days (Apple's limit is 825).
    pub fn server_config(&self, ips: &[IpAddr]) -> std::io::Result<Arc<ServerConfig>> {
        let ca_key = KeyPair::try_from(&PrivatePkcs8KeyDer::from(self.ca_key.clone()))
            .map_err(std::io::Error::other)?;
        let ca = ca_params()
            .and_then(|p| p.self_signed(&ca_key))
            .map_err(std::io::Error::other)?;
        let mut params =
            CertificateParams::new(vec!["posheart.local".to_owned(), "localhost".to_owned()])
                .map_err(std::io::Error::other)?;
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, "POS Heart main register");
        params.distinguished_name = dn;
        let mut addresses: Vec<IpAddr> = ips.to_vec();
        addresses.push(IpAddr::from([127, 0, 0, 1]));
        addresses.dedup();
        params
            .subject_alt_names
            .extend(addresses.into_iter().map(SanType::IpAddress));
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        let now = time::OffsetDateTime::now_utc();
        params.not_before = now - time::Duration::days(1);
        params.not_after = now + time::Duration::days(800);
        let leaf_key = KeyPair::generate().map_err(std::io::Error::other)?;
        let leaf = params
            .signed_by(&leaf_key, &ca, &ca_key)
            .map_err(std::io::Error::other)?;
        let config = ServerConfig::builder_with_provider(Arc::new(ring::default_provider()))
            .with_safe_default_protocol_versions()
            .map_err(std::io::Error::other)?
            .with_no_client_auth()
            .with_single_cert(
                vec![
                    CertificateDer::from(leaf.der().to_vec()),
                    CertificateDer::from(self.ca_cert.clone()),
                ],
                PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(leaf_key.serialize_der())),
            )
            .map_err(std::io::Error::other)?;
        Ok(Arc::new(config))
    }
    /// An Apple configuration profile that installs the shop CA.
    pub fn mobileconfig(&self, shop: &str) -> String {
        let cert = base64(&self.ca_cert);
        let shop = xml_escape(shop);
        let uuid1 = uuid::Uuid::new_v4().to_string().to_uppercase();
        let uuid2 = uuid::Uuid::new_v4().to_string().to_uppercase();
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>PayloadContent</key>
  <array>
    <dict>
      <key>PayloadCertificateFileName</key><string>pos-heart-ca.cer</string>
      <key>PayloadContent</key><data>{cert}</data>
      <key>PayloadDescription</key><string>POS Heart shop certificate</string>
      <key>PayloadDisplayName</key><string>POS Heart ({shop})</string>
      <key>PayloadIdentifier</key><string>app.posheart.ca.cert</string>
      <key>PayloadType</key><string>com.apple.security.root</string>
      <key>PayloadUUID</key><string>{uuid1}</string>
      <key>PayloadVersion</key><integer>1</integer>
    </dict>
  </array>
  <key>PayloadDisplayName</key><string>POS Heart ({shop})</string>
  <key>PayloadIdentifier</key><string>app.posheart.ca</string>
  <key>PayloadType</key><string>Configuration</string>
  <key>PayloadUUID</key><string>{uuid2}</string>
  <key>PayloadVersion</key><integer>1</integer>
</dict>
</plist>
"#
        )
    }
}
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
/// Standard base64 (for the profile's certificate data).
pub fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}
