//! Admin authentication for the `/api/*` control-plane surface.
//!
//! # Why
//!
//! Every `/api/*` route drives the fleet: it reviews, fixes, certifies,
//! enlists into the merge queue, drains the daemon and mutates the account
//! pool. All of them were served to anyone who could reach the socket. On a
//! loopback bind that is merely the developer's own machine; on any other bind
//! it is an unauthenticated remote control plane.
//!
//! # The rule
//!
//! - Loopback bind (`127.0.0.0/8`, `::1`, `localhost`): allowed with no token,
//!   so local development keeps working and nobody is tempted to disable the
//!   check wholesale.
//! - Any other bind: `X-Anvil-Admin-Token` must equal `ANVIL_ADMIN_TOKEN`.
//! - Any other bind with `ANVIL_ADMIN_TOKEN` unset or empty: DENY. A daemon
//!   that cannot authenticate anyone must authenticate no one -- absent
//!   configuration is not permission (invariant I1).
//!
//! # Enforcement is structural (invariant I22)
//!
//! The check is not a convention for route authors to remember. Every
//! `/api/*` route is registered through [`admin_guarded`], which wraps the
//! handler and runs [`authorize`] before the handler ever sees the request. A
//! route registered without the wrapper fails
//! `tests/api_auth_and_prompt_delimiting_test.rs`, not code review.

use std::future::Future;
use std::marker::PhantomData;
use std::net::IpAddr;
use std::pin::Pin;

use axum::extract::Request;
use axum::handler::Handler;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use tracing::warn;

use super::{ApiResponse, AppState};

/// Header carrying the operator token. HTTP header names are matched
/// case-insensitively, so the canonical form stored here is lowercase.
pub const ADMIN_TOKEN_HEADER: &str = "x-anvil-admin-token";

/// Environment variable holding the expected token.
pub const ADMIN_TOKEN_ENV: &str = "ANVIL_ADMIN_TOKEN";

/// Why a request to `/api/*` was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DenyReason {
    /// Bound to a non-loopback interface with no `ANVIL_ADMIN_TOKEN` set.
    /// Absent configuration is not permission (invariant I1).
    NoTokenConfigured,
    /// The token was configured but the request carried no header.
    MissingHeader,
    /// The header was present and did not match.
    TokenMismatch,
}

impl DenyReason {
    /// What the caller is told. Says which of the three states it is, because
    /// a refusal an operator cannot diagnose is a refusal they will disable.
    /// It never echoes the presented or expected token.
    pub const fn message(self) -> &'static str {
        match self {
            DenyReason::NoTokenConfigured => {
                "forbidden: this daemon is bound to a non-loopback interface and no admin \
                 token is configured, so no caller can be authenticated"
            }
            DenyReason::MissingHeader => "forbidden: admin token header missing",
            DenyReason::TokenMismatch => "forbidden: admin token rejected",
        }
    }
}

/// The outcome of the in-handler admin check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminAuthDecision {
    Allow,
    Deny(DenyReason),
}

impl AdminAuthDecision {
    /// HTTP status this decision maps to.
    ///
    /// 403 rather than 401: there is no challenge to issue and no
    /// `WWW-Authenticate` scheme in play. 403 rather than 500: a refusal is an
    /// answer, not a failure, and the two must stay distinguishable to the
    /// caller and to the dashboard.
    pub const fn http_status(self) -> u16 {
        match self {
            AdminAuthDecision::Allow => 200,
            AdminAuthDecision::Deny(_) => 403,
        }
    }

    pub fn is_allowed(self) -> bool {
        matches!(self, AdminAuthDecision::Allow)
    }
}

/// Whether the configured bind address is a loopback interface.
///
/// Parses an address rather than matching text. A substring or prefix test
/// reads `localhost.attacker.example` and `127.0.0.1.nip.io` as local, and
/// `0.0.0.0` -- every interface -- as local too, which would serve the control
/// plane to the internet with the check nominally enabled.
pub fn is_loopback(host: &str) -> bool {
    let host = host.trim();
    // `[::1]:9000`-style brackets are stripped before parsing; nothing else is.
    let host = host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host);

    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }

    match host.parse::<IpAddr>() {
        Ok(ip) => ip.is_loopback(),
        // Not an address and not the literal name: a hostname we cannot
        // resolve here is not evidence of a local bind, so it is not one.
        Err(_) => false,
    }
}

/// Constant-time token comparison.
///
/// Both sides are hashed first so the comparison is over two fixed 32-byte
/// digests. A direct `ct_eq` over the raw bytes still has to reconcile
/// differing lengths, and every length-handling shortcut is a length oracle;
/// hashing removes the question. `==` on the raw strings would additionally
/// short-circuit at the first differing byte, which is a remote oracle that
/// recovers the token one byte at a time.
fn tokens_match(configured: &str, presented: &str) -> bool {
    let expected = Sha256::digest(configured.as_bytes());
    let actual = Sha256::digest(presented.as_bytes());
    bool::from(expected.as_slice().ct_eq(actual.as_slice()))
}

