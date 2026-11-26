//! Tela Dashboard "Grafite" (Resumo da casa): Negócio?, Como estou, Reserva + Histórico, Para
//! onde foi, O que vem, Últimos lançamentos e Quem lançou; ou o estado "Sem dados".
//!
//! Tudo vem de `app.dashboard` (`GrafiteView`, calculado em `App::rebuild`) com os textos ja
//! prontos: esta tela nao formata dinheiro, so escolhe tamanho, cor e lugar. Formatar aqui
//! passaria por cima de "Ocultar valores", que mora no formatador do modelo.
//!
//! O iced nao tem `flex-wrap`: as colunas saem de `flex_lines`, que reproduz a quebra do CSS a
//! partir da largura util (`shell::content_width`), e os cards lado a lado ganham a mesma altura
//! com o fundo desenhado numa camada por baixo (`tiles`).

use ::time::OffsetDateTime;
use iced::widget::text::{IntoFragment, Span, Wrapping};
use iced::widget::{Space, Stack, canvas, column, container, rich_text, row, span, text, tooltip};
use iced::{Alignment, Color, Element, Font, Length, Padding};

use super::app::{App, Message, Screen};
use super::charts::{
    EMPTY_HEIGHT, EMPTY_WIDTH, EmptyChartProgram, HISTORY_HEIGHT, HistoryProgram, MeterProgram,
    PACE_HEIGHT, PaceProgram, SwatchProgram,
};
use super::theme::{self, ButtonKind, ButtonSize, Tokens};
use super::widgets::{Bar, TagTone};
use super::{fonts, icons, shell, time, widgets};
use crate::dashboard::grafite::{
    BusinessBlock, EmergencyBlock, HistoryBody, Line, Section, Tone, UpcomingLine,
};
use crate::dashboard::money::group_thousands;

/// Espaco entre blocos (o `gap: 16px` do `main`).
const GAP: f32 = 16.0;
/// Largura util a partir da qual o Dashboard usa duas colunas como o design (spec 8).
pub(super) const TWO_COLUMNS_MIN: f32 = 1100.0;

// ---- layout ----

/// Faixa da tela: um bloco na largura toda ou duas colunas.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Band {
    Full(Section),
    Split(Vec<Section>, Vec<Section>),
}

/// Secoes do modelo em faixas. Com duas colunas: Reserva | Histórico e O que vem | (Últimos,
/// Quem lançou), como o design; sem "O que vem" (mes fechado) os ultimos e quem lancou dividem
/// a linha. Com uma coluna, cada secao na largura toda, na mesma ordem.
pub(super) fn bands(sections: &[Section], width: f32) -> Vec<Band> {
    if width < TWO_COLUMNS_MIN {
        return sections.iter().map(|s| Band::Full(*s)).collect();
    }
    let has = |s: Section| sections.contains(&s);
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(s) = sections.get(i).copied() {
        i += 1;
        match s {
            Section::Reserve if has(Section::History) => {
                out.push(Band::Split(vec![Section::Reserve], vec![Section::History]));
            }
            Section::History if has(Section::Reserve) => {}
            Section::WhatsNext => {
                let right: Vec<Section> = [Section::Recent, Section::People]
                    .into_iter()
                    .filter(|s| has(*s))
                    .collect();
                out.push(Band::Split(vec![Section::WhatsNext], right));
            }
            Section::Recent | Section::People if has(Section::WhatsNext) => {}
            Section::Recent if has(Section::People) => {
                out.push(Band::Split(vec![Section::Recent], vec![Section::People]));
            }
            Section::People if has(Section::Recent) => {}
            other => out.push(Band::Full(other)),
        }
    }
    out
}

/// Respiro acima da faixa (o `margin-top: 20px` das linhas de "O que vem" e dos ultimos
/// lancamentos no design). Fica na faixa, nao no titulo: lado a lado, os dois titulos tem de
/// comecar na mesma altura.
pub(super) fn band_top(band: &Band) -> f32 {
    let first = match band {
        Band::Full(s) => Some(s),
        Band::Split(left, _) => left.first(),
    };
    if matches!(first, Some(Section::WhatsNext | Section::Recent)) {
        20.0
    } else {
        0.0
    }
}

/// `flex-wrap` do CSS: itens `(base, crescimento)` entram na linha enquanto as bases e os
/// espacos cabem; cada linha reparte a sobra pelo crescimento. Um item maior que a linha
/// sozinho ocupa a linha (`min-width: 0`). Devolve, por linha, `(indice, largura)`.
pub(super) fn flex_lines(total: f32, gap: f32, items: &[(f32, f32)]) -> Vec<Vec<(usize, f32)>> {
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    let mut used = 0.0;
    for (i, (basis, _)) in items.iter().enumerate() {
        let need = if current.is_empty() {
            *basis
        } else {
            used + gap + basis
        };
        if !current.is_empty() && need > total + 0.01 {
            groups.push(std::mem::take(&mut current));
            used = *basis;
        } else {
            used = need;
        }
        current.push(i);
    }
    if !current.is_empty() {
        groups.push(current);
    }
    groups
        .into_iter()
        .map(|idx| {
            let spec = |i: usize| items.get(i).copied().unwrap_or((0.0, 0.0));
            let bases: f32 = idx.iter().map(|i| spec(*i).0).sum::<f32>()
                + gap * idx.len().saturating_sub(1) as f32;
            let grow: f32 = idx.iter().map(|i| spec(*i).1).sum();
            let free = total - bases;
            idx.iter()
                .map(|i| {
                    let (basis, g) = spec(*i);
                    let w = if free < 0.0 {
                        (basis + free).max(0.0)
                    } else if grow > 0.0 {
                        basis + free * g / grow
                    } else {
                        basis
                    };
                    (*i, w)
                })
                .collect()
        })
        .collect()
}

/// Um bloco de `tiles`: base e crescimento do `flex`, fundo opcional (cor, raio) e respiro.
struct Tile<'a> {
    basis: f32,
    grow: f32,
    bg: Option<(Color, f32)>,
    padding: Padding,
    content: Element<'a, Message>,
}

