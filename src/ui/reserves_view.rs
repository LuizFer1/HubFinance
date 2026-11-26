//! Tela Reservas: total separado, emergencia coberta, ultimos 12 meses, o card da reserva de
//! emergencia, as caixinhas, a evolucao empilhada e as movimentacoes; ou o estado vazio.
//!
//! Tudo vem de `app.reserves` (calculado em `App::rebuild`); aqui so se monta a arvore. Todo
//! valor em R$ ja chega formatado (e mascarado com "Ocultar valores"): formatar aqui vazaria o
//! numero por cima do botao de ocultar. Os textos seguem o bloco `isRes` do prototipo e o
//! trecho de `renderVals()` a partir de `RD`.

use iced::widget::text::Text;
use iced::widget::text::Wrapping;
use iced::widget::{Column, Space, canvas, column, container, row, text};
use iced::{Alignment, Color, Element, Length, Padding};

use super::app::{App, Message};
use super::charts::{
    EMPTY_RESERVES_HEIGHT, EMPTY_RESERVES_WIDTH, EmptyReservesProgram, StackedBarsProgram,
    series_color,
};
use super::rows::{author_cell_of, hover_row};
use super::theme::{self, Tokens, space, type_scale};
use super::widgets::{Bar, RowEdge, StatSize, TagTone};
use super::{fonts, icons, shell, widgets};
use crate::dashboard::contract::MovementKind;
use crate::dashboard::periods::short_date;
use crate::dashboard::reserves::{
    DEFAULT_MULTIPLE, EmergencyView, EssentialCost, EssentialPart, MovementRow, Pace, PotRow,
    ReserveColor,
};
use crate::dashboard::view::ReservesView;

/// Entre os blocos da tela: o espaco entre cards de toda tela (7.4).
const GAP: f32 = space::BETWEEN_CARDS;
/// Entre rotulo, valor, nota e barra dos cards de numero.
const KPI_SPACING: f32 = space::S;
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

/// `lifebuoy` e chave interna (a emergencia); o resto e chave do app.
fn reserve_glyph(key: &str) -> &'static str {
    if key == "lifebuoy" {
        icons::LIFEBUOY
    } else {
        icons::glyph(key)
    }
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

/// Card de numero: o padding de todo card (20) e espaco 8 entre rotulo, valor, nota e barra.
fn kpi_card<'a>(t: &'static Tokens, content: Column<'a, Message>) -> Element<'a, Message> {
    widgets::card(t, content.spacing(KPI_SPACING)).into()
}

/// Barra de 6 px com 8 de respiro acima, no pe dos cards Total e Emergencia.
fn kpi_bar<'a>(spec: Bar<'_>) -> Element<'a, Message> {
    container(widgets::bar(spec, 6.0))
        .padding(Padding {
            top: space::S,
            ..Padding::ZERO
        })
        .into()
}

/// Nota 12 `text_muted` sob o valor de um card de numero.
fn kpi_note<'a>(t: &'static Tokens, note: &str) -> Text<'a> {
    text(note.to_string())
        .size(type_scale::META)
        .color(t.text_muted)
}

fn total_card(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let v = &app.reserves;
    let note = row![
        icons::icon(icons::ARROW_DOWN, type_scale::META, t.text_muted),
        kpi_note(t, &v.saved_note),
    ]
    .spacing(space::XS)
    .align_y(Alignment::Center);
    let shares: Vec<(f64, Color)> = v
        .shares
        .iter()
        .map(|s| (s.balance_minor as f64, reserve_color(t, &s.color)))
        .collect();
    kpi_card(
        t,
        column![
            widgets::stat_label(t, "Total separado"),
            widgets::money(t, &v.total, StatSize::L, t.text),
            note,
            kpi_bar(Bar::Parts {
                parts: &shares,
                track: t.neutral_800
            }),
        ],
    )
}

