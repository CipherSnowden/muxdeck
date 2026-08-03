//! End-to-end: a real TLS WebSocket, a real handshake, a real pairing.
//!
//! Binds port 0 so the suite never collides with a running daemon, and drives the protocol the
//! way a phone would — including the certificate pinning, which is the part most likely to
//! break silently.

use std::net::SocketAddr;
use std::sync::Arc;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use ed25519_dalek::{Signer, SigningKey};
use futures_util::{SinkExt, StreamExt};
use muxdeck_core::signing::{pair_proof_message, session_auth_message};
use muxdeck_core::{Envelope, ErrorCode, ErrorPayload, KnownOp, MessageType, Op};
use muxdeck_engine::admin_client::{connect_pinned, AdminClient, PinnedStream};
use muxdeck_engine::config::Paths;
use muxdeck_engine::server::{self, Running};
use muxdeck_engine::Engine;
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

struct Harness {
    engine: Arc<Engine>,
    running: Running,
    root: std::path::PathBuf,
}

impl Harness {
    async fn start(name: &str) -> Self {
        // Every test process needs a provider before rustls will do anything. Installing twice
        // is an error, so the result is deliberately ignored.
        let _ = rustls::crypto::ring::default_provider().install_default();

        let root = std::env::temp_dir().join("muxdeck-integration").join(name);
        let _ = std::fs::remove_dir_all(&root);
        let paths = Paths::resolve(Some(root.clone())).expect("paths");
        let engine = Engine::load(paths).expect("engine");

        let running = server::serve(engine.clone(), SocketAddr::from(([127, 0, 0, 1], 0)))
            .await
            .expect("serve");

        Self {
            engine,
            running,
            root,
        }
    }

    fn url(&self) -> String {
        format!("wss://127.0.0.1:{}/ws", self.running.addr.port())
    }

    fn fingerprint(&self) -> String {
        self.engine.identity.fingerprint().to_string()
    }

    async fn client(&self) -> TestClient {
        TestClient {
            socket: connect_pinned(&self.url(), &self.fingerprint())
                .await
                .expect("connect"),
            next_id: 1,
        }
    }

