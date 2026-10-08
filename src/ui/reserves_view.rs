//! Tela Reservas: total separado, emergencia coberta, ultimos 12 meses, o card da reserva de
//! emergencia, as caixinhas, a evolucao empilhada e as movimentacoes; ou o estado vazio.
//!
//! Tudo vem de `app.reserves` (calculado em `App::rebuild`); aqui so se monta a arvore. Os
//! textos seguem o bloco `isRes` do prototipo e o trecho de `renderVals()` a partir de `RD`.

use iced::widget::text::Wrapping;
use iced::widget::{Space, canvas, column, container, row, text};
use iced::{Alignment, Color, Element, Length, Padding};

use super::app::{App, Message};
use super::charts::{
    EMPTY_RESERVES_HEIGHT, EMPTY_RESERVES_WIDTH, EmptyReservesProgram, StackedBarsProgram,
};
use super::dashboard_view::{author_cell_of, hover_row, kicker_with_icon};
use super::theme::{self, Tokens, mix};
use super::{fonts, icons, shell, widgets};
use crate::dashboard::contract::MovementKind;
use crate::dashboard::money::{format_brl, money_parts, whole_brl};
use crate::dashboard::periods::{month_long, short_date};
use crate::dashboard::reserves::{
    DEFAULT_MULTIPLE, EmergencyView, EssentialCost, EssentialPart, MovementRow, Pace, PotRow,
    ReserveColor, pace_label,
};

const GAP: f32 = 14.0;
/// `minmax(240px, 1fr)` dos KPIs e `minmax(440px, 1fr)` dos dois cards da linha 2.
const KPI_MIN: f32 = 240.0;
const CARD_MIN: f32 = 440.0;
/// `repeat(auto-fill, minmax(180px, 1fr))` da legenda do custo essencial, com 24 entre colunas.
const LEGEND_COL_MIN: f32 = 180.0;
const LEGEND_COL_GAP: f32 = 24.0;
/// Padding lateral dos cards da linha 2 (20 + 20).
const CARD_PADDING_X: f32 = 40.0;

/// Largura minima da coluna de texto do estado vazio, ao lado do esqueleto.
const EMPTY_TEXT_MIN: f32 = 260.0;
const EMPTY_GAP_X: f32 = 56.0;
const EMPTY_PADDING_X: f32 = 32.0;

pub fn view(app: &App) -> Element<'_, Message> {
    let v = &app.reserves;
    if v.is_empty {
        return empty_card(app);
    }
    column![
        kpi_row(app),
        cards_row(app),
        evolution_card(app),
        movements_card(app)
    ]
    .spacing(GAP)
    .width(Length::Fill)
    .into()
}

/// Card de superficie, raio 8, sem borda.
fn card<'a>(
    t: &'static Tokens,
    padding: Padding,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    container(content)
        .padding(padding)
        .width(Length::Fill)
        .style(theme::card(t))
        .into()
}

fn pad(top: f32, x: f32, bottom: f32) -> Padding {
    Padding {
        top,
        right: x,
        bottom,
        left: x,
    }
}

/// Emergencia no acento do tema; caixinha no token da linha.
fn reserve_color(t: &'static Tokens, c: &ReserveColor) -> Color {
    match c {
        ReserveColor::Accent => t.accent,
        ReserveColor::Token(k) => t.token(k),
    }
}

/// Fundo do tile: a cor a 18 % sobre a superficie (o tile das categorias).
fn reserve_tint(t: &'static Tokens, c: &ReserveColor) -> Color {
    match c {
        ReserveColor::Accent => mix(t.surface, t.accent, 0.18),
        ReserveColor::Token(k) => t.token_tint(k, 0.18),
    }
}

/// `lifebuoy` e chave interna (a emergencia); o resto e chave do app.
fn reserve_glyph(key: &str) -> &'static str {
    if key == "lifebuoy" {
        icons::LIFEBUOY
    } else {
        icons::glyph(key)
    }
}

