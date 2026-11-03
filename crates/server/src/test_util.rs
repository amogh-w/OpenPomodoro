//! Test helpers shared by the server's unit tests.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

static NEXT: AtomicU32 = AtomicU32::new(0);

/// A scratch directory that is removed on drop.
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new() -> Self {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("op-server-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&path).expect("create temp dir");
        TempDir(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    http::{Method, Request, StatusCode},
};
use http_body_util::BodyExt;
use op_core::FakeClock;
use serde_json::Value;
use tower::ServiceExt;

use crate::accounts::{Accounts, Hasher};
use crate::config::{Config, PublicUrl};
use crate::db::Db;
use crate::security::{CSRF_HEADER, CSRF_VALUE};
use crate::state::{AppState, Shared};

/// The whole app over an in-memory database, a fake clock and a cheap password hasher.
pub struct TestApp {
    pub state: Shared,
    pub clock: Arc<FakeClock>,
    _web: TempDir,
}

impl TestApp {
    pub fn new() -> Self {
        Self::with_public(None)
    }

    pub fn with_public(public: Option<&str>) -> Self {
        let web = TempDir::new();
        std::fs::write(web.path().join("index.html"), "<p>shell</p>").expect("write index");
        let mut config = Config::from_lookup(|_| None).expect("default config");
        config.web_dir = web.path().to_path_buf();
        config.public = public.map(|p| PublicUrl::parse(p).expect("public url"));
        let clock = Arc::new(FakeClock::new(1_700_000_000_000));
        let state = AppState::new(
            Db::open_in_memory().expect("db"),
            Accounts::new(Hasher::cheap()),
            config,
            clock.clone(),
        );
        Self {
            state,
            clock,
            _web: web,
        }
    }

    pub fn router(&self) -> Router {
        crate::app(self.state.clone())
    }

    pub fn get(&self, path: &str) -> Req<'_> {
        Req::new(self, Method::GET, path, None)
    }

    pub fn post(&self, path: &str, body: Option<Value>) -> Req<'_> {
        Req::new(self, Method::POST, path, body)
    }

    pub fn put(&self, path: &str, body: Option<Value>) -> Req<'_> {
        Req::new(self, Method::PUT, path, body)
    }

    pub fn delete(&self, path: &str, body: Option<Value>) -> Req<'_> {
        Req::new(self, Method::DELETE, path, body)
    }
}

pub struct Req<'a> {
    app: &'a TestApp,
    method: Method,
    path: String,
    body: Option<Value>,
    headers: Vec<(String, String)>,
    host: Option<String>,
    csrf: bool,
}

impl<'a> Req<'a> {
    fn new(app: &'a TestApp, method: Method, path: &str, body: Option<Value>) -> Self {
        Self {
            app,
            method,
            path: path.to_string(),
            body,
            headers: Vec::new(),
            host: Some("localhost:3000".into()),
            csrf: true,
        }
    }

    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_string(), value.to_string()));
        self
    }

    pub fn host(mut self, host: &str) -> Self {
        self.host = Some(host.to_string());
        self
    }

    pub fn no_host(mut self) -> Self {
        self.host = None;
        self
    }

    pub fn without_csrf(mut self) -> Self {
        self.csrf = false;
        self
    }

    /// Send the session cookie the way a browser would (the name depends on the config).
    pub fn cookie(self, token: &str) -> Self {
        let name = self.app.state.config.session_cookie_name();
        self.header("cookie", &format!("{name}={token}"))
    }

    pub async fn send(self) -> Res {
        let mut builder = Request::builder()
            .method(self.method.clone())
            .uri(&self.path);
        if let Some(host) = &self.host {
            builder = builder.header("host", host);
        }
        if self.csrf && self.method != Method::GET {
            builder = builder.header(CSRF_HEADER, CSRF_VALUE);
        }
        for (k, v) in &self.headers {
            builder = builder.header(k, v);
        }
        let body = match &self.body {
            Some(json) => {
                builder = builder.header("content-type", "application/json");
                Body::from(json.to_string())
            }
            None => Body::empty(),
        };
        let res = self
            .app
            .router()
            .oneshot(builder.body(body).expect("request"))
            .await
            .expect("response");
        let (parts, body) = res.into_parts();
        let bytes = body.collect().await.expect("body").to_bytes();
        Res {
            status: parts.status,
            headers: parts.headers,
            text: String::from_utf8_lossy(&bytes).into_owned(),
        }
    }
}

pub struct Res {
    pub status: StatusCode,
    pub headers: axum::http::HeaderMap,
    pub text: String,
}

impl Res {
    pub fn json(&self) -> Value {
        serde_json::from_str(&self.text).unwrap_or(Value::Null)
    }

    pub fn header(&self, name: &str) -> Option<String> {
        self.headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
    }
}
