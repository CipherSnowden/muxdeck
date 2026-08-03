//! Turning `input.*` payloads into backend calls.
//!
//! Two rules shape this module, both from `docs/ENGINE.md` §4 and §9:
//!
//! - **Injection runs on `spawn_blocking`.** `SendInput` and its counterparts are syscalls that
//!   can take a millisecond or two; running them on the async runtime would stall every other
//!   socket for that long.
//! - **`input.key_sequence` is walked here, not in the backend.** The trait stays synchronous
//!   and dumb, and `delay_ms` becomes `tokio::time::sleep` rather than a blocked worker thread.

use std::sync::Arc;
use std::time::Duration;

use muxdeck_core::{
    ErrorCode, KeyCombo, KeySequence, MediaRequest, MouseRequest, SequenceStep, TextRequest,
};
use muxdeck_input::{InputBackend, InputError, MouseEvent};

use crate::error::{EngineError, Result};

/// The longest a single `input.text` may be.
///
/// Each character becomes four `SendInput` events, so an unbounded string is both a memory
/// amplifier and a way to hold the injection thread indefinitely. A deck button legitimately
/// types a snippet, not a document.
const MAX_TEXT_LEN: usize = 4096;

/// The longest a whole `input.key_sequence` may take.
///
/// Sequences run detached from the socket that asked for them, so without a ceiling a client
/// could queue minutes of delays and there would be no way to stop them.
const MAX_SEQUENCE_MS: u64 = 30_000;

pub type SharedBackend = Arc<dyn InputBackend>;

/// Runs a combo. `docs/PROTOCOL.md` §4.3.
pub async fn key_combo(backend: &SharedBackend, request: KeyCombo) -> Result<()> {
    request.validate().map_err(EngineError::Wire)?;

    let backend = Arc::clone(backend);
    let hold = Duration::from_millis(u64::from(request.hold_ms_or_default()));
    let keys = request.keys;

    blocking(move || backend.key_combo(&keys, hold)).await
}

/// Walks a sequence, sleeping between steps in async code.
pub async fn key_sequence(backend: &SharedBackend, request: KeySequence) -> Result<()> {
    request.validate().map_err(EngineError::Wire)?;

    let total: u64 = request
        .steps
        .iter()
        .map(|step| match step {
            SequenceStep::Delay { delay_ms } => u64::from(*delay_ms),
            SequenceStep::Combo(combo) => u64::from(combo.hold_ms_or_default()),
        })
        .sum();

    if total > MAX_SEQUENCE_MS {
        return Err(EngineError::wire(
            ErrorCode::BadRequest,
            format!("the sequence would take {total} ms; the limit is {MAX_SEQUENCE_MS} ms"),
        ));
    }

    for step in request.steps {
        match step {
            SequenceStep::Delay { delay_ms } => {
                tokio::time::sleep(Duration::from_millis(u64::from(delay_ms))).await;
            }
            SequenceStep::Combo(combo) => {
                key_combo(backend, combo).await?;
            }
        }
    }
    Ok(())
}

/// Types a literal string.
pub async fn text(backend: &SharedBackend, request: TextRequest) -> Result<()> {
    if request.text.len() > MAX_TEXT_LEN {
        return Err(EngineError::wire(
            ErrorCode::BadRequest,
            format!(
                "text is {} bytes; the limit is {MAX_TEXT_LEN}",
                request.text.len()
            ),
        ));
    }

    let delay = request.delay_ms_or_default();

    if delay == 0 {
        let backend = Arc::clone(backend);
        return blocking(move || backend.text(&request.text)).await;
    }

    // A per-character delay is a pause between injections, so it belongs in async code for the
    // same reason a sequence delay does. The characters are split here rather than in the
    // backend so the trait stays free of timing.
    for character in request.text.chars() {
        let backend = Arc::clone(backend);
        let single = character.to_string();
        blocking(move || backend.text(&single)).await?;
        tokio::time::sleep(Duration::from_millis(u64::from(delay))).await;
    }
    Ok(())
}

