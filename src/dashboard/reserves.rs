//! Reservas: saldos, series, totais, custo essencial, meta da emergencia, ritmo das caixinhas
//! e a lista de movimentacoes, tudo puro e a partir do `Dataset`.
//!
//! Movimentacao de reserva nunca entra em receita nem despesa: `aggregate.rs` e `list.rs` so
//! leem `transactions`, e o teste `reservas_nao_entram_no_dashboard_nem_nos_lancamentos` de
//! `view.rs` prova isso. A unica leitura de `transactions` aqui e o custo essencial.

use super::contract::{MovementKind, ReserveKind};
use super::dataset::{Dataset, Reserve, ReserveMovement};
use super::periods::month_of;

/// Passo do teto do eixo do grafico: R$ 6.000 (`Math.ceil(max / 6000) * 6000` do prototipo).
pub const RESERVE_AXIS_STEP: i64 = 600_000;

/// Cor de uma reserva na tela: a emergencia usa sempre o acento do tema; caixinha usa o token
/// da linha (desconhecido vira neutro na UI).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReserveColor {
    Accent,
    Token(String),
}

pub fn reserve_color(r: &Reserve) -> ReserveColor {
    match r.kind {
        ReserveKind::Emergency => ReserveColor::Accent,
        ReserveKind::Pot => ReserveColor::Token(r.color.clone()),
    }
}

/// Chave de icone: a emergencia usa sempre `lifebuoy` (chave interna da UI, nao do app); a
/// caixinha usa a chave que o app gravou.
pub fn reserve_icon(r: &Reserve) -> &str {
    match r.kind {
        ReserveKind::Emergency => "lifebuoy",
        ReserveKind::Pot => &r.icon,
    }
}

/// (emergencia do card, demais vivas por id). A emergencia do card e a viva de menor id: dois
/// celulares podem criar uma cada antes de sincronizar (LWW nao deduplica), e o menor ULID e o
/// mesmo em todo aparelho. As outras emergencias vao para a lista junto das caixinhas, na
/// ordem de id (comparacao de string, como todo id do app).
pub fn ordered(dataset: &Dataset) -> (Option<&Reserve>, Vec<&Reserve>) {
    let mut alive: Vec<&Reserve> = dataset.alive_reserves().collect();
    alive.sort_by(|a, b| a.id.cmp(&b.id));
    let emergency = alive
        .iter()
        .position(|r| r.kind == ReserveKind::Emergency)
        .map(|i| alive.remove(i));
    (emergency, alive)
}

/// Deposito soma, retirada subtrai.
fn signed(m: &ReserveMovement) -> i64 {
    match m.kind {
        MovementKind::Deposit => m.amount_minor,
        MovementKind::Withdrawal => m.amount_minor.saturating_neg(),
    }
}