/// O que o KPI "Emergência coberta" mostra no lugar do numero.
#[derive(Debug, PartialEq, Eq)]
enum CoverageFigure<'a> {
    /// "3,5" e "de 6 meses" (o complemento, em 16 ao lado do numero de 36).
    Months { value: &'a str, suffix: String },
    /// Sem numero honesto para mostrar: um "—" grande parecia dado faltando, e "0 de 6 meses"
    /// seria mentira com saldo guardado. A linha de baixo explica o motivo.
    Missing(&'static str),
}

/// "de 6 meses"; "de 1 mês" quando o app grava meta de um mes so ("de 1 meses" saia antes).
fn months_suffix(multiple: u8) -> String {
    if multiple == 1 {
        "de 1 mês".to_string()
    } else {
        format!("de {multiple} meses")
    }
}

fn coverage_figure(emergency: Option<&EmergencyView>) -> CoverageFigure<'_> {
    match emergency {
        None => CoverageFigure::Missing("Sem reserva"),
        // Sem custo essencial a cobertura nao existe (nao e zero).
        Some(e) if e.coverage.is_none() => CoverageFigure::Missing("Sem meta"),
        Some(e) => CoverageFigure::Months {
            value: e.coverage_label.as_str(),
            suffix: months_suffix(e.multiple),
        },
    }
}

fn coverage_card(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let v = &app.reserves;
    let empty_segments = vec![0.0; usize::from(DEFAULT_MULTIPLE)];
    let (remaining, segments) = match &v.emergency {
        Some(e) => (e.remaining_label.as_str(), e.segments.as_slice()),
        None => (
            "Nenhuma reserva de emergência no app",
            empty_segments.as_slice(),
        ),
    };
    let big = StatSize::L.size();
    let value: Element<'_, Message> = match coverage_figure(v.emergency.as_ref()) {
        CoverageFigure::Months { value, suffix } => widgets::value_line(
            vec![
                (value.to_string(), big, fonts::INTER_LIGHT, t.text),
                (suffix, type_scale::SECTION, fonts::INTER, t.text_muted),
            ],
            big,
            6.0,
        ),
        // Um degrau abaixo do valor e na cor secundaria, na mesma altura do valor para o card
        // nao encolher: e um estado, nao um numero.
        CoverageFigure::Missing(label) => container(
            text(label)
                .size(type_scale::VALUE_M)
                .line_height(1.0)
                .color(t.text_muted),
        )
        .height(big)
        .align_y(Alignment::End)
        .into(),
    };
    // "hoje" em dourado: a cobertura e o estado de agora, e o dourado marca o agora (principio 2).
    let label = row![
        container(widgets::stat_label(t, "Emergência coberta")).width(Length::Fill),
        widgets::tag(t, TagTone::Accent, "hoje"),
    ]
    .spacing(space::S)
    .align_y(Alignment::Center);
    kpi_card(
        t,
        column![
            label,
            value,
            kpi_note(t, remaining),
            kpi_bar(Bar::Segments {
                values: segments,
                color: t.accent,
                track: t.neutral_800
            }),
        ],
    )
}

/// Cor do valor e do icone de uma linha do card "Últimos 12 meses". Zero na cor secundaria:
/// "−R$ 0,00" no vermelho de despesa apontava uma retirada que nao houve (o texto do zero, sem
/// sinal, vem do modelo).
fn flow_color(minor: i64, color: Color, muted: Color) -> Color {
    if minor == 0 { muted } else { color }
}

/// Altura de cada linha de valor do card "Últimos 12 meses". Com 25, rotulo + duas linhas + nota
/// somam a altura dos vizinhos (rotulo + numero de 36 + nota + barra): a linha de KPIs do iced
/// nao iguala alturas.
const YEAR_LINE_HEIGHT: f32 = 25.0;

