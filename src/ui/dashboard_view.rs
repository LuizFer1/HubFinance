//! Tela Dashboard: resumo do mes, rosca por categoria, receita x despesa de seis meses e os
//! ultimos lancamentos; ou o estado "Sem dados".
//!
//! Tudo vem de `app.dashboard` (calculado em `App::rebuild`); aqui so se monta a arvore. Os
//! textos seguem o `<script>` do prototipo (`balDelta`, `incNote`, `expNote`, `avgLine`).

use ::time::OffsetDateTime;
use iced::widget::text::Wrapping;
use iced::widget::{Space, button, canvas, column, container, mouse_area, row, text};
use iced::{Alignment, Border, Color, Element, Length, Padding, Theme};

use super::app::{App, Message, Screen};
use super::charts::{
    BarsProgram, DonutProgram, EMPTY_HEIGHT, EMPTY_WIDTH, EmptyChartProgram, categories_label,
    percent,
};
use super::theme::{self, Tokens, alpha};
use super::{fonts, icons, shell, time, widgets};
use crate::dashboard::contract::Kind;
use crate::dashboard::list::{ListRow, RowAuthor};
use crate::dashboard::money::{format_brl, group_thousands, money_parts, signed_brl_plus};
use crate::dashboard::periods::{month_long, month_short, shift_month, short_date};
use crate::dashboard::view::DashboardView;

const GAP: f32 = 14.0;
/// `minmax(240px, 1fr)` dos cards de resumo e `minmax(440px, 1fr)` dos graficos.
const SUMMARY_MIN: f32 = 240.0;
const CHART_MIN: f32 = 440.0;
const DONUT_SIZE: f32 = 184.0;
const BARS_HEIGHT: f32 = 210.0;

pub fn view(app: &App) -> Element<'_, Message> {
    let v = &app.dashboard;
    let mut col = column![summary_row(app)].spacing(GAP);
    if v.is_empty {
        col = col.push(empty_card(app));
    } else {
        col = col.push(charts_row(app)).push(recent_card(app));
    }
    if let Some(footer) = footer(app) {
        col = col.push(footer);
    }
    col.width(Length::Fill).into()
}

/// Card de superficie com o padding do resumo.
fn card<'a>(t: &'static Tokens, content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content)
        .padding([18, 20])
        .width(Length::Fill)
        .style(theme::card(t))
        .into()
}

// ---- resumo ----

/// Tres cards de largura igual; abaixo de 3 x 240 + gaps, dois por linha e o terceiro embaixo
/// (o `auto-fit` do prototipo).
fn summary_row(app: &App) -> Element<'_, Message> {
    widgets::three_up(
        shell::content_width(app),
        SUMMARY_MIN,
        GAP,
        [balance_card(app), income_card(app), expense_card(app)],
    )
}

/// Variacao do saldo contra o mes anterior.
#[derive(Debug, PartialEq, Eq)]
enum BalanceDelta {
    /// Hub sem dados.
    Empty,
    /// Nenhum lancamento antes deste mes.
    First,
    Change {
        text: String,
        up: bool,
    },
}

fn balance_delta(v: &DashboardView) -> BalanceDelta {
    if v.is_empty {
        return BalanceDelta::Empty;
    }
    if !v.summary.had_previous {
        return BalanceDelta::First;
    }
    let delta = v.summary.delta_minor;
    let up = delta >= 0;
    BalanceDelta::Change {
        text: format!(
            "{} {} que {}",
            format_brl(delta),
            if up { "a mais" } else { "a menos" },
            month_long(&shift_month(&v.month, -1))
        ),
        up,
    }
}

/// "12 entradas · +8% vs. ago"; sem base para o %, so a contagem.
fn change_note(count: usize, pct: Option<i32>, month: &str, one: &str, many: &str) -> String {
    let counted = format!("{count} {}", if count == 1 { one } else { many });
    match pct {
        Some(p) => format!(
            "{counted} · {}{p}% vs. {}",
            if p >= 0 { "+" } else { "" },
            month_short(&shift_month(month, -1))
        ),
        None => counted,
    }
}