/// Partes de uma barra proporcional: `FillPortion` e `u16`, entao os valores viram milesimos
/// do total, minimo 1 (mesma ideia do `split_portions` do Dashboard).
fn portions(values: &[i64]) -> Vec<u16> {
    let total: i128 = values.iter().map(|v| i128::from((*v).max(0))).sum();
    values
        .iter()
        .map(|v| {
            if total == 0 {
                1
            } else {
                (i128::from((*v).max(0)) * 1000 / total).clamp(1, 1000) as u16
            }
        })
        .collect()
}

/// Barra de 6 px dividida (gap 3, raio 3); sem partes, um trilho so em `neutral_800`.
fn split_bar<'a>(t: &'static Tokens, parts: &[(i64, Color)]) -> Element<'a, Message> {
    if parts.is_empty() {
        return container(Space::new())
            .width(Length::Fill)
            .height(6)
            .style(theme::fill(t.neutral_800, 3.0))
            .into();
    }
    let values: Vec<i64> = parts.iter().map(|(v, _)| *v).collect();
    let segments = portions(&values)
        .into_iter()
        .zip(parts)
        .map(|(portion, (_, color))| {
            container(Space::new())
                .width(Length::FillPortion(portion))
                .height(6)
                .style(theme::fill(*color, 3.0))
                .into()
        });
    iced::widget::Row::with_children(segments)
        .spacing(3)
        .width(Length::Fill)
        .into()
}

/// Trilho com preenchimento parcial (0..=1) a esquerda.
fn fill_bar<'a>(
    track: Color,
    color: Color,
    fraction: f32,
    height: f32,
    radius: f32,
) -> Element<'a, Message> {
    let fraction = fraction.clamp(0.0, 1.0);
    let filled = (fraction * 1000.0).round() as u16;
    let inner: Element<'a, Message> = if filled == 0 {
        Space::new().width(Length::Fill).height(height).into()
    } else {
        let bar = container(Space::new())
            .width(Length::FillPortion(filled))
            .height(height)
            .style(theme::fill(color, radius));
        if filled >= 1000 {
            bar.into()
        } else {
            row![
                bar,
                Space::new()
                    .width(Length::FillPortion(1000 - filled))
                    .height(height)
            ]
            .into()
        }
    };
    container(inner)
        .width(Length::Fill)
        .height(height)
        .style(theme::fill(track, radius))
        .into()
}

/// Medidor de N segmentos (gap 4, raio 2, trilho `neutral_800`, preenchimento `accent`).
fn meter<'a>(t: &'static Tokens, segments: &[f32], height: f32) -> Element<'a, Message> {
    if segments.is_empty() {
        return Space::new().height(height).into();
    }
    iced::widget::Row::with_children(
        segments
            .iter()
            .map(|f| fill_bar(t.neutral_800, t.accent, *f, height, 2.0)),
    )
    .spacing(4)
    .width(Length::Fill)
    .into()
}

// ---- linha 1: KPIs ----

fn kpi_row(app: &App) -> Element<'_, Message> {
    widgets::three_up(
        shell::content_width(app),
        KPI_MIN,
        GAP,
        [total_card(app), coverage_card(app), year_card(app)],
    )
}

fn kpi_card<'a>(
    t: &'static Tokens,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    card(t, pad(18.0, 20.0, 18.0), content)
}

fn total_card(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let v = &app.reserves;
    let parts = money_parts(v.totals.total_minor);
    let muted = t.text_alpha(0.60);
    let note = row![
        icons::icon(icons::ARROW_DOWN, 12.0, muted),
        text(format!(
            "{} guardados em {}",
            format_brl(v.totals.saved_this_month_minor),
            month_long(&v.current_month)
        ))
        .size(12)
        .color(muted),
    ]
    .spacing(5)
    .align_y(Alignment::Center);
    let shares: Vec<(i64, Color)> = v
        .shares
        .iter()
        .map(|s| (s.balance_minor, reserve_color(t, &s.color)))
        .collect();
    kpi_card(
        t,
        column![
            widgets::kicker(t, "Total separado"),
            widgets::money_big(
                &format!("{}R$", parts.sign),
                &parts.whole,
                &parts.cents,
                t.text,
                t.text_alpha(0.55)
            ),
            note,
            container(split_bar(t, &shares)).padding(Padding {
                top: 8.0,
                ..Padding::ZERO
            }),
        ]
        .spacing(6),
    )
}

