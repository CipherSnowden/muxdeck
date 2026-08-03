//! Exercises a running `muxdeckd` from the terminal: pair a fake device, authenticate, ping.
//!
//! ```powershell
//! cargo run -p muxdeckd -- --foreground --log-level debug   # terminal 1
//! cargo run -p muxdeck-engine --example probe               # terminal 2
//! ```
//!
//! The point is to have something that proves the engine works before any UI exists. It uses a
//! fixed key seed, so re-running it re-pairs the same fake device rather than accumulating
//! junk in the registry.

use std::time::Instant;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use ed25519_dalek::{Signer, SigningKey};
use futures_util::{SinkExt, StreamExt};
use muxdeck_core::signing::{pair_proof_message, session_auth_message};
use muxdeck_core::{Envelope, ErrorPayload, KnownOp, MessageType, Op};
use muxdeck_engine::admin_client::{connect_pinned, AdminClient, PinnedStream};
use muxdeck_engine::config::Paths;
use muxdeck_engine::registry::unix_now_millis;
use muxdeck_engine::Engine;
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

/// The fake device this probe pretends to be.
const DEVICE_SEED: u8 = 0xAB;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|_| "a rustls crypto provider was already installed")?;

    let paths = Paths::resolve(None)?;
    let engine = Engine::load(paths)?;
    let port = engine.settings().port;
    let fingerprint = engine.identity.fingerprint().to_string();

    println!("host      {}", engine.identity.host_id());
    println!("fp        {fingerprint}");
    println!("port      {port}");
    println!();

    // 1. Open a pairing window as the control panel would.
    let mut admin = AdminClient::connect(port, &fingerprint, engine.identity.admin_token()).await?;
    println!("connected as admin, engine {}", admin.ready.engine_version);

    let begin = admin
        .request(KnownOp::PairBegin, &json!({ "ttl_seconds": 60 }))
        .await?;
    let code = begin["code"].as_str().ok_or("no pairing code")?.to_string();
    println!("pairing code {code}");

    // 2. Pair, proving possession of the device key.
    let key = SigningKey::from_bytes(&[DEVICE_SEED; 32]);
    let pubkey = key.verifying_key().to_bytes();
    let proof = key.sign(&pair_proof_message(&code, &pubkey));

    let mut deck = Probe::new(connect_pinned(&url(port), &fingerprint).await?);
    let paired = deck
        .call(
            KnownOp::PairRequest,
            json!({
                "code": code,
                "device_pubkey": BASE64.encode(pubkey),
                "device_name": "probe.rs",
                "platform": "linux",
                "proof": BASE64.encode(proof.to_bytes()),
            }),
        )
        .await?;
    let device_id = paired["device_id"]
        .as_str()
        .ok_or("no device id")?
        .to_string();
    println!("paired as {device_id}");

    // 3. Authenticate on a fresh socket, the way a deck does on every connect.
    let mut deck = Probe::new(connect_pinned(&url(port), &fingerprint).await?);
    let challenge = deck
        .call(
            KnownOp::SessionHello,
            json!({ "device_id": device_id, "client_version": "probe", "platform": "linux" }),
        )
        .await?;
    let nonce = BASE64.decode(challenge["nonce"].as_str().ok_or("no nonce")?)?;
    let host_id = challenge["host_id"].as_str().ok_or("no host id")?;

    let signature = key.sign(&session_auth_message(&nonce, &device_id, host_id));
    let ready = deck
        .call(
            KnownOp::SessionAuth,
            json!({ "signature": BASE64.encode(signature.to_bytes()) }),
        )
        .await?;
    println!("authenticated as role {}", ready["role"]);
    println!("capabilities {}", ready["capabilities"]);

    // 4. Ping, and report the round trip the client would show in its status chip.
    println!();
    for _ in 0..5 {
        let sent = Instant::now();
        let response = deck
            .call(
                KnownOp::SystemPing,
                json!({ "t_client": unix_now_millis() }),
            )
            .await?;
        let rtt = sent.elapsed();
        let one_way =
            response["t_engine"].as_i64().unwrap_or(0) - response["t_client"].as_i64().unwrap_or(0);
        println!(
            "ping  rtt {:>6.2} ms   engine offset {one_way} ms",
            rtt.as_secs_f64() * 1000.0
        );
    }

    println!();
    println!("The latency budget in docs/ARCHITECTURE.md §7 allows 3-12 ms for the LAN round");
    println!("trip. Over loopback this should be well under that; a real phone will not be.");
    Ok(())
}

fn url(port: u16) -> String {
    format!("wss://127.0.0.1:{port}/ws")
}

struct Probe {
    socket: PinnedStream,
    next_id: u64,
}

impl Probe {
    fn new(socket: PinnedStream) -> Self {
        Self { socket, next_id: 1 }
    }

    async fn call(
        &mut self,
        op: KnownOp,
        payload: Value,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        let id = format!("probe-{}", self.next_id);
        self.next_id += 1;

        let envelope = Envelope {
            v: muxdeck_core::PROTOCOL_VERSION,
            t: MessageType::Req,
            id: Some(id.clone()),
            op: Op::of(op),
            d: payload,
        };
        self.socket
            .send(Message::Text(serde_json::to_string(&envelope)?.into()))
            .await?;

        loop {
            let message = self.socket.next().await.ok_or("connection closed")??;
            let Message::Text(text) = message else {
                continue;
            };
            let reply: Envelope<Value> = serde_json::from_str(&text)?;
            if reply.id.as_deref() != Some(id.as_str()) {
                continue;
            }
            return match reply.t {
                MessageType::Err => {
                    let payload: ErrorPayload = serde_json::from_value(reply.d)?;
                    Err(format!("{}: {}", payload.code.as_str(), payload.message).into())
                }
                _ => Ok(reply.d),
            };
        }
    }
}