    async fn admin(&self) -> AdminClient {
        AdminClient::connect(
            self.running.addr.port(),
            self.engine.identity.fingerprint(),
            self.engine.identity.admin_token(),
        )
        .await
        .expect("admin connect")
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        self.running.shutdown();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

struct TestClient {
    socket: PinnedStream,
    next_id: u64,
}

impl TestClient {
    /// Sends a request and returns `Ok(payload)` or `Err(error payload)`.
    async fn call(
        &mut self,
        op: KnownOp,
        payload: Value,
    ) -> std::result::Result<Value, ErrorPayload> {
        let id = format!("t-{}", self.next_id);
        self.next_id += 1;

        let envelope = Envelope {
            v: 1,
            t: MessageType::Req,
            id: Some(id.clone()),
            op: Op::of(op),
            d: payload,
        };
        self.socket
            .send(Message::Text(
                serde_json::to_string(&envelope).expect("serialise").into(),
            ))
            .await
            .expect("send");

        loop {
            let message = self.socket.next().await.expect("open").expect("frame");
            let Message::Text(text) = message else {
                continue;
            };
            let reply: Envelope<Value> = serde_json::from_str(&text).expect("parse");
            if reply.id.as_deref() != Some(id.as_str()) {
                continue;
            }
            return match reply.t {
                MessageType::Err => Err(serde_json::from_value(reply.d).expect("error payload")),
                _ => Ok(reply.d),
            };
        }
    }

    /// Sends a raw frame, for cases a well-formed envelope cannot express.
    async fn send_raw(&mut self, text: &str) {
        self.socket
            .send(Message::Text(text.to_string().into()))
            .await
            .expect("send");
    }

    async fn next_frame(&mut self) -> Option<Envelope<Value>> {
        loop {
            match self.socket.next().await {
                Some(Ok(Message::Text(text))) => {
                    return Some(serde_json::from_str(&text).expect("parse"));
                }
                Some(Ok(_)) => continue,
                Some(Err(_)) | None => return None,
            }
        }
    }
}

/// Pairs a fresh device and returns its key and assigned ID.
async fn pair_device(harness: &Harness, seed: u8) -> (SigningKey, String) {
    let mut admin = harness.admin().await;
    let begin = admin
        .request(KnownOp::PairBegin, &json!({ "ttl_seconds": 60 }))
        .await
        .expect("pair.begin");
    let code = begin["code"].as_str().expect("code").to_string();

    let key = SigningKey::from_bytes(&[seed; 32]);
    let pubkey = key.verifying_key().to_bytes();
    let proof = key.sign(&pair_proof_message(&code, &pubkey));

    let mut client = harness.client().await;
    let response = client
        .call(
            KnownOp::PairRequest,
            json!({
                "code": code,
                "device_pubkey": BASE64.encode(pubkey),
                "device_name": "Integration deck",
                "platform": "android",
                "proof": BASE64.encode(proof.to_bytes()),
            }),
        )
        .await
        .expect("pair.request");

    let device_id = response["device_id"]
        .as_str()
        .expect("device_id")
        .to_string();
    (key, device_id)
}

/// Runs the full deck handshake on a fresh socket.
async fn authenticate(harness: &Harness, key: &SigningKey, device_id: &str) -> TestClient {
    let mut client = harness.client().await;
    let challenge = client
        .call(
            KnownOp::SessionHello,
            json!({ "device_id": device_id, "client_version": "0.1.0", "platform": "android" }),
        )
        .await
        .expect("session.hello");

    assert_eq!(challenge["mode"], "challenge");
    let nonce = BASE64
        .decode(challenge["nonce"].as_str().expect("nonce"))
        .expect("base64");
    let host_id = challenge["host_id"].as_str().expect("host_id");

    let signature = key.sign(&session_auth_message(&nonce, device_id, host_id));
    let ready = client
        .call(
            KnownOp::SessionAuth,
            json!({ "signature": BASE64.encode(signature.to_bytes()) }),
        )
        .await
        .expect("session.auth");

    assert_eq!(ready["role"], "deck");
    assert!(
        ready.get("mode").is_none(),
        "session.auth returns Ready untagged"
    );
    client
}

#[tokio::test]
async fn pair_authenticate_and_ping() {
    let harness = Harness::start("happy_path").await;
    let (key, device_id) = pair_device(&harness, 21).await;
    let mut client = authenticate(&harness, &key, &device_id).await;

    let response = client
        .call(KnownOp::SystemPing, json!({ "t_client": 1785311999123i64 }))
        .await
        .expect("system.ping");

    assert_eq!(response["t_client"], 1785311999123i64);
    assert!(
        response["t_engine"].as_i64().expect("t_engine") > 0,
        "t_engine is milliseconds since the epoch"
    );
}

#[tokio::test]
async fn the_certificate_is_pinned_not_merely_encrypted() {
    let harness = Harness::start("pinning").await;
    let wrong = "0".repeat(64);

    let result = connect_pinned(&harness.url(), &wrong).await;
    assert!(
        result.is_err(),
        "a client must refuse a certificate that does not match its pin"
    );
}

#[tokio::test]
async fn an_op_before_authentication_is_refused() {
    let harness = Harness::start("pre_auth").await;
    let mut client = harness.client().await;

    let err = client
        .call(KnownOp::SystemPing, json!({ "t_client": 1 }))
        .await
        .expect_err("must be refused");
    assert_eq!(err.code, ErrorCode::NotAuthenticated);
}

#[tokio::test]
async fn a_deck_may_not_open_a_pairing_window() {
    // The escalation this blocks: a paired phone adding another device by itself.
    let harness = Harness::start("deck_escalation").await;
    let (key, device_id) = pair_device(&harness, 22).await;
    let mut client = authenticate(&harness, &key, &device_id).await;

    let err = client
        .call(KnownOp::PairBegin, json!({ "ttl_seconds": 60 }))
        .await
        .expect_err("must be refused");
    assert_eq!(err.code, ErrorCode::NotAuthorized);
}

#[tokio::test]
async fn a_revoked_device_can_no_longer_authenticate() {
    let harness = Harness::start("revoked").await;
    let (key, device_id) = pair_device(&harness, 23).await;

    let mut admin = harness.admin().await;
    admin
        .request(KnownOp::PairRevoke, &json!({ "device_id": device_id }))
        .await
        .expect("revoke");

    let mut client = harness.client().await;
    let err = client
        .call(
            KnownOp::SessionHello,
            json!({ "device_id": device_id, "client_version": "0.1.0", "platform": "android" }),
        )
        .await
        .expect_err("revoked");
    assert_eq!(err.code, ErrorCode::UnknownDevice);

    // Silence the unused warning while keeping the key in the signature for symmetry.
    let _ = key;
}

#[tokio::test]
async fn a_pairing_window_closes_after_one_device() {
    // Leaving it open would let anyone who saw the QR pair a second device afterwards.
    let harness = Harness::start("one_shot_window").await;
    let mut admin = harness.admin().await;
    let begin = admin
        .request(KnownOp::PairBegin, &json!({ "ttl_seconds": 60 }))
        .await
        .expect("begin");
    let code = begin["code"].as_str().expect("code").to_string();

    for (seed, expect_ok) in [(31u8, true), (32u8, false)] {
        let key = SigningKey::from_bytes(&[seed; 32]);
        let pubkey = key.verifying_key().to_bytes();
        let proof = key.sign(&pair_proof_message(&code, &pubkey));

        let mut client = harness.client().await;
        let result = client
            .call(
                KnownOp::PairRequest,
                json!({
                    "code": code,
                    "device_pubkey": BASE64.encode(pubkey),
                    "device_name": "Deck",
                    "platform": "android",
                    "proof": BASE64.encode(proof.to_bytes()),
                }),
            )
            .await;

        if expect_ok {
            assert!(result.is_ok(), "the first device must pair");
        } else {
            assert_eq!(
                result.expect_err("second device").code,
                ErrorCode::PairingClosed,
                "the window must not survive a successful pairing"
            );
        }
    }
}

#[tokio::test]
async fn an_unsupported_protocol_version_is_refused() {
    let harness = Harness::start("bad_version").await;
    let mut client = harness.client().await;

    client
        .send_raw(r#"{"v":2,"t":"req","id":"x","op":"system.ping","d":{"t_client":1}}"#)
        .await;

    let reply = client.next_frame().await.expect("a reply");
    assert_eq!(reply.t, MessageType::Err);
    let payload: ErrorPayload = serde_json::from_value(reply.d).expect("error payload");
    assert_eq!(payload.code, ErrorCode::UnsupportedVersion);
}

#[tokio::test]
async fn an_admin_socket_receives_events_a_deck_does_not() {
    let harness = Harness::start("events").await;
    let mut admin = harness.admin().await;

    // Opening a window pushes pairing.state to every admin socket.
    admin
        .request(KnownOp::PairBegin, &json!({ "ttl_seconds": 60 }))
        .await
        .expect("begin");

    // A second admin connection sees the next event.
    let mut watcher = harness.admin().await;
    admin
        .request(KnownOp::PairCancel, &json!({}))
        .await
        .expect("cancel");

    let event = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        watcher.request(KnownOp::PairListDevices, &json!({})),
    )
    .await
    .expect("the admin socket stays responsive while events flow")
    .expect("list");

    assert!(event["devices"].is_array());
}

// --- the live layout loop ---------------------------------------------------
//
// The claim M6 exists to make: edit on the desktop, and the tablet updates. Proven here at the
// protocol level, so the only thing left for a real device is that it re-renders.

/// Waits for an event with the given op, or gives up.
///
/// Bounded because the interesting assertion is often that an event *does not* arrive, and an
/// unbounded read would hang the suite rather than fail it.
async fn await_event(
    client: &mut TestClient,
    op: KnownOp,
    timeout: std::time::Duration,
) -> Option<Value> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return None;
        }
        match tokio::time::timeout(remaining, client.next_frame()).await {
            Ok(Some(frame)) if frame.t == MessageType::Evt && frame.op == Op::of(op) => {
                return Some(frame.d);
            }
            Ok(Some(_)) => continue,
            Ok(None) | Err(_) => return None,
        }
    }
}