fn year_card(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let v = &app.reserves;
    let muted = t.text_muted;
    let line = |glyph: &'static str,
                icon_color: Color,
                color: Color,
                label: &'static str,
                value: &str,
                minor: i64| {
        let value_color = flow_color(minor, color, muted);
        let icon_color = flow_color(minor, icon_color, muted);
        row![
            row![
                icons::icon(glyph, type_scale::LABEL, icon_color),
                text(label).size(type_scale::LABEL).color(t.text_soft()),
            ]
            .spacing(6)
            .align_y(Alignment::Center)
            .width(Length::Fill),
            // Valor secundario: 17 Regular (7.4), a direita como todo dinheiro.
            text(value.to_string())
                .size(type_scale::VALUE_S)
                .line_height(1.0)
                .wrapping(Wrapping::None)
                .color(value_color),
        ]
        .spacing(space::M)
        .height(YEAR_LINE_HEIGHT)
        .align_y(Alignment::Center)
    };
    let mut col = column![
        widgets::stat_label(t, "Últimos 12 meses"),
        line(
            icons::ARROW_DOWN,
            t.accent_300,
            t.text,
            "Guardado",
            &v.deposits,
            v.totals.deposits_minor,
        ),
        line(
            icons::ARROW_UP,
            t.expense_fg,
            t.expense_fg,
            "Retirado",
            &v.withdrawals,
            v.totals.withdrawals_minor,
        ),
    ];
    // "Nenhuma retirada" repetiria a linha de cima, que ja mostra R$ 0,00 em cinza. O vao fica
    // na altura da nota (12 x 1,3) para o card nao ficar mais baixo que os vizinhos.
    col = if v.totals.withdrawal_count > 0 {
        col.push(kpi_note(t, &v.withdrawals_note))
    } else {
        col.push(Space::new().height(16))
    };
    kpi_card(t, col)
}

// ---- linha 2: emergencia e caixinhas ----

/// Titulo da secao da reserva de emergencia, fora do card.
const EMERGENCY_TITLE: &str = "Reserva de emergência";
/// Entre o titulo de secao e o card dele: menor que o espaco entre cards, para o titulo ficar
/// colado ao bloco que nomeia e nao boiar no meio do vao.
const HEADING_GAP: f32 = space::M;

/// Titulo de secao 16 (fora do card) e o card embaixo.
fn titled<'a>(heading: Element<'a, Message>, card: Element<'a, Message>) -> Element<'a, Message> {
    column![heading, card].spacing(HEADING_GAP).into()
}

/// Lado a lado (alinhados ao topo, como o `align-items: start` do prototipo) quando os dois
/// cabem com 440 cada; senao empilhados. Cada um com o titulo de secao em cima, fora do card:
/// "Reserva de emergência" e "Caixinhas" com quantas tem meta ao lado.
fn cards_row(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let v = &app.reserves;
    let width = shell::content_width(app);
    let emergency_heading = widgets::section_heading(t, EMERGENCY_TITLE, "", None, 0.0);
    let pots_heading = widgets::section_heading(t, "Caixinhas", pots_question(v), None, 0.0);
    if width >= 2.0 * CARD_MIN + GAP {
        let card_inner = (width - GAP) / 2.0 - CARD_PADDING_X;
        row![
            titled(emergency_heading, emergency_card(app, card_inner)),
            titled(pots_heading, pots_card(app)),
        ]
        .spacing(GAP)
        .align_y(Alignment::Start)
        .into()
    } else {
        column![
            titled(
                emergency_heading,
                emergency_card(app, width - CARD_PADDING_X)
            ),
            titled(pots_heading, pots_card(app)),
        ]
        .spacing(GAP)
        .into()
    }
}

/// O aparte do titulo "Caixinhas" ("3 com objetivo · 1 sem meta", do modelo); sem caixinha,
/// nada: o card ja diz "Nenhuma caixinha ainda".
fn pots_question(v: &ReservesView) -> &str {
    if v.pots.is_empty() {
        ""
    } else {
        v.pots_note.as_str()
    }
}

/// O nome que a pessoa deu a reserva de emergencia, se nao for o padrao: com o titulo de secao
/// "Reserva de emergência" fora do card, repetir o mesmo nome no cabecalho do card seria eco.
fn custom_emergency_name(name: &str) -> Option<&str> {
    let name = name.trim();
    (!name.is_empty() && name.to_lowercase() != EMERGENCY_TITLE.to_lowercase()).then_some(name)
}

/// Caixa de 36 com borda no acento e a boia: o simbolo da emergencia.
fn lifebuoy_box<'a>(t: &'static Tokens) -> Element<'a, Message> {
    container(icons::icon(icons::LIFEBUOY, 18.0, t.accent_300))
        .center(36)
        .style(theme::outlined(t.accent, 8.0))
        .into()
}

