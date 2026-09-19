//! Shared `serde` envelope types, used by `livetap-core`, the relay, and
//! (via `livetap-core`'s `uniffi` bindings) the mobile UIs. The relay
//! produces these over the WebSocket; `livetap-core` persists and exposes
//! them to whichever UI is calling in.

use serde::{Deserialize, Serialize};

#[cfg(feature = "uniffi")]
uniffi::setup_scaffolding!();

/// A single HTTP header. A named struct rather than a `(String, String)`
/// tuple so it can also be exposed as a `uniffi` record — tuples aren't
/// representable across the FFI boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct HeaderPair {
    pub name: String,
    pub value: String,
}

/// A single captured webhook request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct CapturedRequest {
    /// Unique ID for this capture (e.g. a UUID), assigned by the relay.
    pub id: String,
    /// The session/URL this request was captured against.
    pub session_id: String,
    /// HTTP method, e.g. "POST".
    pub method: String,
    /// Request path, including any suffix after the session ID.
    pub path: String,
    /// Raw request headers, in receipt order.
    pub headers: Vec<HeaderPair>,
    /// Raw request body, base64-encoded when this envelope crosses JSON
    /// transport (the relay's WebSocket wire format) so arbitrary
    /// (including binary/non-UTF-8) payloads survive the round trip. Over
    /// the `uniffi` FFI boundary this is passed as a plain byte buffer.
    #[serde(with = "body_base64")]
    pub body: Vec<u8>,
    /// Sender's remote address, if known.
    pub remote_addr: Option<String>,
    /// RFC 3339 timestamp of when the relay received the request.
    pub received_at: String,
    /// HTTP status the relay acknowledged the capture with. Always 200
    /// today; will vary once the mock-response rule engine (Phase 5) can
    /// answer with a configured status instead of a flat ack.
    pub status: u16,
}

mod body_base64 {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
        let encoded = String::deserialize(deserializer)?;
        STANDARD.decode(encoded).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_json() {
        let original = CapturedRequest {
            id: "req_1".into(),
            session_id: "sess_1".into(),
            method: "POST".into(),
            path: "/hook/sess_1".into(),
            headers: vec![HeaderPair {
                name: "content-type".into(),
                value: "application/json".into(),
            }],
            body: b"{\"hello\":\"world\"}".to_vec(),
            remote_addr: Some("203.0.113.1".into()),
            received_at: "2026-09-18T12:00:00Z".into(),
            status: 200,
        };

        let json = serde_json::to_string(&original).unwrap();
        let decoded: CapturedRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(original, decoded);
    }
}
