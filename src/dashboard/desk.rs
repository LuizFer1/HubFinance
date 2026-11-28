//! View-model do Dashboard do redesenho "App desktop" (spec 6.1.2): "Como estou", "Negócio · PJ",
//! "Reserva" e "Histórico". Toda `String` sai pronta e todo valor em R$ passa pelo `Money` com
//! `hidden`, como no `GrafiteView`: formatar na tela vazaria o número por cima do botão de
//! ocultar. Barras vão como frações (0..=1) para a forma continuar visível com valores ocultos.
//!
//! O `GrafiteView` não muda (o `advisor` e o envelope do assistente dependem dele); este módulo
//! só lê as mesmas funções de `aggregate`/`insights`/`reserves`, então nenhuma regra de "o que
//! conta como receita/despesa" é nova.
//!
//! A parte 2 (Tarefa 17b) acrescenta ao `DeskView` "Para onde foi", "O que vem", "Recentes",
//! "Pessoas" e o rodapé.
#![allow(dead_code)] // usado na Tarefa 18

use super::aggregate::{in_month, totals};
use super::business::business_panel;
use super::dataset::Dataset;
use super::grafite::{LabeledValue, Tone};
use super::insights::history;
use super::money::{AXIS_MASK, BigMoney, MINUS, Money};
use super::periods::{
    days_in_month, month_long, month_of, month_short, partial_label, shift_month,
};
use super::reserves::{self, emergency_view, ordered, saved_in_month};

/// Meses do "Histórico" e das mini-barras do negócio.
pub const DESK_MONTHS: usize = 6;

