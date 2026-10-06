//! Agregacoes do Dashboard, espelho de `breakdown.ts` / `insights.ts` do app mais as regras
//! do prototipo (teto do eixo, media do periodo). So lancamentos vivos entram em conta.

use std::collections::HashMap;

use super::contract::Kind;
use super::dataset::{Dataset, Transaction};
use super::periods::shift_month;

/// Chave, cor e icone da fatia "Sem categoria": soma o lancamento sem categoria e o de
/// categoria apagada (cor e icone de registro morto nao voltam a tela, como `findCategory`).
pub const NO_CATEGORY_KEY: &str = "sem-categoria";
pub const NO_CATEGORY_NAME: &str = "Sem categoria";
pub const NO_CATEGORY_COLOR: &str = "slate";
pub const NO_CATEGORY_ICON: &str = "circle-dashed";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Totals {
    pub income_minor: i64,
    pub expense_minor: i64,
    pub balance_minor: i64,
    pub income_count: usize,
    pub expense_count: usize,
}

/// Soma so o que recebe: quem chama decide o recorte (mes, filtros).
pub fn totals<'a>(items: impl Iterator<Item = &'a Transaction>) -> Totals {
    let mut t = Totals::default();
    for tx in items {
        match tx.kind {
            Kind::Income => {
                t.income_minor = t.income_minor.saturating_add(tx.amount_minor);
                t.income_count += 1;
            }
            Kind::Expense => {
                t.expense_minor = t.expense_minor.saturating_add(tx.amount_minor);
                t.expense_count += 1;
            }
        }
    }
    t.balance_minor = t.income_minor.saturating_sub(t.expense_minor);
    t
}

/// Lancamentos vivos de `month` (`YYYY-MM`). `occurred_on` ja passou pelo contrato, entao os
/// 7 primeiros caracteres sao o mes.
pub(crate) fn in_month<'a, 'm>(
    dataset: &'a Dataset,
    month: &'m str,
) -> impl Iterator<Item = &'a Transaction> + use<'a, 'm> {
    dataset
        .alive_transactions()
        .filter(move |t| t.occurred_on.get(..7) == Some(month))
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MonthSummary {
    pub current: Totals,
    pub previous: Totals,
    /// Ha lancamento vivo antes deste mes. Sem historia o card diz "Primeiro mês" em vez de
    /// comparar com um mes que nunca existiu; com historia e um mes vazio no meio, a
    /// comparacao com zero continua honesta.
    pub had_previous: bool,
    pub delta_minor: i64,
    /// `None` quando o mes anterior e zero: nao ha base para porcentagem.
    pub income_change_pct: Option<i32>,
    pub expense_change_pct: Option<i32>,
}

pub fn month_summary(dataset: &Dataset, month: &str) -> MonthSummary {
    let current = totals(in_month(dataset, month));
    let prev_month = shift_month(month, -1);
    let previous = totals(in_month(dataset, &prev_month));
    // "2026-09" < "2026-09-01": comparar a data inteira com o mes e "antes do dia 1".
    let had_previous = dataset
        .alive_transactions()
        .any(|t| t.occurred_on.as_str() < month);
    MonthSummary {
        current,
        previous,
        had_previous,
        delta_minor: current.balance_minor.saturating_sub(previous.balance_minor),
        income_change_pct: change_pct(current.income_minor, previous.income_minor),
        expense_change_pct: change_pct(current.expense_minor, previous.expense_minor),
    }
}

/// `round((cur - prev) / prev * 100)`, `None` com base zero (nada de divisao por zero).
fn change_pct(current: i64, previous: i64) -> Option<i32> {
    if previous == 0 {
        return None;
    }
    let pct = (current as f64 - previous as f64) / previous as f64 * 100.0;
    Some(js_round(pct) as i32)
}

/// `Math.round` do JS: meio sempre para cima (`-2,5 -> -2`), diferente do `f64::round` do Rust
/// (`-2,5 -> -3`). Os rotulos precisam bater com o prototipo e com o app.
pub(crate) fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

#[derive(Clone, Debug, PartialEq)]
pub struct Slice {
    /// Id da categoria, ou `NO_CATEGORY_KEY`.
    pub key: String,
    pub name: String,
    /// Token de cor do app.
    pub color: String,
    /// Chave de icone do app.
    pub icon: String,
    pub amount_minor: i64,
    /// Fracao do total de despesas do mes (0..=1).
    pub share: f64,
}

