//! OpenPomodoro server: accounts, per-user data, JSON API, and the built PWA.

use std::path::Path;

use axum::{
    Json, Router,
    extract::DefaultBodyLimit,
    http::{Method, Uri},
    middleware,
    routing::get,
};
use tower_http::services::{ServeDir, ServeFile};

pub mod accounts;
pub mod api;
pub mod assets;
pub mod backup;
pub mod config;
pub mod db;
pub mod error;
pub mod export;
pub mod healthcheck;
pub mod hooks;
pub mod hub;
pub mod push;
pub mod security;
pub mod serverlog;
pub mod settings;
pub mod sounds;
pub mod state;
#[cfg(test)]
mod test_util;
pub mod timer_service;
pub mod totp;
pub mod validate;
pub mod webhooks;
pub mod webpush;

use state::Shared;

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }))
}

/// Build the router: `/api/*` plus the built PWA (unknown paths fall back to `index.html`). Every request goes
/// through the host/origin guard, the CSRF check and the security headers.
pub fn app(state: Shared) -> Router {
    let config = state.config.clone();
    let routes = Router::new()
        .route("/api/health", get(health))
        .merge(api::router());
    // Where the web app comes from: a folder if `OP_WEB_DIR` says so, else the copy inside this binary, else (a
    // development build that has not built the web app) the usual build folder.
    let routes = match (&config.web_dir, assets::embedded_available()) {
        (Some(dir), _) => routes.fallback_service(from_folder(dir)),
        (None, true) => routes.fallback(|method: Method, uri: Uri| async move {
            assets::serve(&assets::Embedded, &method, uri.path())
        }),
        (None, false) => routes.fallback_service(from_folder(Path::new("web/dist"))),
    };
    routes
        .layer(DefaultBodyLimit::max(1024 * 1024))
        .layer(middleware::from_fn(security::csrf))
        .layer(middleware::from_fn_with_state(
            config.clone(),
            security::guard,
        ))
        .layer(middleware::from_fn_with_state(
            config,
            security::security_headers,
        ))
        .with_state(state)
}

fn from_folder(dir: &Path) -> ServeDir<ServeFile> {
    ServeDir::new(dir).fallback(ServeFile::new(dir.join("index.html")))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use crate::test_util::TestApp;

    #[tokio::test]
    async fn health_is_ok_on_localhost() {
        let app = TestApp::new();
        let res = app.get("/api/health").send().await;
        assert_eq!(res.status, StatusCode::OK);
        assert_eq!(res.json()["status"], "ok");
    }

    #[tokio::test]
    async fn foreign_host_and_origin_are_refused() {
        let app = TestApp::new();
        assert_eq!(
            app.get("/api/health")
                .host("evil.example")
                .send()
                .await
                .status,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            app.get("/api/health").no_host().send().await.status,
            StatusCode::FORBIDDEN
        );
        let cross = app
            .get("/api/health")
            .header("origin", "http://evil.example")
            .send()
            .await;
        assert_eq!(cross.status, StatusCode::FORBIDDEN);
        let same = app
            .get("/api/health")
            .header("origin", "http://localhost:3000")
            .send()
            .await;
        assert_eq!(same.status, StatusCode::OK);
    }

    #[tokio::test]
    async fn guard_also_covers_static_files() {
        let app = TestApp::new();
        assert_eq!(
            app.get("/").host("evil.example").send().await.status,
            StatusCode::FORBIDDEN
        );
    }

    #[tokio::test]
    async fn state_changing_requests_need_the_csrf_header() {
        let app = TestApp::new();
        // No route yet, so a request that passes the check reaches the SPA fallback and answers 405 (not 403).
        let blocked = app.post("/api/health", None).without_csrf().send().await;
        assert_eq!(blocked.status, StatusCode::FORBIDDEN);
        let allowed = app.post("/api/health", None).send().await;
        assert_ne!(allowed.status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn every_response_carries_security_headers() {
        let app = TestApp::new();
        for res in [
            app.get("/api/health").send().await,
            app.get("/").host("evil.example").send().await,
        ] {
            assert_eq!(
                res.header("x-content-type-options").as_deref(),
                Some("nosniff")
            );
            assert_eq!(
                res.header("referrer-policy").as_deref(),
                Some("no-referrer")
            );
            assert_eq!(res.header("x-frame-options").as_deref(), Some("DENY"));
            let csp = res.header("content-security-policy").expect("csp");
            assert!(csp.contains("script-src 'self'") && csp.contains("frame-ancestors 'none'"));
            assert_eq!(
                res.header("strict-transport-security"),
                None,
                "no HSTS over plain http"
            );
        }
    }

    #[tokio::test]
    async fn serves_index_and_falls_back_to_it() {
        let app = TestApp::new();
        let home = app.get("/").send().await;
        assert_eq!(
            (home.status, home.text.as_str()),
            (StatusCode::OK, "<p>shell</p>")
        );
        let deep = app.get("/some/client/route").send().await;
        assert_eq!(
            (deep.status, deep.text.as_str()),
            (StatusCode::OK, "<p>shell</p>")
        );
    }

    #[tokio::test]
    async fn put_and_delete_need_the_csrf_header_too() {
        let app = TestApp::new();
        assert_eq!(
            app.put("/api/anything", None)
                .without_csrf()
                .send()
                .await
                .status,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            app.delete("/api/anything", None)
                .without_csrf()
                .send()
                .await
                .status,
            StatusCode::FORBIDDEN
        );
        assert_ne!(
            app.put("/api/anything", None).send().await.status,
            StatusCode::FORBIDDEN
        );
    }

    #[tokio::test]
    async fn https_deployments_use_the_host_prefixed_cookie_and_hsts() {
        let app = TestApp::with_public(Some("https://pomodoro.example.com"));
        assert_eq!(app.state.config.session_cookie_name(), "__Host-op_session");
        let res = app
            .get("/api/health")
            .host("pomodoro.example.com")
            .header("origin", "https://pomodoro.example.com")
            .cookie("token")
            .send()
            .await;
        assert_eq!(res.status, StatusCode::OK);
        assert!(
            res.header("strict-transport-security")
                .is_some_and(|v| v.contains("max-age"))
        );
    }

    #[tokio::test]
    async fn the_app_reads_time_from_the_injected_clock() {
        let app = TestApp::new();
        let before = app.state.now_ms();
        app.clock.advance(5_000);
        assert_eq!(app.state.now_ms(), before + 5_000);
    }
}
