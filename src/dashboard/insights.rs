//! Indicadores do "Resumo da casa" (spec do dashboard Grafite 2.2-2.4, 5.6-5.8): ritmo contra
//! "o seu normal", previsao do mes, fixo x variavel, categoria acima do normal, plano, quem lancou
//! e historico. Port de `insights.ts`, `forecast.ts`, `fixed-variable.ts`, `category-normal.ts` e
//! `daily-expenses.ts` do app, na mesma forma aritmetica: cada porcentagem repete a expressao de
//! ponto flutuante do TS (`js_round(a / b * 100)`) e cada media de dinheiro usa `div_round`
//! (`Math.round` de inteiros), senao o celular e o PC mostrariam numeros diferentes num x,5.
//!
//! Tudo recebe a visao de UMA conta (`Dataset::for_book`) e "hoje" por parametro: nada aqui le
//! relogio. Lancamento conta pelo `counts_in_totals` (transferencia e oculto ficam fora de toda
//! soma), como no resto do dashboard.

use super::aggregate::{MonthTotals, in_month, js_round};
use super::contract::Kind;
use super::dataset::{Dataset, Transaction};
use super::periods::{days_in_month, month_of, shift_month};
use super::reserves::div_round;

/// Quantos meses anteriores formam "o seu normal" (`NORMAL_MONTHS` do app).
pub const NORMAL_MONTHS: i32 = 3;

/// Dia do mes de uma data `YYYY-MM-DD` (0 fora da forma).
fn day_of(date: &str) -> usize {
    date.get(8..10).and_then(|d| d.parse().ok()).unwrap_or(0)
}

/// Despesa de cada dia do mes (indice 0 = dia 1), so do que conta nos totais. Um dia alem do fim
/// do mes (`2026-02-31` passa no contrato) fica de fora, como no app.
fn expense_by_day<'a>(items: impl IntoIterator<Item = &'a Transaction>, month: &str) -> Vec<i64> {
    let mut daily = vec![0i64; usize::from(days_in_month(month))];
    for t in items {
        if t.kind != Kind::Expense || !t.counts_in_totals() || t.occurred_on.get(..7) != Some(month)
        {
            continue;
        }
        if let Some(slot) = day_of(&t.occurred_on)
            .checked_sub(1)
            .and_then(|i| daily.get_mut(i))
        {
            *slot = slot.saturating_add(t.amount_minor);
        }
    }
    daily
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DailyExpenses {
    /// Despesa de cada dia do mes (indice 0 = dia 1).
    pub by_day: Vec<i64>,
    /// Maior valor de um dia; 0 se o mes nao teve saida.
    pub max_minor: i64,
    /// Dia (1..n) do maior valor, o primeiro em empate; `None` sem saida.
    pub peak_day: Option<usize>,
    /// Dias que entram nas medias: os que ja aconteceram.
    pub elapsed_days: usize,
    /// Media por dia sobre `elapsed_days`; 0 sem dia.
    pub average_minor: i64,
    /// Dias ja ocorridos sem saida.
    pub empty_days: usize,
}

/// `dailyExpenses` do app. `through` limita as medias aos dias que ja aconteceram (hoje no mes
/// corrente; o fim num mes passado): dividir pelo mes inteiro faria o mes em curso parecer
/// barato. Quem chama passa so os lancamentos que quer contar (o Dashboard, os avulsos).
pub fn daily_expenses<'a>(
    items: impl IntoIterator<Item = &'a Transaction>,
    month: &str,
    through: usize,
) -> DailyExpenses {
    let by_day = expense_by_day(items, month);
    let elapsed_days = through.min(by_day.len());
    let elapsed = by_day.get(..elapsed_days).unwrap_or_default();
    let spent = elapsed.iter().fold(0i64, |acc, v| acc.saturating_add(*v));
    let max_minor = by_day.iter().copied().max().unwrap_or(0).max(0);
    DailyExpenses {
        peak_day: (max_minor > 0)
            .then(|| by_day.iter().position(|v| *v == max_minor).map(|i| i + 1))
            .flatten(),
        max_minor,
        elapsed_days,
        average_minor: if elapsed_days == 0 {
            0
        } else {
            div_round(spent, i64::try_from(elapsed_days).unwrap_or(i64::MAX))
        },
        empty_days: elapsed.iter().filter(|v| **v == 0).count(),
        by_day,
    }
}