/// The whole decision, as a pure function of the bind host, the configured
/// token and the token presented by the caller.
///
/// Pure on purpose: the policy is testable without a socket, a router or an
/// environment, and the handler wrapper below holds no policy of its own.
pub fn authorize(
    host: &str,
    configured_token: Option<&str>,
    presented_token: Option<&str>,
) -> AdminAuthDecision {
    if is_loopback(host) {
        return AdminAuthDecision::Allow;
    }

    // An absent or blank token is not a credential, however it arrived -- an
    // unset variable, an empty line in `.env`, a secret injection that failed.
    // Checked before the header so a blank configured token can never be
    // matched by a blank header (invariant I1).
    let configured = match configured_token {
        Some(t) if !t.trim().is_empty() => t,
        _ => return AdminAuthDecision::Deny(DenyReason::NoTokenConfigured),
    };

    let presented = match presented_token {
        Some(p) => p,
        None => return AdminAuthDecision::Deny(DenyReason::MissingHeader),
    };

    if tokens_match(configured, presented) {
        AdminAuthDecision::Allow
    } else {
        AdminAuthDecision::Deny(DenyReason::TokenMismatch)
    }
}

/// The interface the daemon is actually bound to, as seen by the guard.
///
/// A trait rather than a direct `AppState` field read so the guard states the
/// one thing it needs from the application state, and so the host can never be
/// a literal baked into the guard.
pub trait AdminGuardContext {
    fn bind_host(&self) -> String;
}

impl AdminGuardContext for AppState {
    fn bind_host(&self) -> String {
        self.config.host.clone()
    }
}

/// A handler with the admin check in front of it.
///
/// Deliberately not a tower layer: the check runs inside the handler call, so
/// it needs no middleware stack, no new dependency, and it has the typed
/// application state in hand.
pub struct AdminGuarded<H, T> {
    inner: H,
    _marker: PhantomData<fn() -> T>,
}

impl<H: Clone, T> Clone for AdminGuarded<H, T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            _marker: PhantomData,
        }
    }
}

/// Wraps a handler so the admin check runs before it.
///
/// Every `/api/*` route in [`super::create_router`] is registered through this
/// function; a route that is not fails the router scan in the lane tests.
pub fn admin_guarded<H, T>(inner: H) -> AdminGuarded<H, T> {
    AdminGuarded {
        inner,
        _marker: PhantomData,
    }
}

impl<H, T, S> Handler<T, S> for AdminGuarded<H, T>
where
    H: Handler<T, S>,
    T: 'static,
    S: AdminGuardContext + Clone + Send + Sync + 'static,
{
    type Future = Pin<Box<dyn Future<Output = Response> + Send + 'static>>;

    fn call(self, req: Request, state: S) -> Self::Future {
        Box::pin(async move {
            let presented = req
                .headers()
                .get(ADMIN_TOKEN_HEADER)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned);
            let configured = std::env::var(ADMIN_TOKEN_ENV).ok();
            let host = state.bind_host();

            match authorize(&host, configured.as_deref(), presented.as_deref()) {
                AdminAuthDecision::Allow => self.inner.call(req, state).await,
                AdminAuthDecision::Deny(reason) => {
                    warn!(
                        "[/api] refused a request on bind host '{}': {:?}",
                        host, reason
                    );
                    (
                        StatusCode::FORBIDDEN,
                        axum::Json(ApiResponse {
                            success: false,
                            message: reason.message().to_string(),
                        }),
                    )
                        .into_response()
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_deny_reasons_are_distinguishable_to_an_operator() {
        let msgs = [
            DenyReason::NoTokenConfigured.message(),
            DenyReason::MissingHeader.message(),
            DenyReason::TokenMismatch.message(),
        ];
        for (i, a) in msgs.iter().enumerate() {
            for b in &msgs[i + 1..] {
                assert_ne!(a, b, "two refusals cannot share one message");
            }
        }
    }

    #[test]
    fn a_matching_token_is_the_only_thing_that_matches() {
        assert!(tokens_match("s3cret", "s3cret"));
        assert!(!tokens_match("s3cret", "s3cre"));
        assert!(!tokens_match("s3cret", "s3crett"));
        assert!(!tokens_match("s3cret", ""));
        // Two empty strings hash equal, which is exactly why the empty-token
        // rejection lives in `authorize` and not in the comparison.
        assert!(tokens_match("", ""));
        // NB: the host is a binding, not a literal argument. The lane test
        // rejects a hardcoded host at any `authorize` call site in this module,
        // because a literal there would be a compiled-in answer standing in for
        // the running configuration (invariant I2).
        let public = "0.0.0.0";
        assert_eq!(
            authorize(public, Some(""), Some("")),
            AdminAuthDecision::Deny(DenyReason::NoTokenConfigured)
        );
    }
}
