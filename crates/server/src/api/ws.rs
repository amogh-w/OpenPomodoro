//! `GET /api/ws`: a WebSocket that says "something changed" whenever the signed-in user's data does.

use std::time::Duration;

use axum::{
    Router,
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::Response,
    routing::get,
};
use serde_json::json;

use super::data::state_json;
use super::{AuthUser, TokenUser};
use crate::hub::CHANGED;
use crate::state::Shared;

/// How often an open socket re-checks that its session is still valid (sign-out, password change and
/// "disable account" all end sessions) and pings the client. Short in tests so they stay fast.
const CHECK_EVERY: Duration = if cfg!(test) {
    Duration::from_millis(150)
} else {
    Duration::from_secs(30)
};

pub fn router() -> Router<Shared> {
    Router::new()
        .route("/api/ws", get(upgrade))
        .route("/api/v1/stream", get(upgrade_stream))
}

/// The browser's `Origin` was already checked by the host/origin guard, and the cookie by [`AuthUser`].
async fn upgrade(State(state): State<Shared>, auth: AuthUser, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |socket| run(socket, state, auth))
}

/// `GET /api/v1/stream`: the same push channel for scripts and overlays, authenticated by a personal token. Instead
/// of "something changed" it sends the whole current state (`{"type":"state","state":{...}}`) on connect and after
/// every change, so a listener never needs a second request.
async fn upgrade_stream(
    State(state): State<Shared>,
    auth: TokenUser,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(move |socket| run_stream(socket, state, auth))
}

async fn state_message(state: &Shared, user: &str) -> Option<String> {
    let timer = crate::timer_service::load_state(&state.db(), user).ok()?;
    let body = state_json(state, user, &timer).ok()?;
    Some(json!({ "type": "state", "state": body }).to_string())
}

async fn run_stream(mut socket: WebSocket, state: Shared, auth: TokenUser) {
    let TokenUser(user, token) = auth;
    let Some(mut changes) = state.hub.subscribe(&user.id) else {
        let _ = socket
            .send(Message::Close(Some(axum::extract::ws::CloseFrame {
                code: 1013,
                reason: "busy".into(),
            })))
            .await;
        return;
    };
    let _slot = Slot {
        state: state.clone(),
        user: user.id.clone(),
    };

    let Some(first) = state_message(&state, &user.id).await else {
        return;
    };
    if socket.send(Message::Text(first.into())).await.is_err() {
        return;
    }

    let mut check = tokio::time::interval(CHECK_EVERY);
    check.tick().await;
    loop {
        tokio::select! {
            _ = changes.recv() => {
                // Whether one change or several were missed, the newest state is what matters.
                let Some(text) = state_message(&state, &user.id).await else { break };
                if socket.send(Message::Text(text.into())).await.is_err() {
                    break;
                }
            }
            incoming = socket.recv() => match incoming {
                None | Some(Err(_) | Ok(Message::Close(_))) => break,
                Some(Ok(_)) => {}
            },
            _ = check.tick() => {
                let valid = state.accounts.user_for_api_token(&state.db(), &token, state.now_ms());
                if !matches!(valid, Ok(Some(_))) {
                    let _ = socket.send(Message::Close(None)).await;
                    break;
                }
                if socket.send(Message::Ping(Vec::new().into())).await.is_err() {
                    break;
                }
            }
        }
    }
}

/// Releases the user's connection slot however the socket ends.
struct Slot {
    state: Shared,
    user: String,
}

impl Drop for Slot {
    fn drop(&mut self) {
        self.state.hub.release(&self.user);
    }
}

