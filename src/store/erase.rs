//! Apagar os dados que um aparelho enviou, por area do app.
//!
//! EXCECAO a "o hub nunca edita dado": esta e a unica escrita que nasce no hub, e so acontece a
//! pedido explicito do usuario (desconectar um aparelho marcando areas, ou decidir sobre um
//! aparelho ja removido). O hub nao inventa regra de negocio: reescreve a linha como o app
//! reescreve ao apagar (tombstone: `deletedAt = updatedAt = HLC novo`), nunca faz `DELETE`,
//! porque uma linha que some so do hub deixaria os celulares com ela e o dashboard divergindo.
//!
//! Catalogo (categorias, formas de pagamento) e perfil (`users`) nunca entram aqui: sao
//! compartilhados e regravados por qualquer aparelho.

use std::collections::{BTreeMap, HashMap, HashSet};

use rusqlite::{Connection, OptionalExtension, TransactionBehavior};

use super::{Store, StoreError};
use crate::protocol::hlc::{HLC_LEN, tombstone_after};

/// As quatro areas do modal. Cada uma agrupa tabelas do app; as dependencias (reajustes de uma
/// serie, movimentos de uma reserva) vivem **dentro** da area.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Area {
    Budgets,
    Transactions,
    Reserves,
    Recurrences,
}

impl Area {
    pub const ALL: [Area; 4] = [
        Area::Budgets,
        Area::Transactions,
        Area::Reserves,
        Area::Recurrences,
    ];

    fn bit(self) -> u8 {
        match self {
            Area::Budgets => 1,
            Area::Transactions => 2,
            Area::Reserves => 4,
            Area::Recurrences => 8,
        }
    }
}

/// Tabelas do app que alguma area pode apagar; o `GROUP BY` de deteccao olha so estas.
const AREA_TABLES: [&str; 6] = [
    "budgets",
    "transactions",
    "reserves",
    "reserveMovements",
    "recurrences",
    "recurrenceAdjustments",
];

/// Conjunto de areas. Vazio = "Manter tudo" (o padrao); cheio = "Apagar tudo".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AreaSet(u8);

impl AreaSet {
    pub fn all() -> AreaSet {
        Area::ALL.into_iter().collect()
    }

    pub fn contains(self, area: Area) -> bool {
        self.0 & area.bit() != 0
    }

    pub fn insert(&mut self, area: Area) {
        self.0 |= area.bit();
    }

    pub fn remove(&mut self, area: Area) {
        self.0 &= !area.bit();
    }

    pub fn toggle(&mut self, area: Area) {
        self.0 ^= area.bit();
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn iter(self) -> impl Iterator<Item = Area> {
        Area::ALL.into_iter().filter(move |a| self.contains(*a))
    }
}

impl FromIterator<Area> for AreaSet {
    fn from_iter<I: IntoIterator<Item = Area>>(iter: I) -> Self {
        let mut set = AreaSet::default();
        for area in iter {
            set.insert(area);
        }
        set
    }
}

/// O que o modal mostra e o que a escrita apaga: sai do **mesmo** plano, entao a contagem nao
/// pode divergir do que sera apagado.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeviceData {
    pub budgets: usize,
    pub transactions: usize,
    /// Das `transactions`, quantas sao transferencias internas (`reportState = "transfer"`).
    pub internal_transfers: usize,
    /// Reservas que caem (todas com os seus movimentos).
    pub reserves: usize,
    pub reserve_movements: usize,
    /// Reservas do aparelho **mantidas** porque tem movimento vivo de outro aparelho; so os
    /// movimentos do aparelho nelas caem.
    pub reserves_kept: usize,
    pub recurrences: usize,
    pub adjustments: usize,
    /// Dos `adjustments`, quantos sao de outros aparelhos (cascata da serie apagada).
    pub adjustments_other: usize,
    /// Lancamentos vivos gerados pelas series que caem: continuam (o app tambem nao faz
    /// cascata); so a area Transferencias apaga os do aparelho.
    pub generated: usize,
}

impl DeviceData {
    /// Itens que a area apaga (o numero que o botao "Remover e apagar N itens" soma).
    pub fn items(&self, area: Area) -> usize {
        match area {
            Area::Budgets => self.budgets,
            Area::Transactions => self.transactions,
            Area::Reserves => self.reserves + self.reserve_movements,
            Area::Recurrences => self.recurrences + self.adjustments,
        }
    }

    pub fn total_items(&self, areas: AreaSet) -> usize {
        areas.iter().map(|a| self.items(a)).sum()
    }

    /// Nada a apagar em nenhuma area. E isto, e nao a contagem bruta por tabela, que decide se
    /// um aparelho removido ainda e "orfao": uma reserva mantida por ter movimento alheio conta
    /// na contagem bruta para sempre, e o aviso nunca sairia.
    pub fn is_empty(&self) -> bool {
        self.total_items(AreaSet::all()) == 0
    }

    /// Zera o que nao esta em `areas`: o relatorio de uma apagada so fala do que foi apagado.
    fn masked(&self, areas: AreaSet) -> DeviceData {
        let keep = |area: Area, n: usize| if areas.contains(area) { n } else { 0 };
        DeviceData {
            budgets: keep(Area::Budgets, self.budgets),
            transactions: keep(Area::Transactions, self.transactions),
            internal_transfers: keep(Area::Transactions, self.internal_transfers),
            reserves: keep(Area::Reserves, self.reserves),
            reserve_movements: keep(Area::Reserves, self.reserve_movements),
            reserves_kept: keep(Area::Reserves, self.reserves_kept),
            recurrences: keep(Area::Recurrences, self.recurrences),
            adjustments: keep(Area::Recurrences, self.adjustments),
            adjustments_other: keep(Area::Recurrences, self.adjustments_other),
            generated: keep(Area::Recurrences, self.generated),
        }
    }
}

