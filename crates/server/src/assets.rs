//! Serving the built web app. A release binary carries it inside itself (so one file is the whole server); during
//! development, or when `OP_WEB_DIR` is set, it is read from a folder instead (see `lib.rs`).
//!
//! The serving rules live here, apart from where the files come from, so they can be tested with a fake source.

use std::borrow::Cow;

use axum::{
    http::{HeaderValue, Method, StatusCode, header},
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;

/// The built web app (`web/dist`), embedded at compile time. The folder may be missing in a development build
/// (nothing is embedded then, and the app is read from disk instead).
#[derive(RustEmbed)]
#[folder = "../../web/dist"]
#[allow_missing = true]
struct Dist;

pub struct Asset {
    pub bytes: Cow<'static, [u8]>,
}

/// Where files come from: the embedded folder in production, a map in tests.
pub trait AssetSource {
    fn get(&self, path: &str) -> Option<Asset>;
}

pub struct Embedded;

impl AssetSource for Embedded {
    fn get(&self, path: &str) -> Option<Asset> {
        Dist::get(path).map(|file| Asset { bytes: file.data })
    }
}

/// Was a web app compiled into this binary?
pub fn embedded_available() -> bool {
    Dist::get("index.html").is_some()
}

pub fn content_type(path: &str) -> &'static str {
    match path
        .rsplit('.')
        .next()
        .filter(|_| path.contains('.'))
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("html") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json",
        Some("webmanifest") => "application/manifest+json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        Some("txt") => "text/plain; charset=utf-8",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}

/// Files under `assets/` have a content hash in their name, so they can be kept for a year. Everything else (the
/// page, the service worker, the manifest, icons) is revalidated so an update is noticed.
pub fn cache_control(path: &str) -> &'static str {
    if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    }
}

fn has_extension(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .is_some_and(|name| name.contains('.'))
}

fn file_response(path: &str, asset: Asset) -> Response {
    let mut res = asset.bytes.into_owned().into_response();
    let headers = res.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(content_type(path)),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(cache_control(path)),
    );
    res
}