/// Blocos com `flex-wrap`. Na mesma linha, os fundos tem a altura do mais alto (o `stretch`
/// do design): o fundo vai numa camada por baixo (`Stack::push_under`), que o iced dimensiona
/// pela camada de cima; `Fill` dentro de `scrollable` sozinho viraria zero.
fn tiles<'a>(total: f32, gap: f32, row_gap: f32, items: Vec<Tile<'a>>) -> Element<'a, Message> {
    let specs: Vec<(f32, f32)> = items.iter().map(|t| (t.basis, t.grow)).collect();
    let mut slots: Vec<Option<Tile<'a>>> = items.into_iter().map(Some).collect();
    let mut out = column![].spacing(row_gap);
    for line in flex_lines(total, gap, &specs) {
        let mut front = row![].spacing(gap);
        let mut back = row![].spacing(gap).height(Length::Fill);
        let mut any_bg = false;
        for (i, w) in line {
            let Some(tile) = slots.get_mut(i).and_then(Option::take) else {
                continue;
            };
            front = front.push(container(tile.content).padding(tile.padding).width(w));
            back = back.push(match tile.bg {
                Some((color, radius)) => {
                    any_bg = true;
                    Element::from(
                        container(Space::new())
                            .width(w)
                            .height(Length::Fill)
                            .style(theme::fill(color, radius)),
                    )
                }
                None => Space::new().width(w).into(),
            });
        }
        out = out.push(if any_bg {
            Element::from(Stack::new().push(front).push_under(back))
        } else {
            front.into()
        });
    }
    out.into()
}

// ---- tela ----

pub fn view(app: &App) -> Element<'_, Message> {
    let v = &app.dashboard;
    let width = shell::content_width(app);
    let mut col = column![].spacing(GAP);
    for band in bands(&v.sections(), width) {
        let top = band_top(&band);
        let body = match band {
            Band::Full(s) => section(app, s, width),
            Band::Split(left, right) => split(app, &left, &right, width),
        };
        col = col.push(container(body).padding(Padding {
            top,
            ..Padding::ZERO
        }));
    }
    if let Some(footer) = footer(app) {
        col = col.push(footer);
    }
    col.width(Length::Fill).into()
}

fn section(app: &App, s: Section, width: f32) -> Element<'_, Message> {
    let t = app.tokens;
    match s {
        Section::Empty => empty_card(app),
        Section::Business => business(app, width),
        Section::HowAmI => how_am_i(app, width),
        Section::Reserve => widgets::card(t, reserve_body(app, width - 40.0))
            .padding(Padding::new(20.0))
            .into(),
        Section::History => widgets::card(t, history_body(app))
            .padding(Padding::new(20.0))
            .into(),
        Section::WhereItWent => where_it_went(app, width),
        Section::WhatsNext => whats_next(app, width),
        Section::Recent => recent(app),
        Section::People => people(app).unwrap_or_else(|| Space::new().into()),
    }
}

/// Duas colunas (`2 1 520px` e `1 1 300px` do design).
fn split<'a>(
    app: &'a App,
    left: &[Section],
    right: &[Section],
    width: f32,
) -> Element<'a, Message> {
    let t = app.tokens;
    let widths: Vec<f32> = flex_lines(width, GAP, &[(520.0, 2.0), (300.0, 1.0)])
        .into_iter()
        .flatten()
        .map(|(_, w)| w)
        .collect();
    let (lw, rw) = (
        widths.first().copied().unwrap_or(width),
        widths.get(1).copied().unwrap_or(width),
    );
    // Reserva e Histórico sao cards irmaos: mesma altura.
    if left == [Section::Reserve] && right == [Section::History] {
        let surface = Some((t.surface, 8.0));
        return tiles(
            width,
            GAP,
            GAP,
            vec![
                Tile {
                    basis: 520.0,
                    grow: 2.0,
                    bg: surface,
                    padding: Padding::new(20.0),
                    content: reserve_body(app, lw - 40.0),
                },
                Tile {
                    basis: 300.0,
                    grow: 1.0,
                    bg: surface,
                    padding: Padding::new(20.0),
                    content: history_body(app),
                },
            ],
        );
    }
    let side = |sections: &[Section], w: f32| {
        let mut col = column![].spacing(GAP).width(w);
        for s in sections {
            // "Quem lançou" sozinho a direita dos ultimos lancamentos (mes fechado): desce a
            // altura do titulo para o card alinhar com o card ao lado.
            if *s == Section::People && sections.len() == 1 {
                col = col.push(Space::new().height(16.0 * 1.3));
            }
            col = col.push(section(app, *s, w));
        }
        col
    };
    row![side(left, lw), side(right, rw)].spacing(GAP).into()
}

// ---- pecas ----

fn txt<'a>(content: impl IntoFragment<'a>, size: f32, color: Color) -> text::Text<'a> {
    text(content).size(size).color(color)
}

fn sp<'a>(content: impl IntoFragment<'a>, color: Color) -> Span<'a, (), Font> {
    span(content).color(color)
}

fn tone_color(t: &'static Tokens, tone: Tone, neutral: Color) -> Color {
    match tone {
        Tone::Neutral => neutral,
        Tone::Income => t.income_fg,
        Tone::Expense => t.expense_fg,
        Tone::Warning => t.warning,
    }
}

/// Botao de informacao (icone 16) com o texto em tooltip embaixo, largura max. 280 (o `title`
/// do design; o iced so mostra no hover).
fn info<'a>(t: &'static Tokens, tip: &str) -> Element<'a, Message> {
    tooltip(
        container(icons::icon(icons::INFO, 16.0, t.text_muted)).center(28),
        container(
            txt(tip.to_string(), 12.0, t.text)
                .line_height(1.45)
                .width(Length::Shrink),
        )
        .max_width(280)
        .padding([8, 10])
        .style(theme::menu(t)),
        tooltip::Position::Bottom,
    )
    .gap(2)
    .delay(theme::TOOLTIP_DELAY)
    .into()
}

/// Quadrado/circulo de cor da legenda.
fn swatch<'a>(color: Color, size: f32, radius: f32) -> Element<'a, Message> {
    container(Space::new())
        .width(size)
        .height(size)
        .style(theme::fill(color, radius))
        .into()
}

/// Medidor em canvas (barra, preenchimento e marcas pontilhadas).
fn meter<'a>(program: MeterProgram) -> Element<'a, Message> {
    let h = program.height();
    canvas(program).width(Length::Fill).height(h).into()
}

fn link_button<'a>(t: &'static Tokens, label: &str, message: Message) -> Element<'a, Message> {
    // Linha inteira de 44 (o tamanho grande, texto 14) sem respiro lateral: o link encosta
    // na borda do conteudo do card, como no design. A cor vem do estilo, que muda com o mouse.
    widgets::action_button_with(
        t,
        ButtonKind::Link,
        ButtonSize::Large,
        row![
            text(label.to_string())
                .size(ButtonSize::Large.text_size())
                .width(Length::Fill),
            icons::icon_inherit(icons::CARET_RIGHT, 14.0),
        ]
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding(0)
    .on_press(message)
    .into()
}

