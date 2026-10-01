//! Persistencia SQLite das linhas e dos aparelhos; unico modulo que conhece rusqlite.

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use rusqlite::Connection;

pub mod rows;
pub mod schema;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("mutex envenenado")]
    Poisoned,
}

/// `Connection` nao e `Sync`; o Mutex serializa o acesso. Para um hub domestico, uma
/// conexao e um escritor por vez e mais que suficiente, e evita pool.
pub struct Store {
    conn: Mutex<Connection>,
    epoch: String,
}

impl Store {
    pub fn open(path: &Path) -> Result<Store, StoreError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Store::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Store, StoreError> {
        Store::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Store, StoreError> {
        schema::configure(&conn)?;
        schema::migrate(&conn)?;
        let epoch = schema::ensure_epoch(&conn)?;
        Ok(Store {
            conn: Mutex::new(conn),
            epoch,
        })
    }

    pub fn epoch(&self) -> &str {
        &self.epoch
    }

    pub fn max_seq(&self) -> Result<i64, StoreError> {
        let conn = self.lock()?;
        Ok(conn.query_row("SELECT COALESCE(MAX(seq), 0) FROM rows", [], |r| r.get(0))?)
    }

    pub(crate) fn lock(&self) -> Result<MutexGuard<'_, Connection>, StoreError> {
        self.conn.lock().map_err(|_| StoreError::Poisoned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user_version(store: &Store) -> i32 {
        store
            .lock()
            .unwrap()
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn open_cria_banco_com_schema_e_epoch() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hub.sqlite");
        let store = Store::open(&path).unwrap();
        assert!(path.exists());
        assert_eq!(user_version(&store), schema::SCHEMA_VERSION);
        assert_eq!(store.epoch().len(), 26);
        assert!(ulid::Ulid::from_string(store.epoch()).is_ok());
        let mode: String = store
            .lock()
            .unwrap()
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mode.to_lowercase(), "wal");
    }

    #[test]
    fn reabrir_preserva_o_epoch() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hub.sqlite");
        let first = Store::open(&path).unwrap().epoch().to_string();
        let second = Store::open(&path).unwrap().epoch().to_string();
        assert_eq!(first, second);
    }

    #[test]
    fn epoch_guarda_created_at() {
        let store = Store::open_in_memory().unwrap();
        let created: String = store
            .lock()
            .unwrap()
            .query_row("SELECT value FROM meta WHERE key = 'created_at'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let parsed =
            time::OffsetDateTime::parse(&created, &time::format_description::well_known::Rfc3339);
        assert!(parsed.is_ok(), "{created}");
    }

    #[test]
    fn max_seq_de_banco_vazio_e_zero() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(store.max_seq().unwrap(), 0);
    }

    #[test]
    fn open_cria_diretorio_inexistente() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a").join("b").join("hub.sqlite");
        Store::open(&path).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn migrate_e_idempotente() {
        let store = Store::open_in_memory().unwrap();
        let conn = store.lock().unwrap();
        schema::migrate(&conn).unwrap();
        schema::migrate(&conn).unwrap();
        let epoch = schema::ensure_epoch(&conn).unwrap();
        assert_eq!(epoch, store.epoch());
    }
}
