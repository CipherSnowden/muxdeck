//! The paired-device registry, persisted as JSON. `docs/ENGINE.md` §5.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use ed25519_dalek::VerifyingKey;
use muxdeck_core::{DeviceInfo, ErrorCode, Platform};
use serde::{Deserialize, Serialize};

use crate::config::{read_json_or, write_json};
use crate::error::{EngineError, Result};
use crate::identity::id_from_pubkey_bytes;

/// What is persisted per device.
///
/// Note the public key is stored and the `connected` flag is not: connectivity is live state
/// belonging to the server, and persisting it would leave every device looking connected after
/// a crash.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredDevice {
    pub device_id: String,
    pub name: String,
    pub platform: Platform,
    /// Ed25519 public key, 32 bytes, base64.
    pub pubkey: String,
    pub paired_at: i64,
    pub last_seen: i64,
}

impl StoredDevice {
    /// The key this device authenticates with.
    pub fn verifying_key(&self) -> Result<VerifyingKey> {
        let bytes = BASE64
            .decode(&self.pubkey)
            .map_err(|_| EngineError::CorruptIdentity {
                what: "device public key",
            })?;
        let bytes: [u8; 32] =
            bytes
                .as_slice()
                .try_into()
                .map_err(|_| EngineError::CorruptIdentity {
                    what: "device public key",
                })?;
        VerifyingKey::from_bytes(&bytes).map_err(|_| EngineError::CorruptIdentity {
            what: "device public key",
        })
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct RegistryFile {
    devices: BTreeMap<String, StoredDevice>,
}

/// The set of devices allowed to authenticate.
pub struct Registry {
    path: PathBuf,
    file: RegistryFile,
}

impl Registry {
    pub fn load(path: &Path) -> Result<Self> {
        let file = read_json_or(path, "reading devices.json", RegistryFile::default)?;
        Ok(Self {
            path: path.to_path_buf(),
            file,
        })
    }

    /// Registers a device and returns its assigned ID.
    ///
    /// Pairing the same key twice is not an error: it re-registers under the same derived
    /// device ID and refreshes the name. Re-pairing a device that was wiped and restored should
    /// not leave a duplicate entry behind.
    ///
    /// The key is validated as a real Ed25519 point before it is written. Callers reach here
    /// through `verify_pair_request`, which has already checked it — but a registry that can
    /// store a key it cannot later load would turn a bad write into a mystery authentication
    /// failure days afterwards.
    pub fn insert(
        &mut self,
        pubkey_bytes: &[u8],
        name: String,
        platform: Platform,
    ) -> Result<String> {
        let key_array: [u8; 32] =
            pubkey_bytes
                .try_into()
                .map_err(|_| EngineError::CorruptIdentity {
                    what: "device public key",
                })?;
        VerifyingKey::from_bytes(&key_array).map_err(|_| EngineError::CorruptIdentity {
            what: "device public key",
        })?;

        let device_id = id_from_pubkey_bytes("d_", pubkey_bytes);
        let now = unix_now();
        let paired_at = self
            .file
            .devices
            .get(&device_id)
            .map_or(now, |existing| existing.paired_at);

        self.file.devices.insert(
            device_id.clone(),
            StoredDevice {
                device_id: device_id.clone(),
                name,
                platform,
                pubkey: BASE64.encode(pubkey_bytes),
                paired_at,
                last_seen: now,
            },
        );
        self.persist()?;
        Ok(device_id)
    }

    pub fn get(&self, device_id: &str) -> Option<&StoredDevice> {
        self.file.devices.get(device_id)
    }

    /// Looks up the key a device authenticates with, or the wire error to answer with.
    pub fn verifying_key(&self, device_id: &str) -> Result<VerifyingKey> {
        let device = self.get(device_id).ok_or_else(|| {
            EngineError::wire(
                ErrorCode::UnknownDevice,
                "device is not paired with this host",
            )
        })?;
        device.verifying_key()
    }

    /// Removes a device. Returns whether it was there to begin with.
    ///
    /// The caller is responsible for closing any live socket belonging to it — the registry has
    /// no view of connections. `docs/PROTOCOL.md` §4.2 requires that to happen immediately.
    pub fn revoke(&mut self, device_id: &str) -> Result<bool> {
        let existed = self.file.devices.remove(device_id).is_some();
        if existed {
            self.persist()?;
        }
        Ok(existed)
    }

    pub fn touch(&mut self, device_id: &str) -> Result<()> {
        if let Some(device) = self.file.devices.get_mut(device_id) {
            device.last_seen = unix_now();
            self.persist()?;
        }
        Ok(())
    }

    /// The `pair.list_devices` payload, with live connection state layered on.
    pub fn list(&self, connected: &HashSet<String>) -> Vec<DeviceInfo> {
        self.file
            .devices
            .values()
            .map(|d| DeviceInfo {
                device_id: d.device_id.clone(),
                name: d.name.clone(),
                platform: d.platform,
                paired_at: d.paired_at,
                last_seen: d.last_seen,
                connected: connected.contains(&d.device_id),
            })
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.file.devices.is_empty()
    }

    fn persist(&self) -> Result<()> {
        write_json(&self.path, &self.file)
    }
}

/// Seconds since the Unix epoch. Every timestamp on the wire uses this except `system.ping`,
/// which is in milliseconds — `docs/PROTOCOL.md` §4.8.
pub fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Milliseconds since the Unix epoch, for `system.ping`.
pub fn unix_now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("muxdeck-registry-tests");
        fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join(format!("{name}.json"));
        let _ = fs::remove_file(&path);
        path
    }