// ---- negocio ----

fn business(app: &App, width: f32) -> Element<'_, Message> {
    let t = app.tokens;
    let Some(b) = &app.dashboard.business else {
        return Space::new().into();
    };
    let left = business_result(t, b);
    let right = business_capital(t, b);
    column![
        widgets::section_heading(t, &b.title, &b.question, None, 8.0),
        widgets::card(
            t,
            column![
                tiles(
                    width - 40.0,
                    36.0,
                    24.0,
                    vec![
                        Tile {
                            basis: 300.0,
                            grow: 1.0,
                            bg: None,
                            padding: Padding::ZERO,
                            content: left,
                        },
                        Tile {
                            basis: 300.0,
                            grow: 1.0,
                            bg: None,
                            padding: Padding::ZERO,
                            content: right,
                        },
                    ],
                ),
                txt(b.note.clone(), 12.0, t.text_muted),
            ]
            .spacing(16)
        )
        .padding(Padding::new(20.0)),
    ]
    .spacing(GAP)
    .into()
}

fn business_result<'a>(t: &'static Tokens, b: &BusinessBlock) -> Element<'a, Message> {
    let labeled = |label: &str, value: &str| {
        column![
            txt(label.to_string(), 12.0, t.text_muted),
            txt(value.to_string(), 17.0, t.text).wrapping(Wrapping::None),
        ]
        .spacing(5)
        .width(Length::Fill)
    };
    column![
        txt(b.result.label.clone(), 13.0, t.text_muted),
        txt(
            b.result.value.clone(),
            36.0,
            tone_color(t, b.result_tone, t.text)
        )
        .font(fonts::INTER_LIGHT)
        .line_height(1.0)
        .wrapping(Wrapping::None),
        row![
            labeled(&b.income.label, &b.income.value),
            labeled(&b.expense.label, &b.expense.value),
        ]
        .spacing(16),
    ]
    .spacing(14)
    .into()
}

fn business_capital<'a>(t: &'static Tokens, b: &BusinessBlock) -> Element<'a, Message> {
    let value_row = |label: &str, value: &str, color: Color| {
        row![
            txt(label.to_string(), 14.0, t.text).width(Length::Fill),
            txt(value.to_string(), 14.0, color).wrapping(Wrapping::None),
        ]
        .spacing(12)
    };
    let mut col = column![
        row![
            txt(b.capital_title.clone(), 13.0, t.text_muted).width(Length::Fill),
            widgets::tag(t, TagTone::Accent, &b.today_chip),
        ]
        .align_y(Alignment::Center)
    ]
    .spacing(10);
    for c in &b.capital {
        col = col.push(value_row(&c.label, &c.value, t.text));
    }
    col.push(widgets::divider(t))
        .push(txt(b.debt_title.clone(), 13.0, t.text_muted))
        .push(value_row(&b.debt.label, &b.debt.value, t.expense_fg))
        .into()
}

// ---- como estou ----

fn how_am_i(app: &App, width: f32) -> Element<'_, Message> {
    let t = app.tokens;
    let h = &app.dashboard.how_am_i;
    let body = tiles(
        width - 12.0,
        16.0,
        16.0,
        vec![
            Tile {
                basis: 300.0,
                grow: 1.0,
                bg: None,
                padding: Padding {
                    top: 14.0,
                    right: 0.0,
                    bottom: 14.0,
                    left: 14.0,
                },
                content: balance_column(app),
            },
            Tile {
                basis: 520.0,
                grow: 2.0,
                bg: Some((t.chart_panel(), 6.0)),
                padding: Padding::from([18, 20]),
                content: pace_panel(app),
            },
        ],
    );
    column![
        widgets::section_heading(t, &h.title, &h.question, None, 8.0),
        widgets::card(t, body).padding(Padding::new(6.0)),
    ]
    .spacing(GAP)
    .into()
}

fn outlook_line<'a>(t: &'static Tokens, line: &Line) -> Element<'a, Message> {
    let color = tone_color(t, line.tone, t.text);
    let words = txt(line.text.clone(), 18.0, color)
        .line_height(1.35)
        .width(Length::Fill);
    let glyph = match line.tone {
        Tone::Neutral => None,
        Tone::Income => Some(icons::TREND_UP),
        Tone::Expense | Tone::Warning => Some(icons::TREND_DOWN),
    };
    match glyph {
        Some(g) => row![
            container(icons::icon(g, 20.0, color)).padding(Padding {
                top: 2.0,
                ..Padding::ZERO
            }),
            words
        ]
        .spacing(10)
        .into(),
        None => words.into(),
    }
}

