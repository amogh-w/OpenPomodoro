//! The JSON API under `/api`.

use axum::Router;
use axum::extract::{FromRequest, FromRequestParts, Request};
use axum::http::request::Parts;
use serde::de::DeserializeOwned;

use crate::db::User;
use crate::error::ApiError;
use crate::security::{client_address, peer_ip};
use crate::state::Shared;

pub mod auth;
pub mod data;
pub mod sounds;
pub mod ws;

pub fn router() -> Router<Shared> {
    auth::router()
        .merge(data::router())
        .merge(sounds::router())
        .merge(ws::router())
}

/// JSON body that answers with the API's own error shape when it does not parse.
pub struct ApiJson<T>(pub T);

impl<T: DeserializeOwned> FromRequest<Shared> for ApiJson<T> {
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &Shared) -> Result<Self, Self::Rejection> {
        axum::Json::<T>::from_request(req, state)
            .await
            .map(|axum::Json(value)| ApiJson(value))
            .map_err(|rejection| {
                // A body over the limit is "too large", not "not JSON".
                if rejection.status() == axum::http::StatusCode::PAYLOAD_TOO_LARGE {
                    ApiError::new(
                        axum::http::StatusCode::PAYLOAD_TOO_LARGE,
                        "payload_too_large",
                    )
                } else {
                    ApiError::bad_request("invalid_json")
                }
            })
    }
}

/// The signed-in user behind the session cookie, and the cookie's value (needed to sign out or keep a session).
pub struct AuthUser {
    pub user: User,
    pub token: String,
}

pub fn read_cookie(headers: &axum::http::HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(axum::http::header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|line| line.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v.to_string())
}

impl FromRequestParts<Shared> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Shared,
    ) -> Result<Self, Self::Rejection> {
        let token = read_cookie(&parts.headers, state.config.session_cookie_name())
            .ok_or_else(ApiError::unauthorized)?;
        let user = state
            .accounts
            .user_for_session(&state.db(), &token, state.now_ms())?
            .ok_or_else(ApiError::unauthorized)?;
        Ok(AuthUser { user, token })
    }
}

/// An admin. Anyone else gets 403.
pub struct AdminUser(pub User);

impl FromRequestParts<Shared> for AdminUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Shared,
    ) -> Result<Self, Self::Rejection> {
        let AuthUser { user, .. } = AuthUser::from_request_parts(parts, state).await?;
        if user.role == crate::db::Role::Admin {
            Ok(AdminUser(user))
        } else {
            Err(ApiError::forbidden())
        }
    }
}

/// The client address for rate limiting (see [`client_address`]).
pub struct ClientAddr(pub String);

impl FromRequestParts<Shared> for ClientAddr {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Shared,
    ) -> Result<Self, Self::Rejection> {
        Ok(ClientAddr(client_address(
            &parts.headers,
            peer_ip(&parts.extensions),
            state.config.trust_proxy,
        )))
    }
}

/// Run blocking work (password hashing takes tens of milliseconds) off the async threads.
pub async fn blocking<T: Send + 'static>(
    state: &Shared,
    work: impl FnOnce(&crate::state::AppState) -> T + Send + 'static,
) -> Result<T, ApiError> {
    let state = state.clone();
    tokio::task::spawn_blocking(move || work(&state))
        .await
        .map_err(ApiError::internal)
}
