//! Schema SQLite do hub e o `epoch`.

use std::time::Duration;

use rusqlite::{Connection, OptionalExtension};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

pub const SCHEMA_VERSION: i32 = 2;

/// Versao 1. `rows` e estado (uma linha por entidade), nao log: cresce com entidades, nao com
/// edicoes, por isso nao ha compactacao. `seq` unico e o que torna o cursor do pull confiavel.
const SCHEMA_V1: &str = "
CREATE TABLE meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE rows (
  tbl        TEXT NOT NULL,
  id         TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT,
  origin     TEXT NOT NULL,
  data       TEXT NOT NULL,
  seq        INTEGER NOT NULL,
  PRIMARY KEY (tbl, id)
) WITHOUT ROWID;
CREATE UNIQUE INDEX rows_by_seq ON rows (seq);

CREATE TABLE devices (
  device_id    TEXT PRIMARY KEY,
  name         TEXT NOT NULL,
  key_hash     TEXT NOT NULL UNIQUE,
  paired_at    TEXT NOT NULL,
  last_seen_at TEXT,
  last_push_at TEXT,
  last_pull_at TEXT,
  revoked_at   TEXT
);
";

/// Versao 2. `user_id` e o `meta.localUserId` do app, mandado ao parear: liga o aparelho a
/// pessoa na tela Conexao. Anulavel porque app sem perfil (ou mais antigo) ainda pareia, e
/// porque aparelho pareado na v1 nao tem como saber.
const SCHEMA_V2: &str = "ALTER TABLE devices ADD COLUMN user_id TEXT;";

/// WAL pela durabilidade: um commit interrompido por queda de energia nao corrompe o banco.
/// (Nao e por concorrencia: push e pull passam pelo mesmo Mutex da `Store`.)
/// Em memoria o SQLite ignora o pedido e fica em `memory`, o que basta para os testes.
pub fn configure(conn: &Connection) -> rusqlite::Result<()> {
    let _mode: String = conn.pragma_update_and_check(None, "journal_mode", "WAL", |r| r.get(0))?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.busy_timeout(Duration::from_secs(5))?;
    Ok(())
}

/// Guardado por `user_version`: reabrir um banco ja migrado nao reexecuta o DDL.
pub fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    let version: i32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    // Um passo por versao, cada um na sua transacao: banco novo passa por todos, banco v1
    // so pelo que falta. Pular um passo deixaria coluna faltando sem erro ate a primeira
    // consulta.
    if version < 1 {
        conn.execute_batch(&format!(
            "BEGIN;\n{SCHEMA_V1}\nPRAGMA user_version = 1;\nCOMMIT;"
        ))?;
    }
    // Ultimo passo: leva ao `SCHEMA_VERSION`. Versao nova = mais um bloco como este.
    if version < SCHEMA_VERSION {
        conn.execute_batch(&format!(
            "BEGIN;\n{SCHEMA_V2}\nPRAGMA user_version = {SCHEMA_VERSION};\nCOMMIT;"
        ))?;
    }
    Ok(())
}

/// O `epoch` nasce com o banco e nunca muda: e como o celular percebe que o hub foi
/// recriado e que o cursor que ele guardou nao vale mais.
pub fn ensure_epoch(conn: &Connection) -> rusqlite::Result<String> {
    let existing: Option<String> = conn
        .query_row("SELECT value FROM meta WHERE key = 'epoch'", [], |r| {
            r.get(0)
        })
        .optional()?;
    if let Some(epoch) = existing {
        return Ok(epoch);
    }
    let epoch = ulid::Ulid::generate().to_string();
    let created_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    conn.execute(
        "INSERT INTO meta (key, value) VALUES ('epoch', ?1), ('created_at', ?2)",
        (&epoch, &created_at),
    )?;
    Ok(epoch)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user_version(conn: &Connection) -> i32 {
        conn.query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn banco_v1_migra_para_v2_e_preserva_os_aparelhos() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hub.sqlite");
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(&format!(
                "BEGIN;\n{SCHEMA_V1}\nPRAGMA user_version = 1;\nCOMMIT;"
            ))
            .unwrap();
            conn.execute(
                "INSERT INTO devices (device_id, name, key_hash, paired_at)
                 VALUES ('01HZZZZZZZZZZZZZZZZZZZZZZA', 'Pixel', 'h', '2026-10-01T18:00:00Z')",
                [],
            )
            .unwrap();
        }
        let conn = Connection::open(&path).unwrap();
        migrate(&conn).unwrap();
        assert_eq!(user_version(&conn), 2);
        let (name, user_id): (String, Option<String>) = conn
            .query_row("SELECT name, user_id FROM devices", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(name, "Pixel");
        assert_eq!(user_id, None);
        // Migrar de novo nao faz nada (nem falha com coluna duplicada).
        migrate(&conn).unwrap();
        assert_eq!(user_version(&conn), SCHEMA_VERSION);
    }
}
