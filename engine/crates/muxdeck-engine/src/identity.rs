//! Host identity: Ed25519 keypair, self-signed TLS certificate, and the local admin token.
//!
//! Generated on first run, loaded thereafter. `docs/ARCHITECTURE.md` §5.1.

use std::net::IpAddr;
use std::path::Path;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use ed25519_dalek::{SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};

use crate::config::Paths;
use crate::error::{EngineError, Result};
use crate::secret_file::{read_secret, write_secret};

/// How long the self-signed certificate is valid. `docs/ARCHITECTURE.md` §5.1.
const CERT_VALIDITY_DAYS: i64 = 3650;

/// Length of the local admin token, in bytes, before base64.
const ADMIN_TOKEN_LEN: usize = 32;

/// Everything that identifies this host, loaded once at startup.
pub struct Identity {
    signing_key: SigningKey,
    host_id: String,
    fingerprint: String,
    cert_pem: String,
    key_pem: String,
    admin_token: String,
}

impl Identity {
    /// Loads the host identity, generating it on first run.
    ///
    /// Generation is all-or-nothing in practice: if any piece is missing the whole set is
    /// regenerated, because a key that does not match its certificate is worse than a fresh
    /// pair. Note that regenerating unpairs every device, which is why this only happens when
    /// there is nothing to lose — a first run, or an explicit `--reset-identity`.
    pub fn load_or_generate(paths: &Paths) -> Result<Self> {
        let have_all = paths.identity_key().exists()
            && paths.tls_cert().exists()
            && paths.tls_key().exists()
            && paths.admin_token().exists();

        if have_all {
            Self::load(paths)
        } else {
            Self::generate(paths)
        }
    }

    fn load(paths: &Paths) -> Result<Self> {
        let key_bytes = read_secret(&paths.identity_key())?;
        let key_bytes: [u8; 32] =
            key_bytes
                .as_slice()
                .try_into()
                .map_err(|_| EngineError::CorruptIdentity {
                    what: "host identity key",
                })?;
        let signing_key = SigningKey::from_bytes(&key_bytes);

        let cert_pem = read_string(&paths.tls_cert(), "reading the TLS certificate")?;
        let key_pem = read_string(&paths.tls_key(), "reading the TLS private key")?;
        let admin_token = read_string(&paths.admin_token(), "reading the admin token")?
            .trim()
            .to_string();

        Ok(Self {
            host_id: id_from_pubkey("h_", &signing_key.verifying_key()),
            fingerprint: fingerprint_from_pem(&cert_pem)?,
            signing_key,
            cert_pem,
            key_pem,
            admin_token,
        })
    }

    fn generate(paths: &Paths) -> Result<Self> {
        let signing_key = SigningKey::from_bytes(&random_bytes::<32>()?);
        let (cert_pem, key_pem) = generate_certificate()?;
        let admin_token = BASE64.encode(random_bytes::<ADMIN_TOKEN_LEN>()?);

        write_secret(&paths.identity_key(), signing_key.as_bytes())?;
        write_secret(&paths.tls_key(), key_pem.as_bytes())?;
        write_secret(&paths.admin_token(), admin_token.as_bytes())?;

        // The certificate itself is public — it is sent to every client on every connection —
        // so it does not need owner-only permissions.
        std::fs::write(paths.tls_cert(), &cert_pem)
            .map_err(|e| EngineError::io("writing the TLS certificate", paths.tls_cert(), e))?;

        Ok(Self {
            host_id: id_from_pubkey("h_", &signing_key.verifying_key()),
            fingerprint: fingerprint_from_pem(&cert_pem)?,
            signing_key,
            cert_pem,
            key_pem,
            admin_token,
        })
    }

    /// `"h_"` followed by 16 lowercase hex characters. `docs/PROTOCOL.md` §2.2.
    pub fn host_id(&self) -> &str {
        &self.host_id
    }

    /// Lowercase hex, no separators, SHA-256 over the leaf certificate DER. 64 characters.
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    pub fn cert_pem(&self) -> &str {
        &self.cert_pem
    }

    pub fn key_pem(&self) -> &str {
        &self.key_pem
    }

    /// Never log this. `docs/ENGINE.md` §6.
    pub fn admin_token(&self) -> &str {
        &self.admin_token
    }

    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }
}

/// `prefix` + the first 16 hex characters of SHA-256 over the raw public key bytes.
///
/// The same rule produces `host_id` and `device_id`, which is why it lives in one function —
/// `docs/PROTOCOL.md` §2.2 insists there is exactly one representation of each.
pub fn id_from_pubkey(prefix: &str, key: &VerifyingKey) -> String {
    id_from_pubkey_bytes(prefix, key.as_bytes())
}

