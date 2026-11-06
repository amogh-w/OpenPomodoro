//! Sign-up, sign-in, sign-out, "me", and the admin routes for accounts.

use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

use super::{AdminUser, ApiJson, AuthUser, ClientAddr, blocking};
use crate::accounts::{Registration, SESSION_TTL_MS, Session};
use crate::config::Config;
use crate::db::{Role, User};
use crate::error::ApiError;
use crate::state::Shared;

pub fn router() -> Router<Shared> {
    Router::new()
        .route("/api/setup", get(setup_status).post(setup))
        .route("/api/register", post(register))
        .route("/api/signin", post(sign_in))
        .route("/api/signout", post(sign_out))
        .route("/api/me", get(me).delete(delete_me))
        .route("/api/me/password", post(change_password))
        .route("/api/me/wipe", post(wipe_me))
        .route("/api/me/tokens", get(list_tokens).post(create_token))
        .route("/api/me/tokens/{id}", axum::routing::delete(delete_token))
        .route("/api/me/totp", get(totp_status).delete(totp_disable))
        .route("/api/me/totp/start", post(totp_start))
        .route("/api/me/totp/enable", post(totp_enable))
        .route("/api/me/sessions", get(list_sessions))
        .route(
            "/api/me/sessions/{id}",
            axum::routing::delete(delete_session),
        )
        .route("/api/admin/backup", get(admin_backup))
        .route("/api/admin/log", get(admin_log))
        .route("/api/admin/hooks", get(list_hooks))
        .route("/api/admin/hooks/{index}/test", post(test_hook))
        .route("/api/admin/webhooks/{index}/test", post(test_webhook))
        .route("/api/admin/users", get(list_users).post(create_user))
        .route("/api/admin/users/{id}/password", post(set_user_password))
        .route("/api/admin/users/{id}/disabled", post(set_user_disabled))
        .route(
            "/api/admin/users/{id}/totp",
            axum::routing::delete(reset_user_totp),
        )
        .route(
            "/api/admin/registration",
            get(get_registration).put(put_registration),
        )
}

#[derive(Serialize)]
pub struct UserJson {
    id: String,
    username: String,
    role: &'static str,
    disabled: bool,
    created_at: i64,
}

impl From<&User> for UserJson {
    fn from(u: &User) -> Self {
        Self {
            id: u.id.clone(),
            username: u.username.clone(),
            role: u.role.code(),
            disabled: u.disabled,
            created_at: u.created_at,
        }
    }
}

fn session_cookie(config: &Config, token: &str, max_age_secs: i64) -> String {
    let secure = if config.https() { "; Secure" } else { "" };
    format!(
        "{}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age_secs}{secure}",
        config.session_cookie_name()
    )
}

fn with_cookie(
    config: &Config,
    cookie: &str,
    status: StatusCode,
    body: impl Serialize,
) -> Response {
    let mut res = (status, Json(body)).into_response();
    if let Ok(value) = HeaderValue::from_str(cookie) {
        res.headers_mut().append(header::SET_COOKIE, value);
    }
    let _ = config;
    res
}

fn signed_in(config: &Config, session: &Session) -> Response {
    let cookie = session_cookie(config, &session.token, SESSION_TTL_MS / 1000);
    with_cookie(
        config,
        &cookie,
        StatusCode::OK,
        UserJson::from(&session.user),
    )
}

fn signed_out(config: &Config, status: StatusCode) -> Response {
    let mut res = status.into_response();
    if let Ok(value) = HeaderValue::from_str(&session_cookie(config, "", 0)) {
        res.headers_mut().append(header::SET_COOKIE, value);
    }
    res
}

#[derive(Serialize)]
struct SetupStatus {
    needs_setup: bool,
    registration: &'static str,
}

fn registration_name(r: Registration) -> &'static str {
    if r == Registration::Open {
        "open"
    } else {
        "closed"
    }
}

async fn setup_status(State(state): State<Shared>) -> Result<Json<SetupStatus>, ApiError> {
    let db = state.db();
    Ok(Json(SetupStatus {
        needs_setup: state.accounts.needs_setup(&db)?,
        registration: registration_name(state.accounts.registration(&db)?),
    }))
}

#[derive(Deserialize)]
struct Credentials {
    username: String,
    password: String,
    /// A one-time code or recovery code, for accounts with two-factor sign-in.
    #[serde(default)]
    code: Option<String>,
}

