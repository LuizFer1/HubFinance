//! Dashboard (Main · "Como estou", "Negócio · PJ", "Reserva" e "Histórico"). Parte 1 da tela:
//! a linha "Para onde foi / O que vem / Recentes / Pessoas" e os estados de chegada entram na
//! Tarefa 19.
//!
//! A tela so le o `DeskView` pronto: nenhum texto de dinheiro e formatado aqui, senao o numero
//! vazaria por cima do botao "Ocultar valores".

use gpui::{
    AnyElement, App, Div, Entity, FocusHandle, Hsla, IntoElement, ParentElement as _, Role,
    SharedString, Styled as _, Window, div, prelude::*, px,
};

use crate::dashboard::desk::{
    DeskBusiness, DeskDelta, DeskEmergency, DeskHistory, DeskHowAmI, DeskMonth, DeskReserve,
    DeskTip,
};
use crate::dashboard::grafite::{LabeledValue, Tone};
use crate::dashboard::money::BigMoney;
use crate::gui::charts::bars::{BarMonth, BarTip, GroupedBars, MiniBars};
use crate::gui::components::card::Card;
use crate::gui::components::focus;
use crate::gui::components::money::Money;
use crate::gui::components::progress::Progress;
use crate::gui::components::tooltip::Line;
use crate::gui::fonts;
use crate::gui::icons::{Icon, icon};
use crate::gui::root::{HubState, dispatch_later};
use crate::gui::theme::{Tokens, rgba};
use crate::ui_model::app::{App as Model, Msg, Screen};

/// Teto do eixo das barras: o `DeskView` entrega frações (0..=1) e o canvas trabalha em
/// inteiros; mil passos dão resolução de sobra sem tocar em dinheiro de verdade.
const SCALE: i64 = 1000;

/// Colunas da grade pela largura **da janela** (não da área de conteúdo): no design os cortes
/// são `@media (max-width: …)`, ou seja, da janela inteira com a barra lateral.
pub fn columns(window_width: f32) -> u8 {
    if window_width < 760.0 {
        1
    } else if window_width < 1180.0 {
        2
    } else {
        3
    }
}

/// Quantas colunas um card largo ocupa: duas, mas nunca mais que a grade tem (um `span 2` numa
/// grade de uma coluna criaria uma segunda coluna implícita e estouraria a largura).
pub fn span(columns: u8, wanted: u8) -> u16 {
    u16::from(wanted.min(columns))
}

/// Os `FocusHandle`s dos links da tela. Guardados pela view raiz: um handle novo por quadro
/// perderia o foco a cada redesenho (mesmo motivo do `ShellFocus`).
pub struct DashboardFocus {
    pub transactions: FocusHandle,
    pub reserves: FocusHandle,
}

impl DashboardFocus {
    pub fn new<T: 'static>(cx: &mut gpui::Context<T>) -> DashboardFocus {
        DashboardFocus {
            transactions: cx.focus_handle().tab_stop(true),
            reserves: cx.focus_handle().tab_stop(true),
        }
    }
}

/// O valor grande do `DeskView` como o `Money` o entende ("−R$ 96,40").
fn big_text(m: &BigMoney) -> String {
    format!("{}{}\u{a0}{}{}", m.sign, m.currency, m.whole, m.cents)
}

fn tone_color(t: &Tokens, tone: Tone) -> Hsla {
    match tone {
        Tone::Neutral => t.muted,
        Tone::Income => t.income,
        Tone::Expense => t.expense,
        Tone::Warning => t.warning,
    }
}

pub fn render(
    t: &'static Tokens,
    app: &Model,
    state: &Entity<HubState>,
    focus: &DashboardFocus,
) -> AnyElement {
    let cols = columns(app.window_size.w);
    let desk = &app.desk;
    let wide = span(cols, 2);
    let mut grid = div()
        .grid()
        .grid_cols(u16::from(cols))
        .gap(px(20.0))
        .w_full()
        .child(how_am_i(t, &desk.how_am_i, state, &focus.transactions).col_span(wide));
    if let Some(business) = &desk.business {
        grid = grid.child(business_card(t, business));
    }
    grid.child(reserve_card(t, &desk.reserve, state, &focus.reserves))
        .child(history_card(t, &desk.history).col_span(wide))
        .into_any_element()
}