fn coverage_card(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let v = &app.reserves;
    let empty_segments = vec![0.0; usize::from(DEFAULT_MULTIPLE)];
    let (coverage, multiple, remaining, segments) = match &v.emergency {
        Some(e) => (
            e.coverage_label.as_str(),
            e.multiple,
            e.remaining_label.as_str(),
            e.segments.as_slice(),
        ),
        None => (
            "—",
            DEFAULT_MULTIPLE,
            "Nenhuma reserva de emergência no app",
            empty_segments.as_slice(),
        ),
    };
    let value = row![
        text(coverage.to_string())
            .size(32)
            .font(fonts::INTER_MEDIUM)
            .line_height(1.0)
            .color(t.text),
        container(
            text(format!("de {multiple} meses"))
                .size(16)
                .line_height(1.0)
                .color(t.text_alpha(0.55))
        )
        .padding(Padding {
            bottom: 2.0,
            ..Padding::ZERO
        }),
    ]
    .spacing(6)
    .align_y(Alignment::End);
    kpi_card(
        t,
        column![
            kicker_with_icon(t, icons::LIFEBUOY, t.accent_300, "Emergência coberta"),
            value,
            text(remaining.to_string())
                .size(12)
                .color(t.text_alpha(0.60)),
            container(meter(t, segments, 6.0)).padding(Padding {
                top: 8.0,
                ..Padding::ZERO
            }),
        ]
        .spacing(6),
    )
}

fn year_card(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let v = &app.reserves;
    let line = |glyph: &'static str,
                icon_color: Color,
                label: &'static str,
                value: String,
                value_color: Color| {
        row![
            row![
                icons::icon(glyph, 13.0, icon_color),
                text(label).size(13).color(t.text_alpha(0.75)),
            ]
            .spacing(6)
            .align_y(Alignment::Center)
            .width(Length::Fill),
            text(value)
                .size(20)
                .font(fonts::INTER_MEDIUM)
                .line_height(1.0)
                .wrapping(Wrapping::None)
                .color(value_color),
        ]
        .spacing(12)
        .align_y(Alignment::Center)
    };
    kpi_card(
        t,
        column![
            widgets::kicker(t, "Últimos 12 meses"),
            container(line(
                icons::ARROW_DOWN,
                t.accent_300,
                "Guardado",
                format!("+{}", format_brl(v.totals.deposits_minor)),
                t.text,
            ))
            .padding(Padding {
                top: 4.0,
                ..Padding::ZERO
            }),
            line(
                icons::ARROW_UP,
                t.expense_fg,
                "Retirado",
                format!("\u{2212}{}", format_brl(v.totals.withdrawals_minor)),
                t.expense_fg,
            ),
            text(v.withdrawals_note.clone())
                .size(12)
                .color(t.text_alpha(0.60)),
        ]
        .spacing(6),
    )
}

// ---- linha 2: emergencia e caixinhas ----

/// Lado a lado (alinhados ao topo, como o `align-items: start` do prototipo) quando os dois
/// cabem com 440 cada; senao empilhados.
fn cards_row(app: &App) -> Element<'_, Message> {
    let width = shell::content_width(app);
    if width >= 2.0 * CARD_MIN + GAP {
        let card_inner = (width - GAP) / 2.0 - CARD_PADDING_X;
        row![emergency_card(app, card_inner), pots_card(app)]
            .spacing(GAP)
            .align_y(Alignment::Start)
            .into()
    } else {
        column![emergency_card(app, width - CARD_PADDING_X), pots_card(app)]
            .spacing(GAP)
            .into()
    }
}

/// Caixa de 36 com borda no acento e a boia: o simbolo da emergencia.
fn lifebuoy_box<'a>(t: &'static Tokens) -> Element<'a, Message> {
    container(icons::icon(icons::LIFEBUOY, 18.0, t.accent_300))
        .center(36)
        .style(theme::outlined(t.accent, 8.0))
        .into()
}

