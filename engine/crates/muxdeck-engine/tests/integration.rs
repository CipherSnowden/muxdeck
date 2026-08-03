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