/// Barra dividida: `FillPortion` e `u16`, entao os valores viram milesimos do total (minimo 1,
/// para a parte zerada continuar visivel como no `flex: 0 || 1` do prototipo).
fn split_portions(income: i64, expense: i64) -> (u16, u16) {
    // `i128`: a soma de dois `i64` grandes nao satura (saturar daria 1000 para cada lado).
    let (inc, exp) = (i128::from(income.max(0)), i128::from(expense.max(0)));
    let total = inc + exp;
    if total == 0 {
        return (1, 1);
    }
    let part = |v: i128| (v * 1000 / total).max(1) as u16;
    (part(inc), part(exp))
}

/// O que a barra dividida ocupa no card do saldo (8 de margem + 6 de barra). Os outros dois
/// cards ganham um espaco igual no fim: a grade do prototipo estica os tres para a mesma
/// altura, e o iced nao estica filho de linha dentro de `scrollable`.
const SPLIT_BAR_SPACE: f32 = 14.0;

fn balance_card(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let v = &app.dashboard;
    let current = &v.summary.current;
    let parts = money_parts(current.balance_minor);
    let small = t.text_alpha(0.55);
    let value = widgets::money_big(
        &format!("{}R$", parts.sign),
        &parts.whole,
        &parts.cents,
        t.text,
        small,
    );
    let delta: Element<'_, Message> = match balance_delta(v) {
        BalanceDelta::Empty => text("Sem dados").size(12).color(t.text_alpha(0.60)).into(),
        BalanceDelta::First => text("Primeiro mês")
            .size(12)
            .color(t.text_alpha(0.60))
            .into(),
        BalanceDelta::Change { text: label, up } => {
            let (glyph, color) = if up {
                (icons::TREND_UP, t.income_fg)
            } else {
                (icons::TREND_DOWN, t.expense_fg)
            };
            row![
                icons::icon(glyph, 12.0, color),
                text(label).size(12).color(color)
            ]
            .spacing(5)
            .align_y(Alignment::Center)
            .into()
        }
    };
    let (inc, exp) = split_portions(current.income_minor, current.expense_minor);
    let bar = |portion: u16, color: Color| {
        container(Space::new())
            .width(Length::FillPortion(portion))
            .height(6)
            .style(theme::fill(color, 3.0))
    };
    let split =
        container(row![bar(inc, t.income), bar(exp, t.expense)].spacing(3)).padding(Padding {
            top: SPLIT_BAR_SPACE - 6.0,
            ..Padding::ZERO
        });
    card(
        t,
        column![widgets::kicker(t, "Saldo do mês"), value, delta, split].spacing(6),
    )
}

pub(super) fn kicker_with_icon<'a>(
    t: &'static Tokens,
    glyph: &'static str,
    color: Color,
    label: &str,
) -> Element<'a, Message> {
    row![icons::icon(glyph, 13.0, color), widgets::kicker(t, label)]
        .spacing(6)
        .align_y(Alignment::Center)
        .into()
}

fn income_card(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let v = &app.dashboard;
    let current = &v.summary.current;
    let parts = money_parts(current.income_minor);
    let note = if v.is_empty {
        "Sem dados".to_string()
    } else {
        change_note(
            current.income_count,
            v.summary.income_change_pct,
            &v.month,
            "entrada",
            "entradas",
        )
    };
    card(
        t,
        column![
            kicker_with_icon(t, icons::ARROW_DOWN_LEFT, t.income_fg, "Receitas"),
            widgets::money_big("+R$", &parts.whole, &parts.cents, t.income_fg, t.income_fg),
            text(note).size(12).color(t.text_alpha(0.60)),
            Space::new().height(SPLIT_BAR_SPACE),
        ]
        .spacing(6),
    )
}

fn expense_card(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let v = &app.dashboard;
    let current = &v.summary.current;
    let parts = money_parts(current.expense_minor);
    let pct = v.summary.expense_change_pct;
    let note = if v.is_empty {
        "Sem dados".to_string()
    } else {
        change_note(current.expense_count, pct, &v.month, "saída", "saídas")
    };
    // Despesa subindo e a unica nota em cor: e o aviso que importa.
    let note_color = if !v.is_empty && pct.is_some_and(|p| p > 0) {
        t.expense_fg
    } else {
        t.text_alpha(0.60)
    };
    card(
        t,
        column![
            kicker_with_icon(t, icons::ARROW_UP_RIGHT, t.expense_fg, "Despesas"),
            widgets::money_big(
                "\u{2212}R$",
                &parts.whole,
                &parts.cents,
                t.text,
                t.text_alpha(0.55)
            ),
            text(note).size(12).color(note_color),
            Space::new().height(SPLIT_BAR_SPACE),
        ]
        .spacing(6),
    )
}

