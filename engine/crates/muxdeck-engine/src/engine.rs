//! Shared engine state: everything a connection handler needs to reach.

use std::collections::HashSet;
use std::sync::{Arc, Mutex, RwLock};

use muxdeck_core::{Capabilities, DeviceInfo, Settings, SettingsPatch};
use muxdeck_input::{platform_backend, InputBackend};
use tracing::warn;

use crate::actions::ActionStore;
use crate::config::{load_settings, save_settings, Paths};
use crate::discovery::Advertisement;
use crate::error::Result;
use crate::identity::Identity;
use crate::pairing::PairingWindow;
use crate::registry::Registry;
use crate::store::Store;

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
    /// The platform input backend. `Arc` rather than `Box` because every injection is handed
    /// to `spawn_blocking`, which needs an owned handle.
    input: Arc<dyn InputBackend>,
    store: Mutex<Store>,
    actions: Mutex<ActionStore>,
    /// The live mDNS advertisement, so `settings.set` can re-advertise under a new host name
    /// without restarting the daemon.
    advertisement: Mutex<Option<Advertisement>>,
}

impl Engine {
    pub fn load(paths: Paths) -> Result<Arc<Self>> {
        let identity = Identity::load_or_generate(&paths)?;
        let settings = load_settings(&paths)?;
        let registry = Registry::load(&paths.devices())?;
        let store = Store::load(&paths.profiles())?;
        let actions = ActionStore::load(&paths.actions())?;
        let port = settings.port;

        Ok(Arc::new(Self {
            paths,
            identity,
            settings: RwLock::new(settings),
            registry: Mutex::new(registry),
            pairing: Mutex::new(None),
            connected: Mutex::new(HashSet::new()),
            port: RwLock::new(port),
            input: Arc::from(platform_backend()),
            store: Mutex::new(store),
            actions: Mutex::new(actions),
            advertisement: Mutex::new(None),
        }))
    }

    /// The platform input backend, for handing to `spawn_blocking`.
    pub fn input(&self) -> Arc<dyn InputBackend> {
        Arc::clone(&self.input)
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

    /// Applies a partial `settings.set` and performs its side effects.
    ///
    /// Order matters. `autostart` is applied **before** anything is written, because installing
    /// or removing the platform's startup entry is the one step that can genuinely fail — a
    /// missing `schtasks`, a daemon running from a temporary path — and persisting a setting
    /// whose effect did not happen would leave the panel showing a lie.
    ///
    /// Re-advertising over mDNS is the opposite case: it happens *after* the write and a failure
    /// is logged rather than returned. The user asked for the name to change, the name has
    /// changed, and the advertisement is rebuilt from settings at the next start anyway; losing
    /// the rename over a transient multicast problem would be the worse outcome.
    pub fn apply_settings(&self, patch: SettingsPatch) -> Result<Settings> {
        let current = self.settings();
        let mut next = current.clone();

        if let Some(port) = patch.port {
            next.port = port;
        }
        if let Some(host_name) = patch.host_name {
            next.host_name = host_name;
        }
        if let Some(enabled) = patch.shell_actions_enabled {
            next.shell_actions_enabled = enabled;
        }
        if let Some(enabled) = patch.telemetry_enabled {
            next.telemetry_enabled = enabled;
        }
        if let Some(interval) = patch.telemetry_interval_ms {
            next.telemetry_interval_ms = interval;
        }
        if let Some(autostart) = patch.autostart {
            next.autostart = autostart;
        }

        if next.autostart != current.autostart {
            if next.autostart {
                crate::service::install(self.paths.root())?;
            } else {
                crate::service::uninstall()?;
            }
        }

        self.update_settings(next.clone())?;

        if next.host_name != current.host_name {
            if let Err(error) = self.readvertise() {
                warn!(%error, "the host name changed but mDNS could not be re-advertised");
            }
        }

        Ok(next)
    }

    /// Starts advertising, replacing any existing advertisement.
    pub fn advertise(&self) -> Result<()> {
        let advertisement = Advertisement::start(
            &self.host_name(),
            self.identity.host_id(),
            self.identity.fingerprint(),
            self.port(),
        )?;

        if let Some(previous) = self
            .advertisement
            .lock()
            .expect("advertisement lock poisoned")
            .replace(advertisement)
        {
            previous.stop();
        }
        Ok(())
    }

    /// Tears the advertisement down and puts it back up under the current settings.
    ///
    /// Separate from [`Engine::advertise`] only so the caller reads as what it means; an mDNS
    /// service name cannot be edited in place, it has to be withdrawn and re-registered.
    pub fn readvertise(&self) -> Result<()> {
        self.stop_advertising();
        self.advertise()
    }

    pub fn stop_advertising(&self) {
        if let Some(advertisement) = self
            .advertisement
            .lock()
            .expect("advertisement lock poisoned")
            .take()
        {
            advertisement.stop();
        }
    }

    pub fn with_actions<T>(&self, f: impl FnOnce(&mut ActionStore) -> T) -> T {
        f(&mut self.actions.lock().expect("actions lock poisoned"))
    }

    /// Runs a named action, or explains why it did not.
    ///
    /// The `shell_actions_enabled` check lives here rather than in the socket handler so it
    /// cannot be forgotten by a future caller: this is the only way to execute anything.
    pub fn run_action(&self, action_id: &str) -> Result<()> {
        if !self.settings().shell_actions_enabled {
            return Err(crate::error::EngineError::wire(
                muxdeck_core::ErrorCode::Disabled,
                "shell actions are switched off. Enable them in the MuxDeck control panel; \
                 any paired device will then be able to run every defined action.",
            ));
        }

        let action = self
            .with_actions(|store| store.get(action_id).cloned())
            .ok_or_else(|| {
                crate::error::EngineError::wire(
                    muxdeck_core::ErrorCode::NotFound,
                    "no such action. A button can outlive the action it points at.",
                )
            })?;

        crate::actions::spawn(&action)
    }

    /// What this build can actually do right now.
    ///
    /// Reported in the `Ready` payload so a client greys out buttons rather than watching them
    /// fail at press time (`docs/PROTOCOL.md` §4.1). A backend that fails `preflight` reports
    /// nothing available, whatever it claims to support — on a Linux host with no `/dev/uinput`
    /// access, every input button should be visibly dead rather than optimistically enabled.
    pub fn capabilities(&self) -> Capabilities {
        let usable = self.input.preflight().is_ok();
        let backend = self.input.capabilities();

        Capabilities {
            text_unicode: usable && backend.text_unicode,
            media_keys: usable && backend.media_keys,
            mouse: usable && backend.mouse,
            shell_actions: self.settings().shell_actions_enabled,
        }
    }

    /// Why input injection is unavailable, if it is.
    ///
    /// Surfaced by the control panel as the loudest thing on the dashboard, with the
    /// remediation the backend supplies (`docs/SERVER.md` §6).
    pub fn preflight(&self) -> Result<()> {
        self.input.preflight().map_err(|error| {
            crate::error::EngineError::wire(
                muxdeck_core::ErrorCode::InjectionFailed,
                error.to_string(),
            )
        })
    }

    /// The profile a newly connected deck should display.
    pub fn active_profile_id(&self) -> String {
        self.store.lock().expect("store lock poisoned").active_id()
    }

    pub fn with_store<T>(&self, f: impl FnOnce(&mut Store) -> T) -> T {
        f(&mut self.store.lock().expect("store lock poisoned"))
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