/// Saldo no fim de `month`: movimentacoes visiveis da reserva com mes `<= month` (comparacao
/// de string `YYYY-MM`). Mes futuro fica fora porque saldo atual e "saldo no fim do mes
/// corrente" — a ultima coluna do grafico e o card Total contam a mesma coisa; uma linha
/// agendada entra quando o mes chegar.
pub fn balance_until(dataset: &Dataset, reserve_id: &str, month: &str) -> i64 {
    dataset
        .alive_reserve_movements()
        .filter(|m| m.reserve_id == reserve_id && month_of(&m.occurred_on).as_str() <= month)
        .fold(0i64, |acc, m| acc.saturating_add(signed(m)))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReserveSeries {
    pub id: String,
    pub name: String,
    pub color: ReserveColor,
    /// Saldo no fim de cada mes da janela, o mais antigo primeiro.
    pub balances: Vec<i64>,
}

/// Uma serie por reserva viva, na ordem padrao (emergencia do card primeiro).
pub fn series(dataset: &Dataset, months: &[String]) -> Vec<ReserveSeries> {
    let (emergency, rest) = ordered(dataset);
    emergency
        .into_iter()
        .chain(rest)
        .map(|r| ReserveSeries {
            id: r.id.clone(),
            name: r.name.clone(),
            color: reserve_color(r),
            balances: months
                .iter()
                .map(|m| balance_until(dataset, &r.id, m))
                .collect(),
        })
        .collect()
}

/// Teto do eixo: multiplos de R$ 6.000, no minimo R$ 6.000 (grafico zerado ainda tem eixo).
pub fn reserve_axis_top(peak_minor: i64) -> i64 {
    if peak_minor <= RESERVE_AXIS_STEP {
        return RESERVE_AXIS_STEP;
    }
    let steps = peak_minor.saturating_add(RESERVE_AXIS_STEP - 1) / RESERVE_AXIS_STEP;
    steps.saturating_mul(RESERVE_AXIS_STEP)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReserveTotals {
    /// Soma dos saldos atuais das reservas vivas.
    pub total_minor: i64,
    /// Depositos no mes corrente.
    pub saved_this_month_minor: i64,
    /// Depositos e retiradas na janela de 12 meses.
    pub deposits_minor: i64,
    pub withdrawals_minor: i64,
    pub withdrawal_count: usize,
    /// Nomes das reservas com retirada na janela, na ordem padrao.
    pub withdrawal_reserves: Vec<String>,
}

/// `months` e a janela; o ultimo e o mes corrente.
pub fn totals(dataset: &Dataset, months: &[String]) -> ReserveTotals {
    let Some(current) = months.last() else {
        return ReserveTotals::default();
    };
    let (emergency, rest) = ordered(dataset);
    let order: Vec<&Reserve> = emergency.into_iter().chain(rest).collect();
    let mut t = ReserveTotals {
        total_minor: order.iter().fold(0i64, |acc, r| {
            acc.saturating_add(balance_until(dataset, &r.id, current))
        }),
        ..ReserveTotals::default()
    };
    let mut withdrew: Vec<&str> = Vec::new();
    for m in dataset.alive_reserve_movements() {
        let month = month_of(&m.occurred_on);
        if !months.contains(&month) {
            continue;
        }
        match m.kind {
            MovementKind::Deposit => {
                t.deposits_minor = t.deposits_minor.saturating_add(m.amount_minor);
                if &month == current {
                    t.saved_this_month_minor =
                        t.saved_this_month_minor.saturating_add(m.amount_minor);
                }
            }
            MovementKind::Withdrawal => {
                t.withdrawals_minor = t.withdrawals_minor.saturating_add(m.amount_minor);
                t.withdrawal_count += 1;
                withdrew.push(&m.reserve_id);
            }
        }
    }
    t.withdrawal_reserves = order
        .iter()
        .filter(|r| withdrew.contains(&r.id.as_str()))
        .map(|r| r.name.clone())
        .collect();
    t
}

/// Nota do card "Últimos 12 meses". O texto do prototipo ("todas da reserva de emergência") e
/// dado de exemplo; a regra cobre 0, 1 e N reservas.
pub fn withdrawals_note(t: &ReserveTotals) -> String {
    match (t.withdrawal_count, t.withdrawal_reserves.as_slice()) {
        (0, _) => "Nenhuma retirada".to_string(),
        (1, [name, ..]) => format!("1 retirada, da {name}"),
        (n, [name]) => format!("{n} retiradas, todas da {name}"),
        (n, names) => format!("{n} retiradas de {} reservas", names.len()),
    }
}

/// Um segmento da barra dividida do card Total.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReserveShare {
    pub id: String,
    pub name: String,
    pub color: ReserveColor,
    pub balance_minor: i64,
}

/// So saldo > 0, na ordem padrao.
pub fn shares(dataset: &Dataset, current: &str) -> Vec<ReserveShare> {
    let (emergency, rest) = ordered(dataset);
    emergency
        .into_iter()
        .chain(rest)
        .filter_map(|r| {
            let balance = balance_until(dataset, &r.id, current);
            (balance > 0).then(|| ReserveShare {
                id: r.id.clone(),
                name: r.name.clone(),
                color: reserve_color(r),
                balance_minor: balance,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::dashboard::dataset::{RESERVE_MOVEMENTS, RESERVES, RawRow};
    use crate::dashboard::fixtures_with_reserves;
    use crate::dashboard::periods::month_window;

    pub(super) fn add_reserve(ds: &mut Dataset, id: &str, data: serde_json::Value) {
        let seq = ds.loaded_seq + 1;
        ds.apply([RawRow {
            table: RESERVES.into(),
            id: id.into(),
            deleted_at: None,
            seq,
            data: data.to_string(),
        }]);
    }

    pub(super) fn add_movement(ds: &mut Dataset, id: &str, data: serde_json::Value) {
        let seq = ds.loaded_seq + 1;
        ds.apply([RawRow {
            table: RESERVE_MOVEMENTS.into(),
            id: id.into(),
            deleted_at: None,
            seq,
            data: data.to_string(),
        }]);
    }

    fn months() -> Vec<String> {
        month_window("2026-09-24")
    }

    fn ids<'a>(rs: impl IntoIterator<Item = &'a Reserve>) -> Vec<&'a str> {
        rs.into_iter().map(|r| r.id.as_str()).collect()
    }

    #[test]
    fn ordem_padrao() {
        let ds = fixtures_with_reserves();
        let (e, rest) = ordered(&ds);
        assert_eq!(e.map(|r| r.id.as_str()), Some("RE1"));
        assert_eq!(ids(rest), vec!["RP1", "RP2", "RP3", "RP4", "RP6"]);
        let empty = Dataset::default();
        let (e, rest) = ordered(&empty);
        assert!(e.is_none() && rest.is_empty());
    }

    #[test]
    fn duas_emergencias_a_de_menor_id_e_o_card() {
        let mut ds = fixtures_with_reserves();
        add_reserve(
            &mut ds,
            "RE0",
            json!({ "kind": "emergency", "name": "Outra", "icon": "x", "color": "slate" }),
        );
        let (e, rest) = ordered(&ds);
        assert_eq!(e.map(|r| r.id.as_str()), Some("RE0"));
        // A outra emergencia vai para a lista, por id como string: "RE1" < "RP1".
        assert_eq!(ids(rest), vec!["RE1", "RP1", "RP2", "RP3", "RP4", "RP6"]);
    }

    #[test]
    fn cor_e_icone() {
        let ds = fixtures_with_reserves();
        assert_eq!(reserve_color(&ds.reserves["RE1"]), ReserveColor::Accent);
        assert_eq!(
            reserve_color(&ds.reserves["RP1"]),
            ReserveColor::Token("sky".into())
        );
        assert_eq!(reserve_icon(&ds.reserves["RE1"]), "lifebuoy");
        assert_eq!(reserve_icon(&ds.reserves["RP1"]), "plane");
    }

    #[test]
    fn saldo_ate_o_mes() {
        let ds = fixtures_with_reserves();
        assert_eq!(balance_until(&ds, "RE1", "2026-07"), 50_000);
        assert_eq!(balance_until(&ds, "RE1", "2026-08"), 80_000);
        // Futuro (M06) e apagada (M07) fora.
        assert_eq!(balance_until(&ds, "RE1", "2026-09"), 230_000);
        assert_eq!(balance_until(&ds, "RP1", "2026-09"), 235_000);
        assert_eq!(balance_until(&ds, "RP5", "2026-09"), 0);
        assert_eq!(balance_until(&ds, "RX", "2026-09"), 0);
        assert_eq!(balance_until(&ds, "RE1", "2025-12"), 0);
    }

    #[test]
    fn series_na_ordem_padrao() {
        let ds = fixtures_with_reserves();
        let s = series(&ds, &months());
        assert_eq!(s.len(), 6);
        assert!(s.iter().all(|x| x.balances.len() == 12));
        assert_eq!(s[0].id, "RE1");
        assert_eq!(
            s[0].balances,
            vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 50_000, 80_000, 230_000]
        );
        let per_month: Vec<i64> = (0..12)
            .map(|i| s.iter().map(|x| x.balances[i]).sum())
            .collect();
        assert_eq!(
            per_month,
            vec![
                0, 0, 0, 0, 0, 0, 0, 50_000, 60_000, 290_000, 370_000, 603_000
            ]
        );
    }

    #[test]
    fn teto_do_eixo() {
        assert_eq!(reserve_axis_top(0), 600_000);
        assert_eq!(reserve_axis_top(600_000), 600_000);
        assert_eq!(reserve_axis_top(600_001), 1_200_000);
        assert_eq!(reserve_axis_top(603_000), 1_200_000);
        assert_eq!(reserve_axis_top(2_400_000), 2_400_000);
    }

    #[test]
    fn totais_da_janela() {
        let ds = fixtures_with_reserves();
        let t = totals(&ds, &months());
        assert_eq!(t.total_minor, 603_000);
        assert_eq!(t.saved_this_month_minor, 238_000);
        assert_eq!(t.deposits_minor, 628_000);
        assert_eq!(t.withdrawals_minor, 25_000);
        assert_eq!(t.withdrawal_count, 2);
        assert_eq!(
            t.withdrawal_reserves,
            vec!["Reserva de emergência", "Viagem de julho"]
        );
    }

    #[test]
    fn nota_das_retiradas() {
        let ds = fixtures_with_reserves();
        assert_eq!(
            withdrawals_note(&totals(&ds, &months())),
            "2 retiradas de 2 reservas"
        );
        assert_eq!(
            withdrawals_note(&ReserveTotals::default()),
            "Nenhuma retirada"
        );
        let one = ReserveTotals {
            withdrawal_count: 1,
            withdrawal_reserves: vec!["Viagem de julho".into()],
            ..ReserveTotals::default()
        };
        assert_eq!(withdrawals_note(&one), "1 retirada, da Viagem de julho");
        let three = ReserveTotals {
            withdrawal_count: 3,
            withdrawal_reserves: vec!["Reserva de emergência".into()],
            ..ReserveTotals::default()
        };
        assert_eq!(
            withdrawals_note(&three),
            "3 retiradas, todas da Reserva de emergência"
        );
    }

    #[test]
    fn fatias_do_total() {
        let mut ds = fixtures_with_reserves();
        let s = shares(&ds, "2026-09");
        let got: Vec<(&str, i64)> = s.iter().map(|x| (x.id.as_str(), x.balance_minor)).collect();
        assert_eq!(
            got,
            vec![
                ("RE1", 230_000),
                ("RP1", 235_000),
                ("RP2", 40_000),
                ("RP3", 38_000),
                ("RP4", 10_000),
                ("RP6", 50_000)
            ]
        );
        // Zerada fica fora.
        add_movement(
            &mut ds,
            "MZ",
            json!({ "reserveId": "RP4", "kind": "withdrawal", "amountMinor": 10000, "occurredOn": "2026-09-01" }),
        );
        assert_eq!(shares(&ds, "2026-09").len(), 5);
    }
}