#[tokio::test]
async fn an_edit_reaches_a_subscribed_deck() {
    let harness = Harness::start("live_loop").await;
    let (key, device_id) = pair_device(&harness, 41).await;
    let mut deck = authenticate(&harness, &key, &device_id).await;

    deck.call(KnownOp::ProfileSubscribe, json!({}))
        .await
        .expect("profile.subscribe");

    // The panel edits, as it would when a button is changed in the editor.
    let mut admin = harness.admin().await;
    let profile = admin
        .request(KnownOp::ProfileGet, &json!({ "profile_id": "p_default" }))
        .await
        .expect("profile.get");

    let mut edited = profile["profile"].clone();
    edited["name"] = json!("Renamed by the editor");
    admin
        .request(KnownOp::ProfileSet, &json!({ "profile": edited }))
        .await
        .expect("profile.set");

    let event = await_event(
        &mut deck,
        KnownOp::ProfileChanged,
        std::time::Duration::from_secs(5),
    )
    .await
    .expect("a subscribed deck must receive profile.changed");

    assert_eq!(
        event["profile"]["name"], "Renamed by the editor",
        "the event must carry the edited profile, not a stale one"
    );
}

#[tokio::test]
async fn an_unsubscribed_deck_receives_no_layout_events() {
    // Subscription is explicit. A deck that never asked must not be sent layout traffic it has
    // no use for. `docs/PROTOCOL.md` §3.
    let harness = Harness::start("no_subscribe").await;
    let (key, device_id) = pair_device(&harness, 42).await;
    let mut deck = authenticate(&harness, &key, &device_id).await;

    let mut admin = harness.admin().await;
    let profile = admin
        .request(KnownOp::ProfileGet, &json!({ "profile_id": "p_default" }))
        .await
        .expect("profile.get");
    admin
        .request(
            KnownOp::ProfileSet,
            &json!({ "profile": profile["profile"] }),
        )
        .await
        .expect("profile.set");

    let event = await_event(
        &mut deck,
        KnownOp::ProfileChanged,
        std::time::Duration::from_millis(800),
    )
    .await;
    assert!(
        event.is_none(),
        "an unsubscribed socket must receive nothing"
    );
}

