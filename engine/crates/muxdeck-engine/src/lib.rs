//! The MuxDeck engine.
//!
//! Everything that makes the daemon work lives here as a library, so it can be tested
//! without a binary: the TLS WebSocket server, the session handshake, pairing, the device
//! registry, the profile store, mDNS advertisement and telemetry. `muxdeckd` is a thin
//! wrapper that parses a command line and calls into this crate.
//!
//! Modules arrive in M2 (`identity`, `server`, `session`, `pairing`, `registry`,
//! `dispatch`, `config`), M3 (input dispatch), M6 (`store`) and M8 (`telemetry`,
//! `discovery`). See `docs/ENGINE.md` §5.

#![forbid(unsafe_code)]

// Re-exported so downstream crates and tests reach the protocol and input types through
// one path, and so the workspace dependency direction is exercised rather than merely
// declared: muxdeckd -> muxdeck-engine -> { muxdeck-core, muxdeck-input }.
pub use muxdeck_core;
pub use muxdeck_input;
