//! WebSocket + HTTP client for talking to a self-hosted LiveTap relay
//! (see `relay/`). Plain Rust, not exposed over the `uniffi` boundary yet —
//! streaming a `Stream` across FFI needs a callback-interface design of its
//! own, deferred until mobile UI work (Phase 7) actually needs it. Desktop
//! consumes this module directly.

use futures_util::{Stream, StreamExt};
use livetap_shared::CapturedRequest;
use serde::Deserialize;
use tokio_tungstenite::tungstenite::Message as WsMessage;

/// How long to wait before retrying a dropped or failed connection.
const RECONNECT_DELAY: std::time::Duration = std::time::Duration::from_secs(2);

#[derive(Debug, Clone, Deserialize)]
pub struct RelaySession {
    pub session_id: String,
    pub hook_url: String,
    pub ws_url: String,
    pub expires_in_secs: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum RelayError {
    #[error("relay request failed: {0}")]
    Request(String),
}

/// Mints a new capture session on the relay at `base_url` (e.g.
/// `http://127.0.0.1:8787`).
pub async fn create_session(base_url: &str) -> Result<RelaySession, RelayError> {
    let url = format!("{}/session", base_url.trim_end_matches('/'));
    let response = reqwest::Client::new()
        .post(url)
        .send()
        .await
        .map_err(|err| RelayError::Request(err.to_string()))?
        .error_for_status()
        .map_err(|err| RelayError::Request(err.to_string()))?;
    response
        .json::<RelaySession>()
        .await
        .map_err(|err| RelayError::Request(err.to_string()))
}

/// A live update from the relay's WebSocket feed.
#[derive(Debug, Clone)]
pub enum RelayEvent {
    Connected,
    Disconnected,
    Captured(Box<CapturedRequest>),
}

/// Connects to `ws_url` and yields [`RelayEvent`]s for as long as the
/// stream is polled, transparently reconnecting (after [`RECONNECT_DELAY`])
/// on any drop or failure. Takes `&String` (rather than `&str`) so it can be
/// used directly as the `fn(&D) -> S` builder for `iced::Subscription::run_with`.
pub fn connect(ws_url: &String) -> impl Stream<Item = RelayEvent> + Send + 'static {
    let ws_url = ws_url.clone();
    async_stream::stream! {
        loop {
            match tokio_tungstenite::connect_async(&ws_url).await {
                Ok((stream, _response)) => {
                    yield RelayEvent::Connected;
                    let (_write, mut read) = stream.split();
                    loop {
                        match read.next().await {
                            Some(Ok(WsMessage::Text(text))) => {
                                if let Ok(captured) =
                                    serde_json::from_str::<CapturedRequest>(&text)
                                {
                                    yield RelayEvent::Captured(Box::new(captured));
                                }
                            }
                            Some(Ok(WsMessage::Close(_))) | None => {
                                yield RelayEvent::Disconnected;
                                break;
                            }
                            Some(Err(_)) => {
                                yield RelayEvent::Disconnected;
                                break;
                            }
                            _ => {}
                        }
                    }
                }
                Err(_) => {
                    yield RelayEvent::Disconnected;
                }
            }
            tokio::time::sleep(RECONNECT_DELAY).await;
        }
    }
}
