//! Round-trips every file in `protocol/fixtures/` and asserts semantic equality.
//!
//! The concrete payload type is chosen from `op` and `t` **only**. The variant suffix on a
//! filename plays no part in that decision — a loader that switched on it would have
//! reimplemented the protocol inside the test harness, and would then agree with itself
//! rather than with `docs/PROTOCOL.md` (§8).

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use muxdeck_core::*;

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../protocol/fixtures")
}

fn fixture_files() -> Vec<PathBuf> {
    let dir = fixture_dir();
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .map(|entry| entry.expect("unreadable dir entry").path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no fixtures found in {}", dir.display());
    files
}

/// Parse into `T`, serialise straight back out, and hand the result to the caller for
/// comparison against the original.
fn roundtrip<T: DeserializeOwned + Serialize>(raw: &str, what: &str) -> Value {
    let envelope: Envelope<T> =
        serde_json::from_str(raw).unwrap_or_else(|e| panic!("{what}: deserialise failed: {e}"));
    envelope
        .validate()
        .unwrap_or_else(|e| panic!("{what}: envelope failed validation: {e:?}"));
    serde_json::to_value(&envelope).unwrap_or_else(|e| panic!("{what}: serialise failed: {e}"))
}

/// The single source of the `(op, t) -> payload type` mapping.
fn reserialise(op: KnownOp, t: MessageType, raw: &str, what: &str) -> Value {
    use KnownOp as O;
    use MessageType as M;

    // An `err` carries the same payload whatever op it answers, so it is matched before
    // the op is consulted at all.
    if t == M::Err {
        return roundtrip::<ErrorPayload>(raw, what);
    }

    match (op, t) {
        (O::SessionHello, M::Req) => roundtrip::<HelloRequest>(raw, what),
        (O::SessionHello, M::Res) => roundtrip::<HelloResponse>(raw, what),
        (O::SessionAuth, M::Req) => roundtrip::<AuthRequest>(raw, what),
        (O::SessionAuth, M::Res) => roundtrip::<Ready>(raw, what),

        (O::PairRequest, M::Req) => roundtrip::<PairRequest>(raw, what),
        (O::PairRequest, M::Res) => roundtrip::<PairResponse>(raw, what),
        (O::PairBegin, M::Req) => roundtrip::<PairBeginRequest>(raw, what),
        (O::PairBegin, M::Res) => roundtrip::<PairBeginResponse>(raw, what),
        (O::PairCancel, M::Req | M::Res) => roundtrip::<Empty>(raw, what),
        (O::PairListDevices, M::Req) => roundtrip::<Empty>(raw, what),
        (O::PairListDevices, M::Res) => roundtrip::<PairListDevicesResponse>(raw, what),
        (O::PairRevoke, M::Req) => roundtrip::<PairRevokeRequest>(raw, what),
        (O::PairRevoke, M::Res) => roundtrip::<Empty>(raw, what),

        (O::SystemPing, M::Req) => roundtrip::<PingRequest>(raw, what),
        (O::SystemPing, M::Res) => roundtrip::<PingResponse>(raw, what),

        (O::InputKeyCombo, M::Req) => roundtrip::<KeyCombo>(raw, what),
        (O::InputKeySequence, M::Req) => roundtrip::<KeySequence>(raw, what),
        (O::InputText, M::Req) => roundtrip::<TextRequest>(raw, what),
        (O::InputMedia, M::Req) => roundtrip::<MediaRequest>(raw, what),
        (O::InputMouse, M::Req) => roundtrip::<MouseRequest>(raw, what),
        (
            O::InputKeyCombo | O::InputKeySequence | O::InputText | O::InputMedia | O::InputMouse,
            M::Res,
        ) => roundtrip::<Empty>(raw, what),

        (O::ActionRun, M::Req) => roundtrip::<ActionRunRequest>(raw, what),
        (O::ActionList, M::Req) => roundtrip::<Empty>(raw, what),
        (O::ActionList, M::Res) => roundtrip::<ActionListResponse>(raw, what),
        (O::ActionSet, M::Req) => roundtrip::<ActionSetRequest>(raw, what),
        (O::ActionDelete, M::Req) => roundtrip::<ActionDeleteRequest>(raw, what),
        (O::ActionRun | O::ActionSet | O::ActionDelete, M::Res) => roundtrip::<Empty>(raw, what),

        (O::ProfileGet, M::Req) => roundtrip::<ProfileGetRequest>(raw, what),
        (O::ProfileGet, M::Res) => roundtrip::<ProfileWrapper>(raw, what),
        (O::ProfileList, M::Req) => roundtrip::<Empty>(raw, what),
        (O::ProfileList, M::Res) => roundtrip::<ProfileListResponse>(raw, what),
        (O::ProfileSubscribe, M::Req | M::Res) => roundtrip::<Empty>(raw, what),
        (O::ProfileActivate, M::Req) => roundtrip::<ProfileActivateRequest>(raw, what),
        (O::ProfileSet, M::Req) => roundtrip::<ProfileWrapper>(raw, what),
        (O::ProfileDelete, M::Req) => roundtrip::<ProfileDeleteRequest>(raw, what),
        (O::ProfileActivate | O::ProfileSet | O::ProfileDelete, M::Res) => {
            roundtrip::<Empty>(raw, what)
        }

        (O::TelemetrySubscribe, M::Req | M::Res) => roundtrip::<Empty>(raw, what),

        (O::SettingsGet, M::Req) => roundtrip::<Empty>(raw, what),
        (O::SettingsGet, M::Res) => roundtrip::<Settings>(raw, what),
        (O::SettingsSet, M::Req) => roundtrip::<SettingsPatch>(raw, what),
        (O::SettingsSet, M::Res) => roundtrip::<SettingsSetResponse>(raw, what),

        (O::ProfileChanged, M::Evt) => roundtrip::<ProfileWrapper>(raw, what),
        (O::TelemetryUpdate, M::Evt) => roundtrip::<TelemetryUpdate>(raw, what),
        (O::DeviceChanged, M::Evt) => roundtrip::<DeviceChangedEvent>(raw, what),
        (O::PairingState, M::Evt) => roundtrip::<PairingState>(raw, what),
        (O::EngineShutdown, M::Evt) => roundtrip::<EngineShutdownEvent>(raw, what),

        _ => panic!(
            "{what}: no payload type mapped for ({}, {t:?})",
            op.as_str()
        ),
    }
}

#[test]
fn every_fixture_round_trips() {
    for path in fixture_files() {
        let what = path.file_name().unwrap().to_string_lossy().into_owned();
        let raw = fs::read_to_string(&path).expect("fixture unreadable");

        // The envelope is read twice on purpose: once loosely, to learn `op` and `t`, and
        // once into the concrete payload type those two select.
        let loose: Envelope<Value> =
            serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{what}: bad envelope: {e}"));
        let op = loose
            .op
            .known()
            .unwrap_or_else(|| panic!("{what}: fixture uses an op this build does not know"));

        let original: Value = serde_json::from_str(&raw).expect("fixture is not valid JSON");
        let actual = reserialise(op, loose.t, &raw, &what);

        assert_eq!(
            original, actual,
            "{what}: re-serialised form differs from the fixture"
        );
    }
}

#[test]
fn every_known_op_has_at_least_one_fixture() {
    let mut seen = BTreeSet::new();
    for path in fixture_files() {
        let raw = fs::read_to_string(&path).expect("fixture unreadable");
        let loose: Envelope<Value> = serde_json::from_str(&raw).expect("bad envelope");
        if let Some(op) = loose.op.known() {
            seen.insert(op.as_str());
        }
    }

    let missing: Vec<&str> = KnownOp::ALL
        .iter()
        .map(|op| op.as_str())
        .filter(|op| !seen.contains(op))
        .collect();

    assert!(
        missing.is_empty(),
        "ops with no fixture: {missing:?} — docs/PROTOCOL.md §8 requires one file per message shape"
    );
}

#[test]
fn events_never_carry_a_correlation_id_and_requests_always_do() {
    for path in fixture_files() {
        let what = path.file_name().unwrap().to_string_lossy().into_owned();
        let raw = fs::read_to_string(&path).expect("fixture unreadable");
        let loose: Envelope<Value> = serde_json::from_str(&raw).expect("bad envelope");

        match loose.t {
            MessageType::Evt => assert!(loose.id.is_none(), "{what}: an evt must have no id"),
            _ => assert!(loose.id.is_some(), "{what}: a req/res/err must have an id"),
        }

        if let Some(op) = loose.op.known() {
            assert_eq!(
                op.is_event(),
                loose.t == MessageType::Evt,
                "{what}: op {} and t {:?} disagree about whether this is an event",
                op.as_str(),
                loose.t
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Negative tests. A fixture suite that cannot fail is not testing anything.
// ---------------------------------------------------------------------------

#[test]
fn an_unsupported_version_is_rejected() {
    let raw = r#"{"v":2,"t":"req","id":"x","op":"system.ping","d":{"t_client":1}}"#;
    let envelope: Envelope<PingRequest> = serde_json::from_str(raw).expect("still parses");
    let err = envelope.validate().expect_err("v=2 must not validate");
    assert_eq!(err.code, ErrorCode::UnsupportedVersion);
}

#[test]
fn an_unknown_op_parses_into_a_rejectable_variant_rather_than_failing() {
    // Parsing must survive so the engine can answer UNKNOWN_OP while still echoing the
    // correlation ID. A hard parse error here would leave the client waiting forever.
    let raw = r#"{"v":1,"t":"req","id":"x","op":"input.telepathy","d":{}}"#;
    let envelope: Envelope<Value> = serde_json::from_str(raw).expect("must still parse");
    assert_eq!(envelope.op, Op::Unknown("input.telepathy".to_string()));
    assert!(envelope.op.known().is_none());
    assert_eq!(envelope.op.as_str(), "input.telepathy");
}

#[test]
fn a_missing_required_field_fails_the_parse() {
    // `keys` is mandatory on a key combo.
    let raw = r#"{"v":1,"t":"req","id":"x","op":"input.key_combo","d":{"hold_ms":0}}"#;
    let result: Result<Envelope<KeyCombo>, _> = serde_json::from_str(raw);
    assert!(
        result.is_err(),
        "a combo without `keys` must not deserialise"
    );
}

#[test]
fn an_event_carrying_a_correlation_id_is_rejected() {
    let raw = r#"{"v":1,"t":"evt","id":"x","op":"engine.shutdown","d":{"reason":"fatal_error"}}"#;
    let envelope: Envelope<EngineShutdownEvent> = serde_json::from_str(raw).expect("parses");
    let err = envelope
        .validate()
        .expect_err("an evt with an id must not validate");
    assert_eq!(err.code, ErrorCode::BadRequest);
}

#[test]
fn a_hello_response_with_an_unrecognised_mode_is_rejected() {
    // The tag is the only thing that picks a branch. An unrecognised value is a hard
    // failure, not a field to skip past — this is why the union is internally tagged
    // rather than untagged.
    let raw = r#"{"mode":"maybe","role":"deck"}"#;
    let result: Result<HelloResponse, _> = serde_json::from_str(raw);
    assert!(result.is_err(), "unknown `mode` must not deserialise");
}

#[test]
fn hello_requires_exactly_one_of_device_id_and_admin_token() {
    let both = HelloRequest {
        device_id: Some("d_1".into()),
        admin_token: Some("t".into()),
        client_version: "0.1.0".into(),
        platform: Platform::Windows,
    };
    assert_eq!(
        both.validate().expect_err("both must fail").code,
        ErrorCode::BadRequest
    );

    let neither = HelloRequest {
        device_id: None,
        admin_token: None,
        client_version: "0.1.0".into(),
        platform: Platform::Windows,
    };
    assert_eq!(
        neither.validate().expect_err("neither must fail").code,
        ErrorCode::BadRequest
    );
}

#[test]
fn ready_serialises_without_a_mode_field_but_gains_one_through_the_union() {
    let ready = Ready {
        role: Role::Deck,
        protocol: 1,
        engine_version: "0.1.0".into(),
        host_platform: HostPlatform::Linux,
        active_profile_id: "p_default".into(),
        capabilities: Capabilities {
            text_unicode: false,
            media_keys: true,
            mouse: true,
            shell_actions: false,
        },
    };

    let bare = serde_json::to_value(&ready).unwrap();
    assert!(
        bare.get("mode").is_none(),
        "Ready has no mode field of its own — session.auth returns it untagged"
    );

    let tagged = serde_json::to_value(HelloResponse::Ready(ready)).unwrap();
    assert_eq!(tagged.get("mode").and_then(Value::as_str), Some("ready"));
}

// ---------------------------------------------------------------------------
// Signing layouts, checked as raw bytes rather than as JSON.
// ---------------------------------------------------------------------------

fn from_hex(s: &str) -> Vec<u8> {
    assert!(s.len().is_multiple_of(2), "hex string has an odd length");
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("bad hex digit"))
        .collect()
}

fn signing_fixture(name: &str) -> Value {
    let path = fixture_dir().join("signing").join(name);
    let raw = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&raw).expect("signing fixture is not valid JSON")
}

#[test]
fn session_auth_signing_layout_matches_the_fixture() {
    let f = signing_fixture("session_auth.json");
    let nonce = from_hex(f["nonce_hex"].as_str().unwrap());
    let expected = from_hex(f["message_hex"].as_str().unwrap());

    let actual = muxdeck_core::signing::session_auth_message(
        &nonce,
        f["device_id"].as_str().unwrap(),
        f["host_id"].as_str().unwrap(),
    );

    assert_eq!(actual.len() as u64, f["message_len"].as_u64().unwrap());
    assert_eq!(actual, expected, "session.auth signing buffer differs");
}

#[test]
fn pair_proof_signing_layout_matches_the_fixture() {
    let f = signing_fixture("pair_proof.json");
    let pubkey = from_hex(f["device_pubkey_hex"].as_str().unwrap());
    let expected = from_hex(f["message_hex"].as_str().unwrap());

    let actual = muxdeck_core::signing::pair_proof_message(f["code"].as_str().unwrap(), &pubkey);

    assert_eq!(actual.len() as u64, f["message_len"].as_u64().unwrap());
    assert_eq!(
        actual, expected,
        "pair.request proof signing buffer differs"
    );
}
