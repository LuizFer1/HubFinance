//! Graficos em `canvas`: rosca de gastos por categoria, barras de receita x despesa e o
//! esqueleto tracejado do estado "Sem dados" (Dashboard); barras empilhadas da evolucao e o
//! medidor tracejado do estado vazio (Reservas).
//!
//! A geometria fica num `canvas::Cache` que a `App` limpa quando dados, mes, hover da legenda
//! ou tema mudam; o hover das barras e estado do proprio `Program` e e desenhado num segundo
//! `Frame`, sem cache, para passar o mouse nao refazer o grafico inteiro.

use std::f32::consts::{FRAC_PI_2, TAU};

use iced::alignment::Vertical;
use iced::border::Radius;
use iced::mouse;
use iced::widget::canvas::{
    self, Action, Event, Frame, Geometry, LineDash, Path, Stroke, Text, path::Arc,
};
use iced::widget::text::{Alignment as TextAlign, LineHeight, Shaping};
use iced::{Color, Pixels, Point, Radians, Rectangle, Renderer, Size, Theme};

use super::app::Message;
use super::fonts;
use super::theme::{Tokens, alpha};
use crate::dashboard::aggregate::{MonthTotals, Slice};
use crate::dashboard::money::{axis_label, format_brl, signed_brl};
use crate::dashboard::periods::{month_label, month_label_capitalized, month_short};
use crate::dashboard::reserves::{ReserveColor, ReserveSeries};

/// Texto de canvas na Inter Hub (o canvas nao herda a fonte padrao da aplicacao).
fn label(content: String, position: Point, size: f32, color: Color) -> Text {
    Text {
        content,
        position,
        color,
        size: Pixels(size),
        line_height: LineHeight::Relative(1.3),
        font: fonts::INTER,
        align_x: TextAlign::Center,
        align_y: Vertical::Center,
        shaping: Shaping::Advanced,
        ..Text::default()
    }
}

/// Corta com reticencias pelo numero de caracteres: o canvas nao mede texto, e o miolo da
/// rosca tem largura fixa.
fn clip_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

// ---- rosca ----

/// Raio e traco no `viewBox` 100 do prototipo; folga de 1,2 u de circunferencia entre fatias.
const DONUT_RADIUS: f32 = 40.0;
const DONUT_STROKE: f32 = 11.0;
const DONUT_STROKE_HOVER: f32 = 13.0;
const DONUT_GAP: f32 = 1.2 / DONUT_RADIUS;

pub struct DonutProgram<'a> {
    pub slices: &'a [Slice],
    pub total_minor: i64,
    /// Chave da fatia sob o mouse na legenda.
    pub hover: Option<&'a str>,
    pub tokens: &'static Tokens,
    pub cache: &'a canvas::Cache,
}

/// Angulos (inicio, fim) de cada fatia, em radianos, a partir das 12 h no sentido horario. A
/// folga fica no fim de cada fatia, limitada ao tamanho dela; com fatia unica nao ha folga
/// (o anel fecha).
pub fn donut_angles(shares: &[f64]) -> Vec<(f32, f32)> {
    let mut start = -FRAC_PI_2;
    let gap_on = shares.len() > 1;
    shares
        .iter()
        .map(|share| {
            let sweep = (*share as f32).clamp(0.0, 1.0) * TAU;
            let gap = if gap_on { DONUT_GAP.min(sweep) } else { 0.0 };
            let arc = (start, start + sweep - gap);
            start += sweep;
            arc
        })
        .collect()
}

impl canvas::Program<Message> for DonutProgram<'_> {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let t = self.tokens;
        vec![self.cache.draw(renderer, bounds.size(), |frame| {
            let s = bounds.width.min(bounds.height) / 100.0;
            let center = frame.center();
            let radius = DONUT_RADIUS * s;
            frame.stroke(
                &Path::circle(center, radius),
                Stroke::default()
                    .with_color(t.neutral_900)
                    .with_width(DONUT_STROKE * s),
            );
            let shares: Vec<f64> = self.slices.iter().map(|sl| sl.share).collect();
            for (slice, (start, end)) in self.slices.iter().zip(donut_angles(&shares)) {
                if end - start <= f32::EPSILON {
                    continue;
                }
                let active = self.hover == Some(slice.key.as_str());
                let faded = self.hover.is_some() && !active;
                let color = t.token(&slice.color);
                let color = if faded { alpha(color, 0.28) } else { color };
                let width = if active {
                    DONUT_STROKE_HOVER
                } else {
                    DONUT_STROKE
                } * s;
                let path = if end - start >= TAU - 1e-4 {
                    Path::circle(center, radius)
                } else {
                    Path::new(|b| {
                        b.arc(Arc {
                            center,
                            radius,
                            start_angle: Radians(start),
                            end_angle: Radians(end),
                        });
                    })
                };
                frame.stroke(&path, Stroke::default().with_color(color).with_width(width));
            }
            let (top, middle, bottom) =
                donut_center_lines(self.slices, self.total_minor, self.hover);
            let muted = t.text_alpha(0.55);
            frame.fill_text(label(
                clip_chars(&top, 20),
                Point::new(center.x, center.y - 19.0),
                11.0,
                muted,
            ));
            frame.fill_text(Text {
                font: fonts::INTER_MEDIUM,
                ..label(middle, Point::new(center.x, center.y + 1.0), 17.0, t.text)
            });
            frame.fill_text(label(
                bottom,
                Point::new(center.x, center.y + 19.0),
                11.0,
                muted,
            ));
        })]
    }
}