fn balance_column(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let h = &app.dashboard.how_am_i;
    let b = &h.balance;
    let whole_color = if h.balance_negative {
        t.expense_fg
    } else {
        t.text
    };
    let mut col = widgets::stat(
        t,
        &h.balance_label,
        widgets::value_line(
            vec![
                (
                    format!("{}{}", b.sign, b.currency),
                    16.0,
                    fonts::INTER,
                    t.text_muted,
                ),
                (b.whole.clone(), 44.0, fonts::INTER_LIGHT, whole_color),
                (b.cents.clone(), 18.0, fonts::INTER, t.text_muted),
            ],
            44.0,
            8.0,
        ),
        None,
    )
    .spacing(14);
    if let Some(line) = &h.outlook {
        col = col.push(outlook_line(t, line));
    }
    let average = h.casual_average.as_ref().map(|a| {
        txt(a.clone(), 13.0, t.neutral_300)
            .line_height(1.45)
            .width(Length::Fill)
    });
    match (&h.zero_box, average) {
        (Some(z), average) => {
            let mut inner = column![txt(z.title.clone(), 12.0, t.text_muted)].spacing(6);
            if let Some(value) = &z.value {
                let mut line = row![txt(value.clone(), 24.0, t.text).line_height(1.1)]
                    .spacing(8)
                    .align_y(Alignment::End);
                if let Some(until) = &z.until {
                    line = line.push(container(txt(until.clone(), 13.0, t.text_muted)).padding(
                        Padding {
                            bottom: 2.0,
                            ..Padding::ZERO
                        },
                    ));
                }
                inner = inner.push(line);
            }
            if let Some(note) = &z.note {
                inner = inner.push(txt(note.clone(), 14.0, t.warning).line_height(1.45));
            }
            if let Some(a) = average {
                inner = inner.push(a);
            }
            col = col.push(
                container(inner)
                    .padding([14, 16])
                    .width(Length::Fill)
                    .style(theme::fill(t.surface_2, 6.0)),
            );
        }
        (None, Some(a)) => col = col.push(a),
        (None, None) => {}
    }
    let flow = |dot: Color, label: &str, value: &str| {
        column![
            row![
                swatch(dot, 7.0, 3.5),
                txt(label.to_string(), 12.0, t.text_muted)
            ]
            .spacing(6)
            .align_y(Alignment::Center),
            txt(value.to_string(), 17.0, t.text).wrapping(Wrapping::None),
        ]
        .spacing(5)
        .width(Length::Fill)
    };
    col = col.push(
        row![
            flow(t.income, &h.income.label, &h.income.value),
            flow(t.expense, &h.expense.label, &h.expense.value),
        ]
        .spacing(16),
    );
    let reserves = h.reserves_line.as_ref().map(|r| {
        txt(r.clone(), 12.0, t.text_muted)
            .line_height(1.5)
            .width(Length::Fill)
    });
    if let Some(s) = &h.savings {
        let mut rate = row![
            txt(s.rate.clone(), 24.0, t.text)
                .font(fonts::INTER_LIGHT)
                .line_height(1.0)
                .wrapping(Wrapping::None)
        ]
        .spacing(10)
        .align_y(Alignment::End);
        if let Some(pace) = &s.pace {
            rate = rate.push(
                container(
                    txt(
                        pace.text.clone(),
                        13.0,
                        tone_color(t, pace.tone, t.text_muted),
                    )
                    .line_height(1.0),
                )
                .padding(Padding {
                    bottom: widgets::baseline_pad(24.0, 13.0),
                    ..Padding::ZERO
                }),
            );
        }
        let mut block = column![
            row![
                txt(s.title.clone(), 13.0, t.text_muted).width(Length::Fill),
                txt(s.target.clone(), 12.0, t.text_muted),
            ]
            .spacing(12)
            .align_y(Alignment::End),
            rate,
            column![
                widgets::bar(
                    Bar::Fraction {
                        value: s.fill,
                        color: t.neutral_400,
                        track: Some(t.neutral_800),
                        outlined: false
                    },
                    6.0,
                ),
                row![
                    txt(s.scale_start.clone(), 11.0, t.text_muted).width(Length::Fill),
                    txt(s.scale_end.clone(), 11.0, t.text_muted),
                ],
            ]
            .spacing(5),
        ]
        .spacing(10);
        if let Some(r) = reserves {
            block = block.push(r);
        }
        col = col.push(column![widgets::divider(t), block].spacing(16));
    } else if let Some(r) = reserves {
        col = col.push(r);
    }
    col.width(Length::Fill).into()
}

fn legend_line<'a>(
    t: &'static Tokens,
    color: Color,
    width: f32,
    dashed: bool,
    label: &str,
) -> Element<'a, Message> {
    row![
        canvas(SwatchProgram {
            color,
            width,
            dashed
        })
        .width(14)
        .height(4),
        txt(label.to_string(), 12.0, t.text_muted).wrapping(Wrapping::None),
    ]
    .spacing(6)
    .align_y(Alignment::Center)
    .into()
}

fn pace_panel(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let p = &app.dashboard.how_am_i.pace;
    let mut legend = row![legend_line(t, t.accent, 2.0, false, &p.legend_month)].spacing(16);
    if let Some(l) = &p.legend_forecast {
        legend = legend.push(legend_line(t, t.accent, 2.0, true, l));
    }
    if let Some(l) = &p.legend_normal {
        legend = legend.push(legend_line(t, t.neutral_500, 2.0, true, l));
    }
    if let Some(l) = &p.legend_income {
        legend = legend.push(legend_line(t, t.income, 1.0, false, l));
    }
    let s = &p.sentence;
    let mut spans = vec![sp(s.lead.clone(), t.neutral_300)];
    if let (Some(change), Some(tail)) = (&s.change, &s.tail) {
        spans.push(sp(" — ", t.neutral_300));
        spans.push(
            sp(change.text.clone(), tone_color(t, change.tone, t.text)).font(fonts::INTER_MEDIUM),
        );
        spans.push(sp(format!(" {tail}"), t.neutral_300));
    }
    column![
        row![
            row![txt(p.title.clone(), 13.0, t.text_muted), info(t, &p.info)]
                .spacing(2)
                .align_y(Alignment::Center)
                .width(Length::Fill),
            legend.wrap().vertical_spacing(6),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        rich_text(spans).size(16).line_height(1.45),
        container(
            canvas(PaceProgram {
                chart: p,
                today: app.dashboard.is_current,
                tokens: t,
                cache: &app.pace_cache,
            })
            .width(Length::Fill)
            .height(PACE_HEIGHT)
        )
        .padding(Padding {
            top: 6.0,
            ..Padding::ZERO
        }),
    ]
    .spacing(12)
    .into()
}

// ---- reserva e historico ----

fn reserve_body(app: &App, inner: f32) -> Element<'_, Message> {
    let t = app.tokens;
    let r = &app.dashboard.reserve;
    let left: Element<'_, Message> = match &r.emergency {
        EmergencyBlock::Present(e) => {
            let mut col = column![
                row![
                    txt(e.title.clone(), 13.0, t.text_muted).width(Length::Fill),
                    widgets::tag(t, TagTone::Accent, &e.chip),
                ]
                .align_y(Alignment::Center),
                widgets::value_line(
                    vec![
                        (e.coverage.clone(), 36.0, fonts::INTER_LIGHT, t.text),
                        (e.coverage_suffix.clone(), 14.0, fonts::INTER, t.text_muted),
                    ],
                    36.0,
                    8.0,
                ),
            ]
            .spacing(14);
            let mut segments = row![].spacing(4);
            let mut labels = row![].spacing(4);
            for (k, f) in e.segments.iter().enumerate() {
                segments = segments.push(
                    container(widgets::bar(
                        Bar::Fraction {
                            value: *f,
                            color: t.accent,
                            track: Some(t.neutral_800),
                            outlined: false,
                        },
                        5.0,
                    ))
                    .width(Length::FillPortion(1)),
                );
                let label = e.segment_labels.get(k).cloned().unwrap_or_default();
                labels = labels.push(
                    container(txt(label, 11.0, t.text_muted))
                        .width(Length::FillPortion(1))
                        .align_x(Alignment::End),
                );
            }
            col = col.push(column![segments, labels].spacing(6));
            if let Some(m) = &e.milestone {
                col = col.push(
                    rich_text([
                        sp(format!("{} ", m.label), t.neutral_300),
                        sp(m.target.clone(), t.text).font(fonts::INTER_MEDIUM),
                        sp(format!(" {}", m.missing), t.neutral_300),
                    ])
                    .size(15)
                    .line_height(1.5),
                );
            }
            if let Some(reached) = &e.reached {
                col = col.push(
                    txt(reached.clone(), 15.0, t.income_fg)
                        .font(fonts::INTER_MEDIUM)
                        .line_height(1.5),
                );
            }
            col.push(txt(e.goal_line.clone(), 12.0, t.text_muted).line_height(1.5))
                .into()
        }
        EmergencyBlock::Missing {
            title,
            message,
            hint,
        } => column![
            txt(title.clone(), 13.0, t.text_muted),
            txt(message.clone(), 15.0, t.text).line_height(1.5),
            txt(hint.clone(), 12.0, t.text_muted).line_height(1.5),
        ]
        .spacing(10)
        .into(),
    };
    let mut right = column![txt(r.month.title.clone(), 13.0, t.text_muted)].spacing(10);
    for (lv, color) in [
        (&r.month.deposited, t.income_fg),
        (&r.month.withdrawn, t.expense_fg),
    ] {
        if let Some(lv) = lv {
            right = right.push(
                row![
                    txt(lv.label.clone(), 14.0, t.text).width(Length::Fill),
                    txt(lv.value.clone(), 14.0, color).wrapping(Wrapping::None),
                ]
                .spacing(12),
            );
        }
    }
    if let Some(empty) = &r.month.empty {
        right = right.push(txt(empty.clone(), 14.0, t.text_muted));
    }
    if let EmergencyBlock::Present(e) = &r.emergency
        && let Some(eta) = &e.eta_line
    {
        right = right.push(txt(eta.clone(), 13.0, t.neutral_300).line_height(1.5));
    }
    for pot in &r.pots {
        let mut line = row![
            column![
                txt(pot.name.clone(), 14.0, t.text),
                txt(pot.meta.clone(), 12.0, t.text_muted).line_height(1.45),
            ]
            .spacing(3)
            .width(Length::Fill)
        ]
        .spacing(12);
        if let Some(pace) = &pot.pace {
            line = line.push(
                txt(
                    pace.text.clone(),
                    12.0,
                    tone_color(t, pace.tone, t.text_muted),
                )
                .wrapping(Wrapping::None),
            );
        }
        right = right.push(column![widgets::divider(t), line].spacing(12));
    }
    right = right.push(column![
        widgets::divider(t),
        link_button(t, &r.link, Message::SelectScreen(Screen::Reserves))
    ]);
    tiles(
        inner,
        36.0,
        24.0,
        vec![
            Tile {
                basis: 260.0,
                grow: 1.0,
                bg: None,
                padding: Padding::ZERO,
                content: left,
            },
            Tile {
                basis: 260.0,
                grow: 1.0,
                bg: None,
                padding: Padding::ZERO,
                content: right.into(),
            },
        ],
    )
}

