//! Reservas: saldos, series, totais, custo essencial, meta da emergencia, ritmo das caixinhas
//! e a lista de movimentacoes, tudo puro e a partir do `Dataset`.
//!
//! Movimentacao de reserva nunca entra em receita nem despesa: `aggregate.rs` e `list.rs` so
//! leem `transactions`, e o teste `reservas_nao_entram_no_dashboard_nem_nos_lancamentos` de
//! `view.rs` prova isso. A unica leitura de `transactions` aqui e o custo essencial.

use super::aggregate::js_round;
use super::contract::{Kind, MovementKind, ReserveKind};
use super::dataset::{Dataset, Reserve, ReserveMovement};
use super::money::{format_brl, one_decimal, whole_brl};
use super::periods::{last_months, month_of, month_short_year, shift_month};

/// Passo do teto do eixo do grafico: R$ 6.000 (`Math.ceil(max / 6000) * 6000` do prototipo).
pub const RESERVE_AXIS_STEP: i64 = 600_000;
/// Multiplo da meta da emergencia quando a linha nao diz (o padrao do app).
pub const DEFAULT_MULTIPLE: u8 = 6;
/// Meses completos da media do custo essencial.
pub const ESSENTIAL_MONTHS: usize = 6;

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

/// Divisao inteira com meio para cima (`Math.round` do JS), `den >= 1`. Inteiro porque a
/// media vira dinheiro: o app e o hub tem de chegar no mesmo centavo.
pub(crate) fn div_round(sum: i64, den: i64) -> i64 {
    let den = den.max(1);
    sum.saturating_mul(2)
        .saturating_add(den)
        .div_euclid(den.saturating_mul(2))
}

/// Divisao inteira arredondada para cima, `den >= 1` ("precisa guardar por mes").
pub(crate) fn div_ceil(num: i64, den: i64) -> i64 {
    let den = den.max(1);
    num.saturating_neg().div_euclid(den).saturating_neg()
}

/// Uma linha da legenda do custo essencial.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EssentialPart {
    pub category_id: String,
    /// Nome da categoria viva, ou "Categoria removida".
    pub name: String,
    /// Token de cor; `slate` para categoria apagada ou ausente.
    pub color: String,
    pub average_minor: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EssentialCost {
    /// Soma das medias arredondadas: assim a legenda fecha no total que o card mostra.
    pub total_minor: i64,
    /// Denominador da media: meses da janela com historico.
    pub months: usize,
    pub parts: Vec<EssentialPart>,
}

/// Custo essencial medio por mes. Janela: os seis meses **completos** antes do corrente (o mes
/// corrente e parcial e derrubaria a media). Denominador: meses da janela a partir do primeiro
/// lancamento vivo do hub — uma casa com 2 meses de historico dividiria por 6 e teria meta
/// irreal. Sem lancamento vivo antes do mes corrente -> `None` (sem base).
///
/// Casa pelo id cru, viva ou apagada: o gasto passado foi essencial mesmo que a categoria tenha
/// sido apagada depois.
pub fn essential_cost(
    dataset: &Dataset,
    category_ids: &[String],
    current: &str,
) -> Option<EssentialCost> {
    let first = dataset
        .alive_transactions()
        .map(|t| month_of(&t.occurred_on))
        .min()?;
    if first.as_str() >= current {
        return None;
    }
    let window = last_months(&shift_month(current, -1), ESSENTIAL_MONTHS);
    let months = window.iter().filter(|m| **m >= first).count().max(1);
    let den = i64::try_from(months).unwrap_or(i64::MAX);
    let mut seen: Vec<&str> = Vec::new();
    let mut parts: Vec<EssentialPart> = Vec::new();
    for id in category_ids {
        if seen.contains(&id.as_str()) {
            continue;
        }
        seen.push(id);
        let sum = dataset
            .alive_transactions()
            .filter(|t| {
                t.kind == Kind::Expense
                    && t.category_id.as_deref() == Some(id.as_str())
                    && window.contains(&month_of(&t.occurred_on))
            })
            .fold(0i64, |acc, t| acc.saturating_add(t.amount_minor));
        let (name, color) = match dataset.find_category(Some(id)) {
            Some(c) => (c.name.clone(), c.color.clone()),
            None => ("Categoria removida".to_string(), "slate".to_string()),
        };
        parts.push(EssentialPart {
            category_id: id.clone(),
            name,
            color,
            average_minor: div_round(sum, den),
        });
    }
    parts.sort_by(|a, b| {
        b.average_minor
            .cmp(&a.average_minor)
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.category_id.cmp(&b.category_id))
    });
    Some(EssentialCost {
        total_minor: parts
            .iter()
            .fold(0i64, |acc, p| acc.saturating_add(p.average_minor)),
        months,
        parts,
    })
}