/// Linhas vivas de um aparelho numa tabela (o `GROUP BY` de `device_data_counts`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableCount {
    pub table: String,
    pub count: usize,
}

/// CRITERIO DE "LINHA DO APARELHO" — **PROVISORIO, NAO CONFIRMADO PELO DONO**. E o unico lugar
/// que o define; contagem, deteccao de orfaos e escrita passam por aqui, entao trocar o criterio
/// e trocar este corpo.
///
/// Hoje (conservador): o aparelho foi quem enviou a versao vigente (`origin`) **e** quem a
/// carimbou (sufixo do HLC de `updated_at`), e a linha esta viva. Exigir os dois evita apagar
/// linha que o aparelho so reenviou (`epoch_mismatch` o torna `origin` de linha alheia) ou que
/// outro aparelho editou depois (o sufixo muda). A alternativa menos conservadora e so
/// `origin`, que apaga mais e pega edicoes do aparelho em linha de outros.
///
/// `device` e uma expressao SQL (`?1`, `d.device_id`); a tabela de linhas precisa ter o alias
/// `r`. Metas nao tem autor, entao para elas este e o unico sinal.
fn owned_by(device: &str) -> String {
    // SQLite conta a partir de 1: o sufixo do HLC (26 chars) comeca depois de
    // `millis(13)-counter(4)-`, ou seja, na posicao 20.
    let suffix_from = HLC_LEN - 26 + 1;
    format!(
        "(r.origin = {device} AND substr(r.updated_at, {suffix_from}) = {device} \
         AND r.deleted_at IS NULL)"
    )
}

struct Target {
    area: Area,
    table: &'static str,
    id: String,
}

struct Plan {
    targets: Vec<Target>,
    summary: DeviceData,
}

fn owned_ids(conn: &Connection, device_id: &str, table: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT r.id FROM rows r WHERE r.tbl = ?2 AND {} ORDER BY r.id",
        owned_by("?1")
    ))?;
    stmt.query_map((device_id, table), |r| r.get(0))?.collect()
}

/// Linhas vivas de `table` com um campo de referencia (`json_extract`) e a marca "do aparelho".
fn live_with_ref(
    conn: &Connection,
    device_id: &str,
    table: &str,
    field: &str,
) -> rusqlite::Result<Vec<(String, Option<String>, bool)>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT r.id, json_extract(r.data, '$.{field}'), {} FROM rows r \
         WHERE r.tbl = ?2 AND r.deleted_at IS NULL ORDER BY r.id",
        owned_by("?1")
    ))?;
    stmt.query_map((device_id, table), |r| {
        // Referencia que nao e texto (versao futura do app) vira "sem referencia", nao erro.
        Ok((
            r.get(0)?,
            r.get::<_, Option<String>>(1).ok().flatten(),
            r.get(2)?,
        ))
    })?
    .collect()
}

/// Calcula o que cada area apagaria hoje, com as dependencias. Quem escreve e quem mostra a
/// contagem usam este mesmo plano.
fn plan(conn: &Connection, device_id: &str) -> rusqlite::Result<Plan> {
    let mut targets = Vec::new();
    let mut summary = DeviceData::default();

    // Metas: nada depende delas (dependem da categoria, que nao e tocada).
    for id in owned_ids(conn, device_id, "budgets")? {
        summary.budgets += 1;
        targets.push(Target {
            area: Area::Budgets,
            table: "budgets",
            id,
        });
    }

    // Transferencias: a tabela `transactions` do aparelho. Ninguem referencia lancamento.
    for (id, state, _) in live_with_ref(conn, device_id, "transactions", "reportState")?
        .into_iter()
        .filter(|(_, _, owned)| *owned)
    {
        summary.transactions += 1;
        if state.as_deref() == Some("transfer") {
            summary.internal_transfers += 1;
        }
        targets.push(Target {
            area: Area::Transactions,
            table: "transactions",
            id,
        });
    }

    // Reservas: os movimentos do aparelho caem sempre; a reserva so cai se TODOS os movimentos
    // vivos dela sao do aparelho (senao o saldo dos outros aparelhos seria cortado), e entao
    // leva todos junto — nunca sobra movimento vivo de reserva apagada, que o app continuaria
    // descontando do saldo.
    let movements = live_with_ref(conn, device_id, "reserveMovements", "reserveId")?;
    let with_foreign_movement: HashSet<&str> = movements
        .iter()
        .filter(|(_, _, owned)| !owned)
        .filter_map(|(_, reserve, _)| reserve.as_deref())
        .collect();
    for (id, _, owned) in &movements {
        if *owned {
            summary.reserve_movements += 1;
            targets.push(Target {
                area: Area::Reserves,
                table: "reserveMovements",
                id: id.clone(),
            });
        }
    }
    for id in owned_ids(conn, device_id, "reserves")? {
        if with_foreign_movement.contains(id.as_str()) {
            summary.reserves_kept += 1;
        } else {
            summary.reserves += 1;
            targets.push(Target {
                area: Area::Reserves,
                table: "reserves",
                id,
            });
        }
    }

    // Recorrencias: series do aparelho; os reajustes vivos dessas series caem junto (de
    // qualquer origem: reajuste sem serie e lixo) e tambem os reajustes do proprio aparelho,
    // para apagar a area zerar o que ela conta. As ocorrencias ja materializadas NAO sao
    // tocadas: o app tambem nao faz cascata (`removeSeries`), so a area Transferencias as apaga.
    let series: HashSet<String> = owned_ids(conn, device_id, "recurrences")?
        .into_iter()
        .collect();
    for id in &series {
        summary.recurrences += 1;
        targets.push(Target {
            area: Area::Recurrences,
            table: "recurrences",
            id: id.clone(),
        });
    }
    for (id, series_ref, owned) in
        live_with_ref(conn, device_id, "recurrenceAdjustments", "recurrenceId")?
    {
        let of_erased_series = series_ref.as_deref().is_some_and(|s| series.contains(s));
        if owned || of_erased_series {
            summary.adjustments += 1;
            if !owned {
                summary.adjustments_other += 1;
            }
            targets.push(Target {
                area: Area::Recurrences,
                table: "recurrenceAdjustments",
                id,
            });
        }
    }
    if !series.is_empty() {
        summary.generated = live_with_ref(conn, device_id, "transactions", "recurrenceId")?
            .iter()
            .filter(|(_, series_ref, _)| series_ref.as_deref().is_some_and(|s| series.contains(s)))
            .count();
    }

    Ok(Plan { targets, summary })
}