fn history_body(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let h = &app.dashboard.history;
    let legend_item = |color: Color, label: &str| {
        row![
            swatch(color, 8.0, 2.0),
            txt(label.to_string(), 12.0, t.text_muted)
        ]
        .spacing(6)
        .align_y(Alignment::Center)
    };
    let header = row![
        txt(h.title.clone(), 13.0, t.text_muted).width(Length::Fill),
        row![
            legend_item(t.income, &h.legend_income),
            legend_item(t.expense, &h.legend_expense),
        ]
        .spacing(14),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    let mut col = column![header].spacing(16);
    let mut footnote = None;
    match &h.body {
        HistoryBody::List {
            lines,
            footnote: note,
        } => {
            for l in lines {
                col = col.push(
                    column![
                        row![
                            txt(l.label.clone(), 13.0, t.text).width(Length::Fill),
                            txt(l.text.clone(), 13.0, t.neutral_300).wrapping(Wrapping::None),
                        ]
                        .spacing(12),
                        widgets::bar(
                            Bar::Fraction {
                                value: l.income,
                                color: t.income,
                                track: None,
                                outlined: l.partial
                            },
                            6.0,
                        ),
                        widgets::bar(
                            Bar::Fraction {
                                value: l.expense,
                                color: t.expense,
                                track: None,
                                outlined: l.partial
                            },
                            6.0,
                        ),
                    ]
                    .spacing(7),
                );
            }
            footnote = Some(note.clone());
        }
        HistoryBody::Chart(chart) => {
            col = col.push(
                canvas(HistoryProgram {
                    chart,
                    tokens: t,
                    cache: &app.history_cache,
                })
                .width(Length::Fill)
                .height(HISTORY_HEIGHT),
            );
        }
    }
    let a = &h.average;
    let mut spans = vec![sp(a.label.clone(), t.neutral_300)];
    if let Some(value) = &a.value {
        spans.push(sp(format!(" {value}"), t.text).font(fonts::INTER_MEDIUM));
    }
    if let Some(suffix) = &a.suffix {
        spans.push(sp(format!(" {suffix}"), t.text_muted));
    }
    let mut foot = column![rich_text(spans).size(13).line_height(1.5)].spacing(6);
    if let Some(note) = footnote.filter(|n| !n.is_empty()) {
        foot = foot.push(txt(note, 12.0, t.text_muted).line_height(1.5));
    }
    col.push(column![widgets::divider(t), foot].spacing(14))
        .into()
}

// ---- para onde foi ----

fn where_it_went(app: &App, width: f32) -> Element<'_, Message> {
    let t = app.tokens;
    let w = &app.dashboard.where_it_went;
    let plan = &w.plan;
    let mut plan_col = column![
        row![
            txt(plan.title.clone(), 13.0, t.text_muted).width(Length::Fill),
            txt(
                plan.income_label.clone().unwrap_or_default(),
                12.0,
                t.text_muted
            ),
        ]
        .spacing(8)
        .align_y(Alignment::End)
    ]
    .spacing(12);
    let colors = match plan.bar {
        Some(_) => vec![t.neutral_400, t.neutral_600, t.income],
        // Sem renda fixa so sobra "Gasto sem meta".
        None => vec![t.neutral_600],
    };
    if let Some(bar) = plan.bar {
        let parts: Vec<(f64, Color)> = bar
            .iter()
            .zip(&colors)
            .map(|(f, color)| (f64::from(f.clamp(0.0, 1.0)), *color))
            .collect();
        plan_col = plan_col.push(widgets::bar(
            Bar::Parts {
                parts: &parts,
                track: t.neutral_800,
            },
            8.0,
        ));
    }
    let mut legend = row![].spacing(28);
    for (label, color) in plan.legend.iter().zip(&colors) {
        legend = legend.push(
            row![
                swatch(*color, 8.0, 2.0),
                txt(label.clone(), 13.0, t.neutral_300).wrapping(Wrapping::None)
            ]
            .spacing(7)
            .align_y(Alignment::Center),
        );
    }
    plan_col = plan_col
        .push(legend.wrap().vertical_spacing(6))
        .push(txt(plan.message.clone(), 14.0, t.text).line_height(1.5));

    let mut blocks = Vec::new();
    if let Some(a) = &w.attention {
        let mut col = column![
            row![
                txt(a.title.clone(), 13.0, t.text_muted).width(Length::Fill),
                txt(a.days_left.clone().unwrap_or_default(), 12.0, t.text_muted),
            ]
            .height(36)
            .align_y(Alignment::Center)
        ];
        for r in &a.rows {
            let tone = tone_color(t, r.tone, t.text_muted);
            let fill = match r.tone {
                Tone::Expense => t.expense,
                Tone::Warning => t.warning,
                _ => t.neutral_400,
            };
            col = col.push(category_row(
                t,
                CategoryRow {
                    icon: &r.icon,
                    color: &r.color,
                    name: &r.name,
                    above_normal: r.above_normal.as_deref(),
                    amount: row![
                        txt(r.spent.clone(), 14.0, t.text).wrapping(Wrapping::None),
                        container(
                            txt(r.limit.clone(), 12.0, t.text_muted).wrapping(Wrapping::None)
                        )
                        .padding(Padding {
                            bottom: 1.0,
                            ..Padding::ZERO
                        }),
                    ]
                    .spacing(4)
                    .align_y(Alignment::End)
                    .into(),
                    meter: MeterProgram {
                        fill: r.fill,
                        fill_color: fill,
                        track: t.neutral_800,
                        bar: 4.0,
                        marks: vec![(r.marker, String::new())],
                        mark_color: t.neutral_300,
                        label_color: t.text_muted,
                    },
                    note: txt(r.status.clone(), 12.0, tone).wrapping(Wrapping::None),
                    note_width: 164.0,
                },
            ));
        }
        blocks.push(Tile {
            basis: 360.0,
            grow: 1.0,
            bg: None,
            padding: Padding::ZERO,
            content: col.into(),
        });
    }
    let mut others = column![
        row![
            txt(w.others_title.clone(), 13.0, t.text_muted).width(Length::Fill),
            info(t, &w.others_info),
        ]
        .height(36)
        .align_y(Alignment::Center)
    ];
    for c in &w.others {
        others = others.push(category_row(
            t,
            CategoryRow {
                icon: &c.icon,
                color: &c.color,
                name: &c.name,
                above_normal: c.above_normal.as_deref(),
                amount: txt(c.amount.clone(), 14.0, t.text)
                    .wrapping(Wrapping::None)
                    .into(),
                meter: MeterProgram {
                    fill: c.fill,
                    fill_color: t.neutral_400,
                    track: t.neutral_800,
                    bar: 4.0,
                    marks: c.marker.map(|m| (m, String::new())).into_iter().collect(),
                    mark_color: t.neutral_300,
                    label_color: t.text_muted,
                },
                note: txt(c.note.clone(), 12.0, t.text_muted).wrapping(Wrapping::None),
                note_width: 96.0,
            },
        ));
    }
    blocks.push(Tile {
        basis: 360.0,
        grow: 1.0,
        bg: None,
        padding: Padding::ZERO,
        content: others.into(),
    });
    let body = column![
        container(plan_col).padding(Padding {
            bottom: 18.0,
            ..Padding::ZERO
        }),
        widgets::divider(t),
        container(tiles(width - 40.0, 44.0, 8.0, blocks)).padding(Padding {
            top: 8.0,
            ..Padding::ZERO
        }),
    ];
    column![
        widgets::section_heading(t, &w.title, &w.question, Some(&w.summary), 20.0),
        widgets::card(t, body).padding(Padding {
            top: 20.0,
            right: 20.0,
            bottom: 12.0,
            left: 20.0,
        }),
    ]
    .spacing(GAP)
    .into()
}

