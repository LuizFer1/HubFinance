//! View-model do Dashboard do redesenho "App desktop" (spec 6.1.2): "Como estou", "Negócio · PJ",
//! "Reserva" e "Histórico". Toda `String` sai pronta e todo valor em R$ passa pelo `Money` com
//! `hidden`, como no `GrafiteView`: formatar na tela vazaria o número por cima do botão de
//! ocultar. Barras vão como frações (0..=1) para a forma continuar visível com valores ocultos.
//!
//! O `GrafiteView` não muda (o `advisor` e o envelope do assistente dependem dele); este módulo
//! só lê as mesmas funções de `aggregate`/`insights`/`reserves`, então nenhuma regra de "o que
//! conta como receita/despesa" é nova.
//!
//! A parte 2 (Tarefa 17b) acrescenta "Para onde foi", "O que vem", "Recentes", "Pessoas" e o
//! rodapé; o "Atualizado às …" do rodapé vem do snapshot, na tela.

use super::aggregate::{
    NO_CATEGORY_COLOR, NO_CATEGORY_ICON, NO_CATEGORY_KEY, expense_by_category, in_month, recent,
    totals,
};
use super::business::business_panel;
use super::cards::{InvoiceStatus, upcoming_invoices};
use super::contract::Kind;
use super::dataset::Dataset;
use super::grafite::{LabeledValue, Tone};
use super::insights::{Author, HistoryRow, by_person, history};
use super::list::to_row;
use super::money::{AXIS_MASK, BigMoney, MASK, MINUS, Money, axis_label, group_thousands};
use super::periods::{
    days_between, days_in_month, month_long, month_of, month_short, partial_label, shift_day,
    shift_month,
};
use super::recurrence::upcoming;
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
    pub where_it_went: DeskWhere,
    /// Só no mês corrente (a previsão não faz sentido para mês passado).
    pub upcoming: Option<DeskUpcoming>,
    pub recent: DeskRecent,
    /// Só com dois ou mais autores no mês.
    pub people: Option<DeskPeople>,
    pub footer: DeskFooter,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeskHowAmI {
    pub title: String,
    /// "Saldo do mês · 1 a 27 nov".
    pub balance_label: String,
    pub balance: BigMoney,
    pub balance_negative: bool,
    /// "+R$ 611,75" "vs. out"; `None` sem mês anterior com história (ver `first_month`).
    pub delta: Option<DeskDelta>,
    /// "Primeiro mês" quando não há lançamento antes do mês (no lugar do delta); a tela não
    /// escreve texto, só mostra este.
    pub first_month: Option<String>,
    /// "Ver lançamentos" (cabeçalho do card).
    pub link: String,
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
    /// "NEGÓCIO · PJ" (pílula do design, texto literal em caixa alta).
    pub title: String,
    pub result: LabeledValue,
    pub result_negative: bool,
    /// "+12%" "vs. out"; `None` quando o mês anterior tem resultado zero (sem base).
    pub delta: Option<DeskDelta>,
    /// "Faturado".
    pub income: LabeledValue,
    /// "Despesas PJ".
    pub expense: LabeledValue,
    /// "Resultado PJ · 6 meses · média" (sem o "· média" quando `average` é `None`).
    pub bars_title: String,
    /// "R$ 6,3 mil": média dos meses fechados; `None` sem mês fechado com dado.
    pub average: Option<String>,
    pub bars: Vec<DeskBar>,
    /// Nome acessível do gráfico: "Resultado PJ de junho a novembro: 5,2 mil, … (novembro
    /// parcial); média 6,3 mil". Oculto, os valores viram "valores ocultos".
    pub bars_label: String,
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
    /// "Reserva de emergência não configurada" quando não há.
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
    /// "Emergência coberta" com a meta alcançada; "Ainda não coberta" antes disso (Estados.dc.html).
    pub headline: String,
    /// "6,2" ou "—".
    pub coverage: String,
    /// "meses" ("mês" só em "1,0").
    pub unit: String,
    /// "de custo essencial guardados · meta: 6 meses".
    pub caption: String,
    pub reached: bool,
    /// Preenchimento da barra do card (0..=1, 1 com a meta coberta; 0 sem meta ou sem custo).
    /// Vem pronto para a tela não refazer a conta, e continua visível com valores ocultos
    /// (é forma, não valor), como `DeskUsage.fill`.
    pub fill: f32,
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
    /// Fica visível com valores ocultos: no design o eixo é `.n` (escala), não `.v` (valor).
    pub y_labels: [String; 4],
    /// Nome acessível do gráfico: "Receitas e despesas de junho a novembro. Sobras: 2,5 mil, …
    /// (novembro parcial)." Oculto, as sobras viram "valores ocultos".
    pub chart_label: String,
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
    /// Nome do mês por extenso ("Outubro"), como no hover do design.
    pub title: String,
    pub rows: Vec<LabeledValue>,
}