async fn run(mut socket: WebSocket, state: Shared, auth: AuthUser) {
    let user = auth.user.id;
    let Some(mut changes) = state.hub.subscribe(&user) else {
        // Too many sockets already (1013: try again later).
        let _ = socket
            .send(Message::Close(Some(axum::extract::ws::CloseFrame {
                code: 1013,
                reason: "busy".into(),
            })))
            .await;
        return;
    };
    let _slot = Slot {
        state: state.clone(),
        user: user.clone(),
    };

    let hello = json!({ "type": "hello", "server_time_ms": state.now_ms() }).to_string();
    if socket.send(Message::Text(hello.into())).await.is_err() {
        return;
    }

    let mut check = tokio::time::interval(CHECK_EVERY);
    check.tick().await; // the first tick is immediate
    loop {
        tokio::select! {
            change = changes.recv() => {
                // Lagging means we missed some messages; one "changed" covers all of them.
                let text = change.unwrap_or_else(|_| CHANGED.to_string());
                if socket.send(Message::Text(text.into())).await.is_err() {
                    break;
                }
            }
            incoming = socket.recv() => match incoming {
                None | Some(Err(_) | Ok(Message::Close(_))) => break,
                Some(Ok(_)) => {} // clients have nothing to say; pings are answered by the library
            },
            _ = check.tick() => {
                let valid = state.accounts.user_for_session(&state.db(), &auth.token, state.now_ms());
                if !matches!(valid, Ok(Some(_))) {
                    let _ = socket.send(Message::Close(None)).await;
                    break;
                }
                if socket.send(Message::Ping(Vec::new().into())).await.is_err() {
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use std::time::Duration;

    use futures_util::StreamExt;
    use serde_json::{Value, json};
    use tokio_tungstenite::tungstenite::{
        Error as WsError, Message as ClientMessage, client::IntoClientRequest,
    };

    use crate::test_util::{Res, TestApp};

    const PW: &str = "correct horse battery";
    const WAIT: Duration = Duration::from_secs(3);

    type Client = tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >;

    async fn serve(app: &TestApp) -> SocketAddr {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        let service =
            crate::app(app.state.clone()).into_make_service_with_connect_info::<SocketAddr>();
        tokio::spawn(async move {
            let _ = axum::serve(listener, service).await;
        });
        addr
    }

    fn cookie_of(res: &Res) -> String {
        let set = res.header("set-cookie").expect("cookie");
        set.split(';')
            .next()
            .and_then(|kv| kv.split_once('='))
            .map(|(_, v)| v.to_string())
            .expect("value")
    }

    async fn connect(
        addr: SocketAddr,
        cookie: Option<&str>,
        origin: Option<&str>,
    ) -> Result<Client, WsError> {
        let mut request = format!("ws://{addr}/api/ws")
            .into_client_request()
            .expect("request");
        if let Some(cookie) = cookie {
            request.headers_mut().insert(
                "cookie",
                format!("op_session={cookie}").parse().expect("cookie"),
            );
        }
        if let Some(origin) = origin {
            request
                .headers_mut()
                .insert("origin", origin.parse().expect("origin"));
        }
        tokio_tungstenite::connect_async(request)
            .await
            .map(|(socket, _)| socket)
    }

    /// The next text message, parsed; panics if none arrives within `WAIT` (pings do not extend the wait).
    async fn next_json(socket: &mut Client) -> Value {
        let found = tokio::time::timeout(WAIT, async {
            loop {
                match socket.next().await {
                    Some(Ok(ClientMessage::Text(text))) => {
                        return serde_json::from_str::<Value>(text.as_str()).ok();
                    }
                    Some(Ok(_)) => {}
                    other => panic!("socket ended while waiting for a message: {other:?}"),
                }
            }
        })
        .await;
        found
            .expect("no message arrived in time")
            .expect("message was not json")
    }

    /// True if no text message or close arrives for `ms` milliseconds (pings and pongs do not count).
    async fn silent_for(socket: &mut Client, ms: u64) -> bool {
        tokio::time::timeout(Duration::from_millis(ms), async {
            loop {
                match socket.next().await {
                    Some(Ok(ClientMessage::Ping(_) | ClientMessage::Pong(_))) => {}
                    _ => return,
                }
            }
        })
        .await
        .is_err()
    }

    async fn ended(socket: &mut Client) -> bool {
        tokio::time::timeout(WAIT, async {
            loop {
                match socket.next().await {
                    None | Some(Err(_) | Ok(ClientMessage::Close(_))) => return,
                    Some(Ok(_)) => {}
                }
            }
        })
        .await
        .is_ok()
    }

    async fn connect_stream(addr: SocketAddr, token: &str) -> Result<Client, WsError> {
        let mut request = format!("ws://{addr}/api/v1/stream")
            .into_client_request()
            .expect("request");
        request.headers_mut().insert(
            "authorization",
            format!("Bearer {token}").parse().expect("header"),
        );
        tokio_tungstenite::connect_async(request)
            .await
            .map(|(socket, _)| socket)
    }

    #[tokio::test]
    async fn the_token_stream_sends_the_whole_state_on_connect_and_after_each_change_and_ends_on_revoke()
     {
        let app = TestApp::new();
        let cookie = setup(&app).await;
        let made = app
            .post("/api/me/tokens", Some(json!({"name": "overlay"})))
            .cookie(&cookie)
            .send()
            .await
            .json();
        let token = made["token"].as_str().expect("token").to_string();
        let id = made["id"].as_str().expect("id").to_string();
        let addr = serve(&app).await;

        assert!(connect_stream(addr, "opt_wrong").await.is_err());
        assert!(
            connect(addr, Some(&cookie), None).await.is_ok(),
            "the cookie socket still works"
        );

        let mut socket = connect_stream(addr, &token).await.expect("connect");
        let first = next_json(&mut socket).await;
        assert_eq!(first["type"], "state");
        assert_eq!(first["state"]["timer"]["state"], "idle");

        let cmd = json!({"id": op_core::IdGen::new("t").next(app.state.now_ms()), "name": "init_focus", "intention": "watch me"});
        let res = app
            .post("/api/commands", Some(cmd))
            .cookie(&cookie)
            .send()
            .await;
        assert!(res.status.is_success(), "{}", res.text);
        let pushed = next_json(&mut socket).await;
        assert_eq!(pushed["state"]["timer"]["intention"], "watch me");

        let del = app
            .delete(&format!("/api/me/tokens/{id}"), None)
            .cookie(&cookie)
            .send()
            .await;
        assert!(del.status.is_success());
        assert!(
            ended(&mut socket).await,
            "the stream ends when its token is revoked"
        );
    }

    async fn setup(app: &TestApp) -> String {
        let res = app
            .post(
                "/api/setup",
                Some(json!({"username": "admin", "password": PW})),
            )
            .send()
            .await;
        cookie_of(&res)
    }

    async fn start_focus(app: &TestApp, cookie: &str, id: &str) {
        let res = app
            .post(
                "/api/commands",
                Some(json!({"id": id, "name": "init_focus"})),
            )
            .cookie(cookie)
            .send()
            .await;
        assert_eq!(res.status, axum::http::StatusCode::OK, "{}", res.text);
    }

    #[tokio::test]
    async fn the_upgrade_needs_a_session_and_a_matching_origin() {
        let app = TestApp::new();
        let cookie = setup(&app).await;
        let addr = serve(&app).await;

        let no_cookie = connect(addr, None, None).await.expect_err("refused");
        assert!(
            matches!(no_cookie, WsError::Http(r) if r.status() == 401),
            "no session"
        );
        let foreign = connect(addr, Some(&cookie), Some("http://evil.example"))
            .await
            .expect_err("refused");
        assert!(
            matches!(foreign, WsError::Http(r) if r.status() == 403),
            "foreign origin"
        );
        assert!(
            connect(addr, Some(&cookie), Some(&format!("http://{addr}")))
                .await
                .is_ok(),
            "same origin works"
        );
        assert!(
            connect(addr, Some(&cookie), None).await.is_ok(),
            "non-browser clients send no Origin"
        );
    }

    #[tokio::test]
    async fn a_connected_client_is_told_when_the_users_data_changes() {
        let app = TestApp::new();
        let cookie = setup(&app).await;
        let addr = serve(&app).await;
        let mut socket = connect(addr, Some(&cookie), None).await.expect("connect");

        let hello = next_json(&mut socket).await;
        assert_eq!(hello["type"], "hello");
        assert_eq!(hello["server_time_ms"], app.state.now_ms());

        let id = op_core::IdGen::new("browser").next(app.state.now_ms());
        start_focus(&app, &cookie, &id).await;
        assert_eq!(next_json(&mut socket).await, json!({"type": "changed"}));

        // A data edit counts too.
        let ev = json!({"events": [{"id": op_core::IdGen::new("browser2").next(app.state.now_ms()), "entity": "setting", "op": "add", "key": "strict", "payload": {"value": true}}]});
        app.post("/api/events", Some(ev))
            .cookie(&cookie)
            .send()
            .await;
        assert_eq!(next_json(&mut socket).await["type"], "changed");
    }

    #[tokio::test]
    async fn a_retried_command_or_a_rejected_one_does_not_wake_anyone() {
        let app = TestApp::new();
        let cookie = setup(&app).await;
        let addr = serve(&app).await;
        let mut socket = connect(addr, Some(&cookie), None).await.expect("connect");
        next_json(&mut socket).await; // hello

        let id = op_core::IdGen::new("browser").next(app.state.now_ms());
        start_focus(&app, &cookie, &id).await;
        next_json(&mut socket).await; // changed
        start_focus(&app, &cookie, &id).await; // duplicate
        let rejected = app.post("/api/commands", Some(json!({"id": op_core::IdGen::new("b2").next(app.state.now_ms()), "name": "done"}))).cookie(&cookie).send().await;
        assert_eq!(rejected.status, axum::http::StatusCode::CONFLICT);
        assert!(
            silent_for(&mut socket, 300).await,
            "nothing changed, so nothing was pushed"
        );
    }

    #[tokio::test]
    async fn one_users_changes_never_reach_another_users_socket() {
        let app = TestApp::new();
        let admin = setup(&app).await;
        app.post(
            "/api/admin/users",
            Some(json!({"username": "bob", "password": PW})),
        )
        .cookie(&admin)
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
        let addr = serve(&app).await;

        let mut alice_socket = connect(addr, Some(&admin), None).await.expect("alice");
        let mut bob_socket = connect(addr, Some(&bob), None).await.expect("bob");
        next_json(&mut alice_socket).await;
        next_json(&mut bob_socket).await;

        start_focus(
            &app,
            &bob,
            &op_core::IdGen::new("bobs-browser").next(app.state.now_ms()),
        )
        .await;
        assert_eq!(next_json(&mut bob_socket).await["type"], "changed");
        assert!(
            silent_for(&mut alice_socket, 400).await,
            "Alice must not hear about Bob's timer"
        );
    }

    #[tokio::test]
    async fn signing_out_closes_the_socket() {
        let app = TestApp::new();
        let cookie = setup(&app).await;
        let addr = serve(&app).await;
        let mut socket = connect(addr, Some(&cookie), None).await.expect("connect");
        next_json(&mut socket).await;

        app.post("/api/signout", None).cookie(&cookie).send().await;
        assert!(
            ended(&mut socket).await,
            "the server closes sockets whose session ended"
        );
        assert_eq!(
            app.state.hub.connections(&user_id(&app).await),
            0,
            "and frees the slot"
        );
    }

    async fn user_id(app: &TestApp) -> String {
        app.state.db().list_users().expect("users").remove(0).id
    }

    #[tokio::test]
    async fn a_user_can_hold_only_so_many_sockets() {
        let app = TestApp::new();
        let cookie = setup(&app).await;
        let addr = serve(&app).await;
        let mut held = Vec::new();
        for _ in 0..crate::hub::MAX_CONNECTIONS_PER_USER {
            let mut s = connect(addr, Some(&cookie), None).await.expect("connect");
            assert_eq!(next_json(&mut s).await["type"], "hello");
            held.push(s);
        }
        let mut extra = connect(addr, Some(&cookie), None)
            .await
            .expect("upgrade still succeeds");
        assert!(
            ended(&mut extra).await,
            "the extra socket is closed right away"
        );

        // Freeing one makes room again.
        held.pop();
        tokio::time::sleep(Duration::from_millis(300)).await;
        let mut again = connect(addr, Some(&cookie), None).await.expect("connect");
        assert_eq!(next_json(&mut again).await["type"], "hello");
    }
}