/// Uma linha de categoria (em atenção ou demais): tile, nome com o chip "↑ do normal", valor,
/// medidor e a nota a direita.
struct CategoryRow<'a, 'b> {
    icon: &'b str,
    color: &'b str,
    name: &'b str,
    above_normal: Option<&'b str>,
    amount: Element<'a, Message>,
    meter: MeterProgram,
    note: text::Text<'a>,
    note_width: f32,
}

fn category_row<'a>(t: &'static Tokens, r: CategoryRow<'a, '_>) -> Element<'a, Message> {
    let mut name = row![txt(r.name.to_string(), 14.0, t.text)]
        .spacing(10)
        .align_y(Alignment::Center);
    if let Some(chip) = r.above_normal {
        name = name.push(widgets::tag(t, TagTone::Neutral, chip));
    }
    let token = (!r.color.is_empty()).then_some(r.color);
    container(
        row![
            match token {
                Some(tk) => widgets::token_tile(t, icons::glyph(r.icon), tk, 30.0),
                None => widgets::neutral_tile(t, icons::glyph(r.icon), 30.0),
            },
            column![
                row![container(name).width(Length::Fill), r.amount]
                    .spacing(12)
                    .align_y(Alignment::Center),
                row![
                    container(meter(r.meter))
                        .width(Length::Fill)
                        .align_y(Alignment::Center),
                    container(r.note)
                        .width(r.note_width)
                        .align_x(Alignment::End),
                ]
                .spacing(12)
                .align_y(Alignment::Center),
            ]
            .spacing(8)
            .width(Length::Fill),
        ]
        .spacing(14)
        .align_y(Alignment::Center),
    )
    .padding([10, 0])
    .into()
}

// ---- o que vem ----

fn whats_next(app: &App, width: f32) -> Element<'_, Message> {
    let t = app.tokens;
    let Some(w) = &app.dashboard.whats_next else {
        return Space::new().into();
    };
    let surface = Some((t.surface, 8.0));
    column![
        widgets::section_heading(t, &w.title, &w.question, None, 0.0),
        tiles(
            width,
            GAP,
            GAP,
            vec![
                Tile {
                    basis: 280.0,
                    grow: 1.0,
                    bg: surface,
                    padding: Padding::new(20.0),
                    content: free_card(app),
                },
                Tile {
                    basis: 280.0,
                    grow: 1.0,
                    bg: surface,
                    padding: Padding {
                        top: 20.0,
                        right: 20.0,
                        bottom: 8.0,
                        left: 20.0,
                    },
                    content: upcoming_card(app),
                },
            ],
        ),
    ]
    .spacing(GAP)
    .into()
}