/// Miolo da rosca: "Despesas" / total / "N categorias"; com hover, nome / valor / "X% do mês".
pub fn donut_center_lines(
    slices: &[Slice],
    total_minor: i64,
    hover: Option<&str>,
) -> (String, String, String) {
    match hover.and_then(|key| slices.iter().find(|s| s.key == key)) {
        Some(slice) => (
            slice.name.clone(),
            format_brl(slice.amount_minor),
            format!("{}% do mês", percent(slice.share)),
        ),
        None => (
            "Despesas".to_string(),
            format_brl(total_minor),
            categories_label(slices.len()),
        ),
    }
}

/// "1 categoria" / "4 categorias".
pub fn categories_label(n: usize) -> String {
    if n == 1 {
        "1 categoria".to_string()
    } else {
        format!("{n} categorias")
    }
}

/// Porcentagem inteira como o `Math.round(f * 100)` do prototipo.
pub fn percent(share: f64) -> i64 {
    (share * 100.0 + 0.5).floor() as i64
}

// ---- barras ----

/// Coluna do eixo Y, faixa dos rotulos dos meses e respiro no topo para o rotulo de cima.
/// Mais larga que os 52 do design: no HTML o rotulo transborda para o padding do card, no
/// canvas ele e cortado na borda. 68 cabe ate "R$ 120 mil" com o respiro.
const AXIS_W: f32 = 68.0;
/// Respiro entre o fim do rotulo do eixo e a area das barras.
const AXIS_PAD: f32 = 8.0;
const LABELS_H: f32 = 24.0;
const TOP: f32 = 6.0;
const BAR_W: f32 = 14.0;
const BAR_GAP: f32 = 4.0;

pub struct BarsProgram<'a> {
    pub bars: &'a [MonthTotals],
    pub axis_top: i64,
    /// Indice do mes escolhido no seletor (o ultimo das seis barras).
    pub selected: usize,
    pub tokens: &'static Tokens,
    pub cache: &'a canvas::Cache,
}

/// Area util das barras dentro do canvas.
pub fn plot_area(size: Size) -> Rectangle {
    Rectangle {
        x: AXIS_W,
        y: TOP,
        width: (size.width - AXIS_W).max(0.0),
        height: (size.height - LABELS_H - TOP).max(0.0),
    }
}

/// Coluna sob o ponto (coordenadas do canvas); fora da area util -> `None`.
pub fn bar_column(point: Point, size: Size, columns: usize) -> Option<usize> {
    let plot = plot_area(size);
    if columns == 0 || plot.width <= 0.0 {
        return None;
    }
    // Faixa dos rotulos conta como a coluna: o hover do prototipo e na coluna inteira.
    if point.x < plot.x || point.x >= plot.x + plot.width || point.y < 0.0 || point.y > size.height
    {
        return None;
    }
    let col = ((point.x - plot.x) / (plot.width / columns as f32)).floor() as usize;
    Some(col.min(columns - 1))
}

/// Altura de uma barra: proporcional ao topo do eixo, sem passar dele nem ficar negativa.
fn bar_height(value: i64, top: i64, plot_h: f32) -> f32 {
    if top <= 0 {
        return 0.0;
    }
    ((value as f32 / top as f32).clamp(0.0, 1.0)) * plot_h
}