/// Mes do primeiro lancamento vivo da conta, conte ele nos totais ou nao (`firstMonthOf`): e o
/// comeco do historico. Mes anterior a ele "nao existia", nao "gastou zero".
pub fn first_month(ds: &Dataset) -> Option<String> {
    ds.alive_transactions()
        .map(|t| month_of(&t.occurred_on))
        .min()
}

/// Meses que formam "o seu normal" de `month` (`referenceMonths`): os 3 anteriores, do mais
/// antigo ao mais novo, so os do primeiro lancamento em diante. Dividir por 3 com um mes de
/// historia daria um normal tres vezes menor; um mes vazio *depois* do primeiro conta (zero).
pub fn reference_months(ds: &Dataset, month: &str) -> Vec<String> {
    let Some(first) = first_month(ds) else {
        return Vec::new();
    };
    (1..=NORMAL_MONTHS)
        .rev()
        .map(|back| shift_month(month, -back))
        .filter(|m| *m >= first)
        .collect()
}

/// Despesa acumulada ao fim de cada dia do mes (indice 0 = dia 1).
fn cumulative_expense(ds: &Dataset, month: &str) -> Vec<i64> {
    let mut running = 0i64;
    expense_by_day(ds.alive_transactions(), month)
        .into_iter()
        .map(|v| {
            running = running.saturating_add(v);
            running
        })
        .collect()
}