/// Despesas do mes por categoria viva; o resto vira "Sem categoria". Ordem decrescente por
/// valor, desempate por nome. Sem agrupar em "Outras": o design mostra todas, como o "Para
/// onde foi" do app.
pub fn expense_by_category(dataset: &Dataset, month: &str) -> Vec<Slice> {
    let mut by_key: HashMap<String, Slice> = HashMap::new();
    let mut total: i64 = 0;
    for tx in in_month(dataset, month).filter(|t| t.kind == Kind::Expense) {
        total = total.saturating_add(tx.amount_minor);
        let category = dataset.find_category(tx.category_id.as_deref());
        let key = category.map_or(NO_CATEGORY_KEY, |c| c.id.as_str());
        let slice = by_key
            .entry(key.to_string())
            .or_insert_with(|| match category {
                Some(c) => Slice {
                    key: c.id.clone(),
                    name: c.name.clone(),
                    color: c.color.clone(),
                    icon: c.icon.clone(),
                    amount_minor: 0,
                    share: 0.0,
                },
                None => Slice {
                    key: NO_CATEGORY_KEY.into(),
                    name: NO_CATEGORY_NAME.into(),
                    color: NO_CATEGORY_COLOR.into(),
                    icon: NO_CATEGORY_ICON.into(),
                    amount_minor: 0,
                    share: 0.0,
                },
            });
        slice.amount_minor = slice.amount_minor.saturating_add(tx.amount_minor);
    }
    let mut slices: Vec<Slice> = by_key.into_values().collect();
    for slice in &mut slices {
        // Total zero (so despesas de valor zero) nao divide: a fatia fica sem tamanho.
        slice.share = if total > 0 {
            slice.amount_minor as f64 / total as f64
        } else {
            0.0
        };
    }
    slices.sort_by(|a, b| {
        b.amount_minor
            .cmp(&a.amount_minor)
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.key.cmp(&b.key))
    });
    slices
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MonthTotals {
    pub month: String,
    pub income_minor: i64,
    pub expense_minor: i64,
}

impl MonthTotals {
    pub fn net_minor(&self) -> i64 {
        self.income_minor.saturating_sub(self.expense_minor)
    }
}

/// Um total por mes pedido, na mesma ordem; mes sem lancamento vem zerado.
pub fn monthly_totals(dataset: &Dataset, months: &[String]) -> Vec<MonthTotals> {
    let mut sums: HashMap<&str, (i64, i64)> = months.iter().map(|m| (m.as_str(), (0, 0))).collect();
    for tx in dataset.alive_transactions() {
        let Some(entry) = tx.occurred_on.get(..7).and_then(|m| sums.get_mut(m)) else {
            continue;
        };
        match tx.kind {
            Kind::Income => entry.0 = entry.0.saturating_add(tx.amount_minor),
            Kind::Expense => entry.1 = entry.1.saturating_add(tx.amount_minor),
        }
    }
    months
        .iter()
        .map(|m| {
            let (income_minor, expense_minor) = sums.get(m.as_str()).copied().unwrap_or_default();
            MonthTotals {
                month: m.clone(),
                income_minor,
                expense_minor,
            }
        })
        .collect()
}

/// Topo do eixo: o pico arredondado para cima em multiplos de R$ 2.000, minimo R$ 2.000 (o
/// `Math.ceil(max(1, pico) / 2000) * 2000` do prototipo). Assim a marca do meio e sempre um
/// milhar redondo.
pub fn axis_top(peak_minor: i64) -> i64 {
    const STEP: i64 = 200_000;
    let peak = peak_minor.max(0);
    (peak.saturating_add(STEP - 1) / STEP)
        .max(1)
        .saturating_mul(STEP)
}

/// Media do saldo dos meses (arredondada como o JS); nenhum mes -> 0.
pub fn average_surplus(months: &[MonthTotals]) -> i64 {
    if months.is_empty() {
        return 0;
    }
    let sum: f64 = months.iter().map(|m| m.net_minor() as f64).sum();
    js_round(sum / months.len() as f64) as i64
}

