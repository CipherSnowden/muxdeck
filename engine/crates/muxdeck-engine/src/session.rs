//! The handshake state machine and role assignment.
//!
//! `docs/PROTOCOL.md` §3 and `docs/ARCHITECTURE.md` §5.3–§5.4. This is the module where a
//! mistake is a full host compromise rather than a bug, so every branch below is asserted by a
//! test — including the ones that must *fail*.

use std::net::SocketAddr;
use std::time::Duration;

use ed25519_dalek::Signature;
use muxdeck_core::signing::{session_auth_message, SIGNATURE_LEN};
use muxdeck_core::{
    AuthRequest, Capabilities, Challenge, ErrorCode, HelloRequest, HelloResponse, HostPlatform,
    Ready, Role,
};

use crate::error::{EngineError, Result};
use crate::identity::{random_bytes, Identity};
use crate::pairing::decode_fixed;
use crate::registry::Registry;

/// How long a socket may stay unauthenticated before it is closed.
/// `docs/PROTOCOL.md` §3.
pub const AUTH_TIMEOUT: Duration = Duration::from_secs(10);

/// The host platform this build runs on.
pub const HOST_PLATFORM: HostPlatform = if cfg!(target_os = "windows") {
    HostPlatform::Windows
} else if cfg!(target_os = "macos") {
    HostPlatform::Macos
} else {
    HostPlatform::Linux
};

/// Everything the handshake needs to look at, borrowed for the duration of one message.
pub struct HandshakeContext<'a> {
    pub identity: &'a Identity,
    pub registry: &'a Registry,
    pub host_name: &'a str,
    pub engine_version: &'a str,
    pub active_profile_id: &'a str,
    pub capabilities: Capabilities,
}