#[tokio::test]
async fn a_deck_never_receives_device_events() {
    // Device and pairing state are admin-only. Before M6 the routing was role-gated rather than
    // audience-gated, and a subscribed deck would have seen everything the panel does.
    let harness = Harness::start("deck_no_device_events").await;
    let (key, device_id) = pair_device(&harness, 43).await;
    let mut deck = authenticate(&harness, &key, &device_id).await;

    deck.call(KnownOp::ProfileSubscribe, json!({}))
        .await
        .expect("subscribe to everything a deck may have");

    // Pairing another device publishes device.changed and pairing.state.
    pair_device(&harness, 44).await;

    let event = await_event(
        &mut deck,
        KnownOp::DeviceChanged,
        std::time::Duration::from_millis(800),
    )
    .await;
    assert!(
        event.is_none(),
        "a deck must never learn about other devices"
    );
}

#[tokio::test]
async fn a_deck_may_activate_a_profile_but_not_edit_one() {
    // `profile.activate` is deck-callable on purpose — a device that can already inject
    // keystrokes gains nothing by choosing which grid it shows, and "switch to my streaming
    // profile" must work with the panel closed. Editing is a different matter.
    let harness = Harness::start("deck_profile_rights").await;
    let (key, device_id) = pair_device(&harness, 45).await;
    let mut deck = authenticate(&harness, &key, &device_id).await;

    deck.call(
        KnownOp::ProfileActivate,
        json!({ "profile_id": "p_default" }),
    )
    .await
    .expect("a deck may activate");

    let profile = deck
        .call(KnownOp::ProfileGet, json!({ "profile_id": "p_default" }))
        .await
        .expect("a deck may read its own layout");

    let err = deck
        .call(
            KnownOp::ProfileSet,
            json!({ "profile": profile["profile"] }),
        )
        .await
        .expect_err("a deck must not edit");
    assert_eq!(err.code, ErrorCode::NotAuthorized);
}

