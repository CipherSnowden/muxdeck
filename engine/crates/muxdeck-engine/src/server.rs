//! The TLS WebSocket server and the per-socket message loop.
//!
//! `docs/ENGINE.md` §5. One tokio task per socket; nothing here blocks the runtime.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::connect_info::ConnectInfo;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use axum_server::tls_rustls::RustlsConfig;
use muxdeck_core::{
    ActionDeleteRequest, ActionListResponse, ActionRunRequest, ActionSetRequest, AuthRequest,
    Empty, Envelope, ErrorCode, HelloRequest, KnownOp, MessageType, Op, PairBeginRequest,
    PairBeginResponse, PairListDevicesResponse, PairRequest, PairResponse, PairRevokeRequest,
    PairingState, PingRequest, PingResponse, ProfileActivateRequest, ProfileDeleteRequest,
    ProfileGetRequest, ProfileListResponse, ProfileWrapper, Role, SettingsPatch,
    SettingsSetResponse,
};
use serde::Serialize;
use serde_json::Value;
use tokio::sync::broadcast;
use tokio::time::Instant;
use tracing::{debug, info, warn};

use crate::dispatch::authorize;
use crate::engine::Engine;
use crate::error::{EngineError, Result};
use crate::input_dispatch;
use crate::pairing::verify_pair_request;
use crate::registry::unix_now_millis;
use crate::session::{HandshakeContext, Session, AUTH_TIMEOUT};

/// Who an event is for.
///
/// A direct transcription of the "Delivered to" column of `docs/PROTOCOL.md` §4.9. Every event
/// is routed by one of these rather than by an ad-hoc check at the send site, so the table and
/// the code cannot drift apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventAudience {
    /// Sockets that called `profile.subscribe`, whatever their role.
    ///
    /// Deliberately not admin-only: "edit on the desktop and the tablet updates" *is* a
    /// `profile.changed` reaching a deck.
    ProfileSubscribers,
    /// Sockets that called `telemetry.subscribe`, whatever their role.
    TelemetrySubscribers,
    /// `admin` only — device and pairing state are not a deck's business.
    AdminOnly,
    /// Every authenticated socket, subscribed or not.
    AllAuthenticated,
}

impl EventAudience {
    /// The audience for an event op.
    pub fn for_op(op: KnownOp) -> Self {
        match op {
            KnownOp::ProfileChanged => EventAudience::ProfileSubscribers,
            KnownOp::TelemetryUpdate => EventAudience::TelemetrySubscribers,
            KnownOp::DeviceChanged | KnownOp::PairingState => EventAudience::AdminOnly,
            KnownOp::EngineShutdown => EventAudience::AllAuthenticated,
            // Not an event; nothing will ever be published under it.
            _ => EventAudience::AdminOnly,
        }
    }
}

/// One event, serialised once and filtered per socket on receipt.
#[derive(Clone)]
struct Event {
    audience: EventAudience,
    frame: String,
}

/// Events pushed to connected sockets.
#[derive(Clone)]
pub struct EventBus(broadcast::Sender<Event>);

impl EventBus {
    pub fn new() -> Self {
        Self(broadcast::channel(64).0)
    }

    pub fn publish<T: Serialize>(&self, op: KnownOp, payload: &T) {
        match event_frame(op, payload) {
            Ok(frame) => {
                // An error here only means nobody is listening, which is the normal case when
                // the control panel is closed and no deck has subscribed.
                let _ = self.0.send(Event {
                    audience: EventAudience::for_op(op),
                    frame,
                });
            }
            Err(e) => warn!(error = %e, op = op.as_str(), "could not serialise an event"),
        }
    }

    fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.0.subscribe()
    }

    /// Whether any socket is connected to receive events.
    ///
    /// Used by the telemetry sampler to avoid waking for an empty room. It cannot distinguish a
    /// telemetry subscriber from any other socket — every connection holds a receiver and
    /// filters on arrival — so this is a floor, not an exact audience.
    pub fn has_listeners(&self) -> bool {
        self.0.receiver_count() > 0
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
struct AppState {
    engine: Arc<Engine>,
    events: EventBus,
}

/// A server that is listening. Dropping the handle does not stop it; call [`Running::shutdown`].
pub struct Running {
    pub addr: SocketAddr,
    pub events: EventBus,
    handle: axum_server::Handle<SocketAddr>,
}

impl Running {
    pub fn shutdown(&self) {
        self.handle.shutdown();
    }
}

/// Binds and starts serving in the background.
///
/// The listener is created here rather than inside `axum-server` so the caller can learn the
/// port that was actually bound — which matters when binding to port 0, as the tests do.
pub async fn serve(engine: Arc<Engine>, addr: SocketAddr) -> Result<Running> {
    let listener = std::net::TcpListener::bind(addr)
        .map_err(|e| EngineError::io("binding the listen socket", engine.paths.root(), e))?;
    let local_addr = listener
        .local_addr()
        .map_err(|e| EngineError::io("reading the bound address", engine.paths.root(), e))?;

    // tokio requires an adopted listener to be non-blocking; without this the accept loop
    // blocks a worker thread and every connection hangs at the TLS handshake with no error.
    listener.set_nonblocking(true).map_err(|e| {
        EngineError::io("setting the listener non-blocking", engine.paths.root(), e)
    })?;

    engine.set_bound_port(local_addr.port());

    let tls = RustlsConfig::from_pem(
        engine.identity.cert_pem().as_bytes().to_vec(),
        engine.identity.key_pem().as_bytes().to_vec(),
    )
    .await
    .map_err(|e| EngineError::Certificate(format!("rustls rejected the certificate: {e}")))?;

    let events = EventBus::new();

    // One sampler for the whole process, started here rather than by the binary so every test
    // that calls `serve` exercises the same wiring the daemon does.
    tokio::spawn(crate::telemetry::run(engine.clone(), events.clone()));

    let state = AppState {
        engine,
        events: events.clone(),
    };

    let app = Router::new().route("/ws", get(upgrade)).with_state(state);

    let handle: axum_server::Handle<SocketAddr> = axum_server::Handle::new();
    let server = axum_server::from_tcp_rustls(listener, tls)
        .map_err(|e| EngineError::io("starting the TLS server", "<listener>", e))?
        .handle(handle.clone());

    tokio::spawn(async move {
        let result = server
            .serve(app.into_make_service_with_connect_info::<SocketAddr>())
            .await;
        if let Err(e) = result {
            warn!(error = %e, "the websocket server stopped");
        }
    });

    Ok(Running {
        addr: local_addr,
        events,
        handle,
    })
}

async fn upgrade(
    ws: WebSocketUpgrade,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(move |socket| connection(socket, peer, state))
}

/// One socket, start to finish.
async fn connection(mut socket: WebSocket, peer: SocketAddr, state: AppState) {
    debug!(%peer, "socket opened");
    let mut session = Session::new(peer);
    let mut events = state.events.subscribe();
    let deadline = Instant::now() + AUTH_TIMEOUT;

    loop {
        let incoming = tokio::select! {
            // Only an authenticated socket has an unlimited read; before that the 10-second
            // timeout applies, so an idle connection cannot sit on a slot indefinitely.
            message = read_with_deadline(&mut socket, session.is_ready(), deadline) => match message {
                ReadOutcome::Message(text) => text,
                ReadOutcome::Closed => break,
                ReadOutcome::TimedOut => {
                    debug!(%peer, "closing a socket that did not authenticate in time");
                    break;
                }
            },

            // Every authenticated socket listens; what it actually receives is decided per
            // event by `Session::wants`, against the table in `docs/PROTOCOL.md` §4.9.
            event = events.recv(), if session.is_ready() => {
                match event {
                    Ok(event) => {
                        if !session.wants(event.audience) {
                            continue;
                        }
                        if socket.send(Message::Text(event.frame.into())).await.is_err() {
                            break;
                        }
                        continue;
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        warn!(%peer, skipped, "a socket fell behind the event stream");
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        };

        let reply = handle_text(&state, &mut session, &incoming).await;
        if let Some(frame) = reply {
            if socket.send(Message::Text(frame.into())).await.is_err() {
                break;
            }
        }
    }

    if let Some(device_id) = session.device_id() {
        state.engine.mark_disconnected(device_id);
        state
            .events
            .publish(KnownOp::DeviceChanged, &device_changed(&state.engine));
    }
    debug!(%peer, "socket closed");
}

enum ReadOutcome {
    Message(String),
    Closed,
    TimedOut,
}

async fn read_with_deadline(
    socket: &mut WebSocket,
    authenticated: bool,
    deadline: Instant,
) -> ReadOutcome {
    let recv = async {
        loop {
            match socket.recv().await {
                Some(Ok(Message::Text(text))) => return ReadOutcome::Message(text.to_string()),
                // Ping/pong frames are handled by axum; binary frames are not part of the
                // protocol, which is text-only (`docs/PROTOCOL.md` §1).
                Some(Ok(_)) => continue,
                Some(Err(_)) | None => return ReadOutcome::Closed,
            }
        }
    };

    if authenticated {
        recv.await
    } else {
        match tokio::time::timeout_at(deadline, recv).await {
            Ok(outcome) => outcome,
            Err(_) => ReadOutcome::TimedOut,
        }
    }
}

/// Parses one text frame and produces the reply, if any.
async fn handle_text(state: &AppState, session: &mut Session, text: &str) -> Option<String> {
    let envelope: Envelope<Value> = match serde_json::from_str(text) {
        Ok(envelope) => envelope,
        Err(e) => {
            debug!(error = %e, "discarding an unparseable frame");
            // No correlation ID is available, so there is nothing to answer.
            return None;
        }
    };

    let id = envelope.id.clone();
    let op = envelope.op.clone();

    let outcome = route(state, session, envelope).await;
    Some(match outcome {
        Ok(payload) => frame(MessageType::Res, id, &op, &payload),
        Err(e) => {
            let payload = e.to_payload();
            if payload.code == ErrorCode::Internal {
                warn!(error = %e, op = op.as_str(), "internal error handling an op");
            }
            frame(
                MessageType::Err,
                id,
                &op,
                &serde_json::to_value(payload).unwrap_or(Value::Null),
            )
        }
    })
}

/// Checks the envelope and the capability matrix, then dispatches.
///
/// Input ops are peeled off here because they await; everything else is handled synchronously
/// so the `&mut Session` borrow never has to cross an await point.
async fn route(
    state: &AppState,
    session: &mut Session,
    envelope: Envelope<Value>,
) -> Result<Value> {
    if !envelope.is_supported_version() {
        return Err(EngineError::wire(
            ErrorCode::UnsupportedVersion,
            format!("protocol version {} is not supported", envelope.v),
        ));
    }

    let op = authorize(&envelope.op, session)?;

    if matches!(
        op,
        KnownOp::InputKeyCombo
            | KnownOp::InputKeySequence
            | KnownOp::InputText
            | KnownOp::InputMedia
            | KnownOp::InputMouse
    ) {
        return handle_input(&state.engine, op, envelope.d).await;
    }

    handle_envelope(state, session, envelope)
}

fn handle_envelope(
    state: &AppState,
    session: &mut Session,
    envelope: Envelope<Value>,
) -> Result<Value> {
    if !envelope.is_supported_version() {
        return Err(EngineError::wire(
            ErrorCode::UnsupportedVersion,
            format!("protocol version {} is not supported", envelope.v),
        ));
    }

    // Capability check before the payload is even looked at.
    let op = authorize(&envelope.op, session)?;
    let engine = &state.engine;

    match op {
        KnownOp::SessionHello => {
            let request: HelloRequest = payload(envelope.d)?;
            let host_name = engine.host_name();
            let active_profile_id = engine.active_profile_id();
            let capabilities = engine.capabilities();

            let response = engine.with_registry(|registry| {
                let ctx = HandshakeContext {
                    identity: &engine.identity,
                    registry,
                    host_name: &host_name,
                    engine_version: env!("CARGO_PKG_VERSION"),
                    active_profile_id: &active_profile_id,
                    capabilities,
                };
                session.handle_hello(&request, &ctx)
            })?;

            value(&response)
        }

        KnownOp::SessionAuth => {
            let request: AuthRequest = payload(envelope.d)?;
            let host_name = engine.host_name();
            let active_profile_id = engine.active_profile_id();
            let capabilities = engine.capabilities();

            let ready = engine.with_registry(|registry| {
                let ctx = HandshakeContext {
                    identity: &engine.identity,
                    registry,
                    host_name: &host_name,
                    engine_version: env!("CARGO_PKG_VERSION"),
                    active_profile_id: &active_profile_id,
                    capabilities,
                };
                session.handle_auth(&request, &ctx)
            })?;

            if let Some(device_id) = session.device_id() {
                engine.mark_connected(device_id);
                let _ = engine.with_registry(|r| r.touch(device_id));
            }
            state
                .events
                .publish(KnownOp::DeviceChanged, &device_changed(engine));
            value(&ready)
        }

        KnownOp::PairRequest => {
            let request: PairRequest = payload(envelope.d)?;
            let window = engine.pairing_window();
            let pubkey = verify_pair_request(window.as_ref(), &request)?;

            let device_id = engine.with_registry(|registry| {
                registry.insert(&pubkey, request.device_name.clone(), request.platform)
            })?;

            // One device per window. Leaving it open would let anyone who saw the QR pair a
            // second device afterwards.
            engine.close_pairing();
            info!(device = %device_id, name = %request.device_name, "paired a new device");

            state.events.publish(
                KnownOp::PairingState,
                &PairingState {
                    active: false,
                    expires_at: 0,
                },
            );
            state
                .events
                .publish(KnownOp::DeviceChanged, &device_changed(engine));

            value(&PairResponse {
                device_id,
                host_id: engine.identity.host_id().to_string(),
                host_name: engine.host_name(),
            })
        }

        KnownOp::PairBegin => {
            let request: PairBeginRequest = payload(envelope.d)?;
            request.validate().map_err(EngineError::Wire)?;

            let window = engine.open_pairing(request.ttl_seconds)?;
            let qr_payload = window.qr_payload(
                &engine.advertised_addr(),
                engine.identity.host_id(),
                engine.identity.fingerprint(),
            );

            state.events.publish(
                KnownOp::PairingState,
                &PairingState {
                    active: true,
                    expires_at: window.expires_at(),
                },
            );

            value(&PairBeginResponse {
                code: window.code().to_string(),
                expires_at: window.expires_at(),
                qr_payload,
            })
        }

        KnownOp::PairCancel => {
            engine.close_pairing();
            state.events.publish(
                KnownOp::PairingState,
                &PairingState {
                    active: false,
                    expires_at: 0,
                },
            );
            value(&Empty {})
        }

        KnownOp::PairListDevices => value(&PairListDevicesResponse {
            devices: engine.list_devices(),
        }),

        KnownOp::PairRevoke => {
            let request: PairRevokeRequest = payload(envelope.d)?;
            let existed = engine.with_registry(|registry| registry.revoke(&request.device_id))?;
            if !existed {
                return Err(EngineError::wire(
                    ErrorCode::UnknownDevice,
                    "no such paired device",
                ));
            }
            engine.mark_disconnected(&request.device_id);
            state
                .events
                .publish(KnownOp::DeviceChanged, &device_changed(engine));
            value(&Empty {})
        }

        KnownOp::SystemPing => {
            let request: PingRequest = payload(envelope.d)?;
            value(&PingResponse {
                t_client: request.t_client,
                t_engine: unix_now_millis(),
            })
        }

        KnownOp::ProfileGet => {
            let request: ProfileGetRequest = payload(envelope.d)?;
            let profile = engine
                .with_store(|store| store.get(&request.profile_id).cloned())
                .ok_or_else(|| EngineError::wire(ErrorCode::NotFound, "no such profile"))?;
            value(&ProfileWrapper { profile })
        }

        KnownOp::ProfileList => value(&ProfileListResponse {
            profiles: engine.with_store(|store| store.list()),
        }),

        KnownOp::ProfileSubscribe => {
            // Explicit opt-in: the engine pushes nothing unasked, so a deck that wants live
            // layout updates says so. `docs/PROTOCOL.md` §3.
            session.subscribe_profile();
            value(&Empty {})
        }

        KnownOp::ProfileSet => {
            let request: ProfileWrapper = payload(envelope.d)?;
            let role = session.role().unwrap_or(Role::Deck);
            let profile = request.profile;

            engine.with_store(|store| store.set(profile.clone(), role))?;

            // The live loop. A subscribed deck re-renders without being asked, which is what
            // makes the editor pleasant to use.
            state
                .events
                .publish(KnownOp::ProfileChanged, &ProfileWrapper { profile });
            value(&Empty {})
        }

        KnownOp::ProfileActivate => {
            let request: ProfileActivateRequest = payload(envelope.d)?;
            engine.with_store(|store| store.activate(&request.profile_id))?;

            // Switching profiles changes what every deck should be showing, so the new layout
            // goes out the same way an edit does.
            if let Some(profile) = engine.with_store(|store| store.active().cloned()) {
                state
                    .events
                    .publish(KnownOp::ProfileChanged, &ProfileWrapper { profile });
            }
            value(&Empty {})
        }

        KnownOp::ProfileDelete => {
            let request: ProfileDeleteRequest = payload(envelope.d)?;
            engine.with_store(|store| store.delete(&request.profile_id))?;

            if let Some(profile) = engine.with_store(|store| store.active().cloned()) {
                state
                    .events
                    .publish(KnownOp::ProfileChanged, &ProfileWrapper { profile });
            }
            value(&Empty {})
        }

        KnownOp::TelemetrySubscribe => {
            session.subscribe_telemetry();
            value(&Empty {})
        }

        KnownOp::ActionRun => {
            let request: ActionRunRequest = payload(envelope.d)?;
            engine.run_action(&request.action_id)?;
            value(&Empty {})
        }

        KnownOp::ActionList => {
            // Empty rather than an error when the feature is off, so a client can call this
            // unconditionally at startup (`docs/PROTOCOL.md` §4.4). A deck that asked and got
            // `DISABLED` would have to special-case it on every connect.
            let actions = if engine.settings().shell_actions_enabled {
                engine.with_actions(|store| store.list())
            } else {
                Vec::new()
            };
            value(&ActionListResponse { actions })
        }

        KnownOp::ActionSet => {
            // Defining an action requires the feature on, not merely running one: writing a
            // command to disk while the switch is off would let a panel stage something that
            // becomes runnable the instant anybody flips it.
            require_shell_actions(engine)?;
            let request: ActionSetRequest = payload(envelope.d)?;
            engine.with_actions(|store| store.set(request.action))?;
            value(&Empty {})
        }

        KnownOp::ActionDelete => {
            require_shell_actions(engine)?;
            let request: ActionDeleteRequest = payload(envelope.d)?;
            engine.with_actions(|store| store.delete(&request.action_id))?;
            value(&Empty {})
        }

        KnownOp::SettingsGet => value(&engine.settings()),

        KnownOp::SettingsSet => {
            let patch: SettingsPatch = payload(envelope.d)?;
            let restart_required = patch.requires_restart();
            engine.apply_settings(patch)?;
            value(&SettingsSetResponse { restart_required })
        }

        // Input is the one group that is deliberately not handled here: every arm awaits, and
        // this function is synchronous so the session borrow does not cross an await point.
        KnownOp::InputKeyCombo
        | KnownOp::InputKeySequence
        | KnownOp::InputText
        | KnownOp::InputMedia
        | KnownOp::InputMouse => unreachable!("input ops are routed before this point"),

        // Events, which a client never sends as a request.
        _ => Err(EngineError::wire(
            ErrorCode::UnknownOp,
            format!("{} is an event, not a request", op.as_str()),
        )),
    }
}

/// Refuses the op unless shell actions are switched on.
fn require_shell_actions(engine: &Arc<Engine>) -> Result<()> {
    if engine.settings().shell_actions_enabled {
        Ok(())
    } else {
        Err(EngineError::wire(
            ErrorCode::Disabled,
            "shell actions are switched off. Enable them in the MuxDeck control panel; \
             any paired device will then be able to run every defined action.",
        ))
    }
}

/// Handles the `input.*` ops.
///
/// Split out because these are the only handlers that await: injection goes to
/// `spawn_blocking` and sequence delays sleep, neither of which can happen while a `&mut
/// Session` is held. The capability check has already run by the time this is called.
async fn handle_input(engine: &Arc<Engine>, op: KnownOp, payload_value: Value) -> Result<Value> {
    let backend = engine.input();

    match op {
        KnownOp::InputKeyCombo => {
            input_dispatch::key_combo(&backend, payload(payload_value)?).await?
        }
        KnownOp::InputKeySequence => {
            input_dispatch::key_sequence(&backend, payload(payload_value)?).await?
        }
        KnownOp::InputText => input_dispatch::text(&backend, payload(payload_value)?).await?,
        KnownOp::InputMedia => input_dispatch::media(&backend, payload(payload_value)?).await?,
        KnownOp::InputMouse => input_dispatch::mouse(&backend, payload(payload_value)?).await?,
        _ => unreachable!("handle_input is only called for input ops"),
    }

    value(&Empty {})
}

fn device_changed(engine: &Engine) -> muxdeck_core::DeviceChangedEvent {
    muxdeck_core::DeviceChangedEvent {
        devices: engine.list_devices(),
    }
}

fn payload<T: serde::de::DeserializeOwned>(d: Value) -> Result<T> {
    serde_json::from_value(d)
        .map_err(|e| EngineError::wire(ErrorCode::BadRequest, format!("malformed payload: {e}")))
}

fn value<T: Serialize>(payload: &T) -> Result<Value> {
    serde_json::to_value(payload).map_err(|e| {
        EngineError::wire(
            ErrorCode::Internal,
            format!("could not serialise a reply: {e}"),
        )
    })
}

fn frame(t: MessageType, id: Option<String>, op: &Op, payload: &Value) -> String {
    let envelope = Envelope {
        v: muxdeck_core::PROTOCOL_VERSION,
        t,
        id,
        op: op.clone(),
        d: payload.clone(),
    };
    serde_json::to_string(&envelope).unwrap_or_else(|_| String::from("{}"))
}

fn event_frame<T: Serialize>(op: KnownOp, payload: &T) -> Result<String> {
    let envelope = Envelope {
        v: muxdeck_core::PROTOCOL_VERSION,
        t: MessageType::Evt,
        id: None,
        op: Op::of(op),
        d: serde_json::to_value(payload)
            .map_err(|e| EngineError::wire(ErrorCode::Internal, format!("event payload: {e}")))?,
    };
    serde_json::to_string(&envelope)
        .map_err(|e| EngineError::wire(ErrorCode::Internal, format!("event frame: {e}")))
}
