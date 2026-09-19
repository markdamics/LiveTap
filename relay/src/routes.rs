//! HTTP + WebSocket handlers for the capture loop:
//! `POST /session`, `ANY /hook/:session_id[/*rest]`, `GET /ws/:session_id`.

use std::{collections::HashMap, net::SocketAddr};

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        ConnectInfo, OriginalUri, Path, State,
    },
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use livetap_shared::{CapturedRequest, HeaderPair};
use serde::Serialize;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::AppState;

#[derive(Serialize)]
pub struct CreateSessionResponse {
    pub session_id: String,
    pub hook_url: String,
    pub ws_url: String,
    pub expires_in_secs: u64,
}

pub async fn create_session(State(state): State<AppState>) -> Json<CreateSessionResponse> {
    let session_id = state.store.create();
    Json(CreateSessionResponse {
        hook_url: format!("{}/hook/{session_id}", state.public_base_url),
        ws_url: format!("{}/ws/{session_id}", state.public_ws_base_url),
        expires_in_secs: state.store.ttl().as_secs(),
        session_id,
    })
}

/// Handles every method/path under `/hook/:session_id`, capturing the raw
/// request and publishing it to any connected WebSocket clients.
pub async fn capture_hook(
    State(state): State<AppState>,
    Path(params): Path<HashMap<String, String>>,
    method: Method,
    headers: HeaderMap,
    ConnectInfo(remote_addr): ConnectInfo<SocketAddr>,
    OriginalUri(uri): OriginalUri,
    body: axum::body::Bytes,
) -> Response {
    let Some(session_id) = params.get("session_id").cloned() else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    let Some(tx) = state.store.sender(&session_id) else {
        return (StatusCode::NOT_FOUND, "unknown or expired session").into_response();
    };

    let captured = CapturedRequest {
        id: Uuid::new_v4().to_string(),
        session_id,
        method: method.to_string(),
        path: uri.path().to_string(),
        headers: headers
            .iter()
            .map(|(name, value)| HeaderPair {
                name: name.to_string(),
                value: value.to_str().unwrap_or_default().to_string(),
            })
            .collect(),
        body: body.to_vec(),
        remote_addr: Some(remote_addr.ip().to_string()),
        received_at: Utc::now().to_rfc3339(),
        status: StatusCode::OK.as_u16(),
    };

    // No subscribers just means no client is connected right now; dropping
    // the capture here is expected until Phase 3 adds a short-lived buffer.
    let _ = tx.send(captured);

    StatusCode::OK.into_response()
}

pub async fn ws_upgrade(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    ws: WebSocketUpgrade,
) -> Response {
    let Some(tx) = state.store.sender(&session_id) else {
        return (StatusCode::NOT_FOUND, "unknown or expired session").into_response();
    };
    ws.on_upgrade(move |socket| forward_captures(socket, tx))
}

async fn forward_captures(mut socket: WebSocket, tx: broadcast::Sender<CapturedRequest>) {
    let mut rx = tx.subscribe();
    loop {
        tokio::select! {
            captured = rx.recv() => {
                match captured {
                    Ok(captured) => {
                        let Ok(json) = serde_json::to_string(&captured) else { continue };
                        if socket.send(Message::Text(json.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            incoming = socket.recv() => {
                match incoming {
                    None | Some(Ok(Message::Close(_))) => break,
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                }
            }
        }
    }
}

pub async fn health() -> &'static str {
    "ok"
}