/// Meta derivada: `multiple x custo`; sem base, 0.
fn derived_goal(cost: Option<&EssentialCost>, multiple: u8) -> i64 {
    cost.map_or(0, |c| c.total_minor.saturating_mul(i64::from(multiple)))
}

/// Meta de uma reserva de emergencia: `multiple x custo essencial das categorias dela`.
pub fn emergency_goal(dataset: &Dataset, r: &Reserve, current: &str) -> i64 {
    let cost = essential_cost(dataset, &r.essential_category_ids, current);
    derived_goal(cost.as_ref(), r.multiple.unwrap_or(DEFAULT_MULTIPLE))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EmergencyEta {
    NoGoal,
    Reached,
    NoRecurring,
    Projected { month: String },
}

pub fn eta_label(eta: &EmergencyEta, recurring: Option<i64>) -> String {
    match eta {
        EmergencyEta::NoGoal => "Sem custo essencial para projetar a meta.".to_string(),
        EmergencyEta::Reached => "Meta alcançada.".to_string(),
        EmergencyEta::NoRecurring => "Sem depósito mensal programado.".to_string(),
        EmergencyEta::Projected { month } => format!(
            "Guardando {} por mês, a meta fecha em {}.",
            whole_brl(recurring.unwrap_or(0)),
            month_short_year(month)
        ),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EmergencyView {
    pub id: String,
    pub name: String,
    pub balance_minor: i64,
    pub multiple: u8,
    pub cost: Option<EssentialCost>,
    pub goal_minor: i64,
    /// Meses de custo cobertos. Unico `f64` do modulo: so exibicao (rotulo e medidor), nunca
    /// decide nada.
    pub coverage: Option<f64>,
    /// "3,5" ou "—".
    pub coverage_label: String,
    /// "59% da meta" ou "Sem meta".
    pub pct_label: String,
    /// "Faltam R$ 1.600,00 para a meta", "Meta alcançada" ou o aviso de sem custo.
    pub remaining_label: String,
    /// Preenchimento de cada segmento do medidor (0..=1); `multiple` segmentos.
    pub segments: Vec<f32>,
    pub eta: EmergencyEta,
    pub eta_label: String,
}

pub fn emergency_view(dataset: &Dataset, r: &Reserve, current: &str) -> EmergencyView {
    let balance = balance_until(dataset, &r.id, current);
    let multiple = r.multiple.unwrap_or(DEFAULT_MULTIPLE);
    let cost = essential_cost(dataset, &r.essential_category_ids, current);
    let goal = derived_goal(cost.as_ref(), multiple);
    let coverage = cost
        .as_ref()
        .filter(|c| c.total_minor > 0)
        .map(|c| balance as f64 / c.total_minor as f64);
    let recurring = r.recurring_amount_minor.filter(|v| *v > 0);
    let eta = if goal <= 0 {
        EmergencyEta::NoGoal
    } else if balance >= goal {
        EmergencyEta::Reached
    } else if let Some(rec) = recurring {
        let n = div_ceil(goal.saturating_sub(balance), rec);
        EmergencyEta::Projected {
            month: shift_month(current, i32::try_from(n).unwrap_or(i32::MAX)),
        }
    } else {
        EmergencyEta::NoRecurring
    };
    EmergencyView {
        id: r.id.clone(),
        name: r.name.clone(),
        balance_minor: balance,
        multiple,
        goal_minor: goal,
        coverage,
        coverage_label: coverage.map_or_else(|| "—".to_string(), one_decimal),
        pct_label: if goal > 0 {
            format!(
                "{}% da meta",
                js_round(balance as f64 / goal as f64 * 100.0) as i64
            )
        } else {
            "Sem meta".to_string()
        },
        remaining_label: if goal <= 0 {
            "Sem custo essencial para calcular a meta".to_string()
        } else if balance >= goal {
            "Meta alcançada".to_string()
        } else {
            format!(
                "Faltam {} para a meta",
                format_brl(goal.saturating_sub(balance))
            )
        },
        segments: (0..multiple)
            .map(|i| coverage.map_or(0.0, |c| (c - f64::from(i)).clamp(0.0, 1.0) as f32))
            .collect(),
        eta_label: eta_label(&eta, recurring),
        eta,
        cost,
    }
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

    fn strings(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    /// Dataset so com lancamentos: `(id, kind, valor, data, categoria)`.
    fn with_transactions(rows: &[(&str, &str, i64, &str, &str)]) -> Dataset {
        use crate::dashboard::dataset::{CATEGORIES, TRANSACTIONS};
        let mut ds = Dataset::default();
        ds.apply([RawRow {
            table: CATEGORIES.into(),
            id: "C5".into(),
            deleted_at: Some("x".into()),
            seq: 1,
            data: json!({ "name": "Antiga", "color": "red", "icon": "tag" }).to_string(),
        }]);
        for (i, (id, kind, amount, on, category)) in rows.iter().enumerate() {
            ds.apply([RawRow {
                table: TRANSACTIONS.into(),
                id: (*id).into(),
                deleted_at: None,
                seq: 10 + i as i64,
                data: json!({
                    "kind": kind,
                    "description": "x",
                    "amountMinor": amount,
                    "occurredOn": on,
                    "categoryId": category
                })
                .to_string(),
            }]);
        }
        ds
    }

    #[test]
    fn divisoes_inteiras() {
        assert_eq!(div_round(360_000, 6), 60_000);
        assert_eq!(div_round(7, 2), 4);
        assert_eq!(div_round(5, 2), 3);
        assert_eq!(div_round(0, 6), 0);
        assert_eq!(div_round(1, 3), 0);
        assert_eq!(div_round(2, 3), 1);
        assert_eq!(div_ceil(265_000, 10), 26_500);
        assert_eq!(div_ceil(7, 2), 4);
        assert_eq!(div_ceil(6, 2), 3);
    }

    #[test]
    fn custo_essencial_da_fixture() {
        let ds = fixtures_with_reserves();
        let c = essential_cost(&ds, &strings(&["C1", "C2", "C3", "C9"]), "2026-09").unwrap();
        assert_eq!(c.total_minor, 65_000);
        assert_eq!(c.months, 6);
        let got: Vec<(&str, i64)> = c
            .parts
            .iter()
            .map(|p| (p.category_id.as_str(), p.average_minor))
            .collect();
        assert_eq!(
            got,
            vec![("C2", 60_000), ("C1", 5_000), ("C9", 0), ("C3", 0)]
        );
        assert_eq!(c.parts[2].name, "Categoria removida");
        assert_eq!(c.parts[2].color, "slate");
        assert_eq!(c.parts[0].name, "Moradia");
        assert_eq!(c.parts[0].color, "amber");
    }

    #[test]
    fn denominador_e_meses_com_historico() {
        let ds = with_transactions(&[
            ("T1", "expense", 180_000, "2026-07-10", "C2"),
            ("T2", "expense", 180_000, "2026-08-10", "C2"),
        ]);
        let c = essential_cost(&ds, &strings(&["C2"]), "2026-09").unwrap();
        assert_eq!(c.months, 2);
        assert_eq!(c.total_minor, 180_000);
    }

    #[test]
    fn categoria_apagada_conta_pelo_id() {
        let ds = with_transactions(&[("T1", "expense", 30_000, "2026-08-01", "C5")]);
        let c = essential_cost(&ds, &strings(&["C5"]), "2026-09").unwrap();
        assert_eq!(c.months, 1);
        assert_eq!(c.total_minor, 30_000);
        assert_eq!(c.parts[0].name, "Categoria removida");
    }

    #[test]
    fn sem_historico_antes_do_mes_corrente() {
        let ds = with_transactions(&[("T1", "expense", 30_000, "2026-09-01", "C2")]);
        assert_eq!(essential_cost(&ds, &strings(&["C2"]), "2026-09"), None);
        assert_eq!(
            essential_cost(&Dataset::default(), &strings(&["C2"]), "2026-09"),
            None
        );
    }

    #[test]
    fn lista_vazia_receita_e_mes_corrente() {
        let ds = fixtures_with_reserves();
        let c = essential_cost(&ds, &[], "2026-09").unwrap();
        assert_eq!(c.total_minor, 0);
        assert!(c.parts.is_empty());
        // Receita na categoria essencial nao entra.
        let ds = with_transactions(&[
            ("T1", "expense", 60_000, "2026-08-01", "C2"),
            ("T2", "income", 900_000, "2026-08-02", "C2"),
        ]);
        let c = essential_cost(&ds, &strings(&["C2"]), "2026-09").unwrap();
        assert_eq!(c.total_minor, 60_000);
        // Mes corrente fora da janela: o aluguel de setembro da fixture nao muda o total.
        let mut ds = fixtures_with_reserves();
        let before = essential_cost(&ds, &strings(&["C2"]), "2026-09").unwrap();
        ds.apply([RawRow {
            table: crate::dashboard::dataset::TRANSACTIONS.into(),
            id: "TX".into(),
            deleted_at: None,
            seq: 999,
            data: json!({ "kind": "expense", "description": "x", "amountMinor": 77_777,
                "occurredOn": "2026-09-10", "categoryId": "C2" })
            .to_string(),
        }]);
        let after = essential_cost(&ds, &strings(&["C2"]), "2026-09").unwrap();
        assert_eq!(before, after);
    }

    #[test]
    fn emergencia_da_fixture() {
        let ds = fixtures_with_reserves();
        let e = emergency_view(&ds, &ds.reserves["RE1"], "2026-09");
        assert_eq!(e.balance_minor, 230_000);
        assert_eq!(e.multiple, 6);
        assert_eq!(e.goal_minor, 390_000);
        assert!((e.coverage.unwrap() - 3.538).abs() < 1e-3);
        assert_eq!(e.coverage_label, "3,5");
        assert_eq!(e.pct_label, "59% da meta");
        assert_eq!(e.remaining_label, "Faltam R$ 1.600,00 para a meta");
        let expected = [1.0, 1.0, 1.0, 0.538, 0.0, 0.0];
        assert_eq!(e.segments.len(), 6);
        for (got, want) in e.segments.iter().zip(expected) {
            assert!((got - want).abs() < 1e-3, "{got} {want}");
        }
        assert_eq!(
            e.eta,
            EmergencyEta::Projected {
                month: "2027-01".into()
            }
        );
        assert_eq!(
            e.eta_label,
            "Guardando R$ 500 por mês, a meta fecha em jan 2027."
        );
    }

    fn emergency(multiple: serde_json::Value, recurring: serde_json::Value) -> serde_json::Value {
        json!({
            "kind": "emergency", "name": "Reserva", "icon": "x", "color": "slate",
            "multiple": multiple, "essentialCategoryIds": ["C1", "C2"],
            "recurringAmountMinor": recurring
        })
    }

    #[test]
    fn multiplo_nulo_vale_seis_e_tres_da_tres_segmentos() {
        let mut ds = fixtures_with_reserves();
        add_reserve(&mut ds, "RE1", emergency(json!(null), json!(50000)));
        let e = emergency_view(&ds, &ds.reserves["RE1"], "2026-09");
        assert_eq!(e.segments.len(), 6);
        assert_eq!(e.goal_minor, 65_000 * 6);
        add_reserve(&mut ds, "RE1", emergency(json!(3), json!(50000)));
        let e = emergency_view(&ds, &ds.reserves["RE1"], "2026-09");
        assert_eq!(e.segments.len(), 3);
        assert_eq!(e.goal_minor, 65_000 * 3);
    }

    #[test]
    fn meta_alcancada_e_sem_recorrencia() {
        let mut ds = fixtures_with_reserves();
        add_movement(
            &mut ds,
            "MX",
            json!({ "reserveId": "RE1", "kind": "deposit", "amountMinor": 200000, "occurredOn": "2026-09-02" }),
        );
        let e = emergency_view(&ds, &ds.reserves["RE1"], "2026-09");
        assert_eq!(e.eta, EmergencyEta::Reached);
        assert_eq!(e.eta_label, "Meta alcançada.");
        assert_eq!(e.remaining_label, "Meta alcançada");
        // Percentual nao e limitado: o prototipo nao limita.
        assert_eq!(e.pct_label, "110% da meta");

        for recurring in [json!(null), json!(0), json!(-100)] {
            let mut ds = fixtures_with_reserves();
            add_reserve(&mut ds, "RE1", emergency(json!(6), recurring));
            let e = emergency_view(&ds, &ds.reserves["RE1"], "2026-09");
            assert_eq!(e.eta, EmergencyEta::NoRecurring);
            assert_eq!(e.eta_label, "Sem depósito mensal programado.");
        }
    }

    #[test]
    fn emergencia_sem_custo() {
        let mut ds = Dataset::default();
        add_reserve(&mut ds, "RE1", emergency(json!(6), json!(50000)));
        let e = emergency_view(&ds, &ds.reserves["RE1"], "2026-09");
        assert_eq!(e.cost, None);
        assert_eq!(e.goal_minor, 0);
        assert_eq!(e.coverage_label, "—");
        assert_eq!(e.pct_label, "Sem meta");
        assert_eq!(
            e.remaining_label,
            "Sem custo essencial para calcular a meta"
        );
        assert_eq!(e.segments, vec![0.0; 6]);
        assert_eq!(e.eta, EmergencyEta::NoGoal);
        assert_eq!(e.eta_label, "Sem custo essencial para projetar a meta.");
    }
}