fn emergency_card(app: &App, inner_width: f32) -> Element<'_, Message> {
    let t = app.tokens;
    let Some(e) = &app.reserves.emergency else {
        // O titulo da secao, fora do card, ja diz o que falta; aqui so a boia e o motivo.
        return widgets::card(
            t,
            row![
                lifebuoy_box(t),
                text("Nenhuma reserva de emergência sincronizada")
                    .size(type_scale::LABEL)
                    .color(t.text_muted),
            ]
            .spacing(space::M)
            .align_y(Alignment::Center),
        )
        .into();
    };
    let goal = text(format!("Meta: {} × custo essencial", e.multiple));
    let mut about = column![].width(Length::Fill);
    if let Some(name) = custom_emergency_name(&e.name) {
        about = about
            .push(
                container(
                    text(name.to_string())
                        .size(type_scale::SUPPORT)
                        .font(fonts::INTER_MEDIUM)
                        .wrapping(Wrapping::None)
                        .color(t.text),
                )
                .clip(true),
            )
            .push(goal.size(type_scale::META).color(t.text_muted));
    } else {
        // Sem nome proprio, a meta vira a linha principal ao lado da boia.
        about = about.push(goal.size(type_scale::LABEL).color(t.text_soft()));
    }
    let header = row![
        lifebuoy_box(t),
        about,
        widgets::tag(t, TagTone::Accent, &e.pct_label),
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    let mut balance = row![widgets::money(t, &e.balance, StatSize::L, t.text)]
        .spacing(10)
        .align_y(Alignment::End);
    if let Some(goal) = &e.goal_note {
        balance = balance.push(
            container(
                text(goal.clone())
                    .size(13)
                    .line_height(1.0)
                    .color(t.text_muted),
            )
            .padding(Padding {
                bottom: 2.0,
                ..Padding::ZERO
            }),
        );
    }
    let mut col = column![
        header,
        Space::new().height(16),
        balance,
        Space::new().height(14),
        widgets::bar(
            Bar::Segments {
                values: &e.segments,
                color: t.accent,
                track: t.neutral_800
            },
            10.0,
        ),
    ];
    if !e.segments.is_empty() {
        col = col.push(Space::new().height(6)).push(meter_labels(t, e));
    }
    let eta = row![
        icons::icon(icons::CALENDAR_CHECK, 13.0, t.accent_300),
        text(e.eta_label.clone()).size(13).color(t.text_soft()),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    col = col
        .push(Space::new().height(20))
        .push(widgets::divider(t))
        .push(Space::new().height(16))
        .push(essential_section(app, e.cost.as_ref(), inner_width))
        .push(Space::new().height(14))
        .push(eta);
    widgets::card(t, col).into()
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
                .color(t.text_muted),
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
    let muted = t.text_muted;
    let title = text("Custo essencial médio")
        .size(13)
        .font(fonts::INTER_MEDIUM)
        .color(t.text)
        .width(Length::Fill);
    let Some(cost) = cost else {
        return column![
            title,
            Space::new().height(2),
            text("Sem lançamentos suficientes para calcular o custo essencial")
                .size(12)
                .color(muted),
        ]
        .into();
    };
    let header = row![
        title,
        text(cost.total_label.clone())
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
            text("Custo informado no app").size(12).color(muted),
        ]
        .into();
    }
    if cost.parts.is_empty() {
        return column![
            header,
            Space::new().height(2),
            text("Nenhuma categoria essencial definida no app")
                .size(12)
                .color(muted),
        ]
        .into();
    }
    // "Média dos últimos 6 meses"; numa casa nova, os meses que ela tem.
    let note = if cost.months == 1 {
        "Média do último mês nestas categorias".to_string()
    } else {
        format!("Média dos últimos {} meses nestas categorias", cost.months)
    };
    let bar_parts: Vec<(f64, Color)> = cost
        .parts
        .iter()
        .filter(|p| p.average_minor > 0)
        .map(|p| (p.average_minor as f64, t.token(&p.color)))
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
            text(p.average_label.clone())
                .size(13)
                .wrapping(Wrapping::None)
                .color(t.text_soft()),
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
        widgets::bar(
            Bar::Parts {
                parts: &bar_parts,
                track: t.neutral_800
            },
            6.0,
        ),
        Space::new().height(10),
        legend,
    ]
    .into()
}

