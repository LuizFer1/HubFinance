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

/// Port de `dailyExpenses` mantido como vetor de paridade com o app: o Dashboard usa
/// `expense_by_day` direto, mas os casos do TS continuam conferidos aqui.
#[cfg(test)]
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
#[cfg(test)]
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

/// Quantos meses fechados entram na sobra media: os 12 anteriores ao mes de hoje.
pub const SURPLUS_MONTHS: i32 = 12;

/// `averageSurplus` do app (M2): media da sobra so dos meses **fechados com dado** de `months`
/// (do primeiro lancamento ate o anterior a `current_month`) e no maximo dos 12 anteriores a
/// `current_month` (o mes de **hoje**, nao o do seletor: o numero nao muda ao navegar). O mes
/// corrente pela metade sempre pareceria ter sobrado muito; mes antes do primeiro lancamento
/// diluiria a media. `None` sem nenhum mes fechado.
pub fn average_surplus_closed(
    months: &[MonthTotals],
    first_month: Option<&str>,
    current_month: &str,
) -> Option<SurplusAverage> {
    let first = first_month?;
    let oldest = shift_month(current_month, -SURPLUS_MONTHS);
    let closed: Vec<&MonthTotals> = months
        .iter()
        .filter(|m| {
            let m = m.month.as_str();
            m >= first && m >= oldest.as_str() && m < current_month
        })
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

/// Ate este dia a media diaria e instavel demais: nao ha previsao, fica o ritmo.
pub const FORECAST_HIDDEN_THROUGH_DAY: usize = 5;

/// Alvo da taxa de poupanca, em porcentagem da renda.
pub const SAVINGS_TARGET_PERCENT: i64 = 20;

/// Passo do teto do eixo do grafico de ritmo: R$ 2.500.
pub const PACE_AXIS_STEP: i64 = 250_000;

/// Previsao de fechamento do mes corrente (`Forecast` do app, spec 5.6 e 6.2.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Forecast {
    /// `YYYY-MM` de hoje: so o mes corrente tem previsao.
    pub month: String,
    pub day: usize,
    pub days_in_month: usize,
    /// Dias depois de hoje ate o fim do mes.
    pub remaining_days: usize,
    /// Lancado no mes **inteiro**: um lancamento datado para frente ja e conhecido e conta.
    pub income_minor: i64,
    pub expense_minor: i64,
    /// Despesa avulsa (sem serie) de hoje para tras: a base da media.
    pub casual_minor: i64,
    /// "Sua media ate aqui": `casual / dia`.
    pub casual_daily_minor: i64,
    /// Ocorrencias recorrentes ainda sem linha no mes, vencidas ou por vir.
    pub upcoming_income_minor: i64,
    pub upcoming_expense_minor: i64,
    /// A media avulsa vezes os dias que faltam.
    pub projected_casual_minor: i64,
    pub expected_income_minor: i64,
    pub expected_expense_minor: i64,
    /// Negativo = vai faltar.
    pub expected_surplus_minor: i64,
    /// Taxa de poupanca no ritmo atual; `None` sem receita prevista.
    pub expected_savings_rate: Option<i64>,
}

/// Taxa de poupanca em porcentagem inteira, `(receita - despesa) / receita` (`savingsRate`).
/// Guardar em reserva nao abate: e parte do que sobrou. Sem receita, `None`.
pub fn savings_rate(income_minor: i64, expense_minor: i64) -> Option<i64> {
    (income_minor > 0).then(|| {
        js_round(income_minor.saturating_sub(expense_minor) as f64 / income_minor as f64 * 100.0)
            as i64
    })
}

/// Valor de previsao para a tela ("~R$ 960", `roundToTens`): o modulo arredondado a dezena de
/// reais (meio para cima), com o sinal de volta e nunca `-0`. Centavo em previsao e falsa
/// precisao; arredondar o modulo deixa -R$ 5 e +R$ 5 simetricos.
pub fn round_to_tens(minor: i64) -> i64 {
    let rounded = div_round(minor.saturating_abs(), 1_000).saturating_mul(1_000);
    if minor < 0 { -rounded } else { rounded }
}

/// `forecast` do app: se o mes continuar no ritmo de hoje, como ele fecha.
///
/// A media e so do **avulso** (`recurrenceId` nulo): a serie "valor varia" ja entra pela
/// estimativa dela na projecao, e soma-la na media contaria duas vezes. E so do que ja
/// aconteceu (`occurredOn <= hoje`). O "por vir" e `project_month`: pega a ocorrencia vencida que
/// o celular ainda nao sincronizou uma vez, pelo valor que a materializacao vai gravar.
/// `variavel_projetado = div_round(A x restantes, dia)` multiplica antes de dividir, como o app
/// (arredondar a media e multiplicar daria 129536 em vez de 129534 na fixture). `None` ate o
/// dia 5 e com `today` fora da forma.
pub fn forecast(ds: &Dataset, today: &str) -> Option<Forecast> {
    let month = month_of(today);
    let day = day_of(today);
    let days = usize::from(days_in_month(&month));
    if day <= FORECAST_HIDDEN_THROUGH_DAY || days == 0 || day > days {
        return None;
    }
    let remaining_days = days - day;
    let launched = super::aggregate::totals(in_month(ds, &month));
    let casual_minor = in_month(ds, &month)
        .filter(|t| t.kind == Kind::Expense && t.recurrence_id.is_none() && t.counts_in_totals())
        .filter(|t| t.occurred_on.as_str() <= today)
        .fold(0i64, |acc, t| acc.saturating_add(t.amount_minor));
    let (mut upcoming_income_minor, mut upcoming_expense_minor) = (0i64, 0i64);
    for item in super::recurrence::project_month(ds, &month) {
        match item.kind {
            Kind::Income => {
                upcoming_income_minor = upcoming_income_minor.saturating_add(item.amount_minor)
            }
            Kind::Expense => {
                upcoming_expense_minor = upcoming_expense_minor.saturating_add(item.amount_minor)
            }
        }
    }
    let as_i64 = |n: usize| i64::try_from(n).unwrap_or(i64::MAX);
    let projected_casual_minor = div_round(
        casual_minor.saturating_mul(as_i64(remaining_days)),
        as_i64(day),
    );
    let expected_income_minor = launched.income_minor.saturating_add(upcoming_income_minor);
    let expected_expense_minor = launched
        .expense_minor
        .saturating_add(upcoming_expense_minor)
        .saturating_add(projected_casual_minor);
    Some(Forecast {
        month,
        day,
        days_in_month: days,
        remaining_days,
        income_minor: launched.income_minor,
        expense_minor: launched.expense_minor,
        casual_minor,
        casual_daily_minor: div_round(casual_minor, as_i64(day)),
        upcoming_income_minor,
        upcoming_expense_minor,
        projected_casual_minor,
        expected_income_minor,
        expected_expense_minor,
        expected_surplus_minor: expected_income_minor.saturating_sub(expected_expense_minor),
        expected_savings_rate: savings_rate(expected_income_minor, expected_expense_minor),
    })
}