/// `(gasto / normal - 1) * 100` arredondado com sinal, na forma exata do app. `None` sem base.
fn change_percent(spent: i64, normal: i64) -> Option<i64> {
    (normal != 0).then(|| js_round((spent as f64 / normal as f64 - 1.0) * 100.0) as i64)
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpendingPace {
    /// Acumulado por dia do mes escolhido.
    pub current: Vec<i64>,
    /// "Seu normal": media dia a dia dos acumulados dos meses de referencia, um valor por dia
    /// do mes escolhido. Vazio sem mes de referencia.
    pub normal: Vec<i64>,
    pub reference_months: Vec<String>,
    /// Ultimo dia que ja aconteceu: hoje no mes corrente, o fim num mes passado, 0 no futuro.
    pub cutoff: usize,
    pub spent_minor: i64,
    /// Normal no dia do corte; `None` sem mes de referencia.
    pub normal_at_cutoff_minor: Option<i64>,
    /// Variacao contra o normal no mesmo dia; `None` sem normal ou com normal zero.
    pub change: Option<f64>,
    /// `change` em porcentagem inteira com sinal (`Math.round`).
    pub change_percent: Option<i64>,
}

/// `spendingPace` do app: quanto ja saiu ate o corte contra a media dos 3 meses anteriores com
/// dado no mesmo dia. Num mes de referencia mais curto o dia alem do fim vale o ultimo dia dele.
pub fn spending_pace(ds: &Dataset, month: &str, today: &str) -> SpendingPace {
    let current = cumulative_expense(ds, month);
    let this_month = month_of(today);
    let months = reference_months(ds, month);
    let curves: Vec<Vec<i64>> = months.iter().map(|m| cumulative_expense(ds, m)).collect();
    let normal: Vec<i64> = if curves.is_empty() {
        Vec::new()
    } else {
        let n = i64::try_from(curves.len()).unwrap_or(i64::MAX);
        (1..=current.len())
            .map(|day| {
                let sum = curves.iter().fold(0i64, |acc, curve| {
                    let at = day.min(curve.len()).checked_sub(1);
                    acc.saturating_add(at.and_then(|i| curve.get(i)).copied().unwrap_or(0))
                });
                div_round(sum, n)
            })
            .collect()
    };
    let cutoff = match month.cmp(this_month.as_str()) {
        std::cmp::Ordering::Equal => day_of(today),
        std::cmp::Ordering::Less => current.len(),
        std::cmp::Ordering::Greater => 0,
    };
    let at = |values: &[i64]| {
        cutoff
            .checked_sub(1)
            .and_then(|i| values.get(i))
            .copied()
            .unwrap_or(0)
    };
    let spent_minor = at(&current);
    let normal_at_cutoff_minor = (!normal.is_empty()).then(|| at(&normal));
    let change = normal_at_cutoff_minor
        .filter(|n| *n != 0)
        .map(|n| spent_minor as f64 / n as f64 - 1.0);
    SpendingPace {
        change_percent: normal_at_cutoff_minor.and_then(|n| change_percent(spent_minor, n)),
        current,
        normal,
        reference_months: months,
        cutoff,
        spent_minor,
        normal_at_cutoff_minor,
        change,
    }
}

/// "Seu normal" do saldo (`monthComparison.normalBalanceMinor`, M3): media do saldo dos meses de
/// referencia. `None` sem mes de referencia (a linha "a mais/a menos" some).
pub fn normal_balance(ds: &Dataset, month: &str) -> Option<i64> {
    let months = reference_months(ds, month);
    if months.is_empty() {
        return None;
    }
    let sum = months.iter().fold(0i64, |acc, m| {
        acc.saturating_add(super::aggregate::totals(in_month(ds, m)).balance_minor)
    });
    Some(div_round(
        sum,
        i64::try_from(months.len()).unwrap_or(i64::MAX),
    ))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SurplusAverage {
    /// Media da sobra; negativa quando faltou.
    pub average_minor: i64,
    /// Meses que entraram na media.
    pub months: usize,
}

/// `averageSurplus` do app (M2): media da sobra so dos meses **fechados com dado** de `months`
/// (do primeiro lancamento ate o anterior a `current_month`). O mes corrente pela metade sempre
/// pareceria ter sobrado muito; mes antes do primeiro lancamento diluiria a media. `None` sem
/// nenhum mes fechado.
pub fn average_surplus_closed(
    months: &[MonthTotals],
    first_month: Option<&str>,
    current_month: &str,
) -> Option<SurplusAverage> {
    let first = first_month?;
    let closed: Vec<&MonthTotals> = months
        .iter()
        .filter(|m| m.month.as_str() >= first && m.month.as_str() < current_month)
        .collect();
    if closed.is_empty() {
        return None;
    }
    let sum = closed
        .iter()
        .fold(0i64, |acc, m| acc.saturating_add(m.net_minor()));
    Some(SurplusAverage {
        average_minor: div_round(sum, i64::try_from(closed.len()).unwrap_or(i64::MAX)),
        months: closed.len(),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::dashboard::dataset::TRANSACTIONS;
    use crate::dashboard::{GRAFITE_TODAY, dataset_of, fixtures_grafite};

    type Row = (&'static str, &'static str, bool, Value);

    /// Despesa avulsa (o `record` dos testes do app).
    fn spend(id: &'static str, amount: i64, on: &str) -> Row {
        (
            TRANSACTIONS,
            id,
            false,
            json!({ "kind": "expense", "description": "Mercado", "amountMinor": amount, "occurredOn": on }),
        )
    }

    fn with(mut row: Row, over: Value) -> Row {
        if let (Some(b), Some(o)) = (row.3.as_object_mut(), over.as_object()) {
            for (k, v) in o {
                b.insert(k.clone(), v.clone());
            }
        }
        row
    }

    fn income(id: &'static str, amount: i64, on: &str) -> Row {
        with(spend(id, amount, on), json!({ "kind": "income" }))
    }

    // --- daily-expenses.test.ts ---

    #[test]
    fn soma_por_dia_e_ignora_receita_e_outros_meses() {
        let ds = dataset_of(vec![
            spend("a", 500, "2026-09-03"),
            spend("b", 700, "2026-09-03"),
            spend("c", 300, "2026-09-10"),
            income("d", 9999, "2026-09-10"),
            spend("e", 9999, "2026-08-31"),
        ]);
        let r = daily_expenses(ds.alive_transactions(), "2026-09", 30);
        assert_eq!(r.by_day.len(), 30);
        assert_eq!(r.by_day[2], 1200);
        assert_eq!(r.by_day[9], 300);
        assert_eq!(r.max_minor, 1200);
        assert_eq!(r.peak_day, Some(3));
    }

    #[test]
    fn respeita_conta_nos_totais_transferencia_e_oculto_ficam_de_fora() {
        let ds = dataset_of(vec![
            spend("a", 100, "2026-09-07"),
            with(
                spend("b", 5000, "2026-09-07"),
                json!({ "reportState": "transfer" }),
            ),
            with(
                spend("c", 5000, "2026-09-07"),
                json!({ "reportState": "hidden" }),
            ),
        ]);
        let r = daily_expenses(ds.alive_transactions(), "2026-09", 30);
        assert_eq!(r.by_day[6], 100);
    }

    #[test]
    fn media_e_dias_sem_saida_usam_so_os_dias_que_ja_aconteceram() {
        let ds = dataset_of(vec![
            spend("a", 1000, "2026-09-01"),
            spend("b", 2000, "2026-09-03"),
        ]);
        let r = daily_expenses(ds.alive_transactions(), "2026-09", 4);
        assert_eq!(r.elapsed_days, 4);
        assert_eq!(r.average_minor, 750);
        assert_eq!(r.empty_days, 2);
    }

    #[test]
    fn mes_sem_saida_e_mes_futuro_nao_dividem_por_zero() {
        let ds = dataset_of(vec![]);
        let r = daily_expenses(ds.alive_transactions(), "2026-09", 30);
        assert_eq!((r.peak_day, r.average_minor, r.empty_days), (None, 0, 30));
        let r = daily_expenses(ds.alive_transactions(), "2026-12", 0);
        assert_eq!((r.average_minor, r.empty_days), (0, 0));
    }

    #[test]
    fn empate_no_maior_dia_fica_com_o_primeiro() {
        let ds = dataset_of(vec![
            spend("a", 100, "2026-09-09"),
            spend("b", 100, "2026-09-02"),
        ]);
        assert_eq!(
            daily_expenses(ds.alive_transactions(), "2026-09", 30).peak_day,
            Some(2)
        );
    }

    #[test]
    fn media_diaria_avulsa_da_fixture() {
        // 3.2: div_round(223741, 19) = 11776 -> "~R$ 118".
        let ds = fixtures_grafite();
        let casual = ds
            .alive_transactions()
            .filter(|t| t.recurrence_id.is_none());
        let r = daily_expenses(casual, "2026-11", 19);
        assert_eq!(r.average_minor, 11_776);
    }

    // --- insights.test.ts: spendingPace (M1, 3 meses) ---

    /// Setembro contra o normal de junho, julho e agosto (o primeiro lancamento e de junho).
    fn pace_items() -> Vec<Row> {
        vec![
            spend("s1", 1000, "2026-09-01"),
            spend("s2", 2000, "2026-09-10"),
            spend("s3", 9999, "2026-09-28"),
            spend("a1", 1500, "2026-08-05"),
            spend("a2", 2500, "2026-08-20"),
            spend("a3", 800, "2026-08-31"),
            spend("j1", 3000, "2026-07-03"),
            spend("j2", 1000, "2026-07-24"),
            spend("u1", 500, "2026-06-15"),
            // Receita nao entra no ritmo de gasto.
            income("r", 50_000, "2026-09-02"),
        ]
    }

    #[test]
    fn acumula_a_despesa_dia_a_dia_e_o_normal_e_a_media_dos_3_anteriores_no_mesmo_dia() {
        let p = spending_pace(&dataset_of(pace_items()), "2026-09", "2026-09-24");
        assert_eq!(p.reference_months, ["2026-06", "2026-07", "2026-08"]);
        assert_eq!(p.current.len(), 30);
        assert_eq!(p.normal.len(), 30);
        assert_eq!(p.current[0], 1000);
        assert_eq!(p.current[9], 3000);
        assert_eq!(p.normal[0], 0);
        // Dia 24: (500 + 4000 + 4000) / 3 = 2833,33.
        assert_eq!(p.normal[23], 2833);
    }

    #[test]
    fn no_mes_corrente_corta_em_hoje_e_compara_com_o_normal_do_mesmo_dia() {
        let p = spending_pace(&dataset_of(pace_items()), "2026-09", "2026-09-24");
        assert_eq!(p.cutoff, 24);
        assert_eq!(p.spent_minor, 3000);
        assert_eq!(p.normal_at_cutoff_minor, Some(2833));
        assert_eq!(p.change, Some(3000.0 / 2833.0 - 1.0));
        assert_eq!(p.change_percent, Some(6));
    }

    #[test]
    fn num_mes_passado_corta_no_ultimo_dia() {
        let p = spending_pace(&dataset_of(pace_items()), "2026-08", "2026-09-24");
        assert_eq!(p.cutoff, 31);
        assert_eq!(p.spent_minor, 4800);
        assert_eq!(p.reference_months, ["2026-06", "2026-07"]);
        // Dia 31: junho (30 dias) vale o dia 30 = 500; julho = 4000.
        assert_eq!(p.normal_at_cutoff_minor, Some(2250));
    }

    #[test]
    fn sem_gasto_no_normal_nao_inventa_porcentagem() {
        let mut rows: Vec<Row> = pace_items().into_iter().take(3).collect();
        rows.push(income("ra", 1000, "2026-08-02"));
        let p = spending_pace(&dataset_of(rows), "2026-09", "2026-09-24");
        assert_eq!(p.reference_months, ["2026-08"]);
        assert_eq!(p.normal_at_cutoff_minor, Some(0));
        assert_eq!((p.change, p.change_percent), (None, None));
    }

    #[test]
    fn dia_de_corte_alem_do_fim_de_um_mes_de_referencia_usa_o_ultimo_dia_dele() {
        let ds = dataset_of(vec![spend("f", 700, "2026-02-28")]);
        let p = spending_pace(&ds, "2026-03", "2026-03-31");
        assert_eq!(p.normal_at_cutoff_minor, Some(700));
    }

    #[test]
    fn normal_divide_so_pelos_meses_com_dado() {
        // Primeiro lancamento em agosto: junho e julho nao existiam e nao puxam o normal.
        let rows: Vec<Row> = pace_items().into_iter().take(6).collect();
        let p = spending_pace(&dataset_of(rows), "2026-09", "2026-09-24");
        assert_eq!(p.reference_months, ["2026-08"]);
        assert_eq!(p.normal_at_cutoff_minor, Some(4000));
        assert_eq!(p.change_percent, Some(-25));
    }

    #[test]
    fn mes_sem_gasto_depois_do_primeiro_lancamento_conta_como_zero() {
        let mut rows: Vec<Row> = pace_items().into_iter().take(6).collect();
        rows.push(spend("u1", 500, "2026-06-15"));
        let p = spending_pace(&dataset_of(rows), "2026-09", "2026-09-24");
        assert_eq!(p.reference_months, ["2026-06", "2026-07", "2026-08"]);
        assert_eq!(p.normal_at_cutoff_minor, Some(1500));
    }

    #[test]
    fn normal_sem_mes_de_referencia() {
        let rows: Vec<Row> = pace_items().into_iter().take(3).collect();
        let p = spending_pace(&dataset_of(rows), "2026-09", "2026-09-24");
        assert!(p.reference_months.is_empty());
        assert!(p.normal.is_empty());
        assert_eq!(p.normal_at_cutoff_minor, None);
        assert_eq!((p.change, p.change_percent), (None, None));
    }

    #[test]
    fn lancamento_marcado_tambem_define_o_primeiro_mes() {
        let mut rows: Vec<Row> = pace_items().into_iter().take(6).collect();
        rows.push(with(
            spend("t", 9000, "2026-07-02"),
            json!({ "reportState": "transfer" }),
        ));
        let p = spending_pace(&dataset_of(rows), "2026-09", "2026-09-24");
        assert_eq!(p.reference_months, ["2026-07", "2026-08"]);
        // A transferencia nao soma: julho vale zero. (0 + 4000) / 2.
        assert_eq!(p.normal_at_cutoff_minor, Some(2000));
    }

    #[test]
    fn a_porcentagem_arredonda_com_sinal_como_o_app() {
        // 1 contra 8: -87,5% arredonda para -87 (Math.round), nao -88.
        let ds = dataset_of(vec![
            spend("n", 1, "2026-09-01"),
            spend("o", 8, "2026-08-01"),
        ]);
        assert_eq!(
            spending_pace(&ds, "2026-09", "2026-09-24").change_percent,
            Some(-87)
        );
    }

    #[test]
    fn mes_curto_usa_ultimo_dia_no_normal() {
        // Marco contra fevereiro (28 dias): do dia 28 ao 31 o normal fica no total de fevereiro.
        let ds = dataset_of(vec![
            spend("f1", 700, "2026-02-10"),
            spend("f2", 300, "2026-02-28"),
        ]);
        let p = spending_pace(&ds, "2026-03", "2026-03-31");
        assert_eq!(p.normal.len(), 31);
        assert_eq!(&p.normal[27..], &[1000, 1000, 1000, 1000]);
    }

    #[test]
    fn normal_no_dia_19() {
        // 3.2: so outubro tem dado; acumulado de outubro ate o dia 19.
        let p = spending_pace(&fixtures_grafite(), "2026-11", GRAFITE_TODAY);
        assert_eq!(p.reference_months, ["2026-10"]);
        assert_eq!(p.spent_minor, 311_341);
        assert_eq!(p.normal_at_cutoff_minor, Some(270_500));
    }

    #[test]
    fn ritmo_15_por_cento_acima() {
        let p = spending_pace(&fixtures_grafite(), "2026-11", GRAFITE_TODAY);
        assert_eq!(p.change_percent, Some(15));
    }

    #[test]
    fn normal_no_fim() {
        // Normal no dia 30 de novembro = outubro inteiro.
        let p = spending_pace(&fixtures_grafite(), "2026-11", GRAFITE_TODAY);
        assert_eq!(p.normal.last(), Some(&366_200));
    }

    // --- insights.test.ts: monthComparison (M3) ---

    #[test]
    fn saldo_compara_com_a_media_dos_3_anteriores() {
        let ds = dataset_of(vec![
            income("a", 10_000, "2026-09-02"),
            spend("b", 3000, "2026-09-10"),
            income("c", 8000, "2026-08-02"),
            spend("d", 4000, "2026-08-20"),
            income("e", 5000, "2026-07-02"),
            spend("f", 1000, "2026-06-02"),
        ]);
        // (-1000 + 5000 + 4000) / 3 = 2666,67.
        assert_eq!(normal_balance(&ds, "2026-09"), Some(2667));
    }

    #[test]
    fn saldo_divide_so_pelos_meses_com_dado() {
        let ds = dataset_of(vec![
            income("a", 10_000, "2026-09-02"),
            income("c", 8000, "2026-08-02"),
            spend("d", 4000, "2026-08-20"),
        ]);
        assert_eq!(normal_balance(&ds, "2026-09"), Some(4000));
    }

    #[test]
    fn saldo_atravessa_a_virada_do_ano() {
        let ds = dataset_of(vec![spend("a", 500, "2025-12-31")]);
        assert_eq!(normal_balance(&ds, "2026-01"), Some(-500));
    }

    #[test]
    fn saldo_sem_mes_de_referencia_nao_compara() {
        let ds = dataset_of(vec![spend("a", 500, "2026-09-02")]);
        assert_eq!(normal_balance(&ds, "2026-09"), None);
    }

    #[test]
    fn saldo_da_fixture_e_o_normal_de_outubro() {
        assert_eq!(
            normal_balance(&fixtures_grafite(), "2026-11"),
            Some(141_029)
        );
    }

    // --- insights.test.ts: averageSurplus (M2) ---

    fn mt(month: &str, income: i64, expense: i64) -> MonthTotals {
        MonthTotals {
            month: month.into(),
            income_minor: income,
            expense_minor: expense,
        }
    }

    fn avg(average_minor: i64, months: usize) -> Option<SurplusAverage> {
        Some(SurplusAverage {
            average_minor,
            months,
        })
    }

    #[test]
    fn sobra_media_so_de_meses_fechados_com_dado() {
        let months = [mt("2026-08", 1000, 400), mt("2026-09", 1000, 800)];
        assert_eq!(
            average_surplus_closed(&months, Some("2026-08"), "2026-10"),
            avg(400, 2)
        );
    }

    #[test]
    fn sobra_media_nao_conta_mes_antes_do_primeiro_lancamento() {
        let months = [
            mt("2026-06", 0, 0),
            mt("2026-07", 0, 0),
            mt("2026-08", 1000, 400),
            mt("2026-09", 1000, 800),
        ];
        assert_eq!(
            average_surplus_closed(&months, Some("2026-08"), "2026-10"),
            avg(400, 2)
        );
    }

    #[test]
    fn sobra_media_nao_conta_o_mes_corrente() {
        let months = [
            mt("2026-08", 1000, 400),
            mt("2026-09", 1000, 800),
            mt("2026-10", 5000, 0),
        ];
        assert_eq!(
            average_surplus_closed(&months, Some("2026-08"), "2026-10"),
            avg(400, 2)
        );
    }

    #[test]
    fn sobra_media_mes_vazio_depois_do_primeiro_lancamento_conta_como_zero() {
        let months = [
            mt("2026-07", 1000, 400),
            mt("2026-08", 0, 0),
            mt("2026-09", 1000, 800),
        ];
        assert_eq!(
            average_surplus_closed(&months, Some("2026-07"), "2026-10"),
            avg(267, 3)
        );
    }

    #[test]
    fn sobra_media_sem_mes_fechado_nao_mostra_media() {
        assert_eq!(
            average_surplus_closed(&[mt("2026-10", 1000, 400)], Some("2026-10"), "2026-10"),
            None
        );
        assert_eq!(
            average_surplus_closed(&[mt("2026-09", 0, 0)], None, "2026-10"),
            None
        );
        assert_eq!(
            average_surplus_closed(&[], Some("2026-08"), "2026-10"),
            None
        );
    }
}