fn pots_card(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let v = &app.reserves;
    // O titulo saiu para fora do card: as linhas comecam no topo, com divisor so entre elas. O
    // respiro de 6 em cima e embaixo soma aos 14 de cada linha os 20 de todo card.
    let mut col = column![];
    if v.pots.is_empty() {
        col = col.push(
            container(
                text("Nenhuma caixinha ainda")
                    .size(type_scale::LABEL)
                    .color(t.text_muted),
            )
            .padding([14, 0]),
        );
    }
    for (i, p) in v.pots.iter().enumerate() {
        if i > 0 {
            col = col.push(widgets::divider(t));
        }
        col = col.push(pot_row(app, p));
    }
    widgets::card(t, col).padding(pad(6.0, 20.0, 6.0)).into()
}

/// Icone e cor da linha de ritmo (tabela de `Pace` da spec).
fn pace_style(t: &'static Tokens, pace: &Pace) -> (&'static str, Color) {
    match pace {
        Pace::OnTrack { .. } | Pace::Reached => (icons::CHECK_CIRCLE, t.income_fg),
        Pace::Overdue { .. } | Pace::Behind { .. } => (icons::WARNING_CIRCLE, t.warning),
        Pace::NoRecurring { .. } | Pace::NoDeadline { .. } | Pace::NoGoal => {
            (icons::REPEAT, t.text_muted)
        }
    }
}

fn pot_row<'a>(app: &'a App, p: &'a PotRow) -> Element<'a, Message> {
    let t = app.tokens;
    let color = reserve_color(t, &p.color);
    let tile = widgets::tile(t, reserve_glyph(&p.icon), color, color, 36.0);
    let muted = t.text_muted;
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
            text(p.balance.clone())
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
        body = body.push(Space::new().height(10)).push(widgets::bar(
            Bar::Fraction {
                value: progress,
                color,
                track: Some(t.neutral_900),
                outlined: false,
            },
            6.0,
        ));
    }
    if let Some(label) = p.pace_label.clone() {
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
    let muted = t.text_muted;
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
                    .style(theme::fill(series_color(t, &s.color), 2.0)),
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
        text("Nenhum saldo nos últimos 12 meses")
            .size(13)
            .color(t.text_muted)
            .into()
    } else {
        canvas(StackedBarsProgram {
            series: &v.series,
            months: &v.months,
            axis_top: v.axis_top,
            y_labels: &v.y_labels,
            month_totals: &v.month_totals,
            current: v.months.len().saturating_sub(1),
            tokens: t,
            cache: &app.reserves_cache,
        })
        .width(Length::Fill)
        .height(EVOLUTION_HEIGHT)
        .into()
    };
    widgets::card(t, column![header, Space::new().height(18), body])
        .padding(pad(18.0, 20.0, 20.0))
        .into()
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
        col = col.push(widgets::divider(t)).push(
            container(
                text("Nenhuma movimentação nos últimos 12 meses")
                    .size(13)
                    .color(t.text_muted),
            )
            .height(MOVEMENT_ROW_HEIGHT)
            .padding([0, 8])
            .align_y(Alignment::Center),
        );
    }
    for m in &v.movements {
        col = col.push(widgets::divider(t)).push(movement_row(app, m));
    }
    widgets::card(t, col).padding(pad(18.0, 12.0, 8.0)).into()
}

/// Tile de 32: guardar e seta para baixo no acento; retirar e seta para cima na cor de despesa.
fn movement_tile<'a>(t: &'static Tokens, kind: MovementKind) -> Element<'a, Message> {
    let (glyph, color, tint) = match kind {
        MovementKind::Deposit => (icons::ARROW_DOWN, t.accent_300, t.accent),
        MovementKind::Withdrawal => (icons::ARROW_UP, t.expense_fg, t.expense),
    };
    widgets::tile(t, glyph, color, tint, 32.0)
}