/// "Para fechar no zero" (`zeroAllowance`): quanto da para gastar por dia ate o fim do mes sem a
/// sobra ficar negativa, contando so o que ja se sabe (lancado + recorrentes por vir). **Piso** em
/// reais inteiros (um teto de gasto superestimado faria fechar no vermelho seguindo o conselho),
/// numa divisao so por `dias x 100`; `div_euclid` e o `Math.floor` do app tambem no negativo.
/// `None` no ultimo dia; `<= 0` = as contas que ainda vem ja passam do saldo.
pub fn zero_allowance(f: &Forecast) -> Option<i64> {
    if f.remaining_days == 0 {
        return None;
    }
    let available = f
        .income_minor
        .saturating_sub(f.expense_minor)
        .saturating_add(f.upcoming_income_minor)
        .saturating_sub(f.upcoming_expense_minor);
    let den = i64::try_from(f.remaining_days)
        .unwrap_or(i64::MAX)
        .saturating_mul(100);
    Some(available.div_euclid(den).saturating_mul(100))
}

/// Teto do eixo do grafico de ritmo (spec 2.2, inventada): o maior valor desenhado para cima em
/// multiplos de R$ 2.500, no minimo R$ 2.500. Quem chama passa previsao, normal no fim, renda
/// **e o gasto ate o corte**: num mes fechado acima da renda e do normal a linha do mes nao
/// pode sair do grafico (decisao H2c).
pub fn pace_axis_top(values: &[i64]) -> i64 {
    let peak = values.iter().copied().max().unwrap_or(0).max(1);
    peak.saturating_add(PACE_AXIS_STEP - 1)
        .div_euclid(PACE_AXIS_STEP)
        .saturating_mul(PACE_AXIS_STEP)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FixedVariable {
    /// Despesa de serie viva de valor fixo (inclui parcelas).
    pub fixed_minor: i64,
    /// O resto: avulsos e series "valor varia".
    pub variable_minor: i64,
}

/// "R$ 876 fixos · R$ 2.237 variaveis" (`fixedVariable`, spec 6.2.7). Fixa = despesa que conta,
/// de serie **viva** sem `variable: true` — inclusive pausada e de app antigo sem `rule` (por
/// isso le `variable`/`deleted`, nunca `rule`; decisao 4.7). Serie apagada ou ausente do dataset
/// (ainda nao sincronizada, ou fora do contrato) conta como variavel: nao ha como saber.
pub fn fixed_variable(ds: &Dataset, month: &str) -> FixedVariable {
    let mut out = FixedVariable::default();
    for t in in_month(ds, month).filter(|t| t.kind == Kind::Expense && t.counts_in_totals()) {
        let fixed = t
            .recurrence_id
            .as_deref()
            .and_then(|id| ds.recurrences.get(id))
            .is_some_and(|r| !r.deleted && !r.variable);
        let slot = if fixed {
            &mut out.fixed_minor
        } else {
            &mut out.variable_minor
        };
        *slot = slot.saturating_add(t.amount_minor);
    }
    out
}

/// Acima do normal so a partir de R$ 50 a mais (alem dos +25%).
pub const ABOVE_NORMAL_MIN_MINOR: i64 = 5_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CategoryNormal {
    /// Gasto da categoria no mes, ate o dia de hoje no mes corrente.
    pub spent_minor: i64,
    /// Media dos meses de referencia no mesmo recorte, arredondada ao centavo.
    pub normal_minor: i64,
    /// `(gasto / normal - 1) x 100` com sinal.
    pub change_percent: i64,
}

/// Despesa por chave de linha de "Para onde foi" (`expense_by_category`: id da categoria viva,
/// ou `NO_CATEGORY_KEY`), so dos dias `<= through_day`.
fn by_category_through(
    ds: &Dataset,
    month: &str,
    through_day: usize,
) -> std::collections::BTreeMap<String, i64> {
    let mut sums = std::collections::BTreeMap::new();
    for t in in_month(ds, month).filter(|t| t.kind == Kind::Expense && t.counts_in_totals()) {
        if day_of(&t.occurred_on) > through_day {
            continue;
        }
        let key = ds
            .find_category(t.category_id.as_deref())
            .map_or(super::aggregate::NO_CATEGORY_KEY, |c| c.id.as_str());
        let sum: &mut i64 = sums.entry(key.to_string()).or_default();
        *sum = sum.saturating_add(t.amount_minor);
    }
    sums
}

/// `categoriesAboveNormal` do app (spec 5.8 e 6.2.9): as linhas que passaram do "seu normal",
/// pela chave da linha. Mesma referencia do ritmo, dividindo pelos meses de referencia. Mes
/// corrente: gasto e normal param no dia de hoje; mes fechado: mes inteiro contra mes inteiro
/// (o de 31 dias entra inteiro); mes futuro: nada. O normal e arredondado ao centavo **antes**
/// dos limites; `gasto x 4 >= normal x 5` e o +25% sem ponto flutuante; normal `<= 0` nao marca.
pub fn categories_above_normal(
    ds: &Dataset,
    month: &str,
    today: &str,
) -> std::collections::BTreeMap<String, CategoryNormal> {
    let mut marked = std::collections::BTreeMap::new();
    let this_month = month_of(today);
    if month > this_month.as_str() {
        return marked;
    }
    let current = month == this_month;
    let through_day = if current {
        day_of(today)
    } else {
        usize::from(days_in_month(month))
    };
    let months = reference_months(ds, month);
    if months.is_empty() {
        return marked;
    }
    let n = i64::try_from(months.len()).unwrap_or(i64::MAX);
    let references: Vec<_> = months
        .iter()
        .map(|m| by_category_through(ds, m, if current { through_day } else { 31 }))
        .collect();
    for (key, spent_minor) in by_category_through(ds, month, through_day) {
        let sum = references.iter().fold(0i64, |acc, sums| {
            acc.saturating_add(sums.get(&key).copied().unwrap_or(0))
        });
        let normal_minor = div_round(sum, n);
        if normal_minor <= 0
            || spent_minor.saturating_mul(4) < normal_minor.saturating_mul(5)
            || spent_minor.saturating_sub(normal_minor) < ABOVE_NORMAL_MIN_MINOR
        {
            continue;
        }
        let Some(change_percent) = change_percent(spent_minor, normal_minor) else {
            continue;
        };
        marked.insert(
            key,
            CategoryNormal {
                spent_minor,
                normal_minor,
                change_percent,
            },
        );
    }
    marked
}

/// "O plano cabe na renda?" (spec 2.3 e 7.13, inventada; so no hub).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlanFit {
    /// `recurring_month(mes).income_minor`: a renda fixa do mes.
    pub fixed_income_minor: i64,
    /// Soma dos limites das metas vigentes (uma por categoria viva).
    pub budgets_minor: i64,
    /// Gasto do mes em categoria sem meta, inclusive "Sem categoria" (`unbudgeted` do app).
    pub unbudgeted_minor: i64,
    /// `renda fixa - metas - sem meta`; negativo quando o plano passa da renda.
    pub leftover_minor: i64,
    /// Metas e sem meta em % da renda fixa; `None` sem renda fixa.
    pub budgets_percent: Option<i64>,
    pub unbudgeted_percent: Option<i64>,
    /// O que as reservas vivas pedem por mes (soma de `recurring.amountMinor > 0`).
    pub reserves_ask_minor: i64,
    /// Quantas reservas pedem ("A reserva pede" / "As reservas pedem").
    pub reserves_asking: usize,
}