/// Reescreve as linhas do plano nas `areas` como tombstone. Dentro da transacao de quem chama.
fn erase_planned(
    conn: &Connection,
    device_id: &str,
    areas: AreaSet,
    hub_device_id: &str,
) -> Result<DeviceData, StoreError> {
    let plan = plan(conn, device_id)?;
    let mut select = conn.prepare(
        "SELECT updated_at, data FROM rows WHERE tbl = ?1 AND id = ?2 AND deleted_at IS NULL",
    )?;
    let mut next_seq = conn.prepare("SELECT COALESCE(MAX(seq), 0) + 1 FROM rows")?;
    let mut update = conn.prepare(
        "UPDATE rows SET updated_at = ?3, deleted_at = ?3, origin = ?4, data = ?5, seq = ?6
         WHERE tbl = ?1 AND id = ?2",
    )?;
    for target in plan.targets.iter().filter(|t| areas.contains(t.area)) {
        let current: Option<(String, String)> = select
            .query_row((target.table, &target.id), |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;
        let Some((updated_at, data)) = current else {
            continue;
        };
        // Linha que ja entrou pelo `parse_row` tem HLC valido; se nao tiver, pula em vez de
        // gravar um tombstone que o app recusaria.
        let Some(hlc) = tombstone_after(&updated_at, hub_device_id) else {
            continue;
        };
        let mut value: serde_json::Value = serde_json::from_str(&data)?;
        if let Some(object) = value.as_object_mut() {
            object.insert("updatedAt".into(), hlc.clone().into());
            object.insert("deletedAt".into(), hlc.clone().into());
            object.remove("dirty");
        }
        let seq: i64 = next_seq.query_row([], |r| r.get(0))?;
        update.execute((
            target.table,
            &target.id,
            &hlc,
            hub_device_id,
            serde_json::to_string(&value)?,
            seq,
        ))?;
    }
    Ok(plan.summary.masked(areas))
}

impl Store {
    /// `GROUP BY` unico para todos os aparelhos: linhas vivas "do aparelho" por tabela das
    /// areas. Serve de pre-filtro barato para a deteccao de orfaos (`devices_data`).
    pub fn device_data_counts(&self) -> Result<BTreeMap<String, Vec<TableCount>>, StoreError> {
        let conn = self.lock();
        Ok(counts_by_device(&conn)?)
    }

    /// Para cada aparelho com linha viva nas areas: o plano de apagar tudo. Um aparelho
    /// revogado, sem "Manter tudo" e com `!is_empty()` e um **orfao**.
    pub fn devices_data(&self) -> Result<HashMap<String, DeviceData>, StoreError> {
        let conn = self.lock();
        let mut out = HashMap::new();
        for device_id in counts_by_device(&conn)?.into_keys() {
            let summary = plan(&conn, &device_id)?.summary;
            out.insert(device_id, summary);
        }
        Ok(out)
    }

    /// Plano de apagar tudo de um aparelho (a contagem que o modal mostra).
    pub fn device_data(&self, device_id: &str) -> Result<DeviceData, StoreError> {
        Ok(plan(&self.lock(), device_id)?.summary)
    }

    /// Apaga as `areas` do aparelho numa transacao propria, sem mexer na revogacao (aparelho
    /// que ja foi removido). `None` se o aparelho nao existe. Idempotente: repetir nao acha
    /// mais linha viva.
    pub fn erase_device_rows(
        &self,
        device_id: &str,
        areas: AreaSet,
    ) -> Result<Option<DeviceData>, StoreError> {
        let mut conn = self.lock();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let exists: bool = tx.query_row(
            "SELECT EXISTS (SELECT 1 FROM devices WHERE device_id = ?1)",
            [device_id],
            |r| r.get(0),
        )?;
        if !exists {
            return Ok(None);
        }
        let report = erase_planned(&tx, device_id, areas, &self.hub_device_id)?;
        tx.commit()?;
        Ok(Some(report))
    }

    /// Remove o aparelho (revoga) e apaga as `erase` areas **na mesma transacao**: sem janela
    /// em que o aparelho empurra entre os dois passos, e falha no apagar mantem o aparelho
    /// ativo. Revoga primeiro (corta o push). `erase` vazio e "Manter tudo": grava
    /// `data_kept_at` para o aviso de aparelho removido nao perguntar de novo. `None` se o
    /// aparelho nao existe.
    pub fn remove_device(
        &self,
        device_id: &str,
        erase: AreaSet,
        now: &str,
    ) -> Result<Option<DeviceData>, StoreError> {
        let mut conn = self.lock();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = tx.execute(
            "UPDATE devices SET revoked_at = COALESCE(revoked_at, ?2),
               data_kept_at = CASE WHEN ?3 THEN COALESCE(data_kept_at, ?2) ELSE data_kept_at END
             WHERE device_id = ?1",
            (device_id, now, erase.is_empty()),
        )?;
        if changed == 0 {
            return Ok(None);
        }
        let report = erase_planned(&tx, device_id, erase, &self.hub_device_id)?;
        tx.commit()?;
        Ok(Some(report))
    }

    /// "Manter tudo" de um aparelho removido. Mantem a primeira data se repetir.
    pub fn keep_device_data(&self, device_id: &str, now: &str) -> Result<bool, StoreError> {
        let conn = self.lock();
        let changed = conn.execute(
            "UPDATE devices SET data_kept_at = COALESCE(data_kept_at, ?2) WHERE device_id = ?1",
            (device_id, now),
        )?;
        Ok(changed > 0)
    }
}

fn counts_by_device(conn: &Connection) -> rusqlite::Result<BTreeMap<String, Vec<TableCount>>> {
    let placeholders = AREA_TABLES
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect::<Vec<_>>()
        .join(", ");
    let mut stmt = conn.prepare(&format!(
        "SELECT d.device_id, r.tbl, COUNT(*) FROM devices d
         JOIN rows r ON r.origin = d.device_id
         WHERE r.tbl IN ({placeholders}) AND {}
         GROUP BY d.device_id, r.tbl ORDER BY d.device_id, r.tbl",
        owned_by("d.device_id")
    ))?;
    let mut out: BTreeMap<String, Vec<TableCount>> = BTreeMap::new();
    let mut rows = stmt.query(rusqlite::params_from_iter(AREA_TABLES))?;
    while let Some(r) = rows.next()? {
        let count: i64 = r.get(2)?;
        out.entry(r.get(0)?).or_default().push(TableCount {
            table: r.get(1)?,
            count: usize::try_from(count).unwrap_or(0),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::protocol::hlc::compare_hlc;
    use crate::protocol::messages::PushEntry;

    const A: &str = "01HZZZZZZZZZZZZZZZZZZZZZZA";
    const B: &str = "01HZZZZZZZZZZZZZZZZZZZZZZB";
    const T1: &str = "2026-10-01T18:00:00Z";
    const T2: &str = "2026-10-02T18:00:00Z";

    fn id(prefix: &str, n: u32) -> String {
        format!("{:Z<21}{prefix}{n:03}", "01H")
    }

    fn hlc(millis: u64, device: &str) -> String {
        format!("{millis:013}-0000-{device}")
    }

    /// Linha enviada por `origin`, carimbada por `stamped_by`.
    fn put(store: &Store, origin: &str, table: &str, row_id: &str, stamped_by: &str, extra: Value) {
        let mut row = json!({
            "id": row_id,
            "createdAt": "2026-10-01T18:00:00.000Z",
            "updatedAt": hlc(1_000, stamped_by),
            "deletedAt": null,
            "dirty": 1,
        });
        if let (Some(row), Some(extra)) = (row.as_object_mut(), extra.as_object()) {
            row.extend(extra.clone());
        }
        let out = store
            .apply_batch(
                origin,
                &[PushEntry {
                    table: table.to_string(),
                    row,
                }],
            )
            .unwrap();
        assert_eq!(out.accepted.len(), 1, "{:?}", out.rejected);
    }

    fn own(store: &Store, table: &str, row_id: &str, extra: Value) {
        put(store, A, table, row_id, A, extra);
    }

    fn store_with_devices() -> Store {
        let store = Store::open_in_memory().unwrap();
        store
            .upsert_device(A, "Pixel da Ana", &"a".repeat(64), None, T1)
            .unwrap();
        store
            .upsert_device(B, "iPhone do Bia", &"b".repeat(64), None, T1)
            .unwrap();
        store
    }

    struct Stored {
        updated_at: String,
        deleted_at: Option<String>,
        origin: String,
        data: Value,
        seq: i64,
    }

    fn stored(store: &Store, table: &str, row_id: &str) -> Stored {
        store
            .lock()
            .query_row(
                "SELECT updated_at, deleted_at, origin, data, seq FROM rows WHERE tbl = ?1 AND id = ?2",
                (table, row_id),
                |r| {
                    let data: String = r.get(3)?;
                    Ok(Stored {
                        updated_at: r.get(0)?,
                        deleted_at: r.get(1)?,
                        origin: r.get(2)?,
                        data: serde_json::from_str(&data).unwrap(),
                        seq: r.get(4)?,
                    })
                },
            )
            .unwrap()
    }

    fn alive(store: &Store, table: &str, row_id: &str) -> bool {
        stored(store, table, row_id).deleted_at.is_none()
    }

    fn only(area: Area) -> AreaSet {
        [area].into_iter().collect()
    }

    /// Um aparelho A com uma linha em cada tabela das areas e as do catalogo.
    fn seeded() -> Store {
        let store = store_with_devices();
        own(
            &store,
            "budgets",
            &id("BG", 1),
            json!({ "categoryId": id("CA", 1) }),
        );
        own(
            &store,
            "transactions",
            &id("TX", 1),
            json!({ "reportState": null }),
        );
        own(
            &store,
            "transactions",
            &id("TX", 2),
            json!({ "reportState": "transfer" }),
        );
        own(
            &store,
            "reserves",
            &id("RE", 1),
            json!({ "name": "Emergencia" }),
        );
        own(
            &store,
            "reserveMovements",
            &id("MV", 1),
            json!({ "reserveId": id("RE", 1) }),
        );
        own(&store, "recurrences", &id("RC", 1), json!({}));
        own(
            &store,
            "recurrenceAdjustments",
            &id("AD", 1),
            json!({ "recurrenceId": id("RC", 1) }),
        );
        own(
            &store,
            "categories",
            &id("CA", 1),
            json!({ "name": "Mercado" }),
        );
        own(
            &store,
            "paymentMethods",
            &id("PM", 1),
            json!({ "name": "Cartao" }),
        );
        own(&store, "users", &id("SR", 1), json!({ "name": "Ana" }));
        store
    }

    const CATALOG: [(&str, &str); 3] = [
        ("categories", "CA"),
        ("paymentMethods", "PM"),
        ("users", "SR"),
    ];

    fn assert_catalog_untouched(store: &Store) {
        for (table, prefix) in CATALOG {
            let row = stored(store, table, &id(prefix, 1));
            assert!(row.deleted_at.is_none(), "{table}");
            assert_eq!(row.origin, A, "{table}");
        }
    }

    // --- contagem ---

    #[test]
    fn contagem_agrupa_por_aparelho_e_tabela_e_aplica_o_predicado() {
        let store = seeded();
        // Reenviada pelo B (origin B, sufixo A), editada pelo B (origin e sufixo B) e apagada.
        put(&store, B, "transactions", &id("TX", 10), A, json!({}));
        put(&store, B, "transactions", &id("TX", 11), B, json!({}));
        own(&store, "transactions", &id("TX", 12), json!({}));
        let deleted = PushEntry {
            table: "transactions".into(),
            row: json!({
                "id": id("TX", 12), "createdAt": "2026-10-01T18:00:00.000Z",
                "updatedAt": hlc(2_000, A), "deletedAt": hlc(2_000, A), "dirty": 1
            }),
        };
        store.apply_batch(A, &[deleted]).unwrap();

        let counts = store.device_data_counts().unwrap();
        let of_a: BTreeMap<&str, usize> = counts[A]
            .iter()
            .map(|c| (c.table.as_str(), c.count))
            .collect();
        assert_eq!(
            of_a,
            BTreeMap::from([
                ("budgets", 1),
                ("transactions", 2),
                ("reserves", 1),
                ("reserveMovements", 1),
                ("recurrences", 1),
                ("recurrenceAdjustments", 1),
            ]),
            "catalogo e perfil nao entram; reenviada, editada por outro e apagada tambem nao"
        );
        let of_b: BTreeMap<&str, usize> = counts[B]
            .iter()
            .map(|c| (c.table.as_str(), c.count))
            .collect();
        assert_eq!(of_b, BTreeMap::from([("transactions", 1)]));
    }

    #[test]
    fn contagem_do_plano_e_igual_ao_que_a_escrita_apaga() {
        let store = seeded();
        own(
            &store,
            "transactions",
            &id("TX", 3),
            json!({ "recurrenceId": id("RC", 1) }),
        );
        let before = store.device_data(A).unwrap();
        assert_eq!(before.budgets, 1);
        assert_eq!(before.transactions, 3);
        assert_eq!(before.internal_transfers, 1);
        assert_eq!((before.reserves, before.reserve_movements), (1, 1));
        assert_eq!((before.recurrences, before.adjustments), (1, 1));
        assert_eq!(before.generated, 1);
        assert_eq!(before.total_items(AreaSet::all()), 1 + 3 + 2 + 2);

        let erased = store.erase_device_rows(A, AreaSet::all()).unwrap().unwrap();
        assert_eq!(erased, before);
        assert!(store.device_data(A).unwrap().is_empty());
    }

    // --- escrita por area ---

    #[test]
    fn cada_area_apaga_so_as_suas_tabelas() {
        let tables = |area: Area| -> Vec<(&'static str, &'static str)> {
            match area {
                Area::Budgets => vec![("budgets", "BG")],
                Area::Transactions => vec![("transactions", "TX")],
                Area::Reserves => vec![("reserves", "RE"), ("reserveMovements", "MV")],
                Area::Recurrences => vec![("recurrences", "RC"), ("recurrenceAdjustments", "AD")],
            }
        };
        for area in Area::ALL {
            let store = seeded();
            store.erase_device_rows(A, only(area)).unwrap().unwrap();
            for other in Area::ALL {
                for (table, prefix) in tables(other) {
                    let expected_dead = other == area;
                    for n in 1..=2 {
                        if table == "transactions" || n == 1 {
                            assert_eq!(
                                !alive(&store, table, &id(prefix, n)),
                                expected_dead,
                                "{area:?} apagando, {table} {n}"
                            );
                        }
                    }
                }
            }
            assert_catalog_untouched(&store);
        }
    }

    #[test]
    fn tombstone_tem_hlc_maior_origin_do_hub_seq_crescente_e_data_intacto() {
        let store = seeded();
        let hub = store.hub_device_id().to_string();
        let before = stored(&store, "transactions", &id("TX", 1));
        let max_before = store.max_seq().unwrap();

        store
            .erase_device_rows(A, only(Area::Transactions))
            .unwrap();

        let after = stored(&store, "transactions", &id("TX", 1));
        assert_eq!(
            compare_hlc(&after.updated_at, &before.updated_at),
            std::cmp::Ordering::Greater
        );
        assert_eq!(after.deleted_at.as_deref(), Some(after.updated_at.as_str()));
        assert!(after.updated_at.ends_with(&hub));
        assert_eq!(after.origin, hub);
        assert_ne!(after.origin, A);
        assert_eq!(after.data["updatedAt"], json!(after.updated_at));
        assert_eq!(after.data["deletedAt"], json!(after.updated_at));
        assert!(after.data.get("dirty").is_none());
        assert_eq!(after.data["createdAt"], before.data["createdAt"]);
        let seqs = [
            stored(&store, "transactions", &id("TX", 1)).seq,
            stored(&store, "transactions", &id("TX", 2)).seq,
        ];
        assert!(seqs[0] > max_before && seqs[1] > seqs[0], "{seqs:?}");
        // O outro celular recebe o tombstone no pull; quem o enviou tambem receberia (origin do hub).
        let page = store.pull_after(0, B, 100).unwrap();
        assert!(
            page.rows
                .iter()
                .any(|r| r.row["deletedAt"] == json!(after.updated_at))
        );
    }

    #[test]
    fn nao_toca_linha_reenviada_nem_editada_por_outro_aparelho() {
        let store = store_with_devices();
        // Reenviada pelo A (epoch_mismatch): origin A, sufixo B.
        put(&store, A, "budgets", &id("BG", 1), B, json!({}));
        // Criada pelo A e editada depois pelo B: origin e sufixo B.
        own(&store, "budgets", &id("BG", 2), json!({}));
        let edited = PushEntry {
            table: "budgets".into(),
            row: json!({
                "id": id("BG", 2), "createdAt": "2026-10-01T18:00:00.000Z",
                "updatedAt": hlc(3_000, B), "deletedAt": null, "dirty": 1
            }),
        };
        store.apply_batch(B, &[edited]).unwrap();
        // Do A de verdade.
        own(&store, "budgets", &id("BG", 3), json!({}));

        let report = store.erase_device_rows(A, AreaSet::all()).unwrap().unwrap();
        assert_eq!(report.budgets, 1);
        assert!(alive(&store, "budgets", &id("BG", 1)));
        assert!(alive(&store, "budgets", &id("BG", 2)));
        assert!(!alive(&store, "budgets", &id("BG", 3)));
    }

    #[test]
    fn apagar_e_idempotente() {
        let store = seeded();
        let first = store.erase_device_rows(A, AreaSet::all()).unwrap().unwrap();
        assert!(!first.is_empty());
        let seq = store.max_seq().unwrap();
        let second = store.erase_device_rows(A, AreaSet::all()).unwrap().unwrap();
        assert!(second.is_empty());
        assert_eq!(store.max_seq().unwrap(), seq);
    }

    #[test]
    fn manter_tudo_e_conjunto_vazio_nao_altera_rows_nem_seq() {
        let store = seeded();
        let seq = store.max_seq().unwrap();
        let report = store
            .erase_device_rows(A, AreaSet::default())
            .unwrap()
            .unwrap();
        assert!(report.is_empty());
        assert_eq!(store.max_seq().unwrap(), seq);
        assert!(alive(&store, "transactions", &id("TX", 1)));
        assert_eq!(
            store
                .erase_device_rows("inexistente", AreaSet::all())
                .unwrap(),
            None
        );
    }

    #[test]
    fn apagar_tudo_e_a_uniao_das_quatro_areas() {
        let store = seeded();
        let all = store.erase_device_rows(A, AreaSet::all()).unwrap().unwrap();
        let store2 = seeded();
        let mut sum = 0;
        for area in Area::ALL {
            sum += store2
                .erase_device_rows(A, only(area))
                .unwrap()
                .unwrap()
                .items(area);
        }
        assert_eq!(all.total_items(AreaSet::all()), sum);
        assert_catalog_untouched(&store);
    }

    // --- dependencias ---

    #[test]
    fn apagar_recorrencia_leva_os_reajustes_de_qualquer_origem_e_nao_toca_ocorrencias() {
        let store = seeded();
        // Reajuste da serie do A escrito pelo B, e uma ocorrencia do A materializada.
        put(
            &store,
            B,
            "recurrenceAdjustments",
            &id("AD", 2),
            B,
            json!({ "recurrenceId": id("RC", 1) }),
        );
        own(
            &store,
            "transactions",
            &id("TX", 3),
            json!({ "recurrenceId": id("RC", 1) }),
        );

        let report = store
            .erase_device_rows(A, only(Area::Recurrences))
            .unwrap()
            .unwrap();
        assert_eq!(
            (
                report.recurrences,
                report.adjustments,
                report.adjustments_other
            ),
            (1, 2, 1)
        );
        assert_eq!(report.generated, 1);
        assert!(!alive(&store, "recurrenceAdjustments", &id("AD", 2)));
        assert!(!alive(&store, "recurrences", &id("RC", 1)));
        assert!(
            alive(&store, "transactions", &id("TX", 3)),
            "ocorrencia materializada continua"
        );

        // Com Transferencias marcada, as do aparelho vao junto.
        store
            .erase_device_rows(A, only(Area::Transactions))
            .unwrap();
        assert!(!alive(&store, "transactions", &id("TX", 3)));
    }

    #[test]
    fn reajuste_de_serie_de_outro_aparelho_e_mantido_se_a_serie_fica() {
        let store = store_with_devices();
        put(&store, B, "recurrences", &id("RC", 5), B, json!({}));
        put(
            &store,
            B,
            "recurrenceAdjustments",
            &id("AD", 5),
            B,
            json!({ "recurrenceId": id("RC", 5) }),
        );
        own(
            &store,
            "recurrenceAdjustments",
            &id("AD", 6),
            json!({ "recurrenceId": id("RC", 5) }),
        );
        let report = store
            .erase_device_rows(A, only(Area::Recurrences))
            .unwrap()
            .unwrap();
        // So o reajuste do proprio A cai (para a contagem da area chegar a zero).
        assert_eq!((report.recurrences, report.adjustments), (0, 1));
        assert!(alive(&store, "recurrenceAdjustments", &id("AD", 5)));
        assert!(alive(&store, "recurrences", &id("RC", 5)));
    }

    #[test]
    fn reserva_so_do_aparelho_cai_inteira_com_os_movimentos() {
        let store = seeded();
        let report = store
            .erase_device_rows(A, only(Area::Reserves))
            .unwrap()
            .unwrap();
        assert_eq!(
            (
                report.reserves,
                report.reserve_movements,
                report.reserves_kept
            ),
            (1, 1, 0)
        );
        assert!(!alive(&store, "reserves", &id("RE", 1)));
        assert!(!alive(&store, "reserveMovements", &id("MV", 1)));
    }

    #[test]
    fn reserva_com_movimento_de_outro_aparelho_e_mantida_e_so_os_movimentos_do_aparelho_caem() {
        let store = seeded();
        put(
            &store,
            B,
            "reserveMovements",
            &id("MV", 2),
            B,
            json!({ "reserveId": id("RE", 1) }),
        );
        let report = store
            .erase_device_rows(A, only(Area::Reserves))
            .unwrap()
            .unwrap();
        assert_eq!(
            (
                report.reserves,
                report.reserve_movements,
                report.reserves_kept
            ),
            (0, 1, 1)
        );
        assert!(alive(&store, "reserves", &id("RE", 1)));
        assert!(!alive(&store, "reserveMovements", &id("MV", 1)));
        assert!(alive(&store, "reserveMovements", &id("MV", 2)));
        // Repetir: a reserva segue mantida e nada mais cai.
        let again = store
            .erase_device_rows(A, only(Area::Reserves))
            .unwrap()
            .unwrap();
        assert_eq!(again.reserve_movements, 0);
        assert_eq!(again.reserves, 0);
        assert!(alive(&store, "reserves", &id("RE", 1)));
    }

    #[test]
    fn nunca_sobra_movimento_vivo_de_reserva_apagada() {
        let store = seeded();
        // Movimentos do A em reserva de outro aparelho: caem, a reserva do B fica.
        put(&store, B, "reserves", &id("RE", 2), B, json!({}));
        own(
            &store,
            "reserveMovements",
            &id("MV", 3),
            json!({ "reserveId": id("RE", 2) }),
        );
        store.erase_device_rows(A, AreaSet::all()).unwrap();
        let conn = store.lock();
        let dangling: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM rows m JOIN rows r
                   ON r.tbl = 'reserves' AND r.id = json_extract(m.data, '$.reserveId')
                 WHERE m.tbl = 'reserveMovements' AND m.deleted_at IS NULL
                   AND r.deleted_at IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(dangling, 0);
    }

    #[test]
    fn apagar_transferencias_sozinha_nao_toca_recorrencia_nem_reserva() {
        let store = seeded();
        store
            .erase_device_rows(A, only(Area::Transactions))
            .unwrap();
        assert!(alive(&store, "recurrences", &id("RC", 1)));
        assert!(alive(&store, "reserves", &id("RE", 1)));
        assert!(alive(&store, "reserveMovements", &id("MV", 1)));
    }

    #[test]
    fn duas_metas_da_mesma_categoria_sao_avaliadas_isoladamente() {
        let store = store_with_devices();
        own(
            &store,
            "budgets",
            &id("BG", 1),
            json!({ "categoryId": id("CA", 1) }),
        );
        put(
            &store,
            B,
            "budgets",
            &id("BG", 2),
            B,
            json!({ "categoryId": id("CA", 1) }),
        );
        let report = store
            .erase_device_rows(A, only(Area::Budgets))
            .unwrap()
            .unwrap();
        assert_eq!(report.budgets, 1);
        assert!(!alive(&store, "budgets", &id("BG", 1)));
        assert!(alive(&store, "budgets", &id("BG", 2)));
    }

    // --- orfaos ---

    #[test]
    fn reserva_mantida_nao_deixa_o_aparelho_orfao_para_sempre() {
        let store = seeded();
        put(
            &store,
            B,
            "reserveMovements",
            &id("MV", 2),
            B,
            json!({ "reserveId": id("RE", 1) }),
        );
        store.erase_device_rows(A, AreaSet::all()).unwrap();
        // A contagem bruta ainda ve a reserva, o plano nao tem mais nada a apagar.
        assert!(store.device_data_counts().unwrap().contains_key(A));
        assert!(store.devices_data().unwrap()[A].is_empty());
    }

    #[test]
    fn devices_data_so_traz_quem_tem_linha_viva() {
        let store = seeded();
        let data = store.devices_data().unwrap();
        assert!(data.contains_key(A));
        assert!(!data.contains_key(B));
    }

    // --- remover + apagar ---

    #[test]
    fn remover_e_apagar_na_mesma_transacao_revoga_e_apaga() {
        let store = seeded();
        let report = store
            .remove_device(A, only(Area::Budgets), T2)
            .unwrap()
            .unwrap();
        assert_eq!(report.budgets, 1);
        let device = store
            .list_devices()
            .unwrap()
            .into_iter()
            .find(|d| d.device_id == A)
            .unwrap();
        assert_eq!(device.revoked_at.as_deref(), Some(T2));
        // Apagou so uma area: nao e "Manter tudo", as outras seguem pendentes.
        assert_eq!(device.data_kept_at, None);
        assert!(!alive(&store, "budgets", &id("BG", 1)));
        assert!(alive(&store, "transactions", &id("TX", 1)));
        // Idempotente.
        let again = store
            .remove_device(A, only(Area::Budgets), "2026-10-03T00:00:00Z")
            .unwrap()
            .unwrap();
        assert!(again.is_empty());
        let device = store
            .list_devices()
            .unwrap()
            .into_iter()
            .find(|d| d.device_id == A)
            .unwrap();
        assert_eq!(device.revoked_at.as_deref(), Some(T2));
    }

    #[test]
    fn remover_sem_marcar_nada_mantem_tudo_e_grava_a_decisao() {
        let store = seeded();
        let seq = store.max_seq().unwrap();
        let report = store
            .remove_device(A, AreaSet::default(), T2)
            .unwrap()
            .unwrap();
        assert!(report.is_empty());
        assert_eq!(store.max_seq().unwrap(), seq);
        let device = store
            .list_devices()
            .unwrap()
            .into_iter()
            .find(|d| d.device_id == A)
            .unwrap();
        assert_eq!(device.revoked_at.as_deref(), Some(T2));
        assert_eq!(device.data_kept_at.as_deref(), Some(T2));
        assert_eq!(
            store
                .remove_device("inexistente", AreaSet::all(), T2)
                .unwrap(),
            None
        );
    }

    #[test]
    fn falha_no_apagar_desfaz_a_revogacao_e_todo_o_resto() {
        let store = seeded();
        // O segundo tombstone falha: nada do primeiro nem a revogacao podem ficar.
        let trigger = format!(
            "CREATE TRIGGER boom BEFORE UPDATE ON rows WHEN NEW.id = '{}' \
             BEGIN SELECT RAISE(ABORT, 'boom'); END;",
            id("TX", 2)
        );
        store.lock().execute_batch(&trigger).unwrap();
        let seq = store.max_seq().unwrap();
        assert!(store.remove_device(A, AreaSet::all(), T2).is_err());
        let device = store
            .list_devices()
            .unwrap()
            .into_iter()
            .find(|d| d.device_id == A)
            .unwrap();
        assert_eq!(
            device.revoked_at, None,
            "falha no apagar mantem o aparelho ativo"
        );
        assert_eq!(store.max_seq().unwrap(), seq);
        for (table, prefix) in [
            ("budgets", "BG"),
            ("transactions", "TX"),
            ("reserves", "RE"),
        ] {
            assert!(alive(&store, table, &id(prefix, 1)), "{table}");
        }
    }

    #[test]
    fn manter_grava_e_nao_sobrescreve_e_reparear_limpa() {
        let store = seeded();
        store.remove_device(A, only(Area::Budgets), T1).unwrap();
        assert!(store.keep_device_data(A, T1).unwrap());
        assert!(store.keep_device_data(A, T2).unwrap());
        let kept = |s: &Store| {
            s.list_devices()
                .unwrap()
                .into_iter()
                .find(|d| d.device_id == A)
                .unwrap()
                .data_kept_at
        };
        assert_eq!(kept(&store).as_deref(), Some(T1));
        assert!(!store.keep_device_data("inexistente", T1).unwrap());
        store
            .upsert_device(A, "Pixel", &"c".repeat(64), None, T2)
            .unwrap();
        assert_eq!(kept(&store), None);
    }

    #[test]
    fn apagar_dado_de_aparelho_removido_nao_mexe_na_revogacao() {
        let store = seeded();
        store.revoke_device(A, T1).unwrap();
        store.erase_device_rows(A, only(Area::Budgets)).unwrap();
        let device = store
            .list_devices()
            .unwrap()
            .into_iter()
            .find(|d| d.device_id == A)
            .unwrap();
        assert_eq!(device.revoked_at.as_deref(), Some(T1));
    }
}