/// Cabeçalho do card: bolinha com ícone, título 15/600 e, à direita, o que vier.
fn card_header(t: &Tokens, glyph: Icon, title: &str, right: Option<AnyElement>) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(12.0))
        .min_h(px(32.0))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.0))
                .min_w_0()
                .child(
                    div()
                        .size(px(32.0))
                        .flex_none()
                        .rounded_full()
                        .bg(t.surface_2)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon(glyph, px(16.0), t.text)),
                )
                .child(
                    div()
                        .text_size(px(15.0))
                        .font_weight(fonts::weight(600))
                        .child(SharedString::from(title.to_string())),
                ),
        )
        .children(right)
}

/// Link "Ver …" do card: texto `accent` 13/600 com seta, 32 px de altura de alvo.
fn link(
    t: &'static Tokens,
    id: &'static str,
    label: &str,
    screen: Screen,
    state: &Entity<HubState>,
    handle: &FocusHandle,
) -> AnyElement {
    let state = state.clone();
    div()
        .id(id)
        .track_focus(handle)
        .flex()
        .flex_none()
        .items_center()
        .gap(px(4.0))
        .min_h(px(32.0))
        .text_size(px(13.0))
        .font_weight(fonts::weight(600))
        .text_color(t.accent)
        .cursor_pointer()
        .rounded_full()
        .focus_visible(focus::style(t, t.surface))
        .on_click(move |_, _: &mut Window, cx: &mut App| {
            dispatch_later(&state, Msg::SelectScreen(screen), cx);
        })
        .child(SharedString::from(label.to_string()))
        .child(icon(Icon::ChevronRight, px(14.0), t.accent))
        .into_any_element()
}

/// Pílula de 26 px com o texto na cor do tom e fundo da mesma cor a 16 %.
fn tone_pill(t: &Tokens, tone: Tone, text: String, lead: Option<Icon>) -> Div {
    let color = tone_color(t, tone);
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(6.0))
        .h(px(26.0))
        .px(px(10.0))
        .rounded_full()
        .bg(color.opacity(0.16))
        .text_color(color)
        .text_size(px(12.0))
        .font_weight(fonts::weight(600))
        .children(lead.map(|i| icon(i, px(12.0), color)))
        .child(SharedString::from(text))
}

fn delta_text(d: &DeskDelta) -> String {
    format!("{} {}", d.value, d.suffix)
}

/// Quadro com rótulo e valor (Receitas / Despesas / Gasto médio / Faturado).
fn tile(
    t: &'static Tokens,
    id: &'static str,
    item: &LabeledValue,
    dot: Option<Hsla>,
    bg: Hsla,
    label_color: Hsla,
    value_color: Option<Hsla>,
) -> Div {
    let mut value = Money::new(id, item.value.clone(), 20.0, t);
    if let Some(c) = value_color {
        value = value.color(c);
    }
    div()
        .min_w_0()
        .bg(bg)
        .rounded(px(16.0))
        .px(px(16.0))
        .py(px(14.0))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .text_size(px(13.0))
                .text_color(label_color)
                .children(dot.map(|c| div().size(px(8.0)).rounded_full().bg(c)))
                .child(SharedString::from(item.label.clone())),
        )
        .child(div().mt(px(8.0)).child(value))
}

fn how_am_i(
    t: &'static Tokens,
    v: &DeskHowAmI,
    state: &Entity<HubState>,
    handle: &FocusHandle,
) -> Card {
    let balance_color = if v.balance_negative {
        t.expense
    } else {
        t.text
    };
    let mut headline = div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(14.0))
        .mt(px(10.0))
        .child(Money::new("como-saldo", big_text(&v.balance), 48.0, t).color(balance_color));
    if let Some(d) = &v.delta {
        headline = headline.child(tone_pill(t, d.tone, delta_text(d), None));
    } else if let Some(first) = &v.first_month {
        headline = headline.child(
            div()
                .text_size(px(12.0))
                .text_color(t.muted)
                .child(SharedString::from(first.clone())),
        );
    }
    let mut card = Card::new(t)
        .gap(px(22.0))
        .min_w_0()
        .child(card_header(
            t,
            Icon::Pulse,
            &v.title,
            Some(link(
                t,
                "como-link",
                &v.link,
                Screen::Transactions,
                state,
                handle,
            )),
        ))
        .child(
            div()
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(t.muted)
                        .child(SharedString::from(v.balance_label.clone())),
                )
                .child(headline),
        );
    if let Some(u) = &v.usage {
        let fill = if u.over { t.danger } else { t.expense };
        card = card.child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .gap(px(12.0))
                        .text_size(px(13.0))
                        .text_color(t.muted)
                        .child(SharedString::from(u.text.clone()))
                        .child(
                            div()
                                .text_color(t.text)
                                .font_weight(fonts::weight(600))
                                .child(SharedString::from(u.percent.clone())),
                        ),
                )
                .child(Progress::new(f64::from(u.fill), 1.0, t).fill(fill)),
        );
    }
    card.child(
        div()
            .mt_auto()
            .grid()
            .grid_cols(3)
            .gap(px(12.0))
            .child(tile(
                t,
                "como-receitas",
                &v.income,
                Some(t.income),
                t.surface_2,
                t.muted,
                None,
            ))
            .child(tile(
                t,
                "como-despesas",
                &v.expense,
                Some(t.expense),
                t.surface_2,
                t.muted,
                None,
            ))
            .child(tile(
                t,
                "como-media",
                &v.daily_average,
                None,
                t.surface_2,
                t.muted,
                None,
            )),
    )
}

