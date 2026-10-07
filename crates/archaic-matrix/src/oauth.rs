//! SDK-owned OAuth state/PKCE and token exchange. Only the loopback transport is local.
use crate::{Event, EventKind};
use axum::{
    Router,
    extract::{RawQuery, State},
    http::{HeaderMap, StatusCode},
    routing::get,
};
use matrix_sdk::{Client, authentication::matrix::MatrixSession, ruma::serde::Raw};
use std::sync::Arc;
use tokio::{net::TcpListener, sync::mpsc};

pub(crate) fn session(client: &Client) -> Result<MatrixSession, &'static str> {
    Ok(MatrixSession {
        meta: client.session_meta().cloned().ok_or("session-invalid")?,
        tokens: client.session_tokens().ok_or("session-invalid")?,
    })
}
struct Callback {
    state: String,
    host: String,
    tx: mpsc::Sender<String>,
}
struct Listener(tokio::task::JoinHandle<()>);
impl Drop for Listener {
    fn drop(&mut self) {
        self.0.abort();
    }
}
async fn callback(
    State(state): State<Arc<Callback>>,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
) -> (StatusCode, &'static str) {
    let query = query.unwrap_or_default();
    if headers.get("host").and_then(|h| h.to_str().ok()) != Some(state.host.as_str())
        || !valid_callback(&query, &state.state)
    {
        return (StatusCode::BAD_REQUEST, "Invalid authorization callback.");
    }
    match state.tx.try_send(query) {
        Ok(()) => (
            StatusCode::OK,
            "Return to Archaic to finish signing in. You can close this tab.",
        ),
        Err(_) => (
            StatusCode::CONFLICT,
            "This authorization callback is no longer active.",
        ),
    }
}
fn valid_callback(query: &str, expected: &str) -> bool {
    if query.len() > 16_384 {
        return false;
    }
    let values = url::form_urlencoded::parse(query.as_bytes()).collect::<Vec<_>>();
    let states = values
        .iter()
        .filter(|(k, _)| k == "state")
        .collect::<Vec<_>>();
    let codes = values
        .iter()
        .filter(|(k, _)| k == "code" || k == "error")
        .collect::<Vec<_>>();
    states.len() == 1 && states[0].1 == expected && codes.len() == 1 && !codes[0].1.is_empty()
}
pub(crate) async fn login(
    client: &Client,
    epoch: u64,
    events: mpsc::Sender<Event>,
) -> Result<(), &'static str> {
    let port = std::env::var("ARCHAIC_OAUTH_CALLBACK_PORT")
        .ok()
        .map(|v| v.parse::<u16>().map_err(|_| "oauth-unavailable"))
        .transpose()?
        .unwrap_or(0);
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))
        .await
        .map_err(|_| "sso-unavailable")?;
    let host = listener
        .local_addr()
        .map_err(|_| "sso-unavailable")?
        .to_string();
    let redirect =
        url::Url::parse(&format!("http://{host}/callback")).map_err(|_| "sso-unavailable")?;
    let oauth = client.oauth();
    let metadata = registration_metadata(client, &redirect).await?;
    let data =
        oauth
            .login(
                redirect.clone(),
                client.device_id().map(ToOwned::to_owned),
                client.oauth().client_id().is_none().then(|| {
                    matrix_sdk::authentication::oauth::ClientRegistrationData::new(metadata)
                }),
                None,
            )
            .build()
            .await
            .map_err(|_| "oauth-unavailable")?;
    let (tx, mut rx) = mpsc::channel(1);
    let state = Arc::new(Callback {
        state: data.state.secret().clone(),
        host,
        tx,
    });
    let app = Router::new()
        .route("/callback", get(callback))
        .with_state(state);
    let _server = Listener(tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    }));
    events
        .send(Event {
            epoch,
            kind: EventKind::BrowserUrl(data.url.to_string()),
        })
        .await
        .map_err(|_| "sso-cancelled")?;
    let query = rx.recv().await.ok_or("sso-cancelled")?;
    let mut response = redirect;
    response.set_query(Some(&query));
    oauth
        .finish_login(response.into())
        .await
        .map_err(|_| "oauth-callback-failed")
}
async fn registration_metadata(
    client: &Client,
    redirect: &url::Url,
) -> Result<Raw<matrix_sdk::authentication::oauth::registration::ClientMetadata>, &'static str> {
    let oauth = client.oauth();
    if oauth.client_id().is_none()
        && let Ok(id) = std::env::var("ARCHAIC_OAUTH_CLIENT_ID")
    {
        let expected = std::env::var("ARCHAIC_OAUTH_ISSUER").map_err(|_| "oauth-unavailable")?;
        let expected = url::Url::parse(&expected).map_err(|_| "oauth-unavailable")?;
        let metadata = oauth
            .server_metadata()
            .await
            .map_err(|_| "oauth-unavailable")?;
        if metadata.issuer != expected || id.is_empty() || id.len() > 4096 {
            return Err("oauth-unavailable");
        }
        oauth.restore_registered_client(matrix_sdk::authentication::oauth::ClientId::new(id));
    }
    let mut metadata = serde_json::json!({
        "application_type":"native", "client_name":"Archaic desktop",
        "redirect_uris":[redirect.as_str()], "grant_types":["authorization_code","refresh_token"],
        "response_types":["code"], "token_endpoint_auth_method":"none"
    });
    if let Ok(uri) = std::env::var("ARCHAIC_OAUTH_CLIENT_URI") {
        let uri = url::Url::parse(&uri).map_err(|_| "oauth-unavailable")?;
        if uri.scheme() != "https"
            || !uri.username().is_empty()
            || uri.password().is_some()
            || uri.fragment().is_some()
        {
            return Err("oauth-unavailable");
        }
        metadata["client_uri"] = serde_json::json!(uri);
    }
    let metadata = Raw::from_json(
        serde_json::value::to_raw_value(&metadata).map_err(|_| "oauth-unavailable")?,
    );
    Ok(metadata)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn callback_requires_single_state_and_one_result() {
        assert!(valid_callback("code=ok&state=expected", "expected"));
        assert!(valid_callback(
            "error=access_denied&state=expected",
            "expected"
        ));
        for q in [
            "code=ok",
            "code=ok&state=wrong",
            "code=ok&state=expected&state=expected",
            "code=x&error=x&state=expected",
            "code=&state=expected",
        ] {
            assert!(!valid_callback(q, "expected"));
        }
        assert!(!valid_callback(
            &format!("code={}&state=expected", "x".repeat(16_384)),
            "expected"
        ));
    }
}
