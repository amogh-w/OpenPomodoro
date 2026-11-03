//! OpenPomodoro server: static PWA files, JSON API, local-only guards.

use std::path::Path;

use axum::{
    Json, Router,
    extract::Request,
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
};
use tower_http::services::{ServeDir, ServeFile};

/// Host names the server answers to. Anything else is treated as DNS rebinding.
const LOCAL_HOSTS: [&str; 3] = ["localhost", "127.0.0.1", "[::1]"];

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

fn host_is_local(authority: &str) -> bool {
    LOCAL_HOSTS.contains(&host_part(authority))
}

/// Reject requests whose `Host` is not a local name, or whose `Origin` is not this app's origin.
fn request_allowed(headers: &HeaderMap) -> bool {
    let Some(host) = headers.get(header::HOST).and_then(|v| v.to_str().ok()) else {
        return false;
    };
    if !host_is_local(host) {
        return false;
    }
    match headers.get(header::ORIGIN) {
        None => true,
        Some(origin) => origin
            .to_str()
            .ok()
            .and_then(|o| o.strip_prefix("http://"))
            .is_some_and(|authority| authority == host),
    }
}

async fn guard(req: Request, next: Next) -> Response {
    if request_allowed(req.headers()) {
        next.run(req).await
    } else {
        StatusCode::FORBIDDEN.into_response()
    }
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }))
}

/// Build the router: `/api/*` plus the built PWA from `web_dir` (unknown paths fall back to `index.html`).
pub fn app(web_dir: &Path) -> Router {
    let files = ServeDir::new(web_dir).fallback(ServeFile::new(web_dir.join("index.html")));
    Router::new()
        .route("/api/health", get(health))
        .fallback_service(files)
        .layer(middleware::from_fn(guard))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn web_dir() -> tempdir_lite::Dir {
        let dir = tempdir_lite::Dir::new();
        std::fs::write(dir.path().join("index.html"), "<p>shell</p>").expect("write index");
        dir
    }

    async fn get_with(path: &str, headers: &[(&str, &str)]) -> (StatusCode, String) {
        let dir = web_dir();
        let mut req = Request::builder().uri(path);
        for (k, v) in headers {
            req = req.header(*k, *v);
        }
        let res = app(dir.path())
            .oneshot(req.body(Body::empty()).expect("request"))
            .await
            .expect("response");
        let status = res.status();
        let bytes = res.into_body().collect().await.expect("body").to_bytes();
        (status, String::from_utf8_lossy(&bytes).into_owned())
    }

    #[tokio::test]
    async fn health_ok_on_localhost() {
        let (status, body) = get_with("/api/health", &[("host", "127.0.0.1:3000")]).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("\"status\":\"ok\""));
    }

    #[tokio::test]
    async fn rejects_foreign_host() {
        let (status, _) = get_with("/api/health", &[("host", "evil.example:3000")]).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn rejects_missing_host() {
        let (status, _) = get_with("/api/health", &[]).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn rejects_foreign_origin() {
        let (status, _) = get_with(
            "/api/health",
            &[
                ("host", "localhost:3000"),
                ("origin", "http://evil.example"),
            ],
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn accepts_same_origin() {
        let (status, _) = get_with(
            "/api/health",
            &[
                ("host", "localhost:3000"),
                ("origin", "http://localhost:3000"),
            ],
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn ipv6_loopback_is_local() {
        let (status, _) = get_with("/api/health", &[("host", "[::1]:3000")]).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn serves_index_and_falls_back_to_it() {
        let (status, body) = get_with("/", &[("host", "localhost")]).await;
        assert_eq!((status, body.as_str()), (StatusCode::OK, "<p>shell</p>"));
        let (status, body) = get_with("/some/client/route", &[("host", "localhost")]).await;
        assert_eq!((status, body.as_str()), (StatusCode::OK, "<p>shell</p>"));
    }

    /// Tiny scratch-dir helper so tests need no extra crate.
    mod tempdir_lite {
        use std::path::{Path, PathBuf};
        use std::sync::atomic::{AtomicU32, Ordering};

        static NEXT: AtomicU32 = AtomicU32::new(0);

        pub struct Dir(PathBuf);

        impl Dir {
            pub fn new() -> Self {
                let n = NEXT.fetch_add(1, Ordering::Relaxed);
                let path =
                    std::env::temp_dir().join(format!("op-server-test-{}-{n}", std::process::id()));
                std::fs::create_dir_all(&path).expect("create temp dir");
                Dir(path)
            }
            pub fn path(&self) -> &Path {
                &self.0
            }
        }

        impl Drop for Dir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }
}