fn business_card(t: &'static Tokens, b: &DeskBusiness) -> AnyElement {
    let ink = t.on_highlight;
    let muted = t.on_highlight_muted;
    let glass = rgba(255, 255, 255, 0.32);
    let result_color = if b.result_negative { t.danger } else { ink };
    let mut headline = div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(10.0))
        .mt(px(10.0))
        .child(Money::new("neg-resultado", b.result.value.clone(), 30.0, t).color(result_color));
    if let Some(d) = &b.delta {
        headline = headline.child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .h(px(26.0))
                .px(px(10.0))
                .rounded_full()
                .bg(rgba(255, 255, 255, 0.45))
                .text_size(px(12.0))
                .font_weight(fonts::weight(600))
                .child(SharedString::from(delta_text(d))),
        );
    }
    let pill = div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(8.0))
        .h(px(32.0))
        .pl(px(12.0))
        .pr(px(6.0))
        .rounded_full()
        .bg(ink)
        .text_color(t.highlight)
        .text_size(px(12.0))
        .font_weight(fonts::weight(700))
        .child(SharedString::from(b.title.clone()))
        .child(
            div()
                .size(px(22.0))
                .rounded_full()
                .bg(t.highlight)
                .flex()
                .items_center()
                .justify_center()
                .child(icon(Icon::Briefcase, px(12.0), ink)),
        );
    let months: Vec<Option<i64>> = b
        .bars
        .iter()
        .map(|bar| bar.existed.then(|| bar_units(bar.value)))
        .collect();
    let partial = b.bars.iter().position(|bar| bar.partial);
    let bars_title = match &b.average {
        Some(avg) => format!("{} {}", b.bars_title, avg),
        None => b.bars_title.clone(),
    };
    let labels = b.bars.iter().map(|bar| {
        div()
            .flex_1()
            .flex()
            .justify_center()
            .text_size(px(12.0))
            .text_color(muted)
            .child(SharedString::from(bar.label.clone()))
    });
    let mut card = Card::highlight(t)
        .gap(px(18.0))
        .min_w_0()
        .child(div().flex().child(pill))
        .child(
            div()
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(fonts::weight(500))
                        .text_color(muted)
                        .child(SharedString::from(b.result.label.clone())),
                )
                .child(headline),
        )
        .child(
            div()
                .grid()
                .grid_cols(2)
                .gap(px(10.0))
                .child(tile(
                    t,
                    "neg-faturado",
                    &b.income,
                    None,
                    glass,
                    muted,
                    Some(ink),
                ))
                .child(tile(
                    t,
                    "neg-despesas",
                    &b.expense,
                    None,
                    glass,
                    muted,
                    Some(ink),
                )),
        )
        .child(
            div()
                .child(
                    div()
                        .mb(px(8.0))
                        .text_size(px(12.0))
                        .text_color(muted)
                        .child(SharedString::from(bars_title)),
                )
                .child(
                    MiniBars::new("neg-barras", b.bars_label.clone(), months, SCALE, 34.0, t)
                        .partial(partial),
                )
                .child(div().mt(px(4.0)).flex().children(labels)),
        );
    if let Some(text) = &b.pro_labore {
        card = card.child(
            div()
                .text_size(px(12.0))
                .text_color(muted)
                .child(SharedString::from(text.clone())),
        );
    }
    card.into_any_element()
}

/// Fração 0..=1 do `DeskView` para a escala inteira do canvas.
fn bar_units(fraction: f32) -> i64 {
    (f64::from(fraction.clamp(0.0, 1.0)) * SCALE as f64).round() as i64
}