/// Os `n` mais recentes do mes: data desc, id desc (a ordem de `listTransactions`).
pub fn recent<'a>(dataset: &'a Dataset, month: &str, n: usize) -> Vec<&'a Transaction> {
    let mut items: Vec<&Transaction> = in_month(dataset, month).collect();
    sort_desc(&mut items);
    items.truncate(n);
    items
}

/// Data desc, id desc.
pub(crate) fn sort_desc(items: &mut [&Transaction]) {
    items.sort_by(|a, b| {
        b.occurred_on
            .cmp(&a.occurred_on)
            .then_with(|| b.id.cmp(&a.id))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dashboard::fixtures;
    use crate::dashboard::periods::last_months;

    #[test]
    fn totais_somam_o_que_recebem_e_contam() {
        let ds = fixtures();
        let t = totals(
            ds.alive_transactions()
                .filter(|t| t.occurred_on.starts_with("2026-09")),
        );
        assert_eq!(t.income_minor, 650_000);
        assert_eq!(t.expense_minor, 231_000);
        assert_eq!(t.balance_minor, 419_000);
        assert_eq!(t.income_count, 1);
        assert_eq!(t.expense_count, 6);
        assert_eq!(totals(std::iter::empty()), Totals::default());
    }

    #[test]
    fn resumo_do_mes_com_anterior() {
        let ds = fixtures();
        let s = month_summary(&ds, "2026-09");
        assert_eq!(s.current.income_minor, 650_000);
        assert_eq!(s.current.expense_count, 6);
        assert_eq!(s.previous.expense_minor, 210_000);
        assert!(s.had_previous);
        assert_eq!(s.delta_minor, 419_000 - 440_000);
        assert_eq!(s.income_change_pct, Some(0));
        assert_eq!(s.expense_change_pct, Some(10));
    }

    #[test]
    fn resumo_sem_base_e_sem_historia() {
        let ds = fixtures();
        // Abril: nada no mes, nada em marco alem de uma receita.
        let april = month_summary(&ds, "2026-04");
        assert_eq!(april.current, Totals::default());
        assert!(april.had_previous);
        assert_eq!(april.expense_change_pct, None);
        // Marco e o primeiro mes com dado: nao ha historia antes.
        let march = month_summary(&ds, "2026-03");
        assert!(!march.had_previous);
        assert_eq!(march.income_change_pct, None);
        assert_eq!(march.delta_minor, 650_000);
        // Julho contra junho vazio: sem base para o %, mas ha historia (marco).
        let july = month_summary(&ds, "2026-07");
        assert!(july.had_previous);
        assert_eq!(july.income_change_pct, None);
        assert_eq!(
            month_summary(&Dataset::default(), "2026-09"),
            MonthSummary::default()
        );
    }

    #[test]
    fn saldo_negativo_e_queda() {
        let ds = fixtures();
        // Outubro: nada; o saldo cai para zero e a receita cai 100 %.
        let oct = month_summary(&ds, "2026-10");
        assert_eq!(oct.delta_minor, -419_000);
        assert_eq!(oct.income_change_pct, Some(-100));
        assert_eq!(oct.expense_change_pct, Some(-100));
    }

    #[test]
    fn rosca_soma_a_copia_fundida_no_padrao() {
        let ds = crate::dashboard::fixtures_with_merged();
        let slices = expense_by_category(&ds, "2026-09");
        let names: Vec<(&str, &str, i64)> = slices
            .iter()
            .map(|s| (s.key.as_str(), s.name.as_str(), s.amount_minor))
            .collect();
        assert_eq!(
            names,
            vec![
                ("C2", "Moradia", 180_000),
                ("C1", "Alimentação", 50_000),
                ("C3", "Transporte", 6_000),
                ("sem-categoria", "Sem categoria", 5_000),
            ]
        );
        assert_eq!(slices[1].color, "orange");
    }

    #[test]
    fn rosca_por_categoria() {
        let ds = fixtures();
        let slices = expense_by_category(&ds, "2026-09");
        let names: Vec<(&str, i64)> = slices
            .iter()
            .map(|s| (s.name.as_str(), s.amount_minor))
            .collect();
        assert_eq!(
            names,
            vec![
                ("Moradia", 180_000),
                ("Alimentação", 40_000),
                ("Transporte", 6_000),
                ("Sem categoria", 5_000),
            ]
        );
        let none = &slices[3];
        assert_eq!(none.key, "sem-categoria");
        assert_eq!(none.color, "slate");
        assert_eq!(none.icon, "circle-dashed");
        assert_eq!(slices[0].key, "C2");
        assert_eq!(slices[0].color, "amber");
        assert_eq!(slices[0].icon, "house");
        let sum: f64 = slices.iter().map(|s| s.share).sum();
        assert!((sum - 1.0).abs() < 1e-9, "{sum}");
        // Mes so com receita (marco) ou sem nada: vazio.
        assert!(expense_by_category(&ds, "2026-03").is_empty());
        assert!(expense_by_category(&ds, "2026-05").is_empty());
    }

    #[test]
    fn rosca_desempata_por_nome_e_nunca_agrupa_em_outras() {
        use crate::dashboard::dataset::{CATEGORIES, RawRow, TRANSACTIONS};
        let mut ds = Dataset::default();
        let mut rows = Vec::new();
        for i in 0..8 {
            let id = format!("C{i}");
            rows.push(RawRow {
                table: CATEGORIES.into(),
                id: id.clone(),
                deleted_at: None,
                seq: i64::from(i) * 2 + 1,
                data: serde_json::json!({ "name": format!("Cat {}", 7 - i), "color": "sky", "icon": "tag" })
                    .to_string(),
            });
            rows.push(RawRow {
                table: TRANSACTIONS.into(),
                id: format!("T{i}"),
                deleted_at: None,
                seq: i64::from(i) * 2 + 2,
                data: serde_json::json!({
                    "kind": "expense", "description": "x", "amountMinor": 100,
                    "occurredOn": "2026-09-01", "categoryId": id
                })
                .to_string(),
            });
        }
        ds.apply(rows);
        let slices = expense_by_category(&ds, "2026-09");
        assert_eq!(slices.len(), 8);
        assert_eq!(slices[0].name, "Cat 0");
        assert_eq!(slices[7].name, "Cat 7");
    }

    #[test]
    fn seis_meses_com_zeros() {
        let ds = fixtures();
        let bars = monthly_totals(&ds, &last_months("2026-09", 6));
        let months: Vec<&str> = bars.iter().map(|b| b.month.as_str()).collect();
        assert_eq!(
            months,
            vec![
                "2026-04", "2026-05", "2026-06", "2026-07", "2026-08", "2026-09"
            ]
        );
        assert!(
            bars[..3]
                .iter()
                .all(|b| b.income_minor == 0 && b.expense_minor == 0)
        );
        assert_eq!(bars[3].income_minor, 650_000);
        assert_eq!(bars[4].expense_minor, 210_000);
        assert_eq!(bars[5].expense_minor, 231_000);
        // Marco (fora) nao entra; virada de ano tambem funciona.
        let jan = monthly_totals(&ds, &last_months("2026-01", 6));
        assert_eq!(jan[0].month, "2025-08");
        assert!(jan.iter().all(|b| b.income_minor == 0));
    }

    #[test]
    fn teto_do_eixo() {
        assert_eq!(axis_top(0), 200_000);
        assert_eq!(axis_top(-50), 200_000);
        assert_eq!(axis_top(1), 200_000);
        assert_eq!(axis_top(150_000), 200_000);
        assert_eq!(axis_top(200_000), 200_000);
        assert_eq!(axis_top(200_001), 400_000);
        assert_eq!(axis_top(1_234_567), 1_400_000);
    }

    #[test]
    fn media_arredondada() {
        let m = |inc: i64, exp: i64| MonthTotals {
            month: String::new(),
            income_minor: inc,
            expense_minor: exp,
        };
        assert_eq!(average_surplus(&[m(100, 0), m(0, 400), m(100, 0)]), -67);
        assert_eq!(average_surplus(&[m(3, 0), m(0, 0)]), 2);
        assert_eq!(average_surplus(&[]), 0);
    }

    #[test]
    fn recentes_do_mes() {
        let ds = fixtures();
        let ids: Vec<&str> = recent(&ds, "2026-09", 6)
            .iter()
            .map(|t| t.id.as_str())
            .collect();
        // Mesmo dia: id desc. Apagado (T12) fora.
        assert_eq!(ids, vec!["T10", "T09", "T08", "T13", "T11", "T07"]);
        assert_eq!(recent(&ds, "2026-09", 2).len(), 2);
        assert!(recent(&ds, "2026-05", 6).is_empty());
    }
}
