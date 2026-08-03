//! Config directory resolution, settings, and the shared atomic-write helper.
//!
//! `docs/ENGINE.md` §6.

use std::fs;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use muxdeck_core::Settings;

use crate::error::{EngineError, Result};

/// Default listen port. `docs/PROTOCOL.md` §1.
pub const DEFAULT_PORT: u16 = 47654;

/// Every path the engine reads or writes, resolved once at startup.
#[derive(Debug, Clone)]
pub struct Paths {
    root: PathBuf,
}

impl Paths {
    /// Resolves the config directory, honouring `--config-dir` when given.
    ///
    /// The directory is created if it does not exist, but nothing inside it is — first-run
    /// generation is `identity`'s job.
    pub fn resolve(override_dir: Option<PathBuf>) -> Result<Self> {
        let root = match override_dir {
            Some(dir) => dir,
            None => ProjectDirs::from("in", "redoimagined", "MuxDeck")
                .map(|dirs| dirs.config_dir().to_path_buf())
                .ok_or_else(|| {
                    EngineError::Certificate(
                        "no home directory found; pass --config-dir".to_string(),
                    )
                })?,
        };

        fs::create_dir_all(&root)
            .map_err(|e| EngineError::io("creating the config directory", &root, e))?;
        fs::create_dir_all(root.join("logs"))
            .map_err(|e| EngineError::io("creating the log directory", &root, e))?;

        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Host Ed25519 private key. Secret.
    pub fn identity_key(&self) -> PathBuf {
        self.root.join("identity.key")
    }

    /// Self-signed leaf certificate, PEM.
    pub fn tls_cert(&self) -> PathBuf {
        self.root.join("tls.pem")
    }

    /// Private key for the certificate above, PEM. Secret.
    pub fn tls_key(&self) -> PathBuf {
        self.root.join("tls.key")
    }

    /// Local admin token. Secret — its permissions are a trust boundary, see
    /// [`crate::secret_file`].
    pub fn admin_token(&self) -> PathBuf {
        self.root.join("admin.token")
    }

    pub fn devices(&self) -> PathBuf {
        self.root.join("devices.json")
    }

    pub fn settings(&self) -> PathBuf {
        self.root.join("settings.json")
    }

    /// Deck layouts.
    pub fn profiles(&self) -> PathBuf {
        self.root.join("profiles.json")
    }

    pub fn actions(&self) -> PathBuf {
        self.root.join("actions.json")
    }

    pub fn logs(&self) -> PathBuf {
        self.root.join("logs")
    }
}

/// Writes `bytes` to `path` atomically: temp file, then rename.
///
/// A half-written `devices.json` loses every pairing, and a crash mid-write is exactly when
/// that would happen. Rename is atomic on every platform we target.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes).map_err(|e| EngineError::io("writing", &tmp, e))?;
    fs::rename(&tmp, path).map_err(|e| EngineError::io("installing", path, e))
}

/// Reads and parses a JSON file, or returns `default` when it does not exist yet.
///
/// A *malformed* file is an error, never silently replaced by the default — that would
/// discard every paired device on a single stray byte.
pub fn read_json_or<T>(path: &Path, context: &'static str, default: impl FnOnce() -> T) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    match fs::read(path) {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).map_err(|e| EngineError::json(context, path, e))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(default()),
        Err(e) => Err(EngineError::io(context, path, e)),
    }
}

pub fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    let bytes =
        serde_json::to_vec_pretty(value).map_err(|e| EngineError::json("serialising", path, e))?;
    write_atomic(path, &bytes)
}

/// The settings a fresh install starts with. `docs/PROTOCOL.md` §4.6.
pub fn default_settings(host_name: String) -> Settings {
    Settings {
        port: DEFAULT_PORT,
        host_name,
        // Off by default, deliberately. `docs/ARCHITECTURE.md` §5.5 — this is the single
        // largest footgun in a project like this and only the panel can switch it on.
        shell_actions_enabled: false,
        telemetry_enabled: true,
        telemetry_interval_ms: 1000,
        autostart: true,
    }
}

/// The machine's name, used as the default `host_name` and advertised over mDNS.
pub fn default_host_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "MuxDeck Host".to_string())
}

pub fn load_settings(paths: &Paths) -> Result<Settings> {
    read_json_or(&paths.settings(), "reading settings.json", || {
        default_settings(default_host_name())
    })
}

pub fn save_settings(paths: &Paths, settings: &Settings) -> Result<()> {
    write_json(&paths.settings(), settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("muxdeck-config-tests").join(name);
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn resolve_creates_the_directory_and_a_log_subdirectory() {
        let root = temp_root("resolve");
        let paths = Paths::resolve(Some(root.clone())).expect("resolve");
        assert!(paths.root().is_dir());
        assert!(paths.logs().is_dir());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_json_yields_the_default() {
        let root = temp_root("missing");
        let paths = Paths::resolve(Some(root.clone())).expect("resolve");
        let settings = load_settings(&paths).expect("load");
        assert_eq!(settings.port, DEFAULT_PORT);
        assert!(
            !settings.shell_actions_enabled,
            "shell actions must default to off"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn malformed_json_is_an_error_not_a_silent_reset() {
        let root = temp_root("malformed");
        let paths = Paths::resolve(Some(root.clone())).expect("resolve");
        fs::write(paths.settings(), b"{ this is not json").expect("write");
        assert!(
            load_settings(&paths).is_err(),
            "a corrupt settings file must not be silently replaced by defaults"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn settings_round_trip() {
        let root = temp_root("round_trip");
        let paths = Paths::resolve(Some(root.clone())).expect("resolve");
        let mut settings = default_settings("ENIGMA-ENTROPY".to_string());
        settings.port = 47700;
        save_settings(&paths, &settings).expect("save");
        assert_eq!(load_settings(&paths).expect("load").port, 47700);
        let _ = fs::remove_dir_all(&root);
    }
}
