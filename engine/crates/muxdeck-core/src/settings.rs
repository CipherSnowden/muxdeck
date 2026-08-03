//! Engine settings. `docs/PROTOCOL.md` §4.6. Admin only.

use serde::{Deserialize, Serialize};

/// The full settings object, as returned by `settings.get`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    pub port: u16,
    pub host_name: String,
    pub shell_actions_enabled: bool,
    pub telemetry_enabled: bool,
    pub telemetry_interval_ms: u32,
    pub autostart: bool,
}

/// `settings.set` request: a **partial** settings object.
///
/// Only the keys present are changed; an absent key is left alone rather than reset to its
/// default. An empty patch is valid.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SettingsPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell_actions_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub telemetry_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub telemetry_interval_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub autostart: Option<bool>,
}

impl SettingsPatch {
    /// True when applying this patch needs a daemon restart to take effect.
    ///
    /// Only `port` does. `host_name` triggers an mDNS re-advertise; everything else is
    /// live.
    pub fn requires_restart(&self) -> bool {
        self.port.is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsSetResponse {
    pub restart_required: bool,
}