pub async fn media(backend: &SharedBackend, request: MediaRequest) -> Result<()> {
    let backend = Arc::clone(backend);
    blocking(move || backend.media(request.command)).await
}

pub async fn mouse(backend: &SharedBackend, request: MouseRequest) -> Result<()> {
    let event = mouse_event(request);
    let backend = Arc::clone(backend);
    blocking(move || backend.mouse(event)).await
}

fn mouse_event(request: MouseRequest) -> MouseEvent {
    match request {
        MouseRequest::MoveRel { dx, dy } => MouseEvent::MoveRelative { dx, dy },
        MouseRequest::MoveAbs { x, y } => MouseEvent::MoveAbsolute { x, y },
        // The backend reuses the protocol's own `MouseButton`, so there is nothing to convert.
        MouseRequest::Click { button } => MouseEvent::Click(button),
        MouseRequest::Down { button } => MouseEvent::Down(button),
        MouseRequest::Up { button } => MouseEvent::Up(button),
        MouseRequest::Scroll { dx, dy } => MouseEvent::Scroll { dx, dy },
    }
}

/// Runs an injection off the async runtime and normalises its failure.
async fn blocking<F>(work: F) -> Result<()>
where
    F: FnOnce() -> std::result::Result<(), InputError> + Send + 'static,
{
    let outcome = tokio::task::spawn_blocking(work).await.map_err(|e| {
        EngineError::wire(
            ErrorCode::Internal,
            format!("the injection task failed: {e}"),
        )
    })?;

    outcome.map_err(|error| {
        // Every InputError becomes INJECTION_FAILED with its message intact: those messages
        // are remediation instructions, and the client surfaces them as a toast.
        EngineError::wire(ErrorCode::InjectionFailed, error.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use muxdeck_core::{Key, MouseButton};
    use muxdeck_input::mock::{Call, MockBackend};

    fn backend() -> (SharedBackend, Arc<MockBackend>) {
        let mock = Arc::new(MockBackend::new());
        (mock.clone() as SharedBackend, mock)
    }

    #[tokio::test]
    async fn a_combo_presses_in_order_and_releases_in_reverse() {
        let (shared, mock) = backend();
        key_combo(
            &shared,
            KeyCombo {
                keys: vec![Key::Control, Key::Shift, Key::Escape],
                hold_ms: None,
            },
        )
        .await
        .expect("combo");

        assert_eq!(
            mock.calls(),
            vec![
                Call::KeyDown(Key::Control),
                Call::KeyDown(Key::Shift),
                Call::KeyDown(Key::Escape),
                Call::KeyUp(Key::Escape),
                Call::KeyUp(Key::Shift),
                Call::KeyUp(Key::Control),
            ]
        );
    }

    #[tokio::test]
    async fn a_failed_press_still_releases_and_reports_injection_failed() {
        let mock = Arc::new(MockBackend::failing_at(2));
        let shared = mock.clone() as SharedBackend;

        let err = key_combo(
            &shared,
            KeyCombo {
                keys: vec![Key::Control, Key::Shift, Key::A],
                hold_ms: None,
            },
        )
        .await
        .expect_err("must fail");

        assert_eq!(err.to_payload().code, ErrorCode::InjectionFailed);
        assert_eq!(
            mock.calls(),
            vec![
                Call::KeyDown(Key::Control),
                Call::KeyDown(Key::Shift),
                Call::KeyUp(Key::Shift),
                Call::KeyUp(Key::Control),
            ],
            "a latched modifier outlives the process, so release is unconditional"
        );
    }

    #[tokio::test]
    async fn two_non_modifiers_are_rejected_before_the_backend_is_touched() {
        let (shared, mock) = backend();
        let err = key_combo(
            &shared,
            KeyCombo {
                keys: vec![Key::A, Key::B],
                hold_ms: None,
            },
        )
        .await
        .expect_err("must fail");

        assert_eq!(err.to_payload().code, ErrorCode::BadRequest);
        assert!(mock.calls().is_empty(), "nothing may reach the OS");
    }

    #[tokio::test]
    async fn a_sequence_runs_its_combos_in_order() {
        let (shared, mock) = backend();
        key_sequence(
            &shared,
            KeySequence {
                steps: vec![
                    SequenceStep::Combo(KeyCombo {
                        keys: vec![Key::Control, Key::C],
                        hold_ms: None,
                    }),
                    SequenceStep::Delay { delay_ms: 1 },
                    SequenceStep::Combo(KeyCombo {
                        keys: vec![Key::Control, Key::V],
                        hold_ms: None,
                    }),
                ],
            },
        )
        .await
        .expect("sequence");

        assert_eq!(
            mock.calls(),
            vec![
                Call::KeyDown(Key::Control),
                Call::KeyDown(Key::C),
                Call::KeyUp(Key::C),
                Call::KeyUp(Key::Control),
                Call::KeyDown(Key::Control),
                Call::KeyDown(Key::V),
                Call::KeyUp(Key::V),
                Call::KeyUp(Key::Control),
            ]
        );
    }

    #[tokio::test]
    async fn an_absurdly_long_sequence_is_refused() {
        let (shared, mock) = backend();
        let err = key_sequence(
            &shared,
            KeySequence {
                steps: vec![SequenceStep::Delay {
                    delay_ms: 60 * 60 * 1000,
                }],
            },
        )
        .await
        .expect_err("must fail");

        assert_eq!(err.to_payload().code, ErrorCode::BadRequest);
        assert!(mock.calls().is_empty());
    }

    #[tokio::test]
    async fn text_reaches_the_backend_whole_when_there_is_no_delay() {
        let (shared, mock) = backend();
        text(
            &shared,
            TextRequest {
                text: "muxdeck".into(),
                delay_ms: None,
            },
        )
        .await
        .expect("text");

        assert_eq!(mock.calls(), vec![Call::Text("muxdeck".into())]);
    }

    #[tokio::test]
    async fn text_with_a_delay_is_split_per_character() {
        let (shared, mock) = backend();
        text(
            &shared,
            TextRequest {
                text: "ab".into(),
                delay_ms: Some(1),
            },
        )
        .await
        .expect("text");

        assert_eq!(
            mock.calls(),
            vec![Call::Text("a".into()), Call::Text("b".into())]
        );
    }

    #[tokio::test]
    async fn an_oversized_text_is_refused() {
        let (shared, mock) = backend();
        let err = text(
            &shared,
            TextRequest {
                text: "x".repeat(MAX_TEXT_LEN + 1),
                delay_ms: None,
            },
        )
        .await
        .expect_err("must fail");

        assert_eq!(err.to_payload().code, ErrorCode::BadRequest);
        assert!(mock.calls().is_empty());
    }

    #[tokio::test]
    async fn mouse_actions_map_onto_backend_events() {
        let (shared, mock) = backend();
        mouse(&shared, MouseRequest::MoveRel { dx: 12, dy: -4 })
            .await
            .expect("move");
        mouse(
            &shared,
            MouseRequest::Click {
                button: MouseButton::Left,
            },
        )
        .await
        .expect("click");
        mouse(&shared, MouseRequest::Scroll { dx: 0.0, dy: -1.0 })
            .await
            .expect("scroll");

        assert_eq!(
            mock.calls(),
            vec![
                Call::Mouse(MouseEvent::MoveRelative { dx: 12, dy: -4 }),
                Call::Mouse(MouseEvent::Click(MouseButton::Left)),
                Call::Mouse(MouseEvent::Scroll { dx: 0.0, dy: -1.0 }),
            ]
        );
    }
}
