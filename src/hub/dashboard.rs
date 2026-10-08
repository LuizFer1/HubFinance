//! Dataset do dashboard no nucleo: carga completa no boot e incremental depois de cada push.

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crate::dashboard::dataset::{ApplyReport, Dataset};
use crate::store::Store;

/// O celular manda lotes em sequencia; sem esperar, cada lote clonaria o `Dataset` (a UI
/// segura o `Arc` anterior). Meio segundo junta a rajada e ainda parece imediato.
pub const REFRESH_DEBOUNCE: Duration = Duration::from_millis(500);

/// As tabelas que o contrato de campos le; o resto nem sai do banco. `reserves` e
/// `reserveMovements` sao lidas no formato que o app grava.
pub const TABLES: [&str; 7] = [
    "transactions",
    "categories",
    "users",
    "paymentMethods",
    "recurrences",
    "reserves",
    "reserveMovements",
];

#[derive(Default)]
pub struct DashboardState {
    pub dataset: Arc<Dataset>,
    /// Maior `seq` do banco ja olhado por um refresh. Nao e o `loaded_seq` do dataset: linha de
    /// tabela fora do contrato nem sai do banco, entao `loaded_seq` pode ficar para tras para
    /// sempre e o tique refaria o refresh a cada volta.
    pub seen_seq: i64,
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
        // `max_seq` antes de `rows_since`: toda linha com `seq` ate ele ja estava gravada (o
        // lote inteiro e uma transacao sob o mesmo lock), entao nenhuma fica para tras.
        let (max_seq, rows) = tokio::task::spawn_blocking(move || {
            let max_seq = store.max_seq()?;
            Ok::<_, crate::store::StoreError>((max_seq, store.rows_since(after, &TABLES)?))
        })
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
        self.seen_seq = self.seen_seq.max(max_seq);
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
        self.seen_seq = self.seen_seq.max(dataset.loaded_seq);
        Ok(report)
    }

    /// O banco tem linha que nenhum refresh olhou? O evento `Pushed` sai do handler depois do
    /// `.await` da gravacao; se a requisicao for cancelada no meio (celular fechou a conexao),
    /// as linhas entram e o aviso nao. O tique do nucleo pergunta isto e se recupera sozinho.
    pub async fn is_behind(&self, store: &Arc<Store>) -> Result<bool, String> {
        let store = Arc::clone(store);
        let max_seq = tokio::task::spawn_blocking(move || store.max_seq())
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        Ok(max_seq > self.seen_seq.max(self.dataset.loaded_seq))
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
    async fn push_sem_evento_e_recuperado_no_tique() {
        let store = Arc::new(Store::open_in_memory().unwrap());
        let mut state = DashboardState::default();
        state.refresh(&store, SystemTime::now()).await.unwrap();
        assert!(!state.is_behind(&store).await.unwrap());

        // Gravou e o evento se perdeu (handler cancelado depois do `apply_batch`).
        store
            .apply_batch(
                ORIGIN,
                &[push_entry(
                    "transactions",
                    "01HZZZZZZZZZZZZZZZZZZZZZT1",
                    1000,
                    false,
                    tx(json!(100)),
                )],
            )
            .unwrap();
        // O tique ve a diferenca e o refresh traz a linha.
        assert!(state.is_behind(&store).await.unwrap());
        state.refresh(&store, SystemTime::now()).await.unwrap();
        assert_eq!(state.dataset.alive_transactions().count(), 1);
        assert!(!state.is_behind(&store).await.unwrap());

        // Tabela que o dashboard nao le avanca o banco mas nao deixa o tique em laco.
        store
            .apply_batch(
                ORIGIN,
                &[push_entry(
                    "recurrenceAdjustments",
                    "01HZZZZZZZZZZZZZZZZZZZZZA1",
                    2000,
                    false,
                    json!({ "x": 1 }),
                )],
            )
            .unwrap();
        assert!(state.is_behind(&store).await.unwrap());
        state.refresh(&store, SystemTime::now()).await.unwrap();
        assert!(!state.is_behind(&store).await.unwrap());
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

    #[tokio::test]
    async fn refresh_le_reservas_e_movimentacoes() {
        let store = Arc::new(Store::open_in_memory().unwrap());
        let reserve =
            |kind: &str| json!({ "kind": kind, "name": "Viagem", "icon": "plane", "color": "sky" });
        store
            .apply_batch(
                ORIGIN,
                &[
                    push_entry(
                        "reserves",
                        "01HZZZZZZZZZZZZZZZZZZZZZB1",
                        1000,
                        false,
                        reserve("goal"),
                    ),
                    push_entry(
                        "reserveMovements",
                        "01HZZZZZZZZZZZZZZZZZZZZZV1",
                        1000,
                        false,
                        json!({
                            "reserveId": "01HZZZZZZZZZZZZZZZZZZZZZB1",
                            "kind": "deposit",
                            "amountMinor": 5000,
                            "occurredOn": "2026-09-06"
                        }),
                    ),
                    push_entry(
                        "reserves",
                        "01HZZZZZZZZZZZZZZZZZZZZZB9",
                        1000,
                        false,
                        reserve("box"),
                    ),
                ],
            )
            .unwrap();
        let mut state = DashboardState::default();
        state.refresh(&store, SystemTime::UNIX_EPOCH).await.unwrap();
        assert_eq!(state.dataset.reserve_count(), 1);
        assert_eq!(state.dataset.alive_reserve_movements().count(), 1);
        assert_eq!(state.dataset.ignored_total(), 1);
        assert!(state.dataset.ignored["reserves"].contains("01HZZZZZZZZZZZZZZZZZZZZZB9"));
    }
}