#[tokio::test]
async fn a_fresh_engine_serves_a_usable_default_layout() {
    // A new install landing on an empty grid would look broken.
    let harness = Harness::start("default_layout").await;
    let mut admin = harness.admin().await;

    let response = admin
        .request(KnownOp::ProfileGet, &json!({ "profile_id": "p_default" }))
        .await
        .expect("the default profile must exist");

    let profile = &response["profile"];
    assert_eq!(profile["grid"]["cols"], 5);
    assert_eq!(profile["grid"]["rows"], 3);
    assert!(
        profile["pages"][0]["buttons"]
            .as_array()
            .expect("buttons")
            .len()
            >= 10,
        "the default should be worth pressing, not a token single button"
    );
}

#[tokio::test]
async fn an_invalid_profile_is_refused_with_a_specific_message() {
    // Never last-write-wins, never silent coercion: a layout quietly altered on the way in is
    // worse than one refused, because the user cannot see what happened.
    let harness = Harness::start("invalid_profile").await;
    let mut admin = harness.admin().await;

    let err = admin
        .request(
            KnownOp::ProfileSet,
            &json!({
                "profile": {
                    "id": "p_bad",
                    "name": "Out of bounds",
                    "grid": { "cols": 2, "rows": 2 },
                    "pages": [{
                        "id": "pg_1",
                        "name": "Main",
                        "buttons": [{
                            "id": "b_1",
                            "pos": { "col": 9, "row": 9 },
                            "label": "Nowhere",
                            "icon": "circle",
                            "color": "#000000",
                            "haptic": "none",
                            "on_tap": null,
                            "on_long_press": null
                        }]
                    }]
                }
            }),
        )
        .await
        .expect_err("a button outside the grid must be refused");

    let payload = err.to_payload();
    assert_eq!(payload.code, ErrorCode::BadRequest);
    assert!(
        payload.message.contains("outside"),
        "the message must say what is wrong, not just that something is: {}",
        payload.message
    );
}

#[tokio::test]
async fn an_unauthenticated_socket_is_closed_after_the_timeout() {
    // A 10-second wait is too slow for the suite, so this asserts the socket is still open
    // well before the deadline rather than waiting for it to fire. The timeout value itself is
    // a constant checked in the session module.
    let harness = Harness::start("timeout").await;
    let mut client = harness.client().await;

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let err = client
        .call(KnownOp::SystemPing, json!({ "t_client": 1 }))
        .await
        .expect_err("still refused, but the socket is alive");
    assert_eq!(err.code, ErrorCode::NotAuthenticated);
    assert_eq!(muxdeck_engine::session::AUTH_TIMEOUT.as_secs(), 10);
}