pub fn id_from_pubkey_bytes(prefix: &str, bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(prefix.len() + 16);
    out.push_str(prefix);
    for byte in &digest[..8] {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Reads `n` bytes from the OS entropy source.
pub fn random_bytes<const N: usize>() -> Result<[u8; N]> {
    let mut buf = [0u8; N];
    getrandom::fill(&mut buf).map_err(|e| EngineError::Entropy(e.to_string()))?;
    Ok(buf)
}

/// A six-digit one-time pairing code, uniformly distributed.
///
/// Rejection sampling rather than `% 1_000_000`: the modulo of a `u32` is very slightly biased
/// towards low codes, and biasing a one-time secret is not a corner worth cutting.
pub fn generate_otp() -> Result<String> {
    const LIMIT: u32 = 1_000_000;
    const CUTOFF: u32 = u32::MAX - (u32::MAX % LIMIT) - 1;

    loop {
        let value = u32::from_le_bytes(random_bytes::<4>()?);
        if value <= CUTOFF {
            return Ok(format!("{:06}", value % LIMIT));
        }
    }
}

fn read_string(path: &Path, context: &'static str) -> Result<String> {
    std::fs::read_to_string(path).map_err(|e| EngineError::io(context, path, e))
}

/// SHA-256 over the leaf certificate DER, lowercase hex.
///
/// Over the **DER**, not the PEM — the PEM is base64 with line breaks and a header, and hashing
/// that would produce a value nothing else in the system agrees with.
fn fingerprint_from_pem(pem: &str) -> Result<String> {
    let der = der_from_pem(pem)?;
    Ok(hex_lower(&Sha256::digest(&der)))
}

fn der_from_pem(pem: &str) -> Result<Vec<u8>> {
    let body: String = pem
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect::<Vec<_>>()
        .join("");
    BASE64
        .decode(body.trim())
        .map_err(|e| EngineError::Certificate(format!("certificate is not valid PEM: {e}")))
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Generates the self-signed certificate, with SANs for loopback and every local address.
///
/// The SANs are for tooling convenience only — no client validates them, and this certificate is
/// never regenerated, so a stale address list is harmless by design (`docs/ARCHITECTURE.md`
/// §5.1).
fn generate_certificate() -> Result<(String, String)> {
    use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair, SanType};

    let mut san = vec![
        SanType::DnsName("localhost".try_into().map_err(cert_err)?),
        SanType::IpAddress("127.0.0.1".parse().expect("literal is a valid address")),
        SanType::IpAddress("::1".parse().expect("literal is a valid address")),
    ];
    for ip in local_addresses() {
        san.push(SanType::IpAddress(ip));
    }

    let mut params = CertificateParams::default();
    params.subject_alt_names = san;

    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, "MuxDeck Host");
    dn.push(DnType::OrganizationName, "in.redoimagined");
    params.distinguished_name = dn;

    let now = time::OffsetDateTime::now_utc();
    params.not_before = now - time::Duration::days(1);
    params.not_after = now + time::Duration::days(CERT_VALIDITY_DAYS);

    let key_pair = KeyPair::generate().map_err(cert_err)?;
    let cert = params.self_signed(&key_pair).map_err(cert_err)?;

    Ok((cert.pem(), key_pair.serialize_pem()))
}

fn cert_err(e: impl std::fmt::Display) -> EngineError {
    EngineError::Certificate(e.to_string())
}

/// Every non-loopback address on this machine.
fn local_addresses() -> Vec<IpAddr> {
    if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter(|iface| !iface.is_loopback())
        .map(|iface| iface.ip())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_root(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir()
            .join("muxdeck-identity-tests")
            .join(name);
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn generate_then_load_is_stable() {
        let root = temp_root("stable");
        let paths = Paths::resolve(Some(root.clone())).expect("resolve");

        let first = Identity::load_or_generate(&paths).expect("generate");
        let second = Identity::load_or_generate(&paths).expect("load");

        assert_eq!(first.host_id(), second.host_id());
        assert_eq!(first.fingerprint(), second.fingerprint());
        assert_eq!(first.admin_token(), second.admin_token());
        assert_eq!(first.cert_pem(), second.cert_pem());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn ids_and_fingerprints_have_the_documented_shape() {
        let root = temp_root("shape");
        let paths = Paths::resolve(Some(root.clone())).expect("resolve");
        let identity = Identity::load_or_generate(&paths).expect("generate");

        let host_id = identity.host_id();
        assert_eq!(host_id.len(), 18, "host_id is `h_` plus 16 hex characters");
        assert!(host_id.starts_with("h_"));
        assert!(host_id[2..]
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()));

        let fp = identity.fingerprint();
        assert_eq!(fp.len(), 64, "a SHA-256 in hex is 64 characters");
        assert!(fp
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn otp_is_always_six_digits() {
        for _ in 0..256 {
            let otp = generate_otp().expect("otp");
            assert_eq!(otp.len(), 6);
            assert!(otp.chars().all(|c| c.is_ascii_digit()));
        }
    }

    #[test]
    fn id_derivation_matches_the_spec_example_shape() {
        let id = id_from_pubkey_bytes("d_", &[0u8; 32]);
        assert_eq!(id.len(), 18);
        assert!(id.starts_with("d_"));
    }
}