fn reserve_card(
    t: &'static Tokens,
    r: &DeskReserve,
    state: &Entity<HubState>,
    handle: &FocusHandle,
) -> AnyElement {
    let pill = r.emergency.as_ref().map(|e| {
        let (tone, lead) = if e.reached {
            (Tone::Income, Some(Icon::CheckW3))
        } else {
            (Tone::Warning, None)
        };
        tone_pill(t, tone, e.headline.clone(), lead).into_any_element()
    });
    let mut rows: Vec<(&'static str, &LabeledValue)> = Vec::new();
    let mut card =
        Card::new(t)
            .gap(px(18.0))
            .min_w_0()
            .child(card_header(t, Icon::Shield, &r.title, pill));
    if let Some(e) = &r.emergency {
        card = card.child(emergency_block(t, e));
        rows.push(("res-saldo", &e.balance));
        if let Some(essential) = &e.essential {
            rows.push(("res-essencial", essential));
        }
    } else if let Some(text) = &r.empty {
        card = card.child(
            div()
                .text_size(px(13.0))
                .text_color(t.muted)
                .child(SharedString::from(text.clone())),
        );
    }
    rows.push(("res-total", &r.total));
    if let Some(flow) = &r.month_flow {
        rows.push(("res-fluxo", flow));
    }
    card.child(
        div()
            .flex()
            .flex_col()
            .children(rows.into_iter().map(|(id, item)| value_row(t, id, item))),
    )
    .child(div().mt_auto().child(link(
        t,
        "reserva-link",
        &r.link,
        Screen::Reserves,
        state,
        handle,
    )))
    .into_any_element()
}

fn emergency_block(t: &'static Tokens, e: &DeskEmergency) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(18.0))
        .child(
            div()
                .child(
                    div()
                        .flex()
                        .items_baseline()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_size(px(30.0))
                                .line_height(px(30.0))
                                .font_weight(fonts::weight(600))
                                .child(SharedString::from(e.coverage.clone())),
                        )
                        .child(
                            div()
                                .text_size(px(15.0))
                                .font_weight(fonts::weight(500))
                                .child(SharedString::from(e.unit.clone())),
                        ),
                )
                .child(
                    div()
                        .mt(px(8.0))
                        .text_size(px(13.0))
                        .text_color(t.muted)
                        .child(SharedString::from(e.caption.clone())),
                ),
        )
        .child(Progress::new(f64::from(e.fill), 1.0, t))
}

/// Linha "rótulo ........ valor" com filete em cima (lista da Reserva).
fn value_row(t: &'static Tokens, id: &'static str, item: &LabeledValue) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(12.0))
        .py(px(9.0))
        .border_t_1()
        .border_color(t.divider)
        .text_size(px(13.0))
        .child(
            div()
                .text_color(t.muted)
                .child(SharedString::from(item.label.clone())),
        )
        .child(Money::new(id, item.value.clone(), 13.0, t).weight(600))
}

/// Linhas do tooltip de um mês: os pares que o `DeskView` entrega, o último em negrito
/// ("Sobrou"/"Faltou").
fn tip_of(tip: &DeskTip) -> BarTip {
    let last = tip.rows.len().saturating_sub(1);
    BarTip {
        title: tip.title.clone(),
        lines: tip
            .rows
            .iter()
            .enumerate()
            .map(|(i, row)| Line::Pair {
                label: row.label.clone().into(),
                value: row.value.clone().into(),
                bold: i == last,
            })
            .collect(),
    }
}

fn bar_month(m: &DeskMonth) -> BarMonth {
    BarMonth {
        values: m
            .existed
            .then(|| (bar_units(m.income), bar_units(m.expense))),
        tip: m.tip.as_ref().map(tip_of),
        partial: m.partial,
    }
}

