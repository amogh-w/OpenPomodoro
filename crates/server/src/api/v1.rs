//! Version 1 of the API for scripts: the same data as the app, reached with a personal token instead of a cookie.
//! `Authorization: Bearer opt_...`. Cookies are not accepted here, so these routes need no CSRF header.

use axum::{
    Json, Router,
    extract::{Query, State},
    routing::{get, post},
};
use serde_json::Value;

use super::data::{HistoryQuery, history, run_command, state_json};
use super::{ApiJson, TokenUser};
use crate::error::ApiError;
use crate::state::Shared;
use crate::timer_service::{self, Command};

pub fn router() -> Router<Shared> {
    Router::new()
        .route("/api/v1/state", get(get_state))
        .route("/api/v1/commands", post(post_command))
        .route("/api/v1/sessions", get(get_sessions))
}

/// The clock, the timer, the settings, the categories and the tasks.
async fn get_state(
    State(state): State<Shared>,
    TokenUser(user): TokenUser,
) -> Result<Json<Value>, ApiError> {
    let timer = timer_service::load_state(&state.db(), &user.id)?;
    Ok(Json(state_json(&state, &user.id, &timer)?))
}

/// A timer command, exactly as the app sends it (`id` makes a retry harmless).
async fn post_command(
    State(state): State<Shared>,
    TokenUser(user): TokenUser,
    ApiJson(cmd): ApiJson<Command>,
) -> Result<Json<Value>, ApiError> {
    run_command(&state, &user.id, &cmd).map(Json)
}