// ---- graficos ----

/// Dois cards lado a lado (minimo 440 cada); abaixo disso, empilhados.
///
/// Lado a lado, os dois cards tem a mesma altura (o `stretch` da grade do prototipo). O iced
/// nao estica filho de linha dentro de `scrollable` (altura `Fill` sem limite vira zero), entao
/// a altura do corpo de cada card e calculada: a maior das duas, conhecidas de antemao.
fn charts_row(app: &App) -> Element<'_, Message> {
    let width = shell::content_width(app);
    if width >= 2.0 * CHART_MIN + GAP {
        let (donut_body, bars_body) = chart_bodies(app.dashboard.donut.len());
        row![
            donut_card(app, Some(donut_body)),
            bars_card(app, Some(bars_body))
        ]
        .spacing(GAP)
        .into()
    } else {
        column![donut_card(app, None), bars_card(app, None)]
            .spacing(GAP)
            .into()
    }
}

/// Linha da legenda da rosca.
const LEGEND_ROW: f32 = 30.0;
/// Corpo do card das barras: canvas + 14 + linha da media (13 px, altura 1,3).
const BARS_BODY: f32 = BARS_HEIGHT + 14.0 + 13.0 * 1.3;
/// Espaco entre o titulo e o corpo: 16 na rosca, 18 nas barras (prototipo).
const DONUT_TITLE_GAP: f32 = 16.0;
const BARS_TITLE_GAP: f32 = 18.0;

/// Alturas dos corpos (rosca, barras) para os dois cards ficarem iguais.
fn chart_bodies(slices: usize) -> (f32, f32) {
    let donut = DONUT_SIZE.max(LEGEND_ROW * slices as f32);
    let total = (donut + DONUT_TITLE_GAP).max(BARS_BODY + BARS_TITLE_GAP);
    (total - DONUT_TITLE_GAP, total - BARS_TITLE_GAP)
}

fn chart_card<'a>(
    t: &'static Tokens,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    container(content)
        .padding(Padding {
            top: 18.0,
            right: 20.0,
            bottom: 20.0,
            left: 20.0,
        })
        .width(Length::Fill)
        .style(theme::card(t))
        .into()
}

/// Corpo com altura fixa (lado a lado) ou natural (empilhado), alinhado ao topo.
fn body_with_height<'a>(
    body: impl Into<Element<'a, Message>>,
    height: Option<f32>,
) -> Element<'a, Message> {
    match height {
        Some(h) => container(body).height(h).align_y(Alignment::Start).into(),
        None => body.into(),
    }
}

fn donut_card(app: &App, body_height: Option<f32>) -> Element<'_, Message> {
    let t = app.tokens;
    let v = &app.dashboard;
    let title = widgets::card_title(
        t,
        "Gastos por categoria",
        Some(categories_label(v.donut.len())),
    );
    let body: Element<'_, Message> = if v.donut.is_empty() {
        text(format!("Nenhuma despesa em {}.", month_long(&v.month)))
            .size(13)
            .color(t.text_alpha(0.55))
            .into()
    } else {
        let chart = canvas(DonutProgram {
            slices: &v.donut,
            total_minor: v.summary.current.expense_minor,
            hover: app.hover_category.as_deref(),
            tokens: t,
            cache: &app.donut_cache,
        })
        .width(DONUT_SIZE)
        .height(DONUT_SIZE);
        // 20 e nao 28: as linhas da legenda tem 8 px de padding (o hover passa do texto, como o
        // `margin: 0 -8px` do prototipo) e o texto fica onde o design o poe.
        row![chart, legend(app)]
            .spacing(20)
            .align_y(Alignment::Center)
            .into()
    };
    chart_card(
        t,
        column![
            title,
            Space::new().height(DONUT_TITLE_GAP),
            body_with_height(body, body_height)
        ],
    )
}

