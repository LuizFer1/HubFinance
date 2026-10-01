//! Dataset do dashboard no nucleo: carga completa no boot e incremental depois de cada push.

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crate::dashboard::dataset::{ApplyReport, Dataset};
use crate::store::Store;

/// O celular manda lotes em sequencia; sem esperar, cada lote clonaria o `Dataset` (a UI
/// segura o `Arc` anterior). Meio segundo junta a rajada e ainda parece imediato.
pub const REFRESH_DEBOUNCE: Duration = Duration::from_millis(500);

/// As tabelas que o contrato de campos le; o resto nem sai do banco.
pub const TABLES: [&str; 5] = [
    "transactions",
    "categories",
    "users",
    "paymentMethods",
    "recurrences",
];

#[derive(Default)]
pub struct DashboardState {
    pub dataset: Arc<Dataset>,
}

impl DashboardState {
    /// `rows_since(loaded_seq)` em `spawn_blocking` e aplica o delta. Sem novidade, nao toca
    /// no `Arc`: a UI compara por ponteiro para saber se precisa recalcular.
    pub async fn refresh(
        &mut self,
        store: &Arc<Store>,
        now: SystemTime,
    ) -> Result<ApplyReport, String> {
        let after = self.dataset.loaded_seq;
        let store = Arc::clone(store);
        let rows = tokio::task::spawn_blocking(move || store.rows_since(after, &TABLES))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        if rows.is_empty() {
            return Ok(ApplyReport {
                applied: 0,
                ignored_total: self.dataset.ignored_total(),
            });
        }
        // `make_mut` clona o Dataset se a UI ainda segura o Arc anterior: O(n), so em evento
        // de sync.
        let dataset = Arc::make_mut(&mut self.dataset);
        let report = dataset.apply(rows);
        dataset.refreshed_at = Some(now);
        Ok(report)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::protocol::messages::PushEntry;

    const ORIGIN: &str = "01HZZZZZZZZZZZZZZZZZZZZZZA";

    fn hlc(millis: u64) -> String {
        format!("{millis:013}-0000-{ORIGIN}")
    }

    pub(crate) fn push_entry(
        table: &str,
        id: &str,
        millis: u64,
        deleted: bool,
        fields: Value,
    ) -> PushEntry {
        let mut row = json!({
            "id": id,
            "createdAt": "2026-10-01T18:00:00.000Z",
            "updatedAt": hlc(millis),
            "deletedAt": if deleted { Value::String(hlc(millis)) } else { Value::Null },
            "dirty": 1,
        });
        if let (Some(row), Some(fields)) = (row.as_object_mut(), fields.as_object()) {
            row.extend(fields.clone());
        }
        PushEntry {
            table: table.to_string(),
            row,
        }
    }

    pub(crate) fn tx(amount: Value) -> Value {
        json!({
            "kind": "expense",
            "description": "Mercado",
            "amountMinor": amount,
            "occurredOn": "2026-09-14"
        })
    }

    #[tokio::test]
    async fn refresh_carrega_tudo_e_depois_so_o_delta() {
        let store = Arc::new(Store::open_in_memory().unwrap());
        store
            .apply_batch(
                ORIGIN,
                &[
                    push_entry(
                        "transactions",
                        "01HZZZZZZZZZZZZZZZZZZZZZT1",
                        1000,
                        false,
                        tx(json!(100)),
                    ),
                    push_entry(
                        "transactions",
                        "01HZZZZZZZZZZZZZZZZZZZZZT2",
                        1000,
                        true,
                        tx(json!(200)),
                    ),
                    push_entry(
                        "transactions",
                        "01HZZZZZZZZZZZZZZZZZZZZZT3",
                        1000,
                        false,
                        tx(json!("muito")),
                    ),
                    push_entry(
                        "categories",
                        "01HZZZZZZZZZZZZZZZZZZZZZC1",
                        1000,
                        false,
                        json!({ "name": "Mercado", "color": "orange", "icon": "utensils" }),
                    ),
                ],
            )
            .unwrap();
        let mut state = DashboardState::default();
        let report = state.refresh(&store, SystemTime::UNIX_EPOCH).await.unwrap();
        assert_eq!(report.applied, 4);
        assert_eq!(state.dataset.transactions.len(), 2);
        assert_eq!(state.dataset.alive_transactions().count(), 1);
        assert_eq!(state.dataset.ignored_total(), 1);
        assert_eq!(state.dataset.loaded_seq, 4);
        assert_eq!(state.dataset.refreshed_at, Some(SystemTime::UNIX_EPOCH));

        let before = Arc::clone(&state.dataset);
        let report = state.refresh(&store, SystemTime::now()).await.unwrap();
        assert_eq!(report.applied, 0);
        assert!(Arc::ptr_eq(&before, &state.dataset));

        store
            .apply_batch(
                ORIGIN,
                &[push_entry(
                    "transactions",
                    "01HZZZZZZZZZZZZZZZZZZZZZT4",
                    2000,
                    false,
                    tx(json!(5)),
                )],
            )
            .unwrap();
        let report = state.refresh(&store, SystemTime::now()).await.unwrap();
        assert_eq!(report.applied, 1);
        assert!(!Arc::ptr_eq(&before, &state.dataset));
        assert_eq!(state.dataset.alive_transactions().count(), 2);
        // O Arc antigo (o que a UI segurava) ficou como estava.
        assert_eq!(before.alive_transactions().count(), 1);
    }
}
