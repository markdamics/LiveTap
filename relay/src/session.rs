//! In-memory session registry. Each session owns a broadcast channel that
//! `/hook/:session_id` publishes captured requests onto and `/ws/:session_id`
//! subscribes to for live forwarding. No persistence here — captured
//! payloads are relayed and dropped, never stored durably by design.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use dashmap::DashMap;
use livetap_shared::CapturedRequest;
use tokio::sync::broadcast;

/// How many not-yet-delivered captures a lagging subscriber can fall behind
/// by before older ones are dropped from its view. Live forwarding isn't
/// meant to be a durable buffer (that's Phase 3's job).
const BROADCAST_CAPACITY: usize = 256;

struct Session {
    tx: broadcast::Sender<CapturedRequest>,
    expires_at: Instant,
}

#[derive(Clone)]
pub struct SessionStore {
    sessions: Arc<DashMap<String, Session>>,
    ttl: Duration,
}

impl SessionStore {
    pub fn new(ttl: Duration) -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            ttl,
        }
    }

    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    /// Mints a new session and returns its ID.
    pub fn create(&self) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let (tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
        self.sessions.insert(
            id.clone(),
            Session {
                tx,
                expires_at: Instant::now() + self.ttl,
            },
        );
        id
    }

    /// Returns a sender clone for the session, or `None` if it doesn't
    /// exist or has expired.
    pub fn sender(&self, session_id: &str) -> Option<broadcast::Sender<CapturedRequest>> {
        let session = self.sessions.get(session_id)?;
        if session.expires_at <= Instant::now() {
            return None;
        }
        Some(session.tx.clone())
    }

    /// Drops every session whose TTL has elapsed. Run periodically from a
    /// background task.
    pub fn sweep_expired(&self) {
        let now = Instant::now();
        self.sessions.retain(|_, session| session.expires_at > now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_session_has_no_sender() {
        let store = SessionStore::new(Duration::from_secs(60));
        assert!(store.sender("does-not-exist").is_none());
    }

    #[test]
    fn created_session_has_a_sender() {
        let store = SessionStore::new(Duration::from_secs(60));
        let id = store.create();
        assert!(store.sender(&id).is_some());
    }

    #[test]
    fn sweep_removes_expired_sessions() {
        let store = SessionStore::new(Duration::from_millis(1));
        let id = store.create();
        std::thread::sleep(Duration::from_millis(5));
        store.sweep_expired();
        assert!(store.sender(&id).is_none());
    }

    #[test]
    fn sweep_keeps_live_sessions() {
        let store = SessionStore::new(Duration::from_secs(60));
        let id = store.create();
        store.sweep_expired();
        assert!(store.sender(&id).is_some());
    }
}
