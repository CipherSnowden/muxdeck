//! A WebSocket client that connects over loopback with the `admin` role.
//!
//! Used by the `muxdeckd pair` subcommands. These are deliberately **not** a second control
//! path into the engine: they read `admin.token` and speak the ordinary protocol over the
//! ordinary socket, exactly like the desktop panel (`docs/ENGINE.md` §7). Anything they can do,
//! the panel can do, and vice versa.

use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use muxdeck_core::{Envelope, ErrorPayload, HelloResponse, KnownOp, MessageType, Op, Ready};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::Connector;

use crate::error::{EngineError, Result};

/// A WebSocket stream to the engine.
pub type PinnedStream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Opens a WebSocket connection, accepting only the certificate matching `fingerprint`.
///
/// Shared by the admin CLI, the probe example and the integration tests, so all three exercise
/// the same pinning path a real client uses rather than a relaxed one that would hide a
/// certificate bug.
pub async fn connect_pinned(url: &str, fingerprint: &str) -> Result<PinnedStream> {
    let config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(PinnedFingerprint::new(fingerprint)?))
        .with_no_client_auth();

    let (socket, _) = tokio_tungstenite::connect_async_tls_with_config(
        url,
        None,
        false,
        Some(Connector::Rustls(Arc::new(config))),
    )
    .await
    .map_err(|e| {
        EngineError::Certificate(format!(
            "could not connect to the engine at {url}: {e}. Is muxdeckd running?"
        ))
    })?;

    Ok(socket)
}

/// An authenticated admin connection.
pub struct AdminClient {
    socket: PinnedStream,
    next_id: u64,
    pub ready: Ready,
}

impl AdminClient {
    /// Connects to the engine on loopback and completes the admin handshake.
    pub async fn connect(port: u16, fingerprint: &str, admin_token: &str) -> Result<Self> {
        let socket = connect_pinned(&format!("wss://127.0.0.1:{port}/ws"), fingerprint).await?;

        let mut client = Self {
            socket,
            next_id: 1,
            // Replaced immediately by the handshake below; a Ready is not constructible from
            // nothing, so the field is filled in rather than left optional for the caller to
            // unwrap on every use.
            ready: Ready {
                role: muxdeck_core::Role::Admin,
                protocol: muxdeck_core::PROTOCOL_VERSION,
                engine_version: String::new(),
                host_platform: crate::session::HOST_PLATFORM,
                active_profile_id: String::new(),
                capabilities: muxdeck_core::Capabilities {
                    text_unicode: false,
                    media_keys: false,
                    mouse: false,
                    shell_actions: false,
                },
            },
        };

        let hello = serde_json::json!({
            "admin_token": admin_token,
            "client_version": env!("CARGO_PKG_VERSION"),
            "platform": platform_name(),
        });
        let response = client.request(KnownOp::SessionHello, &hello).await?;

        match serde_json::from_value::<HelloResponse>(response) {
            Ok(HelloResponse::Ready(ready)) => {
                client.ready = ready;
                Ok(client)
            }
            Ok(HelloResponse::Challenge(_)) => Err(EngineError::Certificate(
                "the engine issued a challenge to a loopback admin connection, which should \
                 never happen"
                    .to_string(),
            )),
            Err(e) => Err(EngineError::Certificate(format!(
                "could not read the handshake response: {e}"
            ))),
        }
    }

    /// Sends a request and waits for the matching response.
    ///
    /// Events arriving in the meantime are skipped rather than mistaken for the answer — an
    /// admin socket receives `device.changed` and `pairing.state` unprompted.
    pub async fn request<T: Serialize>(&mut self, op: KnownOp, payload: &T) -> Result<Value> {
        let id = format!("cli-{}", self.next_id);
        self.next_id += 1;

        let envelope = Envelope {
            v: muxdeck_core::PROTOCOL_VERSION,
            t: MessageType::Req,
            id: Some(id.clone()),
            op: Op::of(op),
            d: serde_json::to_value(payload)
                .map_err(|e| EngineError::Certificate(format!("bad request payload: {e}")))?,
        };
        let text = serde_json::to_string(&envelope)
            .map_err(|e| EngineError::Certificate(format!("bad request: {e}")))?;

        self.socket
            .send(Message::Text(text.into()))
            .await
            .map_err(|e| EngineError::Certificate(format!("send failed: {e}")))?;

        loop {
            let message = self
                .socket
                .next()
                .await
                .ok_or_else(|| {
                    EngineError::Certificate("the engine closed the connection".to_string())
                })?
                .map_err(|e| EngineError::Certificate(format!("receive failed: {e}")))?;

            let Message::Text(text) = message else {
                continue;
            };
            let reply: Envelope<Value> = serde_json::from_str(&text)
                .map_err(|e| EngineError::Certificate(format!("unparseable reply: {e}")))?;

            if reply.id.as_deref() != Some(id.as_str()) {
                continue;
            }
            return match reply.t {
                MessageType::Err => {
                    let payload: ErrorPayload = serde_json::from_value(reply.d).map_err(|e| {
                        EngineError::Certificate(format!("unparseable error payload: {e}"))
                    })?;
                    Err(EngineError::Wire(payload))
                }
                _ => Ok(reply.d),
            };
        }
    }
}

fn platform_name() -> &'static str {
    match crate::session::HOST_PLATFORM {
        muxdeck_core::HostPlatform::Windows => "windows",
        muxdeck_core::HostPlatform::Macos => "macos",
        muxdeck_core::HostPlatform::Linux => "linux",
    }
}

/// Accepts exactly one certificate: the one whose SHA-256 matches the expected fingerprint.
///
/// This mirrors what the mobile client does (`docs/CLIENT.md` §3). Hostname and CA validation
/// are deliberately bypassed because the host has no DNS name and no CA — but the certificate
/// still has to be *the* certificate, so a different self-signed cert on the same port is
/// rejected rather than trusted.
#[derive(Debug)]
struct PinnedFingerprint {
    expected: String,
    provider: Arc<rustls::crypto::CryptoProvider>,
}

impl PinnedFingerprint {
    fn new(expected: &str) -> Result<Self> {
        let provider = rustls::crypto::CryptoProvider::get_default()
            .cloned()
            .ok_or_else(|| {
                EngineError::Certificate(
                    "no rustls crypto provider is installed; call install_default() at startup"
                        .to_string(),
                )
            })?;
        Ok(Self {
            expected: expected.to_ascii_lowercase(),
            provider,
        })
    }
}

impl ServerCertVerifier for PinnedFingerprint {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        let digest = Sha256::digest(end_entity.as_ref());
        let actual: String = digest.iter().map(|b| format!("{b:02x}")).collect();

        if actual == self.expected {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General(
                "certificate fingerprint does not match the pinned value".to_string(),
            ))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}