fn pill<'a>(t: &'static Tokens, label: String) -> Element<'a, Message> {
    container(
        text(label)
            .size(11)
            .font(fonts::INTER_MEDIUM)
            .line_height(1.0)
            .color(t.accent_300),
    )
    .height(24)
    .padding([0, 9])
    .align_y(Alignment::Center)
    .style(theme::fill(t.accent_900, 999.0))
    .into()
}

fn emergency_card(app: &App, inner_width: f32) -> Element<'_, Message> {
    let t = app.tokens;
    let padding = pad(18.0, 20.0, 20.0);
    let Some(e) = &app.reserves.emergency else {
        let header = row![
            lifebuoy_box(t),
            text("Reserva de emergência")
                .size(15)
                .font(fonts::INTER_MEDIUM)
                .color(t.text),
        ]
        .spacing(12)
        .align_y(Alignment::Center);
        return card(
            t,
            padding,
            column![
                header,
                Space::new().height(16),
                text("Nenhuma reserva de emergência sincronizada.")
                    .size(13)
                    .color(t.text_alpha(0.55)),
            ],
        );
    };
    let header = row![
        lifebuoy_box(t),
        column![
            container(
                text(e.name.clone())
                    .size(15)
                    .font(fonts::INTER_MEDIUM)
                    .wrapping(Wrapping::None)
                    .color(t.text)
            )
            .clip(true),
            text(format!("Meta: {} × custo essencial", e.multiple))
                .size(12)
                .color(t.text_alpha(0.55)),
        ]
        .width(Length::Fill),
        pill(t, e.pct_label.clone()),
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    let parts = money_parts(e.balance_minor);
    let goal = if e.goal_minor > 0 {
        format!("de {}", format_brl(e.goal_minor))
    } else {
        "sem meta".to_string()
    };
    let balance = row![
        widgets::money_big(
            &format!("{}R$", parts.sign),
            &parts.whole,
            &parts.cents,
            t.text,
            t.text_alpha(0.55)
        ),
        container(
            text(goal)
                .size(13)
                .line_height(1.0)
                .color(t.text_alpha(0.55))
        )
        .padding(Padding {
            bottom: 2.0,
            ..Padding::ZERO
        }),
    ]
    .spacing(10)
    .align_y(Alignment::End);
    let mut col = column![
        header,
        Space::new().height(16),
        balance,
        Space::new().height(14),
        meter(t, &e.segments, 10.0),
    ];
    if !e.segments.is_empty() {
        col = col.push(Space::new().height(6)).push(meter_labels(t, e));
    }
    let eta = row![
        icons::icon(icons::CALENDAR_CHECK, 13.0, t.accent_300),
        text(e.eta_label.clone()).size(13).color(t.text_alpha(0.75)),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    col = col
        .push(Space::new().height(20))
        .push(widgets::hdivider(t))
        .push(Space::new().height(16))
        .push(essential_section(app, e.cost.as_ref(), inner_width))
        .push(Space::new().height(14))
        .push(eta);
    card(t, padding, col)
}

/// "1", "2", …, "6 meses": um rotulo sob cada segmento, com a mesma largura.
fn meter_labels<'a>(t: &'static Tokens, e: &EmergencyView) -> Element<'a, Message> {
    let n = e.segments.len();
    iced::widget::Row::with_children((1..=n).map(|i| {
        let label = if i == n {
            format!("{i} meses")
        } else {
            i.to_string()
        };
        container(
            text(label)
                .size(11)
                .wrapping(Wrapping::None)
                .color(t.text_alpha(0.50)),
        )
        .width(Length::FillPortion(1))
        .into()
    }))
    .spacing(4)
    .width(Length::Fill)
    .into()
}