fn history_card(t: &'static Tokens, h: &DeskHistory) -> Card {
    let months: Vec<BarMonth> = h.months.iter().map(bar_month).collect();
    let grid_line = |top: f32| {
        div()
            .absolute()
            .left_0()
            .right_0()
            .top(px(top))
            .border_t_1()
            .border_dashed()
            .border_color(t.divider)
    };
    let axis = div()
        .flex()
        .flex_col()
        .justify_between()
        .items_end()
        .h(px(176.0))
        .mt(px(-6.0))
        .flex_none()
        .text_size(px(12.0))
        .text_color(t.muted)
        .children(
            h.y_labels
                .iter()
                .map(|l| div().child(SharedString::from(l.clone()))),
        );
    let labels = h.months.iter().map(|m| {
        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .text_size(px(12.0))
            .child(
                div()
                    .font_weight(fonts::weight(if m.partial { 600 } else { 500 }))
                    .child(SharedString::from(m.label.clone())),
            )
            .child(
                div()
                    .mt(px(3.0))
                    .text_color(if m.surplus_negative {
                        t.expense
                    } else {
                        t.muted
                    })
                    .child(SharedString::from(m.surplus.clone())),
            )
    });
    let legend = |color: Hsla, text: &str| {
        div()
            .flex()
            .items_center()
            .gap(px(6.0))
            .child(div().size(px(10.0)).rounded(px(3.0)).bg(color))
            .child(SharedString::from(text.to_string()))
    };
    let chart = div()
        .id("hist-grafico")
        .role(Role::Group)
        .flex()
        .gap(px(12.0))
        .flex_1()
        .child(axis)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .relative()
                        .h(px(170.0))
                        .border_b_1()
                        .border_color(t.divider)
                        .child(grid_line(6.0))
                        .child(grid_line(60.0))
                        .child(grid_line(115.0))
                        .child(GroupedBars::new(
                            "hist-barras",
                            h.chart_label.clone(),
                            months,
                            SCALE,
                            170.0,
                            t,
                        )),
                )
                .child(div().mt(px(10.0)).flex().children(labels)),
        );
    Card::new(t)
        .gap(px(18.0))
        .min_w_0()
        .child(card_header(
            t,
            Icon::ChartBars,
            &h.title,
            Some(
                div()
                    .text_size(px(13.0))
                    .text_color(t.muted)
                    .child(SharedString::from(h.subtitle.clone()))
                    .into_any_element(),
            ),
        ))
        .child(chart)
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_x(px(18.0))
                .gap_y(px(8.0))
                .text_size(px(12.0))
                .text_color(t.muted)
                .child(legend(t.income, &h.legend_income))
                .child(legend(t.expense, &h.legend_expense))
                .child(SharedString::from(h.footnote.clone())),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colunas_pela_largura_da_janela() {
        assert_eq!(columns(1440.0), 3);
        assert_eq!(columns(1180.0), 3);
        assert_eq!(columns(1179.9), 2);
        assert_eq!(columns(1100.0), 2);
        assert_eq!(columns(760.0), 2);
        assert_eq!(columns(759.9), 1);
    }

    #[test]
    fn card_largo_nunca_passa_do_numero_de_colunas() {
        assert_eq!(span(3, 2), 2);
        assert_eq!(span(2, 2), 2);
        assert_eq!(span(1, 2), 1);
    }

    #[test]
    fn fracao_vira_unidade_inteira_limitada() {
        assert_eq!(bar_units(0.0), 0);
        assert_eq!(bar_units(0.5), 500);
        assert_eq!(bar_units(1.0), SCALE);
        assert_eq!(bar_units(1.7), SCALE);
        assert_eq!(bar_units(-0.2), 0);
    }

    #[test]
    fn dinheiro_grande_vira_o_texto_que_o_componente_separa() {
        use crate::gui::components::money::split;
        let m = BigMoney {
            sign: "−",
            currency: "R$",
            whole: "96".into(),
            cents: ",40".into(),
        };
        let p = split(&big_text(&m));
        assert_eq!(
            (p.sign.as_str(), p.currency, p.body.as_str()),
            ("−", true, "96,40")
        );
    }

    #[test]
    fn tooltip_do_mes_so_o_ultimo_par_em_negrito() {
        let tip = DeskTip {
            title: "Outubro".into(),
            rows: ["Receitas", "Despesas", "Faltou"]
                .iter()
                .map(|l| LabeledValue {
                    label: (*l).into(),
                    value: "R$ 1,00".into(),
                })
                .collect(),
        };
        let out = tip_of(&tip);
        assert_eq!(out.title, "Outubro");
        let bold: Vec<bool> = out
            .lines
            .iter()
            .map(|l| matches!(l, Line::Pair { bold: true, .. }))
            .collect();
        assert_eq!(bold, [false, false, true]);
    }

    #[test]
    fn mes_que_nao_existia_nao_tem_barra() {
        let m = DeskMonth {
            existed: false,
            income: 0.5,
            ..DeskMonth::default()
        };
        assert_eq!(bar_month(&m).values, None);
        let m = DeskMonth {
            existed: true,
            income: 1.0,
            expense: 0.25,
            ..DeskMonth::default()
        };
        assert_eq!(bar_month(&m).values, Some((SCALE, 250)));
    }
}