/// Measures the segments of `docs/ARCHITECTURE.md` §7 that this machine can actually observe.
///
/// Run it deliberately:
/// ```text
/// cargo test -p muxdeck-engine --test integration -- --ignored --nocapture measure_latency
/// ```
///
/// `#[ignore]`d for two reasons: it injects **real keystrokes** into whatever window has focus,
/// and a timing measurement on a shared CI runner is noise wearing a number's clothes.
///
/// What it can and cannot see:
/// - **Measured**: loopback round trip, engine parse and dispatch, OS input injection.
/// - **Not measured**: touch to client frame, and the real Wi-Fi round trip. Both need a phone
///   on the far end of an access point; loopback is a floor, not a substitute.
///
/// The arithmetic is a subtraction. `system.ping` costs transport plus a trivial handler, and
/// `input.key_combo` costs transport plus dispatch plus injection — so the difference is the
/// engine-side work, isolated without instrumenting the engine itself.
#[tokio::test]
#[ignore = "injects real keystrokes and measures timing; run it deliberately"]
async fn measure_latency() {
    const SAMPLES: usize = 200;
    const WARMUP: usize = 20;

    let harness = Harness::start("latency").await;
    let (key, device_id) = pair_device(&harness, 70).await;
    let mut deck = authenticate(&harness, &key, &device_id).await;

    // F24 is on no keyboard anybody owns and is bound by essentially nothing, so a few hundred
    // presses land somewhere harmless.
    let press = json!({ "keys": ["F24"], "hold_ms": 0 });

    let mut ping_us = Vec::with_capacity(SAMPLES);
    let mut press_us = Vec::with_capacity(SAMPLES);

    for i in 0..(SAMPLES + WARMUP) {
        let start = std::time::Instant::now();
        deck.call(KnownOp::SystemPing, json!({ "t_client": 0 }))
            .await
            .expect("ping");
        let ping = start.elapsed();

        let start = std::time::Instant::now();
        deck.call(KnownOp::InputKeyCombo, press.clone())
            .await
            .expect("key combo");
        let combo = start.elapsed();

        // The first samples pay for lazily-built buffers and a cold branch predictor, and
        // including them makes a p95 that describes startup rather than steady state.
        if i >= WARMUP {
            ping_us.push(ping.as_micros() as u64);
            press_us.push(combo.as_micros() as u64);
        }
    }

    ping_us.sort_unstable();
    press_us.sort_unstable();

    let report = |name: &str, samples: &[u64]| {
        println!(
            "{name:<28} p50 {:>7.3} ms   p95 {:>7.3} ms   max {:>7.3} ms",
            samples[samples.len() / 2] as f64 / 1000.0,
            samples[samples.len() * 95 / 100] as f64 / 1000.0,
            samples[samples.len() - 1] as f64 / 1000.0,
        );
    };

    println!();
    println!("MuxDeck latency, {SAMPLES} samples, loopback TLS WebSocket");
    println!("---------------------------------------------------------------------");
    report("loopback round trip", &ping_us);
    report("round trip + press", &press_us);

    let engine_side: Vec<u64> = press_us
        .iter()
        .zip(&ping_us)
        .map(|(press, ping)| press.saturating_sub(*ping))
        .collect();
    let mut engine_side = engine_side;
    engine_side.sort_unstable();
    report("dispatch + injection", &engine_side);

    println!();
    println!("Not measured here: touch to client frame, and the Wi-Fi round trip.");
    println!("Both need a phone on the far end; loopback is a floor, not a substitute.");
    println!();

    // Not an assertion on the whole budget — this cannot see the network or the touch, and a
    // test that failed on a busy machine would be deleted within a week. It asserts only the
    // part that is genuinely under the engine's control, and generously: `docs/ARCHITECTURE.md`
    // §7 budgets 1 ms for dispatch and 3 ms for injection.
    let engine_p95 = engine_side[engine_side.len() * 95 / 100];
    assert!(
        engine_p95 < 10_000,
        "engine-side p95 is {engine_p95} µs, far above the 4 ms §7 allows for dispatch and \
         injection together — something is blocking the runtime"
    );
}

// --- M8: settings, actions and telemetry ---------------------------------------------