fn essential_section<'a>(
    app: &'a App,
    cost: Option<&'a EssentialCost>,
    inner_width: f32,
) -> Element<'a, Message> {
    let t = app.tokens;
    let muted = t.text_alpha(0.55);
    let title = text("Custo essencial médio")
        .size(13)
        .font(fonts::INTER_MEDIUM)
        .color(t.text)
        .width(Length::Fill);
    let Some(cost) = cost else {
        return column![
            title,
            Space::new().height(2),
            text("Sem lançamentos suficientes para calcular o custo essencial.")
                .size(12)
                .color(muted),
        ]
        .into();
    };
    let header = row![
        title,
        text(format!("{}/mês", whole_brl(cost.total_minor)))
            .size(13)
            .font(fonts::INTER_MEDIUM)
            .wrapping(Wrapping::None)
            .color(t.text),
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    // Com custo informado não há média por categoria: o app grava só o número que o usuário
    // digitou quando a casa não tinha histórico. Mostramos o total e o rótulo "custo informado
    // no app" em vez de inventar partes ou dizer "nenhuma categoria definida", que seria falso.
    if cost.informed {
        return column![
            header,
            Space::new().height(2),
            text("Custo informado no app.").size(12).color(muted),
        ]
        .into();
    }
    if cost.parts.is_empty() {
        return column![
            header,
            Space::new().height(2),
            text("Nenhuma categoria essencial definida no app.")
                .size(12)
                .color(muted),
        ]
        .into();
    }
    // "Média dos últimos 6 meses"; numa casa nova, os meses que ela tem.
    let note = if cost.months == 1 {
        "Média do último mês nestas categorias.".to_string()
    } else {
        format!("Média dos últimos {} meses nestas categorias.", cost.months)
    };
    let bar_parts: Vec<(i64, Color)> = cost
        .parts
        .iter()
        .filter(|p| p.average_minor > 0)
        .map(|p| (p.average_minor, t.token(&p.color)))
        .collect();
    let columns = if inner_width >= 2.0 * LEGEND_COL_MIN + LEGEND_COL_GAP {
        2
    } else {
        1
    };
    let cell = |p: &'a EssentialPart| -> Element<'a, Message> {
        row![
            widgets::dot(t.token(&p.color), 8.0),
            container(
                text(p.name.clone())
                    .size(13)
                    .wrapping(Wrapping::None)
                    .color(t.text)
            )
            .width(Length::Fill)
            .clip(true),
            text(whole_brl(p.average_minor))
                .size(13)
                .wrapping(Wrapping::None)
                .color(t.text_alpha(0.75)),
        ]
        .spacing(10)
        .height(28)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .into()
    };
    let mut legend = column![];
    for chunk in cost.parts.chunks(columns) {
        let mut line = row![].spacing(LEGEND_COL_GAP);
        for p in chunk {
            line = line.push(cell(p));
        }
        for _ in chunk.len()..columns {
            line = line.push(Space::new().width(Length::Fill));
        }
        legend = legend.push(line);
    }
    column![
        header,
        Space::new().height(2),
        text(note).size(12).color(muted),
        Space::new().height(12),
        split_bar(t, &bar_parts),
        Space::new().height(10),
        legend,
    ]
    .into()
}

fn pots_card(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let v = &app.reserves;
    let aside = (!v.pots.is_empty()).then(|| v.pots_note.clone());
    let mut col = column![
        widgets::card_title(t, "Caixinhas", aside),
        Space::new().height(8)
    ];
    if v.pots.is_empty() {
        col = col.push(widgets::hdivider(t)).push(
            container(
                text("Nenhuma caixinha ainda.")
                    .size(13)
                    .color(t.text_alpha(0.55)),
            )
            .padding([14, 0]),
        );
    }
    for p in &v.pots {
        col = col.push(widgets::hdivider(t)).push(pot_row(app, p));
    }
    card(t, pad(18.0, 20.0, 8.0), col)
}

/// Icone e cor da linha de ritmo (tabela de `Pace` da spec).
fn pace_style(t: &'static Tokens, pace: &Pace) -> (&'static str, Color) {
    match pace {
        Pace::OnTrack { .. } | Pace::Reached => (icons::CHECK_CIRCLE, t.income_fg),
        Pace::Overdue { .. } | Pace::Behind { .. } => (icons::WARNING_CIRCLE, t.token("orange")),
        Pace::NoRecurring { .. } | Pace::NoDeadline { .. } | Pace::NoGoal => {
            (icons::REPEAT, t.text_alpha(0.55))
        }
    }
}