impl canvas::Program<Message> for BarsProgram<'_> {
    /// Mes sob o cursor.
    type State = Option<usize>;

    fn update(
        &self,
        state: &mut Option<usize>,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<Message>> {
        if !matches!(event, Event::Mouse(_)) {
            return None;
        }
        let hovered = cursor
            .position_in(bounds)
            .and_then(|p| bar_column(p, bounds.size(), self.bars.len()));
        if hovered == *state {
            return None;
        }
        *state = hovered;
        Some(Action::request_redraw())
    }

    fn draw(
        &self,
        state: &Option<usize>,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let t = self.tokens;
        let size = bounds.size();
        let plot = plot_area(size);
        let columns = self.bars.len().max(1);
        let col_w = plot.width / columns as f32;
        let chart = self.cache.draw(renderer, size, |frame| {
            let grid = t.divider();
            let muted = t.text_alpha(0.50);
            let marks = [
                (plot.y, axis_label(self.axis_top)),
                (plot.y + plot.height / 2.0, axis_label(self.axis_top / 2)),
                (plot.y + plot.height, axis_label(0)),
            ];
            for (y, text) in marks {
                frame.fill_rectangle(
                    Point::new(plot.x, y.round() - 0.5),
                    Size::new(plot.width, 1.0),
                    grid,
                );
                frame.fill_text(Text {
                    align_x: TextAlign::Right,
                    ..label(text, Point::new(AXIS_W - AXIS_PAD, y), 11.0, muted)
                });
            }
            for (i, bar) in self.bars.iter().enumerate() {
                let selected = i == self.selected;
                let opacity = if selected { 1.0 } else { 0.55 };
                let x0 = plot.x + i as f32 * col_w + (col_w - 2.0 * BAR_W - BAR_GAP) / 2.0;
                let base = plot.y + plot.height;
                for (k, (value, color)) in
                    [(bar.income_minor, t.income), (bar.expense_minor, t.expense)]
                        .into_iter()
                        .enumerate()
                {
                    let h = bar_height(value, self.axis_top, plot.height);
                    if h <= 0.0 {
                        continue;
                    }
                    let x = x0 + k as f32 * (BAR_W + BAR_GAP);
                    frame.fill(
                        &Path::rounded_rectangle(
                            Point::new(x, base - h),
                            Size::new(BAR_W, h),
                            Radius {
                                top_left: 3.0,
                                top_right: 3.0,
                                bottom_right: 0.0,
                                bottom_left: 0.0,
                            },
                        ),
                        alpha(color, opacity),
                    );
                }
                let month = Text {
                    font: if selected {
                        fonts::INTER_MEDIUM
                    } else {
                        fonts::INTER
                    },
                    ..label(
                        month_short(&bar.month).to_string(),
                        Point::new(plot.x + (i as f32 + 0.5) * col_w, size.height - 8.0),
                        12.0,
                        if selected { t.text } else { t.text_alpha(0.55) },
                    )
                };
                frame.fill_text(month);
            }
        });
        let mut geometries = vec![chart];
        if let Some(i) = *state
            && let Some(bar) = self.bars.get(i)
        {
            let mut overlay = Frame::new(renderer, size);
            draw_bar_hover(&mut overlay, t, bar, i, col_w, plot, self.axis_top, size);
            geometries.push(overlay.into_geometry());
        }
        geometries
    }
}

/// Fundo a 5 % na coluna e o tooltip.
#[allow(clippy::too_many_arguments)]
fn draw_bar_hover(
    frame: &mut Frame,
    t: &'static Tokens,
    bar: &MonthTotals,
    i: usize,
    col_w: f32,
    plot: Rectangle,
    top: i64,
    size: Size,
) {
    let col_x = plot.x + i as f32 * col_w;
    frame.fill(
        &Path::rounded_rectangle(
            Point::new(col_x, plot.y),
            Size::new(col_w, plot.height),
            Radius {
                top_left: 6.0,
                top_right: 6.0,
                bottom_right: 0.0,
                bottom_left: 0.0,
            },
        ),
        t.text_alpha(0.05),
    );
    let lines = [
        (month_label(&bar.month), t.text, fonts::INTER_MEDIUM),
        (
            format!("+{}", format_brl(bar.income_minor)),
            t.income_fg,
            fonts::INTER,
        ),
        (
            format!("\u{2212}{}", format_brl(bar.expense_minor)),
            t.expense_fg,
            fonts::INTER,
        ),
        (
            format!("Sobrou {}", signed_brl(bar.net_minor())),
            t.text_alpha(0.60),
            fonts::INTER,
        ),
    ];
    // Largura estimada (o canvas nao mede texto): ~7 px por caractere em 12 px tabular.
    let longest = lines
        .iter()
        .map(|(s, _, _)| s.chars().count())
        .max()
        .unwrap_or(0);
    let line_h = 15.0;
    let gap = 3.0;
    let box_w = longest as f32 * 7.0 + 20.0;
    let box_h = 4.0 * line_h + 3.0 * gap + 16.0;
    let tallest = bar_height(bar.income_minor.max(bar.expense_minor), top, plot.height);
    let bar_top = plot.y + plot.height - tallest;
    let (x, y) = tooltip_origin(
        Size::new(box_w, box_h),
        col_x,
        col_w,
        bar_top,
        plot.y,
        size.width,
    );
    let rect = Path::rounded_rectangle(Point::new(x, y), Size::new(box_w, box_h), 8.0.into());
    // Fundo `bg` como o prototipo (`var(--color-bg)`), contorno de 1 px do `shadow-md`; o
    // canvas nao tem sombra.
    frame.fill(&rect, t.bg);
    frame.stroke(
        &rect,
        Stroke::default()
            .with_color(t.shadow_md_ring())
            .with_width(1.0),
    );
    for (k, (content, color, font)) in lines.into_iter().enumerate() {
        let ly = y + 8.0 + k as f32 * (line_h + gap) + line_h / 2.0;
        frame.fill_text(Text {
            align_x: TextAlign::Left,
            font,
            ..label(content, Point::new(x + 10.0, ly), 12.0, color)
        });
    }
}

