//! Modelo de tela do Dashboard "Grafite" (Resumo da casa; spec do dashboard Grafite 2, 7 e 5.12).
//!
//! Toda `String` daqui ja sai em portugues e formatada: a UI so desenha. Todo valor em R$ passa
//! pelo `Money` com `hidden` (spec 7.4), entao "Ocultar valores" e uma flag na construcao, nao um
//! cuidado de cada tela. Pelo mesmo motivo o modelo nao carrega `_minor` cru: series e barras ja
//! vem como fracoes (0..=1) do teto do eixo, e a forma continua visivel com os valores ocultos.
//!
//! Substituiu o `DashboardView` antigo na H4b; a tela e `ui/dashboard_view.rs`.

use super::aggregate::{
    NO_CATEGORY_COLOR, axis_top, expense_by_category, in_month, js_round, recent, totals,
};
use super::budgets::{BudgetLevel, BudgetStatus, budget_report};
use super::business::business_panel;
use super::cards::{INVOICE_WARNING_PERCENT, InvoiceStatus, upcoming_invoices};
use super::contract::{Book, Kind};
use super::dataset::Dataset;
use super::insights::{
    Author, SAVINGS_TARGET_PERCENT, by_person, categories_above_normal, fixed_variable, forecast,
    history, normal_balance, pace_axis_top, plan_fit, round_to_tens, savings_rate, spending_pace,
    zero_allowance,
};
use super::list::{Filters, list_month, to_row};
use super::money::{BigMoney, MINUS, Money};
use super::periods::{
    days_between, days_in_month, month_long, month_of, month_short, month_short_year,
    partial_label, short_date,
};
use super::recurrence::{committed_percent, next_month_free, project_month, upcoming};
use super::reserves::{
    EmergencyEta, Pace, ReserveColor, emergency_view, month_movements, next_milestone, ordered,
    pot_row, saved_in_month,
};

/// Janela de "Próximas saídas" e das faturas, em dias.
pub const UPCOMING_DAYS: i64 = 30;
/// Itens de "Próximas saídas" antes de "Ver todas as N".
pub const UPCOMING_PREVIEW: usize = 3;
/// Linhas de "Últimos lançamentos" (eram 6 no Dashboard antigo).
pub const RECENT_LINES: usize = 3;
/// Caixinhas no card da reserva.
pub const POT_LINES: usize = 2;
/// Escala das barras "em atenção": a meta fica em 1 / 1,3 = 76,9% (spec 3.3.5).
pub const ATTENTION_SCALE: f64 = 1.3;
/// Passo do teto comum das "Demais categorias" (spec 2.3, inventada): R$ 500.
pub const OTHERS_STEP: i64 = 50_000;
/// Marcas de referencia do comprometido.
pub const COMMITTED_MARKS: [f32; 2] = [0.5, 0.7];

/// Cor semantica de um trecho de texto; a UI traduz para token.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tone {
    #[default]
    Neutral,
    /// `income-fg`: abaixo do normal, vai sobrar.
    Income,
    /// `expense-fg`: meta estourada, saldo abaixo do normal.
    Expense,
    /// `warning`: atencao (acima do normal, vai faltar, precisa de deposito).
    Warning,
}

/// Secoes na ordem em que a tela as desenha (spec 7.2, 7.1, 7.3, 8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    /// Hub sem nenhum lancamento vivo: so o card vazio.
    Empty,
    Business,
    HowAmI,
    Reserve,
    History,
    WhereItWent,
    WhatsNext,
    Recent,
    People,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GrafiteView {
    pub month: String,
    pub is_current: bool,
    /// Hub sem lancamento vivo na conta (estado "Sem dados", spec 7.3).
    pub is_empty: bool,
    pub hidden: bool,
    pub ignored_total: usize,
    pub sidebar: Sidebar,
    pub header: Header,
    /// So na conta comercial, antes de "Como estou".
    pub business: Option<BusinessBlock>,
    pub how_am_i: HowAmI,
    pub reserve: ReserveCard,
    pub history: HistoryCard,
    pub where_it_went: WhereItWent,
    /// So no mes corrente (como no app).
    pub whats_next: Option<WhatsNext>,
    pub recent: Recent,
    /// Some com menos de dois autores.
    pub people: Option<People>,
}

impl GrafiteView {
    pub fn sections(&self) -> Vec<Section> {
        if self.is_empty {
            return vec![Section::Empty];
        }
        let mut out = Vec::new();
        if self.business.is_some() {
            out.push(Section::Business);
        }
        out.extend([
            Section::HowAmI,
            Section::Reserve,
            Section::History,
            Section::WhereItWent,
        ]);
        if self.whats_next.is_some() {
            out.push(Section::WhatsNext);
        }
        out.push(Section::Recent);
        if self.people.is_some() {
            out.push(Section::People);
        }
        out
    }
}

/// O que a lateral tira do dataset. Status, endereco e ultimo sync vem do `Snapshot` (shell),
/// nao daqui: `dashboard/` nao conhece o hub.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sidebar {
    /// "Lançamentos 91": lancamentos vivos da conta.
    pub transactions: usize,
    /// "Reservas 2".
    pub reserves: usize,
    /// `false` = "Hub ligado · sem dados".
    pub has_data: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Header {
    /// "Resumo da casa".
    pub title: String,
    /// "Somente leitura · dados do último sync".
    pub subtitle: String,
    /// "Parcial · até 19 nov": mes corrente com dados.
    pub partial: Option<String>,
    /// "Nov 2026".
    pub month_label: String,
    /// Mostra Pessoal | Comercial.
    pub book_switch: bool,
    /// Tooltip do olho: "Ocultar valores" / "Mostrar valores".
    pub hide_toggle: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LabeledValue {
    pub label: String,
    pub value: String,
}

/// Bloco "Negócio" (spec 7.2): resultado do mes e, com o chip "hoje", capital e dividas.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BusinessBlock {
    pub title: String,
    pub question: String,
    pub result: LabeledValue,
    pub result_tone: Tone,
    pub income: LabeledValue,
    pub expense: LabeledValue,
    pub capital_title: String,
    pub capital: Vec<LabeledValue>,
    pub debt_title: String,
    pub debt: LabeledValue,
    /// "hoje": capital e dividas sao o saldo de agora, nao do mes do seletor.
    pub today_chip: String,
    pub note: String,
}