pub fn plan_fit(ds: &Dataset, month: &str) -> PlanFit {
    let fixed_income_minor = super::recurrence::recurring_month(ds, month).income_minor;
    let budgets = super::budgets::current_budgets(ds);
    let budgets_minor = budgets
        .iter()
        .fold(0i64, |acc, (b, _)| acc.saturating_add(b.limit_minor));
    let budgeted: Vec<&str> = budgets.iter().map(|(_, id)| *id).collect();
    let unbudgeted_minor = in_month(ds, month)
        .filter(|t| t.kind == Kind::Expense && t.counts_in_totals())
        .filter(|t| {
            ds.find_category(t.category_id.as_deref())
                .is_none_or(|c| !budgeted.contains(&c.id.as_str()))
        })
        .fold(0i64, |acc, t| acc.saturating_add(t.amount_minor));
    let percent = |part: i64| {
        (fixed_income_minor > 0)
            .then(|| js_round(part as f64 / fixed_income_minor as f64 * 100.0) as i64)
    };
    let asks: Vec<i64> = ds
        .alive_reserves()
        .filter_map(|r| r.recurring_amount_minor.filter(|v| *v > 0))
        .collect();
    PlanFit {
        fixed_income_minor,
        budgets_minor,
        unbudgeted_minor,
        leftover_minor: fixed_income_minor
            .saturating_sub(budgets_minor)
            .saturating_sub(unbudgeted_minor),
        budgets_percent: percent(budgets_minor),
        unbudgeted_percent: percent(unbudgeted_minor),
        reserves_ask_minor: asks.iter().fold(0i64, |acc, v| acc.saturating_add(*v)),
        reserves_asking: asks.len(),
    }
}

/// Quem e o autor de um grupo de "Quem lancou" (spec 7.12).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Author {
    /// Perfil vivo: nome, cor e avatar vem do `User`.
    Person {
        id: String,
        name: String,
        color: String,
    },
    /// Lancamento sem `userId`: "Sem autor".
    Nobody,
    /// `userId` de perfil apagado (ou ausente): "Perfil removido".
    Removed,
}

