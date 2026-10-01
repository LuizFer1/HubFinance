//! Linhas sincronizadas: LWW por linha com `seq`, push em lote e pull paginado.

use rusqlite::OptionalExtension;

use super::{Store, StoreError};
use crate::protocol::lww::{self, Verdict};
use crate::protocol::messages::{AcceptedRow, IgnoredRow, PushEntry, RejectedRow};
use crate::protocol::row;

#[derive(Debug, Default)]
pub struct BatchOutcome {
    pub accepted: Vec<AcceptedRow>,
    pub ignored: Vec<IgnoredRow>,
    pub rejected: Vec<RejectedRow>,
    pub max_seq: i64,
}

#[derive(Debug, Clone)]
pub struct StoredRow {
    pub table: String,
    pub seq: i64,
    pub row: serde_json::Value,
}

#[derive(Debug)]
pub struct Page {
    pub rows: Vec<StoredRow>,
    pub cursor: i64,
    pub has_more: bool,
}

impl Store {
    /// Uma transacao para o lote inteiro: ou tudo que foi aceito entra, ou nada. `seq` e
    /// `MAX(seq) + 1` lido dentro da transacao — o SQLite serializa escritores, entao nao ha
    /// corrida.
    pub fn apply_batch(
        &self,
        origin: &str,
        entries: &[PushEntry],
    ) -> Result<BatchOutcome, StoreError> {
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;
        let mut outcome = BatchOutcome::default();
        {
            let mut current_stmt =
                tx.prepare("SELECT updated_at FROM rows WHERE tbl = ?1 AND id = ?2")?;
            let mut next_seq_stmt = tx.prepare("SELECT COALESCE(MAX(seq), 0) + 1 FROM rows")?;
            let mut upsert_stmt = tx.prepare(
                "INSERT INTO rows (tbl, id, updated_at, deleted_at, origin, data, seq)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT (tbl, id) DO UPDATE SET
                   updated_at = excluded.updated_at, deleted_at = excluded.deleted_at,
                   origin = excluded.origin, data = excluded.data, seq = excluded.seq",
            )?;

            for (index, entry) in entries.iter().enumerate() {
                // Linha invalida e reportada e o lote segue: um campo ruim numa linha nao pode
                // travar o sync de todas as outras.
                let valid = match row::parse_row(&entry.table, &entry.row) {
                    Ok(valid) => valid,
                    Err(err) => {
                        outcome.rejected.push(RejectedRow {
                            index,
                            error: err.code().to_string(),
                            message: err.message(),
                        });
                        continue;
                    }
                };
                // Lido dentro da transacao: uma segunda entrada da mesma linha no lote decide
                // contra a primeira, como se tivessem chegado em dois pushes.
                let current: Option<String> = current_stmt
                    .query_row((&valid.table, &valid.id), |r| r.get(0))
                    .optional()?;
                match lww::decide(current.as_deref(), &valid.updated_at) {
                    Verdict::Accept => {
                        let seq: i64 = next_seq_stmt.query_row([], |r| r.get(0))?;
                        let data = serde_json::to_string(&valid.data)?;
                        upsert_stmt.execute((
                            &valid.table,
                            &valid.id,
                            &valid.updated_at,
                            &valid.deleted_at,
                            origin,
                            &data,
                            seq,
                        ))?;
                        outcome.accepted.push(AcceptedRow {
                            table: valid.table,
                            id: valid.id,
                            seq,
                        });
                    }
                    Verdict::Ignore(reason) => outcome.ignored.push(IgnoredRow {
                        table: valid.table,
                        id: valid.id,
                        reason: reason.as_str().to_string(),
                    }),
                }
            }
            outcome.max_seq =
                tx.query_row("SELECT COALESCE(MAX(seq), 0) FROM rows", [], |r| r.get(0))?;
        }
        tx.commit()?;
        Ok(outcome)
    }

