//! Encrypted local storage: opens a SQLCipher-encrypted SQLite database
//! whose encryption key lives in the OS credential store (Keychain /
//! Credential Manager / Secret Service), never on disk in plaintext.

use std::path::Path;

use livetap_shared::CapturedRequest;
use rand::RngCore;
use rusqlite::{params, Connection};

const KEYRING_SERVICE: &str = "com.livetap.app";
const KEYRING_USER: &str = "sqlcipher-db-key";
const DB_FILE_NAME: &str = "livetap.db";

/// Fetches the SQLCipher key from the OS keychain, generating and storing
/// a new random one on first run.
pub fn get_or_create_db_key() -> Result<String, keyring::Error> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)?;

    match entry.get_password() {
        Ok(key) => Ok(key),
        Err(keyring::Error::NoEntry) => {
            let mut raw = [0u8; 32];
            rand::thread_rng().fill_bytes(&mut raw);
            let key = raw.iter().map(|b| format!("{b:02x}")).collect::<String>();
            entry.set_password(&key)?;
            Ok(key)
        }
        Err(err) => Err(err),
    }
}

/// Opens (creating if needed) the SQLCipher-encrypted database in `app_data_dir`
/// and runs the initial schema migration.
pub fn open(app_data_dir: &Path, key: &str) -> rusqlite::Result<Connection> {
    let db_path = app_data_dir.join(DB_FILE_NAME);
    let conn = Connection::open(db_path)?;
    conn.pragma_update(None, "key", key)?;
    // Touches the DB so an incorrect key fails fast, instead of silently
    // opening what looks like an empty, unreadable database.
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))?;

    run_migrations(&conn)?;
    Ok(conn)
}

fn run_migrations(conn: &Connection) -> rusqlite::Result<()> {
    // Mirrors the shared `livetap_shared::CapturedRequest` JSON envelope.
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS captured_requests (
            id            TEXT PRIMARY KEY,
            session_id    TEXT NOT NULL,
            method        TEXT NOT NULL,
            path          TEXT NOT NULL,
            headers_json  TEXT NOT NULL,
            body          BLOB NOT NULL,
            remote_addr   TEXT,
            received_at   TEXT NOT NULL,
            status        INTEGER NOT NULL DEFAULT 200
        );
        CREATE INDEX IF NOT EXISTS idx_captured_requests_session
            ON captured_requests (session_id, received_at);",
    )
}

/// Persists a captured request using the shared JSON envelope shape.
pub fn insert_captured_request(conn: &Connection, req: &CapturedRequest) -> rusqlite::Result<()> {
    let headers_json = serde_json::to_string(&req.headers)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    conn.execute(
        "INSERT OR REPLACE INTO captured_requests
            (id, session_id, method, path, headers_json, body, remote_addr, received_at, status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            req.id,
            req.session_id,
            req.method,
            req.path,
            headers_json,
            req.body,
            req.remote_addr,
            req.received_at,
            req.status,
        ],
    )?;
    Ok(())
}

/// Loads every captured request for a session, oldest first.
pub fn list_captured_requests(
    conn: &Connection,
    session_id: &str,
) -> rusqlite::Result<Vec<CapturedRequest>> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, method, path, headers_json, body, remote_addr, received_at, status
         FROM captured_requests
         WHERE session_id = ?1
         ORDER BY received_at ASC",
    )?;
    let rows = stmt.query_map(params![session_id], |row| {
        let headers_json: String = row.get(4)?;
        let headers = serde_json::from_str(&headers_json).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(e))
        })?;
        Ok(CapturedRequest {
            id: row.get(0)?,
            session_id: row.get(1)?,
            method: row.get(2)?,
            path: row.get(3)?,
            headers,
            body: row.get(5)?,
            remote_addr: row.get(6)?,
            received_at: row.get(7)?,
            status: row.get(8)?,
        })
    })?;
    rows.collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use livetap_shared::HeaderPair;

    #[test]
    fn migrations_create_expected_table() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        let table_exists: bool = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'captured_requests'",
                [],
                |row| row.get::<_, i64>(0).map(|c| c == 1),
            )
            .unwrap();
        assert!(table_exists);
    }

    #[test]
    fn insert_and_list_round_trips_the_shared_envelope() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();

        let req = CapturedRequest {
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
        insert_captured_request(&conn, &req).unwrap();

        let loaded = list_captured_requests(&conn, "sess_1").unwrap();
        assert_eq!(loaded, vec![req]);
    }
}