impl HandshakeContext<'_> {
    fn ready(&self, role: Role) -> Ready {
        Ready {
            role,
            protocol: muxdeck_core::PROTOCOL_VERSION,
            engine_version: self.engine_version.to_string(),
            host_platform: HOST_PLATFORM,
            active_profile_id: self.active_profile_id.to_string(),
            capabilities: self.capabilities,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum State {
    /// Nothing has been proven. Only `session.*` and `pair.*` are accepted.
    Unauthenticated,
    /// A challenge has been issued and we are waiting for the signature over it.
    AwaitingAuth { device_id: String, nonce: [u8; 32] },
    Ready {
        role: Role,
        device_id: Option<String>,
    },
}

/// One socket's authentication state.
pub struct Session {
    peer: SocketAddr,
    state: State,
}

impl Session {
    pub fn new(peer: SocketAddr) -> Self {
        Self {
            peer,
            state: State::Unauthenticated,
        }
    }

    pub fn peer(&self) -> SocketAddr {
        self.peer
    }

    /// The role this socket has been granted, if it has finished authenticating.
    pub fn role(&self) -> Option<Role> {
        match &self.state {
            State::Ready { role, .. } => Some(*role),
            _ => None,
        }
    }

    /// The device behind this socket. `None` for the admin panel, which has no device identity.
    pub fn device_id(&self) -> Option<&str> {
        match &self.state {
            State::Ready { device_id, .. } => device_id.as_deref(),
            State::AwaitingAuth { device_id, .. } => Some(device_id),
            State::Unauthenticated => None,
        }
    }

    pub fn is_ready(&self) -> bool {
        matches!(self.state, State::Ready { .. })
    }

    /// Handles `session.hello`. Returns the tagged union to send back.
    pub fn handle_hello(
        &mut self,
        request: &HelloRequest,
        ctx: &HandshakeContext<'_>,
    ) -> Result<HelloResponse> {
        if self.is_ready() {
            return Err(EngineError::wire(
                ErrorCode::BadRequest,
                "this session has already completed its handshake",
            ));
        }

        // Exactly one of device_id and admin_token. Both, or neither, is BAD_REQUEST.
        request.validate().map_err(EngineError::Wire)?;

        if let Some(token) = &request.admin_token {
            return self.hello_admin(token, ctx).map(HelloResponse::Ready);
        }

        let device_id = request
            .device_id
            .as_deref()
            .expect("validate() guarantees one of the two is present");
        self.hello_deck(device_id, ctx)
            .map(HelloResponse::Challenge)
    }

    /// The admin path: loopback **and** a matching token, no challenge round trip.
    ///
    /// `admin` cannot be requested — it is granted or it is not. Both halves are required:
    /// loopback alone would admit a second logged-in user on a multi-user desktop, and the token
    /// alone would be reachable from the network. `docs/ARCHITECTURE.md` §5.4.
    fn hello_admin(&mut self, token: &str, ctx: &HandshakeContext<'_>) -> Result<Ready> {
        let from_loopback = self.peer.ip().is_loopback();
        let token_matches =
            constant_time_eq(token.as_bytes(), ctx.identity.admin_token().as_bytes());

        if !from_loopback || !token_matches {
            // One error for both failures on purpose: telling a remote caller that its token was
            // correct would confirm it had stolen a valid one.
            return Err(EngineError::wire(
                ErrorCode::NotAuthorized,
                "the admin role is available only to a local client presenting the admin token",
            ));
        }

        let ready = ctx.ready(Role::Admin);
        self.state = State::Ready {
            role: Role::Admin,
            device_id: None,
        };
        Ok(ready)
    }

    /// The deck path: issue a challenge for the device to sign.
    fn hello_deck(&mut self, device_id: &str, ctx: &HandshakeContext<'_>) -> Result<Challenge> {
        // Looked up now so an unknown device fails here rather than after it has signed.
        ctx.registry.verifying_key(device_id)?;

        let nonce = random_bytes::<32>()?;
        let challenge = Challenge {
            nonce: base64_encode(&nonce),
            host_id: ctx.identity.host_id().to_string(),
            host_name: ctx.host_name.to_string(),
        };

        self.state = State::AwaitingAuth {
            device_id: device_id.to_string(),
            nonce,
        };
        Ok(challenge)
    }

    /// Handles `session.auth`. Valid only directly after a challenge.
    pub fn handle_auth(
        &mut self,
        request: &AuthRequest,
        ctx: &HandshakeContext<'_>,
    ) -> Result<Ready> {
        let (device_id, nonce) = match &self.state {
            State::AwaitingAuth { device_id, nonce } => (device_id.clone(), *nonce),
            _ => {
                return Err(EngineError::wire(
                    ErrorCode::NotAuthenticated,
                    "session.auth is valid only after a session.hello challenge",
                ));
            }
        };

        let verifying_key = ctx.registry.verifying_key(&device_id)?;
        let signature_bytes = decode_fixed::<SIGNATURE_LEN>(&request.signature, "signature")?;
        let signature = Signature::from_bytes(&signature_bytes);

        // Built by muxdeck-core, which is fixture-tested byte-for-byte against the Dart client.
        // Do not inline this concatenation — a mismatch here authenticates nothing and produces
        // no diagnosable symptom.
        let message = session_auth_message(&nonce, &device_id, ctx.identity.host_id());

        verifying_key
            .verify_strict(&message, &signature)
            .map_err(|_| {
                EngineError::wire(
                    ErrorCode::BadSignature,
                    "challenge signature did not verify",
                )
            })?;

        let ready = ctx.ready(Role::Deck);
        self.state = State::Ready {
            role: Role::Deck,
            device_id: Some(device_id),
        };
        Ok(ready)
    }
}

/// Sessions in a known state, for tests in other modules.
///
/// `State` is private so nothing outside this file can mint an authenticated session by
/// accident. These constructors are the deliberate, test-only exception; the transitions
/// themselves are covered by this module's own tests.
#[cfg(test)]
pub mod tests_support {
    use super::*;

    fn peer() -> SocketAddr {
        "127.0.0.1:50000".parse().expect("addr")
    }

    pub fn unauthenticated() -> Session {
        Session::new(peer())
    }

    pub fn ready_deck() -> Session {
        Session {
            peer: peer(),
            state: State::Ready {
                role: Role::Deck,
                device_id: Some("d_7f3a91c2b4e05d18".to_string()),
            },
        }
    }

    pub fn ready_admin() -> Session {
        Session {
            peer: peer(),
            state: State::Ready {
                role: Role::Admin,
                device_id: None,
            },
        }
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD as BASE64;
    use base64::Engine as _;
    use ed25519_dalek::{Signer, SigningKey};
    use muxdeck_core::Platform;

    use crate::config::Paths;

    struct Fixture {
        identity: Identity,
        registry: Registry,
        device_key: SigningKey,
        device_id: String,
        _root: std::path::PathBuf,
    }

    fn fixture(name: &str) -> Fixture {
        let root = std::env::temp_dir()
            .join("muxdeck-session-tests")
            .join(name);
        let _ = std::fs::remove_dir_all(&root);
        let paths = Paths::resolve(Some(root.clone())).expect("paths");
        let identity = Identity::load_or_generate(&paths).expect("identity");

        let device_key = SigningKey::from_bytes(&[11u8; 32]);
        let mut registry = Registry::load(&paths.devices()).expect("registry");
        let device_id = registry
            .insert(
                &device_key.verifying_key().to_bytes(),
                "Test deck".into(),
                Platform::Android,
            )
            .expect("insert");

        Fixture {
            identity,
            registry,
            device_key,
            device_id,
            _root: root,
        }
    }

    fn context<'a>(f: &'a Fixture) -> HandshakeContext<'a> {
        HandshakeContext {
            identity: &f.identity,
            registry: &f.registry,
            host_name: "ENIGMA-ENTROPY",
            engine_version: "0.1.0",
            active_profile_id: "p_default",
            capabilities: Capabilities {
                text_unicode: true,
                media_keys: true,
                mouse: true,
                shell_actions: false,
            },
        }
    }

    fn loopback() -> SocketAddr {
        "127.0.0.1:50000".parse().expect("addr")
    }

    fn remote() -> SocketAddr {
        "192.168.1.50:50000".parse().expect("addr")
    }

    fn deck_hello(device_id: &str) -> HelloRequest {
        HelloRequest {
            device_id: Some(device_id.to_string()),
            admin_token: None,
            client_version: "0.1.0".into(),
            platform: Platform::Android,
        }
    }

    fn admin_hello(token: &str) -> HelloRequest {
        HelloRequest {
            device_id: None,
            admin_token: Some(token.to_string()),
            client_version: "0.1.0".into(),
            platform: Platform::Windows,
        }
    }

    // --- the deck path ---------------------------------------------------

    #[test]
    fn deck_happy_path_reaches_ready() {
        let f = fixture("deck_happy");
        let ctx = context(&f);
        let mut session = Session::new(remote());

        let response = session
            .handle_hello(&deck_hello(&f.device_id), &ctx)
            .expect("hello");
        let challenge = match response {
            HelloResponse::Challenge(c) => c,
            HelloResponse::Ready(_) => panic!("a deck must be challenged, never made ready"),
        };
        assert_eq!(challenge.host_id, f.identity.host_id());
        assert!(!session.is_ready(), "a challenge is not authentication");

        let nonce = BASE64.decode(&challenge.nonce).expect("nonce");
        let message = session_auth_message(&nonce, &f.device_id, f.identity.host_id());
        let signature = f.device_key.sign(&message);

        let ready = session
            .handle_auth(
                &AuthRequest {
                    signature: BASE64.encode(signature.to_bytes()),
                },
                &ctx,
            )
            .expect("auth");

        assert_eq!(ready.role, Role::Deck);
        assert!(session.is_ready());
        assert_eq!(session.device_id(), Some(f.device_id.as_str()));
    }

    #[test]
    fn a_signature_over_a_different_nonce_is_rejected() {
        // The replay this blocks: a signature captured from an earlier session.
        let f = fixture("wrong_nonce");
        let ctx = context(&f);
        let mut session = Session::new(remote());
        session
            .handle_hello(&deck_hello(&f.device_id), &ctx)
            .expect("hello");

        let message = session_auth_message(&[0u8; 32], &f.device_id, f.identity.host_id());
        let signature = f.device_key.sign(&message);

        let err = session
            .handle_auth(
                &AuthRequest {
                    signature: BASE64.encode(signature.to_bytes()),
                },
                &ctx,
            )
            .expect_err("wrong nonce");
        assert_eq!(err.to_payload().code, ErrorCode::BadSignature);
        assert!(!session.is_ready());
    }

    #[test]
    fn a_signature_bound_to_another_host_is_rejected() {
        // Why host_id is in the signed buffer: without it, a signature captured against one
        // host would authenticate against another. `docs/PROTOCOL.md` §4.1.
        let f = fixture("wrong_host");
        let ctx = context(&f);
        let mut session = Session::new(remote());
        let response = session
            .handle_hello(&deck_hello(&f.device_id), &ctx)
            .expect("hello");
        let challenge = match response {
            HelloResponse::Challenge(c) => c,
            _ => unreachable!(),
        };

        let nonce = BASE64.decode(&challenge.nonce).expect("nonce");
        let message = session_auth_message(&nonce, &f.device_id, "h_0000000000000000");
        let signature = f.device_key.sign(&message);

        let err = session
            .handle_auth(
                &AuthRequest {
                    signature: BASE64.encode(signature.to_bytes()),
                },
                &ctx,
            )
            .expect_err("wrong host");
        assert_eq!(err.to_payload().code, ErrorCode::BadSignature);
    }

    #[test]
    fn an_unknown_device_is_rejected_before_any_challenge_is_issued() {
        let f = fixture("unknown_device");
        let ctx = context(&f);
        let mut session = Session::new(remote());

        let err = session
            .handle_hello(&deck_hello("d_0000000000000000"), &ctx)
            .expect_err("unknown device");
        assert_eq!(err.to_payload().code, ErrorCode::UnknownDevice);
        assert!(
            session.device_id().is_none(),
            "no state may be retained for an unknown device"
        );
    }

    #[test]
    fn auth_without_a_prior_challenge_is_rejected() {
        let f = fixture("auth_first");
        let ctx = context(&f);
        let mut session = Session::new(remote());

        let err = session
            .handle_auth(
                &AuthRequest {
                    signature: BASE64.encode([0u8; 64]),
                },
                &ctx,
            )
            .expect_err("no challenge");
        assert_eq!(err.to_payload().code, ErrorCode::NotAuthenticated);
    }

    // --- the admin path --------------------------------------------------

    #[test]
    fn admin_over_loopback_with_the_token_is_ready_immediately() {
        let f = fixture("admin_happy");
        let ctx = context(&f);
        let mut session = Session::new(loopback());

        let response = session
            .handle_hello(&admin_hello(f.identity.admin_token()), &ctx)
            .expect("hello");

        match response {
            HelloResponse::Ready(ready) => assert_eq!(ready.role, Role::Admin),
            HelloResponse::Challenge(_) => panic!("the panel must not be challenged"),
        }
        assert!(session.is_ready());
        assert_eq!(
            session.device_id(),
            None,
            "the panel has no device identity"
        );
    }

    #[test]
    fn a_valid_admin_token_from_a_remote_peer_grants_nothing() {
        // The token leaking must not be enough. Loopback is the other half of the check.
        let f = fixture("admin_remote");
        let ctx = context(&f);
        let mut session = Session::new(remote());

        let err = session
            .handle_hello(&admin_hello(f.identity.admin_token()), &ctx)
            .expect_err("remote admin");
        assert_eq!(err.to_payload().code, ErrorCode::NotAuthorized);
        assert!(session.role().is_none());
    }

    #[test]
    fn loopback_with_the_wrong_token_grants_nothing() {
        // And loopback must not be enough either: on a multi-user desktop a second logged-in
        // user can also reach 127.0.0.1.
        let f = fixture("admin_bad_token");
        let ctx = context(&f);
        let mut session = Session::new(loopback());

        let err = session
            .handle_hello(&admin_hello("not-the-token"), &ctx)
            .expect_err("wrong token");
        assert_eq!(err.to_payload().code, ErrorCode::NotAuthorized);
        assert!(session.role().is_none());
    }

    // --- malformed hellos ------------------------------------------------

    #[test]
    fn a_hello_with_both_credentials_is_bad_request() {
        let f = fixture("both");
        let ctx = context(&f);
        let mut session = Session::new(loopback());

        let request = HelloRequest {
            device_id: Some(f.device_id.clone()),
            admin_token: Some(f.identity.admin_token().to_string()),
            client_version: "0.1.0".into(),
            platform: Platform::Windows,
        };

        let err = session.handle_hello(&request, &ctx).expect_err("both");
        assert_eq!(err.to_payload().code, ErrorCode::BadRequest);
    }

    #[test]
    fn a_hello_with_neither_credential_is_bad_request() {
        let f = fixture("neither");
        let ctx = context(&f);
        let mut session = Session::new(loopback());

        let request = HelloRequest {
            device_id: None,
            admin_token: None,
            client_version: "0.1.0".into(),
            platform: Platform::Windows,
        };

        let err = session.handle_hello(&request, &ctx).expect_err("neither");
        assert_eq!(err.to_payload().code, ErrorCode::BadRequest);
    }

    #[test]
    fn a_second_hello_after_ready_is_rejected() {
        let f = fixture("rehello");
        let ctx = context(&f);
        let mut session = Session::new(loopback());
        session
            .handle_hello(&admin_hello(f.identity.admin_token()), &ctx)
            .expect("first hello");

        let err = session
            .handle_hello(&admin_hello(f.identity.admin_token()), &ctx)
            .expect_err("second hello");
        assert_eq!(err.to_payload().code, ErrorCode::BadRequest);
    }
}
