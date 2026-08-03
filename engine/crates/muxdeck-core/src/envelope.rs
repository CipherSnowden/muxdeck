//! The message envelope, operation names and error payloads. `docs/PROTOCOL.md` §2.

use serde::{Deserialize, Serialize};

/// The only protocol major version this build speaks. Anything else is
/// [`ErrorCode::UnsupportedVersion`].
pub const PROTOCOL_VERSION: u8 = 1;

/// Every message is this shape. `docs/PROTOCOL.md` §2.
///
/// Generic over its payload because `d`'s type is a function of `op` and `t` — and of
/// nothing else. A reader picks the concrete `T` from those two fields; there is no third
/// input to that decision, and in particular the variant suffix on a fixture filename is
/// not one (`docs/PROTOCOL.md` §8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Envelope<T> {
    /// Protocol major version. Reject anything but [`PROTOCOL_VERSION`].
    pub v: u8,
    /// `req`, `res`, `err` or `evt`.
    pub t: MessageType,
    /// Correlation ID. Present on `req`, `res` and `err`; absent on `evt`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Operation name. A `res` or `err` echoes the op of the `req` it answers.
    pub op: Op,
    /// Payload. `{}` when empty, never `null`.
    pub d: T,
}

impl<T> Envelope<T> {
    /// True when `v` matches the version this build implements.
    pub fn is_supported_version(&self) -> bool {
        self.v == PROTOCOL_VERSION
    }

    /// Checks the envelope's structural invariants — the ones that hold regardless of
    /// which op this is. Payload-level validation belongs with the payload type.
    ///
    /// An `evt` carries its own op name because there is no request to echo, so it has no
    /// correlation ID; every other kind must have one.
    pub fn validate(&self) -> Result<(), ErrorPayload> {
        if !self.is_supported_version() {
            return Err(ErrorPayload::new(
                ErrorCode::UnsupportedVersion,
                format!("protocol version {} is not supported", self.v),
            ));
        }
        match (self.t, self.id.is_some()) {
            (MessageType::Evt, true) => Err(ErrorPayload::new(
                ErrorCode::BadRequest,
                "an evt must not carry a correlation id",
            )),
            (MessageType::Evt, false) => Ok(()),
            (_, false) => Err(ErrorPayload::new(
                ErrorCode::BadRequest,
                "a req, res or err must carry a correlation id",
            )),
            (_, true) => Ok(()),
        }
    }
}

/// The `t` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageType {
    Req,
    Res,
    Err,
    Evt,
}

/// An operation name.
///
/// Unknown ops deserialise into [`Op::Unknown`] rather than failing the parse outright, so
/// the engine can answer with a well-formed [`ErrorCode::UnknownOp`] that still echoes
/// what was asked for. A parse error here would cost the correlation ID and leave the
/// client waiting.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Op {
    Known(KnownOp),
    Unknown(String),
}

impl Op {
    pub fn of(op: KnownOp) -> Self {
        Op::Known(op)
    }

    pub fn parse(wire: &str) -> Self {
        match KnownOp::ALL
            .iter()
            .find(|candidate| candidate.as_str() == wire)
        {
            Some(op) => Op::Known(*op),
            None => Op::Unknown(wire.to_string()),
        }
    }

    /// The wire string, whether or not this op is one we know.
    pub fn as_str(&self) -> &str {
        match self {
            Op::Known(op) => op.as_str(),
            Op::Unknown(raw) => raw,
        }
    }

    pub fn known(&self) -> Option<KnownOp> {
        match self {
            Op::Known(op) => Some(*op),
            Op::Unknown(_) => None,
        }
    }
}

impl From<KnownOp> for Op {
    fn from(op: KnownOp) -> Self {
        Op::Known(op)
    }
}