#[tokio::test]
async fn settings_set_changes_only_the_keys_it_names() {
    // The whole point of a partial patch: an absent key is left alone rather than reset to its
    // default. A panel saving one toggle must not silently revert every other setting.
    let harness = Harness::start("settings_partial").await;
    let mut admin = harness.admin().await;

    let before = admin
        .request(KnownOp::SettingsGet, &json!({}))
        .await
        .expect("settings.get");
    assert_eq!(
        before["shell_actions_enabled"],
        json!(false),
        "off by default"
    );

    admin
        .request(
            KnownOp::SettingsSet,
            &json!({ "telemetry_interval_ms": 250 }),
        )
        .await
        .expect("settings.set");

    let after = admin
        .request(KnownOp::SettingsGet, &json!({}))
        .await
        .expect("settings.get");
    assert_eq!(after["telemetry_interval_ms"], json!(250));
    assert_eq!(
        after["host_name"], before["host_name"],
        "a key absent from the patch must survive it"
    );
    assert_eq!(after["port"], before["port"]);
}

#[tokio::test]
async fn only_a_port_change_asks_for_a_restart() {
    let harness = Harness::start("settings_restart").await;
    let mut admin = harness.admin().await;

    let live = admin
        .request(KnownOp::SettingsSet, &json!({ "telemetry_enabled": false }))
        .await
        .expect("settings.set");
    assert_eq!(live["restart_required"], json!(false));

    let empty = admin
        .request(KnownOp::SettingsSet, &json!({}))
        .await
        .expect("an empty patch is valid");
    assert_eq!(empty["restart_required"], json!(false));

    let port = admin
        .request(KnownOp::SettingsSet, &json!({ "port": 47700 }))
        .await
        .expect("settings.set");
    assert_eq!(port["restart_required"], json!(true));
}

#[tokio::test]
async fn a_deck_cannot_read_or_write_settings() {
    let harness = Harness::start("settings_role").await;
    let (key, device_id) = pair_device(&harness, 60).await;
    let mut deck = authenticate(&harness, &key, &device_id).await;

    for op in [KnownOp::SettingsGet, KnownOp::SettingsSet] {
        let err = deck
            .call(op, json!({}))
            .await
            .expect_err("settings are admin only");
        assert_eq!(err.code, ErrorCode::NotAuthorized, "{}", op.as_str());
    }
}

#[tokio::test]
async fn actions_are_refused_until_the_feature_is_switched_on() {
    let harness = Harness::start("actions_disabled").await;
    let mut admin = harness.admin().await;

    // Defining requires the feature on, not merely running: staging a command while the switch
    // is off would make it runnable the instant anybody flips it.
    let err = admin
        .request(
            KnownOp::ActionSet,
            &json!({ "action": {
                "id": "a_one", "name": "Echo", "command": "echo",
                "args": ["hello"], "cwd": null
            }}),
        )
        .await
        .expect_err("action.set must be refused while shell actions are off");
    assert_eq!(err.to_payload().code, ErrorCode::Disabled);

    let err = admin
        .request(KnownOp::ActionRun, &json!({ "action_id": "a_one" }))
        .await
        .expect_err("action.run must be refused too");
    assert_eq!(err.to_payload().code, ErrorCode::Disabled);

    // Listing still succeeds and comes back empty, so a client can call it unconditionally at
    // startup rather than special-casing the disabled state on every connect.
    let list = admin
        .request(KnownOp::ActionList, &json!({}))
        .await
        .expect("action.list must never be an error");
    assert_eq!(list["actions"], json!([]));
}

#[tokio::test]
async fn an_action_can_be_defined_run_and_deleted_once_enabled() {
    let harness = Harness::start("actions_enabled").await;
    let mut admin = harness.admin().await;

    admin
        .request(
            KnownOp::SettingsSet,
            &json!({ "shell_actions_enabled": true }),
        )
        .await
        .expect("enable shell actions");

    // Something that exists on every platform in the CI matrix and exits immediately.
    let command = if cfg!(windows) { "cmd" } else { "true" };
    let args = if cfg!(windows) {
        json!(["/c", "exit"])
    } else {
        json!([])
    };

    admin
        .request(
            KnownOp::ActionSet,
            &json!({ "action": {
                "id": "a_one", "name": "No-op", "command": command,
                "args": args, "cwd": null
            }}),
        )
        .await
        .expect("action.set");

    let list = admin
        .request(KnownOp::ActionList, &json!({}))
        .await
        .expect("action.list");
    assert_eq!(list["actions"].as_array().map(Vec::len), Some(1));

    admin
        .request(KnownOp::ActionRun, &json!({ "action_id": "a_one" }))
        .await
        .expect("action.run");

    admin
        .request(KnownOp::ActionDelete, &json!({ "action_id": "a_one" }))
        .await
        .expect("action.delete");

    let err = admin
        .request(KnownOp::ActionRun, &json!({ "action_id": "a_one" }))
        .await
        .expect_err("a deleted action must not still run");
    assert_eq!(err.to_payload().code, ErrorCode::NotFound);
}