fn free_card(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let Some(w) = &app.dashboard.whats_next else {
        return Space::new().into();
    };
    let f = &w.free;
    let mut col = column![txt(f.title.clone(), 13.0, t.text_muted)].spacing(14);
    if let Some(b) = &f.amount {
        let whole = if f.amount_negative {
            t.expense_fg
        } else {
            t.text
        };
        let prefix = format!(
            "{}{}{}",
            if f.approx { "~ " } else { "" },
            b.sign,
            b.currency
        );
        let mut pieces = vec![
            (prefix, 16.0, fonts::INTER, t.text_muted),
            (b.whole.clone(), 36.0, fonts::INTER_LIGHT, whole),
            (b.cents.clone(), 16.0, fonts::INTER, t.text_muted),
        ];
        if let Some(of) = &f.of_income {
            pieces.push((format!(" {of}"), 13.0, fonts::INTER, t.text_muted));
        }
        col = col.push(widgets::value_line(pieces, 36.0, 6.0));
        col = col.push(
            container(meter(MeterProgram {
                fill: f.committed_fill,
                fill_color: t.neutral_400,
                track: t.neutral_800,
                bar: 6.0,
                marks: f.marks.clone(),
                mark_color: t.neutral_500,
                label_color: t.text_muted,
            }))
            .padding(Padding {
                top: 4.0,
                ..Padding::ZERO
            }),
        );
    }
    let mut committed = vec![sp(f.committed.clone(), t.neutral_300)];
    if let Some(p) = &f.committed_percent {
        committed.push(sp(format!(" {p}"), t.text).font(fonts::INTER_MEDIUM));
    }
    col = col.push(rich_text(committed).size(14).line_height(1.5));
    if let Some(r) = &f.reference {
        col = col.push(txt(r.clone(), 12.0, t.text_muted).line_height(1.5));
    }
    if let Some(i) = &f.invite {
        col = col.push(txt(i.clone(), 14.0, t.neutral_300).line_height(1.5));
    }
    let v = &w.variable;
    col = col.push(
        container(
            column![
                txt(v.title.clone(), 12.0, t.text_muted),
                txt(v.text.clone(), 14.0, t.text).line_height(1.5),
            ]
            .spacing(6),
        )
        .padding([14, 16])
        .width(Length::Fill)
        .style(theme::fill(t.surface_2, 6.0)),
    );
    if let Some(n) = &f.estimate_note {
        col = col.push(txt(n.clone(), 12.0, t.text_muted).line_height(1.5));
    }
    col.into()
}

fn upcoming_row<'a>(t: &'static Tokens, u: &UpcomingLine) -> Element<'a, Message> {
    let glyph = match &u.color {
        Some(_) => icons::glyph(&u.icon),
        None => icons::REPEAT,
    };
    let amount_color = if u.income { t.income_fg } else { t.text };
    column![
        row![
            match u.color.as_deref() {
                Some(tk) => widgets::token_tile(t, glyph, tk, 32.0),
                None => widgets::neutral_tile(t, glyph, 32.0),
            },
            column![
                txt(u.description.clone(), 14.0, t.text),
                txt(u.when.clone(), 12.0, t.text_muted),
            ]
            .spacing(3)
            .width(Length::Fill),
            txt(u.amount.clone(), 14.0, amount_color).wrapping(Wrapping::None),
        ]
        .spacing(12)
        .padding([12, 0])
        .align_y(Alignment::Center),
        widgets::divider(t),
    ]
    .into()
}

fn upcoming_card(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let Some(w) = &app.dashboard.whats_next else {
        return Space::new().into();
    };
    let u = &w.upcoming;
    let mut col = column![
        row![
            txt(u.title.clone(), 13.0, t.text_muted).width(Length::Fill),
            txt(u.window.clone(), 12.0, t.text_muted),
        ]
        .spacing(12)
        .padding(Padding {
            bottom: 12.0,
            ..Padding::ZERO
        })
    ];
    for inv in &u.invoices {
        let mut note = Vec::new();
        if let Some(p) = &inv.percent {
            note.push(sp(
                format!("{} ", p.text),
                tone_color(t, p.tone, t.neutral_300),
            ));
        }
        note.push(sp(inv.note.clone(), t.neutral_300));
        col = col.push(
            container(
                column![
                    row![
                        container(icons::icon(icons::glyph(&inv.icon), 16.0, t.accent_400))
                            .center(32)
                            .style(theme::fill(t.accent_900, 6.0)),
                        column![
                            txt(inv.title.clone(), 14.0, t.text).font(fonts::INTER_MEDIUM),
                            txt(inv.when.clone(), 12.0, t.text_muted),
                        ]
                        .spacing(3)
                        .width(Length::Fill),
                        txt(inv.amount.clone(), 16.0, t.text).wrapping(Wrapping::None),
                    ]
                    .spacing(12)
                    .align_y(Alignment::Center),
                    rich_text(note).size(12).line_height(1.5),
                ]
                .spacing(10),
            )
            .padding(14)
            .width(Length::Fill)
            .style(theme::fill(t.surface_2, 6.0)),
        );
    }
    let shown = if app.upcoming_expanded {
        u.items.len()
    } else {
        u.preview.min(u.items.len())
    };
    for item in u.items.iter().take(shown) {
        col = col.push(upcoming_row(t, item));
    }
    if let Some(empty) = &u.empty {
        col = col.push(container(txt(empty.clone(), 13.0, t.text_muted)).padding([12, 0]));
    }
    if let Some(all) = &u.see_all
        && !app.upcoming_expanded
    {
        col = col.push(link_button(t, all, Message::ExpandUpcoming));
    }
    col.spacing(0).into()
}

// ---- ultimos lancamentos e quem lancou ----