/// Onde fica o tooltip (canto superior esquerdo). O do prototipo sai por cima do grafico; o
/// canvas recorta no proprio limite, entao: acima das barras da coluna quando cabe; senao ao
/// lado da coluna (a direita, ou a esquerda perto da borda), no topo da area util, para nao
/// cobrir as barras do mes que se esta lendo.
pub fn tooltip_origin(
    tip: Size,
    col_x: f32,
    col_w: f32,
    bar_top: f32,
    plot_top: f32,
    width: f32,
) -> (f32, f32) {
    let above = bar_top - 6.0 - tip.height;
    if above >= 0.0 {
        let x = (col_x + col_w / 2.0 - tip.width / 2.0).clamp(0.0, (width - tip.width).max(0.0));
        return (x, above);
    }
    let right = col_x + col_w + 4.0;
    let x = if right + tip.width <= width {
        right
    } else {
        (col_x - 4.0 - tip.width).max(0.0)
    };
    (x, plot_top)
}

// ---- esqueleto vazio ----

/// Rosca de 128 px + 28 de espaco + quatro barras de 14 com 8 entre elas.
pub const EMPTY_WIDTH: f32 = 128.0 + 28.0 + 80.0;
pub const EMPTY_HEIGHT: f32 = 128.0;

pub struct EmptyChartProgram<'a> {
    pub tokens: &'static Tokens,
    pub cache: &'a canvas::Cache,
}

impl canvas::Program<Message> for EmptyChartProgram<'_> {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let t = self.tokens;
        vec![self.cache.draw(renderer, bounds.size(), |frame| {
            // Tudo a 70 %, como o `opacity: .7` do prototipo.
            let ring = alpha(t.neutral_800, 0.7);
            let bars = alpha(t.neutral_700, 0.7);
            // Borda de 12 px numa caixa de 128: a linha central fica em 64 - 6.
            frame.stroke(
                &Path::circle(Point::new(64.0, 64.0), 58.0),
                Stroke {
                    line_dash: LineDash {
                        segments: &[14.0, 10.0],
                        offset: 0,
                    },
                    ..Stroke::default().with_color(ring).with_width(12.0)
                },
            );
            let base = EMPTY_HEIGHT;
            let full = 110.0;
            for (k, share) in [0.55_f32, 0.80, 0.40, 1.0].into_iter().enumerate() {
                let x = 128.0 + 28.0 + k as f32 * (14.0 + 8.0) + 0.75;
                let w = 14.0 - 1.5;
                let h = full * share - 0.75;
                let top = base - h;
                let r = 3.0;
                // Contorno sem a base, cantos de cima arredondados.
                let outline = Path::new(|b| {
                    b.move_to(Point::new(x, base));
                    b.line_to(Point::new(x, top + r));
                    b.arc_to(Point::new(x, top), Point::new(x + r, top), r);
                    b.line_to(Point::new(x + w - r, top));
                    b.arc_to(Point::new(x + w, top), Point::new(x + w, top + r), r);
                    b.line_to(Point::new(x + w, base));
                });
                frame.stroke(
                    &outline,
                    Stroke {
                        line_dash: LineDash {
                            segments: &[4.0, 3.0],
                            offset: 0,
                        },
                        ..Stroke::default().with_color(bars).with_width(1.5)
                    },
                );
            }
        })]
    }
}

// ---- barras empilhadas das reservas ----

/// Espaco entre segmentos de uma pilha.
pub const STACK_GAP: f32 = 2.0;
/// Area util abaixo desta largura: barras finas (a janela de 1024 px).
const STACKED_NARROW: f32 = 640.0;

/// Largura das barras: 22 px, 14 quando a area util tem menos de 640.
pub fn stacked_bar_width(plot_width: f32) -> f32 {
    if plot_width >= STACKED_NARROW {
        22.0
    } else {
        14.0
    }
}

/// `(y0, h)` de cada valor, de baixo para cima: `y0` e a distancia da base ate o pe do segmento.
/// Mesmo comprimento de `values` para quem desenha casar com as series; valor `<= 0` vira
/// altura 0 e nao abre gap. Teto `<= 0` -> vazio.
pub fn stack_segments(values: &[i64], top: i64, plot_h: f32) -> Vec<(f32, f32)> {
    if top <= 0 {
        return Vec::new();
    }
    let mut y = 0.0;
    let mut first = true;
    values
        .iter()
        .map(|v| {
            let h = bar_height(*v, top, plot_h);
            if h <= 0.0 {
                return (y, 0.0);
            }
            if !first {
                y += STACK_GAP;
            }
            first = false;
            let seg = (y, h);
            y += h;
            seg
        })
        .collect()
}

pub struct StackedBarsProgram<'a> {
    pub series: &'a [ReserveSeries],
    pub months: &'a [String],
    pub axis_top: i64,
    /// Coluna do mes corrente (a ultima).
    pub current: usize,
    pub tokens: &'static Tokens,
    pub cache: &'a canvas::Cache,
}

