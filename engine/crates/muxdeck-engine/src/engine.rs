//! Shared engine state: everything a connection handler needs to reach.

use std::collections::HashSet;
use std::sync::{Arc, Mutex, RwLock};

use muxdeck_core::{Capabilities, DeviceInfo, Settings};

use crate::config::{load_settings, save_settings, Paths};
use crate::error::Result;
use crate::identity::Identity;
use crate::pairing::PairingWindow;
use crate::registry::Registry;

/// State shared by every socket, plus the identity and settings loaded at startup.
pub struct Engine {
    pub paths: Paths,
    pub identity: Identity,
    settings: RwLock<Settings>,
    registry: Mutex<Registry>,
    pairing: Mutex<Option<PairingWindow>>,
    /// Device IDs with a live authenticated socket right now. Not persisted — see
    /// [`crate::registry::StoredDevice`].
    connected: Mutex<HashSet<String>>,
    /// The port actually bound, which differs from the configured one when binding to 0.
    port: RwLock<u16>,
}

impl Engine {
    pub fn load(paths: Paths) -> Result<Arc<Self>> {
        let identity = Identity::load_or_generate(&paths)?;
        let settings = load_settings(&paths)?;
        let registry = Registry::load(&paths.devices())?;
        let port = settings.port;

        Ok(Arc::new(Self {
            paths,
            identity,
            settings: RwLock::new(settings),
            registry: Mutex::new(registry),
            pairing: Mutex::new(None),
            connected: Mutex::new(HashSet::new()),
            port: RwLock::new(port),
        }))
    }

    pub fn settings(&self) -> Settings {
        self.settings
            .read()
            .expect("settings lock poisoned")
            .clone()
    }

    pub fn host_name(&self) -> String {
        self.settings().host_name
    }

    pub fn port(&self) -> u16 {
        *self.port.read().expect("port lock poisoned")
    }

    pub fn set_bound_port(&self, port: u16) {
        *self.port.write().expect("port lock poisoned") = port;
    }

    pub fn update_settings(&self, settings: Settings) -> Result<()> {
        save_settings(&self.paths, &settings)?;
        *self.settings.write().expect("settings lock poisoned") = settings;
        Ok(())
    }

    /// What this build can actually do.
    ///
    /// Every input capability is false until M3 lands the Windows backend; from then on these
    /// come from `InputBackend::preflight`, so a client greys out buttons rather than watching
    /// them fail at press time (`docs/PROTOCOL.md` §4.1).
    pub fn capabilities(&self) -> Capabilities {
        Capabilities {
            text_unicode: false,
            media_keys: false,
            mouse: false,
            shell_actions: self.settings().shell_actions_enabled,
        }
    }

    /// Placeholder until the profile store lands in M6.
    pub fn active_profile_id(&self) -> String {
        "p_default".to_string()
    }

    pub fn with_registry<T>(&self, f: impl FnOnce(&mut Registry) -> T) -> T {
        f(&mut self.registry.lock().expect("registry lock poisoned"))
    }

    pub fn list_devices(&self) -> Vec<DeviceInfo> {
        let connected = self
            .connected
            .lock()
            .expect("connected lock poisoned")
            .clone();
        self.registry
            .lock()
            .expect("registry lock poisoned")
            .list(&connected)
    }

    pub fn mark_connected(&self, device_id: &str) {
        self.connected
            .lock()
            .expect("connected lock poisoned")
            .insert(device_id.to_string());
    }

    pub fn mark_disconnected(&self, device_id: &str) {
        self.connected
            .lock()
            .expect("connected lock poisoned")
            .remove(device_id);
    }

    /// Opens a pairing window, replacing any existing one.
    pub fn open_pairing(&self, ttl_seconds: Option<u32>) -> Result<PairingWindow> {
        let window = PairingWindow::open(ttl_seconds)?;
        *self.pairing.lock().expect("pairing lock poisoned") = Some(window.clone());
        Ok(window)
    }

    pub fn close_pairing(&self) {
        *self.pairing.lock().expect("pairing lock poisoned") = None;
    }

    /// The current window, if one is open and unexpired.
    pub fn pairing_window(&self) -> Option<PairingWindow> {
        self.pairing
            .lock()
            .expect("pairing lock poisoned")
            .clone()
            .filter(|w| w.is_open())
    }

    /// `<ip>:<port>` for the QR payload — the address a phone should dial.
    ///
    /// Falls back to loopback when no non-loopback interface exists, which is useless for a
    /// real phone but keeps the payload well-formed rather than empty.
    pub fn advertised_addr(&self) -> String {
        let ip = if_addrs::get_if_addrs()
            .unwrap_or_default()
            .into_iter()
            .find(|iface| !iface.is_loopback() && iface.ip().is_ipv4())
            .map(|iface| iface.ip().to_string())
            .unwrap_or_else(|| "127.0.0.1".to_string());
        format!("{ip}:{}", self.port())
    }
}