fn recent(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let r = &app.dashboard.recent;
    let mut list = column![];
    for (k, l) in r.lines.iter().enumerate() {
        let mut title = row![txt(l.description.clone(), 14.0, t.text)]
            .spacing(8)
            .align_y(Alignment::Center);
        if let Some(chip) = &l.chip {
            title = title.push(widgets::tag(t, TagTone::Accent, chip));
        }
        if let Some(tag) = &l.tag {
            // `text_soft`: o `text_muted` sobre o divisor fica abaixo de 4,5:1 no claro.
            title = title.push(widgets::tag(t, TagTone::Neutral, tag));
        }
        let mut meta = row![].spacing(6).align_y(Alignment::Center);
        if let Some(dot) = &l.dot {
            meta = meta.push(swatch(t.token(dot), 6.0, 3.0));
        }
        meta = meta.push(txt(l.meta.clone(), 12.0, t.text_muted));
        let color = if l.excluded {
            t.text_muted
        } else if l.income {
            t.income_fg
        } else {
            t.text
        };
        if k > 0 {
            list = list.push(widgets::divider(t));
        }
        list = list.push(
            row![
                column![title.wrap().vertical_spacing(4), meta]
                    .spacing(4)
                    .width(Length::Fill),
                txt(l.amount.clone(), 14.0, color).wrapping(Wrapping::None),
            ]
            .spacing(12)
            .padding([11, 0])
            .align_y(Alignment::Center),
        );
    }
    if let Some(empty) = &r.empty {
        list = list.push(container(txt(empty.clone(), 13.0, t.text_muted)).padding([11, 0]));
    }
    let foot = row![
        container(link_button(
            t,
            &r.link,
            Message::SelectScreen(Screen::Transactions)
        ))
        .width(Length::Fill),
        info(t, &r.info),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    column![
        widgets::section_heading(t, &r.title, "", Some(&r.count), 0.0),
        widgets::card(t, column![list, widgets::divider(t), foot]).padding(Padding {
            top: 10.0,
            right: 20.0,
            bottom: 8.0,
            left: 20.0,
        }),
    ]
    .spacing(GAP)
    .into()
}

fn people(app: &App) -> Option<Element<'_, Message>> {
    let t = app.tokens;
    let p = app.dashboard.people.as_ref()?;
    let mut col = column![txt(p.title.clone(), 13.0, t.text_muted)].spacing(14);
    for person in &p.rows {
        let (bg, fg) = match &person.color {
            Some(tk) => (t.token(tk), t.bg),
            None => t.avatar_neutral(),
        };
        col = col.push(
            row![
                container(
                    txt(person.initial.clone(), 12.0, fg)
                        .font(fonts::INTER_SEMIBOLD)
                        .line_height(1.0)
                )
                .center(28)
                .style(theme::fill(bg, 14.0)),
                column![
                    txt(person.name.clone(), 14.0, t.text),
                    txt(person.line.clone(), 12.0, t.text_muted).line_height(1.45),
                ]
                .spacing(3)
                .width(Length::Fill),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        );
    }
    col = col.push(txt(p.footnote.clone(), 12.0, t.text_muted).line_height(1.5));
    Some(widgets::card(t, col).padding(Padding::new(20.0)).into())
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
    let mut copy = column![
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
        .color(t.text_muted),
        Space::new().height(16),
        row![
            swatch(t.warning, 7.0, 3.5),
            txt(waiting, 12.0, t.text_muted),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    ]
    .max_width(460);
    if let Some(connect) = shell::connect_phone_button(app) {
        copy = copy.push(Space::new().height(20)).push(connect);
    }
    widgets::card(t, row![sketch, copy].spacing(56).align_y(Alignment::Center))
        .padding([56, 32])
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
    if v.is_empty && v.ignored_total == 0 {
        return None;
    }
    let updated = app
        .snapshot
        .dashboard
        .refreshed_at
        .map(|at| time::format_time(OffsetDateTime::from(at), app.offset));
    Some(
        container(
            txt(
                footer_text(
                    updated.as_deref(),
                    shell::transactions_count(app),
                    v.ignored_total,
                ),
                12.0,
                app.tokens.text_muted,
            )
            .width(Length::Fill),
        )
        .padding([0, 2])
        .into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn flex_lines_reparte_a_sobra_e_quebra_como_o_css() {
        // Como estou a 1100: card de 1088 por dentro, 300 / 520 com crescimento 1 / 2.
        assert_eq!(
            flex_lines(1088.0, 16.0, &[(300.0, 1.0), (520.0, 2.0)]),
            vec![vec![(0, 384.0), (1, 688.0)]]
        );
        // Nao cabe: cada um na sua linha, ocupando a largura toda.
        assert_eq!(
            flex_lines(800.0, 16.0, &[(300.0, 1.0), (520.0, 2.0)]),
            vec![vec![(0, 800.0)], vec![(1, 800.0)]]
        );
        // Item maior que a linha sozinho: encolhe ate a linha.
        assert_eq!(
            flex_lines(200.0, 16.0, &[(260.0, 1.0)]),
            vec![vec![(0, 200.0)]]
        );
        assert!(flex_lines(500.0, 16.0, &[]).is_empty());
    }

    #[test]
    fn duas_colunas_a_partir_de_1100_na_ordem_da_spec() {
        use Section::*;
        let current = [
            HowAmI,
            Reserve,
            History,
            WhereItWent,
            WhatsNext,
            Recent,
            People,
        ];
        assert_eq!(
            bands(&current, 1100.0),
            vec![
                Band::Full(HowAmI),
                Band::Split(vec![Reserve], vec![History]),
                Band::Full(WhereItWent),
                Band::Split(vec![WhatsNext], vec![Recent, People]),
            ]
        );
        // Abaixo de 1100: uma coluna, mesma ordem.
        let one: Vec<Band> = current.iter().map(|s| Band::Full(*s)).collect();
        assert_eq!(bands(&current, 1099.0), one);
        // Mes fechado: "O que vem" some e os ultimos dividem a linha com quem lancou.
        let closed = [HowAmI, Reserve, History, WhereItWent, Recent, People];
        assert_eq!(
            bands(&closed, 1400.0).last(),
            Some(&Band::Split(vec![Recent], vec![People]))
        );
        // Uma pessoa so: os ultimos na linha toda.
        let alone = [HowAmI, Reserve, History, WhereItWent, Recent];
        assert_eq!(bands(&alone, 1400.0).last(), Some(&Band::Full(Recent)));
        // Conta comercial: Negócio primeiro.
        let business = [
            Business,
            HowAmI,
            Reserve,
            History,
            WhereItWent,
            WhatsNext,
            Recent,
        ];
        let b = bands(&business, 1400.0);
        assert_eq!(b[0], Band::Full(Business));
        assert_eq!(b[4], Band::Split(vec![WhatsNext], vec![Recent]));
        assert_eq!(bands(&[Empty], 1400.0), vec![Band::Full(Empty)]);
        // O respiro de 20 e da faixa: os dois titulos lado a lado comecam juntos.
        assert_eq!(band_top(&Band::Split(vec![WhatsNext], vec![Recent])), 20.0);
        assert_eq!(band_top(&Band::Split(vec![Recent], vec![People])), 20.0);
        assert_eq!(band_top(&Band::Split(vec![Reserve], vec![History])), 0.0);
        assert_eq!(band_top(&Band::Full(People)), 0.0);
    }
}
