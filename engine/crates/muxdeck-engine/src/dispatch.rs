//! Capability gating: which role may invoke which op.
//!
//! The authoritative table is `docs/ARCHITECTURE.md` §5.4. This module is an implementation of
//! it, and the test at the bottom is a transcription of it — so a row changed in one place and
//! not the other fails the build rather than quietly widening access.

use muxdeck_core::{ErrorCode, KnownOp, Op, Role};

use crate::error::{EngineError, Result};
use crate::session::Session;

/// Who may invoke an op.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// Only before the handshake completes. Sending it afterwards is a protocol error.
    PreAuth,
    /// Any authenticated socket, `deck` or `admin`.
    Authenticated,
    /// `admin` only.
    AdminOnly,
    /// Never inbound — the engine pushes these, clients do not send them.
    EventOnly,
}

/// The capability matrix, in one place.
pub fn access_for(op: KnownOp) -> Access {
    use KnownOp as O;

    match op {
        // Handshake and pairing happen before there is a role to check.
        O::SessionHello | O::SessionAuth | O::PairRequest => Access::PreAuth,

        // Managing pairing is an admin operation — opening a pairing window is precisely how a
        // new device gains access, so a deck must never be able to do it.
        O::PairBegin | O::PairCancel | O::PairListDevices | O::PairRevoke => Access::AdminOnly,

        // A deck's whole purpose.
        O::SystemPing
        | O::InputKeyCombo
        | O::InputKeySequence
        | O::InputText
        | O::InputMedia
        | O::InputMouse
        | O::ActionRun
        | O::ActionList
        | O::ProfileGet
        | O::ProfileList
        | O::ProfileSubscribe
        | O::TelemetrySubscribe => Access::Authenticated,

        // Deck-callable deliberately: a device that can already inject arbitrary keystrokes
        // gains nothing by choosing which grid it displays, and "switch to my streaming profile"
        // is a table-stakes deck button that must work with the panel closed.
        O::ProfileActivate => Access::Authenticated,

        // Changing what the deck *is*, rather than using it.
        O::ActionSet
        | O::ActionDelete
        | O::ProfileSet
        | O::ProfileDelete
        | O::SettingsGet
        | O::SettingsSet => Access::AdminOnly,

        O::ProfileChanged
        | O::TelemetryUpdate
        | O::DeviceChanged
        | O::PairingState
        | O::EngineShutdown => Access::EventOnly,
    }
}

/// Checks whether this socket may invoke this op, before any payload is looked at.
///
/// Ordering matters: an unauthenticated socket gets `NOT_AUTHENTICATED` rather than
/// `NOT_AUTHORIZED`, so it cannot use the difference to enumerate which ops exist.
pub fn authorize(op: &Op, session: &Session) -> Result<KnownOp> {
    let known = op.known().ok_or_else(|| {
        EngineError::wire(ErrorCode::UnknownOp, format!("no such op: {}", op.as_str()))
    })?;

    match access_for(known) {
        Access::PreAuth => {
            if session.is_ready() {
                Err(EngineError::wire(
                    ErrorCode::BadRequest,
                    "this op is only valid before the session handshake completes",
                ))
            } else {
                Ok(known)
            }
        }
        Access::Authenticated => {
            if session.is_ready() {
                Ok(known)
            } else {
                Err(not_authenticated())
            }
        }
        Access::AdminOnly => match session.role() {
            Some(Role::Admin) => Ok(known),
            Some(Role::Deck) => Err(EngineError::wire(
                ErrorCode::NotAuthorized,
                "role 'deck' may not call this op",
            )),
            None => Err(not_authenticated()),
        },
        Access::EventOnly => Err(EngineError::wire(
            ErrorCode::UnknownOp,
            "this op is an event pushed by the engine and cannot be sent to it",
        )),
    }
}