/// "Para onde foi": rosca das 5 maiores categorias mais "Outros".
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeskWhere {
    pub title: String,
    /// "R$ 10.767" no centro da rosca; `None` sem despesa no mês.
    pub total: Option<BigMoney>,
    /// "gastos no mês".
    pub total_caption: String,
    pub slices: Vec<DeskSlice>,
    /// "Mercado subiu 8,4% em relação a outubro"; `None` sem alta (ou sem mês anterior).
    pub rise: Option<DeskRise>,
    /// "Nenhuma despesa em novembro".
    pub empty: Option<String>,
    /// Nome acessível da rosca: "Despesas por categoria: Moradia 31,8%, …". Porcentagem não é
    /// valor em R$, então segue visível com os valores ocultos.
    pub chart_label: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeskSlice {
    /// Id da categoria; vazio em "Outros".
    pub key: String,
    pub name: String,
    /// Token de cor; "Outros" usa o neutro (`NO_CATEGORY_COLOR`).
    pub color: String,
    pub icon: String,
    /// "31,8%".
    pub percent: String,
    /// "R$ 3.420,00".
    pub amount: String,
    /// Fração do total (0..=1): ângulo da rosca e barra, visíveis com valores ocultos.
    pub share: f32,
    pub other: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeskRise {
    pub category: String,
    /// "8,4%".
    pub percent: String,
    /// Frase inteira, para a tela não montar texto.
    pub sentence: String,
}

/// "O que vem": saídas dos próximos 15 dias (só o mês corrente, como o `GrafiteView`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeskUpcoming {
    pub title: String,
    /// "Próximos 15 dias".
    pub window: String,
    /// `None` sem saída na janela.
    pub total: Option<BigMoney>,
    /// "previstos em 6 contas".
    pub caption: String,
    pub items: Vec<DeskDue>,
    /// "Depois: DAS do negócio · 20 dez", a primeira saída fora da janela.
    pub later: Option<DeskLater>,
    /// "Nada programado nos próximos 15 dias."
    pub empty: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeskDue {
    /// "01".
    pub day: String,
    /// "dez".
    pub month: String,
    pub title: String,
    /// "Pix" (forma de pagamento) ou "Fatura aberta"; vazio sem forma.
    pub detail: String,
    /// "mensal": só em saída recorrente.
    pub recurrence: Option<String>,
    /// "R$ 2.800,00"; "~R$ …" quando o valor é estimativa.
    pub amount: String,
    /// "em 4 dias".
    pub in_days: String,
    pub invoice: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeskLater {
    pub title: String,
    /// "20 dez".
    pub date: String,
    pub amount: String,
    /// Frase pronta, como no design: "Depois: Fatura Mercado Pago · 8 dez".
    pub label: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeskRecent {
    pub title: String,
    /// "Ver todos".
    pub link: String,
    pub lines: Vec<DeskRecentLine>,
    /// "Nenhum lançamento em novembro".
    pub empty: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeskRecentLine {
    pub id: String,
    pub description: String,
    /// Nome da categoria (o `title` do tile).
    pub category: String,
    pub icon: String,
    pub color: String,
    /// "Mercado · Ana · hoje".
    pub meta: String,
    /// "−R$ 412,38" / "+R$ 800,00".
    pub amount: String,
    pub income: bool,
    /// "Transferência" / "Oculto": aparece, mas fica fora da soma.
    pub tag: Option<String>,
    pub excluded: bool,
}

/// "Pessoas": só com dois ou mais autores no mês (`by_person` devolve vazio antes disso).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeskPeople {
    pub title: String,
    /// "Gasto por pessoa".
    pub subtitle: String,
    pub rows: Vec<DeskPerson>,
    /// Barra dividida, na mesma ordem das linhas.
    pub split: Vec<DeskSplit>,
    /// "Divisão dos gastos: Ana 56,8%, Rafael 43,2%".
    pub split_label: String,
    /// "Ver lançamentos por pessoa".
    pub link: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeskPerson {
    pub name: String,
    pub initial: String,
    /// Token de cor do perfil; `None` em "Sem autor"/"Perfil removido".
    pub color: Option<String>,
    pub amount: String,
    /// "56,8%" do gasto total do mês.
    pub percent: String,
    pub fill: f32,
    /// "34 despesas".
    pub count: String,
    /// Categoria em que a pessoa mais gastou; `None` sem despesa.
    pub top: Option<DeskTopCategory>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeskTopCategory {
    pub name: String,
    /// "56,8%": parte da categoria no gasto *da pessoa* (não do mês).
    pub percent: String,
    /// "mais em Moradia".
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeskSplit {
    pub name: String,
    pub color: Option<String>,
    pub fraction: f32,
    pub percent: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeskFooter {
    /// "62 lançamentos"; o "Atualizado às …" vem do snapshot, na tela.
    pub count: String,
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

/// Eixo sem o "R$ " do `axis_label` ("15 mil", "500"): o design põe só o número. Não mascara
/// com valores ocultos: o eixo do design é `.n` (escala do gráfico), então "15 mil" e "0" ficam.
fn axis_plain(minor: i64) -> String {
    axis_label(minor).trim_start_matches("R$ ").to_string()
}

/// "5,2 mil" com espaço não separável, para o nome acessível dos gráficos.
fn spoken_thousands(minor: i64) -> String {
    let body = match minor.cmp(&0) {
        std::cmp::Ordering::Equal => "0".to_string(),
        std::cmp::Ordering::Greater => surplus_thousands(minor),
        std::cmp::Ordering::Less => format!("{MINUS}{}", surplus_thousands(minor)),
    };
    body.replace(' ', "\u{a0}")
}

/// "a, b e c" (sem vírgula antes do "e", como no design). Oculto não lista valor nenhum.
fn spoken_list(m: Money, values: &[String]) -> String {
    if m.hidden {
        return "valores ocultos".to_string();
    }
    match values {
        [] => String::new(),
        [only] => only.clone(),
        [init @ .., last] => format!("{} e {last}", init.join(", ")),
    }
}

/// " (novembro parcial)" quando o intervalo tem mês parcial.
fn partial_suffix(rows: &[&HistoryRow]) -> String {
    rows.iter()
        .find(|r| r.partial)
        .map(|r| format!(" ({} parcial)", month_long(&r.month)))
        .unwrap_or_default()
}

/// Valor da sobra em milhares com uma casa fixa ("3,0 mil", "0,4 mil"): ao contrario do eixo
/// ("15 mil"), o design desenha a sobra sempre com decimal, inclusive abaixo de R$ 1.000.
fn surplus_thousands(minor: i64) -> String {
    let tenths = (minor.unsigned_abs() + 5_000) / 10_000;
    format!("{},{} mil", group_thousands(tenths / 10), tenths % 10)
}

/// "+2,5 mil" / "−1,2 mil" / "0" embaixo de cada mês do Histórico.
fn surplus_label(m: Money, minor: i64) -> String {
    if m.hidden {
        return AXIS_MASK.to_string();
    }
    match minor.cmp(&0) {
        std::cmp::Ordering::Equal => "0".to_string(),
        std::cmp::Ordering::Greater => format!("+{}", surplus_thousands(minor)),
        std::cmp::Ordering::Less => format!("{MINUS}{}", surplus_thousands(minor)),
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
        where_it_went: where_card(ds, month, m),
        upcoming: (month == month_of(today)).then(|| upcoming_card(ds, today, m)),
        recent: recent_card(ds, today, month, m),
        people: people_card(ds, month, m),
        footer: DeskFooter {
            count: plural(in_month(ds, month).count(), "lançamento", "lançamentos"),
        },
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
        first_month: (!had_previous).then(|| "Primeiro mês".to_string()),
        link: "Ver lançamentos".to_string(),
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
        let avg = sum / closed.len() as i64;
        // `axis` descarta o sinal (unsigned_abs): sem pôr à mão, prejuízo médio leria como lucro.
        // Oculto não leva sinal, só a máscara.
        // Oculto: a média é `.v` (dinheiro) no design, então leva `MASK` ("R$ •••••"), não a
        // máscara de eixo (`.v.n`).
        if m.hidden {
            MASK.to_string()
        } else if avg < 0 {
            format!("{MINUS}{}", m.axis(avg))
        } else {
            m.axis(avg)
        }
    });
    DeskBusiness {
        title: "NEGÓCIO · PJ".to_string(),
        result: lv("Resultado do mês", m.signed(p.result_minor)),
        result_negative: p.result_minor < 0,
        delta,
        income: lv("Faturado", m.brl(p.income_minor)),
        expense: lv("Despesas PJ", m.brl(p.expense_minor)),
        // Sem mês fechado não há média: o sufixo ficaria pendurado sem valor.
        bars_title: if average.is_some() {
            format!("Resultado PJ · {DESK_MONTHS} meses · média")
        } else {
            format!("Resultado PJ · {DESK_MONTHS} meses")
        },
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
        bars_label: {
            let vals: Vec<String> = rows
                .iter()
                .filter(|r| r.existed)
                .map(|r| spoken_thousands(r.surplus_minor))
                .collect();
            let avg = if m.hidden || closed.is_empty() {
                String::new()
            } else {
                let sum = closed.iter().fold(0i64, |a, v| a.saturating_add(*v));
                format!("; média {}", spoken_thousands(sum / closed.len() as i64))
            };
            format!(
                "Resultado PJ de {} a {}: {}{}{avg}",
                rows.first().map_or("", |r| month_long(&r.month)),
                rows.last().map_or("", |r| month_long(&r.month)),
                spoken_list(m, &vals),
                partial_suffix(&rows),
            )
        },
        average,
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
                "Ainda não coberta"
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
            fill: frac(v.balance_minor, v.goal_minor),
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
            .then(|| "Reserva de emergência não configurada".to_string()),
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
                    title: capitalized(month_long(&r.month)),
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
            axis_plain(top),
            axis_plain(top / 3 * 2),
            axis_plain(top / 3),
            axis_plain(0),
        ],
        chart_label: {
            let sobras: Vec<String> = rows
                .iter()
                .filter(|r| r.existed)
                .map(|r| spoken_thousands(r.surplus_minor))
                .collect();
            format!(
                "Receitas e despesas de {} a {}. Sobras: {}{}.",
                rows.first().map_or("", |r| month_long(&r.month)),
                rows.last().map_or("", |r| month_long(&r.month)),
                spoken_list(m, &sobras),
                partial_suffix(&rows),
            )
        },
        footnote: format!(
            "Valor abaixo do mês = quanto sobrou{}",
            partial_note.unwrap_or_default()
        ),
    }
}

/// Quantas categorias a rosca de "Para onde foi" mostra antes de juntar o resto em "Outros".
const WHERE_SLICES: usize = 5;
/// Janela do "O que vem" (o design fala em 15 dias; o Grafite usa 30).
pub const DESK_UPCOMING_DAYS: i64 = 15;
/// Até onde se procura a primeira saída "Depois:" fora da janela.
const LATER_LOOKAHEAD_DAYS: i64 = 90;
/// Linhas de "Recentes".
pub const DESK_RECENT_LINES: usize = 4;

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("{n} {one}")
    } else {
        format!("{n} {many}")
    }
}

/// "em 4 dias" / "em 1 dia" / "hoje".
fn in_days(today: &str, date: &str) -> String {
    match days_between(today, date) {
        0 => "hoje".to_string(),
        1 => "em 1 dia".to_string(),
        n => format!("em {n} dias"),
    }
}

fn where_card(ds: &Dataset, month: &str, m: Money) -> DeskWhere {
    let slices = expense_by_category(ds, month);
    let total: i64 = slices
        .iter()
        .fold(0i64, |acc, s| acc.saturating_add(s.amount_minor));
    let mut shown: Vec<DeskSlice> = slices
        .iter()
        .take(WHERE_SLICES)
        .map(|s| DeskSlice {
            key: s.key.clone(),
            name: s.name.clone(),
            color: s.color.clone(),
            icon: s.icon.clone(),
            percent: percent_tenths(s.amount_minor, total.max(1)),
            amount: m.brl(s.amount_minor),
            share: s.share as f32,
            other: false,
        })
        .collect();
    if slices.len() > WHERE_SLICES {
        let rest = slices
            .iter()
            .skip(WHERE_SLICES)
            .fold(0i64, |acc, s| acc.saturating_add(s.amount_minor));
        shown.push(DeskSlice {
            key: String::new(),
            name: "Outros".to_string(),
            color: NO_CATEGORY_COLOR.to_string(),
            icon: NO_CATEGORY_ICON.to_string(),
            percent: percent_tenths(rest, total.max(1)),
            amount: m.brl(rest),
            share: frac(rest, total),
            other: true,
        });
    }
    // Maior alta contra o mês anterior (a frase do design é mês a mês; `categories_above_normal`
    // compara com a média). Compara a razão (cur - prev) / prev por multiplicação cruzada, sem
    // float; empate fica com o maior valor, depois o nome.
    let prev_month = shift_month(month, -1);
    let prev = expense_by_category(ds, &prev_month);
    // Só as fatias que o card mostra: uma categoria escondida em "Outros" não pode ser citada
    // numa frase de um card que não a lista.
    let rise = slices
        .iter()
        .take(WHERE_SLICES)
        .filter_map(|s| {
            let before = prev.iter().find(|p| p.key == s.key)?.amount_minor;
            (before > 0 && s.amount_minor > before).then_some((s, before))
        })
        .max_by(|(a, pa), (b, pb)| {
            let ra = i128::from(a.amount_minor - pa) * i128::from(*pb);
            let rb = i128::from(b.amount_minor - pb) * i128::from(*pa);
            ra.cmp(&rb)
                .then_with(|| a.amount_minor.cmp(&b.amount_minor))
                .then_with(|| b.name.cmp(&a.name))
        })
        .map(|(s, before)| {
            let percent = percent_tenths(s.amount_minor - before, before);
            DeskRise {
                sentence: format!(
                    "{} subiu {percent} em relação a {}",
                    s.name,
                    month_long(&prev_month)
                ),
                category: s.name.clone(),
                percent,
            }
        })
        // Alta menor que 0,05% arredonda para "0,0%": não é notícia.
        .filter(|r| r.percent != "0,0%");
    let chart_label = format!(
        "Despesas por categoria: {}",
        shown
            .iter()
            .map(|s| format!("{} {}", s.name, s.percent))
            .collect::<Vec<_>>()
            .join(", ")
    );
    DeskWhere {
        title: "Para onde foi".to_string(),
        total: (total > 0).then(|| m.big_whole(total)),
        total_caption: "gastos no mês".to_string(),
        empty: (total <= 0).then(|| format!("Nenhuma despesa em {}", month_long(month))),
        slices: shown,
        rise,
        chart_label,
    }
}

/// Uma saída futura antes de virar texto: guarda a data para ordenar fatura e recorrência
/// juntas.
struct Due {
    date: String,
    title: String,
    detail: String,
    recurrence: Option<String>,
    amount_minor: i64,
    estimated: bool,
    invoice: bool,
}

/// Saídas (recorrências de despesa e faturas) até `today + days`, por data. A recorrência entra
/// só a partir de amanhã; a fatura entra a partir de hoje (`upcoming_invoices` inclui o
/// vencimento de hoje, e é só por isso que o braço "hoje" de `in_days` é alcançável). Receita
/// recorrente fica de fora: "O que vem" responde "quanto sai da conta".
fn dues(ds: &Dataset, today: &str, days: i64) -> Vec<Due> {
    let mut out: Vec<Due> = upcoming(ds, today, days)
        .into_iter()
        .filter(|o| o.kind == Kind::Expense)
        .map(|o| Due {
            detail: o
                .payment_method_id
                .as_deref()
                .map(|id| ds.payment_method_name(Some(id)))
                .unwrap_or_default(),
            recurrence: ds
                .recurrence_label(Some(&o.recurrence_id))
                .map(|l| l.to_lowercase()),
            title: o
                .description
                .clone()
                .unwrap_or_else(|| "Recorrência".to_string()),
            date: o.date,
            amount_minor: o.amount_minor,
            estimated: o.estimated,
            invoice: false,
        })
        .collect();
    out.extend(upcoming_invoices(ds, today, days).into_iter().map(|inv| {
        let open = inv.status == InvoiceStatus::Open;
        Due {
            title: format!("Fatura {}", inv.card_name),
            detail: if open {
                "Fatura aberta"
            } else {
                "Fatura fechada"
            }
            .to_string(),
            recurrence: None,
            date: inv.due_on,
            amount_minor: inv.total_minor,
            // Fatura aberta é estimativa ("~"), como no Grafite.
            estimated: open,
            invoice: true,
        }
    }));
    // Estável: na mesma data a recorrência fica antes da fatura.
    out.sort_by(|a, b| a.date.cmp(&b.date));
    out
}

fn upcoming_card(ds: &Dataset, today: &str, m: Money) -> DeskUpcoming {
    let limit = shift_day(today, DESK_UPCOMING_DAYS);
    let list = dues(ds, today, DESK_UPCOMING_DAYS);
    let total = list
        .iter()
        .fold(0i64, |acc, d| acc.saturating_add(d.amount_minor));
    let approx = |d: &Due| if d.estimated { "~" } else { "" };
    let later = dues(ds, today, LATER_LOOKAHEAD_DAYS)
        .into_iter()
        .find(|d| d.date > limit)
        .map(|d| DeskLater {
            amount: format!("{}{}", approx(&d), m.brl(d.amount_minor)),
            label: format!("Depois: {} · {}", d.title, partial_label(&d.date)),
            date: partial_label(&d.date),
            title: d.title,
        });
    DeskUpcoming {
        title: "O que vem".to_string(),
        window: format!("Próximos {DESK_UPCOMING_DAYS} dias"),
        total: (!list.is_empty()).then(|| m.big(total)),
        caption: format!("previstos em {}", plural(list.len(), "conta", "contas")),
        empty: list
            .is_empty()
            .then(|| format!("Nada programado nos próximos {DESK_UPCOMING_DAYS} dias.")),
        items: list
            .iter()
            .map(|d| DeskDue {
                day: d.date.get(8..10).unwrap_or_default().to_string(),
                month: month_short(&month_of(&d.date)).to_string(),
                title: d.title.clone(),
                detail: d.detail.clone(),
                recurrence: d.recurrence.clone(),
                amount: format!("{}{}", approx(d), m.brl(d.amount_minor)),
                in_days: in_days(today, &d.date),
                invoice: d.invoice,
            })
            .collect(),
        later,
    }
}

fn recent_card(ds: &Dataset, today: &str, month: &str, m: Money) -> DeskRecent {
    let lines: Vec<DeskRecentLine> = recent(ds, month, DESK_RECENT_LINES)
        .into_iter()
        .map(|tx| {
            let row = to_row(ds, tx, m);
            let income = row.kind == Kind::Income;
            let when = match days_between(&row.occurred_on, today) {
                0 => "hoje".to_string(),
                1 => "ontem".to_string(),
                _ => partial_label(&row.occurred_on),
            };
            let mut meta = vec![row.category_name.clone()];
            if let Some(a) = &row.author {
                meta.push(a.name.clone());
            }
            meta.push(when);
            DeskRecentLine {
                // Mesma regra do Grafite: "−R$ 0,00" não aparece na despesa zerada de propósito.
                amount: if income {
                    m.signed_plus(row.amount_minor)
                } else {
                    m.signed(row.amount_minor.saturating_neg())
                },
                tag: row.report_state.map(|s| s.label().to_string()),
                excluded: row.report_state.is_some(),
                icon: row
                    .category_icon
                    .clone()
                    .unwrap_or_else(|| NO_CATEGORY_ICON.to_string()),
                color: row
                    .category_color
                    .clone()
                    .unwrap_or_else(|| NO_CATEGORY_COLOR.to_string()),
                category: row.category_name.clone(),
                meta: meta.join(" · "),
                description: row.description,
                id: row.id,
                income,
            }
        })
        .collect();
    DeskRecent {
        title: "Recentes".to_string(),
        link: "Ver todos".to_string(),
        empty: lines
            .is_empty()
            .then(|| format!("Nenhum lançamento em {}", month_long(month))),
        lines,
    }
}

/// Autor do lançamento, com a mesma regra de `insights::by_person` (que não expõe a função):
/// sem `userId` é "Sem autor"; `userId` sem perfil vivo é "Perfil removido".
fn author_of(ds: &Dataset, user_id: Option<&str>) -> Author {
    match (user_id, ds.find_user(user_id)) {
        (None, _) => Author::Nobody,
        (Some(_), Some(u)) => Author::Person {
            id: u.id.clone(),
            name: u.name.clone(),
            color: u.color.clone(),
        },
        (Some(_), None) => Author::Removed,
    }
}

fn people_card(ds: &Dataset, month: &str, m: Money) -> Option<DeskPeople> {
    let groups = by_person(ds, month);
    if groups.is_empty() {
        return None;
    }
    let total = groups
        .iter()
        .fold(0i64, |acc, g| acc.saturating_add(g.expense_minor));
    let rows: Vec<DeskPerson> = groups
        .iter()
        .map(|g| {
            // (id da categoria, nome, soma) das despesas da pessoa.
            let mut count = 0usize;
            let mut by_cat: Vec<(String, String, i64)> = Vec::new();
            for t in in_month(ds, month)
                .filter(|t| t.kind == Kind::Expense && t.counts_in_totals())
                .filter(|t| author_of(ds, t.user_id.as_deref()) == g.author)
            {
                count += 1;
                let key = ds
                    .find_category(t.category_id.as_deref())
                    .map_or(NO_CATEGORY_KEY, |c| c.id.as_str());
                match by_cat.iter_mut().find(|(k, _, _)| k == key) {
                    Some(e) => e.2 = e.2.saturating_add(t.amount_minor),
                    None => by_cat.push((
                        key.to_string(),
                        ds.category_name(t.category_id.as_deref()),
                        t.amount_minor,
                    )),
                }
            }
            let top = by_cat
                .iter()
                .filter(|_| g.expense_minor > 0)
                .max_by(|a, b| a.2.cmp(&b.2).then_with(|| b.1.cmp(&a.1)))
                .map(|(_, name, amount)| DeskTopCategory {
                    percent: percent_tenths(*amount, g.expense_minor),
                    text: format!("mais em {name}"),
                    name: name.clone(),
                });
            let name = g.author.label().to_string();
            DeskPerson {
                initial: name
                    .chars()
                    .next()
                    .map_or_else(String::new, |c| c.to_uppercase().collect()),
                color: match &g.author {
                    Author::Person { color, .. } => Some(color.clone()),
                    _ => None,
                },
                amount: m.brl(g.expense_minor),
                percent: if total > 0 {
                    percent_tenths(g.expense_minor, total)
                } else {
                    "0,0%".to_string()
                },
                fill: frac(g.expense_minor, total),
                count: plural(count, "despesa", "despesas"),
                top,
                name,
            }
        })
        .collect();
    let split: Vec<DeskSplit> = rows
        .iter()
        .map(|p| DeskSplit {
            name: p.name.clone(),
            color: p.color.clone(),
            fraction: p.fill,
            percent: p.percent.clone(),
        })
        .collect();
    Some(DeskPeople {
        title: "Pessoas".to_string(),
        subtitle: "Gasto por pessoa".to_string(),
        split_label: format!(
            "Divisão dos gastos: {}",
            split
                .iter()
                .map(|s| format!("{} {}", s.name, s.percent))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        rows,
        split,
        link: "Ver lançamentos por pessoa".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dashboard::contract::Book;
    use crate::dashboard::dataset::{self, BOOKS, TRANSACTIONS};
    use crate::dashboard::money::leaks;
    use crate::dashboard::{GRAFITE_TODAY, dataset_from, fixtures_grafite, grafite_rows};
    use serde_json::json;

    fn nov() -> DeskView {
        build_desk(&fixtures_grafite(), None, GRAFITE_TODAY, "2026-11", false)
    }

    /// Fixture: novembro (até o dia 19) tem receitas de R$ 3.500,00 e despesas de R$ 3.113,41;
    /// outubro fechou com sobra de R$ 1.410,29 (a mesma "sobra média" que o Grafite fixa).
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
        assert_eq!(c.first_month, None);
        assert_eq!(c.link, "Ver lançamentos");
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

    /// Conta comercial: novembro com 800 de venda e 300 de despesa; outubro com `oct` (positivo
    /// é venda, negativo vira despesa, para fechar o mês no prejuízo; `None` sem lançamento).
    fn negocio_fixture(oct: Option<i64>) -> (Dataset, Dataset) {
        let mut rows = grafite_rows();
        rows.push((
            BOOKS,
            "BK".to_string(),
            json!({ "kind": "business", "name": "Padaria", "color": "amber", "icon": "s" }),
        ));
        let mut txs = vec![
            ("1110-1-VENDA", "income", 80_000, "2026-11-10"),
            ("1111-1-FARINHA", "expense", 30_000, "2026-11-11"),
        ];
        if let Some(v) = oct {
            let kind = if v >= 0 { "income" } else { "expense" };
            txs.push(("1010-1-VENDA", kind, v.abs(), "2026-10-10"));
        }
        for (id, kind, amount, on) in txs {
            rows.push((
                TRANSACTIONS,
                id.to_string(),
                json!({ "kind": kind, "description": "x", "amountMinor": amount,
                        "occurredOn": on, "book": "business", "userId": "UL" }),
            ));
        }
        let all = dataset_from(rows);
        let biz = all.for_book(Book::Business);
        (all, biz)
    }

    #[test]
    fn negocio_some_sem_conta_comercial_e_calcula_com_ela() {
        assert_eq!(nov().business, None);
        let (all, biz) = negocio_fixture(Some(40_000));
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
        assert_eq!(b.bars_title, "Resultado PJ · 6 meses · média");
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
    fn negocio_com_mes_fechado_negativo_mantem_o_sinal_da_media() {
        let (all, biz) = negocio_fixture(Some(-40_000));
        let v = build_desk(
            &all.for_book(Book::Personal),
            Some(&biz),
            GRAFITE_TODAY,
            "2026-11",
            false,
        );
        let b = v.business.expect("negocio");
        // Só outubro está fechado, com resultado de -R$ 400.
        assert_eq!(b.average.as_deref(), Some("\u{2212}R$ 400"));
    }

    #[test]
    fn negocio_sem_mes_fechado_tira_o_sufixo_media_do_titulo() {
        let (all, biz) = negocio_fixture(None);
        let v = build_desk(
            &all.for_book(Book::Personal),
            Some(&biz),
            GRAFITE_TODAY,
            "2026-11",
            false,
        );
        let b = v.business.expect("negocio");
        assert_eq!(b.average, None);
        assert_eq!(b.bars_title, "Resultado PJ · 6 meses");
    }

    #[test]
    fn primeiro_mes_sem_lancamento_anterior() {
        // A fixture só tem lançamento a partir de outubro: outubro é o primeiro mês.
        let v = build_desk(&fixtures_grafite(), None, GRAFITE_TODAY, "2026-10", false);
        assert_eq!(v.how_am_i.delta, None);
        assert_eq!(v.how_am_i.first_month.as_deref(), Some("Primeiro mês"));
    }

    #[test]
    fn reserva_da_fixture() {
        let r = nov().reserve;
        let e = r.emergency.expect("emergencia");
        // Cobertura = reserva de emergência / custo essencial = 350 / 2.500 = 0,14 -> "0,1".
        assert_eq!(e.coverage, "0,1");
        assert_eq!(e.headline, "Ainda não coberta");
        assert_eq!(e.caption, "de custo essencial guardados · meta: 6 meses");
        assert!(!e.reached);
        // 350 / (6 x 2.500 = 15.000) = 0,0233.
        assert!((e.fill - 0.023_333).abs() < 1e-4);
        assert_eq!(e.balance.value, "R$ 350,00");
        assert_eq!(e.essential.expect("custo").value, "R$ 2.500,00/mês");
        // Todas as reservas vivas: emergência 550 + 100 - 300 = 350 e a caixinha Casamento
        // com 150 -> 500.
        assert_eq!(r.total.value, "R$ 500,00");
        // Só há uma retirada de R$ 200 em novembro.
        let f = r.month_flow.expect("movimento");
        assert_eq!(
            (f.label.as_str(), f.value.as_str()),
            ("Retirado em novembro", "\u{2212}R$ 200,00")
        );
        assert_eq!(r.link, "Ver reservas");
    }

    /// Estado principal do design: reserva coberta, mês com guardado positivo e despesa acima
    /// da receita no mesmo mês.
    #[test]
    fn reserva_coberta_guardado_positivo_e_despesa_acima_da_receita() {
        let mut rows = vec![(
            dataset::RESERVES,
            "RE".to_string(),
            json!({ "kind": "emergency", "name": "Reserva", "icon": "lifebuoy",
                    "color": "violet", "multiple": 1, "essentialOverrideMinor": 100_000 }),
        )];
        // Depósito de 1.000,00 em novembro: saldo igual à meta (1 x 1.000,00).
        rows.push((
            dataset::RESERVE_MOVEMENTS,
            "MV".to_string(),
            json!({ "reserveId": "RE", "amountMinor": 100_000, "occurredOn": "2026-11-06",
                    "userId": "UL", "recurring": false }),
        ));
        // Receita 1.000,00 e despesa 1.500,00: despesa passa da receita (150%).
        for (id, kind, amount) in [("T1", "income", 100_000), ("T2", "expense", 150_000)] {
            rows.push((
                TRANSACTIONS,
                id.to_string(),
                json!({ "kind": kind, "description": "x", "amountMinor": amount,
                        "occurredOn": "2026-11-10", "book": "personal", "userId": "UL" }),
            ));
        }
        let v = build_desk(&dataset_from(rows), None, GRAFITE_TODAY, "2026-11", false);
        let e = v.reserve.emergency.expect("emergencia");
        // Saldo 1.000,00 >= meta 1.000,00: coberta; cobertura 1.000/1.000 = "1,0" (singular).
        assert_eq!(e.headline, "Emergência coberta");
        assert!(e.reached);
        assert_eq!(e.fill, 1.0);
        assert_eq!(e.coverage, "1,0");
        assert_eq!(e.unit, "mês");
        let f = v.reserve.month_flow.expect("movimento");
        assert_eq!(
            (f.label.as_str(), f.value.as_str()),
            ("Guardado em novembro", "+R$ 1.000,00")
        );
        // 1.500 / 1.000 passa de 100%: `over` e a barra trava em 1.
        let u = v.how_am_i.usage.expect("uso");
        assert!(u.over);
        assert_eq!(u.fill, 1.0);
        assert_eq!(u.percent, "150,0%");
    }

    #[test]
    fn sobra_sempre_com_uma_casa() {
        let m = Money { hidden: false };
        assert_eq!(surplus_label(m, 300_000), "+3,0 mil");
        assert_eq!(surplus_label(m, 295_000), "+3,0 mil");
        assert_eq!(surplus_label(m, 38_600), "+0,4 mil");
        assert_eq!(surplus_label(m, -125_000), format!("{MINUS}1,3 mil"));
        assert_eq!(surplus_label(m, 0), "0");
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
        assert_eq!(tip.title, "Outubro");
    }

    #[test]
    fn oculto_nao_vaza_digito() {
        let v = build_desk(&fixtures_grafite(), None, GRAFITE_TODAY, "2026-11", true);
        assert_eq!(leaks(&format!("{v:?}")), None);
        // Card Negócio (resultado, faturado, despesas PJ, média) também mascara.
        let (all, biz) = negocio_fixture(Some(-40_000));
        let hidden = build_desk(
            &all.for_book(Book::Personal),
            Some(&biz),
            GRAFITE_TODAY,
            "2026-11",
            true,
        );
        assert!(hidden.business.is_some());
        assert_eq!(
            hidden.business.as_ref().and_then(|b| b.average.as_deref()),
            Some(MASK)
        );
        assert_eq!(leaks(&format!("{:?}", hidden.business)), None);
        // Forma continua visível: barras e porcentagem não são valores.
        assert!(v.history.months[5].income > 0.0);
        assert!(v.how_am_i.usage.is_some());
        assert_eq!(v.how_am_i.balance.whole, "•••••");
        // Eixo é escala (`.n` no design): continua legível, e o "0" nunca vira máscara.
        assert_eq!(v.history.y_labels, ["6 mil", "4 mil", "2 mil", "0"]);
        assert_eq!(v.history.months[4].surplus, "•••");
        assert_eq!(
            v.history.chart_label,
            "Receitas e despesas de junho a novembro. Sobras: valores ocultos (novembro parcial)."
        );
        assert_eq!(
            hidden.business.as_ref().map(|b| b.bars_label.as_str()),
            Some("Resultado PJ de junho a novembro: valores ocultos (novembro parcial)")
        );
    }

    #[test]
    fn nome_acessivel_dos_graficos_visiveis() {
        let (all, biz) = negocio_fixture(Some(60_000));
        let v = build_desk(
            &all.for_book(Book::Personal),
            Some(&biz),
            GRAFITE_TODAY,
            "2026-11",
            false,
        );
        let b = v.business.expect("negocio");
        assert_eq!(
            b.bars_label,
            "Resultado PJ de junho a novembro: 0,6\u{a0}mil e 0,5\u{a0}mil (novembro parcial); média 0,6\u{a0}mil"
        );
        assert_eq!(
            v.history.chart_label,
            "Receitas e despesas de junho a novembro. Sobras: 1,4\u{a0}mil e 0,4\u{a0}mil (novembro parcial)."
        );
        assert!(!v.history.chart_label.contains('+'));
    }

    /// Fixture, novembro: despesas que contam somam R$ 3.113,41 (Mercado R$ 1.089,34 = 35,0%).
    /// Há mais de cinco categorias: as cinco maiores ficam e o resto vira "Outros"
    /// (R$ 400,65 = 12,9%).
    #[test]
    fn para_onde_foi_da_fixture() {
        let w = nov().where_it_went;
        assert_eq!(w.total.as_ref().map(|t| t.whole.as_str()), Some("3.113"));
        let got: Vec<(&str, &str, &str)> = w
            .slices
            .iter()
            .map(|s| (s.name.as_str(), s.percent.as_str(), s.amount.as_str()))
            .collect();
        assert_eq!(
            got,
            vec![
                ("Mercado", "35,0%", "R$ 1.089,34"),
                ("Contas", "17,3%", "R$ 539,01"),
                ("Compras", "16,5%", "R$ 515,15"),
                ("Conforto", "12,7%", "R$ 394,36"),
                ("Sogrinha", "5,6%", "R$ 174,90"),
                ("Outros", "12,9%", "R$ 400,65"),
            ]
        );
        assert!(w.slices.last().is_some_and(|s| s.other));
        let sum: f32 = w.slices.iter().map(|s| s.share).sum();
        assert!((sum - 1.0).abs() < 0.001);
        assert!(
            w.chart_label
                .starts_with("Despesas por categoria: Mercado 35,0%")
        );
        assert_eq!(w.empty, None);
    }

    /// Outubro: Compras R$ 423,82; novembro R$ 515,15; (51515 - 42382) / 42382 = 21,5%.
    /// Mercado caiu (R$ 3.238,18 para R$ 1.089,34), então não concorre.
    #[test]
    fn maior_alta_contra_o_mes_anterior() {
        let r = nov().where_it_went.rise.expect("Compras subiu");
        assert_eq!(r.category, "Compras");
        assert_eq!(r.percent, "21,5%");
        assert_eq!(r.sentence, "Compras subiu 21,5% em relação a outubro");
        // Sem despesa em setembro, outubro não tem base de comparação: sem frase.
        let oct = build_desk(&fixtures_grafite(), None, GRAFITE_TODAY, "2026-10", false);
        assert_eq!(oct.where_it_went.rise, None);
    }

    /// Uma categoria pequena que some em "Outros" e sobe muito não pode ser citada na frase.
    #[test]
    fn alta_so_cita_categoria_que_o_card_mostra() {
        let mut rows = Vec::new();
        for i in 0..7 {
            rows.push((
                dataset::CATEGORIES,
                format!("C{i}"),
                json!({ "name": format!("Cat{i}"), "color": "orange", "icon": "utensils" }),
            ));
        }
        // Cat0..Cat4 grandes (Cat0 sobe 10%); Cat6 ("Pets") vai de 2,00 a 40,00 e fica em Outros.
        let mut txs = vec![
            ("C0", 110_000, "2026-11-05"),
            ("C0", 100_000, "2026-10-05"),
            ("C6", 4_000, "2026-11-06"),
            ("C6", 200, "2026-10-06"),
            ("C5", 3_000, "2026-11-07"),
        ];
        for i in 1..5 {
            txs.push((
                ["C0", "C1", "C2", "C3", "C4"][i],
                90_000 - i as i64 * 1_000,
                "2026-11-08",
            ));
        }
        for (n, (cat, amount, on)) in txs.into_iter().enumerate() {
            rows.push((
                TRANSACTIONS,
                format!("T{n}"),
                json!({ "kind": "expense", "description": "x", "amountMinor": amount,
                        "occurredOn": on, "categoryId": cat, "userId": "UL" }),
            ));
        }
        let v = build_desk(&dataset_from(rows), None, GRAFITE_TODAY, "2026-11", false);
        assert!(v.where_it_went.slices.last().is_some_and(|s| s.other));
        let r = v.where_it_went.rise.expect("Cat0 subiu");
        assert_eq!(r.category, "Cat0");
        assert!(!r.sentence.contains("Cat6"));
    }

    /// Hoje é 19/11: a janela de 15 dias vai até 4/dez, e só entram saídas (R$ 50,52 + 9,99 +
    /// 19,99 + 168,62 = R$ 249,12, com centavos). A fatura do Mercado Pago vence
    /// 8/dez, fora da janela, e vira o "Depois:".
    #[test]
    fn o_que_vem_da_fixture() {
        let u = nov().upcoming.expect("mes corrente");
        assert_eq!(u.window, "Próximos 15 dias");
        // Com centavos, como no design ("R$ 249,12 previstos em 4 contas").
        assert_eq!(
            u.total
                .as_ref()
                .map(|t| (t.whole.as_str(), t.cents.as_str())),
            Some(("249", ",12"))
        );
        assert_eq!(u.caption, "previstos em 4 contas");
        let got: Vec<(&str, &str, &str, &str, &str)> = u
            .items
            .iter()
            .map(|d| {
                (
                    d.day.as_str(),
                    d.month.as_str(),
                    d.title.as_str(),
                    d.amount.as_str(),
                    d.in_days.as_str(),
                )
            })
            .collect();
        assert_eq!(
            got,
            vec![
                ("21", "nov", "Cadeira (2x)", "R$ 50,52", "em 2 dias"),
                ("01", "dez", "Meli+", "R$ 9,99", "em 12 dias"),
                ("01", "dez", "Amazon prime", "R$ 19,99", "em 12 dias"),
                ("02", "dez", "Mesada da sogrinha", "R$ 168,62", "em 13 dias"),
            ]
        );
        assert_eq!(u.items[0].recurrence.as_deref(), Some("mensal"));
        let later = u.later.expect("fatura depois da janela");
        assert_eq!(
            (
                later.title.as_str(),
                later.date.as_str(),
                later.amount.as_str(),
                later.label.as_str()
            ),
            (
                "Fatura Mercado Pago",
                "8 dez",
                "~R$ 515,15",
                "Depois: Fatura Mercado Pago · 8 dez"
            )
        );
        assert_eq!(u.empty, None);
    }

    #[test]
    fn o_que_vem_so_no_mes_corrente_e_vazio_diz_o_porque() {
        let oct = build_desk(&fixtures_grafite(), None, GRAFITE_TODAY, "2026-10", false);
        assert_eq!(oct.upcoming, None);
        // Sem nenhuma série nem fatura, a janela fica vazia.
        let far = build_desk(
            &dataset_from(Vec::new()),
            None,
            GRAFITE_TODAY,
            "2026-11",
            false,
        );
        let u = far.upcoming.expect("mes corrente");
        assert_eq!(u.total, None);
        assert_eq!(u.caption, "previstos em 0 contas");
        assert_eq!(
            u.empty.as_deref(),
            Some("Nada programado nos próximos 15 dias.")
        );
        assert_eq!(u.later, None);
    }

    /// Dia 19: 18/11 é "ontem"; 17/11 já é a data. Os quatro mais recentes por data e id.
    #[test]
    fn recentes_da_fixture() {
        let r = nov().recent;
        let got: Vec<(&str, &str, &str)> = r
            .lines
            .iter()
            .map(|l| (l.description.as_str(), l.meta.as_str(), l.amount.as_str()))
            .collect();
        assert_eq!(
            got,
            vec![
                (
                    "Claude code",
                    "Assinaturas · Luiz · ontem",
                    "\u{2212}R$ 118,40"
                ),
                ("Almoço", "Conforto · Luiz · ontem", "\u{2212}R$ 20,97"),
                ("Jantinha", "Conforto · Luiz · ontem", "\u{2212}R$ 65,89"),
                ("Uber", "Transporte · Gigi · 17 nov", "\u{2212}R$ 50,00"),
            ]
        );
        assert_eq!(r.lines[0].category, "Assinaturas");
        assert_eq!(r.link, "Ver todos");
        assert_eq!(r.empty, None);
    }

    /// Luiz: 14 despesas, R$ 2.640,41 (84,8% de R$ 3.113,41); a maior é Mercado, 32,4% do gasto
    /// *dele*. Gigi: 5 despesas, R$ 473,00 (15,2%), Mercado em 49,6% do dela.
    #[test]
    fn pessoas_da_fixture() {
        let p = nov().people.expect("dois autores");
        let rows: Vec<_> = p
            .rows
            .iter()
            .map(|r| {
                (
                    r.name.as_str(),
                    r.amount.as_str(),
                    r.percent.as_str(),
                    r.count.as_str(),
                    r.top.as_ref().map(|t| t.text.as_str()),
                    r.top.as_ref().map(|t| t.percent.as_str()),
                )
            })
            .collect();
        assert_eq!(
            rows,
            vec![
                (
                    "Luiz",
                    "R$ 2.640,41",
                    "84,8%",
                    "14 despesas",
                    Some("mais em Mercado"),
                    Some("32,4%")
                ),
                (
                    "Gigi",
                    "R$ 473,00",
                    "15,2%",
                    "5 despesas",
                    Some("mais em Mercado"),
                    Some("49,6%")
                ),
            ]
        );
        assert_eq!(p.split_label, "Divisão dos gastos: Luiz 84,8%, Gigi 15,2%");
        assert_eq!(p.rows[0].initial, "L");
    }

    #[test]
    fn rodape_conta_os_lancamentos_do_mes() {
        // 20 lançamentos vivos em novembro (a mesma conta do "20 em novembro" do Grafite).
        assert_eq!(nov().footer.count, "20 lançamentos");
    }

    #[test]
    fn mes_sem_nada_tem_vazios_e_sem_pessoas() {
        let v = build_desk(&fixtures_grafite(), None, GRAFITE_TODAY, "2026-03", false);
        assert_eq!(v.where_it_went.total, None);
        assert_eq!(
            v.where_it_went.empty.as_deref(),
            Some("Nenhuma despesa em março")
        );
        assert!(v.where_it_went.slices.is_empty());
        assert_eq!(
            v.recent.empty.as_deref(),
            Some("Nenhum lançamento em março")
        );
        assert_eq!(v.people, None);
        assert_eq!(v.footer.count, "0 lançamentos");
    }

    /// Com um autor só o card some (`by_person` devolve vazio): nada a combinar.
    #[test]
    fn uma_pessoa_so_nao_mostra_pessoas() {
        let one = dataset_from(
            grafite_rows()
                .into_iter()
                .filter(|(table, _, data)| *table != TRANSACTIONS || data["userId"] != json!("UG"))
                .collect(),
        );
        let v = build_desk(&one, None, GRAFITE_TODAY, "2026-11", false);
        assert_eq!(v.people, None);
    }

    #[test]
    fn oculto_mascara_os_blocos_novos_e_mantem_a_forma() {
        let v = build_desk(&fixtures_grafite(), None, GRAFITE_TODAY, "2026-11", true);
        assert_eq!(leaks(&format!("{:?}", v.where_it_went)), None);
        assert_eq!(leaks(&format!("{:?}", v.upcoming)), None);
        assert_eq!(leaks(&format!("{:?}", v.recent)), None);
        assert_eq!(leaks(&format!("{:?}", v.people)), None);
        assert_eq!(v.where_it_went.slices[0].amount, MASK);
        assert_eq!(v.recent.lines[0].amount, MASK);
        // Porcentagens e frações são forma, não valor.
        assert_eq!(v.where_it_went.slices[0].percent, "35,0%");
        assert!(v.where_it_went.slices[0].share > 0.3);
        assert_eq!(
            v.people.as_ref().map(|p| p.rows[0].percent.as_str()),
            Some("84,8%")
        );
    }
}