/// First-run only: create the admin account and sign in.
async fn setup(
    State(state): State<Shared>,
    ApiJson(body): ApiJson<Credentials>,
) -> Result<Response, ApiError> {
    let now = state.now_ms();
    let session = blocking(&state, move |s| {
        s.accounts
            .setup_admin(&s.db(), &body.username, &body.password, now)
    })
    .await??;
    Ok(signed_in(&state.config, &session))
}

async fn register(
    State(state): State<Shared>,
    ApiJson(body): ApiJson<Credentials>,
) -> Result<Response, ApiError> {
    let now = state.now_ms();
    let session = blocking(&state, move |s| {
        s.accounts
            .register(&s.db(), &body.username, &body.password, now)
    })
    .await??;
    Ok(signed_in(&state.config, &session))
}

async fn sign_in(
    State(state): State<Shared>,
    ClientAddr(address): ClientAddr,
    headers: axum::http::HeaderMap,
    ApiJson(body): ApiJson<Credentials>,
) -> Result<Response, ApiError> {
    let now = state.now_ms();
    let session = blocking(&state, move |s| {
        s.accounts.sign_in_with_code(
            &s.db(),
            &body.username,
            &body.password,
            body.code.as_deref(),
            &address,
            now,
        )
    })
    .await??;
    let agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    state
        .accounts
        .label_session(&state.db(), &session.token, agent)?;
    Ok(signed_in(&state.config, &session))
}

/// Always clears the cookie; ends the server-side session when there is one.
async fn sign_out(
    State(state): State<Shared>,
    headers: axum::http::HeaderMap,
) -> Result<Response, ApiError> {
    if let Some(token) = super::read_cookie(&headers, state.config.session_cookie_name()) {
        state.accounts.sign_out(&state.db(), &token)?;
    }
    Ok(signed_out(&state.config, StatusCode::NO_CONTENT))
}

#[derive(Deserialize)]
struct NewToken {
    name: String,
}

async fn list_tokens(
    State(state): State<Shared>,
    auth: AuthUser,
) -> Result<Json<serde_json::Value>, ApiError> {
    let tokens = state.db().api_tokens_of(&auth.user.id)?;
    Ok(Json(serde_json::json!({ "tokens": tokens })))
}

/// Make a token. The secret is in this answer and nowhere else, ever.
async fn create_token(
    State(state): State<Shared>,
    auth: AuthUser,
    ApiJson(body): ApiJson<NewToken>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let name = body.name.trim();
    if name.is_empty() {
        return Err(ApiError::bad_request("name_required"));
    }
    let now = state.now_ms();
    let db = state.db();
    if db.api_tokens_of(&auth.user.id)?.len() >= crate::accounts::MAX_API_TOKENS {
        return Err(ApiError::bad_request("too_many_tokens"));
    }
    let (id, secret) = state
        .accounts
        .create_api_token(&db, &auth.user, name, now)?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({ "id": id, "name": name, "token": secret, "created_at": now })),
    ))
}

async fn delete_token(
    State(state): State<Shared>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    if state.db().delete_api_token(&auth.user.id, &id)? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found())
    }
}

async fn list_sessions(
    State(state): State<Shared>,
    auth: AuthUser,
) -> Result<Json<serde_json::Value>, ApiError> {
    let current = crate::accounts::Accounts::session_id(&auth.token);
    let sessions: Vec<serde_json::Value> = state
        .db()
        .auth_sessions_of(&auth.user.id, state.now_ms())?
        .into_iter()
        .map(|s| {
            serde_json::json!({
                "id": s.id, "created_at": s.created_at, "last_seen": s.last_seen,
                "agent": s.agent, "current": s.id == current,
            })
        })
        .collect();
    Ok(Json(serde_json::json!({ "sessions": sessions })))
}

/// Sign one browser out. Signing out the one you are using is what "sign out" already does, so it is refused here.
async fn delete_session(
    State(state): State<Shared>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    if id == crate::accounts::Accounts::session_id(&auth.token) {
        return Err(ApiError::bad_request("current_session"));
    }
    if state.db().delete_auth_session_by_id(&auth.user.id, &id)? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found())
    }
}

/// What the server noted lately (failed hooks and webhooks, refused timer commands), oldest first.
async fn admin_log(_admin: AdminUser) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "lines": crate::serverlog::lines() }))
}

