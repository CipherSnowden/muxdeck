//! Telemetry, ping and lifecycle events. `docs/PROTOCOL.md` §4.7, §4.8 and §4.9.

use serde::{Deserialize, Serialize};

use crate::pairing::DeviceInfo;

/// `system.ping` request. Milliseconds since the Unix epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PingRequest {
    pub t_client: i64,
}

/// `system.ping` response — there is no `pong` op, this *is* the pong.
///
/// `t_client` is echoed verbatim so a client can match a reply to a send. The client
/// computes RTT from its own send and receive timestamps and does not trust `t_engine` for
/// clock sync, only for one-way-delay estimation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PingResponse {
    pub t_client: i64,
    pub t_engine: i64,
}

/// `evt telemetry.update`, to sockets that called `telemetry.subscribe`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TelemetryUpdate {
    /// Unix timestamp, seconds.
    pub ts: i64,
    pub cpu_pct: f64,
    pub ram_pct: f64,
}

/// `evt device.changed`, to `admin` sockets only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceChangedEvent {
    pub devices: Vec<DeviceInfo>,
}

/// `evt engine.shutdown`, to every authenticated socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineShutdownEvent {
    pub reason: ShutdownReason,
}

/// An enum, not free text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShutdownReason {
    UserRequested,
    SettingsChanged,
    FatalError,
}
