//! API errors: a status, a stable machine-readable code, and nothing else the client does not need.

use axum::{
    Json,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde_json::json;

use crate::accounts::AccountError;
use crate::db::DbError;

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub retry_after_ms: Option<i64>,
}

impl ApiError {
    pub fn new(status: StatusCode, code: &'static str) -> Self {
        Self {
            status,
            code,
            retry_after_ms: None,
        }
    }

    pub fn bad_request(code: &'static str) -> Self {
        Self::new(StatusCode::BAD_REQUEST, code)
    }

    pub fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "unauthorized")
    }

    pub fn forbidden() -> Self {
        Self::new(StatusCode::FORBIDDEN, "forbidden")
    }

    pub fn not_found() -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found")
    }

    /// Log the real cause on the server and tell the client nothing about it.
    pub fn internal(cause: impl std::fmt::Display) -> Self {
        eprintln!("internal error: {cause}");
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal")
    }
}

impl From<AccountError> for ApiError {
    fn from(e: AccountError) -> Self {
        use AccountError::*;
        match e {
            InvalidUsername => Self::bad_request("invalid_username"),
            InvalidPassword => Self::bad_request("invalid_password"),
            UsernameTaken => Self::new(StatusCode::CONFLICT, "username_taken"),
            InvalidCredentials => Self::new(StatusCode::UNAUTHORIZED, "invalid_credentials"),
            RateLimited { retry_after_ms } => Self {
                status: StatusCode::TOO_MANY_REQUESTS,
                code: "rate_limited",
                retry_after_ms: Some(retry_after_ms),
            },
            RegistrationClosed => Self::forbidden_with("registration_closed"),
            SetupAlreadyDone => Self::new(StatusCode::CONFLICT, "setup_done"),
            Forbidden => Self::forbidden(),
            NotFound => Self::not_found(),
            LastAdmin => Self::new(StatusCode::CONFLICT, "last_admin"),
            SelfAction => Self::new(StatusCode::CONFLICT, "self_action"),
            TotpRequired => Self::new(StatusCode::UNAUTHORIZED, "totp_required"),
            InvalidCode => Self::new(StatusCode::UNAUTHORIZED, "invalid_code"),
            TotpState => Self::new(StatusCode::CONFLICT, "totp_state"),
            Db(cause) => Self::internal(cause),
        }
    }
}

impl ApiError {
    fn forbidden_with(code: &'static str) -> Self {
        Self::new(StatusCode::FORBIDDEN, code)
    }
}

impl From<DbError> for ApiError {
    fn from(e: DbError) -> Self {
        Self::internal(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut body = json!({ "error": self.code });
        if let Some(ms) = self.retry_after_ms {
            body["retry_after_ms"] = json!(ms);
        }
        let mut res = (self.status, Json(body)).into_response();
        if let Some(ms) = self.retry_after_ms {
            let secs = (ms.max(0) + 999) / 1000;
            if let Ok(v) = HeaderValue::from_str(&secs.to_string()) {
                res.headers_mut().insert(header::RETRY_AFTER, v);
            }
        }
        res
    }
}