/// The configured hooks, for the admin to see (the file is the only place they are changed).
async fn list_hooks(State(state): State<Shared>, _admin: AdminUser) -> Json<serde_json::Value> {
    let hooks: Vec<serde_json::Value> = state
        .hooks
        .get()
        .map(|r| {
            r.hooks()
                .iter()
                .map(|h| serde_json::json!({"event": h.event, "command": h.command, "timeout_s": h.timeout_s, "enabled": h.enabled}))
                .collect()
        })
        .unwrap_or_default();
    // The secret of a webhook is never sent back, only whether there is one.
    let webhooks: Vec<serde_json::Value> = state
        .hooks
        .get()
        .map(|r| {
            r.webhooks()
                .iter()
                .map(|h| serde_json::json!({"event": h.event, "url": h.url, "signed": h.secret.is_some(), "enabled": h.enabled}))
                .collect()
        })
        .unwrap_or_default();
    Json(serde_json::json!({ "hooks": hooks, "webhooks": webhooks }))
}

/// Send one webhook now with a sample event and say what came back.
async fn test_webhook(
    State(state): State<Shared>,
    AdminUser(admin): AdminUser,
    Path(index): Path<usize>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let Some(runner) = state.hooks.get().cloned() else {
        return Err(ApiError::not_found());
    };
    let Some(hook) = runner.webhooks().get(index).cloned() else {
        return Err(ApiError::not_found());
    };
    let event = if hook.event == "*" {
        "focus_started"
    } else {
        hook.event.as_str()
    }
    .to_string();
    let timer = op_core::TimerState::idle();
    let body = crate::hooks::payload(&event, state.now_ms(), &admin.id, &admin.username, &timer)
        .to_string();
    let allow_private = runner.allow_private_webhooks();
    let sent = tokio::task::spawn_blocking({
        let (hook, event) = (hook.clone(), event.clone());
        move || crate::webhooks::deliver(&hook, &event, &body, allow_private)
    })
    .await
    .map_err(ApiError::internal)?;
    let (result, detail) = match sent {
        Ok(status) => ("delivered", serde_json::json!(status)),
        Err(crate::webhooks::Failure::Status(status)) => ("status", serde_json::json!(status)),
        Err(crate::webhooks::Failure::Network(why)) => ("failed", serde_json::json!(why)),
    };
    Ok(Json(
        serde_json::json!({ "event": event, "result": result, "detail": detail }),
    ))
}

/// Run one hook now with a sample event and say how it ended.
async fn test_hook(
    State(state): State<Shared>,
    AdminUser(admin): AdminUser,
    Path(index): Path<usize>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let Some(hook) = state
        .hooks
        .get()
        .and_then(|r| r.hooks().get(index))
        .cloned()
    else {
        return Err(ApiError::not_found());
    };
    let event = if hook.event == "*" {
        "focus_started"
    } else {
        hook.event.as_str()
    };
    let timer = op_core::TimerState::idle();
    let body = crate::hooks::payload(event, state.now_ms(), &admin.id, &admin.username, &timer)
        .to_string();
    let outcome = crate::hooks::run_hook(&hook, event, &body).await;
    let (result, detail) = match outcome {
        crate::hooks::Outcome::Exited(code) => ("exited", serde_json::json!(code)),
        crate::hooks::Outcome::TimedOut => ("timed_out", serde_json::Value::Null),
        crate::hooks::Outcome::Failed(why) => ("failed", serde_json::json!(why)),
    };
    Ok(Json(
        serde_json::json!({ "event": event, "result": result, "detail": detail }),
    ))
}

#[derive(Deserialize)]
struct PasswordOnly {
    password: String,
}

#[derive(Deserialize)]
struct CodeOnly {
    code: String,
}

async fn totp_status(
    State(state): State<Shared>,
    auth: AuthUser,
) -> Result<Json<serde_json::Value>, ApiError> {
    let (enabled, recovery_left) = state.accounts.totp_status(&state.db(), &auth.user)?;
    Ok(Json(
        serde_json::json!({ "enabled": enabled, "recovery_left": recovery_left }),
    ))
}

/// Start two-factor setup: the key to put into an authenticator app (and the address that a QR code would hold).
async fn totp_start(
    State(state): State<Shared>,
    auth: AuthUser,
    ApiJson(body): ApiJson<PasswordOnly>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let (secret, uri) = blocking(&state, move |s| {
        s.accounts.totp_begin(&s.db(), &auth.user, &body.password)
    })
    .await??;
    Ok(Json(serde_json::json!({ "secret": secret, "uri": uri })))
}

/// Finish setup with a code from the app. The recovery codes are in this answer and nowhere else.
async fn totp_enable(
    State(state): State<Shared>,
    auth: AuthUser,
    ApiJson(body): ApiJson<CodeOnly>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let codes = state
        .accounts
        .totp_enable(&state.db(), &auth.user, &body.code, state.now_ms())?;
    Ok(Json(serde_json::json!({ "recovery_codes": codes })))
}

