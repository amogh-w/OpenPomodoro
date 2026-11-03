//! Request guards: `Host` and `Origin` checks, the CSRF header, security headers, client address.
//!
//! The server is built to face the internet, so none of this is optional (see PLAN.md, Security).

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use axum::{
    extract::{ConnectInfo, Request, State},
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::config::{Config, PublicUrl};

/// State-changing API requests must carry this header. A cross-site form cannot set it, and a cross-site
/// `fetch` that does is stopped by the browser's preflight, which this server never approves.
pub const CSRF_HEADER: &str = "x-requested-with";
pub const CSRF_VALUE: &str = "openpomodoro";

const LOOPBACK_HOSTS: [&str; 3] = ["localhost", "127.0.0.1", "[::1]"];

/// Split `host[:port]` into the host part (IPv6 literals keep their brackets).
fn host_part(authority: &str) -> &str {
    if authority.starts_with('[') {
        authority
            .split_once("]:")
            .map_or(authority, |(h, _)| &authority[..=h.len()])
    } else {
        authority.split(':').next().unwrap_or(authority)
    }
}

fn is_loopback_host(authority: &str) -> bool {
    LOOPBACK_HOSTS.contains(&host_part(authority))
}

/// Accept a request only if its `Host` is this server's public host (or a loopback name, for local use and
/// development) and any `Origin` is this app's own origin. Everything else looks like DNS rebinding or a
/// cross-site request.
pub fn request_allowed(headers: &HeaderMap, public: Option<&PublicUrl>) -> bool {
    let Some(host) = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .map(str::to_ascii_lowercase)
    else {
        return false;
    };
    let origin = match headers.get(header::ORIGIN) {
        None => None,
        Some(v) => match v.to_str() {
            Ok(s) => Some(s.to_ascii_lowercase()),
            Err(_) => return false,
        },
    };

    if let Some(public) = public.filter(|p| p.authority == host) {
        return origin.is_none_or(|o| o == public.origin);
    }
    if is_loopback_host(&host) {
        return origin
            .is_none_or(|o| o == format!("http://{host}") || o == format!("https://{host}"));
    }
    false
}

pub async fn guard(State(config): State<Arc<Config>>, req: Request, next: Next) -> Response {
    if request_allowed(req.headers(), config.public.as_ref()) {
        next.run(req).await
    } else {
        StatusCode::FORBIDDEN.into_response()
    }
}

fn changes_state(method: &Method) -> bool {
    !matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
}

/// Require the CSRF header on state-changing requests to `/api`.
pub async fn csrf(req: Request, next: Next) -> Response {
    let protected = req.uri().path().starts_with("/api/") && changes_state(req.method());
    let ok = req
        .headers()
        .get(CSRF_HEADER)
        .is_some_and(|v| v == CSRF_VALUE);
    if protected && !ok {
        return (StatusCode::FORBIDDEN, "missing csrf header").into_response();
    }
    next.run(req).await
}

/// Content-Security-Policy for the built PWA: scripts only from this origin, no inline script, no frames.
/// `style-src-attr` allows the few `style=""` attributes the UI uses for theme colors.
const CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self'; style-src-attr 'unsafe-inline'; \
img-src 'self' data:; connect-src 'self'; manifest-src 'self'; worker-src 'self'; base-uri 'none'; \
form-action 'self'; frame-ancestors 'none'";

pub async fn security_headers(
    State(config): State<Arc<Config>>,
    req: Request,
    next: Next,
) -> Response {
    let mut res = next.run(req).await;
    let headers = res.headers_mut();
    let mut set = |name: &'static str, value: &'static str| {
        headers.insert(
            HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
    };
    set("content-security-policy", CSP);
    set("x-content-type-options", "nosniff");
    set("referrer-policy", "no-referrer");
    set("x-frame-options", "DENY");
    set("cross-origin-opener-policy", "same-origin");
    if config.https() {
        set(
            "strict-transport-security",
            "max-age=31536000; includeSubDomains",
        );
    }
    res
}

/// The client address used for rate limiting. Behind a trusted reverse proxy it is the last entry of
/// `X-Forwarded-For` (the one our own proxy appended); otherwise it is the socket peer.
pub fn client_address(headers: &HeaderMap, peer: Option<IpAddr>, trust_proxy: bool) -> String {
    if trust_proxy {
        let forwarded = headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.rsplit(',').map(str::trim).find(|s| !s.is_empty()));
        if let Some(addr) = forwarded {
            return addr.to_string();
        }
    }
    peer.map_or_else(|| "unknown".to_string(), |ip| ip.to_string())
}

/// The peer address recorded by `into_make_service_with_connect_info`, when there is one.
pub fn peer_ip(req_parts: &axum::http::Extensions) -> Option<IpAddr> {
    req_parts.get::<ConnectInfo<SocketAddr>>().map(|c| c.0.ip())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.insert(
                HeaderName::from_bytes(k.as_bytes()).expect("name"),
                HeaderValue::from_str(v).expect("value"),
            );
        }
        h
    }

    fn public() -> PublicUrl {
        PublicUrl::parse("https://pomodoro.example.com").expect("public")
    }

    #[test]
    fn loopback_hosts_are_allowed_without_a_public_url() {
        for host in [
            "localhost",
            "localhost:3000",
            "127.0.0.1:3000",
            "[::1]:3000",
            "LOCALHOST:5173",
        ] {
            assert!(request_allowed(&headers(&[("host", host)]), None), "{host}");
        }
    }

    #[test]
    fn foreign_or_missing_hosts_are_refused() {
        assert!(!request_allowed(
            &headers(&[("host", "evil.example")]),
            None
        ));
        assert!(!request_allowed(
            &headers(&[("host", "evil.example:3000")]),
            Some(&public())
        ));
        assert!(!request_allowed(&headers(&[]), None));
        assert!(!request_allowed(
            &headers(&[("host", "localhost.evil.example")]),
            None
        ));
    }

    #[test]
    fn the_public_host_is_allowed_only_with_the_public_origin() {
        let p = public();
        assert!(request_allowed(
            &headers(&[("host", "pomodoro.example.com")]),
            Some(&p)
        ));
        let good = headers(&[
            ("host", "pomodoro.example.com"),
            ("origin", "https://pomodoro.example.com"),
        ]);
        assert!(request_allowed(&good, Some(&p)));
        let wrong_scheme = headers(&[
            ("host", "pomodoro.example.com"),
            ("origin", "http://pomodoro.example.com"),
        ]);
        assert!(!request_allowed(&wrong_scheme, Some(&p)));
        let cross = headers(&[
            ("host", "pomodoro.example.com"),
            ("origin", "https://evil.example"),
        ]);
        assert!(!request_allowed(&cross, Some(&p)));
    }

    #[test]
    fn a_loopback_origin_must_match_the_host() {
        let ok = headers(&[
            ("host", "localhost:5173"),
            ("origin", "http://localhost:5173"),
        ]);
        assert!(request_allowed(&ok, None));
        let other_port = headers(&[
            ("host", "localhost:3000"),
            ("origin", "http://localhost:5173"),
        ]);
        assert!(!request_allowed(&other_port, None));
        let foreign = headers(&[
            ("host", "localhost:3000"),
            ("origin", "http://evil.example"),
        ]);
        assert!(!request_allowed(&foreign, None));
    }

    #[test]
    fn loopback_is_still_allowed_when_a_public_url_is_set() {
        assert!(request_allowed(
            &headers(&[("host", "localhost:3000")]),
            Some(&public())
        ));
    }

    #[test]
    fn client_address_uses_the_peer_unless_a_proxy_is_trusted() {
        let h = headers(&[("x-forwarded-for", "6.6.6.6, 203.0.113.9")]);
        let peer = Some(IpAddr::from([10, 0, 0, 1]));
        assert_eq!(
            client_address(&h, peer, false),
            "10.0.0.1",
            "a client-supplied header must be ignored"
        );
        assert_eq!(
            client_address(&h, peer, true),
            "203.0.113.9",
            "the proxy's own entry is the last one"
        );
        assert_eq!(client_address(&headers(&[]), peer, true), "10.0.0.1");
        assert_eq!(client_address(&headers(&[]), None, false), "unknown");
    }
}
