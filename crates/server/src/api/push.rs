//! Registering a browser for alerts while the app is closed (Web Push).

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post, put},
};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{ApiJson, AuthUser};
use crate::error::ApiError;
use crate::push::{self, Prefs};
use crate::state::Shared;

/// Subscriptions one person may have (a few browsers and phones).
const MAX_SUBSCRIPTIONS: usize = 10;

pub fn router() -> Router<Shared> {
    Router::new()
        .route("/api/push/key", get(key))
        .route("/api/push/subscription", put(subscribe).delete(unsubscribe))
        .route("/api/push/test", post(test))
}

/// The server's public key, which a browser needs to subscribe.
async fn key(State(state): State<Shared>, _auth: AuthUser) -> Result<Json<Value>, ApiError> {
    let vapid = push::vapid_key(&state.db()).map_err(ApiError::internal)?;
    Ok(Json(
        json!({ "key": crate::webpush::public_key_b64(&vapid) }),
    ))
}

#[derive(Deserialize)]
struct Keys {
    p256dh: String,
    auth: String,
}

#[derive(Deserialize)]
struct Subscribe {
    endpoint: String,
    keys: Keys,
    prefs: Prefs,
}

/// Add or update this browser's subscription (and what it wants to hear about).
async fn subscribe(
    State(state): State<Shared>,
    auth: AuthUser,
    ApiJson(body): ApiJson<Subscribe>,
) -> Result<StatusCode, ApiError> {
    if !push::endpoint_allowed(&body.endpoint) {
        return Err(ApiError::bad_request("bad_endpoint"));
    }
    if !push::keys_valid(&body.keys.p256dh, &body.keys.auth) {
        return Err(ApiError::bad_request("bad_keys"));
    }
    body.prefs.validate().map_err(ApiError::bad_request)?;
    let prefs = serde_json::to_string(&body.prefs).map_err(ApiError::internal)?;
    let db = state.db();
    let existing = db.push_subs_of(&auth.user.id)?;
    if existing.len() >= MAX_SUBSCRIPTIONS && !existing.iter().any(|s| s.endpoint == body.endpoint)
    {
        return Err(ApiError::bad_request("too_many_subscriptions"));
    }
    db.upsert_push_sub(
        &auth.user.id,
        &body.endpoint,
        &body.keys.p256dh,
        &body.keys.auth,
        &prefs,
        state.now_ms(),
    )?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct Unsubscribe {
    endpoint: String,
}

async fn unsubscribe(
    State(state): State<Shared>,
    auth: AuthUser,
    ApiJson(body): ApiJson<Unsubscribe>,
) -> Result<StatusCode, ApiError> {
    // Not finding it is fine: the goal (not subscribed) is already true.
    state.db().delete_push_sub(&auth.user.id, &body.endpoint)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Send a test message to all of this person's subscribed browsers, so they can see it works.
async fn test(State(state): State<Shared>, auth: AuthUser) -> Result<Json<Value>, ApiError> {
    let sent = push::send_to_user(&state, &auth.user.id, |_, p| {
        Some(push::message(
            &p.texts.test_title,
            &p.texts.test_body,
            "op-test",
        ))
    })
    .await;
    Ok(Json(json!({ "sent": sent })))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::{Value, json};

    use crate::test_util::{Res, TestApp};
    use crate::webpush::b64_encode;

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

    fn keys() -> Value {
        let public = p256::SecretKey::from_slice(&[3u8; 32])
            .expect("key")
            .public_key()
            .to_sec1_bytes();
        json!({"p256dh": b64_encode(&public), "auth": b64_encode(&[1u8; 16])})
    }

    fn prefs() -> Value {
        json!({
            "phase": true, "daily": null, "quiet": null, "tz": 60,
            "texts": {"focus_title": "Focus complete", "focus_body": "Break time.", "break_title": "Break over", "break_body": "Ready?",
                      "daily_title": "Time to focus?", "daily_body": "Daily reminder.", "test_title": "It works", "test_body": "Test."}
        })
    }

    fn body(endpoint: &str) -> Value {
        json!({"endpoint": endpoint, "keys": keys(), "prefs": prefs()})
    }

    #[tokio::test]
    async fn the_key_is_stable_and_needs_a_sign_in() {
        let app = TestApp::new();
        assert_eq!(
            app.get("/api/push/key").send().await.status,
            StatusCode::UNAUTHORIZED
        );
        let me = admin(&app).await;
        let first = app.get("/api/push/key").cookie(&me).send().await.json();
        let second = app.get("/api/push/key").cookie(&me).send().await.json();
        assert_eq!(first, second);
        assert_eq!(
            first["key"].as_str().expect("key").len(),
            87,
            "65 bytes as base64url"
        );
    }

    #[tokio::test]
    async fn subscribing_checks_the_endpoint_the_keys_and_the_preferences() {
        let app = TestApp::new();
        let me = admin(&app).await;
        let ok = "https://fcm.googleapis.com/fcm/send/abc";
        let put = |b: Value| app.put("/api/push/subscription", Some(b));
        assert_eq!(
            put(body(ok)).cookie(&me).send().await.status,
            StatusCode::NO_CONTENT
        );
        assert_eq!(
            put(body(ok)).cookie(&me).send().await.status,
            StatusCode::NO_CONTENT,
            "again just updates"
        );
        assert_eq!(
            put(body("https://evil.example/x"))
                .cookie(&me)
                .send()
                .await
                .json()["error"],
            "bad_endpoint"
        );
        let mut bad_keys = body(ok);
        bad_keys["keys"]["auth"] = json!("AAAA");
        assert_eq!(
            put(bad_keys).cookie(&me).send().await.json()["error"],
            "bad_keys"
        );
        let mut bad_prefs = body(ok);
        bad_prefs["prefs"]["tz"] = json!(99_999);
        assert_eq!(
            put(bad_prefs).cookie(&me).send().await.json()["error"],
            "bad_tz"
        );
        assert_eq!(put(body(ok)).send().await.status, StatusCode::UNAUTHORIZED);
        let no_csrf = put(body(ok)).cookie(&me).without_csrf().send().await;
        assert_eq!(no_csrf.status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn a_person_has_a_limited_number_of_subscriptions_and_can_remove_one() {
        let app = TestApp::new();
        let me = admin(&app).await;
        for i in 0..10 {
            let res = app
                .put(
                    "/api/push/subscription",
                    Some(body(&format!("https://fcm.googleapis.com/fcm/send/{i}"))),
                )
                .cookie(&me)
                .send()
                .await;
            assert_eq!(res.status, StatusCode::NO_CONTENT);
        }
        let eleventh = app
            .put(
                "/api/push/subscription",
                Some(body("https://fcm.googleapis.com/fcm/send/extra")),
            )
            .cookie(&me)
            .send()
            .await;
        assert_eq!(eleventh.json()["error"], "too_many_subscriptions");
        let gone = app
            .delete(
                "/api/push/subscription",
                Some(json!({"endpoint": "https://fcm.googleapis.com/fcm/send/3"})),
            )
            .cookie(&me)
            .send()
            .await;
        assert_eq!(gone.status, StatusCode::NO_CONTENT);
        let again = app
            .put(
                "/api/push/subscription",
                Some(body("https://fcm.googleapis.com/fcm/send/extra")),
            )
            .cookie(&me)
            .send()
            .await;
        assert_eq!(again.status, StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn a_subscription_belongs_to_one_person_and_the_test_reaches_only_their_own() {
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
        let endpoint = "https://fcm.googleapis.com/fcm/send/shared";
        app.put("/api/push/subscription", Some(body(endpoint)))
            .cookie(&boss)
            .send()
            .await;
        // Bob cannot remove the boss's subscription.
        app.delete(
            "/api/push/subscription",
            Some(json!({"endpoint": endpoint})),
        )
        .cookie(&bob)
        .send()
        .await;
        let boss_id = admin_id(&app);
        let subs = app.state.db().push_subs_of(&boss_id).expect("subs");
        assert_eq!(subs.len(), 1);
        // Bob has none, so a test sends to nobody (and tries no network).
        let sent = app
            .post("/api/push/test", None)
            .cookie(&bob)
            .send()
            .await
            .json();
        assert_eq!(sent["sent"], 0);
    }

    fn admin_id(app: &TestApp) -> String {
        app.state
            .db()
            .list_users()
            .expect("users")
            .into_iter()
            .find(|u| u.username == "admin")
            .expect("admin")
            .id
    }
}