async fn totp_disable(
    State(state): State<Shared>,
    auth: AuthUser,
    ApiJson(body): ApiJson<PasswordOnly>,
) -> Result<StatusCode, ApiError> {
    blocking(&state, move |s| {
        s.accounts.totp_disable(&s.db(), &auth.user, &body.password)
    })
    .await??;
    Ok(StatusCode::NO_CONTENT)
}

async fn me(auth: AuthUser) -> Json<UserJson> {
    Json(UserJson::from(&auth.user))
}

#[derive(Deserialize)]
struct ChangePassword {
    current: String,
    new: String,
}

async fn change_password(
    State(state): State<Shared>,
    auth: AuthUser,
    ApiJson(body): ApiJson<ChangePassword>,
) -> Result<StatusCode, ApiError> {
    blocking(&state, move |s| {
        s.accounts
            .change_password(&s.db(), &auth.user, &body.current, &body.new, &auth.token)
    })
    .await??;
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_me(
    State(state): State<Shared>,
    auth: AuthUser,
    ApiJson(body): ApiJson<PasswordOnly>,
) -> Result<Response, ApiError> {
    blocking(&state, move |s| {
        s.accounts
            .delete_account(&s.db(), &auth.user, &body.password)
    })
    .await??;
    Ok(signed_out(&state.config, StatusCode::NO_CONTENT))
}

/// Delete all of the signed-in user's recorded data (sessions, categories, settings, timer) but keep the account.
async fn wipe_me(
    State(state): State<Shared>,
    auth: AuthUser,
    ApiJson(body): ApiJson<PasswordOnly>,
) -> Result<StatusCode, ApiError> {
    let user = auth.user.id.clone();
    blocking(&state, move |s| {
        s.accounts.wipe_data(&s.db(), &auth.user, &body.password)
    })
    .await??;
    state.hub.publish(&user, crate::hub::CHANGED);
    Ok(StatusCode::NO_CONTENT)
}

// ---- admin ----

/// A consistent copy of the whole database file, for the admin. It holds every account's data and the password
/// hashes, so it is for the admin only and should be kept as safely as the server itself.
async fn admin_backup(
    State(state): State<Shared>,
    AdminUser(_): AdminUser,
) -> Result<Response, ApiError> {
    let now = state.now_ms();
    let bytes = blocking(&state, move |s| -> Result<Vec<u8>, ApiError> {
        let path = std::env::temp_dir().join(format!("op-backup-{}.db", uuid::Uuid::new_v4()));
        let made = s.db().backup_to(&path);
        let bytes = made
            .map_err(ApiError::from)
            .and_then(|()| std::fs::read(&path).map_err(ApiError::internal));
        let _ = std::fs::remove_file(&path);
        bytes
    })
    .await??;
    let day: String = op_core::ids::format_ts(now.div_euclid(1000))
        .chars()
        .take(8)
        .collect();
    let mut res = bytes.into_response();
    let headers = res.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/vnd.sqlite3"),
    );
    if let Ok(v) = HeaderValue::from_str(&format!(
        "attachment; filename=\"openpomodoro-backup-{day}.db\""
    )) {
        headers.insert(header::CONTENT_DISPOSITION, v);
    }
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(res)
}

async fn list_users(
    State(state): State<Shared>,
    AdminUser(admin): AdminUser,
) -> Result<Json<Vec<UserJson>>, ApiError> {
    let users = state.accounts.list_users(&state.db(), &admin)?;
    Ok(Json(users.iter().map(UserJson::from).collect()))
}

#[derive(Deserialize)]
struct NewUser {
    username: String,
    password: String,
    #[serde(default)]
    role: Option<String>,
}

async fn create_user(
    State(state): State<Shared>,
    AdminUser(admin): AdminUser,
    ApiJson(body): ApiJson<NewUser>,
) -> Result<(StatusCode, Json<UserJson>), ApiError> {
    let role = match body.role.as_deref() {
        None | Some("user") => Role::User,
        Some("admin") => Role::Admin,
        Some(_) => return Err(ApiError::bad_request("invalid_role")),
    };
    let now = state.now_ms();
    let user = blocking(&state, move |s| {
        s.accounts
            .admin_create_user(&s.db(), &admin, &body.username, &body.password, role, now)
    })
    .await??;
    Ok((StatusCode::CREATED, Json(UserJson::from(&user))))
}