/// Every op defined by the protocol.
///
/// This list is exhaustive against the capability matrix in `docs/ARCHITECTURE.md` §5.4 —
/// that table and this enum are checked against each other, so an op added to one without
/// the other is a bug.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KnownOp {
    #[serde(rename = "session.hello")]
    SessionHello,
    #[serde(rename = "session.auth")]
    SessionAuth,

    #[serde(rename = "pair.request")]
    PairRequest,
    #[serde(rename = "pair.begin")]
    PairBegin,
    #[serde(rename = "pair.cancel")]
    PairCancel,
    #[serde(rename = "pair.list_devices")]
    PairListDevices,
    #[serde(rename = "pair.revoke")]
    PairRevoke,

    #[serde(rename = "system.ping")]
    SystemPing,

    #[serde(rename = "input.key_combo")]
    InputKeyCombo,
    #[serde(rename = "input.key_sequence")]
    InputKeySequence,
    #[serde(rename = "input.text")]
    InputText,
    #[serde(rename = "input.media")]
    InputMedia,
    #[serde(rename = "input.mouse")]
    InputMouse,

    #[serde(rename = "action.run")]
    ActionRun,
    #[serde(rename = "action.list")]
    ActionList,
    #[serde(rename = "action.set")]
    ActionSet,
    #[serde(rename = "action.delete")]
    ActionDelete,

    #[serde(rename = "profile.get")]
    ProfileGet,
    #[serde(rename = "profile.list")]
    ProfileList,
    #[serde(rename = "profile.subscribe")]
    ProfileSubscribe,
    #[serde(rename = "profile.activate")]
    ProfileActivate,
    #[serde(rename = "profile.set")]
    ProfileSet,
    #[serde(rename = "profile.delete")]
    ProfileDelete,

    #[serde(rename = "telemetry.subscribe")]
    TelemetrySubscribe,

    #[serde(rename = "settings.get")]
    SettingsGet,
    #[serde(rename = "settings.set")]
    SettingsSet,

    // Events. These appear only with `t: "evt"` and never as a request.
    #[serde(rename = "profile.changed")]
    ProfileChanged,
    #[serde(rename = "telemetry.update")]
    TelemetryUpdate,
    #[serde(rename = "device.changed")]
    DeviceChanged,
    #[serde(rename = "pairing.state")]
    PairingState,
    #[serde(rename = "engine.shutdown")]
    EngineShutdown,
}

impl KnownOp {
    pub fn as_str(&self) -> &'static str {
        match self {
            KnownOp::SessionHello => "session.hello",
            KnownOp::SessionAuth => "session.auth",
            KnownOp::PairRequest => "pair.request",
            KnownOp::PairBegin => "pair.begin",
            KnownOp::PairCancel => "pair.cancel",
            KnownOp::PairListDevices => "pair.list_devices",
            KnownOp::PairRevoke => "pair.revoke",
            KnownOp::SystemPing => "system.ping",
            KnownOp::InputKeyCombo => "input.key_combo",
            KnownOp::InputKeySequence => "input.key_sequence",
            KnownOp::InputText => "input.text",
            KnownOp::InputMedia => "input.media",
            KnownOp::InputMouse => "input.mouse",
            KnownOp::ActionRun => "action.run",
            KnownOp::ActionList => "action.list",
            KnownOp::ActionSet => "action.set",
            KnownOp::ActionDelete => "action.delete",
            KnownOp::ProfileGet => "profile.get",
            KnownOp::ProfileList => "profile.list",
            KnownOp::ProfileSubscribe => "profile.subscribe",
            KnownOp::ProfileActivate => "profile.activate",
            KnownOp::ProfileSet => "profile.set",
            KnownOp::ProfileDelete => "profile.delete",
            KnownOp::TelemetrySubscribe => "telemetry.subscribe",
            KnownOp::SettingsGet => "settings.get",
            KnownOp::SettingsSet => "settings.set",
            KnownOp::ProfileChanged => "profile.changed",
            KnownOp::TelemetryUpdate => "telemetry.update",
            KnownOp::DeviceChanged => "device.changed",
            KnownOp::PairingState => "pairing.state",
            KnownOp::EngineShutdown => "engine.shutdown",
        }
    }

    /// True for the five ops that only ever appear as `t: "evt"`.
    pub fn is_event(&self) -> bool {
        matches!(
            self,
            KnownOp::ProfileChanged
                | KnownOp::TelemetryUpdate
                | KnownOp::DeviceChanged
                | KnownOp::PairingState
                | KnownOp::EngineShutdown
        )
    }

    /// Every op, for exhaustiveness tests against `docs/ARCHITECTURE.md` §5.4.
    pub const ALL: &'static [KnownOp] = &[
        KnownOp::SessionHello,
        KnownOp::SessionAuth,
        KnownOp::PairRequest,
        KnownOp::PairBegin,
        KnownOp::PairCancel,
        KnownOp::PairListDevices,
        KnownOp::PairRevoke,
        KnownOp::SystemPing,
        KnownOp::InputKeyCombo,
        KnownOp::InputKeySequence,
        KnownOp::InputText,
        KnownOp::InputMedia,
        KnownOp::InputMouse,
        KnownOp::ActionRun,
        KnownOp::ActionList,
        KnownOp::ActionSet,
        KnownOp::ActionDelete,
        KnownOp::ProfileGet,
        KnownOp::ProfileList,
        KnownOp::ProfileSubscribe,
        KnownOp::ProfileActivate,
        KnownOp::ProfileSet,
        KnownOp::ProfileDelete,
        KnownOp::TelemetrySubscribe,
        KnownOp::SettingsGet,
        KnownOp::SettingsSet,
        KnownOp::ProfileChanged,
        KnownOp::TelemetryUpdate,
        KnownOp::DeviceChanged,
        KnownOp::PairingState,
        KnownOp::EngineShutdown,
    ];
}