fn legend(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let v = &app.dashboard;
    let hover = app.hover_category.as_deref();
    let mut col = column![].width(Length::Fill);
    for slice in &v.donut {
        let active = hover == Some(slice.key.as_str());
        let faded = hover.is_some() && !active;
        let fade = |c: Color| if faded { alpha(c, c.a * 0.28) } else { c };
        let line = row![
            widgets::dot(fade(t.token(&slice.color)), 8.0),
            container(
                text(slice.name.clone())
                    .size(13)
                    .wrapping(Wrapping::None)
                    .color(fade(t.text))
            )
            .width(Length::Fill)
            .clip(true),
            text(format_brl(slice.amount_minor))
                .size(13)
                .color(fade(t.text)),
            container(
                text(format!("{}%", percent(slice.share)))
                    .size(12)
                    .color(fade(t.text_alpha(0.55)))
            )
            .width(38)
            .align_x(Alignment::End),
        ]
        .spacing(10)
        .align_y(Alignment::Center);
        let body = container(line)
            .height(30)
            .padding([0, 8])
            .align_y(Alignment::Center)
            .style(move |_: &Theme| container::Style {
                background: active.then(|| t.text_alpha(0.05).into()),
                border: Border {
                    radius: 6.0.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            });
        col = col.push(
            mouse_area(body)
                .on_enter(Message::HoverCategory(slice.key.clone()))
                .on_exit(Message::UnhoverCategory(slice.key.clone())),
        );
    }
    container(col).width(Length::Fill).into()
}

fn bars_card(app: &App, body_height: Option<f32>) -> Element<'_, Message> {
    let t = app.tokens;
    let v = &app.dashboard;
    let muted = t.text_alpha(0.60);
    let square = |color: Color| {
        container(Space::new())
            .width(8)
            .height(8)
            .style(theme::fill(color, 2.0))
    };
    let key = |color: Color, label: &'static str| {
        row![square(color), text(label).size(12).color(muted)]
            .spacing(6)
            .align_y(Alignment::Center)
    };
    let header = row![
        text("Receita × despesa")
            .size(15)
            .font(fonts::INTER_MEDIUM)
            .color(t.text)
            .width(Length::Fill),
        row![
            key(t.income, "Receitas"),
            key(t.expense, "Despesas"),
            text("últimos 6 meses").size(12).color(muted),
        ]
        .spacing(14)
        .align_y(Alignment::Center),
    ]
    .spacing(16)
    .align_y(Alignment::Center);
    let quiet = v
        .bars
        .iter()
        .all(|b| b.income_minor == 0 && b.expense_minor == 0);
    let body: Element<'_, Message> = if quiet {
        text("Sem movimento nos últimos seis meses.")
            .size(13)
            .color(t.text_alpha(0.55))
            .into()
    } else {
        let chart = canvas(BarsProgram {
            bars: &v.bars,
            axis_top: v.axis_top,
            selected: v.bars.len().saturating_sub(1),
            tokens: t,
            cache: &app.bars_cache,
        })
        .width(Length::Fill)
        .height(BARS_HEIGHT);
        column![
            chart,
            Space::new().height(14),
            text(average_line(v))
                .size(13)
                .line_height(1.3)
                .color(t.text_alpha(0.65)),
        ]
        .into()
    };
    chart_card(
        t,
        column![
            header,
            Space::new().height(BARS_TITLE_GAP),
            body_with_height(body, body_height)
        ],
    )
}

/// "Sobrou em média R$ 2.413,00 por mês entre abr e set." ou "Faltou em média …".
fn average_line(v: &DashboardView) -> String {
    let avg = v.average_surplus;
    format!(
        "{} em média {} por mês {}.",
        if avg >= 0 { "Sobrou" } else { "Faltou" },
        format_brl(avg),
        v.bars_range_label
    )
}

// ---- ultimos lancamentos ----

/// Colunas `2.2fr 1.2fr 1fr 128px` do prototipo.
const RECENT_COLUMNS: [u16; 3] = [22, 12, 10];
const VALUE_WIDTH: f32 = 128.0;