impl StackedBarsProgram<'_> {
    fn column_values(&self, i: usize) -> Vec<i64> {
        self.series
            .iter()
            .map(|s| s.balances.get(i).copied().unwrap_or(0))
            .collect()
    }
}

fn series_color(t: &'static Tokens, c: &ReserveColor) -> Color {
    match c {
        ReserveColor::Accent => t.accent,
        ReserveColor::Token(k) => t.token(k),
    }
}

impl canvas::Program<Message> for StackedBarsProgram<'_> {
    /// Coluna sob o cursor.
    type State = Option<usize>;

    fn update(
        &self,
        state: &mut Option<usize>,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<Message>> {
        if !matches!(event, Event::Mouse(_)) {
            return None;
        }
        let hovered = cursor
            .position_in(bounds)
            .and_then(|p| bar_column(p, bounds.size(), self.months.len()));
        if hovered == *state {
            return None;
        }
        *state = hovered;
        Some(Action::request_redraw())
    }

    fn draw(
        &self,
        state: &Option<usize>,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let t = self.tokens;
        let size = bounds.size();
        let plot = plot_area(size);
        let columns = self.months.len().max(1);
        let col_w = plot.width / columns as f32;
        // Grade, eixo e meses no cache; as barras nao, porque o hover escurece as outras colunas
        // e redesenhar 12 pilhas por quadro e barato.
        let chart = self.cache.draw(renderer, size, |frame| {
            let grid = t.divider();
            let muted = t.text_alpha(0.50);
            let marks = [
                (plot.y, axis_label(self.axis_top)),
                (plot.y + plot.height / 2.0, axis_label(self.axis_top / 2)),
                (plot.y + plot.height, axis_label(0)),
            ];
            for (y, text) in marks {
                frame.fill_rectangle(
                    Point::new(plot.x, y.round() - 0.5),
                    Size::new(plot.width, 1.0),
                    grid,
                );
                frame.fill_text(Text {
                    align_x: TextAlign::Right,
                    ..label(text, Point::new(AXIS_W - AXIS_PAD, y), 11.0, muted)
                });
            }
            for (i, month) in self.months.iter().enumerate() {
                let current = i == self.current;
                frame.fill_text(Text {
                    font: if current {
                        fonts::INTER_MEDIUM
                    } else {
                        fonts::INTER
                    },
                    ..label(
                        month_short(month).to_string(),
                        Point::new(plot.x + (i as f32 + 0.5) * col_w, size.height - 8.0),
                        12.0,
                        if current { t.text } else { t.text_alpha(0.55) },
                    )
                });
            }
        });
        let mut bars = Frame::new(renderer, size);
        let bar_w = stacked_bar_width(plot.width);
        let base = plot.y + plot.height;
        if let Some(i) = *state {
            draw_column_highlight(&mut bars, t, plot.x + i as f32 * col_w, col_w, plot);
        }
        let mut tops = vec![base; self.months.len()];
        for (i, top) in tops.iter_mut().enumerate() {
            let x = plot.x + i as f32 * col_w + (col_w - bar_w) / 2.0;
            let faded = state.is_some_and(|j| j != i);
            let values = self.column_values(i);
            for (serie, (y0, h)) in
                self.series
                    .iter()
                    .zip(stack_segments(&values, self.axis_top, plot.height))
            {
                if h <= 0.0 {
                    continue;
                }
                let color = series_color(t, &serie.color);
                let color = if faded { alpha(color, 0.45) } else { color };
                bars.fill(
                    &Path::rounded_rectangle(
                        Point::new(x, base - y0 - h),
                        Size::new(bar_w, h),
                        2.0.into(),
                    ),
                    color,
                );
                *top = top.min(base - y0 - h);
            }
        }
        if let Some(i) = *state
            && let Some(month) = self.months.get(i)
        {
            let bar_top = tops.get(i).copied().unwrap_or(base);
            draw_stack_tooltip(
                &mut bars,
                t,
                self.series,
                month,
                i,
                (plot.x + i as f32 * col_w, col_w),
                bar_top,
                plot,
                size,
            );
        }
        vec![chart, bars.into_geometry()]
    }
}

/// Fundo a 5 % na coluna sob o mouse, raio 6 em cima.
fn draw_column_highlight(
    frame: &mut Frame,
    t: &'static Tokens,
    col_x: f32,
    col_w: f32,
    plot: Rectangle,
) {
    frame.fill(
        &Path::rounded_rectangle(
            Point::new(col_x, plot.y),
            Size::new(col_w, plot.height),
            Radius {
                top_left: 6.0,
                top_right: 6.0,
                bottom_right: 0.0,
                bottom_left: 0.0,
            },
        ),
        t.text_alpha(0.05),
    );
}