    /// `seq > cursor AND origin != exclude_origin ORDER BY seq LIMIT limit + 1`.
    /// Sem `has_more`, `cursor` devolvido e `MAX(seq)` global, para o celular pular as
    /// proprias linhas.
    pub fn pull_after(
        &self,
        cursor: i64,
        exclude_origin: &str,
        limit: usize,
    ) -> Result<Page, StoreError> {
        let mut conn = self.lock()?;
        // Transacao de leitura: a pagina e o `MAX(seq)` precisam ver o mesmo estado, senao o
        // cursor devolvido poderia pular uma linha gravada entre as duas consultas.
        let tx = conn.transaction()?;
        // O `+1` so serve para saber se ha mais; essa linha extra nao e devolvida.
        let fetch = i64::try_from(limit.saturating_add(1)).unwrap_or(i64::MAX);
        let mut rows = Vec::new();
        {
            let mut stmt = tx.prepare(
                "SELECT tbl, seq, data FROM rows
                 WHERE seq > ?1 AND origin != ?2
                 ORDER BY seq LIMIT ?3",
            )?;
            let mut query = stmt.query((cursor, exclude_origin, fetch))?;
            while let Some(r) = query.next()? {
                let data: String = r.get(2)?;
                rows.push(StoredRow {
                    table: r.get(0)?,
                    seq: r.get(1)?,
                    row: serde_json::from_str(&data)?,
                });
            }
        }
        let has_more = rows.len() > limit;
        rows.truncate(limit);
        let cursor = match rows.last() {
            Some(last) if has_more => last.seq,
            _ => tx.query_row("SELECT COALESCE(MAX(seq), 0) FROM rows", [], |r| r.get(0))?,
        };
        tx.commit()?;
        Ok(Page {
            rows,
            cursor,
            has_more,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    const A: &str = "01HZZZZZZZZZZZZZZZZZZZZZZA";
    const B: &str = "01HZZZZZZZZZZZZZZZZZZZZZZB";
    const ID1: &str = "01HZZZZZZZZZZZZZZZZZZZZZC1";
    const ID2: &str = "01HZZZZZZZZZZZZZZZZZZZZZC2";
    const ID3: &str = "01HZZZZZZZZZZZZZZZZZZZZZC3";

    fn t(millis: u64) -> String {
        format!("{millis:013}-0000-{A}")
    }

    fn entry(
        table: &str,
        id: &str,
        updated_at: &str,
        deleted_at: Option<&str>,
        extra: Value,
    ) -> PushEntry {
        let mut row = json!({
            "id": id,
            "createdAt": "2026-10-01T18:00:00.000Z",
            "updatedAt": updated_at,
            "deletedAt": deleted_at,
            "dirty": 1,
        });
        if let (Some(row), Some(extra)) = (row.as_object_mut(), extra.as_object()) {
            row.extend(extra.clone());
        }
        PushEntry {
            table: table.to_string(),
            row,
        }
    }

    fn cat(id: &str, millis: u64) -> PushEntry {
        entry(
            "categories",
            id,
            &t(millis),
            None,
            json!({ "name": "Mercado" }),
        )
    }

    fn stored(store: &Store, id: &str) -> (String, Option<String>, Value, i64) {
        let conn = store.lock().unwrap();
        conn.query_row(
            "SELECT updated_at, deleted_at, data, seq FROM rows WHERE tbl = 'categories' AND id = ?1",
            [id],
            |r| {
                let data: String = r.get(2)?;
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    serde_json::from_str(&data).unwrap(),
                    r.get(3)?,
                ))
            },
        )
        .unwrap()
    }

    fn count(store: &Store) -> i64 {
        let conn = store.lock().unwrap();
        conn.query_row("SELECT COUNT(*) FROM rows", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn linha_nova_e_aceita_com_seq_1_e_sem_dirty() {
        let store = Store::open_in_memory().unwrap();
        let out = store.apply_batch(A, &[cat(ID1, 1000)]).unwrap();
        assert_eq!(out.accepted.len(), 1);
        assert_eq!(out.accepted[0].seq, 1);
        assert_eq!(out.accepted[0].id, ID1);
        assert_eq!(out.accepted[0].table, "categories");
        assert_eq!(out.max_seq, 1);
        assert_eq!(store.max_seq().unwrap(), 1);
        let (_, _, data, _) = stored(&store, ID1);
        assert!(data.get("dirty").is_none());
        assert_eq!(data["name"], json!("Mercado"));
    }

    #[test]
    fn repetir_o_lote_e_idempotente() {
        let store = Store::open_in_memory().unwrap();
        store.apply_batch(A, &[cat(ID1, 1000)]).unwrap();
        let out = store.apply_batch(A, &[cat(ID1, 1000)]).unwrap();
        assert!(out.accepted.is_empty());
        assert_eq!(out.ignored.len(), 1);
        assert_eq!(out.ignored[0].reason, "same");
        assert_eq!(out.max_seq, 1);
        assert_eq!(store.max_seq().unwrap(), 1);
    }

    #[test]
    fn versao_mais_antiga_e_ignorada() {
        let store = Store::open_in_memory().unwrap();
        store.apply_batch(A, &[cat(ID1, 2000)]).unwrap();
        let out = store.apply_batch(B, &[cat(ID1, 1000)]).unwrap();
        assert_eq!(out.ignored[0].reason, "older");
        let (updated_at, _, _, seq) = stored(&store, ID1);
        assert_eq!(updated_at, t(2000));
        assert_eq!(seq, 1);
    }

    #[test]
    fn versao_mais_nova_troca_a_linha_e_ganha_seq_novo() {
        let store = Store::open_in_memory().unwrap();
        store.apply_batch(A, &[cat(ID1, 1000)]).unwrap();
        let newer = entry(
            "categories",
            ID1,
            &t(2000),
            None,
            json!({ "name": "Feira" }),
        );
        let out = store.apply_batch(B, &[newer]).unwrap();
        assert_eq!(out.accepted[0].seq, 2);
        let (updated_at, _, data, seq) = stored(&store, ID1);
        assert_eq!(updated_at, t(2000));
        assert_eq!(data["name"], json!("Feira"));
        assert_eq!(seq, 2);
        assert_eq!(count(&store), 1);
    }

    #[test]
    fn apagar_e_reviver() {
        let store = Store::open_in_memory().unwrap();
        store.apply_batch(A, &[cat(ID1, 1000)]).unwrap();
        let deleted = entry("categories", ID1, &t(2000), Some(&t(2000)), json!({}));
        assert_eq!(store.apply_batch(B, &[deleted]).unwrap().accepted.len(), 1);
        assert_eq!(stored(&store, ID1).1, Some(t(2000)));
        let revived = entry("categories", ID1, &t(3000), None, json!({}));
        assert_eq!(store.apply_batch(A, &[revived]).unwrap().accepted[0].seq, 3);
        assert_eq!(stored(&store, ID1).1, None);
    }

    #[test]
    fn invalida_e_reportada_sem_derrubar_as_validas_nem_consumir_seq() {
        let store = Store::open_in_memory().unwrap();
        let bad = entry("categories", "abc", "x", None, json!({}));
        let out = store
            .apply_batch(A, &[cat(ID1, 1000), bad, cat(ID2, 1000)])
            .unwrap();
        assert_eq!(out.rejected.len(), 1);
        assert_eq!(out.rejected[0].index, 1);
        assert_eq!(out.rejected[0].error, "invalid_id");
        assert!(!out.rejected[0].message.is_empty());
        let seqs: Vec<i64> = out.accepted.iter().map(|a| a.seq).collect();
        assert_eq!(seqs, vec![1, 2]);
    }

    #[test]
    fn mesma_linha_duas_vezes_no_lote_decide_contra_a_primeira() {
        let store = Store::open_in_memory().unwrap();
        let second = entry(
            "categories",
            ID1,
            &t(2000),
            None,
            json!({ "name": "Feira" }),
        );
        let out = store.apply_batch(A, &[cat(ID1, 1000), second]).unwrap();
        let seqs: Vec<i64> = out.accepted.iter().map(|a| a.seq).collect();
        assert_eq!(seqs, vec![1, 2]);
        let (_, _, data, seq) = stored(&store, ID1);
        assert_eq!(data["name"], json!("Feira"));
        assert_eq!(seq, 2);
        // Ordem inversa: a segunda e mais antiga e perde para a primeira, do mesmo lote.
        let out = store
            .apply_batch(A, &[cat(ID2, 2000), cat(ID2, 1000)])
            .unwrap();
        assert_eq!(out.accepted.len(), 1);
        assert_eq!(out.ignored[0].reason, "older");
    }

    #[test]
    fn erro_de_banco_no_meio_desfaz_o_lote_inteiro() {
        let store = Store::open_in_memory().unwrap();
        let trigger = format!(
            "CREATE TRIGGER boom BEFORE INSERT ON rows WHEN NEW.id = '{ID2}' \
             BEGIN SELECT RAISE(ABORT, 'boom'); END;"
        );
        store.lock().unwrap().execute_batch(&trigger).unwrap();
        let result = store.apply_batch(A, &[cat(ID1, 1000), cat(ID2, 1000)]);
        assert!(result.is_err());
        assert_eq!(count(&store), 0);
        assert_eq!(store.max_seq().unwrap(), 0);
    }

    #[test]
    fn pull_devolve_em_ordem_de_seq_sem_dirty() {
        let store = Store::open_in_memory().unwrap();
        store
            .apply_batch(A, &[cat(ID2, 1000), cat(ID1, 1000)])
            .unwrap();
        let page = store.pull_after(0, "outro", 10).unwrap();
        let seqs: Vec<i64> = page.rows.iter().map(|r| r.seq).collect();
        assert_eq!(seqs, vec![1, 2]);
        assert_eq!(page.rows[0].row["id"], json!(ID2));
        assert_eq!(page.rows[0].table, "categories");
        assert!(page.rows.iter().all(|r| r.row.get("dirty").is_none()));
        assert_eq!(page.cursor, 2);
        assert!(!page.has_more);
    }

    #[test]
    fn pull_exclui_a_propria_origem_mas_avanca_o_cursor() {
        let store = Store::open_in_memory().unwrap();
        store.apply_batch(A, &[cat(ID1, 1000)]).unwrap();
        store.apply_batch(B, &[cat(ID2, 1000)]).unwrap();
        store.apply_batch(A, &[cat(ID3, 1000)]).unwrap();
        let page = store.pull_after(0, A, 10).unwrap();
        assert_eq!(page.rows.len(), 1);
        assert_eq!(page.rows[0].row["id"], json!(ID2));
        assert_eq!(page.cursor, 3);
        assert!(!page.has_more);
    }

    #[test]
    fn pull_paginado() {
        let store = Store::open_in_memory().unwrap();
        let entries: Vec<PushEntry> = (1..=5)
            .map(|n| cat(&format!("01HZZZZZZZZZZZZZZZZZZZZZD{n}"), 1000))
            .collect();
        store.apply_batch(A, &entries).unwrap();

        let first = store.pull_after(0, B, 2).unwrap();
        assert_eq!(first.rows.len(), 2);
        assert!(first.has_more);
        assert_eq!(first.cursor, 2);

        let second = store.pull_after(first.cursor, B, 2).unwrap();
        let seqs: Vec<i64> = second.rows.iter().map(|r| r.seq).collect();
        assert_eq!(seqs, vec![3, 4]);
        assert!(second.has_more);
        assert_eq!(second.cursor, 4);

        let last = store.pull_after(second.cursor, B, 2).unwrap();
        assert_eq!(last.rows.len(), 1);
        assert_eq!(last.cursor, 5);
        assert!(!last.has_more);
    }

    #[test]
    fn pull_de_banco_vazio() {
        let store = Store::open_in_memory().unwrap();
        let page = store.pull_after(0, A, 10).unwrap();
        assert!(page.rows.is_empty());
        assert_eq!(page.cursor, 0);
        assert!(!page.has_more);
    }
}
