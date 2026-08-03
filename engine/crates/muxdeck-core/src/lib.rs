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
//!
//! Populated in milestone M1.

#![forbid(unsafe_code)]
