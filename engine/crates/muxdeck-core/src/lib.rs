//! MuxDeck wire protocol types.
//!
//! This crate is an implementation of `docs/PROTOCOL.md`, which is the single source of
//! truth — not the other way around. To change the protocol, edit that document first,
//! then `protocol/fixtures/`, then this crate, then the Dart types, in that order and in
//! one commit.
//!
//! It performs **no I/O** and contains **no platform code**, so the protocol stays
//! trivially testable and could be reused without pulling in a runtime. Nothing here may
//! depend on another crate in this workspace.

#![forbid(unsafe_code)]

pub mod action;
pub mod envelope;
pub mod input;
pub mod pairing;
pub mod profile;
pub mod session;
pub mod settings;
pub mod signing;
pub mod telemetry;

pub use action::{
    Action, ActionDeleteRequest, ActionListResponse, ActionRunRequest, ActionSetRequest,
};
pub use envelope::{
    Empty, Envelope, ErrorCode, ErrorPayload, KnownOp, MessageType, Op, PROTOCOL_VERSION,
};
pub use input::{
    Key, KeyCombo, KeySequence, MediaCommand, MediaRequest, MouseButton, MouseRequest,
    SequenceStep, TextRequest,
};
pub use pairing::{
    DeviceInfo, PairBeginRequest, PairBeginResponse, PairListDevicesResponse, PairRequest,
    PairResponse, PairRevokeRequest, PairingState,
};
pub use profile::{
    Button, ButtonAction, Grid, Haptic, Position, Profile, ProfileActivateRequest,
    ProfileDeleteRequest, ProfileGetRequest, ProfileListResponse, ProfileSummary, ProfileWrapper,
};
pub use session::{
    AuthRequest, Capabilities, Challenge, HelloRequest, HelloResponse, HostPlatform, Platform,
    Ready, Role,
};
pub use settings::{Settings, SettingsPatch, SettingsSetResponse};
pub use telemetry::{
    DeviceChangedEvent, EngineShutdownEvent, PingRequest, PingResponse, ShutdownReason,
    TelemetryUpdate,
};