fn pot_row<'a>(app: &'a App, p: &'a PotRow) -> Element<'a, Message> {
    let t = app.tokens;
    let color = reserve_color(t, &p.color);
    let tile = container(icons::icon(reserve_glyph(&p.icon), 18.0, color))
        .center(36)
        .style(theme::fill(reserve_tint(t, &p.color), 8.0));
    let muted = t.text_alpha(0.55);
    let mut meta = row![
        text(p.meta_left.clone())
            .size(12)
            .color(muted)
            .width(Length::Fill)
    ]
    .spacing(12);
    if let Some(right) = &p.meta_right {
        meta = meta.push(
            text(right.clone())
                .size(12)
                .wrapping(Wrapping::None)
                .color(muted),
        );
    }
    let mut body = column![
        row![
            container(
                text(p.name.clone())
                    .size(14)
                    .font(fonts::INTER_MEDIUM)
                    .wrapping(Wrapping::None)
                    .color(t.text)
            )
            .width(Length::Fill)
            .clip(true),
            text(format_brl(p.balance_minor))
                .size(14)
                .font(fonts::INTER_MEDIUM)
                .wrapping(Wrapping::None)
                .color(t.text),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
        Space::new().height(2),
        meta,
    ]
    .width(Length::Fill);
    if let Some(progress) = p.progress {
        body = body.push(Space::new().height(10)).push(fill_bar(
            t.neutral_900,
            color,
            progress,
            6.0,
            3.0,
        ));
    }
    if let Some(label) = pace_label(&p.pace) {
        let (glyph, pace_color) = pace_style(t, &p.pace);
        body = body.push(Space::new().height(8)).push(
            row![
                icons::icon(glyph, 12.0, pace_color),
                text(label).size(12).color(pace_color),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        );
    }
    container(row![tile, body].spacing(14).align_y(Alignment::Start))
        .padding([14, 0])
        .width(Length::Fill)
        .into()
}

// ---- linha 3: evolucao ----

/// Abaixo disto a legenda vai para baixo do titulo (o `flex-wrap` do cabecalho).
const EVOLUTION_LEGEND_INLINE: f32 = 760.0;
const EVOLUTION_HEIGHT: f32 = 230.0;

fn evolution_card(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let v = &app.reserves;
    let muted = t.text_alpha(0.60);
    let title = text("Evolução do dinheiro separado")
        .size(15)
        .font(fonts::INTER_MEDIUM)
        .color(t.text);
    let mut keys = row![].spacing(14).align_y(Alignment::Center);
    for s in &v.series {
        keys = keys.push(
            row![
                container(Space::new())
                    .width(8)
                    .height(8)
                    .style(theme::fill(reserve_color(t, &s.color), 2.0)),
                text(s.name.clone()).size(12).color(muted),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        );
    }
    keys = keys.push(text("saldo no fim de cada mês").size(12).color(muted));
    let legend = keys.wrap().vertical_spacing(6);
    let header: Element<'_, Message> = if shell::content_width(app) >= EVOLUTION_LEGEND_INLINE {
        row![container(title).width(Length::Fill), legend]
            .spacing(16)
            .align_y(Alignment::Center)
            .into()
    } else {
        column![title, legend].spacing(8).into()
    };
    let quiet = v.series.iter().all(|s| s.balances.iter().all(|b| *b <= 0));
    let body: Element<'_, Message> = if quiet {
        text("Nenhum saldo nos últimos 12 meses.")
            .size(13)
            .color(t.text_alpha(0.55))
            .into()
    } else {
        canvas(StackedBarsProgram {
            series: &v.series,
            months: &v.months,
            axis_top: v.axis_top,
            current: v.months.len().saturating_sub(1),
            tokens: t,
            cache: &app.reserves_cache,
        })
        .width(Length::Fill)
        .height(EVOLUTION_HEIGHT)
        .into()
    };
    card(
        t,
        pad(18.0, 20.0, 20.0),
        column![header, Space::new().height(18), body],
    )
}

// ---- linha 4: movimentacoes ----

/// Colunas `2.4fr 1.6fr 0.9fr 116px` do prototipo.
const MOVEMENT_COLUMNS: [u16; 3] = [24, 16, 9];
const MOVEMENT_VALUE_WIDTH: f32 = 116.0;
const MOVEMENT_ROW_HEIGHT: f32 = 56.0;

fn movements_card(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let v = &app.reserves;
    // Padding lateral 12 (e nao 20): o hover da linha passa 8 px do conteudo, como o
    // `margin: 0 -8px` do prototipo.
    let title = container(widgets::card_title(
        t,
        "Movimentações",
        Some(format!("{} nos últimos 12 meses", v.movement_count)),
    ))
    .padding([0, 8]);
    let mut col = column![title, Space::new().height(8)];
    if v.movements.is_empty() {
        col = col.push(widgets::hdivider(t)).push(
            container(
                text("Nenhuma movimentação nos últimos 12 meses.")
                    .size(13)
                    .color(t.text_alpha(0.55)),
            )
            .height(MOVEMENT_ROW_HEIGHT)
            .padding([0, 8])
            .align_y(Alignment::Center),
        );
    }
    for m in &v.movements {
        col = col.push(widgets::hdivider(t)).push(movement_row(app, m));
    }
    card(t, pad(18.0, 12.0, 8.0), col)
}

/// Tile de 32: guardar e seta para baixo no acento; retirar e seta para cima na cor de despesa.
fn movement_tile<'a>(t: &'static Tokens, kind: MovementKind) -> Element<'a, Message> {
    let (glyph, color, tint) = match kind {
        MovementKind::Deposit => (
            icons::ARROW_DOWN,
            t.accent_300,
            mix(t.surface, t.accent, 0.16),
        ),
        MovementKind::Withdrawal => (
            icons::ARROW_UP,
            t.expense_fg,
            mix(t.surface, t.expense, 0.16),
        ),
    };
    container(icons::icon(glyph, 16.0, color))
        .center(32)
        .style(theme::fill(tint, 8.0))
        .into()
}

