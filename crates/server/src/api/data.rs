//! The signed-in user's data: current state, timer commands, event intake and history.

use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
};
use op_core::{Applied, Entity, EventRecord, Reason, Row};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{ApiJson, AuthUser};
use crate::error::ApiError;
use crate::settings::Settings;
use crate::state::Shared;
use crate::timer_service::{self, Command, CommandError, CommandResult, TimerState};
use crate::validate;

/// Most events accepted in one request.
const MAX_BATCH: usize = 100;
/// Longest history window and most rows returned.
const MAX_HISTORY_SPAN_MS: i64 = 366 * 24 * 60 * 60 * 1000;
const MAX_HISTORY_ROWS: usize = 5000;

pub fn router() -> Router<Shared> {
    Router::new()
        .route("/api/state", get(get_state))
        .route("/api/commands", post(post_command))
        .route("/api/events", post(post_events))
        .route("/api/history", get(get_history))
}

fn row_json(key: &str, row: &Row) -> Value {
    let mut map = row.data.clone();
    map.insert("id".into(), json!(key));
    Value::Object(map)
}

/// Everything a client needs to draw the app: the clock, the timer, the effective settings, the categories.
fn state_json(state: &Shared, user: &str, timer: &TimerState) -> Result<Value, ApiError> {
    let db = state.db();
    let settings = Settings::load(&db, user)?;
    let categories: Vec<Value> = db
        .rows(user, Entity::Category)?
        .iter()
        .map(|(k, r)| row_json(k, r))
        .collect();
    Ok(json!({
        "server_time_ms": state.now_ms(),
        "timer": timer.to_json(),
        "settings": settings.to_json(),
        "categories": categories,
    }))
}

async fn get_state(State(state): State<Shared>, auth: AuthUser) -> Result<Json<Value>, ApiError> {
    let timer = TimerState::load(&state.db(), &auth.user.id)?;
    Ok(Json(state_json(&state, &auth.user.id, &timer)?))
}

fn command_error(e: CommandError) -> ApiError {
    match e {
        CommandError::Invalid(code) => ApiError::bad_request(code),
        CommandError::Rejected(reason) => ApiError::new(
            StatusCode::CONFLICT,
            match reason {
                "strict" => "rejected_strict",
                "not_ended" => "rejected_not_ended",
                "unknown" => "rejected_unknown",
                _ => "rejected",
            },
        ),
        CommandError::Db(cause) => ApiError::internal(cause),
    }
}

async fn post_command(
    State(state): State<Shared>,
    auth: AuthUser,
    ApiJson(cmd): ApiJson<Command>,
) -> Result<Json<Value>, ApiError> {
    let user = auth.user.id;
    let (status, timer) = {
        let mut db = state.db();
        let mut ids = state.ids();
        match timer_service::apply_command(&mut db, &user, &cmd, state.now_ms(), &mut ids)
            .map_err(command_error)?
        {
            CommandResult::Applied(timer) => ("applied", timer),
            CommandResult::Duplicate => ("duplicate", TimerState::load(&db, &user)?),
        }
    };
    Ok(Json(
        json!({ "status": status, "state": state_json(&state, &user, &timer)? }),
    ))
}

#[derive(Deserialize)]
struct EventBatch {
    events: Vec<EventRecord>,
}

fn applied_name(applied: Applied) -> &'static str {
    match applied {
        Applied::Added => "added",
        Applied::Edited => "edited",
        Applied::Deleted => "deleted",
        Applied::Duplicate => "duplicate",
        Applied::Stale => "stale",
        Applied::Superseded => "superseded",
        // An edit or delete that arrived before its add: kept, and applied when the add shows up.
        Applied::Rejected(Reason::Missing) => "waiting",
        Applied::Rejected(Reason::BadPayload) => "invalid",
    }
}

#[derive(Serialize)]
struct EventResult {
    id: String,
    result: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<&'static str>,
}