impl Author {
    pub fn label(&self) -> &str {
        match self {
            Author::Person { name, .. } => name,
            Author::Nobody => "Sem autor",
            Author::Removed => "Perfil removido",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersonTotals {
    pub author: Author,
    pub expense_minor: i64,
    pub income_minor: i64,
}

/// "Quem lancou" do mes (spec 6.8 e 7.12): despesas e receitas que contam, por autor. Perfis
/// removidos colapsam num grupo so (como `find_user`: distinguir vazaria "esse perfil existiu").
/// Maior despesa primeiro, depois o rotulo. Com menos de dois grupos devolve vazio: o card some
/// ("para combinar a divisao da casa" nao tem o que combinar com uma pessoa so).
pub fn by_person(ds: &Dataset, month: &str) -> Vec<PersonTotals> {
    let mut groups: Vec<PersonTotals> = Vec::new();
    for t in in_month(ds, month).filter(|t| t.counts_in_totals()) {
        let author = match (t.user_id.as_deref(), ds.find_user(t.user_id.as_deref())) {
            (None, _) => Author::Nobody,
            (Some(_), Some(u)) => Author::Person {
                id: u.id.clone(),
                name: u.name.clone(),
                color: u.color.clone(),
            },
            (Some(_), None) => Author::Removed,
        };
        let index = match groups.iter().position(|g| g.author == author) {
            Some(i) => i,
            None => {
                groups.push(PersonTotals {
                    author,
                    expense_minor: 0,
                    income_minor: 0,
                });
                groups.len() - 1
            }
        };
        if let Some(g) = groups.get_mut(index) {
            match t.kind {
                Kind::Expense => g.expense_minor = g.expense_minor.saturating_add(t.amount_minor),
                Kind::Income => g.income_minor = g.income_minor.saturating_add(t.amount_minor),
            }
        }
    }
    if groups.len() < 2 {
        return Vec::new();
    }
    let id_of = |a: &Author| match a {
        Author::Person { id, .. } => id.clone(),
        _ => String::new(),
    };
    groups.sort_by(|a, b| {
        b.expense_minor
            .cmp(&a.expense_minor)
            .then_with(|| a.author.label().cmp(b.author.label()))
            .then_with(|| id_of(&a.author).cmp(&id_of(&b.author)))
    });
    groups
}

/// A partir de quantos meses de historico o card vira grafico de 12 meses (spec 7.5).
pub const HISTORY_CHART_FROM: usize = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryRow {
    pub month: String,
    pub income_minor: i64,
    pub expense_minor: i64,
    pub surplus_minor: i64,
    /// Taxa de poupanca do mes; `None` sem receita.
    pub savings_rate: Option<i64>,
    /// O mes e do primeiro lancamento em diante (antes dele a coluna fica vazia: "nao existia").
    pub existed: bool,
    /// Mes de hoje: barras em contorno, "parcial, ate dia N".
    pub partial: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct History {
    /// Meses do primeiro lancamento vivo ate o escolhido, inclusive (0 sem lancamento).
    pub months_of_history: usize,
    /// Os 12 meses que terminam no escolhido, o mais antigo primeiro.
    pub rows: Vec<HistoryRow>,
    /// Sobra media dos meses fechados com dado dos 12 anteriores ao mes de hoje (`None`: "Ainda
    /// nao ha mes fechado").
    pub average: Option<SurplusAverage>,
}

impl History {
    /// `< 4` meses: lista; `>= 4`: grafico de 12 meses.
    pub fn is_chart(&self) -> bool {
        self.months_of_history >= HISTORY_CHART_FROM
    }
}

/// Card "Historico" (spec 2.2 e 7.5).
pub fn history(ds: &Dataset, month: &str, today: &str) -> History {
    let first = first_month(ds);
    let this_month = month_of(today);
    let window = super::periods::last_months(month, 12);
    let totals = super::aggregate::monthly_totals(ds, &window);
    let months_of_history = first.as_deref().filter(|f| *f <= month).map_or(0, |f| {
        usize::try_from(super::periods::months_between(f, month).saturating_add(1)).unwrap_or(0)
    });
    // A media tem a janela propria, ancorada em hoje: os 12 fechados antes do mes corrente.
    let closed = super::aggregate::monthly_totals(
        ds,
        &super::periods::last_months(&shift_month(&this_month, -1), 12),
    );
    History {
        months_of_history,
        average: average_surplus_closed(&closed, first.as_deref(), &this_month),
        rows: totals
            .iter()
            .map(|t| HistoryRow {
                month: t.month.clone(),
                income_minor: t.income_minor,
                expense_minor: t.expense_minor,
                surplus_minor: t.net_minor(),
                savings_rate: savings_rate(t.income_minor, t.expense_minor),
                existed: first.as_deref().is_some_and(|f| t.month.as_str() >= f),
                partial: t.month == this_month,
            })
            .collect(),
    }
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

    #[test]
    fn com_8_meses_fechados_a_media_cobre_os_8() {
        // Marco a outubro, sobras de 1000 a 8000: (1000 + ... + 8000) / 8 = 4500.
        let mut months: Vec<MonthTotals> = crate::dashboard::periods::last_months("2026-10", 8)
            .into_iter()
            .zip(1..)
            .map(|(m, i)| MonthTotals {
                month: m,
                income_minor: i * 1000,
                expense_minor: 0,
            })
            .collect();
        months.push(mt("2026-11", 99_999, 0));
        assert_eq!(
            average_surplus_closed(&months, Some("2026-03"), "2026-11"),
            avg(4500, 8)
        );
    }

    #[test]
    fn com_14_meses_fechados_so_os_ultimos_12_contam() {
        // Setembro e outubro de 2025 ficam antes de novembro - 12: as sobras enormes deles nao
        // podem puxar a media.
        let months: Vec<MonthTotals> = crate::dashboard::periods::last_months("2026-10", 14)
            .into_iter()
            .map(|m| MonthTotals {
                income_minor: if m.as_str() < "2025-11" {
                    1_000_000
                } else {
                    1000
                },
                month: m,
                expense_minor: 0,
            })
            .collect();
        assert_eq!(
            average_surplus_closed(&months, Some("2025-09"), "2026-11"),
            avg(1000, 12)
        );
    }

    #[test]
    fn sobra_media_nao_muda_com_o_mes_escolhido() {
        // A janela e a de hoje (12 fechados antes do mes corrente), nao a do seletor: escolher
        // um mes antigo nao pode tirar da media os meses fechados que vieram depois dele.
        let ds = dataset_of(vec![
            income("i1", 10_000, "2026-08-05"),
            income("i2", 4000, "2026-09-05"),
            income("i3", 1000, "2026-10-05"),
        ]);
        let expected = avg(5000, 3);
        for month in ["2026-08", "2026-09", "2026-10", "2026-11"] {
            assert_eq!(
                history(&ds, month, GRAFITE_TODAY).average,
                expected,
                "{month}"
            );
        }
    }

    // --- forecast.test.ts (M4) e tabela 3.2 ---

    use crate::dashboard::dataset::RECURRENCES;
    use crate::dashboard::{GrafiteRow, dataset_from, grafite_rows};

    /// A fixture sem as linhas `drop` (o `delete` dos testes do app) e com `add`.
    fn grafite_edit(drop: &[&str], add: Vec<GrafiteRow>) -> Dataset {
        let mut rows: Vec<GrafiteRow> = grafite_rows()
            .into_iter()
            .filter(|(_, id, _)| !drop.contains(&id.as_str()))
            .collect();
        rows.extend(add);
        dataset_from(rows)
    }

    fn expense_row(id: &str, amount: i64, on: &str, extra: Value) -> GrafiteRow {
        let mut data = json!({
            "kind": "expense", "description": "Extra", "amountMinor": amount, "occurredOn": on,
        });
        if let (Some(b), Some(o)) = (data.as_object_mut(), extra.as_object()) {
            for (k, v) in o {
                b.insert(k.clone(), v.clone());
            }
        }
        (TRANSACTIONS, id.to_string(), data)
    }

    /// Serie "valor varia" Luz, R$ 120, dia 12 de novembro (spec 5.9).
    fn luz(materialized: bool) -> Vec<GrafiteRow> {
        let mut rows = vec![(
            RECURRENCES,
            "RLUZ".to_string(),
            json!({
                "frequency": "monthly", "kind": "expense", "description": "Luz",
                "amountMinor": 12_000, "categoryId": "CAT-CONTAS", "scheduleType": "dayOfMonth",
                "scheduleN": 12, "startOn": "2026-11-12", "endOn": null, "active": true,
                "variable": true, "userId": "UL",
            }),
        )];
        if materialized {
            rows.push(expense_row(
                &crate::dashboard::ids::stable_entity_id_v1("RLUZ:2026-11"),
                12_000,
                "2026-11-12",
                json!({ "categoryId": "CAT-CONTAS", "recurrenceId": "RLUZ",
                        "occurrenceKey": "RLUZ:2026-11", "userId": "UL" }),
            ));
        }
        rows
    }

    fn forecast_of(ds: &Dataset, today: &str) -> Forecast {
        forecast(ds, today).expect("previsao")
    }

    fn grafite_forecast() -> Forecast {
        forecast_of(&fixtures_grafite(), GRAFITE_TODAY)
    }

    #[test]
    fn hoje_e_dia_19_de_30_faltam_11_dias() {
        let f = grafite_forecast();
        assert_eq!(
            (f.month.as_str(), f.day, f.days_in_month, f.remaining_days),
            ("2026-11", 19, 30, 11)
        );
    }

    #[test]
    fn saldo_ate_hoje() {
        // 3.2: 350000 - 311341 = 38659 -> "R$ 386,59".
        let f = grafite_forecast();
        assert_eq!((f.income_minor, f.expense_minor), (350_000, 311_341));
        assert_eq!(f.income_minor - f.expense_minor, 38_659);
    }

    #[test]
    fn taxa_de_poupanca_ate_agora() {
        let f = grafite_forecast();
        assert_eq!(savings_rate(f.income_minor, f.expense_minor), Some(11));
    }

    #[test]
    fn media_avulsa() {
        let f = grafite_forecast();
        assert_eq!((f.casual_minor, f.casual_daily_minor), (223_741, 11_776));
    }

    #[test]
    fn recorrentes_por_vir() {
        let f = grafite_forecast();
        assert_eq!(
            (f.upcoming_income_minor, f.upcoming_expense_minor),
            (0, 5_052)
        );
    }

    #[test]
    fn variavel_projetado() {
        assert_eq!(grafite_forecast().projected_casual_minor, 129_534);
    }

    #[test]
    fn despesa_prevista() {
        let f = grafite_forecast();
        assert_eq!(f.expected_income_minor, 350_000);
        assert_eq!(f.expected_expense_minor, 445_927);
        assert_eq!(round_to_tens(f.expected_expense_minor), 446_000);
    }

    #[test]
    fn sobra_prevista() {
        let f = grafite_forecast();
        assert_eq!(f.expected_surplus_minor, -95_927);
        assert_eq!(round_to_tens(f.expected_surplus_minor), -96_000);
    }

    #[test]
    fn taxa_no_ritmo() {
        assert_eq!(grafite_forecast().expected_savings_rate, Some(-27));
    }

    #[test]
    fn fechar_no_zero() {
        // Piso: (38659 - 5052) / 11 = 30,55 -> R$ 30 (arredondando daria 31).
        assert_eq!(zero_allowance(&grafite_forecast()), Some(3_000));
    }

    #[test]
    fn teto_do_eixo() {
        let f = grafite_forecast();
        let p = spending_pace(&fixtures_grafite(), "2026-11", GRAFITE_TODAY);
        let normal_end = p.normal.last().copied().unwrap_or(0);
        assert_eq!(
            pace_axis_top(&[
                f.expected_expense_minor,
                normal_end,
                f.expected_income_minor,
                p.spent_minor
            ]),
            500_000
        );
        assert_eq!(pace_axis_top(&[]), 250_000);
        assert_eq!(pace_axis_top(&[250_000]), 250_000);
        assert_eq!(pace_axis_top(&[250_001]), 500_000);
    }

    #[test]
    fn previsao_some_ate_o_dia_5() {
        let ds = fixtures_grafite();
        assert!(forecast(&ds, "2026-11-01").is_none());
        assert!(forecast(&ds, "2026-11-05").is_none());
        assert!(forecast(&ds, "2026-11-06").is_some());
    }

    #[test]
    fn valor_varia_materializado_e_variavel_mas_nao_avulso() {
        let ds = grafite_edit(&[], luz(true));
        let f = forecast_of(&ds, GRAFITE_TODAY);
        assert_eq!((f.casual_minor, f.casual_daily_minor), (223_741, 11_776));
        assert_eq!(f.expense_minor, 311_341 + 12_000);
        // Ja gravada: a projecao nao a estima de novo.
        assert_eq!(f.upcoming_expense_minor, 5_052);
        let fv = fixed_variable(&ds, "2026-11");
        assert_eq!((fv.fixed_minor, fv.variable_minor), (87_600, 235_741));
    }

    #[test]
    fn valor_varia_ainda_sem_linha_entra_na_previsao_pela_estimativa() {
        let f = forecast_of(&grafite_edit(&[], luz(false)), GRAFITE_TODAY);
        assert_eq!(f.casual_minor, 223_741);
        assert_eq!(f.expense_minor, 311_341);
        assert_eq!(f.upcoming_expense_minor, 5_052 + 12_000);
    }

    #[test]
    fn lancamento_com_data_futura_no_mes_soma_em_d_e_nao_na_media() {
        let ds = grafite_edit(
            &[],
            vec![expense_row(
                "1125-1-FUTURO",
                10_000,
                "2026-11-25",
                json!({}),
            )],
        );
        let f = forecast_of(&ds, GRAFITE_TODAY);
        assert_eq!(f.expense_minor, 321_341);
        assert_eq!(f.casual_minor, 223_741);
        assert_eq!(f.projected_casual_minor, 129_534);
        assert_eq!(f.expected_expense_minor, 455_927);
    }

    #[test]
    fn vencida_sem_linha_entra_uma_vez_pelo_valor_da_serie() {
        let ds = grafite_edit(&["FBCWZFMRQBMWQZCRFVCCZZM8QV"], vec![]);
        let f = forecast_of(&ds, GRAFITE_TODAY);
        assert_eq!(f.expense_minor, 311_341 - 53_901);
        assert_eq!(f.upcoming_expense_minor, 5_052 + 53_901);
        assert_eq!(f.expected_expense_minor, 445_927);
    }

    #[test]
    fn sem_receita_nem_lancada_nem_por_vir_nao_tem_taxa_no_ritmo() {
        let ds = grafite_edit(&["7748FB44FQCR7BWM7Q4RFV4MF7", "RSAL"], vec![]);
        let f = forecast_of(&ds, GRAFITE_TODAY);
        assert_eq!(f.expected_income_minor, 0);
        assert_eq!(f.expected_savings_rate, None);
    }

    #[test]
    fn transferencia_e_oculto_nao_contam_na_previsao() {
        let ds = grafite_edit(
            &[],
            vec![expense_row(
                "1110-1-TRANSF",
                99_999,
                "2026-11-10",
                json!({ "reportState": "transfer" }),
            )],
        );
        let f = forecast_of(&ds, GRAFITE_TODAY);
        assert_eq!((f.expense_minor, f.casual_minor), (311_341, 223_741));
    }

    #[test]
    fn fechar_no_zero_sem_dias_restantes() {
        let f = forecast_of(&fixtures_grafite(), "2026-11-30");
        assert_eq!(f.remaining_days, 0);
        assert_eq!(zero_allowance(&f), None);
    }

    #[test]
    fn contas_por_vir_acima_do_saldo_dao_zero_ou_menos() {
        let ds = grafite_edit(
            &[],
            vec![expense_row(
                "1118-3-GRANDE",
                40_000,
                "2026-11-18",
                json!({}),
            )],
        );
        let allowance = zero_allowance(&forecast_of(&ds, GRAFITE_TODAY));
        assert!(allowance.is_some_and(|a| a <= 0), "{allowance:?}");
    }

    #[test]
    fn savings_rate_sem_receita_negativa_e_outubro() {
        assert_eq!(savings_rate(0, 0), None);
        assert_eq!(savings_rate(0, 5_000), None);
        assert_eq!(savings_rate(1_000, 1_500), Some(-50));
        assert_eq!(savings_rate(507_229, 366_200), Some(28));
    }

    #[test]
    fn round_to_tens_arredonda_o_modulo_e_mantem_o_sinal() {
        assert_eq!(round_to_tens(95_927), 96_000);
        assert_eq!(round_to_tens(-95_927), -96_000);
        assert_eq!(round_to_tens(-500), -1_000);
        assert_eq!(round_to_tens(499), 0);
        assert_eq!(round_to_tens(-499), 0);
        assert_eq!(round_to_tens(0), 0);
    }

    // --- fixed-variable.test.ts (M5) ---

    fn series_edit(id: &str, over: Value) -> GrafiteRow {
        let (table, id, mut data) = grafite_rows()
            .into_iter()
            .find(|(_, row_id, _)| row_id == id)
            .expect("serie da fixture");
        if let (Some(b), Some(o)) = (data.as_object_mut(), over.as_object()) {
            for (k, v) in o {
                b.insert(k.clone(), v.clone());
            }
        }
        (table, id, data)
    }

    #[test]
    fn fixos_e_variaveis() {
        // 3.2: "R$ 876 fixos · R$ 2.237 variaveis".
        let fv = fixed_variable(&fixtures_grafite(), "2026-11");
        assert_eq!((fv.fixed_minor, fv.variable_minor), (87_600, 223_741));
    }

    #[test]
    fn outubro_so_a_cadeira_e_fixa() {
        let fv = fixed_variable(&fixtures_grafite(), "2026-10");
        assert_eq!((fv.fixed_minor, fv.variable_minor), (5_052, 361_148));
    }

    #[test]
    fn recorrencia_apagada_conta_como_variavel() {
        let mut ds = fixtures_grafite();
        ds.apply([crate::dashboard::dataset::RawRow {
            table: RECURRENCES.into(),
            id: "RCON".into(),
            deleted_at: Some("2026-11-19T00:00:00Z".into()),
            seq: 1_000,
            data: series_edit("RCON", json!({})).2.to_string(),
        }]);
        let fv = fixed_variable(&ds, "2026-11");
        assert_eq!(
            (fv.fixed_minor, fv.variable_minor),
            (87_600 - 53_901, 223_741 + 53_901)
        );
    }

    #[test]
    fn serie_ausente_conta_como_variavel() {
        let fv = fixed_variable(&grafite_edit(&["RCON"], vec![]), "2026-11");
        assert_eq!(fv.variable_minor, 223_741 + 53_901);
    }

    #[test]
    fn serie_viva_sem_variable_e_fixa_mesmo_pausada_ou_de_app_antigo() {
        let paused = series_edit("RCON", json!({ "active": false }));
        let ds = grafite_edit(&["RCON"], vec![paused]);
        assert_eq!(fixed_variable(&ds, "2026-11").fixed_minor, 87_600);
        // App antigo: so `frequency`, sem regra; continua fixa (decisao 4.7).
        let old = (
            RECURRENCES,
            "RCON".to_string(),
            json!({ "frequency": "monthly" }),
        );
        let ds = grafite_edit(&["RCON"], vec![old]);
        assert!(ds.recurrences["RCON"].rule.is_none());
        assert_eq!(fixed_variable(&ds, "2026-11").fixed_minor, 87_600);
    }

    #[test]
    fn parcela_que_continua_em_dezembro_entra_no_comprometido_e_nao_na_fixa_de_novembro() {
        let longer = series_edit("RCAD", json!({ "endOn": "2026-12-21" }));
        let ds = grafite_edit(&["RCAD"], vec![longer]);
        assert_eq!(
            crate::dashboard::recurrence::recurring_month(&ds, "2026-12").expense_minor,
            92_652
        );
        assert_eq!(fixed_variable(&ds, "2026-11").fixed_minor, 87_600);
    }

    #[test]
    fn fixo_variavel_ignora_receita_transferencia_e_oculto() {
        let ds = grafite_edit(
            &[],
            vec![
                expense_row(
                    "T",
                    9_999,
                    "2026-11-10",
                    json!({ "reportState": "transfer" }),
                ),
                expense_row("H", 9_999, "2026-11-10", json!({ "reportState": "hidden" })),
            ],
        );
        let fv = fixed_variable(&ds, "2026-11");
        assert_eq!((fv.fixed_minor, fv.variable_minor), (87_600, 223_741));
    }

    // --- category-normal.test.ts (M5) ---

    fn mercado(id: &str, amount: i64, on: &str) -> GrafiteRow {
        expense_row(id, amount, on, json!({ "categoryId": "CAT-MERCADO" }))
    }

    /// So as categorias da fixture e os lancamentos dados (o `STATE` + `items` do teste do app).
    fn only_categories(items: Vec<GrafiteRow>) -> Dataset {
        let mut rows: Vec<GrafiteRow> = grafite_rows()
            .into_iter()
            .filter(|(t, _, _)| *t == crate::dashboard::dataset::CATEGORIES)
            .collect();
        rows.extend(items);
        dataset_from(rows)
    }

    fn above(items: Vec<GrafiteRow>, month: &str, today: &str) -> Vec<(String, CategoryNormal)> {
        categories_above_normal(&only_categories(items), month, today)
            .into_iter()
            .collect()
    }

    #[test]
    fn compras_acima_do_normal() {
        // 3.2: 51515 contra 37330 de outubro ate o dia 19 -> "↑ 38% do normal"; so Compras.
        let marked = categories_above_normal(&fixtures_grafite(), "2026-11", GRAFITE_TODAY);
        let expected = CategoryNormal {
            spent_minor: 51_515,
            normal_minor: 37_330,
            change_percent: 38,
        };
        assert_eq!(
            marked.into_iter().collect::<Vec<_>>(),
            [("CAT-COMPRAS".to_string(), expected)]
        );
    }

    #[test]
    fn no_primeiro_mes_nada_e_marcado() {
        assert!(categories_above_normal(&fixtures_grafite(), "2026-10", GRAFITE_TODAY).is_empty());
    }

    #[test]
    fn categoria_acima_do_normal_exige_25_por_cento_e_50_reais() {
        let marked = above(
            vec![
                mercado("now", 25_000, "2026-08-05"),
                mercado("ref", 20_000, "2026-07-05"),
            ],
            "2026-08",
            "2026-08-20",
        );
        assert_eq!(marked[0].1.change_percent, 25);
        // +40% mas so R$ 40 a mais.
        let small = vec![
            mercado("now", 14_000, "2026-08-05"),
            mercado("ref", 10_000, "2026-07-05"),
        ];
        assert!(above(small, "2026-08", "2026-08-20").is_empty());
        // R$ 200 a mais mas so +20%.
        let big = vec![
            mercado("now", 120_000, "2026-08-05"),
            mercado("ref", 100_000, "2026-07-05"),
        ];
        assert!(above(big, "2026-08", "2026-08-20").is_empty());
    }

    #[test]
    fn mes_corrente_compara_ate_o_mesmo_dia() {
        let later = vec![
            mercado("now", 10_000, "2026-08-05"),
            mercado("later", 90_000, "2026-08-25"),
            mercado("ref", 10_000, "2026-07-05"),
        ];
        assert!(above(later, "2026-08", "2026-08-20").is_empty());
        let normal_too = vec![
            mercado("now", 20_000, "2026-08-05"),
            mercado("ref", 10_000, "2026-07-05"),
            mercado("ref2", 50_000, "2026-07-25"),
        ];
        assert_eq!(
            above(normal_too, "2026-08", "2026-08-20")[0].1.normal_minor,
            10_000
        );
    }

    #[test]
    fn mes_fechado_compara_o_mes_inteiro_com_o_mes_inteiro() {
        let items = vec![
            mercado("now", 10_000, "2026-08-05"),
            mercado("late", 15_000, "2026-08-28"),
            mercado("ref", 10_000, "2026-07-05"),
        ];
        assert_eq!(
            above(items, "2026-08", "2026-09-10")[0].1,
            CategoryNormal {
                spent_minor: 25_000,
                normal_minor: 10_000,
                change_percent: 150,
            }
        );
    }

    #[test]
    fn categoria_divide_pelos_meses_de_referencia_com_dado() {
        // Junho tem 30000, julho nada: normal = 15000.
        let items = vec![
            mercado("now", 30_000, "2026-08-05"),
            mercado("jun", 30_000, "2026-06-05"),
        ];
        let marked = above(items, "2026-08", "2026-08-20");
        assert_eq!(
            (marked[0].1.normal_minor, marked[0].1.change_percent),
            (15_000, 100)
        );
    }

    #[test]
    fn categoria_ignora_transferencia_e_oculto() {
        let items = vec![
            mercado("now", 10_000, "2026-08-05"),
            expense_row(
                "t",
                90_000,
                "2026-08-06",
                json!({ "categoryId": "CAT-MERCADO", "reportState": "transfer" }),
            ),
            mercado("ref", 10_000, "2026-07-05"),
        ];
        assert!(above(items, "2026-08", "2026-08-20").is_empty());
    }

    // --- Plano, quem lancou e historico (so hub; sem par no app) ---

    #[test]
    fn plano() {
        // 3.2: metas 304500; sem meta 40718; sobra 4782 -> "~R$ 48"; 87% / 12%.
        let p = plan_fit(&fixtures_grafite(), "2026-11");
        assert_eq!(p.fixed_income_minor, 350_000);
        assert_eq!(p.budgets_minor, 304_500);
        assert_eq!(p.unbudgeted_minor, 40_718);
        assert_eq!(p.leftover_minor, 4_782);
        assert_eq!(
            (p.budgets_percent, p.unbudgeted_percent),
            (Some(87), Some(12))
        );
        assert_eq!((p.reserves_ask_minor, p.reserves_asking), (55_000, 1));
    }

    #[test]
    fn plano_sem_renda_fixa_nao_tem_porcentagem() {
        let ds = grafite_edit(&["RSAL", "7748FB44FQCR7BWM7Q4RFV4MF7"], vec![]);
        let p = plan_fit(&ds, "2026-11");
        assert_eq!(p.fixed_income_minor, 0);
        assert_eq!((p.budgets_percent, p.unbudgeted_percent), (None, None));
    }

    #[test]
    fn quem_lancou() {
        // 3.2: UL 264041 (receita 350000) / UG 47300 (sem receita), maior despesa primeiro.
        let people = by_person(&fixtures_grafite(), "2026-11");
        let got: Vec<(&str, i64, i64)> = people
            .iter()
            .map(|p| (p.author.label(), p.expense_minor, p.income_minor))
            .collect();
        assert_eq!(got, [("Luiz", 264_041, 350_000), ("Gigi", 47_300, 0)]);
    }

    #[test]
    fn quem_lancou_some_com_uma_pessoa() {
        // Outubro so tem lancamentos de UL.
        assert!(by_person(&fixtures_grafite(), "2026-10").is_empty());
    }

    #[test]
    fn quem_lancou_separa_sem_autor_e_perfil_removido() {
        let ds = grafite_edit(
            &[],
            vec![
                expense_row("X1", 100, "2026-10-02", json!({})),
                expense_row("X2", 200, "2026-10-03", json!({ "userId": "U-SUMIU" })),
            ],
        );
        let people = by_person(&ds, "2026-10");
        let labels: Vec<&str> = people.iter().map(|p| p.author.label()).collect();
        assert_eq!(labels, ["Luiz", "Perfil removido", "Sem autor"]);
    }

    #[test]
    fn outubro_no_historico() {
        // 3.2: 507229 - 366200 = 141029; round(27,8) = 28%.
        let h = history(&fixtures_grafite(), "2026-11", GRAFITE_TODAY);
        let oct = h
            .rows
            .iter()
            .find(|r| r.month == "2026-10")
            .expect("outubro");
        assert_eq!((oct.surplus_minor, oct.savings_rate), (141_029, Some(28)));
        assert!(oct.existed && !oct.partial);
        let nov = h.rows.last().expect("novembro");
        assert_eq!((nov.surplus_minor, nov.savings_rate), (38_659, Some(11)));
        assert!(nov.partial);
    }

    #[test]
    fn sobra_media() {
        // 3.2: so outubro (fechado e com dado) -> R$ 1.410,29.
        let h = history(&fixtures_grafite(), "2026-11", GRAFITE_TODAY);
        assert_eq!(h.average, avg(141_029, 1));
    }

    #[test]
    fn historico_conta_meses_desde_o_primeiro_lancamento() {
        let h = history(&fixtures_grafite(), "2026-11", GRAFITE_TODAY);
        assert_eq!(h.months_of_history, 2);
        assert!(!h.is_chart());
        assert_eq!(h.rows.len(), 12);
        assert_eq!(h.rows.iter().filter(|r| r.existed).count(), 2);
        // Quatro meses de historia: vira grafico.
        let ds = grafite_edit(&[], vec![expense_row("OLD", 100, "2026-08-02", json!({}))]);
        let h = history(&ds, "2026-11", GRAFITE_TODAY);
        assert_eq!(h.months_of_history, 4);
        assert!(h.is_chart());
        // Mes escolhido antes do primeiro lancamento: nenhum mes de historia.
        assert_eq!(
            history(&fixtures_grafite(), "2026-09", GRAFITE_TODAY).months_of_history,
            0
        );
    }

    #[test]
    fn historico_de_mes_fechado_ainda_tira_a_media_so_dos_fechados() {
        // Escolhido outubro com hoje em novembro: outubro esta fechado e entra.
        let h = history(&fixtures_grafite(), "2026-10", GRAFITE_TODAY);
        assert_eq!(h.average, avg(141_029, 1));
        assert!(h.rows.iter().all(|r| !r.partial));
    }
}
