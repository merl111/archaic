//! Browser reauthorization keeps the existing crypto store and validates the renewed identity.
use crate::{Event, EventKind, connection::Session};
use axum::{
    Router,
    extract::{RawQuery, State},
    http::{HeaderMap, StatusCode},
    routing::get,
};
use std::sync::Arc;
use tokio::sync::mpsc;

pub(crate) async fn browser(
    active: &Session,
    oauth: bool,
    events: mpsc::Sender<Event>,
) -> Result<(), &'static str> {
    if !active.revoked || !active.soft_logout {
        return Err("reauth-unavailable");
    }
    let store = active.token_store.as_ref().ok_or("reauth-unavailable")?;
    let current = crate::oauth::session(&active.client)?;
    if oauth != active.client.oauth().client_id().is_some() {
        return Err("reauth-unavailable");
    }
    active.client.send_queue().set_enabled(false).await;
    // SDK OAuth exchange updates in-memory tokens before whoami validates identity. Never
    // persist these through an incidental refresh callback until the validation succeeds.
    store.suspend();
    let renewed = if oauth {
        crate::oauth::login(&active.client, active.epoch, events).await?;
        let who = active.client.whoami().await.map_err(|_| "reauth-failed")?;
        if who.user_id != current.meta.user_id
            || who.device_id.as_ref() != Some(&current.meta.device_id)
        {
            return Err("session-key-mismatch");
        }
        crate::oauth::session(&active.client)?
    } else {
        let token = sso_token(&active.client, active.epoch, events).await?;
        use matrix_sdk::ruma::api::client::session::login::v3::{LoginInfo, Request, Token};
        let mut request = Request::new(LoginInfo::Token(Token::new(token)));
        request.device_id = Some(current.meta.device_id.clone());
        request.refresh_token = true;
        let response = active
            .client
            .send(request)
            .await
            .map_err(|_| "reauth-failed")?;
        (&response).into()
    };
    if renewed.meta != current.meta {
        return Err("session-key-mismatch");
    }
    store.replace_tokens(renewed)
}
struct Callback {
    host: String,
    tx: mpsc::Sender<String>,
}
struct Server(tokio::task::JoinHandle<()>);
impl Drop for Server {
    fn drop(&mut self) {
        self.0.abort();
    }
}
async fn callback(
    State(s): State<Arc<Callback>>,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
) -> (StatusCode, &'static str) {
    let query = query.unwrap_or_default();
    if headers.get("host").and_then(|h| h.to_str().ok()) != Some(&s.host) || query.len() > 16384 {
        return (StatusCode::BAD_REQUEST, "Invalid callback");
    }
    let tokens = url::form_urlencoded::parse(query.as_bytes())
        .filter(|(k, _)| k == "loginToken")
        .map(|(_, v)| v.into_owned())
        .collect::<Vec<_>>();
    if tokens.len() != 1 || tokens[0].is_empty() {
        return (StatusCode::BAD_REQUEST, "Invalid callback");
    }
    if s.tx.try_send(tokens[0].clone()).is_err() {
        return (StatusCode::CONFLICT, "Callback already received");
    }
    (StatusCode::OK, "Return to Archaic to finish signing in.")
}
async fn sso_token(
    client: &matrix_sdk::Client,
    epoch: u64,
    events: mpsc::Sender<Event>,
) -> Result<String, &'static str> {
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .map_err(|_| "sso-unavailable")?;
    let host = listener
        .local_addr()
        .map_err(|_| "sso-unavailable")?
        .to_string();
    let mut secret = [0u8; 32];
    getrandom::fill(&mut secret).map_err(|_| "sso-unavailable")?;
    let path = format!(
        "/callback/{}",
        secret
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    let redirect = format!("http://{host}{path}");
    let url = client
        .matrix_auth()
        .get_sso_login_url(&redirect, None)
        .await
        .map_err(|_| "sso-unavailable")?;
    let (tx, mut rx) = mpsc::channel(1);
    let app = Router::new()
        .route(&path, get(callback))
        .with_state(Arc::new(Callback { host, tx }));
    let _server = Server(tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    }));
    events
        .send(Event {
            epoch,
            kind: EventKind::BrowserUrl(url),
        })
        .await
        .map_err(|_| "sso-cancelled")?;
    rx.recv().await.ok_or("sso-cancelled")
}