    /// A real Ed25519 public key. Arbitrary byte arrays are not valid curve points, so the
    /// registry rejects them — deliberately.
    fn pubkey(seed: u8) -> [u8; 32] {
        ed25519_dalek::SigningKey::from_bytes(&[seed; 32])
            .verifying_key()
            .to_bytes()
    }

    #[test]
    fn insert_then_reload_keeps_the_device() {
        let path = temp_path("reload");
        let mut registry = Registry::load(&path).expect("load");
        let id = registry
            .insert(&pubkey(7), "Cipher's iPad".into(), Platform::Ios)
            .expect("insert");

        let reloaded = Registry::load(&path).expect("reload");
        let device = reloaded.get(&id).expect("device survives a reload");
        assert_eq!(device.name, "Cipher's iPad");
        assert_eq!(device.platform, Platform::Ios);
        assert!(reloaded.verifying_key(&id).is_ok());

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn the_device_id_is_derived_from_the_key_not_assigned() {
        let path = temp_path("derived");
        let mut registry = Registry::load(&path).expect("load");
        let first = registry
            .insert(&pubkey(9), "One".into(), Platform::Android)
            .expect("insert");
        let second = registry
            .insert(&pubkey(9), "One renamed".into(), Platform::Android)
            .expect("insert again");

        assert_eq!(first, second, "the same key must map to the same device id");
        assert_eq!(
            registry.list(&HashSet::new()).len(),
            1,
            "re-pairing the same key must not create a duplicate"
        );
        assert_eq!(registry.get(&first).expect("present").name, "One renamed");

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn revoke_removes_and_reports_whether_it_existed() {
        let path = temp_path("revoke");
        let mut registry = Registry::load(&path).expect("load");
        let id = registry
            .insert(&pubkey(3), "Gone".into(), Platform::Linux)
            .expect("insert");

        assert!(registry.revoke(&id).expect("revoke"));
        assert!(!registry.revoke(&id).expect("second revoke"));
        assert!(registry.is_empty());
        assert!(Registry::load(&path).expect("reload").is_empty());

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn an_unknown_device_is_a_wire_error_not_a_panic() {
        let path = temp_path("unknown");
        let registry = Registry::load(&path).expect("load");
        let err = registry
            .verifying_key("d_0000000000000000")
            .expect_err("unknown device");
        assert_eq!(err.to_payload().code, ErrorCode::UnknownDevice);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn list_layers_live_connection_state_on_top() {
        let path = temp_path("connected");
        let mut registry = Registry::load(&path).expect("load");
        let id = registry
            .insert(&pubkey(1), "Live".into(), Platform::Windows)
            .expect("insert");

        assert!(!registry.list(&HashSet::new())[0].connected);
        let connected = HashSet::from([id]);
        assert!(registry.list(&connected)[0].connected);

        let _ = fs::remove_file(&path);
    }
}