/// Tooltip do mes: titulo, uma linha por reserva com saldo, divisor e "Total".
#[allow(clippy::too_many_arguments)]
fn draw_stack_tooltip(
    frame: &mut Frame,
    t: &'static Tokens,
    series: &[ReserveSeries],
    month: &str,
    i: usize,
    (col_x, col_w): (f32, f32),
    bar_top: f32,
    plot: Rectangle,
    size: Size,
) {
    let rows: Vec<(String, String, Color)> = series
        .iter()
        .filter_map(|s| {
            let v = s.balances.get(i).copied().unwrap_or(0);
            (v > 0).then(|| {
                (
                    clip_chars(&s.name, 24),
                    format_brl(v),
                    series_color(t, &s.color),
                )
            })
        })
        .collect();
    let total: i64 = series
        .iter()
        .filter_map(|s| s.balances.get(i).copied())
        .filter(|v| *v > 0)
        .fold(0i64, i64::saturating_add);
    let total_label = format_brl(total);
    // Largura estimada (o canvas nao mede texto): ~7 px por caractere em 12 px tabular, mais o
    // ponto, os respiros e o padding.
    let longest = rows
        .iter()
        .map(|(n, v, _)| n.chars().count() + v.chars().count())
        .chain(std::iter::once(5 + total_label.chars().count()))
        .max()
        .unwrap_or(0);
    let line_h = 15.0;
    let gap = 4.0;
    let lines = rows.len() + 2;
    let box_w = (longest as f32 * 7.0 + 40.0).max(210.0);
    let box_h = 8.0 + lines as f32 * line_h + (lines - 1) as f32 * gap + 8.0 + 5.0;
    let (x, y) = tooltip_origin(
        Size::new(box_w, box_h),
        col_x,
        col_w,
        bar_top,
        plot.y,
        size.width,
    );
    let rect = Path::rounded_rectangle(Point::new(x, y), Size::new(box_w, box_h), 8.0.into());
    frame.fill(&rect, t.bg);
    frame.stroke(
        &rect,
        Stroke::default()
            .with_color(t.shadow_md_ring())
            .with_width(1.0),
    );
    let center_of = |k: usize| y + 8.0 + k as f32 * (line_h + gap) + line_h / 2.0;
    let left = |content: String, ly: f32, color: Color, font| Text {
        align_x: TextAlign::Left,
        font,
        ..label(content, Point::new(x + 10.0, ly), 12.0, color)
    };
    let right = |content: String, ly: f32, font| Text {
        align_x: TextAlign::Right,
        font,
        ..label(content, Point::new(x + box_w - 10.0, ly), 12.0, t.text)
    };
    frame.fill_text(left(
        month_label_capitalized(month),
        center_of(0),
        t.text,
        fonts::INTER_MEDIUM,
    ));
    for (k, (name, value, color)) in rows.into_iter().enumerate() {
        let ly = center_of(k + 1);
        frame.fill(&Path::circle(Point::new(x + 13.0, ly), 3.0), color);
        frame.fill_text(Text {
            align_x: TextAlign::Left,
            ..label(name, Point::new(x + 10.0 + 12.0, ly), 12.0, t.text)
        });
        frame.fill_text(right(value, ly, fonts::INTER));
    }
    // Borda de 1 px e 4 de respiro antes do "Total" (o `border-top` + `padding-top` do prototipo).
    let divider_y = y + 8.0 + (lines - 1) as f32 * (line_h + gap);
    frame.fill_rectangle(
        Point::new(x + 10.0, divider_y),
        Size::new(box_w - 20.0, 1.0),
        t.divider(),
    );
    let total_y = divider_y + 5.0 + line_h / 2.0;
    frame.fill_text(left(
        "Total".to_string(),
        total_y,
        t.text,
        fonts::INTER_MEDIUM,
    ));
    frame.fill_text(right(total_label, total_y, fonts::INTER_MEDIUM));
}

// ---- esqueleto vazio das reservas ----

/// Seis retangulos de 16 px de altura em 240 de largura, 6 entre eles (o medidor de seis meses
/// do prototipo, tracejado).
pub const EMPTY_RESERVES_WIDTH: f32 = 240.0;
pub const EMPTY_RESERVES_HEIGHT: f32 = 16.0;
const EMPTY_RESERVES_GAP: f32 = 6.0;

/// O `container` do iced nao tem borda tracejada: o esqueleto e desenhado em canvas.
pub struct EmptyReservesProgram<'a> {
    pub tokens: &'static Tokens,
    pub cache: &'a canvas::Cache,
}

impl canvas::Program<Message> for EmptyReservesProgram<'_> {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let t = self.tokens;
        vec![self.cache.draw(renderer, bounds.size(), |frame| {
            // Tudo a 70 %, como o `opacity: .7` do prototipo.
            let color = alpha(t.neutral_700, 0.7);
            let w = (EMPTY_RESERVES_WIDTH - 5.0 * EMPTY_RESERVES_GAP) / 6.0;
            // Traco de 1,5 centrado na borda: recua meio traco para caber na caixa.
            let inset = 0.75;
            for k in 0..6 {
                let x = k as f32 * (w + EMPTY_RESERVES_GAP) + inset;
                let rect = Path::rounded_rectangle(
                    Point::new(x, inset),
                    Size::new(w - 2.0 * inset, EMPTY_RESERVES_HEIGHT - 2.0 * inset),
                    3.0.into(),
                );
                frame.stroke(
                    &rect,
                    Stroke {
                        line_dash: LineDash {
                            segments: &[4.0, 3.0],
                            offset: 0,
                        },
                        ..Stroke::default().with_color(color).with_width(1.5)
                    },
                );
            }
        })]
    }
}