/// Passo do teto do eixo do Histórico: R$ 3.000, para as três divisões caírem em milhares
/// redondos ("15 mil · 10 mil · 5 mil · 0" no design).
const AXIS_STEP_MINOR: i64 = 300_000;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeskView {
    pub month: String,
    pub hidden: bool,
    pub how_am_i: DeskHowAmI,
    /// Só com conta comercial que tenha lançamento; aparece também na visão Pessoal (spec 6.1.1).
    pub business: Option<DeskBusiness>,
    pub reserve: DeskReserve,
    pub history: DeskHistory,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeskHowAmI {
    pub title: String,
    /// "Saldo do mês · 1 a 27 nov".
    pub balance_label: String,
    pub balance: BigMoney,
    pub balance_negative: bool,
    /// "+R$ 611,75" "vs. out"; `None` sem mês anterior com história ("Primeiro mês").
    pub delta: Option<DeskDelta>,
    /// Barra "Despesas já usaram 75,6% das receitas"; `None` sem receita (não há base).
    pub usage: Option<DeskUsage>,
    pub income: LabeledValue,
    pub expense: LabeledValue,
    /// "Gasto médio por dia".
    pub daily_average: LabeledValue,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeskDelta {
    pub value: String,
    /// "vs. out".
    pub suffix: String,
    pub tone: Tone,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeskUsage {
    /// "Despesas já usaram 75,6% das receitas".
    pub text: String,
    /// "75,6%".
    pub percent: String,
    /// Preenchimento da barra, 0..=1 (despesa acima da receita enche a barra).
    pub fill: f32,
    /// Despesa passou da receita: a tela pinta de `expense`.
    pub over: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeskBusiness {
    /// "Negócio · PJ".
    pub title: String,
    pub result: LabeledValue,
    pub result_negative: bool,
    /// "+12%" "vs. out"; `None` quando o mês anterior tem resultado zero (sem base).
    pub delta: Option<DeskDelta>,
    /// "Faturado".
    pub income: LabeledValue,
    /// "Despesas PJ".
    pub expense: LabeledValue,
    /// "Resultado PJ · 6 meses · média".
    pub bars_title: String,
    /// "R$ 6,3 mil": média dos meses fechados; `None` sem mês fechado com dado.
    pub average: Option<String>,
    pub bars: Vec<DeskBar>,
    /// "R$ 5.000,00 foram para a casa como pró-labore. DAS de R$ 588,00 vence em 20 dez."
    /// Só com dado real (presunção 3 da spec): o hub não tem transferência marcada nem
    /// recorrência de DAS ligadas ao negócio hoje, então fica `None` e a frase some.
    pub pro_labore: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeskBar {
    /// "Jun" / "Nov*".
    pub label: String,
    /// Resultado do mês na escala do maior resultado positivo (negativo vira 0 e `negative`).
    pub value: f32,
    pub negative: bool,
    pub partial: bool,
    pub existed: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeskReserve {
    pub title: String,
    pub emergency: Option<DeskEmergency>,
    /// "Ainda não há reserva de emergência" quando não há.
    pub empty: Option<String>,
    /// "Total separado": soma de todas as reservas vivas, hoje.
    pub total: LabeledValue,
    /// "Guardado em novembro" / "Retirado em novembro"; `None` sem movimento no mês.
    pub month_flow: Option<LabeledValue>,
    /// "Ver reservas".
    pub link: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeskEmergency {
    /// "Emergência coberta" com a meta alcançada; "Cobertura da emergência" antes disso.
    pub headline: String,
    /// "6,2" ou "—".
    pub coverage: String,
    /// "meses" ("mês" só em "1,0").
    pub unit: String,
    /// "de custo essencial guardados · meta: 6 meses".
    pub caption: String,
    pub reached: bool,
    /// "Reserva de emergência" com o saldo.
    pub balance: LabeledValue,
    /// "Custo essencial médio" com "R$ 5.420,00/mês"; `None` sem custo.
    pub essential: Option<LabeledValue>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeskHistory {
    pub title: String,
    /// "Últimos 6 meses".
    pub subtitle: String,
    pub legend_income: String,
    pub legend_expense: String,
    pub months: Vec<DeskMonth>,
    /// Eixo de cima para baixo: "15 mil", "10 mil", "5 mil", "0".
    pub y_labels: [String; 4],
    /// "Valor abaixo do mês = quanto sobrou · *novembro parcial, até 27 nov".
    pub footnote: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeskMonth {
    /// "Jun" / "Nov*".
    pub label: String,
    pub existed: bool,
    pub partial: bool,
    pub income: f32,
    pub expense: f32,
    /// "+2,5 mil" embaixo do mês; vazio antes do primeiro lançamento.
    pub surplus: String,
    pub surplus_negative: bool,
    /// Hover da barra: Receitas, Despesas, Sobrou/Faltou.
    pub tip: Option<DeskTip>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeskTip {
    /// "Nov 2026".
    pub title: String,
    pub rows: Vec<LabeledValue>,
}

fn capitalized(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn day_of(date: &str) -> i64 {
    date.get(8..10).and_then(|d| d.parse().ok()).unwrap_or(0)
}

fn lv(label: &str, value: String) -> LabeledValue {
    LabeledValue {
        label: label.to_string(),
        value,
    }
}

fn frac(part: i64, whole: i64) -> f32 {
    if whole <= 0 {
        0.0
    } else {
        (part as f64 / whole as f64).clamp(0.0, 1.0) as f32
    }
}

/// "75,6%": uma casa com vírgula, meio para cima em décimos com inteiros (sem `f64`, que
/// arredondaria 0,05 para o lado errado por representação). `whole` deve ser positivo.
fn percent_tenths(part: i64, whole: i64) -> String {
    let tenths = (i128::from(part) * 2000 + i128::from(whole)) / (i128::from(whole) * 2);
    format!("{},{}%", tenths / 10, tenths % 10)
}

/// Eixo sem o "R$ " do `axis_label` ("15 mil", "500"): o design põe só o número. Oculto, só pontos.
fn axis_plain(m: Money, minor: i64) -> String {
    m.axis(minor).trim_start_matches("R$ ").to_string()
}

/// "+2,5 mil" / "−1,2 mil" / "0" embaixo de cada mês do Histórico.
fn surplus_label(m: Money, minor: i64) -> String {
    if m.hidden {
        return AXIS_MASK.to_string();
    }
    match minor.cmp(&0) {
        std::cmp::Ordering::Equal => "0".to_string(),
        std::cmp::Ordering::Greater => format!("+{}", axis_plain(m, minor)),
        std::cmp::Ordering::Less => format!("{MINUS}{}", axis_plain(m, minor)),
    }
}

/// Variação `+12%` / `−8%` de `cur` sobre `prev`, meio para cima como `Math.round`. Sem base
/// (zero) não há porcentagem.
fn change_label(cur: i64, prev: i64) -> Option<(String, Tone)> {
    if prev == 0 {
        return None;
    }
    let base = i128::from(prev).abs();
    let pct = ((i128::from(cur) - i128::from(prev)) * 200 + base).div_euclid(base * 2);
    Some(match pct.cmp(&0) {
        std::cmp::Ordering::Less => (format!("{MINUS}{}%", pct.unsigned_abs()), Tone::Expense),
        _ => (format!("+{pct}%"), Tone::Income),
    })
}

/// "Jun", ou "Nov*" no mês parcial.
fn month_label(month: &str, partial: bool) -> String {
    let name = capitalized(month_short(month));
    if partial { format!("{name}*") } else { name }
}

/// `ds` é a conta pessoal; `business` é a visão da conta comercial (`Dataset::for_book`), ou
/// `None` sem conta comercial. `today` é `YYYY-MM-DD`, `month` é `YYYY-MM`.
pub fn build_desk(
    ds: &Dataset,
    business: Option<&Dataset>,
    today: &str,
    month: &str,
    hidden: bool,
) -> DeskView {
    let m = Money::new(hidden);
    DeskView {
        month: month.to_string(),
        hidden,
        how_am_i: how_am_i(ds, today, month, m),
        business: business
            .filter(|b| !b.is_empty())
            .map(|b| business_card(b, today, month, m)),
        reserve: reserve_card(ds, today, month, m),
        history: history_card(ds, today, month, m),
    }
}

fn how_am_i(ds: &Dataset, today: &str, month: &str, m: Money) -> DeskHowAmI {
    let is_current = month == month_of(today);
    let t = totals(in_month(ds, month));
    let prev_month = shift_month(month, -1);
    let prev = totals(in_month(ds, &prev_month));
    // Sem lançamento antes deste mês não há o que comparar: "2026-09" < "2026-09-01".
    let had_previous = ds
        .alive_transactions()
        .any(|t| t.occurred_on.as_str() < month);
    let delta = had_previous.then(|| {
        let d = t.balance_minor.saturating_sub(prev.balance_minor);
        DeskDelta {
            value: m.signed_plus(d),
            suffix: format!("vs. {}", month_short(&prev_month)),
            tone: if d < 0 { Tone::Expense } else { Tone::Income },
        }
    });
    let usage = (t.income_minor > 0).then(|| {
        let percent = percent_tenths(t.expense_minor, t.income_minor);
        DeskUsage {
            text: format!("Despesas já usaram {percent} das receitas"),
            percent,
            fill: frac(t.expense_minor, t.income_minor),
            over: t.expense_minor > t.income_minor,
        }
    });
    // Mês corrente divide pelos dias já vividos (dividir pelo mês inteiro subestimaria o ritmo);
    // mês fechado, pelo mês inteiro.
    let last_day = if is_current {
        day_of(today).max(1)
    } else {
        i64::from(days_in_month(month))
    };
    let daily = t.expense_minor.saturating_add(last_day / 2) / last_day;
    DeskHowAmI {
        title: "Como estou".to_string(),
        balance_label: format!("Saldo do mês · 1 a {last_day} {}", month_short(month)),
        balance: m.big(t.balance_minor),
        balance_negative: t.balance_minor < 0,
        delta,
        usage,
        income: lv("Receitas", m.brl(t.income_minor)),
        expense: lv("Despesas", m.brl(t.expense_minor)),
        daily_average: lv("Gasto médio por dia", m.brl(daily)),
    }
}

fn business_card(b: &Dataset, today: &str, month: &str, m: Money) -> DeskBusiness {
    let p = business_panel(b, month);
    let prev_month = shift_month(month, -1);
    let prev = business_panel(b, &prev_month);
    let delta = change_label(p.result_minor, prev.result_minor).map(|(value, tone)| DeskDelta {
        value,
        suffix: format!("vs. {}", month_short(&prev_month)),
        tone,
    });
    let h = history(b, month, today);
    let rows: Vec<_> = h
        .rows
        .iter()
        .skip(h.rows.len().saturating_sub(DESK_MONTHS))
        .collect();
    let peak = rows
        .iter()
        .filter(|r| r.existed)
        .map(|r| r.surplus_minor)
        .max()
        .unwrap_or(0);
    // O mês parcial puxaria a média para baixo: só os fechados entram.
    let closed: Vec<i64> = rows
        .iter()
        .filter(|r| r.existed && !r.partial)
        .map(|r| r.surplus_minor)
        .collect();
    let average = (!closed.is_empty()).then(|| {
        let sum = closed.iter().fold(0i64, |a, v| a.saturating_add(*v));
        m.axis(sum / closed.len() as i64)
    });
    DeskBusiness {
        title: "Negócio · PJ".to_string(),
        result: lv("Resultado do mês", m.signed(p.result_minor)),
        result_negative: p.result_minor < 0,
        delta,
        income: lv("Faturado", m.brl(p.income_minor)),
        expense: lv("Despesas PJ", m.brl(p.expense_minor)),
        bars_title: "Resultado PJ · 6 meses · média".to_string(),
        average,
        bars: rows
            .iter()
            .map(|r| DeskBar {
                label: month_label(&r.month, r.partial),
                value: if r.existed {
                    frac(r.surplus_minor, peak)
                } else {
                    0.0
                },
                negative: r.existed && r.surplus_minor < 0,
                partial: r.partial,
                existed: r.existed,
            })
            .collect(),
        pro_labore: None,
    }
}

fn reserve_card(ds: &Dataset, today: &str, month: &str, m: Money) -> DeskReserve {
    // A reserva é o saldo de hoje em qualquer mês escolhido; só "Guardado em …" segue o seletor.
    let current = month_of(today);
    let (emergency, _) = ordered(ds);
    let block = emergency.map(|r| {
        let v = emergency_view(ds, r, &current, m);
        let reached = v.goal_minor > 0 && v.balance_minor >= v.goal_minor;
        DeskEmergency {
            headline: if reached {
                "Emergência coberta"
            } else {
                "Cobertura da emergência"
            }
            .to_string(),
            unit: if v.coverage_label == "1,0" {
                "mês"
            } else {
                "meses"
            }
            .to_string(),
            caption: if v.cost.is_some() {
                format!(
                    "de custo essencial guardados · meta: {} {}",
                    v.multiple,
                    if v.multiple == 1 { "mês" } else { "meses" }
                )
            } else {
                "Sem custo essencial para calcular a cobertura".to_string()
            },
            coverage: v.coverage_label.clone(),
            reached,
            balance: lv("Reserva de emergência", m.brl(v.balance_minor)),
            essential: v.cost.as_ref().map(|c| {
                lv(
                    "Custo essencial médio",
                    format!("{}/mês", m.brl(c.total_minor)),
                )
            }),
        }
    });
    let total = reserves::totals(ds, std::slice::from_ref(&current)).total_minor;
    let saved = saved_in_month(ds, month);
    let name = month_long(month);
    DeskReserve {
        title: "Reserva".to_string(),
        empty: block
            .is_none()
            .then(|| "Ainda não há reserva de emergência".to_string()),
        emergency: block,
        total: lv("Total separado", m.brl(total)),
        month_flow: match saved.cmp(&0) {
            std::cmp::Ordering::Equal => None,
            std::cmp::Ordering::Greater => {
                Some(lv(&format!("Guardado em {name}"), m.signed_plus(saved)))
            }
            std::cmp::Ordering::Less => Some(lv(&format!("Retirado em {name}"), m.signed(saved))),
        },
        link: "Ver reservas".to_string(),
    }
}

fn history_card(ds: &Dataset, today: &str, month: &str, m: Money) -> DeskHistory {
    let h = history(ds, month, today);
    let rows: Vec<_> = h
        .rows
        .iter()
        .skip(h.rows.len().saturating_sub(DESK_MONTHS))
        .collect();
    let peak = rows
        .iter()
        .filter(|r| r.existed)
        .map(|r| r.income_minor.max(r.expense_minor))
        .max()
        .unwrap_or(0);
    let top = (peak.max(1).saturating_add(AXIS_STEP_MINOR - 1) / AXIS_STEP_MINOR)
        .max(1)
        .saturating_mul(AXIS_STEP_MINOR);
    let partial_note = rows.iter().any(|r| r.partial).then(|| {
        format!(
            " · *{} parcial, até {}",
            month_long(month),
            partial_label(today)
        )
    });
    DeskHistory {
        title: "Histórico".to_string(),
        subtitle: format!("Últimos {DESK_MONTHS} meses"),
        legend_income: "Receitas".to_string(),
        legend_expense: "Despesas".to_string(),
        months: rows
            .iter()
            .map(|r| DeskMonth {
                label: month_label(&r.month, r.partial),
                existed: r.existed,
                partial: r.partial,
                income: if r.existed {
                    frac(r.income_minor, top)
                } else {
                    0.0
                },
                expense: if r.existed {
                    frac(r.expense_minor, top)
                } else {
                    0.0
                },
                surplus: if r.existed {
                    surplus_label(m, r.surplus_minor)
                } else {
                    String::new()
                },
                surplus_negative: r.existed && r.surplus_minor < 0,
                tip: r.existed.then(|| DeskTip {
                    title: format!(
                        "{} {}",
                        capitalized(month_short(&r.month)),
                        r.month.get(..4).unwrap_or("")
                    ),
                    rows: vec![
                        lv("Receitas", m.brl(r.income_minor)),
                        lv("Despesas", m.brl(r.expense_minor)),
                        lv(
                            if r.surplus_minor < 0 {
                                "Faltou"
                            } else {
                                "Sobrou"
                            },
                            m.brl(r.surplus_minor.saturating_abs()),
                        ),
                    ],
                }),
            })
            .collect(),
        y_labels: [
            axis_plain(m, top),
            axis_plain(m, top / 3 * 2),
            axis_plain(m, top / 3),
            axis_plain(m, 0),
        ],
        footnote: format!(
            "Valor abaixo do mês = quanto sobrou{}",
            partial_note.unwrap_or_default()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dashboard::contract::Book;
    use crate::dashboard::dataset::{BOOKS, TRANSACTIONS};
    use crate::dashboard::money::leaks;
    use crate::dashboard::{GRAFITE_TODAY, dataset_from, fixtures_grafite, grafite_rows};
    use serde_json::json;

    fn nov() -> DeskView {
        build_desk(&fixtures_grafite(), None, GRAFITE_TODAY, "2026-11", false)
    }

    /// Fixture: novembro (até o dia 19) tem receitas de R$ 3.500,00 e despesas de R$ 3.113,41;
    /// outubro fechou com sobra de R$ 1.410,29 (a mesma "sobra média" que o Grafite pina).
    #[test]
    fn como_estou_da_fixture() {
        let c = nov().how_am_i;
        assert_eq!(c.balance_label, "Saldo do mês · 1 a 19 nov");
        // 3.500,00 - 3.113,41.
        assert_eq!(
            (c.balance.whole.as_str(), c.balance.cents.as_str()),
            ("386", ",59")
        );
        assert!(!c.balance_negative);
        // 386,59 - 1.410,29 = -1.023,70.
        let d = c.delta.expect("delta");
        assert_eq!(
            (d.value.as_str(), d.suffix.as_str()),
            ("\u{2212}R$ 1.023,70", "vs. out")
        );
        assert_eq!(d.tone, Tone::Expense);
        // 3113,41 / 3500 = 88,954...% -> 89,0%.
        let u = c.usage.expect("uso");
        assert_eq!(u.text, "Despesas já usaram 89,0% das receitas");
        assert!(!u.over);
        assert_eq!(c.income.value, "R$ 3.500,00");
        assert_eq!(c.expense.value, "R$ 3.113,41");
        // 3113,41 / 19 dias vividos = 163,86.
        assert_eq!(c.daily_average.value, "R$ 163,86");
    }

    #[test]
    fn mes_fechado_divide_pelo_mes_inteiro() {
        let v = build_desk(&fixtures_grafite(), None, GRAFITE_TODAY, "2026-10", false);
        assert_eq!(v.how_am_i.balance_label, "Saldo do mês · 1 a 31 out");
        // 3662,00 / 31 = 118,13.
        assert_eq!(v.how_am_i.daily_average.value, "R$ 118,13");
    }

    #[test]
    fn negocio_some_sem_conta_comercial_e_calcula_com_ela() {
        assert_eq!(nov().business, None);
        let mut rows = grafite_rows();
        rows.push((
            BOOKS,
            "BK".to_string(),
            json!({ "kind": "business", "name": "Padaria", "color": "amber", "icon": "s" }),
        ));
        for (id, kind, amount, on) in [
            ("1110-1-VENDA", "income", 80_000, "2026-11-10"),
            ("1111-1-FARINHA", "expense", 30_000, "2026-11-11"),
            ("1010-1-VENDA", "income", 40_000, "2026-10-10"),
        ] {
            rows.push((
                TRANSACTIONS,
                id.to_string(),
                json!({ "kind": kind, "description": "x", "amountMinor": amount,
                        "occurredOn": on, "book": "business", "userId": "UL" }),
            ));
        }
        let all = dataset_from(rows);
        let biz = all.for_book(Book::Business);
        let v = build_desk(
            &all.for_book(Book::Personal),
            Some(&biz),
            GRAFITE_TODAY,
            "2026-11",
            false,
        );
        let b = v.business.expect("negocio");
        // Novembro: 800 - 300 = 500; outubro: 400 -> +25%.
        assert_eq!(b.result.value, "R$ 500,00");
        assert_eq!(b.income.value, "R$ 800,00");
        assert_eq!(b.expense.value, "R$ 300,00");
        let d = b.delta.expect("delta");
        assert_eq!((d.value.as_str(), d.suffix.as_str()), ("+25%", "vs. out"));
        // Só outubro está fechado: média R$ 400.
        assert_eq!(b.average.as_deref(), Some("R$ 400"));
        assert_eq!(b.bars.len(), 6);
        assert_eq!(b.bars[5].label, "Nov*");
        assert_eq!(b.bars[5].value, 1.0);
        assert!((b.bars[4].value - 0.8).abs() < 1e-6);
        assert!(!b.bars[0].existed);
        assert_eq!(b.pro_labore, None);
        // Conta comercial vazia não vira card.
        let empty = build_desk(
            &all,
            Some(&Dataset::default()),
            GRAFITE_TODAY,
            "2026-11",
            false,
        );
        assert_eq!(empty.business, None);
    }

    #[test]
    fn reserva_da_fixture() {
        let r = nov().reserve;
        let e = r.emergency.expect("emergencia");
        assert_eq!(e.coverage, "0,1");
        assert_eq!(e.caption, "de custo essencial guardados · meta: 6 meses");
        assert!(!e.reached);
        assert_eq!(e.balance.value, "R$ 350,00");
        assert_eq!(e.essential.expect("custo").value, "R$ 2.500,00/mês");
        assert_eq!(r.total.value, "R$ 500,00");
        // Só há uma retirada de R$ 200 em novembro.
        let f = r.month_flow.expect("movimento");
        assert_eq!(
            (f.label.as_str(), f.value.as_str()),
            ("Retirado em novembro", "\u{2212}R$ 200,00")
        );
        assert_eq!(r.link, "Ver reservas");
    }

    #[test]
    fn historico_da_fixture() {
        let h = nov().history;
        assert_eq!(h.months.len(), 6);
        let labels: Vec<_> = h.months.iter().map(|m| m.label.as_str()).collect();
        assert_eq!(labels, ["Jun", "Jul", "Ago", "Set", "Out", "Nov*"]);
        // Antes de outubro não havia lançamento: coluna vazia.
        assert!(h.months[..4].iter().all(|m| !m.existed && m.tip.is_none()));
        assert_eq!(h.months[4].surplus, "+1,4 mil");
        // Pico 5.072,29 -> teto R$ 6.000 (passo de R$ 3.000).
        assert_eq!(h.y_labels, ["6 mil", "4 mil", "2 mil", "0"]);
        assert!((h.months[4].income - 0.845_381_7).abs() < 1e-4);
        assert!(h.months[5].partial);
        assert_eq!(
            h.footnote,
            "Valor abaixo do mês = quanto sobrou · *novembro parcial, até 19 nov"
        );
        let tip = h.months[4].tip.as_ref().expect("tip");
        assert_eq!(tip.rows[2].value, "R$ 1.410,29");
    }

    #[test]
    fn oculto_nao_vaza_digito() {
        let v = build_desk(&fixtures_grafite(), None, GRAFITE_TODAY, "2026-11", true);
        assert_eq!(leaks(&format!("{v:?}")), None);
        // Forma continua visível: barras e porcentagem não são valores.
        assert!(v.history.months[5].income > 0.0);
        assert!(v.how_am_i.usage.is_some());
        assert_eq!(v.how_am_i.balance.whole, "•••••");
        assert_eq!(v.history.y_labels[0], "•••");
        assert_eq!(v.history.months[4].surplus, "•••");
    }
}