fn recent_card(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let v = &app.dashboard;
    let see_all = button(
        row![
            text("Ver todos").size(13).font(fonts::INTER_MEDIUM),
            icons::icon_inherit(icons::ARROW_RIGHT, 13.0),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding(0)
    .style(theme::link(t))
    .on_press(Message::SelectScreen(Screen::Transactions));
    let title = container(
        row![
            text("Últimos lançamentos")
                .size(15)
                .font(fonts::INTER_MEDIUM)
                .color(t.text)
                .width(Length::Fill),
            see_all,
        ]
        .align_y(Alignment::Center),
    )
    .padding([0, 8]);
    let mut col = column![title, Space::new().height(8)];
    if v.recent.is_empty() {
        col = col.push(widgets::hdivider(t)).push(
            container(
                text(format!("Nenhum lançamento em {}.", month_long(&v.month)))
                    .size(13)
                    .color(t.text_alpha(0.55)),
            )
            .height(52)
            .padding([0, 8])
            .align_y(Alignment::Center),
        );
    }
    for r in &v.recent {
        col = col.push(widgets::hdivider(t)).push(recent_row(app, r));
    }
    // Padding lateral de 12 (e nao 20) para o hover da linha passar 8 px do conteudo, como o
    // `margin: 0 -8px` do prototipo.
    container(col)
        .padding(Padding {
            top: 18.0,
            right: 12.0,
            bottom: 8.0,
            left: 12.0,
        })
        .width(Length::Fill)
        .style(theme::card(t))
        .into()
}

/// Tile da categoria; sem categoria viva, o tile neutro com `circle-dashed`.
pub(super) fn category_tile<'a>(t: &'static Tokens, r: &ListRow) -> Element<'a, Message> {
    match (&r.category_icon, &r.category_color) {
        (Some(icon), Some(color)) => widgets::tile(t, icons::glyph(icon), color),
        _ => widgets::tile(t, icons::CIRCLE_DASHED, "slate"),
    }
}

/// Celula de autor de uma linha de lancamento.
pub(super) fn author_cell<'a>(app: &'a App, r: &'a ListRow) -> Element<'a, Message> {
    author_cell_of(app, r.author.as_ref())
}

/// Avatar de 22 + nome; sem autor visivel, `circle-dashed` em `neutral_600` e "—". Serve as
/// linhas de lancamento e as de movimentacao de reserva.
pub(super) fn author_cell_of<'a>(
    app: &'a App,
    author: Option<&'a RowAuthor>,
) -> Element<'a, Message> {
    let t = app.tokens;
    let (avatar, name): (Element<'a, Message>, String) = match author {
        Some(author) => (
            widgets::avatar(
                t,
                &author.name,
                Some(&author.color),
                app.user_avatars.get(&author.id).map(|a| &a.handle),
                22.0,
            ),
            author.name.clone(),
        ),
        None => (
            container(icons::icon(icons::CIRCLE_DASHED, 22.0, t.neutral_600))
                .center(22)
                .into(),
            "—".to_string(),
        ),
    };
    row![
        avatar,
        container(text(name).size(13).wrapping(Wrapping::None).color(t.text))
            .width(Length::Fill)
            .clip(true),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}

/// Valor da linha: receita com `+` na cor de receita; despesa com `−` na cor do texto (so a
/// receita ganha cor, como pede o README).
pub(super) fn value_cell<'a>(t: &'static Tokens, r: &ListRow) -> Element<'a, Message> {
    let (label, color) = match r.kind {
        Kind::Income => (signed_brl_plus(r.amount_minor.max(0)), t.income_fg),
        Kind::Expense => (format!("\u{2212}{}", format_brl(r.amount_minor)), t.text),
    };
    container(
        text(label)
            .size(14)
            .font(fonts::INTER_MEDIUM)
            .wrapping(Wrapping::None)
            .color(color),
    )
    .width(VALUE_WIDTH)
    .align_x(Alignment::End)
    .into()
}

