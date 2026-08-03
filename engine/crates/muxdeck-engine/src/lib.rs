//! The MuxDeck engine.
//!
//! Everything that makes the daemon work lives here as a library, so it can be tested without a
//! binary: the TLS WebSocket server, the session handshake, pairing, the device registry, and
//! mDNS advertisement. `muxdeckd` is a thin wrapper that parses a command line and calls in.

#![forbid(unsafe_code)]

pub mod admin_client;
pub mod config;
pub mod discovery;
pub mod dispatch;
pub mod engine;
pub mod error;
pub mod identity;
pub mod input_dispatch;
pub mod pairing;
pub mod registry;
pub mod secret_file;
pub mod server;
pub mod service;
pub mod session;
pub mod store;

pub use engine::Engine;
pub use error::{EngineError, Result};
pub use muxdeck_core;
pub use muxdeck_input;