/// An empty payload. Serialises to `{}`, which is what the protocol requires — never
/// `null`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Empty {}

/// The payload of an `err` message. `docs/PROTOCOL.md` §2.1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorPayload {
    pub code: ErrorCode,
    pub message: String,
}

impl ErrorPayload {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// `docs/PROTOCOL.md` §2.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    /// Malformed envelope or payload.
    BadRequest,
    /// `v` is not 1.
    UnsupportedVersion,
    /// No such op.
    UnknownOp,
    /// Op requires a completed session handshake.
    NotAuthenticated,
    /// Role lacks capability for this op.
    NotAuthorized,
    /// Not in pairing mode, or the window expired.
    PairingClosed,
    /// Wrong one-time pairing code.
    BadCode,
    /// Device ID not in the registry.
    UnknownDevice,
    /// Challenge signature did not verify.
    BadSignature,
    /// The OS refused the input event.
    InjectionFailed,
    /// Profile or action does not exist.
    NotFound,
    /// Feature is switched off, e.g. shell execution.
    Disabled,
    /// Engine bug. Always logged with a trace ID.
    Internal,
}

impl ErrorCode {
    /// The wire string. Serde already knows this mapping; this exposes it for logging and
    /// error messages, where going through `serde_json` would be absurd.
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorCode::BadRequest => "BAD_REQUEST",
            ErrorCode::UnsupportedVersion => "UNSUPPORTED_VERSION",
            ErrorCode::UnknownOp => "UNKNOWN_OP",
            ErrorCode::NotAuthenticated => "NOT_AUTHENTICATED",
            ErrorCode::NotAuthorized => "NOT_AUTHORIZED",
            ErrorCode::PairingClosed => "PAIRING_CLOSED",
            ErrorCode::BadCode => "BAD_CODE",
            ErrorCode::UnknownDevice => "UNKNOWN_DEVICE",
            ErrorCode::BadSignature => "BAD_SIGNATURE",
            ErrorCode::InjectionFailed => "INJECTION_FAILED",
            ErrorCode::NotFound => "NOT_FOUND",
            ErrorCode::Disabled => "DISABLED",
            ErrorCode::Internal => "INTERNAL",
        }
    }
}