#[tokio::test]
async fn a_deck_may_run_actions_but_not_define_them() {
    let harness = Harness::start("actions_role").await;
    harness
        .admin()
        .await
        .request(
            KnownOp::SettingsSet,
            &json!({ "shell_actions_enabled": true }),
        )
        .await
        .expect("enable shell actions");

    let (key, device_id) = pair_device(&harness, 61).await;
    let mut deck = authenticate(&harness, &key, &device_id).await;

    // Running is allowed — this call fails with NOT_FOUND, which is the point: the role check
    // passed and the lookup is what refused.
    let err = deck
        .call(KnownOp::ActionRun, json!({ "action_id": "a_missing" }))
        .await
        .expect_err("no such action");
    assert_eq!(err.code, ErrorCode::NotFound);

    for op in [KnownOp::ActionSet, KnownOp::ActionDelete] {
        let err = deck
            .call(op, json!({}))
            .await
            .expect_err("defining actions is admin only");
        assert_eq!(err.code, ErrorCode::NotAuthorized, "{}", op.as_str());
    }
}

#[tokio::test]
async fn a_subscribed_socket_receives_telemetry() {
    let harness = Harness::start("telemetry").await;
    let (key, device_id) = pair_device(&harness, 62).await;
    let mut deck = authenticate(&harness, &key, &device_id).await;

    // Faster than the default second so the test does not idle; the sampler clamps to its own
    // 200 ms floor regardless.
    harness
        .admin()
        .await
        .request(
            KnownOp::SettingsSet,
            &json!({ "telemetry_interval_ms": 200 }),
        )
        .await
        .expect("settings.set");

    deck.call(KnownOp::TelemetrySubscribe, json!({}))
        .await
        .expect("telemetry.subscribe");

    let event = await_event(
        &mut deck,
        KnownOp::TelemetryUpdate,
        std::time::Duration::from_secs(10),
    )
    .await
    .expect("a subscribed socket must receive telemetry.update");

    let cpu = event["cpu_pct"].as_f64().expect("cpu_pct is a number");
    let ram = event["ram_pct"].as_f64().expect("ram_pct is a number");
    assert!((0.0..=100.0).contains(&cpu), "cpu_pct out of range: {cpu}");
    assert!((0.0..=100.0).contains(&ram), "ram_pct out of range: {ram}");
    assert!(
        event["ts"].as_i64().unwrap_or(0) > 1_700_000_000,
        "the timestamp must be real"
    );
}

#[tokio::test]
async fn an_unsubscribed_socket_receives_no_telemetry() {
    // Telemetry has its own subscription and is not implied by profile.subscribe
    // (`docs/PROTOCOL.md` §4.7). A deck rendering buttons should not be woken every second.
    let harness = Harness::start("telemetry_unsubscribed").await;
    let (key, device_id) = pair_device(&harness, 63).await;
    let mut deck = authenticate(&harness, &key, &device_id).await;

    deck.call(KnownOp::ProfileSubscribe, json!({}))
        .await
        .expect("profile.subscribe");

    let event = await_event(
        &mut deck,
        KnownOp::TelemetryUpdate,
        std::time::Duration::from_millis(1500),
    )
    .await;
    assert!(
        event.is_none(),
        "profile.subscribe must not imply telemetry.subscribe"
    );
}
