//! `muxdeckd` — the MuxDeck daemon.
//!
//! This binary owns nothing but startup: command-line parsing, config directory
//! resolution, tracing setup, then handing control to `muxdeck-engine`. Real work belongs
//! in the library so it stays testable.
//!
//! The CLI in `docs/ENGINE.md` §7 arrives in M2.

// Proves the dependency chain compiles end to end: muxdeckd -> muxdeck-engine ->
// { muxdeck-core, muxdeck-input }.
use muxdeck_engine as _;

fn main() {
    println!(
        "muxdeckd {} — not implemented yet, see docs/BUILD-PLAN.md",
        env!("CARGO_PKG_VERSION")
    );
}