/// Linha com hover de 4 % (o mesmo `hover_row` da tela Conexao: so uma tela por vez). Altura
/// por parametro: 52 nos lancamentos, 56 nas movimentacoes de reserva.
pub(super) fn hover_row<'a>(
    app: &'a App,
    id: &str,
    content: impl Into<Element<'a, Message>>,
    padding: [u16; 2],
    radius: f32,
    height: f32,
) -> Element<'a, Message> {
    let t = app.tokens;
    let hovered = app.hover_row.as_deref() == Some(id);
    let body = container(content)
        .height(height)
        .padding(padding)
        .width(Length::Fill)
        .align_y(Alignment::Center)
        .style(move |_: &Theme| container::Style {
            background: hovered.then(|| t.text_alpha(0.04).into()),
            border: Border {
                radius: radius.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    mouse_area(body)
        .on_enter(Message::HoverRow(id.to_string()))
        .on_exit(Message::UnhoverRow(id.to_string()))
        .into()
}

fn recent_row<'a>(app: &'a App, r: &'a ListRow) -> Element<'a, Message> {
    let t = app.tokens;
    let [desc_w, cat_w, author_w] = RECENT_COLUMNS;
    let description = row![
        category_tile(t, r),
        column![
            text(r.description.clone())
                .size(14)
                .font(fonts::INTER_MEDIUM)
                .wrapping(Wrapping::None)
                .color(t.text),
            text(short_date(&r.occurred_on))
                .size(12)
                .color(t.text_alpha(0.55)),
        ],
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    let content = row![
        container(description)
            .width(Length::FillPortion(desc_w))
            .clip(true),
        container(
            text(r.category_name.clone())
                .size(13)
                .wrapping(Wrapping::None)
                .color(t.text_alpha(0.75))
        )
        .width(Length::FillPortion(cat_w))
        .clip(true),
        container(author_cell(app, r)).width(Length::FillPortion(author_w)),
        value_cell(t, r),
    ]
    .spacing(16)
    .align_y(Alignment::Center);
    hover_row(app, &r.id, content, [0, 8], 8.0, 52.0)
}

// ---- sem dados ----

fn empty_card(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let snap = &app.snapshot;
    let waiting = match snap.addresses.first() {
        Some(ip) => format!("Aguardando o primeiro sync em {ip}:{}", snap.https_port),
        None => "Aguardando o primeiro sync (sem rede local)".to_string(),
    };
    let sketch = canvas(EmptyChartProgram {
        tokens: t,
        cache: &app.empty_cache,
    })
    .width(EMPTY_WIDTH)
    .height(EMPTY_HEIGHT);
    let copy = column![
        text("Nenhum dado por aqui ainda")
            .size(20)
            .font(fonts::INTER_MEDIUM)
            .color(t.text),
        Space::new().height(8),
        text(
            "Assim que um celular pareado sincronizar, o dashboard se monta sozinho. Os dados \
             viajam só pela rede de casa."
        )
        .size(14)
        .line_height(1.55)
        .color(t.text_alpha(0.65)),
        Space::new().height(16),
        row![
            widgets::dot(t.amber, 7.0),
            text(waiting).size(12).color(t.text_alpha(0.60)),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    ]
    .max_width(460);
    container(row![sketch, copy].spacing(56).align_y(Alignment::Center))
        .padding([56, 32])
        .width(Length::Fill)
        .style(theme::card(t))
        .into()
}

// ---- rodape ----

/// "Atualizado às 18:42 · 44 lançamentos · 1 linha ignorada (fora do contrato de campos)".
fn footer_text(updated_at: Option<&str>, count: usize, ignored: usize) -> String {
    let mut parts = Vec::new();
    if let Some(at) = updated_at {
        parts.push(format!("Atualizado às {at}"));
    }
    parts.push(format!(
        "{} {}",
        group_thousands(count as u64),
        if count == 1 {
            "lançamento"
        } else {
            "lançamentos"
        }
    ));
    let mut line = parts.join(" · ");
    if ignored > 0 {
        line.push_str(&format!(
            " · {ignored} {} (fora do contrato de campos)",
            if ignored == 1 {
                "linha ignorada"
            } else {
                "linhas ignoradas"
            }
        ));
    }
    line
}

/// Rodape discreto; no estado vazio so aparece se houver linha ignorada (e o unico jeito de
/// saber que algo chegou e nao entrou).
fn footer(app: &App) -> Option<Element<'_, Message>> {
    let v = &app.dashboard;
    let dataset = &app.snapshot.dashboard;
    if v.is_empty && v.ignored_total == 0 {
        return None;
    }
    let updated = dataset
        .refreshed_at
        .map(|at| time::format_time(OffsetDateTime::from(at), app.offset));
    Some(
        container(
            text(footer_text(
                updated.as_deref(),
                shell::transactions_count(app),
                v.ignored_total,
            ))
            .size(12)
            .color(app.tokens.text_alpha(0.50)),
        )
        .padding([0, 2])
        .into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dashboard::fixtures;
    use crate::dashboard::view::build_dashboard;

    #[test]
    fn variacao_do_saldo() {
        let ds = fixtures();
        let sep = build_dashboard(&ds, "2026-09-24", "2026-09");
        assert_eq!(
            balance_delta(&sep),
            BalanceDelta::Change {
                text: "R$ 210,00 a menos que agosto".into(),
                up: false
            }
        );
        let aug = build_dashboard(&ds, "2026-09-24", "2026-08");
        assert_eq!(
            balance_delta(&aug),
            BalanceDelta::Change {
                text: "R$ 300,00 a menos que julho".into(),
                up: false
            }
        );
        // Julho contra junho vazio: sobe o saldo inteiro.
        let jul = build_dashboard(&ds, "2026-09-24", "2026-07");
        assert_eq!(
            balance_delta(&jul),
            BalanceDelta::Change {
                text: "R$ 4.700,00 a mais que junho".into(),
                up: true
            }
        );
        let mar = build_dashboard(&ds, "2026-09-24", "2026-03");
        assert_eq!(balance_delta(&mar), BalanceDelta::First);
        let empty = build_dashboard(&Default::default(), "2026-09-24", "2026-09");
        assert_eq!(balance_delta(&empty), BalanceDelta::Empty);
    }

    #[test]
    fn notas_de_receita_e_despesa() {
        assert_eq!(
            change_note(12, Some(8), "2026-09", "entrada", "entradas"),
            "12 entradas · +8% vs. ago"
        );
        assert_eq!(
            change_note(1, Some(-5), "2026-01", "saída", "saídas"),
            "1 saída · -5% vs. dez"
        );
        assert_eq!(
            change_note(0, Some(0), "2026-09", "entrada", "entradas"),
            "0 entradas · +0% vs. ago"
        );
        assert_eq!(
            change_note(3, None, "2026-09", "entrada", "entradas"),
            "3 entradas"
        );
    }

    #[test]
    fn barra_dividida_cabe_em_u16() {
        assert_eq!(split_portions(0, 0), (1, 1));
        assert_eq!(split_portions(650_000, 231_000), (737, 262));
        assert_eq!(split_portions(100, 0), (1000, 1));
        assert_eq!(split_portions(-100, 50), (1, 1000));
        assert_eq!(split_portions(i64::MAX, i64::MAX), (500, 500));
        assert_eq!(split_portions(1, 1_000_000_000), (1, 999));
    }

    #[test]
    fn linha_da_media() {
        let ds = fixtures();
        let sep = build_dashboard(&ds, "2026-09-24", "2026-09");
        assert_eq!(
            average_line(&sep),
            "Sobrou em média R$ 2.215,00 por mês entre abr e set."
        );
        let mut negative = sep.clone();
        negative.average_surplus = -1234;
        assert_eq!(
            average_line(&negative),
            "Faltou em média R$ 12,34 por mês entre abr e set."
        );
    }

    #[test]
    fn cards_dos_graficos_com_a_mesma_altura() {
        let (donut, bars) = chart_bodies(4);
        assert!((donut + DONUT_TITLE_GAP - (bars + BARS_TITLE_GAP)).abs() < 1e-3);
        assert!(
            (bars - BARS_BODY).abs() < 1e-3,
            "barras mandam com poucas fatias"
        );
        // Doze categorias: a legenda manda e as barras ganham espaco embaixo.
        let (donut, bars) = chart_bodies(12);
        assert!((donut - 360.0).abs() < 1e-3);
        assert!((donut + DONUT_TITLE_GAP - (bars + BARS_TITLE_GAP)).abs() < 1e-3);
    }

    #[test]
    fn rodape() {
        assert_eq!(
            footer_text(Some("18:42"), 44, 0),
            "Atualizado às 18:42 · 44 lançamentos"
        );
        assert_eq!(
            footer_text(Some("18:42"), 1, 1),
            "Atualizado às 18:42 · 1 lançamento · 1 linha ignorada (fora do contrato de campos)"
        );
        assert_eq!(
            footer_text(None, 1234, 3),
            "1.234 lançamentos · 3 linhas ignoradas (fora do contrato de campos)"
        );
    }
}