fn movement_row<'a>(app: &'a App, m: &'a MovementRow) -> Element<'a, Message> {
    let t = app.tokens;
    let [desc_w, reserve_w, author_w] = MOVEMENT_COLUMNS;
    let mut meta = row![
        text(short_date(&m.occurred_on))
            .size(12)
            .wrapping(Wrapping::None)
            .color(t.text_muted)
    ]
    .spacing(6)
    .align_y(Alignment::Center);
    if m.recurring {
        meta = meta.push(widgets::tag_icon(
            t,
            TagTone::Accent,
            icons::REPEAT,
            "Mensal",
        ));
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
            .color(t.text_soft()),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    let value_color = match m.kind {
        MovementKind::Deposit => t.text,
        MovementKind::Withdrawal => t.expense_fg,
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
            text(m.amount.clone())
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
    hover_row(
        app,
        &m.id,
        content,
        [0, 8],
        RowEdge::Inset,
        Some(MOVEMENT_ROW_HEIGHT),
    )
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
    let mut copy = widgets::empty_copy(
        t,
        "Nenhuma reserva sincronizada",
        "A reserva de emergência e as caixinhas são criadas no app. Assim que um celular pareado \
         sincronizar, saldos, metas e movimentações aparecem aqui.",
        false,
    );
    if let Some(connect) = shell::connect_phone_button(app) {
        copy = copy.push(Space::new().height(20)).push(connect);
    }
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
    widgets::card(t, body)
        .padding([56.0, EMPTY_PADDING_X])
        .into()
}

#[cfg(test)]
mod tests {
    use super::super::app::tests::{app_with, app_with_reserves};
    use super::*;
    use crate::hub::snapshot::Snapshot;

    #[test]
    fn zero_dos_ultimos_12_meses_na_cor_secundaria() {
        let t = &theme::DARK;
        assert_eq!(flow_color(0, t.expense_fg, t.text_muted), t.text_muted);
        assert_eq!(flow_color(12_345, t.expense_fg, t.text_muted), t.expense_fg);
        assert_eq!(flow_color(920_000, t.text, t.text_muted), t.text);
    }

    fn emergency(coverage: Option<f64>, label: &str) -> EmergencyView {
        EmergencyView {
            id: "E".into(),
            name: "Emergência".into(),
            balance_minor: 1_880_000,
            balance: Default::default(),
            goal_note: None,
            multiple: 6,
            cost: None,
            goal_minor: 0,
            coverage,
            coverage_label: label.into(),
            pct_label: String::new(),
            remaining_label: String::new(),
            segments: vec![0.0; 6],
            eta: crate::dashboard::reserves::EmergencyEta::NoGoal,
            eta_label: String::new(),
        }
    }

    #[test]
    fn cobertura_sem_numero_vira_estado_e_nao_traco() {
        assert_eq!(
            coverage_figure(None),
            CoverageFigure::Missing("Sem reserva")
        );
        let no_cost = emergency(None, "—");
        assert_eq!(
            coverage_figure(Some(&no_cost)),
            CoverageFigure::Missing("Sem meta")
        );
        let covered = emergency(Some(3.5), "3,5");
        assert_eq!(
            coverage_figure(Some(&covered)),
            CoverageFigure::Months {
                value: "3,5",
                suffix: "de 6 meses".to_string()
            }
        );
        // Meta de um mes so: o complemento concorda com o numero.
        let one = EmergencyView {
            multiple: 1,
            ..emergency(Some(0.5), "0,5")
        };
        assert_eq!(
            coverage_figure(Some(&one)),
            CoverageFigure::Months {
                value: "0,5",
                suffix: "de 1 mês".to_string()
            }
        );
    }

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
                total_label: "R$ 2.500/mês".into(),
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
    fn nome_da_emergencia_so_no_card_quando_e_proprio() {
        assert_eq!(custom_emergency_name("Reserva de emergência"), None);
        assert_eq!(custom_emergency_name("  reserva de EMERGÊNCIA "), None);
        assert_eq!(custom_emergency_name(""), None);
        assert_eq!(
            custom_emergency_name("Colchão da casa"),
            Some("Colchão da casa")
        );
    }

    #[test]
    fn aparte_das_caixinhas_vem_do_modelo_e_some_sem_caixinha() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with_reserves(dir.path());
        assert!(!app.reserves.pots.is_empty());
        assert_eq!(pots_question(&app.reserves), app.reserves.pots_note);
        app.reserves.pots.clear();
        assert_eq!(pots_question(&app.reserves), "");
        let _ = view(&app);
    }

    #[test]
    fn glifo_da_reserva() {
        assert_eq!(reserve_glyph("lifebuoy"), icons::LIFEBUOY);
        assert_eq!(reserve_glyph("plane"), icons::AIRPLANE);
        assert_eq!(reserve_glyph("nao-existe"), icons::CIRCLE_DASHED);
    }
}
