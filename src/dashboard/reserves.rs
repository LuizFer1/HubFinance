//! Reservas: saldos, series, totais, custo essencial, meta da emergencia, ritmo das caixinhas
//! e a lista de movimentacoes, tudo puro e a partir do `Dataset`.
//!
//! Movimentacao de reserva nunca entra em receita nem despesa: `aggregate.rs` e `list.rs` so
//! leem `transactions`, e o teste `reservas_nao_entram_no_dashboard_nem_nos_lancamentos` de
//! `view.rs` prova isso. A unica leitura de `transactions` aqui e o custo essencial.

use super::aggregate::js_round;
use super::contract::{Kind, MovementKind, ReserveKind};
use super::dataset::{Dataset, Reserve, ReserveMovement};
use super::list::RowAuthor;
use super::money::{format_brl, one_decimal, whole_brl};
use super::periods::{last_months, month_of, month_short_year, months_between, shift_month};

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
        ReserveKind::Goal => ReserveColor::Token(r.color.clone()),
    }
}

/// Chave de icone: a emergencia usa sempre `lifebuoy` (chave interna da UI, nao do app); a
/// caixinha usa a chave que o app gravou.
pub fn reserve_icon(r: &Reserve) -> &str {
    match r.kind {
        ReserveKind::Emergency => "lifebuoy",
        ReserveKind::Goal => &r.icon,
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
    /// True quando o total veio de `essentialOverrideMinor` (custo digitado no app quando a casa
    /// nao tinha historico); nesse caso `parts` e vazio e `months` e 0, porque nao existe media
    /// por categoria para mostrar.
    pub informed: bool,
}

/// Custo essencial medio por mes. Janela: os seis meses **completos** antes do corrente (o mes
/// corrente e parcial e derrubaria a media). Denominador: meses da janela a partir do primeiro
/// lancamento vivo do hub — uma casa com 2 meses de historico dividiria por 6 e teria meta
/// irreal. Sem lancamento vivo antes do mes corrente -> `None` (sem base).
///
/// Casa pelo id resolvido (`mergedInto` seguido so em tombstone), viva ou apagada: o gasto
/// passado foi essencial mesmo que a categoria tenha sido apagada depois.
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
    for raw in category_ids {
        // Copia fundida e padrao na mesma lista sao a mesma categoria: deduplicar pelo id
        // resolvido, senao ela contaria duas vezes. Categoria apagada sem `mergedInto` resolve
        // para ela mesma e continua casando pelo id cru.
        let Some(id) = dataset.resolve_category_id(Some(raw.as_str())) else {
            continue;
        };
        if seen.contains(&id) {
            continue;
        }
        seen.push(id);
        // Lancamento antigo ainda aponta para a copia (o app nao regrava): resolve dos dois
        // lados.
        let sum = dataset
            .alive_transactions()
            .filter(|t| {
                t.kind == Kind::Expense
                    && dataset.resolve_category_id(t.category_id.as_deref()) == Some(id)
                    && window.contains(&month_of(&t.occurred_on))
            })
            .fold(0i64, |acc, t| acc.saturating_add(t.amount_minor));
        let (name, color) = match dataset.find_category(Some(id)) {
            Some(c) => (c.name.clone(), c.color.clone()),
            None => ("Categoria removida".to_string(), "slate".to_string()),
        };
        parts.push(EssentialPart {
            category_id: id.to_string(),
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
        informed: false,
    })
}

/// Custo da emergencia: espelha `emergencyTarget` do app. O override vence o calculado mesmo
/// havendo historico (o usuario disse quanto custa); override <= 0 e sem base — o app devolve
/// meta nula e nao cai para o calculado, e o hub nunca "conserta" o dado.
pub fn emergency_cost(dataset: &Dataset, r: &Reserve, current: &str) -> Option<EssentialCost> {
    match r.essential_override_minor {
        Some(o) if o > 0 => Some(EssentialCost {
            total_minor: o,
            months: 0,
            parts: Vec::new(),
            informed: true,
        }),
        Some(_) => None,
        None => essential_cost(dataset, &r.essential_category_ids, current),
    }
}

/// Meta derivada: `multiple x custo`; sem base ou com custo <= 0 (como o app), 0.
fn derived_goal(cost: Option<&EssentialCost>, multiple: u8) -> i64 {
    cost.filter(|c| c.total_minor > 0)
        .map_or(0, |c| c.total_minor.saturating_mul(i64::from(multiple)))
}

/// Meta de uma reserva de emergencia: `multiple x custo informado no app`, ou, sem ele,
/// `multiple x custo essencial das categorias dela`.
pub fn emergency_goal(dataset: &Dataset, r: &Reserve, current: &str) -> i64 {
    let cost = emergency_cost(dataset, r, current);
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
    let cost = emergency_cost(dataset, r, current);
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

/// Ritmo de uma caixinha.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pace {
    NoGoal,
    Reached,
    Overdue { remaining: i64 },
    NoDeadline { recurring: Option<i64> },
    NoRecurring { needed: i64 },
    OnTrack { recurring: i64, needed: i64 },
    Behind { recurring: i64, needed: i64 },
}

/// A ordem das condicoes e a da tabela da spec e importa: meta alcancada vence prazo vencido
/// (quem fechou a meta tarde fechou), e prazo vencido vence ritmo (nao ha "por mes" sem mes
/// pela frente). Recorrencia `<= 0` vale como ausente.
pub fn pace(
    balance: i64,
    goal: Option<i64>,
    due: Option<&str>,
    recurring: Option<i64>,
    current: &str,
) -> Pace {
    let Some(goal) = goal else {
        return Pace::NoGoal;
    };
    if balance >= goal {
        return Pace::Reached;
    }
    let remaining = goal.saturating_sub(balance);
    let recurring = recurring.filter(|r| *r > 0);
    let Some(due) = due else {
        return Pace::NoDeadline { recurring };
    };
    let months_left = months_between(current, due);
    if months_left <= 0 {
        return Pace::Overdue { remaining };
    }
    let needed = div_ceil(remaining, i64::from(months_left));
    match recurring {
        None => Pace::NoRecurring { needed },
        Some(r) if r >= needed => Pace::OnTrack {
            recurring: r,
            needed,
        },
        Some(r) => Pace::Behind {
            recurring: r,
            needed,
        },
    }
}

/// Texto da linha de ritmo; `None` = sem linha.
pub fn pace_label(p: &Pace) -> Option<String> {
    Some(match p {
        Pace::NoGoal | Pace::NoDeadline { recurring: None } => return None,
        Pace::Reached => "Meta alcançada".to_string(),
        Pace::Overdue { remaining } => {
            format!("Prazo encerrado · faltam {}", whole_brl(*remaining))
        }
        Pace::NoDeadline { recurring: Some(r) } => format!("Guarda {}/mês", whole_brl(*r)),
        Pace::NoRecurring { needed } => format!(
            "Sem depósito mensal · precisa de {}/mês",
            whole_brl(*needed)
        ),
        Pace::OnTrack { recurring, needed } => format!(
            "No ritmo · guarda {}/mês, precisa de {}",
            whole_brl(*recurring),
            whole_brl(*needed)
        ),
        Pace::Behind { recurring, needed } => format!(
            "Abaixo do ritmo · precisa de {}/mês, guarda {}",
            whole_brl(*needed),
            whole_brl(*recurring)
        ),
    })
}

/// Uma linha do card Caixinhas.
#[derive(Clone, Debug, PartialEq)]
pub struct PotRow {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub color: ReserveColor,
    pub balance_minor: i64,
    pub goal_minor: Option<i64>,
    pub due_month: Option<String>,
    /// Preenchimento da barra (0..=1); `None` sem meta (sem barra).
    pub progress: Option<f32>,
    /// "até jul 2027 · faltam R$ 2.650", "faltam R$ 300", "até nov 2026" ou "sem meta".
    pub meta_left: String,
    /// "de R$ 5.000,00 · 47%"; so com meta.
    pub meta_right: Option<String>,
    pub pace: Pace,
}

/// `derived_goal` e a meta de uma emergencia extra (as que nao viraram o card): ela entra aqui
/// com a meta derivada, sem prazo, `lifebuoy` e a cor do acento. Para caixinha, `None`. Meta
/// `<= 0` vale como sem meta: uma barra sobre zero nao diz nada e dividiria por zero.
pub fn pot_row(dataset: &Dataset, r: &Reserve, current: &str, derived_goal: Option<i64>) -> PotRow {
    let balance = balance_until(dataset, &r.id, current);
    let (goal, due) = match r.kind {
        ReserveKind::Emergency => (derived_goal, None),
        ReserveKind::Goal => (r.goal_minor, r.due_month.clone()),
    };
    let goal = goal.filter(|g| *g > 0);
    let remaining = goal.map(|g| g.saturating_sub(balance)).filter(|v| *v > 0);
    let faltam = remaining.map(|v| format!("faltam {}", whole_brl(v)));
    let meta_left = match (due.as_deref(), faltam) {
        (Some(d), Some(f)) => format!("até {} · {f}", month_short_year(d)),
        (Some(d), None) => format!("até {}", month_short_year(d)),
        (None, Some(f)) => f,
        (None, None) if goal.is_some() => "sem prazo".to_string(),
        (None, None) => "sem meta".to_string(),
    };
    PotRow {
        id: r.id.clone(),
        name: r.name.clone(),
        icon: reserve_icon(r).to_string(),
        color: reserve_color(r),
        balance_minor: balance,
        goal_minor: goal,
        progress: goal.map(|g| (balance as f64 / g as f64).clamp(0.0, 1.0) as f32),
        meta_right: goal.map(|g| {
            format!(
                "de {} · {}%",
                format_brl(g),
                js_round(balance as f64 / g as f64 * 100.0) as i64
            )
        }),
        meta_left,
        pace: pace(
            balance,
            goal,
            due.as_deref(),
            r.recurring_amount_minor,
            current,
        ),
        due_month: due,
    }
}

/// "3 com objetivo" ou "3 com objetivo · 1 sem meta".
pub fn pots_note(pots: &[PotRow]) -> String {
    let with = pots.iter().filter(|p| p.goal_minor.is_some()).count();
    let without = pots.len() - with;
    if without == 0 {
        format!("{with} com objetivo")
    } else {
        format!("{with} com objetivo · {without} sem meta")
    }
}

/// Linhas mostradas em "Movimentações".
pub const MOVEMENT_ROWS: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MovementRow {
    pub id: String,
    pub occurred_on: String,
    /// A da linha, ou "Guardado" / "Retirado".
    pub description: String,
    pub kind: MovementKind,
    pub amount_minor: i64,
    pub reserve_name: String,
    pub reserve_color: ReserveColor,
    pub author: Option<RowAuthor>,
    pub recurring: bool,
}

/// As `MOVEMENT_ROWS` mais recentes da janela (data desc, id desc, a ordem de `sort_desc`) e o
/// total da janela.
pub fn movements(dataset: &Dataset, months: &[String]) -> (Vec<MovementRow>, usize) {
    let mut items: Vec<&ReserveMovement> = dataset
        .alive_reserve_movements()
        .filter(|m| months.contains(&month_of(&m.occurred_on)))
        .collect();
    let total = items.len();
    items.sort_by(|a, b| {
        b.occurred_on
            .cmp(&a.occurred_on)
            .then_with(|| b.id.cmp(&a.id))
    });
    let rows = items
        .into_iter()
        .take(MOVEMENT_ROWS)
        .filter_map(|m| {
            // `alive_reserve_movements` ja garante a reserva viva.
            let r = dataset.reserves.get(&m.reserve_id)?;
            Some(MovementRow {
                id: m.id.clone(),
                occurred_on: m.occurred_on.clone(),
                description: m.description.clone().unwrap_or_else(|| {
                    match m.kind {
                        MovementKind::Deposit => "Guardado",
                        MovementKind::Withdrawal => "Retirado",
                    }
                    .to_string()
                }),
                kind: m.kind,
                amount_minor: m.amount_minor,
                reserve_name: r.name.clone(),
                reserve_color: reserve_color(r),
                author: dataset.find_user(m.user_id.as_deref()).map(|u| RowAuthor {
                    id: u.id.clone(),
                    name: u.name.clone(),
                    color: u.color.clone(),
                }),
                recurring: m.recurring,
            })
        })
        .collect();
    (rows, total)
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
            json!({ "reserveId": "RP4", "amountMinor": -10000, "occurredOn": "2026-09-01" }),
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
    fn copia_fundida_e_padrao_sao_uma_parte_so() {
        let mut ds = with_transactions(&[
            ("T1", "expense", 30_000, "2026-08-01", "C6"),
            ("T2", "expense", 20_000, "2026-08-02", "C1"),
        ]);
        ds.apply([
            RawRow {
                table: crate::dashboard::dataset::CATEGORIES.into(),
                id: "C1".into(),
                deleted_at: None,
                seq: 100,
                data: json!({ "name": "Alimentação", "color": "orange", "icon": "utensils" })
                    .to_string(),
            },
            RawRow {
                table: crate::dashboard::dataset::CATEGORIES.into(),
                id: "C6".into(),
                deleted_at: Some("x".into()),
                seq: 101,
                data: json!({ "name": "Alimentação", "color": "red", "icon": "tag",
                    "mergedInto": "C1" })
                .to_string(),
            },
        ]);
        let c = essential_cost(&ds, &strings(&["C6", "C1"]), "2026-09").unwrap();
        assert_eq!(c.parts.len(), 1);
        assert_eq!(c.parts[0].category_id, "C1");
        assert_eq!(c.parts[0].name, "Alimentação");
        assert_eq!(c.parts[0].color, "orange");
        assert_eq!(c.total_minor, 50_000);
        // So a copia na lista tambem pega os lancamentos do padrao.
        let c = essential_cost(&ds, &strings(&["C6"]), "2026-09").unwrap();
        assert_eq!(c.total_minor, 50_000);
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
            "recurring": if recurring.is_null() {
                serde_json::Value::Null
            } else {
                json!({ "amountMinor": recurring })
            }
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
            json!({ "reserveId": "RE1", "amountMinor": 200000, "occurredOn": "2026-09-02" }),
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

    const NOW: &str = "2026-09";

    #[test]
    fn ritmo_nas_sete_variantes() {
        assert_eq!(
            pace(235_000, Some(500_000), Some("2027-07"), Some(30_000), NOW),
            Pace::OnTrack {
                recurring: 30_000,
                needed: 26_500
            }
        );
        assert_eq!(
            pace(40_000, Some(240_000), Some("2027-01"), Some(20_000), NOW),
            Pace::Behind {
                recurring: 20_000,
                needed: 50_000
            }
        );
        assert_eq!(
            pace(38_000, Some(80_000), Some("2026-12"), None, NOW),
            Pace::NoRecurring { needed: 14_000 }
        );
        assert_eq!(pace(10_000, None, None, None, NOW), Pace::NoGoal);
        assert_eq!(
            pace(50_000, Some(50_000), Some("2026-11"), Some(10_000), NOW),
            Pace::Reached
        );
        for due in ["2026-09", "2026-08"] {
            assert_eq!(
                pace(10_000, Some(50_000), Some(due), Some(10_000), NOW),
                Pace::Overdue { remaining: 40_000 }
            );
        }
        assert_eq!(
            pace(10_000, Some(50_000), None, Some(30_000), NOW),
            Pace::NoDeadline {
                recurring: Some(30_000)
            }
        );
        assert_eq!(
            pace(10_000, Some(50_000), None, None, NOW),
            Pace::NoDeadline { recurring: None }
        );
        // Igual ao necessario esta no ritmo; zero vale como ausente.
        assert_eq!(
            pace(0, Some(30_000), Some("2026-12"), Some(10_000), NOW),
            Pace::OnTrack {
                recurring: 10_000,
                needed: 10_000
            }
        );
        assert_eq!(
            pace(0, Some(30_000), Some("2026-12"), Some(0), NOW),
            Pace::NoRecurring { needed: 10_000 }
        );
    }

    #[test]
    fn textos_do_ritmo() {
        let label = |p: Pace| pace_label(&p);
        assert_eq!(
            label(Pace::OnTrack {
                recurring: 30_000,
                needed: 26_500
            })
            .as_deref(),
            Some("No ritmo · guarda R$ 300/mês, precisa de R$ 265")
        );
        assert_eq!(
            label(Pace::Behind {
                recurring: 20_000,
                needed: 50_000
            })
            .as_deref(),
            Some("Abaixo do ritmo · precisa de R$ 500/mês, guarda R$ 200")
        );
        assert_eq!(
            label(Pace::NoRecurring { needed: 14_000 }).as_deref(),
            Some("Sem depósito mensal · precisa de R$ 140/mês")
        );
        assert_eq!(label(Pace::Reached).as_deref(), Some("Meta alcançada"));
        assert_eq!(
            label(Pace::Overdue { remaining: 40_000 }).as_deref(),
            Some("Prazo encerrado · faltam R$ 400")
        );
        assert_eq!(
            label(Pace::NoDeadline {
                recurring: Some(30_000)
            })
            .as_deref(),
            Some("Guarda R$ 300/mês")
        );
        assert_eq!(label(Pace::NoDeadline { recurring: None }), None);
        assert_eq!(label(Pace::NoGoal), None);
    }

    #[test]
    fn linhas_das_caixinhas() {
        let ds = fixtures_with_reserves();
        let row = |id: &str| pot_row(&ds, &ds.reserves[id], NOW, None);
        let p1 = row("RP1");
        assert_eq!(p1.balance_minor, 235_000);
        assert!((p1.progress.unwrap() - 0.47).abs() < 1e-6);
        assert_eq!(p1.meta_left, "até jul 2027 · faltam R$ 2.650");
        assert_eq!(p1.meta_right.as_deref(), Some("de R$ 5.000,00 · 47%"));
        assert_eq!(p1.icon, "plane");
        assert_eq!(p1.color, ReserveColor::Token("sky".into()));
        assert_eq!(
            row("RP2").meta_right.as_deref(),
            Some("de R$ 2.400,00 · 17%")
        );
        assert_eq!(row("RP3").meta_right.as_deref(), Some("de R$ 800,00 · 48%"));
        let p4 = row("RP4");
        assert_eq!(p4.meta_left, "sem meta");
        assert_eq!(p4.meta_right, None);
        assert_eq!(p4.progress, None);
        assert_eq!(pace_label(&p4.pace), None);
        let p6 = row("RP6");
        assert_eq!(p6.meta_left, "até nov 2026");
        assert_eq!(p6.progress, Some(1.0));
        assert_eq!(p6.meta_right.as_deref(), Some("de R$ 500,00 · 100%"));
        assert_eq!(pace_label(&p6.pace).as_deref(), Some("Meta alcançada"));
    }

    #[test]
    fn caixinha_so_com_meta_ou_so_com_prazo() {
        let mut ds = fixtures_with_reserves();
        let pot = |goal: serde_json::Value, due: serde_json::Value| {
            json!({ "kind": "goal", "name": "X", "icon": "tag", "color": "teal",
                "targetMinor": goal, "deadline": due })
        };
        add_reserve(&mut ds, "RP7", pot(json!(30_000), json!(null)));
        add_reserve(&mut ds, "RP8", pot(json!(null), json!("2027-03")));
        let only_goal = pot_row(&ds, &ds.reserves["RP7"], NOW, None);
        assert_eq!(only_goal.meta_left, "faltam R$ 300");
        assert_eq!(only_goal.meta_right.as_deref(), Some("de R$ 300,00 · 0%"));
        let only_due = pot_row(&ds, &ds.reserves["RP8"], NOW, None);
        assert_eq!(only_due.meta_left, "até mar 2027");
        assert_eq!(only_due.meta_right, None);
        assert_eq!(only_due.progress, None);
    }

    #[test]
    fn emergencia_extra_como_caixinha() {
        let mut ds = fixtures_with_reserves();
        add_reserve(
            &mut ds,
            "RE9",
            json!({ "kind": "emergency", "name": "Outra", "icon": "x", "color": "slate",
                "essentialCategoryIds": ["C1", "C2"], "deadline": "2027-01", "targetMinor": 1 }),
        );
        let r = &ds.reserves["RE9"];
        let goal = emergency_goal(&ds, r, NOW);
        assert_eq!(goal, 390_000);
        let p = pot_row(&ds, r, NOW, Some(goal));
        assert_eq!(p.goal_minor, Some(390_000));
        assert_eq!(p.icon, "lifebuoy");
        assert_eq!(p.color, ReserveColor::Accent);
        assert_eq!(p.due_month, None);
        assert_eq!(p.meta_left, "faltam R$ 3.900");
    }

    fn informed_json(
        cats: serde_json::Value,
        multiple: serde_json::Value,
        o: i64,
    ) -> serde_json::Value {
        json!({ "kind": "emergency", "name": "Reserva", "icon": "lifebuoy", "color": "violet",
            "targetMinor": null, "multiple": multiple, "essentialCategoryIds": cats,
            "essentialOverrideMinor": o, "deadline": null, "recurring": null })
    }

    #[test]
    fn emergencia_com_custo_informado_sem_lancamentos() {
        let mut ds = Dataset::default();
        add_reserve(&mut ds, "RE1", informed_json(json!([]), json!(3), 250_000));
        let e = emergency_view(&ds, &ds.reserves["RE1"], NOW);
        assert_eq!(
            e.cost,
            Some(EssentialCost {
                total_minor: 250_000,
                months: 0,
                parts: vec![],
                informed: true
            })
        );
        assert_eq!(e.goal_minor, 750_000);
        assert_eq!(e.pct_label, "0% da meta");
        assert!(e.remaining_label.starts_with("Faltam"));
        assert_eq!(emergency_goal(&ds, &ds.reserves["RE1"], NOW), 750_000);

        add_reserve(
            &mut ds,
            "RE1",
            informed_json(json!([]), json!(null), 250_000),
        );
        assert_eq!(emergency_goal(&ds, &ds.reserves["RE1"], NOW), 1_500_000);
    }

    #[test]
    fn custo_informado_vence_o_calculado() {
        let mut ds = fixtures_with_reserves();
        add_reserve(
            &mut ds,
            "RE1",
            informed_json(json!(["C1", "C2"]), json!(6), 100_000),
        );
        let e = emergency_view(&ds, &ds.reserves["RE1"], NOW);
        assert_eq!(e.goal_minor, 600_000);
        assert!(e.cost.is_some_and(|c| c.informed));
        assert_eq!(emergency_goal(&ds, &ds.reserves["RE1"], NOW), 600_000);

        add_reserve(
            &mut ds,
            "RE9",
            informed_json(json!(["C1", "C2"]), json!(6), 100_000),
        );
        let r = &ds.reserves["RE9"];
        let p = pot_row(&ds, r, NOW, Some(emergency_goal(&ds, r, NOW)));
        assert_eq!(p.goal_minor, Some(600_000));
    }

    #[test]
    fn custo_informado_zero_ou_negativo_e_sem_meta() {
        for o in [0, -500] {
            let mut ds = fixtures_with_reserves();
            add_reserve(
                &mut ds,
                "RE1",
                informed_json(json!(["C1", "C2"]), json!(6), o),
            );
            let e = emergency_view(&ds, &ds.reserves["RE1"], NOW);
            assert_eq!(e.cost, None);
            assert_eq!(e.goal_minor, 0);
            assert_eq!(e.pct_label, "Sem meta");
            assert_eq!(e.eta, EmergencyEta::NoGoal);
            assert_eq!(emergency_goal(&ds, &ds.reserves["RE1"], NOW), 0);
        }
    }

    #[test]
    fn nota_das_caixinhas() {
        let ds = fixtures_with_reserves();
        let (_, rest) = ordered(&ds);
        let rows: Vec<PotRow> = rest.iter().map(|r| pot_row(&ds, r, NOW, None)).collect();
        assert_eq!(pots_note(&rows), "4 com objetivo · 1 sem meta");
        let with_goal: Vec<PotRow> = rows
            .iter()
            .filter(|p| p.goal_minor.is_some())
            .take(3)
            .cloned()
            .collect();
        assert_eq!(pots_note(&with_goal), "3 com objetivo");
        assert_eq!(pots_note(&[]), "0 com objetivo");
    }

    #[test]
    fn lista_de_movimentacoes() {
        let ds = fixtures_with_reserves();
        let (rows, total) = movements(&ds, &months());
        assert_eq!(total, 15);
        let got: Vec<&str> = rows.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(
            got,
            vec!["M12", "M15", "M11", "M04", "M14", "M05", "M03", "M10"]
        );
        let m12 = &rows[0];
        assert_eq!(m12.kind, MovementKind::Withdrawal);
        assert_eq!(m12.reserve_name, "Viagem de julho");
        assert_eq!(m12.reserve_color, ReserveColor::Token("sky".into()));
        assert!(rows[3].recurring);
        assert!(!rows[5].recurring);
        assert_eq!(
            rows[6].author.as_ref().map(|a| a.name.as_str()),
            Some("Ana")
        );

        let early = vec!["2026-05".to_string(), "2026-06".to_string()];
        let (rows, total) = movements(&ds, &early);
        assert_eq!(total, 2);
        let m16 = rows.iter().find(|m| m.id == "M16").unwrap();
        assert_eq!(m16.author, None);
        assert_eq!(m16.description, "Guardado");
    }

    #[test]
    fn retirada_sem_descricao() {
        let mut ds = fixtures_with_reserves();
        add_movement(
            &mut ds,
            "MZ",
            json!({ "reserveId": "RP1", "amountMinor": -100, "occurredOn": "2026-09-23" }),
        );
        let (rows, _) = movements(&ds, &months());
        assert_eq!(rows[0].id, "MZ");
        assert_eq!(rows[0].description, "Retirado");
    }
}
