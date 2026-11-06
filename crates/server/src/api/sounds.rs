//! Uploaded alert sounds of the signed-in user: list, upload, fetch and remove one per alert slot.

use axum::{
    Json, Router,
    body::Bytes,
    extract::{Path, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use serde_json::{Value, json};

use super::AuthUser;
use crate::error::ApiError;
use crate::hub::CHANGED;
use crate::sounds::{MAX_BYTES, SLOTS, sniff};
use crate::state::Shared;

pub fn router() -> Router<Shared> {
    Router::new()
        .route("/api/sounds", get(list))
        .route("/api/sounds/{slot}", get(fetch).put(upload).delete(remove))
}

fn check_slot(slot: &str) -> Result<(), ApiError> {
    if SLOTS.contains(&slot) {
        Ok(())
    } else {
        Err(ApiError::not_found())
    }
}

async fn list(State(state): State<Shared>, auth: AuthUser) -> Result<Json<Value>, ApiError> {
    let sounds: Vec<Value> = state
        .db()
        .sounds(&auth.user.id)?
        .into_iter()
        .map(|(slot, mime, size, updated_at)| json!({"slot": slot, "mime": mime, "size": size, "updated_at": updated_at}))
        .collect();
    Ok(Json(json!({ "sounds": sounds })))
}

async fn upload(
    State(state): State<Shared>,
    auth: AuthUser,
    Path(slot): Path<String>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    check_slot(&slot)?;
    if body.len() > MAX_BYTES {
        return Err(ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "sound_too_large",
        ));
    }
    let Some(mime) = sniff(&body) else {
        return Err(ApiError::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "not_audio",
        ));
    };
    let now = state.now_ms();
    state
        .db()
        .put_sound(&auth.user.id, &slot, mime, &body, now)?;
    state.hub.publish(&auth.user.id, CHANGED);
    Ok(Json(
        json!({"slot": slot, "mime": mime, "size": body.len(), "updated_at": now}),
    ))
}

async fn fetch(
    State(state): State<Shared>,
    auth: AuthUser,
    Path(slot): Path<String>,
) -> Result<Response, ApiError> {
    check_slot(&slot)?;
    let Some((mime, data)) = state.db().sound(&auth.user.id, &slot)? else {
        return Err(ApiError::not_found());
    };
    let mut res = data.into_response();
    let headers = res.headers_mut();
    if let Ok(value) = HeaderValue::from_str(&mime) {
        headers.insert(header::CONTENT_TYPE, value);
    }
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-cache"),
    );
    Ok(res)
}

async fn remove(
    State(state): State<Shared>,
    auth: AuthUser,
    Path(slot): Path<String>,
) -> Result<StatusCode, ApiError> {
    check_slot(&slot)?;
    if !state.db().delete_sound(&auth.user.id, &slot)? {
        return Err(ApiError::not_found());
    }
    state.hub.publish(&auth.user.id, CHANGED);
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::json;

    use crate::test_util::TestApp;

    const PW: &str = "correct horse battery staple";

    fn wav() -> Vec<u8> {
        let mut v = b"RIFF\x24\0\0\0WAVEfmt ".to_vec();
        v.extend_from_slice(&[0; 32]);
        v
    }

    async fn signed_in(app: &TestApp, name: &str, admin: Option<&str>) -> String {
        let cookie_of = |res: &crate::test_util::Res| {
            let raw = res.header("set-cookie").expect("cookie");
            raw.split(';')
                .next()
                .expect("pair")
                .split_once('=')
                .expect("kv")
                .1
                .to_string()
        };
        match admin {
            None => cookie_of(
                &app.post(
                    "/api/setup",
                    Some(json!({"username": name, "password": PW})),
                )
                .send()
                .await,
            ),
            Some(boss) => {
                app.post(
                    "/api/admin/users",
                    Some(json!({"username": name, "password": PW})),
                )
                .cookie(boss)
                .send()
                .await;
                cookie_of(
                    &app.post(
                        "/api/signin",
                        Some(json!({"username": name, "password": PW})),
                    )
                    .send()
                    .await,
                )
            }
        }
    }

    #[tokio::test]
    async fn upload_list_fetch_and_remove() {
        let app = TestApp::new();
        let me = signed_in(&app, "admin", None).await;

        let anon = app
            .put("/api/sounds/focus_end", None)
            .raw(wav())
            .send()
            .await;
        assert_eq!(anon.status, StatusCode::UNAUTHORIZED);

        let put = app
            .put("/api/sounds/focus_end", None)
            .cookie(&me)
            .raw(wav())
            .send()
            .await;
        assert_eq!(put.status, StatusCode::OK, "{}", put.text);
        assert_eq!(put.json()["mime"], "audio/wav");

        let list = app.get("/api/sounds").cookie(&me).send().await;
        assert_eq!(list.json()["sounds"][0]["slot"], "focus_end");

        let got = app.get("/api/sounds/focus_end").cookie(&me).send().await;
        assert_eq!(got.status, StatusCode::OK);
        assert_eq!(got.header("content-type").as_deref(), Some("audio/wav"));
        assert_eq!(got.bytes, wav());

        let del = app
            .delete("/api/sounds/focus_end", None)
            .cookie(&me)
            .send()
            .await;
        assert_eq!(del.status, StatusCode::NO_CONTENT);
        let gone = app.get("/api/sounds/focus_end").cookie(&me).send().await;
        assert_eq!(gone.status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn only_real_audio_in_known_slots_within_the_size_limit() {
        let app = TestApp::new();
        let me = signed_in(&app, "admin", None).await;

        let html = app
            .put("/api/sounds/focus_end", None)
            .cookie(&me)
            .raw(b"<script>x</script>".to_vec())
            .send()
            .await;
        assert_eq!(html.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);

        let slot = app
            .put("/api/sounds/evil", None)
            .cookie(&me)
            .raw(wav())
            .send()
            .await;
        assert_eq!(slot.status, StatusCode::NOT_FOUND);

        let mut big = wav();
        big.resize(crate::sounds::MAX_BYTES + 1, 0);
        let huge = app
            .put("/api/sounds/focus_end", None)
            .cookie(&me)
            .raw(big)
            .send()
            .await;
        assert_eq!(huge.status, StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn sounds_are_private_to_their_owner() {
        let app = TestApp::new();
        let boss = signed_in(&app, "admin", None).await;
        let alice = signed_in(&app, "alice", Some(&boss)).await;
        let bob = signed_in(&app, "bob", Some(&boss)).await;
        app.put("/api/sounds/break_end", None)
            .cookie(&alice)
            .raw(wav())
            .send()
            .await;

        assert_eq!(
            app.get("/api/sounds/break_end")
                .cookie(&bob)
                .send()
                .await
                .status,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            app.get("/api/sounds").cookie(&bob).send().await.json()["sounds"],
            json!([])
        );
        assert_eq!(
            app.delete("/api/sounds/break_end", None)
                .cookie(&bob)
                .send()
                .await
                .status,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            app.get("/api/sounds/break_end")
                .cookie(&alice)
                .send()
                .await
                .status,
            StatusCode::OK
        );
    }
}