async fn set_user_password(
    State(state): State<Shared>,
    AdminUser(admin): AdminUser,
    Path(id): Path<String>,
    ApiJson(body): ApiJson<PasswordOnly>,
) -> Result<StatusCode, ApiError> {
    blocking(&state, move |s| {
        s.accounts
            .admin_set_password(&s.db(), &admin, &id, &body.password)
    })
    .await??;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct Disabled {
    disabled: bool,
}

async fn set_user_disabled(
    State(state): State<Shared>,
    AdminUser(admin): AdminUser,
    Path(id): Path<String>,
    ApiJson(body): ApiJson<Disabled>,
) -> Result<StatusCode, ApiError> {
    state
        .accounts
        .admin_set_disabled(&state.db(), &admin, &id, body.disabled)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Switch two-step sign-in off for a person who has lost their app and recovery codes.
async fn reset_user_totp(
    State(state): State<Shared>,
    AdminUser(admin): AdminUser,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    state.accounts.admin_reset_totp(&state.db(), &admin, &id)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize, Deserialize)]
struct RegistrationBody {
    policy: String,
}

async fn get_registration(
    State(state): State<Shared>,
    AdminUser(_): AdminUser,
) -> Result<Json<RegistrationBody>, ApiError> {
    let policy = state.accounts.registration(&state.db())?;
    Ok(Json(RegistrationBody {
        policy: registration_name(policy).to_string(),
    }))
}

async fn put_registration(
    State(state): State<Shared>,
    AdminUser(admin): AdminUser,
    ApiJson(body): ApiJson<RegistrationBody>,
) -> Result<StatusCode, ApiError> {
    let policy = match body.policy.as_str() {
        "open" => Registration::Open,
        "closed" => Registration::Closed,
        _ => return Err(ApiError::bad_request("invalid_policy")),
    };
    state
        .accounts
        .set_registration(&state.db(), &admin, policy)?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::{Value, json};

    use crate::test_util::{Res, TestApp};

    const PW: &str = "correct horse battery";

    fn cookie_value(res: &Res) -> String {
        let set = res.header("set-cookie").expect("set-cookie");
        set.split(';')
            .next()
            .and_then(|kv| kv.split_once('='))
            .map(|(_, v)| v.to_string())
            .expect("cookie value")
    }

    /// Run first-run setup and return the admin's cookie.
    async fn setup_admin(app: &TestApp) -> String {
        let res = app
            .post(
                "/api/setup",
                Some(json!({"username": "admin", "password": PW})),
            )
            .send()
            .await;
        assert_eq!(res.status, StatusCode::OK, "{}", res.text);
        cookie_value(&res)
    }

    async fn create_user(app: &TestApp, admin_cookie: &str, name: &str) -> Value {
        let res = app
            .post(
                "/api/admin/users",
                Some(json!({"username": name, "password": PW})),
            )
            .cookie(admin_cookie)
            .send()
            .await;
        assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
        res.json()
    }

    async fn sign_in(app: &TestApp, name: &str, password: &str) -> Res {
        app.post(
            "/api/signin",
            Some(json!({"username": name, "password": password})),
        )
        .send()
        .await
    }

    #[tokio::test]
    async fn fresh_install_needs_setup_then_first_account_is_admin() {
        let app = TestApp::new();
        let status = app.get("/api/setup").send().await.json();
        assert_eq!(
            status,
            json!({"needs_setup": true, "registration": "closed"})
        );

        let cookie = setup_admin(&app).await;
        let me = app.get("/api/me").cookie(&cookie).send().await;
        assert_eq!(me.status, StatusCode::OK);
        assert_eq!(me.json()["username"], "admin");
        assert_eq!(me.json()["role"], "admin");

        assert_eq!(
            app.get("/api/setup").send().await.json()["needs_setup"],
            false
        );
        let again = app
            .post(
                "/api/setup",
                Some(json!({"username": "second", "password": PW})),
            )
            .send()
            .await;
        assert_eq!(again.status, StatusCode::CONFLICT);
        assert_eq!(again.json()["error"], "setup_done");
    }

    #[tokio::test]
    async fn the_session_cookie_is_httponly_samesite_and_plain_over_http() {
        let app = TestApp::new();
        let res = app
            .post(
                "/api/setup",
                Some(json!({"username": "admin", "password": PW})),
            )
            .send()
            .await;
        let set = res.header("set-cookie").expect("cookie");
        assert!(set.starts_with("op_session="), "{set}");
        for part in ["Path=/", "HttpOnly", "SameSite=Lax", "Max-Age=2592000"] {
            assert!(set.contains(part), "{set} lacks {part}");
        }
        assert!(!set.contains("Secure"));
    }

    #[tokio::test]
    async fn over_https_the_cookie_is_secure_and_host_prefixed() {
        let app = TestApp::with_public(Some("https://pomodoro.example.com"));
        let res = app
            .post(
                "/api/setup",
                Some(json!({"username": "admin", "password": PW})),
            )
            .host("pomodoro.example.com")
            .header("origin", "https://pomodoro.example.com")
            .send()
            .await;
        assert_eq!(res.status, StatusCode::OK, "{}", res.text);
        let set = res.header("set-cookie").expect("cookie");
        assert!(
            set.starts_with("__Host-op_session=") && set.contains("; Secure"),
            "{set}"
        );
    }

    #[tokio::test]
    async fn me_needs_a_valid_session() {
        let app = TestApp::new();
        setup_admin(&app).await;
        assert_eq!(
            app.get("/api/me").send().await.status,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            app.get("/api/me").cookie("garbage").send().await.status,
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn responses_never_contain_password_hashes() {
        let app = TestApp::new();
        let cookie = setup_admin(&app).await;
        create_user(&app, &cookie, "bob").await;
        for res in [
            app.get("/api/me").cookie(&cookie).send().await,
            app.get("/api/admin/users").cookie(&cookie).send().await,
            sign_in(&app, "admin", PW).await,
        ] {
            assert!(
                !res.text.contains("argon2") && !res.text.contains("password"),
                "{}",
                res.text
            );
        }
    }

    #[tokio::test]
    async fn sign_in_succeeds_and_failures_are_uniform() {
        let app = TestApp::new();
        let admin = setup_admin(&app).await;
        create_user(&app, &admin, "bob").await;

        assert_eq!(sign_in(&app, "bob", PW).await.status, StatusCode::OK);
        let wrong = sign_in(&app, "bob", "wrong password!").await;
        let unknown = sign_in(&app, "nobody", PW).await;
        assert_eq!(wrong.status, StatusCode::UNAUTHORIZED);
        assert_eq!(unknown.status, StatusCode::UNAUTHORIZED);
        assert_eq!(
            wrong.text, unknown.text,
            "no hint about which part was wrong"
        );
    }

    #[tokio::test]
    async fn repeated_failures_are_rate_limited_with_retry_after() {
        let app = TestApp::new();
        setup_admin(&app).await;
        for _ in 0..5 {
            assert_eq!(
                sign_in(&app, "admin", "wrong password!").await.status,
                StatusCode::UNAUTHORIZED
            );
        }
        let limited = sign_in(&app, "admin", PW).await;
        assert_eq!(limited.status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(limited.json()["error"], "rate_limited");
        assert!(
            limited
                .header("retry-after")
                .and_then(|v| v.parse::<u32>().ok())
                .is_some_and(|s| s > 0)
        );
        // After the lock passes the right password works again.
        app.clock.advance(60_000);
        assert_eq!(sign_in(&app, "admin", PW).await.status, StatusCode::OK);
    }

    #[tokio::test]
    async fn a_spoofed_forwarded_header_does_not_dodge_the_limit_without_a_trusted_proxy() {
        let app = TestApp::new();
        setup_admin(&app).await;
        for i in 0..5 {
            let res = app
                .post(
                    "/api/signin",
                    Some(json!({"username": "admin", "password": "wrong password!"})),
                )
                .header("x-forwarded-for", &format!("1.2.3.{i}"))
                .send()
                .await;
            assert_eq!(res.status, StatusCode::UNAUTHORIZED);
        }
        let res = app
            .post(
                "/api/signin",
                Some(json!({"username": "admin", "password": PW})),
            )
            .header("x-forwarded-for", "9.9.9.9")
            .send()
            .await;
        assert_eq!(res.status, StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn sign_out_ends_the_session_and_clears_the_cookie() {
        let app = TestApp::new();
        let cookie = setup_admin(&app).await;
        let res = app.post("/api/signout", None).cookie(&cookie).send().await;
        assert_eq!(res.status, StatusCode::NO_CONTENT);
        assert!(
            res.header("set-cookie")
                .is_some_and(|c| c.contains("Max-Age=0"))
        );
        assert_eq!(
            app.get("/api/me").cookie(&cookie).send().await.status,
            StatusCode::UNAUTHORIZED
        );
        // Signing out when already signed out is fine.
        assert_eq!(
            app.post("/api/signout", None).send().await.status,
            StatusCode::NO_CONTENT
        );
    }

    #[tokio::test]
    async fn registration_follows_the_policy() {
        let app = TestApp::new();
        let admin = setup_admin(&app).await;
        let body = json!({"username": "carol", "password": PW});

        let closed = app.post("/api/register", Some(body.clone())).send().await;
        assert_eq!(closed.status, StatusCode::FORBIDDEN);
        assert_eq!(closed.json()["error"], "registration_closed");

        let open = app
            .put("/api/admin/registration", Some(json!({"policy": "open"})))
            .cookie(&admin)
            .send()
            .await;
        assert_eq!(open.status, StatusCode::NO_CONTENT);
        assert_eq!(
            app.get("/api/setup").send().await.json()["registration"],
            "open"
        );

        let ok = app.post("/api/register", Some(body.clone())).send().await;
        assert_eq!(ok.status, StatusCode::OK);
        assert_eq!(ok.json()["role"], "user");
        let cookie = cookie_value(&ok);
        assert_eq!(
            app.get("/api/me").cookie(&cookie).send().await.json()["username"],
            "carol"
        );

        let taken = app
            .post(
                "/api/register",
                Some(json!({"username": "CAROL", "password": PW})),
            )
            .send()
            .await;
        assert_eq!(taken.status, StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn registration_before_setup_is_refused() {
        let app = TestApp::new();
        let res = app
            .post(
                "/api/register",
                Some(json!({"username": "early", "password": PW})),
            )
            .send()
            .await;
        assert_eq!(res.status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn bad_input_gets_clear_codes() {
        let app = TestApp::new();
        let res = app
            .post(
                "/api/setup",
                Some(json!({"username": "no", "password": PW})),
            )
            .send()
            .await;
        assert_eq!(
            (res.status, res.json()["error"].clone()),
            (StatusCode::BAD_REQUEST, json!("invalid_username"))
        );
        let res = app
            .post(
                "/api/setup",
                Some(json!({"username": "admin", "password": "short"})),
            )
            .send()
            .await;
        assert_eq!(
            (res.status, res.json()["error"].clone()),
            (StatusCode::BAD_REQUEST, json!("invalid_password"))
        );
        let res = app
            .post("/api/setup", Some(json!({"nope": 1})))
            .send()
            .await;
        assert_eq!(
            (res.status, res.json()["error"].clone()),
            (StatusCode::BAD_REQUEST, json!("invalid_json"))
        );
    }

    #[tokio::test]
    async fn changing_the_password_keeps_this_session_and_ends_the_others() {
        let app = TestApp::new();
        let admin = setup_admin(&app).await;
        let other = cookie_value(&sign_in(&app, "admin", PW).await);

        let wrong = app
            .post(
                "/api/me/password",
                Some(json!({"current": "not my password", "new": "another long password"})),
            )
            .cookie(&admin)
            .send()
            .await;
        assert_eq!(wrong.status, StatusCode::UNAUTHORIZED);

        let ok = app
            .post(
                "/api/me/password",
                Some(json!({"current": PW, "new": "another long password"})),
            )
            .cookie(&admin)
            .send()
            .await;
        assert_eq!(ok.status, StatusCode::NO_CONTENT);
        assert_eq!(
            app.get("/api/me").cookie(&admin).send().await.status,
            StatusCode::OK
        );
        assert_eq!(
            app.get("/api/me").cookie(&other).send().await.status,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            sign_in(&app, "admin", "another long password").await.status,
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn deleting_your_account_needs_the_password_and_signs_you_out() {
        let app = TestApp::new();
        let admin = setup_admin(&app).await;
        create_user(&app, &admin, "bob").await;
        let bob = cookie_value(&sign_in(&app, "bob", PW).await);

        let wrong = app
            .delete("/api/me", Some(json!({"password": "nope nope nope"})))
            .cookie(&bob)
            .send()
            .await;
        assert_eq!(wrong.status, StatusCode::UNAUTHORIZED);
        let ok = app
            .delete("/api/me", Some(json!({"password": PW})))
            .cookie(&bob)
            .send()
            .await;
        assert_eq!(ok.status, StatusCode::NO_CONTENT);
        assert_eq!(
            app.get("/api/me").cookie(&bob).send().await.status,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            sign_in(&app, "bob", PW).await.status,
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn admin_routes_are_for_admins_only() {
        let app = TestApp::new();
        let admin = setup_admin(&app).await;
        let user = create_user(&app, &admin, "bob").await;
        let bob = cookie_value(&sign_in(&app, "bob", PW).await);
        let id = user["id"].as_str().expect("id");

        // Signed out: 401. Signed in as a normal user: 403.
        assert_eq!(
            app.get("/api/admin/users").send().await.status,
            StatusCode::UNAUTHORIZED
        );
        for res in [
            app.get("/api/admin/users").cookie(&bob).send().await,
            app.post(
                "/api/admin/users",
                Some(json!({"username": "eve", "password": PW})),
            )
            .cookie(&bob)
            .send()
            .await,
            app.post(
                &format!("/api/admin/users/{id}/password"),
                Some(json!({"password": PW})),
            )
            .cookie(&bob)
            .send()
            .await,
            app.post(
                &format!("/api/admin/users/{id}/disabled"),
                Some(json!({"disabled": true})),
            )
            .cookie(&bob)
            .send()
            .await,
            app.get("/api/admin/registration").cookie(&bob).send().await,
            app.put("/api/admin/registration", Some(json!({"policy": "open"})))
                .cookie(&bob)
                .send()
                .await,
        ] {
            assert_eq!(res.status, StatusCode::FORBIDDEN, "{}", res.text);
        }

        let list = app
            .get("/api/admin/users")
            .cookie(&admin)
            .send()
            .await
            .json();
        assert_eq!(list.as_array().map(Vec::len), Some(2));
    }

    #[tokio::test]
    async fn admin_can_set_a_temporary_password_and_disable_accounts() {
        let app = TestApp::new();
        let admin = setup_admin(&app).await;
        let user = create_user(&app, &admin, "bob").await;
        let id = user["id"].as_str().expect("id").to_string();
        let bob = cookie_value(&sign_in(&app, "bob", PW).await);

        let reset = app
            .post(
                &format!("/api/admin/users/{id}/password"),
                Some(json!({"password": "temporary password 1"})),
            )
            .cookie(&admin)
            .send()
            .await;
        assert_eq!(reset.status, StatusCode::NO_CONTENT);
        assert_eq!(
            app.get("/api/me").cookie(&bob).send().await.status,
            StatusCode::UNAUTHORIZED,
            "signed out"
        );
        assert_eq!(
            sign_in(&app, "bob", "temporary password 1").await.status,
            StatusCode::OK
        );

        let off = app
            .post(
                &format!("/api/admin/users/{id}/disabled"),
                Some(json!({"disabled": true})),
            )
            .cookie(&admin)
            .send()
            .await;
        assert_eq!(off.status, StatusCode::NO_CONTENT);
        assert_eq!(
            sign_in(&app, "bob", "temporary password 1").await.status,
            StatusCode::UNAUTHORIZED
        );
        let me = app
            .get("/api/admin/users")
            .cookie(&admin)
            .send()
            .await
            .json();
        assert!(
            me.as_array()
                .expect("list")
                .iter()
                .any(|u| u["username"] == "bob" && u["disabled"] == true)
        );

        let missing = app
            .post(
                "/api/admin/users/nope/disabled",
                Some(json!({"disabled": true})),
            )
            .cookie(&admin)
            .send()
            .await;
        assert_eq!(missing.status, StatusCode::NOT_FOUND);
        let admin_id = app.get("/api/me").cookie(&admin).send().await.json()["id"]
            .as_str()
            .expect("id")
            .to_string();
        let selfie = app
            .post(
                &format!("/api/admin/users/{admin_id}/disabled"),
                Some(json!({"disabled": true})),
            )
            .cookie(&admin)
            .send()
            .await;
        assert_eq!(selfie.status, StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn invalid_role_and_policy_are_rejected() {
        let app = TestApp::new();
        let admin = setup_admin(&app).await;
        let role = app
            .post(
                "/api/admin/users",
                Some(json!({"username": "eve", "password": PW, "role": "root"})),
            )
            .cookie(&admin)
            .send()
            .await;
        assert_eq!(role.json()["error"], "invalid_role");
        let policy = app
            .put("/api/admin/registration", Some(json!({"policy": "maybe"})))
            .cookie(&admin)
            .send()
            .await;
        assert_eq!(policy.json()["error"], "invalid_policy");
    }

    #[tokio::test]
    async fn auth_posts_without_the_csrf_header_are_refused() {
        let app = TestApp::new();
        let res = app
            .post(
                "/api/setup",
                Some(json!({"username": "admin", "password": PW})),
            )
            .without_csrf()
            .send()
            .await;
        assert_eq!(res.status, StatusCode::FORBIDDEN);
        assert_eq!(
            app.get("/api/setup").send().await.json()["needs_setup"],
            true,
            "nothing was created"
        );
    }
}