/// Answer a request for `uri_path`. A real file is served as it is. A path with no file extension is a screen of the
/// app (`/history`), so it gets `index.html` and the app's own router takes over. A missing file with an extension
/// is a plain 404: serving a page where a script was expected only hides the problem.
pub fn serve(source: &dyn AssetSource, method: &Method, uri_path: &str) -> Response {
    if *method != Method::GET && *method != Method::HEAD {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    if uri_path == "/api" || uri_path.starts_with("/api/") {
        return StatusCode::NOT_FOUND.into_response();
    }
    let path = uri_path.trim_start_matches('/');
    if !path.is_empty()
        && let Some(asset) = source.get(path)
    {
        return file_response(path, asset);
    }
    if has_extension(path) {
        return StatusCode::NOT_FOUND.into_response();
    }
    match source.get("index.html") {
        Some(index) => file_response("index.html", index),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct Fake(HashMap<&'static str, &'static str>);

    impl AssetSource for Fake {
        fn get(&self, path: &str) -> Option<Asset> {
            self.0.get(path).map(|body| Asset {
                bytes: Cow::Borrowed(body.as_bytes()),
            })
        }
    }

    fn site() -> Fake {
        Fake(HashMap::from([
            ("index.html", "<p>shell</p>"),
            ("sw.js", "// worker"),
            ("manifest.webmanifest", "{}"),
            ("assets/index-abc123.js", "console.log(1)"),
            ("assets/index-abc123.css", "body{}"),
            ("icon.svg", "<svg/>"),
        ]))
    }

    async fn body(res: Response) -> String {
        use http_body_util::BodyExt;
        String::from_utf8_lossy(&res.into_body().collect().await.expect("body").to_bytes())
            .into_owned()
    }

    fn header_of(res: &Response, name: header::HeaderName) -> Option<String> {
        res.headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
    }

    #[tokio::test]
    async fn the_root_and_index_give_the_page() {
        for path in ["/", "/index.html"] {
            let res = serve(&site(), &Method::GET, path);
            assert_eq!(res.status(), StatusCode::OK, "{path}");
            assert_eq!(
                header_of(&res, header::CONTENT_TYPE).as_deref(),
                Some("text/html; charset=utf-8")
            );
            assert_eq!(body(res).await, "<p>shell</p>");
        }
    }

    #[tokio::test]
    async fn a_screen_of_the_app_gets_the_page_so_the_apps_router_can_take_over() {
        for path in ["/history", "/settings/", "/a/b/c", "/%E2%9C%93"] {
            let res = serve(&site(), &Method::GET, path);
            assert_eq!(
                (res.status(), body(res).await.as_str()),
                (StatusCode::OK, "<p>shell</p>"),
                "{path}"
            );
        }
    }

    #[tokio::test]
    async fn files_are_served_with_their_type() {
        let cases = [
            ("/sw.js", "text/javascript; charset=utf-8", "// worker"),
            (
                "/assets/index-abc123.css",
                "text/css; charset=utf-8",
                "body{}",
            ),
            ("/manifest.webmanifest", "application/manifest+json", "{}"),
            ("/icon.svg", "image/svg+xml", "<svg/>"),
        ];
        for (path, mime, text) in cases {
            let res = serve(&site(), &Method::GET, path);
            assert_eq!(
                header_of(&res, header::CONTENT_TYPE).as_deref(),
                Some(mime),
                "{path}"
            );
            assert_eq!(body(res).await, text, "{path}");
        }
    }

    #[test]
    fn hashed_files_are_kept_a_year_and_everything_else_is_revalidated() {
        let hashed = serve(&site(), &Method::GET, "/assets/index-abc123.js");
        assert_eq!(
            header_of(&hashed, header::CACHE_CONTROL).as_deref(),
            Some("public, max-age=31536000, immutable")
        );
        for path in [
            "/",
            "/sw.js",
            "/manifest.webmanifest",
            "/icon.svg",
            "/history",
        ] {
            let res = serve(&site(), &Method::GET, path);
            assert_eq!(
                header_of(&res, header::CACHE_CONTROL).as_deref(),
                Some("no-cache"),
                "{path}"
            );
        }
    }

    #[test]
    fn a_missing_file_is_a_404_not_a_page() {
        for path in ["/assets/gone-123.js", "/nope.css", "/img/missing.png"] {
            assert_eq!(
                serve(&site(), &Method::GET, path).status(),
                StatusCode::NOT_FOUND,
                "{path}"
            );
        }
    }

    #[test]
    fn unknown_api_paths_are_404_and_never_the_page() {
        for path in ["/api", "/api/", "/api/nothing", "/api/ws/extra"] {
            assert_eq!(
                serve(&site(), &Method::GET, path).status(),
                StatusCode::NOT_FOUND,
                "{path}"
            );
        }
    }

    #[test]
    fn only_get_and_head_are_allowed() {
        assert_eq!(serve(&site(), &Method::HEAD, "/").status(), StatusCode::OK);
        for method in [Method::POST, Method::PUT, Method::DELETE, Method::PATCH] {
            assert_eq!(
                serve(&site(), &method, "/").status(),
                StatusCode::METHOD_NOT_ALLOWED,
                "{method}"
            );
        }
    }

    #[test]
    fn a_site_without_a_page_is_404() {
        let empty = Fake(HashMap::new());
        assert_eq!(
            serve(&empty, &Method::GET, "/").status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            serve(&empty, &Method::GET, "/history").status(),
            StatusCode::NOT_FOUND
        );
    }

    #[tokio::test]
    async fn dot_dot_paths_cannot_reach_anything_outside() {
        // Files are looked up by exact key, so "../" finds nothing; with no extension it is just a screen of the
        // app and gets the page, and with one it is a missing file.
        for path in [
            "/../../etc/passwd",
            "/../secret",
            "/%2e%2e/%2e%2e/etc/passwd",
        ] {
            let res = serve(&site(), &Method::GET, path);
            assert_eq!(
                (res.status(), body(res).await.as_str()),
                (StatusCode::OK, "<p>shell</p>"),
                "{path}"
            );
        }
        assert_eq!(
            serve(&site(), &Method::GET, "/../../etc/passwd.txt").status(),
            StatusCode::NOT_FOUND
        );
    }

    #[test]
    fn content_types_by_extension() {
        assert_eq!(content_type("a/b.JS"), "text/javascript; charset=utf-8");
        assert_eq!(content_type("x.png"), "image/png");
        assert_eq!(content_type("noext"), "application/octet-stream");
        assert_eq!(content_type("weird.unknown"), "application/octet-stream");
    }
}
