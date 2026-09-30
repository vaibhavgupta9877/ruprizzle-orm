//! Request guard for Ruprizzle Studio: `Host`, `Origin` and session-token checks.
//!
//! Binding to loopback keeps other *hosts* out, but not other *pages* in the
//! developer's own browser. Two attacks get through a bare loopback bind:
//!
//! - **CSRF.** A cross-origin `<form method=post>` is a CORS "simple request": the
//!   browser sends it without a preflight, and Studio's mutation routes accept
//!   form bodies.
//! - **DNS rebinding.** A hostname that re-resolves to `127.0.0.1` reaches Studio
//!   with `Host: attacker.example`, and the attacker's page is then same-origin
//!   with Studio and can read every response.
//!
//! [`guard_request`] closes both. Every request must carry a `Host` that names
//! Studio. Every request that is not `GET`/`HEAD`/`OPTIONS` must also carry an
//! `Origin` equal to `http://{Host}` and the per-process token that the page
//! shell hands to htmx through `hx-headers`.

use axum::extract::{Request, State};
use axum::http::{Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::sync::Arc;

use super::config::StudioConfig;
use super::handlers::AppState;
use super::is_loopback_host;

/// Header that carries the per-process session token on mutating requests.
pub const TOKEN_HEADER: &str = "x-studio-token";

/// Returns a fresh 128-bit hex token for one Studio process.
///
/// `RandomState` is seeded from the operating system's RNG, and `SipHash` output
/// under an unknown key is unpredictable, so this needs no extra dependency.
#[must_use]
pub fn new_session_token() -> String {
    use std::collections::hash_map::RandomState;
    use std::fmt::Write as _;
    use std::hash::{BuildHasher, Hasher};

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let mut out = String::with_capacity(32);
    for lane in 0u8..2 {
        let mut hasher = RandomState::new().build_hasher();
        hasher.write_u8(lane);
        hasher.write_u128(nanos);
        let _ = write!(out, "{:016x}", hasher.finish());
    }
    out
}

/// Strips an optional `:port` from a `Host` header value, keeping IPv6 brackets.
fn host_name(authority: &str) -> &str {
    let authority = authority.trim();
    if authority.starts_with('[') {
        return authority
            .find(']')
            .map_or(authority, |end| &authority[..=end]);
    }
    authority.split(':').next().unwrap_or(authority)
}

/// Whether a request's `Host` header names this Studio instance.
///
/// Loopback names are always accepted. So is the exact address Studio was bound
/// to. Any other name is refused, because it is what a DNS-rebinding page sends,
/// unless Studio was deliberately bound off loopback with `--yes-i-know`: then
/// the user has said the network is trusted and clients may use any name for it.
#[must_use]
pub fn host_is_allowed(host_header: Option<&str>, config: &StudioConfig) -> bool {
    let Some(host) = host_header.map(host_name).filter(|h| !h.is_empty()) else {
        return false;
    };
    if is_loopback_host(host) {
        return true;
    }
    let bound = config.host.trim();
    if host.eq_ignore_ascii_case(bound)
        || host.trim_start_matches('[').trim_end_matches(']') == bound
    {
        return true;
    }
    !is_loopback_host(bound) && config.yes_i_know
}

fn refuse(reason: &'static str) -> Response {
    (StatusCode::FORBIDDEN, reason).into_response()
}

/// Axum middleware applying the `Host`, `Origin` and token checks.
pub async fn guard_request(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok());
    if !host_is_allowed(host, &state.config) {
        return refuse("Studio refused this request: its Host header does not name Studio.");
    }

    let safe_method = matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    );
    if !safe_method {
        let expected_origin = host.map(|h| format!("http://{}", h.trim()));
        let origin = request
            .headers()
            .get(header::ORIGIN)
            .and_then(|v| v.to_str().ok());
        let same_origin = match (origin, expected_origin.as_deref()) {
            (Some(o), Some(e)) => o.trim().eq_ignore_ascii_case(e),
            _ => false,
        };
        if !same_origin {
            return refuse(
                "Studio refused this request: it did not come from a Studio page (Origin mismatch).",
            );
        }

        let token_ok = request
            .headers()
            .get(TOKEN_HEADER)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|t| t == state.session_token);
        if !token_ok {
            return refuse("Studio refused this request: missing or wrong session token.");
        }
    }

    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_name_strips_the_port() {
        assert_eq!(host_name("127.0.0.1:5555"), "127.0.0.1");
        assert_eq!(host_name("localhost"), "localhost");
        assert_eq!(host_name("[::1]:5555"), "[::1]");
    }

    #[test]
    fn tokens_are_long_and_distinct() {
        let a = new_session_token();
        let b = new_session_token();
        assert_eq!(a.len(), 32);
        assert_ne!(a, b);
    }
}
