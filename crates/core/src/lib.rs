//! `livetap-core` — the shared Rust core used directly by the desktop
//! (iced) UI and, via the `uniffi`-generated bindings, by the Android and
//! iOS UIs. Holds the SQLCipher-encrypted local database and the
//! keychain-backed encryption key; later phases add the WebSocket relay
//! client and GitHub sync here too.

mod db;
pub mod relay_client;

use std::sync::Mutex;

use livetap_shared::CapturedRequest;
use rusqlite::Connection;

uniffi::setup_scaffolding!();

#[derive(Debug, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum CoreError {
    #[error("database error: {0}")]
    Database(String),
    #[error("keychain error: {0}")]
    Keychain(String),
    #[error("lock error: {0}")]
    Lock(String),
}

impl From<rusqlite::Error> for CoreError {
    fn from(err: rusqlite::Error) -> Self {
        CoreError::Database(err.to_string())
    }
}

impl From<keyring::Error> for CoreError {
    fn from(err: keyring::Error) -> Self {
        CoreError::Keychain(err.to_string())
    }
}

impl From<std::io::Error> for CoreError {
    fn from(err: std::io::Error) -> Self {
        CoreError::Database(err.to_string())
    }
}

/// The shared core's single entry point: an encrypted local store, opened
/// once per app launch and handed out as a `uniffi` object so mobile UIs
/// can call straight into it.
#[derive(uniffi::Object)]
pub struct LiveTapCore {
    db: Mutex<Connection>,
}

#[uniffi::export]
impl LiveTapCore {
    /// Opens (creating on first run) the encrypted database under
    /// `app_data_dir`, generating and storing its key in the OS keychain
    /// if one doesn't already exist there.
    #[uniffi::constructor]
    pub fn open(app_data_dir: String) -> Result<Self, CoreError> {
        let dir = std::path::Path::new(&app_data_dir);
        std::fs::create_dir_all(dir)?;
        let key = db::get_or_create_db_key()?;
        let conn = db::open(dir, &key)?;
        Ok(LiveTapCore {
            db: Mutex::new(conn),
        })
    }

    pub fn insert_captured_request(&self, req: CapturedRequest) -> Result<(), CoreError> {
        let conn = self
            .db
            .lock()
            .map_err(|e| CoreError::Lock(e.to_string()))?;
        db::insert_captured_request(&conn, &req)?;
        Ok(())
    }

    pub fn list_captured_requests(&self, session_id: String) -> Result<Vec<CapturedRequest>, CoreError> {
        let conn = self
            .db
            .lock()
            .map_err(|e| CoreError::Lock(e.to_string()))?;
        Ok(db::list_captured_requests(&conn, &session_id)?)
    }
}