#[cfg(test)]
mod tests {
    use iced::widget::canvas::Program;

    use super::*;
    use crate::ui::theme::DARK;

    fn slice(key: &str, amount: i64, share: f64) -> Slice {
        Slice {
            key: key.into(),
            name: format!("Cat {key}"),
            color: "sky".into(),
            icon: "tag".into(),
            amount_minor: amount,
            share,
        }
    }

    #[test]
    fn angulos_da_rosca() {
        let arcs = donut_angles(&[0.5, 0.5]);
        assert!((arcs[0].0 + FRAC_PI_2).abs() < 1e-6, "comeca as 12 h");
        assert!((arcs[0].1 - (FRAC_PI_2 - DONUT_GAP)).abs() < 1e-5);
        assert!((arcs[1].0 - FRAC_PI_2).abs() < 1e-5);
        // Fatia unica: anel inteiro, sem folga.
        let one = donut_angles(&[1.0]);
        assert!((one[0].1 - one[0].0 - TAU).abs() < 1e-5);
        // Fatia menor que a folga: some, sem angulo negativo.
        let tiny = donut_angles(&[0.999, 0.001]);
        assert!(tiny[1].1 >= tiny[1].0);
        assert!(donut_angles(&[]).is_empty());
    }

    #[test]
    fn miolo_da_rosca_com_e_sem_hover() {
        let slices = [slice("A", 30_000, 0.75), slice("B", 10_000, 0.25)];
        assert_eq!(
            donut_center_lines(&slices, 40_000, None),
            (
                "Despesas".to_string(),
                "R$ 400,00".to_string(),
                "2 categorias".to_string()
            )
        );
        assert_eq!(
            donut_center_lines(&slices, 40_000, Some("B")),
            (
                "Cat B".to_string(),
                "R$ 100,00".to_string(),
                "25% do mês".to_string()
            )
        );
        // Hover de uma chave que sumiu (mes trocou): volta ao total.
        assert_eq!(donut_center_lines(&slices, 40_000, Some("Z")).0, "Despesas");
        assert_eq!(categories_label(1), "1 categoria");
        assert_eq!(percent(0.125), 13);
        assert_eq!(percent(0.0), 0);
    }

    #[test]
    fn rotulo_mais_longo_do_eixo_cabe_na_coluna() {
        // Sem medir texto no canvas: Inter 11 px fica perto de 5,8 px por caractere com os
        // espacos. O rotulo e alinhado a direita e o canvas corta o que passa da borda
        // esquerda, entao "R$ 12 mil" numa coluna de 52 perdia o "R".
        for minor in [1_200_000, 2_400_000, 12_000_000, 250_000] {
            let text = axis_label(minor);
            let width = text.chars().count() as f32 * 5.8;
            assert!(width <= AXIS_W - AXIS_PAD, "{text}: {width}");
        }
    }

    #[test]
    fn coluna_sob_o_cursor() {
        let size = Size::new(668.0, 210.0);
        // Area util: x de 68 a 668, seis colunas de 100.
        assert_eq!(bar_column(Point::new(10.0, 100.0), size, 6), None, "eixo");
        assert_eq!(bar_column(Point::new(68.0, 100.0), size, 6), Some(0));
        assert_eq!(bar_column(Point::new(167.9, 100.0), size, 6), Some(0));
        assert_eq!(bar_column(Point::new(168.0, 100.0), size, 6), Some(1));
        assert_eq!(bar_column(Point::new(667.0, 205.0), size, 6), Some(5));
        assert_eq!(bar_column(Point::new(668.0, 100.0), size, 6), None);
        assert_eq!(bar_column(Point::new(300.0, 100.0), size, 0), None);
    }