/// Uma frase com cor.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Line {
    pub text: String,
    pub tone: Tone,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HowAmI {
    pub title: String,
    pub question: String,
    /// "Saldo até hoje · dia 19" / "Saldo do mês".
    pub balance_label: String,
    pub balance: BigMoney,
    pub balance_negative: bool,
    /// Mes corrente: a previsao ("Vão faltar ~R$ 960 ..."); mes fechado: contra o seu normal.
    pub outlook: Option<Line>,
    pub zero_box: Option<ZeroBox>,
    /// "Sua média até aqui: ~R$ 118 por dia no gasto do dia a dia (avulso)".
    pub casual_average: Option<String>,
    pub income: LabeledValue,
    pub expense: LabeledValue,
    pub savings: Option<Savings>,
    /// "R$ 200 saíram das reservas neste mês".
    pub reserves_line: Option<String>,
    pub pace: PaceChart,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ZeroBox {
    /// "Para fechar no zero".
    pub title: String,
    /// "~R$ 30 por dia"; `None` quando as contas que vem ja passam do saldo.
    pub value: Option<String>,
    /// "até o dia 30".
    pub until: Option<String>,
    /// "As contas que ainda vêm já passam do saldo do mês".
    pub note: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Savings {
    pub title: String,
    /// "alvo: 20% da renda".
    pub target: String,
    /// "11% até agora" / "28% no mês".
    pub rate: String,
    /// "no ritmo atual, ~−27%": `expense-fg` quando a previsao e de faltar (taxa negativa).
    pub pace: Option<Line>,
    /// Barra 0..20%.
    pub fill: f32,
    pub scale_start: String,
    pub scale_end: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PaceChart {
    pub title: String,
    pub info: String,
    /// "nov".
    pub legend_month: String,
    pub legend_forecast: Option<String>,
    pub legend_normal: Option<String>,
    pub legend_income: Option<String>,
    pub days: usize,
    /// Dia do corte (hoje no mes corrente, o ultimo num mes fechado).
    pub cutoff: usize,
    /// Acumulado ate o corte, um por dia, fracao do teto.
    pub current: Vec<f32>,
    /// "Seu normal" dia a dia; vazio sem mes de referencia.
    pub normal: Vec<f32>,
    /// Reta tracejada de (corte, acumulado) ate (ultimo dia, despesa prevista).
    pub forecast: Option<(f32, f32)>,
    pub income: Option<f32>,
    /// Eixo y de cima para baixo: "R$ 5 mil", "R$ 2,5 mil", "R$ 0".
    pub y_labels: [String; 3],
    /// Eixo x: (dia, rotulo) — 1, 10, "hoje", ultimo.
    pub x_labels: Vec<(usize, String)>,
    /// "~R$ 4.460".
    pub forecast_label: Option<String>,
    /// "normal R$ 3.662".
    pub normal_label: Option<String>,
    /// "renda R$ 3.500".
    pub income_label: Option<String>,
    pub sentence: PaceSentence,
}

/// "R$ 3.113,41 até o dia 19 — 15% acima do seu normal", com o trecho colorido separado.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PaceSentence {
    pub lead: String,
    /// "15% acima" / "8% abaixo" / "igual"; `None` sem normal.
    pub change: Option<Line>,
    /// "do seu normal" / "ao seu normal".
    pub tail: Option<String>,
}

impl PaceSentence {
    /// A frase corrida, para os testes (a tela desenha os trechos com cor).
    #[cfg(test)]
    pub fn text(&self) -> String {
        match (&self.change, &self.tail) {
            (Some(c), Some(t)) => format!("{} — {} {t}", self.lead, c.text),
            _ => self.lead.clone(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReserveCard {
    pub emergency: EmergencyBlock,
    pub month: ReserveMonth,
    pub pots: Vec<PotLine>,
    /// "Ver reservas".
    pub link: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EmergencyBlock {
    /// Em caixa: o card e bem maior que o estado vazio.
    Present(Box<EmergencyCard>),
    /// Spec 7.6.
    Missing {
        title: String,
        message: String,
        hint: String,
    },
}

impl Default for EmergencyBlock {
    fn default() -> Self {
        Self::Missing {
            title: String::new(),
            message: String::new(),
            hint: String::new(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EmergencyCard {
    pub title: String,
    /// "hoje": a reserva e o saldo de agora em qualquer mes.
    pub chip: String,
    /// "0,1" ou "—".
    pub coverage: String,
    /// "de 6 meses cobertos".
    pub coverage_suffix: String,
    pub segments: Vec<f32>,
    pub segment_labels: Vec<String>,
    pub milestone: Option<MilestoneLine>,
    /// "Meta alcançada".
    pub reached: Option<String>,
    /// "Meta final: 6 meses de custo essencial, R$ 15.000."
    pub goal_line: String,
    /// "Se guardar R$ 550 por mês, a meta fecha em ~fev 2029."
    pub eta_line: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MilestoneLine {
    /// "Próximo marco:".
    pub label: String,
    /// "1 mês (R$ 2.500)".
    pub target: String,
    /// "· faltam R$ 2.150".
    pub missing: String,
}

/// "Neste mês" / "Em outubro".
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReserveMonth {
    pub title: String,
    /// ("Guardado", "+R$ 100,00"); `None` sem deposito no mes.
    pub deposited: Option<LabeledValue>,
    /// ("Retirado", "−R$ 300,00").
    pub withdrawn: Option<LabeledValue>,
    /// "Nenhum movimento".
    pub empty: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PotLine {
    pub name: String,
    pub icon: String,
    pub color: ReserveColor,
    /// "R$ 150 de R$ 1.500 · até nov 2027 · sem depósito mensal".
    pub meta: String,
    /// "precisa de ~R$ 113/mês".
    pub pace: Option<Line>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HistoryCard {
    pub title: String,
    pub legend_income: String,
    pub legend_expense: String,
    pub body: HistoryBody,
    pub average: AverageLine,
}

#[derive(Clone, Debug, PartialEq)]
pub enum HistoryBody {
    /// Menos de 4 meses de historico (spec 7.5): uma linha por mes, o mais antigo primeiro, e a
    /// nota de quando o grafico aparece.
    List {
        lines: Vec<HistoryLine>,
        footnote: String,
    },
    Chart(HistoryChart),
}

impl Default for HistoryBody {
    fn default() -> Self {
        Self::List {
            lines: Vec::new(),
            footnote: String::new(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HistoryLine {
    /// "out · fechado" / "nov · parcial, até dia 19".
    pub label: String,
    /// "sobrou R$ 1.410 · 28%" / "R$ 387 até agora · 11%".
    pub text: String,
    pub income: f32,
    pub expense: f32,
    /// Barras em contorno.
    pub partial: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HistoryChart {
    pub columns: Vec<HistoryColumn>,
    /// Eixo R$ de cima para baixo.
    pub y_labels: [String; 3],
    /// Faixa do eixo % (−20..60, aberta pelo dado).
    pub rate_min: i64,
    pub rate_max: i64,
    pub rate_labels: [String; 2],
    pub target: f32,
    /// "alvo 20%".
    pub target_label: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HistoryColumn {
    /// "out".
    pub label: String,
    /// Antes do primeiro lancamento: coluna vazia, rotulo a 40%.
    pub existed: bool,
    pub partial: bool,
    pub income: f32,
    pub expense: f32,
    /// Taxa de poupanca na escala `rate_min..rate_max` (0..=1); `None` sem receita.
    pub rate: Option<f32>,
    /// "out 2026 · receitas R$ X · despesas R$ Y · sobrou R$ Z (28%)".
    pub tooltip: Option<String>,
}

/// "Sobra média dos meses fechados: R$ 1.410,29 · só outubro".
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AverageLine {
    /// "Sobra média dos meses fechados:" ou "Ainda não há mês fechado".
    pub label: String,
    pub value: Option<String>,
    pub suffix: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WhereItWent {
    pub title: String,
    pub question: String,
    /// "R$ 876 fixos · R$ 2.237 variáveis · 9 categorias".
    pub summary: String,
    pub plan: PlanCard,
    pub attention: Option<Attention>,
    /// "Demais categorias" / "Categorias".
    pub others_title: String,
    pub others_info: String,
    pub others: Vec<CategoryLine>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlanCard {
    pub title: String,
    /// "renda fixa R$ 3.500"; `None` sem renda recorrente.
    pub income_label: Option<String>,
    /// Metas, gasto sem meta e sobra, fracoes da barra dividida.
    pub bar: Option<[f32; 3]>,
    pub legend: Vec<String>,
    pub message: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Attention {
    /// "Metas em atenção · 4 de 5".
    pub title: String,
    /// "faltam 11 dias no mês"; so mes corrente.
    pub days_left: Option<String>,
    pub rows: Vec<AttentionRow>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AttentionRow {
    pub key: String,
    pub name: String,
    pub icon: String,
    pub color: String,
    /// "↑ 38% do normal".
    pub above_normal: Option<String>,
    /// "R$ 515,15".
    pub spent: String,
    /// "de R$ 400".
    pub limit: String,
    /// "128% · passou R$ 115,15" / "98% · restam R$ 5,64".
    pub status: String,
    pub tone: Tone,
    pub fill: f32,
    /// Traco da meta.
    pub marker: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CategoryLine {
    pub key: String,
    pub name: String,
    pub icon: String,
    pub color: String,
    pub above_normal: Option<String>,
    /// "R$ 1.089,34".
    pub amount: String,
    /// "76% da meta" / "6% do gasto".
    pub note: String,
    pub fill: f32,
    /// Traco da meta, so com meta.
    pub marker: Option<f32>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WhatsNext {
    pub title: String,
    pub question: String,
    pub free: FreeCard,
    pub variable: VariableNote,
    pub upcoming: UpcomingCard,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FreeCard {
    /// "Livre em dezembro".
    pub title: String,
    /// "~" antes do valor: so quando entra estimativa.
    pub approx: bool,
    /// `None` sem renda fixa (fica o convite).
    pub amount: Option<BigMoney>,
    pub amount_negative: bool,
    /// "de R$ 3.500 de renda fixa".
    pub of_income: Option<String>,
    pub committed_fill: f32,
    /// Marcas 50% e 70%, so com porcentagem.
    pub marks: Vec<(f32, String)>,
    /// "R$ 876 já comprometido ·" (com o ponto so quando segue a porcentagem).
    pub committed: String,
    /// "25% da renda fixa".
    pub committed_percent: Option<String>,
    pub reference: Option<String>,
    pub invite: Option<String>,
    /// "Inclui a estimativa de contas com valor variável."
    pub estimate_note: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VariableNote {
    /// "E o gasto variável?".
    pub title: String,
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UpcomingCard {
    /// "Próximas saídas da conta".
    pub title: String,
    /// "30 dias".
    pub window: String,
    /// Faturas primeiro.
    pub invoices: Vec<InvoiceLine>,
    /// Lista inteira: "Ver todas" expande no lugar.
    pub items: Vec<UpcomingLine>,
    pub preview: usize,
    /// "Ver todas as 9"; so com mais itens que o preview.
    pub see_all: Option<String>,
    /// "Nada programado nos próximos 30 dias."
    pub empty: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InvoiceLine {
    /// "Fatura Mercado Pago".
    pub title: String,
    pub icon: String,
    /// "vence ter, 8 dez · em 19 dias · fatura aberta".
    pub when: String,
    /// "~R$ 515,15" (aberta) / "R$ 515,15" (fechada).
    pub amount: String,
    /// "15% da renda." — em `warning` acima de 30%.
    pub percent: Option<Line>,
    pub note: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UpcomingLine {
    pub description: String,
    /// Icone da categoria da serie, ou `repeat` no tile neutro.
    pub icon: String,
    /// Token da categoria; `None` = tile neutro.
    pub color: Option<String>,
    /// "sáb, 21 nov · em 2 dias".
    pub when: String,
    /// "R$ 50,52", "+R$ 3.500,00", "~R$ 120,00".
    pub amount: String,
    pub income: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Recent {
    pub title: String,
    /// "20 em novembro".
    pub count: String,
    pub lines: Vec<RecentLine>,
    /// "Nenhum lançamento em novembro".
    pub empty: Option<String>,
    pub info: String,
    /// "Ver todos".
    pub link: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RecentLine {
    pub id: String,
    pub description: String,
    /// "Mensal".
    pub chip: Option<String>,
    /// "Transferência" / "Oculto": a linha aparece, mas nao soma.
    pub tag: Option<String>,
    /// Token do ponto da categoria.
    pub dot: Option<String>,
    /// "Assinaturas · Luiz · 18 nov".
    pub meta: String,
    /// "−R$ 118,40".
    pub amount: String,
    pub income: bool,
    /// Valor em `fg` a 55%: fora da soma.
    pub excluded: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct People {
    /// "Quem lançou · novembro".
    pub title: String,
    pub rows: Vec<PersonLine>,
    pub footnote: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PersonLine {
    pub initial: String,
    pub name: String,
    /// Token do perfil; `None` = avatar neutro.
    pub color: Option<String>,
    /// "despesas R$ 2.640,41 · receitas R$ 3.500,00".
    pub line: String,
}

/// Porcentagem com o menos tipografico.
fn pct(p: i64) -> String {
    if p < 0 {
        format!("{MINUS}{}%", p.unsigned_abs())
    } else {
        format!("{p}%")
    }
}

fn day_of(date: &str) -> usize {
    date.get(8..10).and_then(|d| d.parse().ok()).unwrap_or(0)
}

/// "sáb, 21 nov": `short_date` com o dia da semana minusculo, como o design e o app.
fn short_lower(date: &str) -> String {
    let s = short_date(date);
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().chain(chars).collect(),
        None => s,
    }
}

fn capitalized(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn frac(part: i64, whole: i64) -> f32 {
    if whole <= 0 {
        0.0
    } else {
        (part as f64 / whole as f64).clamp(0.0, 1.0) as f32
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("{n} {one}")
    } else {
        format!("{n} {many}")
    }
}

/// "em 2 dias" / "em 1 dia" / "hoje" (`inDaysText` do app).
fn in_days(today: &str, date: &str) -> String {
    match days_between(today, date) {
        0 => "hoje".to_string(),
        1 => "em 1 dia".to_string(),
        n => format!("em {n} dias"),
    }
}

/// `ds` e a visao de UMA conta (`Dataset::for_book(book)`, feita por quem chama); `book` so
/// decide o bloco Negocio. `today` e `YYYY-MM-DD`, `month` e `YYYY-MM`.
pub fn build_grafite(
    ds: &Dataset,
    today: &str,
    month: &str,
    book: Book,
    hidden: bool,
) -> GrafiteView {
    let m = Money::new(hidden);
    let is_current = month == month_of(today);
    let is_empty = ds.is_empty();
    GrafiteView {
        month: month.to_string(),
        is_current,
        is_empty,
        hidden,
        ignored_total: ds.ignored_total(),
        sidebar: Sidebar {
            transactions: ds.alive_transactions().count(),
            reserves: ds.reserve_count(),
            has_data: !is_empty,
        },
        header: header(ds, today, month, book, hidden),
        business: (book == Book::Business).then(|| business(ds, month, m)),
        how_am_i: how_am_i(ds, today, month, m),
        reserve: reserve_card(ds, today, month, m),
        history: history_card(ds, today, month, m),
        where_it_went: where_it_went(ds, today, month, m),
        whats_next: is_current.then(|| whats_next(ds, today, m)),
        recent: recent_card(ds, month, m),
        people: people(ds, month, m),
    }
}

fn header(ds: &Dataset, today: &str, month: &str, book: Book, hidden: bool) -> Header {
    let is_empty = ds.is_empty();
    let subtitle = if is_empty {
        "Nada sincronizado ainda".to_string()
    } else {
        match ds.business_book().filter(|_| book == Book::Business) {
            Some(b) => format!("{} · somente leitura · dados do último sync", b.name),
            None => "Somente leitura · dados do último sync".to_string(),
        }
    };
    Header {
        title: "Resumo da casa".to_string(),
        subtitle,
        partial: (month == month_of(today) && !is_empty)
            .then(|| format!("Parcial · até {}", partial_label(today))),
        month_label: capitalized(&month_short_year(month)),
        book_switch: ds.has_business_book(),
        hide_toggle: if hidden {
            "Mostrar valores"
        } else {
            "Ocultar valores"
        }
        .to_string(),
    }
}

fn business(ds: &Dataset, month: &str, m: Money) -> BusinessBlock {
    let p = business_panel(ds, month);
    let lv = |label: &str, value: String| LabeledValue {
        label: label.to_string(),
        value,
    };
    BusinessBlock {
        title: "Negócio".to_string(),
        question: "O negócio dá resultado?".to_string(),
        result: lv("Resultado do mês", m.signed(p.result_minor)),
        result_tone: if p.result_minor < 0 {
            Tone::Expense
        } else {
            Tone::Neutral
        },
        income: lv("Receitas", m.brl(p.income_minor)),
        expense: lv("Despesas", m.brl(p.expense_minor)),
        capital_title: "Capital".to_string(),
        capital: vec![
            lv("Investimentos", m.brl(p.investment_minor)),
            lv("Capital em estoque", m.brl(p.inventory_minor)),
            lv("Capital em equipamento", m.brl(p.equipment_minor)),
            lv("Capital total aplicado", m.brl(p.capital_minor)),
        ],
        debt_title: "Dívidas".to_string(),
        debt: lv("Dívidas totais", m.brl(p.debt_minor)),
        today_chip: "hoje".to_string(),
        note: "Bens e dívidas não entram em receita nem em despesa.".to_string(),
    }
}

fn how_am_i(ds: &Dataset, today: &str, month: &str, m: Money) -> HowAmI {
    let is_current = month == month_of(today);
    let t = totals(in_month(ds, month));
    let fc = if is_current {
        forecast(ds, today)
    } else {
        None
    };
    let outlook = match &fc {
        Some(f) => {
            let rounded = round_to_tens(f.expected_surplus_minor);
            Some(match rounded.cmp(&0) {
                std::cmp::Ordering::Less => Line {
                    text: format!(
                        "Vão faltar ~{} no fim do mês, se continuar no ritmo atual",
                        m.whole(rounded)
                    ),
                    tone: Tone::Warning,
                },
                std::cmp::Ordering::Greater => Line {
                    text: format!(
                        "Vão sobrar ~{} no fim do mês, se continuar no ritmo atual",
                        m.whole(rounded)
                    ),
                    tone: Tone::Income,
                },
                std::cmp::Ordering::Equal => Line {
                    text: "Deve fechar perto do zero, se continuar no ritmo atual".to_string(),
                    tone: Tone::Neutral,
                },
            })
        }
        // Mes corrente sem previsao (ate o dia 5): parcial contra mes inteiro enganava, entao
        // nao compara (M3).
        None if is_current => None,
        None => normal_balance(ds, month).map(|normal| {
            let delta = t.balance_minor.saturating_sub(normal);
            match delta.cmp(&0) {
                std::cmp::Ordering::Greater => Line {
                    text: format!("{} a mais que o seu normal", m.brl(delta)),
                    tone: Tone::Income,
                },
                std::cmp::Ordering::Less => Line {
                    text: format!("{} a menos que o seu normal", m.brl(delta)),
                    tone: Tone::Expense,
                },
                std::cmp::Ordering::Equal => Line {
                    text: "Igual ao seu normal".to_string(),
                    tone: Tone::Neutral,
                },
            }
        }),
    };
    let zero_box = fc.as_ref().and_then(|f| {
        zero_allowance(f).map(|v| {
            if v > 0 {
                ZeroBox {
                    title: "Para fechar no zero".to_string(),
                    value: Some(format!("~{} por dia", m.whole(v))),
                    until: Some(format!("até o dia {}", f.days_in_month)),
                    note: None,
                }
            } else {
                ZeroBox {
                    title: "Para fechar no zero".to_string(),
                    value: None,
                    until: None,
                    note: Some("As contas que ainda vêm já passam do saldo do mês".to_string()),
                }
            }
        })
    });
    let casual_average = fc.as_ref().map(|f| {
        format!(
            "Sua média até aqui: ~{} por dia no gasto do dia a dia (avulso)",
            m.whole(f.casual_daily_minor)
        )
    });
    let savings = savings_rate(t.income_minor, t.expense_minor).map(|rate| Savings {
        title: "Taxa de poupança".to_string(),
        target: format!("alvo: {SAVINGS_TARGET_PERCENT}% da renda"),
        rate: format!(
            "{} {}",
            pct(rate),
            if is_current { "até agora" } else { "no mês" }
        ),
        pace: fc
            .as_ref()
            .and_then(|f| f.expected_savings_rate)
            .map(|r| Line {
                text: format!("no ritmo atual, ~{}", pct(r)),
                tone: if r < 0 { Tone::Expense } else { Tone::Neutral },
            }),
        fill: (rate as f64 / SAVINGS_TARGET_PERCENT as f64).clamp(0.0, 1.0) as f32,
        scale_start: "0%".to_string(),
        scale_end: format!("{SAVINGS_TARGET_PERCENT}%"),
    });
    let saved = saved_in_month(ds, month);
    let reserves_line = (saved != 0).then(|| {
        format!(
            "{} {} {}",
            m.compact(saved.saturating_abs()),
            if saved < 0 {
                "saíram das reservas"
            } else {
                "foram para reservas"
            },
            if is_current { "neste mês" } else { "no mês" }
        )
    });
    HowAmI {
        title: "Como estou".to_string(),
        question: "O mês está sob controle?".to_string(),
        balance_label: if is_current {
            format!("Saldo até hoje · dia {}", day_of(today))
        } else {
            "Saldo do mês".to_string()
        },
        balance: m.big(t.balance_minor),
        balance_negative: t.balance_minor < 0,
        outlook,
        zero_box,
        casual_average,
        income: LabeledValue {
            label: "Receitas".to_string(),
            value: m.brl(t.income_minor),
        },
        expense: LabeledValue {
            label: "Despesas".to_string(),
            value: m.brl(t.expense_minor),
        },
        savings,
        reserves_line,
        pace: pace_chart(ds, today, month, m, fc.as_ref(), t.income_minor),
    }
}

/// "por enquanto só outubro tem dados" a partir dos meses de referencia.
fn pace_info(reference: &[String], with_forecast: bool) -> String {
    let names: Vec<&str> = reference.iter().map(|r| month_long(r)).collect();
    let data = match names.as_slice() {
        [] => "; ainda não há mês anterior com dados".to_string(),
        [one] => format!("; por enquanto só {one} tem dados"),
        [a, b] => format!("; por enquanto só {a} e {b} têm dados"),
        _ => String::new(),
    };
    let mut text = format!("“Seu normal” é a média dos 3 meses anteriores no mesmo dia{data}.");
    if with_forecast {
        text.push_str(
            " A previsão soma as recorrentes que ainda vêm e projeta o gasto avulso pela média diária.",
        );
    }
    text
}

fn pace_chart(
    ds: &Dataset,
    today: &str,
    month: &str,
    m: Money,
    fc: Option<&super::insights::Forecast>,
    launched_income: i64,
) -> PaceChart {
    let is_current = month == month_of(today);
    let sp = spending_pace(ds, month, today);
    let days = usize::from(days_in_month(month));
    // Renda do mes: lancada + recorrentes por vir (no corrente); a do mes, num fechado.
    let income = if is_current {
        project_month(ds, month)
            .iter()
            .filter(|o| o.kind == Kind::Income)
            .fold(launched_income, |acc, o| acc.saturating_add(o.amount_minor))
    } else {
        launched_income
    };
    let income = (income > 0).then_some(income);
    let normal_end = sp.normal.last().copied();
    let expected = fc.map(|f| f.expected_expense_minor);
    let top = pace_axis_top(
        &[expected, normal_end, income, Some(sp.spent_minor)]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>(),
    );
    let mut x_labels = vec![(1, "1".to_string()), (10, "10".to_string())];
    if is_current {
        x_labels.retain(|(d, _)| *d != sp.cutoff);
        x_labels.push((sp.cutoff, "hoje".to_string()));
    }
    if !x_labels.iter().any(|(d, _)| *d == days) {
        x_labels.push((days, days.to_string()));
    }
    // "hoje" antes do dia 10 entra depois dele no vetor: a UI desenha da esquerda para a direita.
    x_labels.sort_by_key(|(d, _)| *d);
    let change = sp.change_percent.map(|p| match p.cmp(&0) {
        std::cmp::Ordering::Greater => Line {
            text: format!("{p}% acima"),
            tone: Tone::Warning,
        },
        std::cmp::Ordering::Less => Line {
            text: format!("{}% abaixo", p.unsigned_abs()),
            tone: Tone::Income,
        },
        std::cmp::Ordering::Equal => Line {
            text: "igual".to_string(),
            tone: Tone::Neutral,
        },
    });
    let tail = sp.change_percent.map(|p| {
        if p == 0 {
            "ao seu normal"
        } else {
            "do seu normal"
        }
        .to_string()
    });
    PaceChart {
        title: "Ritmo de gastos".to_string(),
        info: pace_info(&sp.reference_months, fc.is_some()),
        legend_month: month_short(month).to_string(),
        legend_forecast: fc.map(|_| "previsão".to_string()),
        legend_normal: (!sp.normal.is_empty()).then(|| "seu normal".to_string()),
        legend_income: income.map(|_| "renda".to_string()),
        days,
        cutoff: sp.cutoff,
        current: sp
            .current
            .iter()
            .take(sp.cutoff)
            .map(|v| frac(*v, top))
            .collect(),
        normal: sp.normal.iter().map(|v| frac(*v, top)).collect(),
        forecast: expected.map(|e| (frac(sp.spent_minor, top), frac(e, top))),
        income: income.map(|i| frac(i, top)),
        y_labels: [m.axis(top), m.axis(top / 2), m.axis(0)],
        x_labels,
        forecast_label: expected.map(|e| format!("~{}", m.whole(round_to_tens(e)))),
        normal_label: normal_end.map(|n| format!("normal {}", m.whole(n))),
        income_label: income.map(|i| format!("renda {}", m.whole(i))),
        sentence: PaceSentence {
            lead: if is_current {
                format!("{} até o dia {}", m.brl(sp.spent_minor), sp.cutoff)
            } else {
                format!("{} no mês", m.brl(sp.spent_minor))
            },
            change,
            tail,
        },
    }
}

fn reserve_card(ds: &Dataset, today: &str, month: &str, m: Money) -> ReserveCard {
    // A reserva e o saldo de hoje em qualquer mes escolhido (chip "hoje"); so "Neste mês" segue
    // o seletor.
    let current = month_of(today);
    let (emergency, rest) = ordered(ds);
    let emergency = match emergency {
        Some(r) => EmergencyBlock::Present(Box::new(emergency_card(
            &emergency_view(ds, r, &current, m),
            r.recurring_amount_minor.filter(|v| *v > 0),
            m,
        ))),
        None => EmergencyBlock::Missing {
            title: "Reserva de emergência".to_string(),
            message: "Ainda não há reserva de emergência".to_string(),
            hint:
                "Crie no app em Reservas; ela mede quantos meses de custo essencial você aguenta."
                    .to_string(),
        },
    };
    let mm = month_movements(ds, month);
    let none = mm.deposited_minor == 0 && mm.withdrawn_minor == 0;
    let month_block = ReserveMonth {
        title: if month == current {
            "Neste mês".to_string()
        } else {
            format!("Em {}", month_long(month))
        },
        deposited: (mm.deposited_minor != 0).then(|| LabeledValue {
            label: "Guardado".to_string(),
            value: m.signed_plus(mm.deposited_minor),
        }),
        withdrawn: (mm.withdrawn_minor != 0).then(|| LabeledValue {
            label: "Retirado".to_string(),
            value: m.signed(mm.withdrawn_minor.saturating_neg()),
        }),
        empty: none.then(|| "Nenhum movimento".to_string()),
    };
    let rows: Vec<_> = rest
        .iter()
        .map(|r| {
            let derived = (r.kind == super::contract::ReserveKind::Emergency)
                .then(|| super::reserves::emergency_goal(ds, r, &current));
            pot_row(ds, r, &current, derived, m)
        })
        .collect();
    let acting: Vec<_> = rows
        .iter()
        .filter(|p| {
            matches!(
                p.pace,
                Pace::NoRecurring { .. } | Pace::Behind { .. } | Pace::Overdue { .. }
            )
        })
        .take(POT_LINES)
        .collect();
    let chosen: Vec<_> = if acting.is_empty() {
        rows.iter()
            .filter(|p| p.goal_minor.is_some())
            .take(1)
            .collect()
    } else {
        acting
    };
    ReserveCard {
        emergency,
        month: month_block,
        pots: chosen.into_iter().map(|p| pot_line(p, m)).collect(),
        link: "Ver reservas".to_string(),
    }
}

fn emergency_card(
    v: &super::reserves::EmergencyView,
    recurring: Option<i64>,
    m: Money,
) -> EmergencyCard {
    let multiple = usize::from(v.multiple);
    let reached = v.goal_minor > 0 && v.balance_minor >= v.goal_minor;
    let informed = v.cost.as_ref().is_some_and(|c| c.informed);
    EmergencyCard {
        title: "Reserva de emergência".to_string(),
        chip: "hoje".to_string(),
        coverage: v.coverage_label.clone(),
        coverage_suffix: if multiple == 1 {
            "de 1 mês coberto".to_string()
        } else {
            format!("de {multiple} meses cobertos")
        },
        segments: v.segments.clone(),
        segment_labels: (1..=multiple).map(|i| i.to_string()).collect(),
        milestone: next_milestone(v).map(|ms| MilestoneLine {
            label: "Próximo marco:".to_string(),
            target: format!(
                "{} ({})",
                plural(usize::from(ms.months), "mês", "meses"),
                m.whole(ms.target_minor)
            ),
            missing: format!("· faltam {}", m.whole(ms.missing_minor)),
        }),
        reached: reached.then(|| "Meta alcançada".to_string()),
        goal_line: if v.goal_minor > 0 {
            format!(
                "Meta final: {} de {}, {}.",
                plural(multiple, "mês", "meses"),
                if informed {
                    "custo informado no app"
                } else {
                    "custo essencial"
                },
                m.whole(v.goal_minor)
            )
        } else {
            "Sem custo essencial para calcular a meta.".to_string()
        },
        eta_line: match (&v.eta, recurring) {
            (EmergencyEta::Projected { month }, Some(rec)) => Some(format!(
                "Se guardar {} por mês, a meta fecha em ~{}.",
                m.compact(rec),
                month_short_year(month)
            )),
            (EmergencyEta::NoRecurring, _) => Some("Sem depósito mensal programado.".to_string()),
            _ => None,
        },
    }
}

fn pot_line(p: &super::reserves::PotRow, m: Money) -> PotLine {
    let mut meta = match p.goal_minor {
        Some(g) => format!("{} de {}", m.whole(p.balance_minor), m.whole(g)),
        None => m.whole(p.balance_minor),
    };
    match &p.due_month {
        Some(d) => meta.push_str(&format!(" · até {}", month_short_year(d))),
        None if p.goal_minor.is_some() => meta.push_str(" · sem prazo"),
        None => {}
    }
    let warn = |text: String| {
        Some(Line {
            text,
            tone: Tone::Warning,
        })
    };
    let pace = match &p.pace {
        Pace::NoRecurring { needed } => {
            meta.push_str(" · sem depósito mensal");
            warn(format!("precisa de ~{}/mês", m.whole(*needed)))
        }
        Pace::Behind { recurring, needed } => {
            meta.push_str(&format!(" · guarda {}/mês", m.compact(*recurring)));
            warn(format!("precisa de ~{}/mês", m.whole(*needed)))
        }
        Pace::Overdue { remaining } => {
            meta.push_str(" · prazo encerrado");
            warn(format!("faltam {}", m.whole(*remaining)))
        }
        Pace::OnTrack { recurring, .. } => {
            meta.push_str(&format!(" · guarda {}/mês", m.compact(*recurring)));
            Some(Line {
                text: "No ritmo".to_string(),
                tone: Tone::Income,
            })
        }
        Pace::NoDeadline { recurring: Some(r) } => {
            meta.push_str(&format!(" · guarda {}/mês", m.compact(*r)));
            None
        }
        Pace::Reached => Some(Line {
            text: "Meta alcançada".to_string(),
            tone: Tone::Income,
        }),
        Pace::NoGoal | Pace::NoDeadline { recurring: None } => None,
    };
    PotLine {
        name: p.name.clone(),
        icon: p.icon.clone(),
        color: p.color.clone(),
        meta,
        pace,
    }
}

fn history_card(ds: &Dataset, today: &str, month: &str, m: Money) -> HistoryCard {
    let h = history(ds, month, today);
    let existed: Vec<_> = h.rows.iter().filter(|r| r.existed).collect();
    let peak = existed
        .iter()
        .map(|r| r.income_minor.max(r.expense_minor))
        .max()
        .unwrap_or(0);
    let top = axis_top(peak);
    let rate_text = |rate: Option<i64>| rate.map_or_else(String::new, |r| format!(" · {}", pct(r)));
    let body = if h.is_chart() {
        let rates: Vec<i64> = h.rows.iter().filter_map(|r| r.savings_rate).collect();
        let lo = rates.iter().copied().min().unwrap_or(0);
        let hi = rates.iter().copied().max().unwrap_or(0);
        // Faixa -20%..60% aberta pelo dado, em passos de 20.
        let rate_min = (-20i64).min(lo.div_euclid(20).saturating_mul(20));
        let rate_max = 60i64.max((hi.saturating_add(19)).div_euclid(20).saturating_mul(20));
        let span = (rate_max - rate_min).max(1) as f64;
        let on_rate = |r: i64| ((r - rate_min) as f64 / span).clamp(0.0, 1.0) as f32;
        HistoryBody::Chart(HistoryChart {
            columns: h
                .rows
                .iter()
                .map(|r| HistoryColumn {
                    label: month_short(&r.month).to_string(),
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
                    rate: r.savings_rate.filter(|_| r.existed).map(on_rate),
                    tooltip: r.existed.then(|| {
                        let rate = r
                            .savings_rate
                            .map_or_else(String::new, |p| format!(" ({})", pct(p)));
                        format!(
                            "{} · receitas {} · despesas {} · {} {}{rate}",
                            month_short_year(&r.month),
                            m.brl(r.income_minor),
                            m.brl(r.expense_minor),
                            if r.surplus_minor < 0 {
                                "faltou"
                            } else {
                                "sobrou"
                            },
                            m.brl(r.surplus_minor),
                        )
                    }),
                })
                .collect(),
            y_labels: [m.axis(top), m.axis(top / 2), m.axis(0)],
            rate_labels: [pct(rate_min), pct(rate_max)],
            rate_min,
            rate_max,
            target: on_rate(SAVINGS_TARGET_PERCENT),
            target_label: format!("alvo {SAVINGS_TARGET_PERCENT}%"),
        })
    } else {
        HistoryBody::List {
            lines: existed
                .iter()
                .map(|r| HistoryLine {
                    label: if r.partial {
                        format!(
                            "{} · parcial, até dia {}",
                            month_short(&r.month),
                            day_of(today)
                        )
                    } else {
                        format!("{} · fechado", month_short(&r.month))
                    },
                    text: if r.partial {
                        format!(
                            "{} até agora{}",
                            m.signed_whole(r.surplus_minor),
                            rate_text(r.savings_rate)
                        )
                    } else {
                        format!(
                            "{} {}{}",
                            if r.surplus_minor < 0 {
                                "faltou"
                            } else {
                                "sobrou"
                            },
                            m.whole(r.surplus_minor),
                            rate_text(r.savings_rate)
                        )
                    },
                    income: frac(r.income_minor, top),
                    expense: frac(r.expense_minor, top),
                    partial: r.partial,
                })
                .collect(),
            footnote: "O gráfico de 12 meses aparece a partir do 4º mês de histórico.".to_string(),
        }
    };
    let average = match h.average {
        Some(avg) => {
            // Um mes so na media: e o primeiro (com mais historia a janela teria mais de um),
            // para "· só outubro".
            let only = super::insights::first_month(ds).filter(|_| avg.months == 1);
            AverageLine {
                label: "Sobra média dos meses fechados:".to_string(),
                value: Some(m.signed(avg.average_minor)),
                suffix: Some(match only {
                    Some(mo) => format!("· só {}", month_long(&mo)),
                    None => format!("· {}", plural(avg.months, "mês", "meses")),
                }),
            }
        }
        None => AverageLine {
            label: "Ainda não há mês fechado".to_string(),
            value: None,
            suffix: None,
        },
    };
    HistoryCard {
        title: "Histórico".to_string(),
        legend_income: "Receitas".to_string(),
        legend_expense: "Despesas".to_string(),
        body,
        average,
    }
}

fn where_it_went(ds: &Dataset, today: &str, month: &str, m: Money) -> WhereItWent {
    let is_current = month == month_of(today);
    let slices = expense_by_category(ds, month);
    let fv = fixed_variable(ds, month);
    let above = categories_above_normal(ds, month, today);
    let above_of = |key: &str| {
        above
            .get(key)
            .map(|c| format!("↑ {}% do normal", c.change_percent))
    };
    let budgets = budget_report(ds, month);
    let attention_rows: Vec<&BudgetStatus> = budgets
        .iter()
        .filter(|b| b.level != BudgetLevel::Ok)
        .collect();
    let attention = (!attention_rows.is_empty()).then(|| {
        let left = usize::from(days_in_month(month)).saturating_sub(day_of(today));
        Attention {
            title: format!(
                "Metas em atenção · {} de {}",
                attention_rows.len(),
                budgets.len()
            ),
            days_left: is_current.then(|| match left {
                0 => "último dia do mês".to_string(),
                1 => "falta 1 dia no mês".to_string(),
                n => format!("faltam {n} dias no mês"),
            }),
            rows: attention_rows
                .iter()
                .map(|b| {
                    let used = (b.used_ratio * 100.0).floor() as i64;
                    let over = b.level == BudgetLevel::Over;
                    let scale = b.limit_minor as f64 * ATTENTION_SCALE;
                    AttentionRow {
                        key: b.category_id.clone(),
                        name: b.name.clone(),
                        icon: b.icon.clone(),
                        color: b.color.clone(),
                        above_normal: above_of(&b.category_id),
                        spent: m.brl(b.spent_minor),
                        limit: format!("de {}", m.compact(b.limit_minor)),
                        status: if over {
                            format!("{} · passou {}", pct(used), m.brl(b.over_minor))
                        } else {
                            format!("{} · restam {}", pct(used), m.brl(b.remaining_minor))
                        },
                        tone: if over { Tone::Expense } else { Tone::Warning },
                        fill: if scale > 0.0 {
                            (b.spent_minor as f64 / scale).clamp(0.0, 1.0) as f32
                        } else {
                            1.0
                        },
                        marker: (1.0 / ATTENTION_SCALE) as f32,
                    }
                })
                .collect(),
        }
    });
    let in_attention: Vec<&str> = attention_rows
        .iter()
        .map(|b| b.category_id.as_str())
        .collect();
    let budget_of = |key: &str| budgets.iter().find(|b| b.category_id == key);
    let others: Vec<_> = slices
        .iter()
        .filter(|s| !in_attention.contains(&s.key.as_str()))
        .collect();
    let peak = others
        .iter()
        .map(|s| {
            s.amount_minor
                .max(budget_of(&s.key).map_or(0, |b| b.limit_minor))
        })
        .max()
        .unwrap_or(0)
        .max(1);
    let scale = peak
        .saturating_add(OTHERS_STEP - 1)
        .div_euclid(OTHERS_STEP)
        .saturating_mul(OTHERS_STEP);
    WhereItWent {
        title: "Para onde foi".to_string(),
        question: "Onde dá para cortar ou ajustar a meta?".to_string(),
        summary: format!(
            "{} fixos · {} variáveis · {}",
            m.whole(fv.fixed_minor),
            m.whole(fv.variable_minor),
            plural(slices.len(), "categoria", "categorias")
        ),
        plan: plan_card(ds, month, m),
        others_title: if attention.is_some() {
            "Demais categorias"
        } else {
            "Categorias"
        }
        .to_string(),
        attention,
        others_info: "Em “Metas em atenção”, a barra mostra quanto da meta já foi e o traço pontilhado é a meta. Nas demais, a barra mostra o valor gasto. A barra só ganha cor quando a meta entra em atenção (80%) ou estoura.".to_string(),
        others: others
            .iter()
            .map(|s| {
                let budget = budget_of(&s.key);
                CategoryLine {
                    key: s.key.clone(),
                    name: s.name.clone(),
                    icon: s.icon.clone(),
                    color: s.color.clone(),
                    above_normal: above_of(&s.key),
                    amount: m.brl(s.amount_minor),
                    note: match budget {
                        Some(b) => format!("{} da meta", pct((b.used_ratio * 100.0).floor() as i64)),
                        None => format!("{} do gasto", pct(js_round(s.share * 100.0) as i64)),
                    },
                    fill: frac(s.amount_minor, scale),
                    marker: budget.map(|b| frac(b.limit_minor, scale)),
                }
            })
            .collect(),
    }
}

fn plan_card(ds: &Dataset, month: &str, m: Money) -> PlanCard {
    let p = plan_fit(ds, month);
    let title = "O plano cabe na renda?".to_string();
    let unbudgeted_label = |percent: Option<i64>| {
        let base = format!("Gasto sem meta {}", m.whole(p.unbudgeted_minor));
        match percent {
            Some(v) => format!("{base} · {}", pct(v)),
            None => base,
        }
    };
    if p.fixed_income_minor <= 0 {
        return PlanCard {
            title,
            income_label: None,
            bar: None,
            legend: vec![unbudgeted_label(None)],
            message: "Cadastre o salário como recorrente no app para ver quanto fica livre"
                .to_string(),
        };
    }
    let used = p.budgets_minor.saturating_add(p.unbudgeted_minor);
    let whole = p.fixed_income_minor.max(used);
    let mut legend = vec![
        format!(
            "Metas ~{}{}",
            m.whole(p.budgets_minor),
            p.budgets_percent
                .map_or_else(String::new, |v| format!(" · {}", pct(v)))
        ),
        unbudgeted_label(p.unbudgeted_percent),
    ];
    if p.leftover_minor > 0 {
        legend.push(format!("Sobra para guardar ~{}", m.whole(p.leftover_minor)));
    }
    let ask = p.reserves_ask_minor;
    let left = p.leftover_minor;
    let many = p.reserves_asking > 1;
    let message = if left <= 0 {
        format!(
            "Metas e gasto sem meta passam da renda fixa em {}. Ajuste as metas no app.",
            m.whole(left)
        )
    } else if ask <= 0 {
        format!("Sobram ~{} por mês para guardar.", m.whole(left))
    } else if left >= ask {
        format!(
            "O plano deixa ~{}; cabe o depósito das reservas ({} por mês).",
            m.whole(left),
            m.compact(ask)
        )
    } else {
        format!(
            "{} {} por mês e o plano deixa ~{}. Para {}, ajuste as metas no app.",
            if many {
                "As reservas pedem"
            } else {
                "A reserva pede"
            },
            m.compact(ask),
            m.whole(left),
            if many { "elas caberem" } else { "ela caber" }
        )
    };
    PlanCard {
        title,
        income_label: Some(format!("renda fixa {}", m.whole(p.fixed_income_minor))),
        bar: Some([
            frac(p.budgets_minor, whole),
            frac(p.unbudgeted_minor, whole),
            frac(left.max(0), whole),
        ]),
        legend,
        message,
    }
}

fn whats_next(ds: &Dataset, today: &str, m: Money) -> WhatsNext {
    let free = next_month_free(ds, today);
    let percent = committed_percent(&free);
    let has_income = free.income_minor > 0;
    let next_name = month_long(&free.month);
    let approx = if free.estimated { "~" } else { "" };
    let this_month = month_of(today);
    let variable = fixed_variable(ds, &this_month).variable_minor;
    let day = day_of(today);
    let mut variable_text = format!(
        "Em {} ele já levou {} em {}.",
        month_long(&this_month),
        m.whole(variable),
        plural(day, "dia", "dias")
    );
    if has_income {
        if free.free_minor > 0 {
            variable_text.push_str(&format!(
                " Em {next_name} precisa caber em {approx}{}.",
                m.whole(free.free_minor)
            ));
        } else {
            variable_text.push_str(&format!(
                " Em {next_name} as contas fixas já levam toda a renda fixa."
            ));
        }
    }
    WhatsNext {
        title: "O que vem".to_string(),
        question: "O que já está comprometido?".to_string(),
        free: FreeCard {
            title: format!("Livre em {next_name}"),
            approx: free.estimated && has_income,
            amount: has_income.then(|| m.big_whole(free.free_minor)),
            amount_negative: has_income && free.free_minor < 0,
            of_income: has_income
                .then(|| format!("de {} de renda fixa", m.whole(free.income_minor))),
            committed_fill: frac(free.expense_minor, free.income_minor),
            marks: if percent.is_some() {
                COMMITTED_MARKS
                    .iter()
                    .map(|v| (*v, pct((f64::from(*v) * 100.0).round() as i64)))
                    .collect()
            } else {
                Vec::new()
            },
            committed: format!(
                "{} já comprometido{}",
                m.whole(free.expense_minor),
                if percent.is_some() { " ·" } else { "" }
            ),
            committed_percent: percent.map(|p| format!("{} da renda fixa", pct(p))),
            reference: percent.map(|_| {
                "Referência: até 50% há folga; acima de 70% qualquer imprevisto aperta.".to_string()
            }),
            invite: (!has_income).then(|| {
                "Cadastre o salário como recorrente no app para ver quanto fica livre".to_string()
            }),
            estimate_note: free
                .estimated
                .then(|| "Inclui a estimativa de contas com valor variável.".to_string()),
        },
        variable: VariableNote {
            title: "E o gasto variável?".to_string(),
            text: variable_text,
        },
        upcoming: upcoming_card(ds, today, m),
    }
}

fn upcoming_card(ds: &Dataset, today: &str, m: Money) -> UpcomingCard {
    let invoices: Vec<InvoiceLine> = upcoming_invoices(ds, today, UPCOMING_DAYS)
        .into_iter()
        .map(|inv| {
            let open = inv.status == InvoiceStatus::Open;
            InvoiceLine {
                title: format!("Fatura {}", inv.card_name),
                icon: inv.icon.clone(),
                when: format!(
                    "vence {} · {} · fatura {}",
                    short_lower(&inv.due_on),
                    in_days(today, &inv.due_on),
                    if open { "aberta" } else { "fechada" }
                ),
                amount: format!("{}{}", if open { "~" } else { "" }, m.brl(inv.total_minor)),
                percent: inv.income_percent.map(|p| Line {
                    text: format!("{} da renda.", pct(p)),
                    tone: if p > INVOICE_WARNING_PERCENT {
                        Tone::Warning
                    } else {
                        Tone::Neutral
                    },
                }),
                note:
                    "Já está nas despesas: é o dinheiro que precisa estar na conta no vencimento."
                        .to_string(),
            }
        })
        .collect();
    let items: Vec<UpcomingLine> = upcoming(ds, today, UPCOMING_DAYS)
        .into_iter()
        .map(|o| {
            let category = ds.find_category(o.category_id.as_deref());
            let income = o.kind == Kind::Income;
            let approx = if o.estimated { "~" } else { "" };
            UpcomingLine {
                description: o
                    .description
                    .clone()
                    .unwrap_or_else(|| "Recorrência".to_string()),
                icon: category.map_or_else(|| "repeat".to_string(), |c| c.icon.clone()),
                color: category.map(|c| c.color.clone()),
                when: format!("{} · {}", short_lower(&o.date), in_days(today, &o.date)),
                amount: if income {
                    format!("{approx}{}", m.signed_plus(o.amount_minor))
                } else {
                    format!("{approx}{}", m.brl(o.amount_minor))
                },
                income,
            }
        })
        .collect();
    let total = invoices.len() + items.len();
    UpcomingCard {
        title: "Próximas saídas da conta".to_string(),
        window: format!("{UPCOMING_DAYS} dias"),
        preview: UPCOMING_PREVIEW,
        see_all: (total > UPCOMING_PREVIEW).then(|| format!("Ver todas as {total}")),
        empty: (total == 0).then(|| format!("Nada programado nos próximos {UPCOMING_DAYS} dias.")),
        invoices,
        items,
    }
}

fn recent_card(ds: &Dataset, month: &str, m: Money) -> Recent {
    let count = list_month(ds, month, &Filters::default(), m).count;
    let name = month_long(month);
    Recent {
        title: "Últimos lançamentos".to_string(),
        count: format!("{count} em {name}"),
        lines: recent(ds, month, RECENT_LINES)
            .into_iter()
            .map(|tx| {
                let row = to_row(ds, tx, m);
                let income = row.kind == Kind::Income;
                let mut meta = vec![row.category_name.clone()];
                if let Some(a) = &row.author {
                    meta.push(a.name.clone());
                }
                meta.push(partial_label(&row.occurred_on));
                RecentLine {
                    amount: if income {
                        m.signed_plus(row.amount_minor)
                    } else {
                        m.signed(row.amount_minor.saturating_neg())
                    },
                    tag: row.report_state.map(|s| s.label().to_string()),
                    excluded: row.report_state.is_some(),
                    dot: row
                        .category_color
                        .clone()
                        .or_else(|| Some(NO_CATEGORY_COLOR.to_string())),
                    meta: meta.join(" · "),
                    chip: row.recurrence_label.clone(),
                    description: row.description,
                    id: row.id,
                    income,
                }
            })
            .collect(),
        empty: (count == 0).then(|| format!("Nenhum lançamento em {name}")),
        info: "Mesma regra dos totais: transferências aparecem marcadas e ficam fora da soma."
            .to_string(),
        link: "Ver todos".to_string(),
    }
}

fn people(ds: &Dataset, month: &str, m: Money) -> Option<People> {
    let groups = by_person(ds, month);
    if groups.is_empty() {
        return None;
    }
    Some(People {
        title: format!("Quem lançou · {}", month_long(month)),
        rows: groups
            .iter()
            .map(|g| {
                let name = g.author.label().to_string();
                let mut line = format!("despesas {}", m.brl(g.expense_minor));
                if g.income_minor > 0 {
                    line.push_str(&format!(" · receitas {}", m.brl(g.income_minor)));
                }
                PersonLine {
                    initial: name
                        .chars()
                        .next()
                        .map_or_else(String::new, |c| c.to_uppercase().collect()),
                    color: match &g.author {
                        Author::Person { color, .. } => Some(color.clone()),
                        _ => None,
                    },
                    name,
                    line,
                }
            })
            .collect(),
        footnote: "Para combinar a divisão da casa.".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::dashboard::dataset::{BOOKS, PAYMENT_METHODS, RESERVES, TRANSACTIONS};
    use crate::dashboard::money::leaks;
    use crate::dashboard::{
        GRAFITE_TODAY, GrafiteRow, dataset_from, dataset_of, fixtures_grafite, grafite_rows,
    };

    fn nov() -> GrafiteView {
        build_grafite(
            &fixtures_grafite(),
            GRAFITE_TODAY,
            "2026-11",
            Book::Personal,
            false,
        )
    }

    fn edited(drop: impl Fn(&str) -> bool, add: Vec<GrafiteRow>) -> Dataset {
        let mut rows: Vec<GrafiteRow> = grafite_rows()
            .into_iter()
            .filter(|(_, id, _)| !drop(id))
            .collect();
        rows.extend(add);
        dataset_from(rows)
    }

    fn tx(id: &str, kind: &str, amount: i64, on: &str, extra: Value) -> GrafiteRow {
        let mut data = json!({
            "kind": kind, "description": "Extra", "amountMinor": amount, "occurredOn": on,
        });
        if let (Some(b), Some(o)) = (data.as_object_mut(), extra.as_object()) {
            for (k, v) in o {
                b.insert(k.clone(), v.clone());
            }
        }
        (TRANSACTIONS, id.to_string(), data)
    }

    fn business_book() -> GrafiteRow {
        (
            BOOKS,
            "BK".to_string(),
            json!({ "kind": "business", "name": "Padaria", "color": "amber", "icon": "s" }),
        )
    }

    fn close(a: f32, b: f64) -> bool {
        (f64::from(a) - b).abs() < 0.001
    }

    /// A fixture com a linha `id` trocada por `patch` (campos de cima sobrescritos).
    fn patched(id: &str, patch: Value) -> Dataset {
        let rows = grafite_rows()
            .into_iter()
            .map(|(table, rid, mut data)| {
                if rid == id
                    && let (Some(b), Some(o)) = (data.as_object_mut(), patch.as_object())
                {
                    for (k, v) in o {
                        b.insert(k.clone(), v.clone());
                    }
                }
                (table, rid, data)
            })
            .collect();
        dataset_from(rows)
    }

    fn view_of(ds: &Dataset) -> GrafiteView {
        build_grafite(ds, GRAFITE_TODAY, "2026-11", Book::Personal, false)
    }

    #[test]
    fn eixo_x_fica_em_ordem_antes_do_dia_10() {
        let v = build_grafite(
            &fixtures_grafite(),
            "2026-11-05",
            "2026-11",
            Book::Personal,
            false,
        );
        let x: Vec<(usize, &str)> = v
            .how_am_i
            .pace
            .x_labels
            .iter()
            .map(|(d, l)| (*d, l.as_str()))
            .collect();
        assert_eq!(x, vec![(1, "1"), (5, "hoje"), (10, "10"), (30, "30")]);
    }

    #[test]
    fn caixinhas_no_maximo_duas_que_pedem_acao_senao_a_primeira_com_meta() {
        let goal = |name: &str| {
            json!({ "kind": "goal", "name": name, "icon": "gift", "color": "sky",
                    "targetMinor": 100_000, "deadline": "2027-11", "recurring": null })
        };
        let ds = edited(
            |_| false,
            vec![
                (RESERVES, "RD".to_string(), goal("Viagem")),
                (RESERVES, "RF".to_string(), goal("Carro")),
            ],
        );
        let names: Vec<String> = view_of(&ds)
            .reserve
            .pots
            .into_iter()
            .map(|p| p.name)
            .collect();
        assert_eq!(names, ["Casamento", "Viagem"]);
        // Casamento no ritmo: nenhuma pede acao, fica a primeira com meta, sem atencao.
        let on_track = patched(
            "RC",
            json!({ "recurring": { "amountMinor": 20_000, "day": 5, "since": "2026-11" } }),
        );
        let pots = view_of(&on_track).reserve.pots;
        assert_eq!(pots.len(), 1);
        assert_eq!(
            pots[0].meta,
            "R$ 150 de R$ 1.500 · até nov 2027 · guarda R$ 200/mês"
        );
        assert_eq!(
            pots[0].pace,
            Some(Line {
                text: "No ritmo".into(),
                tone: Tone::Income
            })
        );
    }

    #[test]
    fn textos_do_plano() {
        // Pedido menor que a sobra: cabe.
        let fits = patched(
            "RE",
            json!({ "recurring": { "amountMinor": 4000, "day": 5, "since": "2026-11" } }),
        );
        assert_eq!(
            view_of(&fits).where_it_went.plan.message,
            "O plano deixa ~R$ 48; cabe o depósito das reservas (R$ 40 por mês)."
        );
        // Gasto sem meta empurra o plano para alem da renda fixa.
        let over = edited(
            |_| false,
            vec![tx(
                "1115-9-EXTRA",
                "expense",
                100_000,
                "2026-11-15",
                json!({}),
            )],
        );
        let plan = view_of(&over).where_it_went.plan;
        assert_eq!(
            plan.message,
            "Metas e gasto sem meta passam da renda fixa em R$ 952. Ajuste as metas no app."
        );
        assert_eq!(plan.legend.len(), 2);
        let [a, b, c] = plan.bar.expect("barra");
        assert!(close(a + b, 1.0) && c == 0.0);
        // Duas reservas pedindo: plural.
        let both = patched(
            "RC",
            json!({ "recurring": { "amountMinor": 1000, "day": 5, "since": "2026-11" } }),
        );
        assert_eq!(
            view_of(&both).where_it_went.plan.message,
            "As reservas pedem R$ 560 por mês e o plano deixa ~R$ 48. Para elas caberem, ajuste \
             as metas no app."
        );
    }

    #[test]
    fn reserva_com_meta_alcancada_e_sem_custo() {
        let done = patched("RE", json!({ "essentialOverrideMinor": 5000 }));
        let EmergencyBlock::Present(e) = view_of(&done).reserve.emergency else {
            panic!("emergencia");
        };
        assert_eq!(e.reached.as_deref(), Some("Meta alcançada"));
        assert_eq!((e.milestone.as_ref(), e.eta_line.as_ref()), (None, None));
        assert_eq!(e.coverage, "7,0");
        let no_cost = patched("RE", json!({ "essentialOverrideMinor": 0 }));
        let EmergencyBlock::Present(e) = view_of(&no_cost).reserve.emergency else {
            panic!("emergencia");
        };
        assert_eq!(e.coverage, "—");
        assert!(e.segments.iter().all(|s| *s == 0.0));
        assert_eq!(e.goal_line, "Sem custo essencial para calcular a meta.");
        assert_eq!((e.milestone.as_ref(), e.reached.as_ref()), (None, None));
    }

    #[test]
    fn fatura_fechada_sem_til_e_acima_de_30_por_cento_em_atencao() {
        let ds = edited(
            |_| false,
            vec![
                (
                    PAYMENT_METHODS,
                    "NU".to_string(),
                    json!({ "name": "Nubank", "icon": "credit-card", "kind": "credit",
                            "bank": "Nubank", "closingDay": 15, "dueDay": 25 }),
                ),
                tx(
                    "1103-9-TV",
                    "expense",
                    150_000,
                    "2026-11-03",
                    json!({ "paymentMethodId": "NU" }),
                ),
            ],
        );
        let n = view_of(&ds).whats_next.expect("mes corrente");
        let first = &n.upcoming.invoices[0];
        assert_eq!(first.title, "Fatura Nubank");
        assert_eq!(first.when, "vence qua, 25 nov · em 6 dias · fatura fechada");
        assert_eq!(first.amount, "R$ 1.500,00");
        assert_eq!(
            first.percent,
            Some(Line {
                text: "43% da renda.".into(),
                tone: Tone::Warning
            })
        );
        assert_eq!(n.upcoming.invoices.len(), 2);
        assert_eq!(n.upcoming.see_all.as_deref(), Some("Ver todas as 10"));
    }

    #[test]
    fn detector_de_vazamento_casa_a_regex_do_app() {
        assert!(leaks("R$ 386,59").is_some());
        assert!(leaks("1.410,29").is_some());
        assert!(leaks("R$5 ").is_some());
        assert!(leaks("R$ •••••").is_none());
        assert!(leaks("0,1 de 6 meses; 11% até agora; nov · parcial, até dia 19").is_none());
    }

    #[test]
    fn cabecalho_e_lateral() {
        let v = nov();
        assert_eq!(v.header.title, "Resumo da casa");
        assert_eq!(v.header.subtitle, "Somente leitura · dados do último sync");
        assert_eq!(v.header.partial.as_deref(), Some("Parcial · até 19 nov"));
        assert_eq!(v.header.month_label, "Nov 2026");
        assert!(!v.header.book_switch);
        assert_eq!(v.header.hide_toggle, "Ocultar valores");
        // 7 ocorrencias de novembro + Cadeira de outubro + 17 avulsos.
        assert_eq!(
            v.sidebar,
            Sidebar {
                transactions: 25,
                reserves: 2,
                has_data: true
            }
        );
        assert_eq!(
            v.sections(),
            vec![
                Section::HowAmI,
                Section::Reserve,
                Section::History,
                Section::WhereItWent,
                Section::WhatsNext,
                Section::Recent,
                Section::People,
            ]
        );
    }

    #[test]
    fn como_estou_da_fixture() {
        let c = nov().how_am_i;
        assert_eq!(c.title, "Como estou");
        assert_eq!(c.question, "O mês está sob controle?");
        assert_eq!(c.balance_label, "Saldo até hoje · dia 19");
        let b = &c.balance;
        assert_eq!(
            (b.sign, b.currency, b.whole.as_str(), b.cents.as_str()),
            ("", "R$", "386", ",59")
        );
        assert!(!c.balance_negative);
        assert_eq!(
            c.outlook,
            Some(Line {
                text: "Vão faltar ~R$ 960 no fim do mês, se continuar no ritmo atual".into(),
                tone: Tone::Warning
            })
        );
        let zero = c.zero_box.clone().expect("caixa");
        assert_eq!(zero.title, "Para fechar no zero");
        assert_eq!(zero.value.as_deref(), Some("~R$ 30 por dia"));
        assert_eq!(zero.until.as_deref(), Some("até o dia 30"));
        assert_eq!(zero.note, None);
        assert_eq!(
            c.casual_average.as_deref(),
            Some("Sua média até aqui: ~R$ 118 por dia no gasto do dia a dia (avulso)")
        );
        assert_eq!(
            (c.income.label.as_str(), c.income.value.as_str()),
            ("Receitas", "R$ 3.500,00")
        );
        assert_eq!(
            (c.expense.label.as_str(), c.expense.value.as_str()),
            ("Despesas", "R$ 3.113,41")
        );
        let s = c.savings.clone().expect("taxa");
        assert_eq!(s.title, "Taxa de poupança");
        assert_eq!(s.target, "alvo: 20% da renda");
        assert_eq!(s.rate, "11% até agora");
        assert_eq!(
            s.pace,
            Some(Line {
                text: "no ritmo atual, ~\u{2212}27%".into(),
                tone: Tone::Expense,
            })
        );
        assert!(close(s.fill, 0.55));
        assert_eq!(
            (s.scale_start.as_str(), s.scale_end.as_str()),
            ("0%", "20%")
        );
        assert_eq!(
            c.reserves_line.as_deref(),
            Some("R$ 200 saíram das reservas neste mês")
        );
    }

    #[test]
    fn ritmo_de_gastos_da_fixture() {
        let p = nov().how_am_i.pace;
        assert_eq!(p.title, "Ritmo de gastos");
        assert_eq!(
            p.info,
            "“Seu normal” é a média dos 3 meses anteriores no mesmo dia; por enquanto só outubro \
             tem dados. A previsão soma as recorrentes que ainda vêm e projeta o gasto avulso pela \
             média diária."
        );
        assert_eq!(p.legend_month, "nov");
        assert_eq!(p.legend_forecast.as_deref(), Some("previsão"));
        assert_eq!(p.legend_normal.as_deref(), Some("seu normal"));
        assert_eq!(p.legend_income.as_deref(), Some("renda"));
        assert_eq!((p.days, p.cutoff), (30, 19));
        assert_eq!((p.current.len(), p.normal.len()), (19, 30));
        // Teto R$ 5 mil: acumulado 311341, normal no dia 19 = 270500, renda 350000.
        assert!(p.current.last().is_some_and(|v| close(*v, 0.622_682)));
        assert!(p.normal.get(18).is_some_and(|v| close(*v, 0.541)));
        let (from, to) = p.forecast.expect("previsao");
        assert!(close(from, 0.622_682) && close(to, 0.891_854));
        assert!(p.income.is_some_and(|v| close(v, 0.7)));
        assert_eq!(p.y_labels, ["R$ 5 mil", "R$ 2,5 mil", "R$ 0"]);
        let x: Vec<(usize, &str)> = p.x_labels.iter().map(|(d, l)| (*d, l.as_str())).collect();
        assert_eq!(x, vec![(1, "1"), (10, "10"), (19, "hoje"), (30, "30")]);
        assert_eq!(p.forecast_label.as_deref(), Some("~R$ 4.460"));
        assert_eq!(p.normal_label.as_deref(), Some("normal R$ 3.662"));
        assert_eq!(p.income_label.as_deref(), Some("renda R$ 3.500"));
        assert_eq!(p.sentence.lead, "R$ 3.113,41 até o dia 19");
        assert_eq!(
            p.sentence.change,
            Some(Line {
                text: "15% acima".into(),
                tone: Tone::Warning
            })
        );
        assert_eq!(p.sentence.tail.as_deref(), Some("do seu normal"));
        assert_eq!(
            p.sentence.text(),
            "R$ 3.113,41 até o dia 19 — 15% acima do seu normal"
        );
    }

    #[test]
    fn reserva_e_caixinha_da_fixture() {
        let r = nov().reserve;
        let EmergencyBlock::Present(e) = &r.emergency else {
            panic!("fixture tem emergencia");
        };
        assert_eq!(e.title, "Reserva de emergência");
        assert_eq!(e.chip, "hoje");
        assert_eq!(e.coverage, "0,1");
        assert_eq!(e.coverage_suffix, "de 6 meses cobertos");
        assert_eq!(e.segments.len(), 6);
        assert!(close(e.segments[0], 0.14) && e.segments[1] == 0.0);
        assert_eq!(e.segment_labels, ["1", "2", "3", "4", "5", "6"]);
        let ms = e.milestone.as_ref().expect("marco");
        assert_eq!(ms.label, "Próximo marco:");
        assert_eq!(ms.target, "1 mês (R$ 2.500)");
        assert_eq!(ms.missing, "· faltam R$ 2.150");
        assert_eq!(e.reached, None);
        // O custo da fixture e o informado no app (`essentialOverrideMinor`): spec 2.2.
        assert_eq!(
            e.goal_line,
            "Meta final: 6 meses de custo informado no app, R$ 15.000."
        );
        assert_eq!(
            e.eta_line.as_deref(),
            Some("Se guardar R$ 550 por mês, a meta fecha em ~fev 2029.")
        );
        assert_eq!(r.month.title, "Neste mês");
        assert_eq!(
            r.month.deposited,
            Some(LabeledValue {
                label: "Guardado".into(),
                value: "+R$ 100,00".into()
            })
        );
        assert_eq!(
            r.month.withdrawn,
            Some(LabeledValue {
                label: "Retirado".into(),
                value: "\u{2212}R$ 300,00".into()
            })
        );
        assert_eq!(r.month.empty, None);
        assert_eq!(r.pots.len(), 1);
        let pot = &r.pots[0];
        assert_eq!(pot.name, "Casamento");
        assert_eq!(
            pot.meta,
            "R$ 150 de R$ 1.500 · até nov 2027 · sem depósito mensal"
        );
        assert_eq!(
            pot.pace,
            Some(Line {
                text: "precisa de ~R$ 113/mês".into(),
                tone: Tone::Warning
            })
        );
        assert_eq!(r.link, "Ver reservas");
    }

    #[test]
    fn meta_final_com_custo_das_categorias_diz_custo_essencial() {
        let ds = fixtures_grafite();
        let (re, _) = ordered(&ds);
        let mut view = emergency_view(&ds, re.expect("emergencia"), "2026-11", Money::default());
        if let Some(c) = view.cost.as_mut() {
            c.informed = false;
        }
        assert_eq!(
            emergency_card(&view, Some(55_000), Money::default()).goal_line,
            "Meta final: 6 meses de custo essencial, R$ 15.000."
        );
    }

    #[test]
    fn historico_da_fixture_em_lista() {
        let h = nov().history;
        assert_eq!(h.title, "Histórico");
        assert_eq!(
            (h.legend_income.as_str(), h.legend_expense.as_str()),
            ("Receitas", "Despesas")
        );
        let HistoryBody::List { lines, footnote } = &h.body else {
            panic!("dois meses de historico: lista");
        };
        let texts: Vec<(&str, &str)> = lines
            .iter()
            .map(|l| (l.label.as_str(), l.text.as_str()))
            .collect();
        assert_eq!(
            texts,
            vec![
                ("out · fechado", "sobrou R$ 1.410 · 28%"),
                ("nov · parcial, até dia 19", "R$ 387 até agora · 11%"),
            ]
        );
        // Barras na escala R$ 6.000, como o design (84,5% / 61% / 58,3% / 51,9%).
        assert!(close(lines[0].income, 0.845_381) && close(lines[0].expense, 0.610_333));
        assert!(close(lines[1].income, 0.583_333) && close(lines[1].expense, 0.518_901));
        assert!(!lines[0].partial && lines[1].partial);
        assert_eq!(
            footnote,
            "O gráfico de 12 meses aparece a partir do 4º mês de histórico."
        );
        assert_eq!(
            h.average,
            AverageLine {
                label: "Sobra média dos meses fechados:".into(),
                value: Some("R$ 1.410,29".into()),
                suffix: Some("· só outubro".into()),
            }
        );
    }

    #[test]
    fn historico_com_quatro_meses_vira_grafico() {
        let ds = edited(
            |_| false,
            vec![tx("OLD", "expense", 100, "2026-08-02", json!({}))],
        );
        let v = build_grafite(&ds, GRAFITE_TODAY, "2026-11", Book::Personal, false);
        let HistoryBody::Chart(c) = &v.history.body else {
            panic!("quatro meses: grafico");
        };
        assert_eq!(c.columns.len(), 12);
        assert_eq!(c.columns.iter().filter(|c| c.existed).count(), 4);
        assert_eq!(c.columns[0].label, "dez");
        assert_eq!(c.columns[0].tooltip, None);
        assert_eq!(
            c.columns[10].tooltip.as_deref(),
            Some(
                "out 2026 · receitas R$ 5.072,29 · despesas R$ 3.662,00 · sobrou R$ 1.410,29 (28%)"
            )
        );
        assert_eq!(
            c.columns[8].tooltip.as_deref(),
            Some("ago 2026 · receitas R$ 0,00 · despesas R$ 1,00 · faltou R$ 1,00")
        );
        assert_eq!(c.columns[8].rate, None);
        assert!(c.columns[11].partial);
        assert_eq!(c.y_labels, ["R$ 6 mil", "R$ 3 mil", "R$ 0"]);
        assert_eq!(c.rate_labels, ["\u{2212}20%", "60%"]);
        assert!(close(c.target, 0.5));
        assert_eq!(c.target_label, "alvo 20%");
        // Agosto (-1,00), setembro (vazio, conta zero) e outubro.
        assert_eq!(
            v.history.average,
            AverageLine {
                label: "Sobra média dos meses fechados:".into(),
                value: Some("R$ 469,76".into()),
                suffix: Some("· 3 meses".into()),
            }
        );
    }

    #[test]
    fn para_onde_foi_da_fixture() {
        let w = nov().where_it_went;
        assert_eq!(w.title, "Para onde foi");
        assert_eq!(w.question, "Onde dá para cortar ou ajustar a meta?");
        assert_eq!(
            w.summary,
            "R$ 876 fixos · R$ 2.237 variáveis · 9 categorias"
        );
        assert_eq!(w.plan.title, "O plano cabe na renda?");
        assert_eq!(w.plan.income_label.as_deref(), Some("renda fixa R$ 3.500"));
        assert_eq!(
            w.plan.legend,
            [
                "Metas ~R$ 3.045 · 87%",
                "Gasto sem meta R$ 407 · 12%",
                "Sobra para guardar ~R$ 48"
            ]
        );
        let [metas, sem_meta, sobra] = w.plan.bar.expect("barra");
        assert!(close(metas, 0.87) && close(sem_meta, 0.116_337) && close(sobra, 0.013_663));
        assert_eq!(
            w.plan.message,
            "A reserva pede R$ 550 por mês e o plano deixa ~R$ 48. Para ela caber, ajuste as \
             metas no app."
        );
        let a = w.attention.clone().expect("metas em atencao");
        assert_eq!(a.title, "Metas em atenção · 4 de 5");
        assert_eq!(a.days_left.as_deref(), Some("faltam 11 dias no mês"));
        let rows: Vec<(&str, &str, &str, &str, Tone)> = a
            .rows
            .iter()
            .map(|r| {
                (
                    r.name.as_str(),
                    r.spent.as_str(),
                    r.limit.as_str(),
                    r.status.as_str(),
                    r.tone,
                )
            })
            .collect();
        assert_eq!(
            rows,
            vec![
                (
                    "Compras",
                    "R$ 515,15",
                    "de R$ 400",
                    "128% · passou R$ 115,15",
                    Tone::Expense
                ),
                (
                    "Conforto",
                    "R$ 394,36",
                    "de R$ 400",
                    "98% · restam R$ 5,64",
                    Tone::Warning
                ),
                (
                    "Assinaturas",
                    "R$ 168,37",
                    "de R$ 180",
                    "93% · restam R$ 11,63",
                    Tone::Warning
                ),
                (
                    "Contas",
                    "R$ 539,01",
                    "de R$ 650",
                    "82% · restam R$ 110,99",
                    Tone::Warning
                ),
            ]
        );
        let marks: Vec<Option<&str>> = a.rows.iter().map(|r| r.above_normal.as_deref()).collect();
        assert_eq!(marks, [Some("↑ 38% do normal"), None, None, None]);
        assert!(close(a.rows[1].fill, 0.758_385));
        assert!(a.rows.iter().all(|r| close(r.marker, 0.769_231)));
        assert_eq!(w.others_title, "Demais categorias");
        assert!(
            w.others_info
                .starts_with("Em “Metas em atenção”, a barra mostra")
        );
        let others: Vec<(&str, &str, &str)> = w
            .others
            .iter()
            .map(|c| (c.name.as_str(), c.amount.as_str(), c.note.as_str()))
            .collect();
        assert_eq!(
            others,
            vec![
                ("Mercado", "R$ 1.089,34", "76% da meta"),
                ("Sogrinha", "R$ 174,90", "6% do gasto"),
                ("Manutenção", "R$ 107,28", "3% do gasto"),
                ("Pets", "R$ 75,00", "2% do gasto"),
                ("Transporte", "R$ 50,00", "2% do gasto"),
            ]
        );
        // Teto comum R$ 1.500 (spec 2.3): 72,6% e o traco da meta em 94,3%.
        assert!(close(w.others[0].fill, 0.726_227));
        assert!(w.others[0].marker.is_some_and(|m| close(m, 0.943_333)));
        assert!(close(w.others[1].fill, 0.1166) && w.others[1].marker.is_none());
    }

    #[test]
    fn o_que_vem_da_fixture() {
        let n = nov().whats_next.expect("mes corrente");
        assert_eq!(n.title, "O que vem");
        assert_eq!(n.question, "O que já está comprometido?");
        let f = &n.free;
        assert_eq!(f.title, "Livre em dezembro");
        // Sem serie "valor varia" nem estimada: sem "~" (spec 3.2, decisao do app).
        assert!(!f.approx);
        let amount = f.amount.as_ref().expect("valor");
        assert_eq!(
            (amount.whole.as_str(), amount.cents.as_str()),
            ("2.624", "")
        );
        assert_eq!(f.of_income.as_deref(), Some("de R$ 3.500 de renda fixa"));
        assert!(close(f.committed_fill, 0.25));
        assert_eq!(
            f.marks,
            vec![(0.5, "50%".to_string()), (0.7, "70%".to_string())]
        );
        assert_eq!(f.committed, "R$ 876 já comprometido ·");
        assert_eq!(f.committed_percent.as_deref(), Some("25% da renda fixa"));
        assert_eq!(
            f.reference.as_deref(),
            Some("Referência: até 50% há folga; acima de 70% qualquer imprevisto aperta.")
        );
        assert_eq!((f.invite.as_ref(), f.estimate_note.as_ref()), (None, None));
        assert_eq!(n.variable.title, "E o gasto variável?");
        assert_eq!(
            n.variable.text,
            "Em novembro ele já levou R$ 2.237 em 19 dias. Em dezembro precisa caber em R$ 2.624."
        );
        let u = &n.upcoming;
        assert_eq!(u.title, "Próximas saídas da conta");
        assert_eq!(u.window, "30 dias");
        assert_eq!(
            u.invoices,
            vec![InvoiceLine {
                title: "Fatura Mercado Pago".into(),
                icon: "credit-card".into(),
                when: "vence ter, 8 dez · em 19 dias · fatura aberta".into(),
                amount: "~R$ 515,15".into(),
                percent: Some(Line {
                    text: "15% da renda.".into(),
                    tone: Tone::Neutral
                }),
                note: "Já está nas despesas: é o dinheiro que precisa estar na conta no \
                       vencimento."
                    .into(),
            }]
        );
        let items: Vec<(&str, &str, &str)> = u
            .items
            .iter()
            .map(|i| (i.description.as_str(), i.when.as_str(), i.amount.as_str()))
            .collect();
        assert_eq!(items.len(), 8);
        assert_eq!(
            items[..3],
            [
                ("Cadeira (2x)", "sáb, 21 nov · em 2 dias", "R$ 50,52"),
                // Mesma data: desempate pelo id da serie (RMEL < RPRI, decisao 5.10).
                ("Meli+", "ter, 1 dez · em 12 dias", "R$ 9,99"),
                ("Amazon prime", "ter, 1 dez · em 12 dias", "R$ 19,99"),
            ]
        );
        // Serie sem categoria: tile neutro com `repeat`; com categoria: o tile dela.
        assert_eq!(
            (u.items[1].icon.as_str(), u.items[1].color.as_ref()),
            ("repeat", None)
        );
        assert_eq!(u.items[0].icon, "shopping-bag");
        assert_eq!(u.items[0].color.as_deref(), Some("violet"));
        let salary = u
            .items
            .iter()
            .find(|i| i.description == "Salário")
            .expect("salario");
        assert_eq!(salary.amount, "+R$ 3.500,00");
        assert!(salary.income);
        assert_eq!(u.preview, 3);
        assert_eq!(u.see_all.as_deref(), Some("Ver todas as 9"));
        assert_eq!(u.empty, None);
    }

    #[test]
    fn ultimos_lancamentos_e_quem_lancou_da_fixture() {
        let v = nov();
        let r = &v.recent;
        assert_eq!(r.title, "Últimos lançamentos");
        assert_eq!(r.count, "20 em novembro");
        let lines: Vec<(&str, Option<&str>, &str, &str)> = r
            .lines
            .iter()
            .map(|l| {
                (
                    l.description.as_str(),
                    l.chip.as_deref(),
                    l.meta.as_str(),
                    l.amount.as_str(),
                )
            })
            .collect();
        assert_eq!(
            lines,
            vec![
                (
                    "Claude code",
                    Some("Mensal"),
                    "Assinaturas · Luiz · 18 nov",
                    "\u{2212}R$ 118,40"
                ),
                (
                    "Almoço",
                    None,
                    "Conforto · Luiz · 18 nov",
                    "\u{2212}R$ 20,97"
                ),
                (
                    "Jantinha",
                    None,
                    "Conforto · Luiz · 18 nov",
                    "\u{2212}R$ 65,89"
                ),
            ]
        );
        assert_eq!(r.lines[0].dot.as_deref(), Some("indigo"));
        assert!(r.lines.iter().all(|l| !l.excluded && l.tag.is_none()));
        assert_eq!(
            r.info,
            "Mesma regra dos totais: transferências aparecem marcadas e ficam fora da soma."
        );
        assert_eq!(r.link, "Ver todos");
        assert_eq!(r.empty, None);
        let p = v.people.clone().expect("duas pessoas");
        assert_eq!(p.title, "Quem lançou · novembro");
        assert_eq!(
            p.rows,
            vec![
                PersonLine {
                    initial: "L".into(),
                    name: "Luiz".into(),
                    color: Some("teal".into()),
                    line: "despesas R$ 2.640,41 · receitas R$ 3.500,00".into(),
                },
                PersonLine {
                    initial: "G".into(),
                    name: "Gigi".into(),
                    color: Some("rose".into()),
                    line: "despesas R$ 473,00".into(),
                },
            ]
        );
        assert_eq!(p.footnote, "Para combinar a divisão da casa.");
    }

    #[test]
    fn transferencia_aparece_marcada_e_fora_das_somas() {
        let ds = edited(
            |_| false,
            vec![tx(
                "1119-9-TRANSF",
                "expense",
                99_999,
                "2026-11-19",
                json!({ "reportState": "transfer", "userId": "UL" }),
            )],
        );
        let v = build_grafite(&ds, GRAFITE_TODAY, "2026-11", Book::Personal, false);
        let first = &v.recent.lines[0];
        assert_eq!(first.tag.as_deref(), Some("Transferência"));
        assert!(first.excluded);
        assert_eq!(v.recent.count, "21 em novembro");
        // Nenhuma soma muda.
        let base = nov();
        assert_eq!(v.how_am_i, base.how_am_i);
        assert_eq!(v.where_it_went, base.where_it_went);
        assert_eq!(v.people, base.people);
    }

    #[test]
    fn mes_fechado_compara_com_o_seu_normal_e_some_o_que_vem() {
        // Setembro com R$ 1.000 de saldo vira o normal de outubro.
        let ds = edited(
            |_| false,
            vec![tx(
                "0905-1-RECEITA",
                "income",
                100_000,
                "2026-09-05",
                json!({}),
            )],
        );
        let v = build_grafite(&ds, GRAFITE_TODAY, "2026-10", Book::Personal, false);
        assert!(!v.is_current);
        assert_eq!(v.header.partial, None);
        assert_eq!(v.header.month_label, "Out 2026");
        let c = &v.how_am_i;
        assert_eq!(c.balance_label, "Saldo do mês");
        assert_eq!(
            c.outlook,
            Some(Line {
                text: "R$ 410,29 a mais que o seu normal".into(),
                tone: Tone::Income
            })
        );
        assert_eq!(
            (c.zero_box.as_ref(), c.casual_average.as_ref()),
            (None, None)
        );
        let s = c.savings.as_ref().expect("taxa");
        assert_eq!((s.rate.as_str(), s.pace.as_ref()), ("28% no mês", None));
        assert_eq!(
            c.reserves_line.as_deref(),
            Some("R$ 700 foram para reservas no mês")
        );
        let p = &c.pace;
        assert_eq!(p.legend_forecast, None);
        assert_eq!(p.forecast, None);
        assert_eq!(p.sentence.lead, "R$ 3.662,00 no mês");
        let x: Vec<(usize, &str)> = p.x_labels.iter().map(|(d, l)| (*d, l.as_str())).collect();
        assert_eq!(x, vec![(1, "1"), (10, "10"), (31, "31")]);
        assert!(!p.info.contains("previsão"));
        // A reserva continua com os valores de hoje; so o bloco do mes segue o seletor.
        let EmergencyBlock::Present(e) = &v.reserve.emergency else {
            panic!("emergencia");
        };
        assert_eq!(e.coverage, "0,1");
        assert_eq!(v.reserve.month.title, "Em outubro");
        assert_eq!(
            v.reserve.month.deposited.as_ref().map(|d| d.value.as_str()),
            Some("+R$ 700,00")
        );
        assert_eq!(v.reserve.month.withdrawn, None);
        assert_eq!(
            v.where_it_went
                .attention
                .as_ref()
                .and_then(|a| a.days_left.clone()),
            None
        );
        assert_eq!(v.whats_next, None);
        assert_eq!(v.recent.count, "5 em outubro");
        assert_eq!(
            v.sections(),
            vec![
                Section::HowAmI,
                Section::Reserve,
                Section::History,
                Section::WhereItWent,
                Section::Recent,
            ]
        );
        // Mes fechado sem mes de referencia: a linha some.
        let alone = build_grafite(
            &fixtures_grafite(),
            GRAFITE_TODAY,
            "2026-10",
            Book::Personal,
            false,
        );
        assert_eq!(alone.how_am_i.outlook, None);
    }

    #[test]
    fn ate_o_dia_5_nao_ha_previsao() {
        let v = build_grafite(
            &fixtures_grafite(),
            "2026-11-05",
            "2026-11",
            Book::Personal,
            false,
        );
        let c = &v.how_am_i;
        assert_eq!((c.outlook.as_ref(), c.zero_box.as_ref()), (None, None));
        assert_eq!(c.casual_average, None);
        assert_eq!(c.savings.as_ref().and_then(|s| s.pace.clone()), None);
        assert_eq!(
            (c.pace.forecast, c.pace.legend_forecast.as_ref()),
            (None, None)
        );
        assert!(c.pace.sentence.lead.ends_with("até o dia 5"));
    }

    #[test]
    fn conta_comercial_traz_o_negocio_primeiro() {
        let ds = edited(
            |_| false,
            vec![
                business_book(),
                tx(
                    "1110-1-VENDA",
                    "income",
                    80_000,
                    "2026-11-10",
                    json!({ "book": "business", "userId": "UL" }),
                ),
                tx(
                    "1111-1-FARINHA",
                    "expense",
                    30_000,
                    "2026-11-11",
                    json!({ "book": "business", "userId": "UL" }),
                ),
            ],
        );
        let personal = build_grafite(
            &ds.for_book(Book::Personal),
            GRAFITE_TODAY,
            "2026-11",
            Book::Personal,
            false,
        );
        assert!(personal.header.book_switch);
        assert_eq!(personal.business, None);
        let v = build_grafite(
            &ds.for_book(Book::Business),
            GRAFITE_TODAY,
            "2026-11",
            Book::Business,
            false,
        );
        assert_eq!(
            v.header.subtitle,
            "Padaria · somente leitura · dados do último sync"
        );
        assert_eq!(v.sections()[..2], [Section::Business, Section::HowAmI]);
        let b = v.business.clone().expect("negocio");
        assert_eq!(b.title, "Negócio");
        assert_eq!(b.question, "O negócio dá resultado?");
        assert_eq!(b.result.value, "R$ 500,00");
        assert_eq!(b.income.value, "R$ 800,00");
        assert_eq!(b.expense.value, "R$ 300,00");
        assert_eq!(b.today_chip, "hoje");
        assert_eq!(
            (b.capital_title.as_str(), b.debt_title.as_str()),
            ("Capital", "Dívidas")
        );
        // O resto roda sobre a conta comercial: sem reserva de emergencia nela (7.6).
        assert!(matches!(
            v.reserve.emergency,
            EmergencyBlock::Missing { .. }
        ));
        assert_eq!(v.how_am_i.income.value, "R$ 800,00");
    }

    #[test]
    fn sem_dados() {
        let v = build_grafite(
            &Dataset::default(),
            GRAFITE_TODAY,
            "2026-11",
            Book::Personal,
            false,
        );
        assert!(v.is_empty);
        assert_eq!(v.sections(), vec![Section::Empty]);
        assert_eq!(v.header.partial, None);
        assert_eq!(v.header.subtitle, "Nada sincronizado ainda");
        assert!(!v.sidebar.has_data);
        assert_eq!(
            v.recent.empty.as_deref(),
            Some("Nenhum lançamento em novembro")
        );
        assert_eq!(v.history.average.label, "Ainda não há mês fechado");
    }

    #[test]
    fn sem_reserva_de_emergencia_e_sem_reserva_nenhuma() {
        let ds = edited(|id| id == "RE" || id.starts_with("MOV-RE"), Vec::new());
        let v = build_grafite(&ds, GRAFITE_TODAY, "2026-11", Book::Personal, false);
        assert_eq!(
            v.reserve.emergency,
            EmergencyBlock::Missing {
                title: "Reserva de emergência".into(),
                message: "Ainda não há reserva de emergência".into(),
                hint: "Crie no app em Reservas; ela mede quantos meses de custo essencial você \
                       aguenta."
                    .into(),
            }
        );
        assert_eq!(v.reserve.pots.len(), 1);
        assert_eq!(v.reserve.month.empty.as_deref(), Some("Nenhum movimento"));
        // Sem pedido de deposito: o plano so diz quanto sobra.
        assert_eq!(
            v.where_it_went.plan.message,
            "Sobram ~R$ 48 por mês para guardar."
        );
        let none = edited(
            |id| id == "RE" || id == "RC" || id.starts_with("MOV-"),
            Vec::new(),
        );
        let v = build_grafite(&none, GRAFITE_TODAY, "2026-11", Book::Personal, false);
        assert!(v.reserve.pots.is_empty());
        assert_eq!(v.reserve.month.empty.as_deref(), Some("Nenhum movimento"));
        assert_eq!(v.how_am_i.reserves_line, None);
        assert_eq!(v.sidebar.reserves, 0);
    }

    #[test]
    fn sem_renda_recorrente_sem_metas_e_lista_vazia() {
        let ds = dataset_of(vec![
            (
                TRANSACTIONS,
                "A",
                false,
                json!({ "kind": "expense", "description": "Mercado", "amountMinor": 4000,
                        "occurredOn": "2026-11-10", "userId": "U1" }),
            ),
            (
                TRANSACTIONS,
                "B",
                false,
                json!({ "kind": "income", "description": "Freela", "amountMinor": 10_000,
                        "occurredOn": "2026-11-12", "userId": "U1" }),
            ),
        ]);
        let v = build_grafite(&ds, GRAFITE_TODAY, "2026-11", Book::Personal, false);
        let n = v.whats_next.clone().expect("mes corrente");
        assert_eq!(n.free.amount, None);
        assert_eq!(
            n.free.invite.as_deref(),
            Some("Cadastre o salário como recorrente no app para ver quanto fica livre")
        );
        assert!(n.free.marks.is_empty());
        assert_eq!(
            (n.free.committed_percent.as_ref(), n.free.reference.as_ref()),
            (None, None)
        );
        assert_eq!(n.free.committed, "R$ 0 já comprometido");
        assert_eq!(
            n.variable.text,
            "Em novembro ele já levou R$ 40 em 19 dias."
        );
        assert!(n.upcoming.invoices.is_empty() && n.upcoming.items.is_empty());
        assert_eq!(
            n.upcoming.empty.as_deref(),
            Some("Nada programado nos próximos 30 dias.")
        );
        assert_eq!(n.upcoming.see_all, None);
        let plan = &v.where_it_went.plan;
        assert_eq!((plan.income_label.as_ref(), plan.bar), (None, None));
        assert_eq!(plan.legend, ["Gasto sem meta R$ 40"]);
        assert_eq!(
            plan.message,
            "Cadastre o salário como recorrente no app para ver quanto fica livre"
        );
        assert_eq!(v.where_it_went.attention, None);
        assert_eq!(v.where_it_went.others_title, "Categorias");
        assert_eq!(v.where_it_went.others[0].note, "100% do gasto");
        // Um autor so: o card some.
        assert_eq!(v.people, None);
        // Primeiro mes de historico: ritmo sem normal.
        let p = &v.how_am_i.pace;
        assert_eq!(
            (p.legend_normal.as_ref(), p.sentence.change.as_ref()),
            (None, None)
        );
        assert!(p.info.contains("ainda não há mês anterior com dados"));
        assert_eq!(p.sentence.text(), "R$ 40,00 até o dia 19");
    }

    #[test]
    fn com_valores_ocultos_nenhum_real_aparece() {
        let four = edited(
            |_| false,
            vec![
                tx("OLD", "expense", 100, "2026-08-02", json!({})),
                tx(
                    "1119-9-TRANSF",
                    "expense",
                    99_999,
                    "2026-11-19",
                    json!({ "reportState": "transfer", "userId": "UL" }),
                ),
                business_book(),
                tx(
                    "1110-1-VENDA",
                    "income",
                    80_000,
                    "2026-11-10",
                    json!({ "book": "business" }),
                ),
            ],
        );
        let cases = [
            (fixtures_grafite(), GRAFITE_TODAY, "2026-11", Book::Personal),
            (fixtures_grafite(), GRAFITE_TODAY, "2026-10", Book::Personal),
            (fixtures_grafite(), "2026-11-05", "2026-11", Book::Personal),
            (
                four.for_book(Book::Personal),
                GRAFITE_TODAY,
                "2026-11",
                Book::Personal,
            ),
            (
                four.for_book(Book::Business),
                GRAFITE_TODAY,
                "2026-11",
                Book::Business,
            ),
            (Dataset::default(), GRAFITE_TODAY, "2026-11", Book::Personal),
        ];
        for (ds, today, month, book) in cases {
            let shown = format!("{:#?}", build_grafite(&ds, today, month, book, false));
            let hidden_view = build_grafite(&ds, today, month, book, true);
            let hidden = format!("{hidden_view:#?}");
            assert!(
                leaks(&shown).is_some(),
                "{month} {book:?}: o visivel tem valor"
            );
            assert_eq!(leaks(&hidden), None, "{month} {book:?}");
            assert_eq!(hidden_view.header.hide_toggle, "Mostrar valores");
        }
        // Porcentagens, meses cobertos, datas e contagens ficam.
        let v = build_grafite(
            &fixtures_grafite(),
            GRAFITE_TODAY,
            "2026-11",
            Book::Personal,
            true,
        );
        assert_eq!(v.how_am_i.balance.whole, "•••••");
        assert_eq!(v.how_am_i.balance.cents, "");
        assert_eq!(
            v.how_am_i.savings.as_ref().map(|s| s.rate.as_str()),
            Some("11% até agora")
        );
        assert_eq!(
            v.how_am_i.outlook.as_ref().map(|l| l.text.as_str()),
            Some("Vão faltar ~R$ ••••• no fim do mês, se continuar no ritmo atual")
        );
        assert_eq!(v.how_am_i.pace.y_labels, ["•••", "•••", "•••"]);
        assert_eq!(
            v.where_it_went
                .attention
                .as_ref()
                .map(|a| a.rows[0].status.as_str()),
            Some("128% · passou R$ •••••")
        );
        let EmergencyBlock::Present(e) = &v.reserve.emergency else {
            panic!("emergencia");
        };
        assert_eq!(e.coverage, "0,1");
        assert_eq!(v.recent.count, "20 em novembro");
        // A forma continua: as barras nao mudam com a mascara.
        assert_eq!(v.how_am_i.pace.current, nov().how_am_i.pace.current);
    }

    /// Regra n. 1 do handoff ("guardar nao e gastar") no Dashboard novo: movimento de reserva so
    /// muda o card da reserva e a linha "saíram das reservas", nunca receita, despesa, ritmo,
    /// previsao, plano ou lancamentos.
    #[test]
    fn movimentos_de_reserva_so_mudam_o_card_da_reserva() {
        let without = edited(|id| id.starts_with("MOV-"), Vec::new());
        for month in ["2026-09", "2026-10", "2026-11"] {
            let mut a = build_grafite(
                &fixtures_grafite(),
                GRAFITE_TODAY,
                month,
                Book::Personal,
                false,
            );
            let mut b = build_grafite(&without, GRAFITE_TODAY, month, Book::Personal, false);
            assert_ne!(a.reserve, b.reserve, "{month}");
            for v in [&mut a, &mut b] {
                v.reserve = ReserveCard::default();
                v.how_am_i.reserves_line = None;
            }
            assert_eq!(a, b, "{month}");
        }
    }
}