/// Sessions that started between `from` (included) and `to` (excluded), Unix milliseconds, up to a year.
async fn get_sessions(
    State(state): State<Shared>,
    TokenUser(user): TokenUser,
    Query(q): Query<HistoryQuery>,
) -> Result<Json<Value>, ApiError> {
    history(&state, &user.id, &q).map(Json)
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::{Value, json};

    use crate::test_util::{Res, TestApp};

    const PW: &str = "correct horse battery staple";

    fn cookie_of(res: &Res) -> String {
        let raw = res.header("set-cookie").expect("cookie");
        raw.split(';')
            .next()
            .expect("pair")
            .split_once('=')
            .expect("kv")
            .1
            .to_string()
    }

    async fn admin(app: &TestApp) -> String {
        cookie_of(
            &app.post(
                "/api/setup",
                Some(json!({"username": "admin", "password": PW})),
            )
            .send()
            .await,
        )
    }

    async fn make_token(app: &TestApp, cookie: &str, name: &str) -> (String, String) {
        let res = app
            .post("/api/me/tokens", Some(json!({"name": name})))
            .cookie(cookie)
            .send()
            .await;
        assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
        let body = res.json();
        (
            body["id"].as_str().expect("id").to_string(),
            body["token"].as_str().expect("token").to_string(),
        )
    }

    fn bearer(token: &str) -> String {
        format!("Bearer {token}")
    }

    #[tokio::test]
    async fn a_token_reads_state_and_runs_commands_without_a_cookie_or_csrf_header() {
        let app = TestApp::new();
        let me = admin(&app).await;
        let (_, token) = make_token(&app, &me, "script").await;
        assert!(token.starts_with("opt_"));

        let state = app
            .get("/api/v1/state")
            .header("authorization", &bearer(&token))
            .send()
            .await;
        assert_eq!(state.status, StatusCode::OK, "{}", state.text);
        assert_eq!(state.json()["timer"]["state"], "idle");

        let id = op_core::IdGen::new("script").next(app.state.now_ms());
        let cmd = json!({"id": id, "name": "init_focus", "intention": "from a script"});
        let res = app
            .post("/api/v1/commands", Some(cmd))
            .without_csrf()
            .header("authorization", &bearer(&token))
            .send()
            .await;
        assert_eq!(res.status, StatusCode::OK, "{}", res.text);
        assert_eq!(res.json()["state"]["timer"]["intention"], "from a script");

        let now = app.state.now_ms();
        let list = app
            .get(&format!(
                "/api/v1/sessions?from={}&to={}",
                now - 86_400_000,
                now + 86_400_000
            ))
            .header("authorization", &bearer(&token))
            .send()
            .await;
        assert_eq!(list.status, StatusCode::OK);
        assert_eq!(list.json()["sessions"], json!([]));
    }

    #[tokio::test]
    async fn the_token_api_refuses_cookies_and_bad_tokens() {
        let app = TestApp::new();
        let me = admin(&app).await;
        assert_eq!(
            app.get("/api/v1/state").cookie(&me).send().await.status,
            StatusCode::UNAUTHORIZED
        );
        for bad in ["Bearer nope", "Bearer opt_deadbeef", "Basic abc", "Bearer "] {
            let res = app
                .get("/api/v1/state")
                .header("authorization", bad)
                .send()
                .await;
            assert_eq!(res.status, StatusCode::UNAUTHORIZED, "{bad}");
        }
        // A cookie-only change to the app's own API still needs the CSRF header.
        let forged = app
            .post("/api/me/tokens", Some(json!({"name": "x"})))
            .cookie(&me)
            .without_csrf()
            .send()
            .await;
        assert_eq!(forged.status, StatusCode::FORBIDDEN);
        // The exemption is for the token API only.
        let cmd =
            json!({"id": op_core::IdGen::new("x").next(app.state.now_ms()), "name": "init_focus"});
        let res = app
            .post("/api/commands", Some(cmd))
            .cookie(&me)
            .without_csrf()
            .header("authorization", "Bearer x")
            .send()
            .await;
        assert_eq!(res.status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn tokens_are_listed_without_their_secret_and_stop_working_when_revoked() {
        let app = TestApp::new();
        let me = admin(&app).await;
        let (id, token) = make_token(&app, &me, "laptop").await;

        let list = app.get("/api/me/tokens").cookie(&me).send().await;
        assert_eq!(list.json()["tokens"][0]["name"], "laptop");
        assert!(!list.text.contains(&token));
        assert!(!list.text.contains("token_hash"));

        let used = app
            .get("/api/v1/state")
            .header("authorization", &bearer(&token))
            .send()
            .await;
        assert_eq!(used.status, StatusCode::OK);
        let listed = app.get("/api/me/tokens").cookie(&me).send().await.json();
        assert!(listed["tokens"][0]["last_used"].is_i64());

        let del = app
            .delete(&format!("/api/me/tokens/{id}"), None)
            .cookie(&me)
            .send()
            .await;
        assert_eq!(del.status, StatusCode::NO_CONTENT);
        let after = app
            .get("/api/v1/state")
            .header("authorization", &bearer(&token))
            .send()
            .await;
        assert_eq!(after.status, StatusCode::UNAUTHORIZED);
        let again = app
            .delete(&format!("/api/me/tokens/{id}"), None)
            .cookie(&me)
            .send()
            .await;
        assert_eq!(again.status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn tokens_belong_to_their_owner_and_are_limited_in_number() {
        let app = TestApp::new();
        let boss = admin(&app).await;
        app.post(
            "/api/admin/users",
            Some(json!({"username": "bob", "password": PW})),
        )
        .cookie(&boss)
        .send()
        .await;
        let bob = cookie_of(
            &app.post(
                "/api/signin",
                Some(json!({"username": "bob", "password": PW})),
            )
            .send()
            .await,
        );
        let (id, _) = make_token(&app, &boss, "boss").await;

        assert_eq!(
            app.get("/api/me/tokens").cookie(&bob).send().await.json()["tokens"],
            json!([])
        );
        let steal = app
            .delete(&format!("/api/me/tokens/{id}"), None)
            .cookie(&bob)
            .send()
            .await;
        assert_eq!(steal.status, StatusCode::NOT_FOUND);

        let empty = app
            .post("/api/me/tokens", Some(json!({"name": "  "})))
            .cookie(&bob)
            .send()
            .await;
        assert_eq!(empty.status, StatusCode::BAD_REQUEST);
        for i in 0..crate::accounts::MAX_API_TOKENS {
            make_token(&app, &bob, &format!("t{i}")).await;
        }
        let more = app
            .post("/api/me/tokens", Some(json!({"name": "one too many"})))
            .cookie(&bob)
            .send()
            .await;
        assert_eq!(more.json()["error"], "too_many_tokens");
    }

    #[tokio::test]
    async fn a_disabled_account_token_stops_working() {
        let app = TestApp::new();
        let boss = admin(&app).await;
        let created = app
            .post(
                "/api/admin/users",
                Some(json!({"username": "bob", "password": PW})),
            )
            .cookie(&boss)
            .send()
            .await;
        let bob_id = created.json()["id"].as_str().expect("id").to_string();
        let bob = cookie_of(
            &app.post(
                "/api/signin",
                Some(json!({"username": "bob", "password": PW})),
            )
            .send()
            .await,
        );
        let (_, token) = make_token(&app, &bob, "t").await;
        assert_eq!(
            app.get("/api/v1/state")
                .header("authorization", &bearer(&token))
                .send()
                .await
                .status,
            StatusCode::OK
        );
        let off = app
            .post(
                &format!("/api/admin/users/{bob_id}/disabled"),
                Some(json!({"disabled": true})),
            )
            .cookie(&boss)
            .send()
            .await;
        assert!(off.status.is_success(), "{}", off.text);
        assert_eq!(
            app.get("/api/v1/state")
                .header("authorization", &bearer(&token))
                .send()
                .await
                .status,
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn the_session_list_names_browsers_and_revokes_one_at_a_time() {
        let app = TestApp::new();
        let first = admin(&app).await;
        let second = cookie_of(
            &app.post(
                "/api/signin",
                Some(json!({"username": "admin", "password": PW})),
            )
            .header("user-agent", "Firefox on Linux")
            .send()
            .await,
        );

        let list: Value = app
            .get("/api/me/sessions")
            .cookie(&second)
            .send()
            .await
            .json();
        let sessions = list["sessions"].as_array().expect("list");
        assert_eq!(sessions.len(), 2);
        let mine = sessions
            .iter()
            .find(|s| s["current"] == true)
            .expect("current");
        assert_eq!(mine["agent"], "Firefox on Linux");
        let other = sessions
            .iter()
            .find(|s| s["current"] == false)
            .expect("other");
        let other_id = other["id"].as_str().expect("id").to_string();
        assert!(
            !list.to_string().contains(&first),
            "the list never contains a usable secret"
        );

        let own = app
            .delete(
                &format!("/api/me/sessions/{}", mine["id"].as_str().expect("id")),
                None,
            )
            .cookie(&second)
            .send()
            .await;
        assert_eq!(own.status, StatusCode::BAD_REQUEST);

        let gone = app
            .delete(&format!("/api/me/sessions/{other_id}"), None)
            .cookie(&second)
            .send()
            .await;
        assert_eq!(gone.status, StatusCode::NO_CONTENT);
        assert_eq!(
            app.get("/api/me").cookie(&first).send().await.status,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            app.get("/api/me").cookie(&second).send().await.status,
            StatusCode::OK
        );

        for bad in ["zzzz", "0123456789abcdef"] {
            let res = app
                .delete(&format!("/api/me/sessions/{bad}"), None)
                .cookie(&second)
                .send()
                .await;
            assert_eq!(res.status, StatusCode::NOT_FOUND, "{bad}");
        }
    }
}