    #[test]
    fn hover_das_barras_so_redesenha_quando_muda() {
        let cache = canvas::Cache::new();
        let bars: Vec<MonthTotals> = (4..10)
            .map(|m| MonthTotals {
                month: format!("2026-{m:02}"),
                income_minor: 100_000,
                expense_minor: 50_000,
            })
            .collect();
        let program = BarsProgram {
            bars: &bars,
            axis_top: 200_000,
            selected: 5,
            tokens: &DARK,
            cache: &cache,
        };
        let bounds = Rectangle::new(Point::new(100.0, 300.0), Size::new(668.0, 210.0));
        let moved = |x: f32, y: f32| {
            (
                Event::Mouse(mouse::Event::CursorMoved {
                    position: Point::new(x, y),
                }),
                mouse::Cursor::Available(Point::new(x, y)),
            )
        };
        let mut state = None;
        let (event, cursor) = moved(100.0 + 176.0, 400.0);
        assert!(program.update(&mut state, &event, bounds, cursor).is_some());
        assert_eq!(state, Some(1));
        // Mesma coluna: nada a redesenhar.
        let (event, cursor) = moved(100.0 + 186.0, 410.0);
        assert!(program.update(&mut state, &event, bounds, cursor).is_none());
        // Saiu do canvas: some o tooltip.
        let (event, cursor) = moved(10.0, 10.0);
        assert!(program.update(&mut state, &event, bounds, cursor).is_some());
        assert_eq!(state, None);
        let left = Event::Mouse(mouse::Event::CursorLeft);
        assert!(
            program
                .update(&mut state, &left, bounds, mouse::Cursor::Unavailable)
                .is_none()
        );
    }

    #[test]
    fn tooltip_acima_ou_ao_lado() {
        let tip = Size::new(120.0, 85.0);
        // Barra baixa: cabe em cima, centrado na coluna.
        assert_eq!(
            tooltip_origin(tip, 152.0, 100.0, 150.0, 6.0, 652.0),
            (142.0, 59.0)
        );
        // Barra alta: vai para a direita da coluna, no topo.
        assert_eq!(
            tooltip_origin(tip, 152.0, 100.0, 40.0, 6.0, 652.0),
            (256.0, 6.0)
        );
        // Ultima coluna: nao cabe a direita, vai para a esquerda.
        assert_eq!(
            tooltip_origin(tip, 552.0, 100.0, 40.0, 6.0, 652.0),
            (428.0, 6.0)
        );
    }

    #[test]
    fn altura_da_barra_sem_estourar() {
        assert_eq!(bar_height(100, 200, 180.0), 90.0);
        assert_eq!(bar_height(500, 200, 180.0), 180.0);
        assert_eq!(bar_height(-5, 200, 180.0), 0.0);
        assert_eq!(bar_height(5, 0, 180.0), 0.0);
    }

    #[test]
    fn largura_das_barras_empilhadas() {
        assert_eq!(stacked_bar_width(640.0), 22.0);
        assert_eq!(stacked_bar_width(900.0), 22.0);
        assert_eq!(stacked_bar_width(639.0), 14.0);
    }

    #[test]
    fn segmentos_da_pilha() {
        let segs = stack_segments(&[100, 200], 400, 200.0);
        assert_eq!(segs, vec![(0.0, 50.0), (52.0, 100.0)]);
        // Zero nao abre gap.
        let segs = stack_segments(&[100, 0, 200], 400, 200.0);
        assert_eq!(segs, vec![(0.0, 50.0), (50.0, 0.0), (52.0, 100.0)]);
        let segs = stack_segments(&[0, 200], 400, 200.0);
        assert_eq!(segs, vec![(0.0, 0.0), (0.0, 100.0)]);
        assert!(stack_segments(&[100], 0, 200.0).is_empty());
    }

    #[test]
    fn doze_colunas() {
        let size = Size::new(668.0, 230.0);
        // Area util de 600: colunas de 50.
        assert_eq!(bar_column(Point::new(68.0, 100.0), size, 12), Some(0));
        assert_eq!(bar_column(Point::new(117.9, 100.0), size, 12), Some(0));
        assert_eq!(bar_column(Point::new(118.0, 100.0), size, 12), Some(1));
        assert_eq!(bar_column(Point::new(667.0, 100.0), size, 12), Some(11));
    }

    #[test]
    fn hover_das_pilhas_so_redesenha_quando_muda() {
        let cache = canvas::Cache::new();
        let months: Vec<String> = (1..=12).map(|m| format!("2026-{m:02}")).collect();
        let series = vec![ReserveSeries {
            id: "R".into(),
            name: "Reserva".into(),
            color: ReserveColor::Accent,
            balances: vec![100_000; 12],
        }];
        let program = StackedBarsProgram {
            series: &series,
            months: &months,
            axis_top: 600_000,
            current: 11,
            tokens: &DARK,
            cache: &cache,
        };
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(668.0, 230.0));
        let at = |x: f32| {
            (
                Event::Mouse(mouse::Event::CursorMoved {
                    position: Point::new(x, 100.0),
                }),
                mouse::Cursor::Available(Point::new(x, 100.0)),
            )
        };
        let mut state = None;
        let (event, cursor) = at(126.0);
        assert!(program.update(&mut state, &event, bounds, cursor).is_some());
        assert_eq!(state, Some(1));
        let (event, cursor) = at(136.0);
        assert!(program.update(&mut state, &event, bounds, cursor).is_none());
    }

    #[test]
    fn reticencias_por_caractere() {
        assert_eq!(clip_chars("Mercado", 20), "Mercado");
        assert_eq!(clip_chars("Alimentação e bebidas do mês", 10), "Alimentaç…");
    }
}