/// Data edits (settings, categories, session notes and moods, retroactive sessions), each validated on its own.
/// This is also the intake for the offline outbox, so every event answers separately.
async fn post_events(
    State(state): State<Shared>,
    auth: AuthUser,
    ApiJson(batch): ApiJson<EventBatch>,
) -> Result<Json<Value>, ApiError> {
    if batch.events.len() > MAX_BATCH {
        return Err(ApiError::bad_request("too_many_events"));
    }
    let now = state.now_ms();
    let mut results = Vec::with_capacity(batch.events.len());
    for event in &batch.events {
        let result = match validate::check_client_event(event, now) {
            Err(error) => EventResult {
                id: event.id.clone(),
                result: "invalid",
                error: Some(error),
            },
            Ok(()) => {
                let applied = state.db().apply_event(&auth.user.id, event)?;
                EventResult {
                    id: event.id.clone(),
                    result: applied_name(applied),
                    error: None,
                }
            }
        };
        results.push(result);
    }
    Ok(Json(json!({ "results": results })))
}

#[derive(Deserialize)]
struct HistoryQuery {
    from: i64,
    to: i64,
}

async fn get_history(
    State(state): State<Shared>,
    auth: AuthUser,
    Query(q): Query<HistoryQuery>,
) -> Result<Json<Value>, ApiError> {
    if q.from >= q.to || q.to - q.from > MAX_HISTORY_SPAN_MS {
        return Err(ApiError::bad_request("bad_range"));
    }
    let mut rows = state.db().sessions_between(&auth.user.id, q.from, q.to)?;
    let truncated = rows.len() > MAX_HISTORY_ROWS;
    rows.truncate(MAX_HISTORY_ROWS);
    let sessions: Vec<Value> = rows.iter().map(|(k, r)| row_json(k, r)).collect();
    Ok(Json(
        json!({ "sessions": sessions, "truncated": truncated }),
    ))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use op_core::IdGen;
    use serde_json::{Value, json};

    use crate::test_util::{Res, TestApp};

    const PW: &str = "correct horse battery";
    const MIN: i64 = 60_000;

    /// A signed-in user with their own client-side id generator, like a browser would have.
    struct Client {
        cookie: String,
        ids: IdGen,
    }

    impl Client {
        fn id(&mut self, app: &TestApp) -> String {
            self.ids.next(app.state.now_ms())
        }

        async fn command(&mut self, app: &TestApp, name: &str, extra: Value) -> Res {
            let mut body = json!({"id": self.id(app), "name": name});
            if let (Some(base), Some(more)) = (body.as_object_mut(), extra.as_object()) {
                base.extend(more.clone());
            }
            app.post("/api/commands", Some(body))
                .cookie(&self.cookie)
                .send()
                .await
        }

        async fn state(&self, app: &TestApp) -> Value {
            let res = app.get("/api/state").cookie(&self.cookie).send().await;
            assert_eq!(res.status, StatusCode::OK, "{}", res.text);
            res.json()
        }

        async fn events(&self, app: &TestApp, events: Value) -> Res {
            app.post("/api/events", Some(json!({"events": events})))
                .cookie(&self.cookie)
                .send()
                .await
        }
    }

    /// A history query covering the last 300 days up to a minute from now.
    fn recent(app: &TestApp) -> String {
        let now = app.state.now_ms();
        format!(
            "/api/history?from={}&to={}",
            now - 300 * 24 * 60 * MIN,
            now + MIN
        )
    }

    fn cookie_of(res: &Res) -> String {
        let set = res.header("set-cookie").expect("cookie");
        set.split(';')
            .next()
            .and_then(|kv| kv.split_once('='))
            .map(|(_, v)| v.to_string())
            .expect("value")
    }

    async fn admin(app: &TestApp) -> Client {
        let res = app
            .post(
                "/api/setup",
                Some(json!({"username": "admin", "password": PW})),
            )
            .send()
            .await;
        Client {
            cookie: cookie_of(&res),
            ids: IdGen::new("browser-admin"),
        }
    }

    async fn user(app: &TestApp, admin: &Client, name: &str) -> Client {
        let res = app
            .post(
                "/api/admin/users",
                Some(json!({"username": name, "password": PW})),
            )
            .cookie(&admin.cookie)
            .send()
            .await;
        assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
        let res = app
            .post(
                "/api/signin",
                Some(json!({"username": name, "password": PW})),
            )
            .send()
            .await;
        Client {
            cookie: cookie_of(&res),
            ids: IdGen::new(&format!("browser-{name}")),
        }
    }

    #[tokio::test]
    async fn every_data_route_needs_a_session() {
        let app = TestApp::new();
        admin(&app).await;
        assert_eq!(
            app.get("/api/state").send().await.status,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            app.get("/api/history?from=0&to=1000").send().await.status,
            StatusCode::UNAUTHORIZED
        );
        let cmd = Some(json!({"id": "x", "name": "init_focus"}));
        assert_eq!(
            app.post("/api/commands", cmd).send().await.status,
            StatusCode::UNAUTHORIZED
        );
        let events = Some(json!({"events": []}));
        assert_eq!(
            app.post("/api/events", events).send().await.status,
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn a_new_account_starts_idle_with_default_settings() {
        let app = TestApp::new();
        let me = admin(&app).await;
        let state = me.state(&app).await;
        assert_eq!(state["timer"]["state"], "idle");
        assert_eq!(state["settings"]["focus_ms"], 25 * MIN);
        assert_eq!(state["settings"]["strict"], false);
        assert_eq!(state["categories"], json!([]));
        assert_eq!(state["server_time_ms"], app.state.now_ms());
    }

    #[tokio::test]
    async fn a_focus_session_runs_end_to_end_over_http() {
        let app = TestApp::new();
        let mut me = admin(&app).await;

        let res = me
            .command(
                &app,
                "init_focus",
                json!({"intention": "write", "category": "work"}),
            )
            .await;
        assert_eq!(res.status, StatusCode::OK, "{}", res.text);
        assert_eq!(res.json()["status"], "applied");
        assert_eq!(res.json()["state"]["timer"]["state"], "session");
        assert_eq!(res.json()["state"]["timer"]["total"], 25 * MIN);

        app.clock.advance(25 * MIN);
        assert_eq!(
            me.command(&app, "timer_ends", json!({})).await.json()["state"]["timer"]["state"],
            "session_end"
        );
        assert_eq!(
            me.command(&app, "stop", json!({})).await.json()["state"]["timer"]["state"],
            "reflect"
        );
        assert_eq!(
            me.command(&app, "done", json!({})).await.json()["state"]["timer"]["state"],
            "idle"
        );
        let history = app
            .get(&recent(&app))
            .cookie(&me.cookie)
            .send()
            .await
            .json();
        let sessions = history["sessions"].as_array().expect("sessions");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0]["kind"], "focus");
        assert_eq!(sessions[0]["intention"], "write");
        assert_eq!(sessions[0]["duration_ms"], 25 * MIN);
        assert_eq!(history["truncated"], false);
    }

    #[tokio::test]
    async fn rejected_and_invalid_commands_have_distinct_codes() {
        let app = TestApp::new();
        let mut me = admin(&app).await;

        let res = me.command(&app, "done", json!({})).await;
        assert_eq!(
            (res.status, res.json()["error"].clone()),
            (StatusCode::CONFLICT, json!("rejected_unknown"))
        );

        me.command(&app, "init_focus", json!({})).await;
        let res = me.command(&app, "timer_ends", json!({})).await;
        assert_eq!(
            (res.status, res.json()["error"].clone()),
            (StatusCode::CONFLICT, json!("rejected_not_ended"))
        );

        let res = me.command(&app, "teleport", json!({})).await;
        assert_eq!(
            (res.status, res.json()["error"].clone()),
            (StatusCode::BAD_REQUEST, json!("unknown_command"))
        );

        let res = app
            .post("/api/commands", Some(json!({"id": "junk", "name": "stop"})))
            .cookie(&me.cookie)
            .send()
            .await;
        assert_eq!(res.json()["error"], "bad_id");
        let res = app
            .post("/api/commands", Some(json!({"nope": 1})))
            .cookie(&me.cookie)
            .send()
            .await;
        assert_eq!(res.json()["error"], "invalid_json");
    }

    #[tokio::test]
    async fn strict_mode_is_enforced_by_the_server() {
        let app = TestApp::new();
        let mut me = admin(&app).await;
        let id = me.id(&app);
        let set = json!([{"id": id, "entity": "setting", "op": "add", "key": "strict", "payload": {"value": true}}]);
        assert_eq!(
            me.events(&app, set).await.json()["results"][0]["result"],
            "added"
        );
        me.command(&app, "init_focus", json!({})).await;
        let res = me.command(&app, "abandon", json!({})).await;
        assert_eq!(
            (res.status, res.json()["error"].clone()),
            (StatusCode::CONFLICT, json!("rejected_strict"))
        );
    }

    #[tokio::test]
    async fn a_retried_command_is_reported_as_a_duplicate() {
        let app = TestApp::new();
        let me = admin(&app).await;
        let body = json!({"id": me.ids.clone().next(app.state.now_ms()), "name": "init_focus"});
        let first = app
            .post("/api/commands", Some(body.clone()))
            .cookie(&me.cookie)
            .send()
            .await;
        assert_eq!(first.json()["status"], "applied");
        app.clock.advance(MIN);
        let again = app
            .post("/api/commands", Some(body))
            .cookie(&me.cookie)
            .send()
            .await;
        assert_eq!(
            (again.status, again.json()["status"].clone()),
            (StatusCode::OK, json!("duplicate"))
        );
        assert_eq!(again.json()["state"]["timer"]["state"], "session");
    }

    #[tokio::test]
    async fn settings_events_change_what_the_timer_does() {
        let app = TestApp::new();
        let mut me = admin(&app).await;
        let id = me.id(&app);
        let set = json!([{"id": id, "entity": "setting", "op": "add", "key": "focus_ms", "payload": {"value": 50 * MIN}}]);
        assert_eq!(
            me.events(&app, set).await.json()["results"][0]["result"],
            "added"
        );
        assert_eq!(me.state(&app).await["settings"]["focus_ms"], 50 * MIN);
        let res = me.command(&app, "init_focus", json!({})).await;
        assert_eq!(res.json()["state"]["timer"]["total"], 50 * MIN);
    }

    #[tokio::test]
    async fn event_intake_answers_each_event_separately() {
        let app = TestApp::new();
        let mut me = admin(&app).await;
        let (a, b, c, d) = (me.id(&app), me.id(&app), me.id(&app), me.id(&app));
        let batch = json!([
            {"id": a, "entity": "category", "op": "add", "key": "c1", "payload": {"name": "Writing", "color": "peach"}},
            {"id": a, "entity": "category", "op": "add", "key": "c1", "payload": {"name": "Writing", "color": "peach"}},
            {"id": b, "entity": "timer", "op": "add", "key": "current", "payload": {"state": "session"}},
            {"id": c, "entity": "setting", "op": "add", "key": "focus_ms", "payload": {"value": 5}},
            {"id": d, "entity": "session", "op": "edit", "key": "not-there-yet", "payload": {"mood": "focused"}},
            {"id": "junk", "entity": "session", "op": "delete", "key": "x", "payload": null},
        ]);
        let res = me.events(&app, batch).await;
        assert_eq!(res.status, StatusCode::OK, "{}", res.text);
        let results: Vec<(String, Option<String>)> = res.json()["results"]
            .as_array()
            .expect("results")
            .iter()
            .map(|r| {
                (
                    r["result"].as_str().expect("result").to_string(),
                    r["error"].as_str().map(str::to_string),
                )
            })
            .collect();
        let expect = |r: &str, e: Option<&str>| (r.to_string(), e.map(str::to_string));
        assert_eq!(
            results,
            [
                expect("added", None),
                expect("duplicate", None),
                expect("invalid", Some("forbidden_entity")),
                expect("invalid", Some("bad_setting_value")),
                expect("waiting", None),
                expect("invalid", Some("bad_id")),
            ]
        );
        assert_eq!(me.state(&app).await["categories"][0]["name"], "Writing");
        assert_eq!(
            me.state(&app).await["timer"]["state"],
            "idle",
            "clients cannot write the timer directly"
        );
    }

    #[tokio::test]
    async fn too_many_events_in_one_request_are_refused() {
        let app = TestApp::new();
        let me = admin(&app).await;
        let events: Vec<Value> = (0..101)
            .map(
                |i| json!({"id": format!("e{i}"), "entity": "session", "op": "delete", "key": "x"}),
            )
            .collect();
        let res = me.events(&app, json!(events)).await;
        assert_eq!(
            (res.status, res.json()["error"].clone()),
            (StatusCode::BAD_REQUEST, json!("too_many_events"))
        );
    }

    #[tokio::test]
    async fn ids_from_the_far_future_are_refused() {
        let app = TestApp::new();
        let me = admin(&app).await;
        let far = IdGen::new("clock-gone-wrong").next(app.state.now_ms() + 3 * 24 * 60 * MIN);
        let ev = json!([{"id": far, "entity": "setting", "op": "add", "key": "strict", "payload": {"value": true}}]);
        let res = me.events(&app, ev).await;
        assert_eq!(res.json()["results"][0]["error"], "id_in_future");
        assert_eq!(me.state(&app).await["settings"]["strict"], false);
    }

    #[tokio::test]
    async fn history_bounds_are_checked() {
        let app = TestApp::new();
        let me = admin(&app).await;
        for q in ["from=10&to=10", "from=10&to=5", "from=0&to=99999999999999"] {
            let res = app
                .get(&format!("/api/history?{q}"))
                .cookie(&me.cookie)
                .send()
                .await;
            assert_eq!(
                (res.status, res.json()["error"].clone()),
                (StatusCode::BAD_REQUEST, json!("bad_range")),
                "{q}"
            );
        }
    }

    #[tokio::test]
    async fn users_cannot_see_or_touch_each_others_data() {
        let app = TestApp::new();
        let boss = admin(&app).await;
        let mut alice = user(&app, &boss, "alice").await;
        let mut bob = user(&app, &boss, "bob").await;

        // Alice works, sets a setting and a category.
        alice
            .command(&app, "init_focus", json!({"intention": "secret plans"}))
            .await;
        app.clock.advance(25 * MIN);
        alice.command(&app, "timer_ends", json!({})).await;
        alice.command(&app, "stop", json!({})).await;
        alice.command(&app, "done", json!({})).await;
        let id = alice.id(&app);
        let ev = json!([
            {"id": id, "entity": "setting", "op": "add", "key": "strict", "payload": {"value": true}},
        ]);
        alice.events(&app, ev).await;

        // Bob sees none of it: not in his state, not in his history.
        let state = bob.state(&app).await;
        assert_eq!(state["timer"]["state"], "idle");
        assert_eq!(state["timer"]["rounds"], 0);
        assert_eq!(state["settings"]["strict"], false);
        let history = app.get(&recent(&app)).cookie(&bob.cookie).send().await;
        assert_eq!(history.json()["sessions"], json!([]));
        assert!(!history.text.contains("secret plans"));

        // Bob editing a row key that exists for Alice only waits for his own add; Alice's row is untouched.
        let alice_history = app
            .get(&recent(&app))
            .cookie(&alice.cookie)
            .send()
            .await
            .json();
        let alice_session = alice_history["sessions"][0]["id"]
            .as_str()
            .expect("id")
            .to_string();
        let id = bob.id(&app);
        let probe = json!([{"id": id, "entity": "session", "op": "edit", "key": alice_session, "payload": {"notes": "mine now"}}]);
        assert_eq!(
            bob.events(&app, probe).await.json()["results"][0]["result"],
            "waiting"
        );
        let after = app
            .get(&recent(&app))
            .cookie(&alice.cookie)
            .send()
            .await
            .json();
        assert_eq!(
            after["sessions"][0].get("notes"),
            None,
            "Alice's row did not change"
        );

        // Bob's own timer is independent of Alice's.
        assert_eq!(
            bob.command(&app, "init_focus", json!({})).await.json()["state"]["timer"]["state"],
            "session"
        );
        assert_eq!(alice.state(&app).await["timer"]["state"], "idle");
    }

    #[tokio::test]
    async fn data_posts_need_the_csrf_header() {
        let app = TestApp::new();
        let me = admin(&app).await;
        let cmd =
            Some(json!({"id": me.ids.clone().next(app.state.now_ms()), "name": "init_focus"}));
        let res = app
            .post("/api/commands", cmd)
            .cookie(&me.cookie)
            .without_csrf()
            .send()
            .await;
        assert_eq!(res.status, StatusCode::FORBIDDEN);
        assert_eq!(me.state(&app).await["timer"]["state"], "idle");
    }

    #[tokio::test]
    async fn a_disabled_account_loses_access_to_its_data() {
        let app = TestApp::new();
        let boss = admin(&app).await;
        let alice = user(&app, &boss, "alice").await;
        let list = app
            .get("/api/admin/users")
            .cookie(&boss.cookie)
            .send()
            .await
            .json();
        let id = list
            .as_array()
            .expect("list")
            .iter()
            .find(|u| u["username"] == "alice")
            .expect("alice")["id"]
            .as_str()
            .expect("id")
            .to_string();
        app.post(
            &format!("/api/admin/users/{id}/disabled"),
            Some(json!({"disabled": true})),
        )
        .cookie(&boss.cookie)
        .send()
        .await;
        assert_eq!(
            app.get("/api/state")
                .cookie(&alice.cookie)
                .send()
                .await
                .status,
            StatusCode::UNAUTHORIZED
        );
    }
}