fn not_authenticated() -> EngineError {
    EngineError::wire(
        ErrorCode::NotAuthenticated,
        "this op requires a completed session handshake",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A transcription of `docs/ARCHITECTURE.md` §5.4, written out by hand rather than derived
    /// from `access_for` — deriving it would make the test agree with the code instead of with
    /// the specification.
    const MATRIX: &[(KnownOp, Access)] = &[
        (KnownOp::SessionHello, Access::PreAuth),
        (KnownOp::SessionAuth, Access::PreAuth),
        (KnownOp::PairRequest, Access::PreAuth),
        (KnownOp::PairBegin, Access::AdminOnly),
        (KnownOp::PairCancel, Access::AdminOnly),
        (KnownOp::PairListDevices, Access::AdminOnly),
        (KnownOp::PairRevoke, Access::AdminOnly),
        (KnownOp::SystemPing, Access::Authenticated),
        (KnownOp::InputKeyCombo, Access::Authenticated),
        (KnownOp::InputKeySequence, Access::Authenticated),
        (KnownOp::InputText, Access::Authenticated),
        (KnownOp::InputMedia, Access::Authenticated),
        (KnownOp::InputMouse, Access::Authenticated),
        (KnownOp::ActionRun, Access::Authenticated),
        (KnownOp::ActionList, Access::Authenticated),
        (KnownOp::ActionSet, Access::AdminOnly),
        (KnownOp::ActionDelete, Access::AdminOnly),
        (KnownOp::ProfileGet, Access::Authenticated),
        (KnownOp::ProfileList, Access::Authenticated),
        (KnownOp::ProfileSubscribe, Access::Authenticated),
        (KnownOp::ProfileActivate, Access::Authenticated),
        (KnownOp::ProfileSet, Access::AdminOnly),
        (KnownOp::ProfileDelete, Access::AdminOnly),
        (KnownOp::TelemetrySubscribe, Access::Authenticated),
        (KnownOp::SettingsGet, Access::AdminOnly),
        (KnownOp::SettingsSet, Access::AdminOnly),
        (KnownOp::ProfileChanged, Access::EventOnly),
        (KnownOp::TelemetryUpdate, Access::EventOnly),
        (KnownOp::DeviceChanged, Access::EventOnly),
        (KnownOp::PairingState, Access::EventOnly),
        (KnownOp::EngineShutdown, Access::EventOnly),
    ];

    #[test]
    fn the_matrix_covers_every_op_exactly_once() {
        assert_eq!(
            MATRIX.len(),
            KnownOp::ALL.len(),
            "docs/ARCHITECTURE.md §5.4 must list every op exactly once"
        );
        for op in KnownOp::ALL {
            let rows = MATRIX
                .iter()
                .filter(|(candidate, _)| candidate == op)
                .count();
            assert_eq!(
                rows,
                1,
                "{} appears {rows} times in the matrix",
                op.as_str()
            );
        }
    }

    #[test]
    fn access_matches_the_specification() {
        for (op, expected) in MATRIX {
            assert_eq!(
                access_for(*op),
                *expected,
                "{} has the wrong access level",
                op.as_str()
            );
        }
    }

    // The three socket states, built without a full handshake so this test stays a unit test.
    // `Session` has no constructor for a given role by design, so these go through the real
    // state machine in session.rs — see that module's tests for the transitions themselves.
    mod sockets {
        use super::*;
        use crate::session::tests_support::{ready_admin, ready_deck, unauthenticated};

        #[test]
        fn an_unauthenticated_socket_may_send_only_pre_auth_ops() {
            let session = unauthenticated();
            for (op, access) in MATRIX {
                let result = authorize(&Op::of(*op), &session);
                match access {
                    Access::PreAuth => assert!(result.is_ok(), "{} must be allowed", op.as_str()),
                    _ => {
                        let code = result.expect_err("must be refused").to_payload().code;
                        let expected = if *access == Access::EventOnly {
                            ErrorCode::UnknownOp
                        } else {
                            ErrorCode::NotAuthenticated
                        };
                        assert_eq!(code, expected, "{} gave the wrong error", op.as_str());
                    }
                }
            }
        }

        #[test]
        fn a_deck_may_not_touch_admin_ops() {
            let session = ready_deck();
            for (op, access) in MATRIX {
                let result = authorize(&Op::of(*op), &session);
                match access {
                    Access::Authenticated => {
                        assert!(result.is_ok(), "{} must be allowed to a deck", op.as_str())
                    }
                    Access::AdminOnly => assert_eq!(
                        result.expect_err("refused").to_payload().code,
                        ErrorCode::NotAuthorized,
                        "{} must be refused to a deck",
                        op.as_str()
                    ),
                    Access::PreAuth => assert_eq!(
                        result.expect_err("refused").to_payload().code,
                        ErrorCode::BadRequest,
                        "{} must not be repeatable after the handshake",
                        op.as_str()
                    ),
                    Access::EventOnly => assert_eq!(
                        result.expect_err("refused").to_payload().code,
                        ErrorCode::UnknownOp
                    ),
                }
            }
        }

        #[test]
        fn an_admin_may_touch_everything_except_events_and_handshake_ops() {
            let session = ready_admin();
            for (op, access) in MATRIX {
                let result = authorize(&Op::of(*op), &session);
                match access {
                    Access::Authenticated | Access::AdminOnly => {
                        assert!(result.is_ok(), "{} must be allowed to admin", op.as_str())
                    }
                    Access::PreAuth => assert!(result.is_err(), "{} must not repeat", op.as_str()),
                    Access::EventOnly => assert!(result.is_err(), "{} is outbound", op.as_str()),
                }
            }
        }
    }

    #[test]
    fn an_unknown_op_is_unknown_op_not_a_panic() {
        let session = crate::session::tests_support::ready_admin();
        let err = authorize(&Op::parse("input.telepathy"), &session).expect_err("unknown");
        assert_eq!(err.to_payload().code, ErrorCode::UnknownOp);
    }
}