/// Tag "Mensal" de 18 px do deposito recorrente.
fn recurring_tag<'a>(t: &'static Tokens) -> Element<'a, Message> {
    container(
        row![
            icons::icon(icons::REPEAT, 10.0, t.accent_300),
            text("Mensal")
                .size(10)
                .font(fonts::INTER_MEDIUM)
                .line_height(1.0)
                .color(t.accent_300),
        ]
        .spacing(3)
        .align_y(Alignment::Center),
    )
    .height(18)
    .padding([0, 6])
    .align_y(Alignment::Center)
    .style(theme::fill(t.accent_900, 999.0))
    .into()
}

fn movement_row<'a>(app: &'a App, m: &'a MovementRow) -> Element<'a, Message> {
    let t = app.tokens;
    let [desc_w, reserve_w, author_w] = MOVEMENT_COLUMNS;
    let mut meta = row![
        text(short_date(&m.occurred_on))
            .size(12)
            .wrapping(Wrapping::None)
            .color(t.text_alpha(0.55))
    ]
    .spacing(6)
    .align_y(Alignment::Center);
    if m.recurring {
        meta = meta.push(recurring_tag(t));
    }
    let description = row![
        movement_tile(t, m.kind),
        column![
            text(m.description.clone())
                .size(14)
                .font(fonts::INTER_MEDIUM)
                .wrapping(Wrapping::None)
                .color(t.text),
            meta,
        ]
        .spacing(2),
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    let reserve = row![
        widgets::dot(reserve_color(t, &m.reserve_color), 8.0),
        text(m.reserve_name.clone())
            .size(13)
            .wrapping(Wrapping::None)
            .color(t.text_alpha(0.80)),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    let (value, value_color) = match m.kind {
        MovementKind::Deposit => (format!("+{}", format_brl(m.amount_minor)), t.text),
        MovementKind::Withdrawal => (
            format!("\u{2212}{}", format_brl(m.amount_minor)),
            t.expense_fg,
        ),
    };
    let content = row![
        container(description)
            .width(Length::FillPortion(desc_w))
            .clip(true),
        container(reserve)
            .width(Length::FillPortion(reserve_w))
            .clip(true),
        container(author_cell_of(app, m.author.as_ref()))
            .width(Length::FillPortion(author_w))
            .clip(true),
        container(
            text(value)
                .size(14)
                .font(fonts::INTER_MEDIUM)
                .wrapping(Wrapping::None)
                .color(value_color),
        )
        .width(MOVEMENT_VALUE_WIDTH)
        .align_x(Alignment::End),
    ]
    .spacing(14)
    .align_y(Alignment::Center);
    hover_row(app, &m.id, content, [0, 8], 8.0, MOVEMENT_ROW_HEIGHT)
}

// ---- estado vazio ----

/// Um card so: o medidor tracejado e o texto. O `flex-wrap` do prototipo vira uma quebra manual
/// (o iced nao tem): lado a lado quando os dois cabem, senao empilhados.
fn empty_card(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let sketch = canvas(EmptyReservesProgram {
        tokens: t,
        cache: &app.reserves_empty_cache,
    })
    .width(EMPTY_RESERVES_WIDTH)
    .height(EMPTY_RESERVES_HEIGHT);
    let copy = column![
        text("Nenhuma reserva sincronizada")
            .size(20)
            .font(fonts::INTER_MEDIUM)
            .color(t.text),
        Space::new().height(8),
        text(
            "A reserva de emergência e as caixinhas são criadas no app. Assim que um celular \
             pareado sincronizar, saldos, metas e movimentações aparecem aqui."
        )
        .size(14)
        .line_height(1.55)
        .color(t.text_alpha(0.65)),
    ]
    .max_width(460);
    let inner = shell::content_width(app) - 2.0 * EMPTY_PADDING_X;
    let body: Element<'_, Message> = if inner >= EMPTY_RESERVES_WIDTH + EMPTY_GAP_X + EMPTY_TEXT_MIN
    {
        row![sketch, copy]
            .spacing(EMPTY_GAP_X)
            .align_y(Alignment::Center)
            .into()
    } else {
        column![sketch, copy].spacing(32).into()
    };
    container(body)
        .padding([56.0, EMPTY_PADDING_X])
        .width(Length::Fill)
        .style(theme::card(t))
        .into()
}

#[cfg(test)]
mod tests {
    use super::super::app::tests::{app_with, app_with_reserves};
    use super::*;
    use crate::hub::snapshot::Snapshot;

    #[test]
    fn constroi_vazia_e_com_dados_sem_panico() {
        let dir = tempfile::tempdir().unwrap();
        let (app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        assert!(app.reserves.is_empty);
        let _ = view(&app);
        let (mut app, _rx, _tx) = app_with_reserves(dir.path());
        assert!(!app.reserves.is_empty);
        let _ = view(&app);
        // Janela estreita: KPIs em 2 + 1, cards empilhados, legenda numa coluna.
        app.window_size = iced::Size::new(1024.0, 720.0);
        let _ = view(&app);
        app.window_size = iced::Size::new(600.0, 720.0);
        let _ = view(&app);
    }

    #[test]
    fn custo_informado_constroi_sem_panico() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with_reserves(dir.path());
        assert!(app.reserves.emergency.is_some());
        if let Some(e) = app.reserves.emergency.as_mut() {
            e.cost = Some(EssentialCost {
                total_minor: 250_000,
                months: 0,
                parts: Vec::new(),
                informed: true,
            });
        }
        let _ = view(&app);
        app.window_size = iced::Size::new(600.0, 720.0);
        let _ = view(&app);
    }

    #[test]
    fn sem_emergencia_e_sem_caixinhas_constroi() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with_reserves(dir.path());
        app.reserves.emergency = None;
        app.reserves.pots.clear();
        app.reserves.movements.clear();
        app.reserves.series.clear();
        let _ = view(&app);
    }

    #[test]
    fn partes_proporcionais_em_milesimos() {
        assert_eq!(portions(&[]), Vec::<u16>::new());
        assert_eq!(portions(&[0, 0]), vec![1, 1]);
        assert_eq!(portions(&[300, 100]), vec![750, 250]);
        assert_eq!(portions(&[1, 1_000_000]), vec![1, 999]);
        assert_eq!(portions(&[-5, 10]), vec![1, 1000]);
    }

    #[test]
    fn glifo_da_reserva() {
        assert_eq!(reserve_glyph("lifebuoy"), icons::LIFEBUOY);
        assert_eq!(reserve_glyph("plane"), icons::AIRPLANE);
        assert_eq!(reserve_glyph("nao-existe"), icons::CIRCLE_DASHED);
    }
}
