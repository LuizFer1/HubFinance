//! Graficos em `canvas`: ritmo de gastos, medidores com marca, historico de 12 meses e o
//! esqueleto tracejado do estado "Sem dados" (Dashboard); barras empilhadas da evolucao e o
//! medidor tracejado do estado vazio (Reservas).
//!
//! A geometria fica num `canvas::Cache` que a `App` limpa quando dados, mes, tema ou "Ocultar
//! valores" mudam (os rotulos em R$ ficam gravados na geometria); o hover das barras e estado
//! do proprio `Program` e e desenhado num segundo `Frame`, sem cache, para passar o mouse nao
//! refazer o grafico inteiro.

use iced::alignment::Vertical;
use iced::border::Radius;
use iced::mouse;
use iced::widget::canvas::{
    self, Action, Event, Frame, Geometry, LineDash, LineJoin, Path, Stroke, Text,
};
use iced::widget::text::{Alignment as TextAlign, LineHeight, Shaping};
use iced::{Color, Pixels, Point, Rectangle, Renderer, Size, Theme};

use super::app::Message;
use super::fonts;
use super::theme::{Tokens, alpha};
use crate::dashboard::grafite::{HistoryChart, HistoryColumn, PaceChart};
use crate::dashboard::periods::{month_label_capitalized, month_short};
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

// ---- barras ----

/// Coluna do eixo Y, faixa dos rotulos dos meses e respiro no topo para o rotulo de cima.
/// Mais larga que os 52 do design: no HTML o rotulo transborda para o padding do card, no
/// canvas ele e cortado na borda. 68 cabe ate "R$ 120 mil" com o respiro.
const AXIS_W: f32 = 68.0;
/// Respiro entre o fim do rotulo do eixo e a area das barras.
const AXIS_PAD: f32 = 8.0;
const LABELS_H: f32 = 24.0;
const TOP: f32 = 6.0;

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
    /// Rotulos do eixo (teto, metade, zero) e o "Total" de cada mes, prontos no modelo: com
    /// "Ocultar valores" ja vem a mascara, e o canvas nunca formata dinheiro.
    pub y_labels: &'a [String; 3],
    pub month_totals: &'a [String],
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

/// Cor de uma serie da evolucao das reservas, e a cor de identidade de cada reserva em toda a
/// tela Reservas (barras, tooltip, legenda, barra do Total, tile, ponto das movimentacoes:
/// `reserves_view::reserve_color` delega aqui). A emergencia vai em `neutral_300`, e nao no
/// acento: o acento ficava a 1,08:1 da caixinha `amber` e as duas pilhas viravam uma so
/// (decisao do dono, relatorio 14.7). Ela e a primeira serie, entao fica na base da pilha.
pub(super) fn series_color(t: &'static Tokens, c: &ReserveColor) -> Color {
    match c {
        ReserveColor::Accent => t.neutral_300,
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
            let muted = t.text_muted;
            let [top, half, zero] = self.y_labels;
            let marks = [
                (plot.y, top.clone()),
                (plot.y + plot.height / 2.0, half.clone()),
                (plot.y + plot.height, zero.clone()),
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
                        if current { t.text } else { t.text_muted },
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
                &stack_tooltip(self.series, self.month_totals, i),
                month,
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

/// O que o tooltip do mes `i` mostra: uma linha por reserva com saldo (nome cortado, valor e
/// cor) e o total. Os textos vem do modelo, ja mascarados com "Ocultar valores".
#[derive(Debug, PartialEq)]
struct StackTooltip<'a> {
    rows: Vec<(String, &'a str, &'a ReserveColor)>,
    total: &'a str,
}

fn stack_tooltip<'a>(
    series: &'a [ReserveSeries],
    month_totals: &'a [String],
    i: usize,
) -> StackTooltip<'a> {
    StackTooltip {
        rows: series
            .iter()
            .filter(|s| s.balances.get(i).copied().unwrap_or(0) > 0)
            .map(|s| {
                (
                    clip_chars(&s.name, 24),
                    s.labels.get(i).map_or("", String::as_str),
                    &s.color,
                )
            })
            .collect(),
        total: month_totals.get(i).map_or("", String::as_str),
    }
}

/// Tooltip do mes: titulo, uma linha por reserva com saldo, divisor e "Total".
#[allow(clippy::too_many_arguments)]
fn draw_stack_tooltip(
    frame: &mut Frame,
    t: &'static Tokens,
    tip: &StackTooltip<'_>,
    month: &str,
    (col_x, col_w): (f32, f32),
    bar_top: f32,
    plot: Rectangle,
    size: Size,
) {
    let rows: Vec<(String, String, Color)> = tip
        .rows
        .iter()
        .map(|(name, value, color)| (name.clone(), value.to_string(), series_color(t, color)))
        .collect();
    let total_label = tip.total.to_string();
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

// ---- ritmo de gastos ----

/// Coluna do eixo y (54 do design) e o respiro ate a area do grafico.
const PACE_AXIS_W: f32 = 54.0;
const PACE_AXIS_GAP: f32 = 10.0;
/// Faixa dos rotulos a direita ("~R$ 4.460", "normal R$ 3.662"): o `padding-right: 108px`.
const PACE_RIGHT: f32 = 108.0;
const PACE_PLOT_H: f32 = 290.0;
/// Respiro no topo: o rotulo de cima do eixo fica centrado na linha do teto e o canvas corta o
/// que passa da borda (no HTML ele transborda para o padding).
const PACE_TOP: f32 = 6.0;
/// 10 de espaco + 16 da faixa dos dias.
const PACE_X_LABELS: f32 = 26.0;
pub const PACE_HEIGHT: f32 = PACE_TOP + PACE_PLOT_H + PACE_X_LABELS;
/// Distancia minima entre os centros dos dois rotulos da direita (12 px a 1,3 de altura + 0,4).
const PACE_LABEL_GAP: f32 = 16.0;

/// Area das linhas dentro do canvas do ritmo.
pub fn pace_plot(size: Size) -> Rectangle {
    let x = PACE_AXIS_W + PACE_AXIS_GAP;
    Rectangle {
        x,
        y: PACE_TOP,
        width: (size.width - x - PACE_RIGHT).max(0.0),
        height: PACE_PLOT_H.min((size.height - PACE_TOP - PACE_X_LABELS).max(0.0)),
    }
}

/// x do dia `day` (1 = primeiro): `day / days` da largura, como o `viewBox` 300 do design (dia
/// 1 em 10, ultimo dia na borda direita).
pub fn day_x(day: usize, days: usize, plot: Rectangle) -> f32 {
    if days == 0 {
        return plot.x;
    }
    plot.x + plot.width * (day as f32 / days as f32)
}

/// y de uma fracao do teto (0 embaixo, 1 no topo), sem sair da area.
pub fn value_y(v: f32, plot: Rectangle) -> f32 {
    plot.y + plot.height * (1.0 - v.clamp(0.0, 1.0))
}

/// Um ponto por dia: `values[0]` e o dia 1.
pub fn pace_points(values: &[f32], days: usize, plot: Rectangle) -> Vec<Point> {
    values
        .iter()
        .enumerate()
        .map(|(i, v)| Point::new(day_x(i + 1, days, plot), value_y(*v, plot)))
        .collect()
}

/// Reta tracejada da previsao: do acumulado no dia do corte ate a despesa prevista no ultimo
/// dia (fracoes do teto).
pub fn forecast_segment(
    cutoff: usize,
    days: usize,
    (from, to): (f32, f32),
    plot: Rectangle,
) -> (Point, Point) {
    (
        Point::new(day_x(cutoff, days, plot), value_y(from, plot)),
        Point::new(day_x(days, days, plot), value_y(to, plot)),
    )
}

/// Centro do rotulo "renda R$ X": 12 px acima da linha, como o design; sem espaco acima (renda
/// no teto do eixo, que e multiplo de R$ 2.500 e pode ser a propria renda) desce para baixo da
/// linha, senao o canvas cortaria o rotulo.
pub fn income_label_y(line_y: f32) -> f32 {
    const OFFSET: f32 = 12.0;
    /// Meia altura do texto de 12 px a 1,3.
    const HALF: f32 = 8.0;
    if line_y - OFFSET - HALF >= 0.0 {
        line_y - OFFSET
    } else {
        line_y + OFFSET
    }
}

/// Afasta dois rotulos (centros em y) que se encostariam: cada um anda metade do que falta,
/// sem sair de `[top, bottom]`. Previsao e normal no fim do mes costumam estar perto.
pub fn separate_labels(a: f32, b: f32, min_gap: f32, top: f32, bottom: f32) -> (f32, f32) {
    let d = (a - b).abs();
    if d >= min_gap {
        return (a, b);
    }
    let push = (min_gap - d) / 2.0;
    let (a, b) = if a <= b {
        (a - push, b + push)
    } else {
        (a + push, b - push)
    };
    let down = (top - a.min(b)).max(0.0);
    let up = (a.max(b) - bottom).max(0.0);
    let shift = down - up;
    (a + shift, b + shift)
}

fn polyline(points: &[Point]) -> Path {
    Path::new(|b| {
        if let Some((first, rest)) = points.split_first() {
            b.move_to(*first);
            for p in rest {
                b.line_to(*p);
            }
        }
    })
}

fn dashed<'a>(color: Color, width: f32, segments: &'a [f32]) -> Stroke<'a> {
    Stroke {
        line_dash: LineDash {
            segments,
            offset: 0,
        },
        ..Stroke::default().with_color(color).with_width(width)
    }
}

/// "Ritmo de gastos": eixo, grade, renda, area e linha do mes, normal e previsao tracejados,
/// marcador de hoje, rotulos a direita e dias embaixo. Tudo no `Cache`: o grafico nao tem
/// hover, e os rotulos em R$ vem prontos (mascarados) do modelo, por isso o cache e limpo
/// quando "Ocultar valores" muda.
pub struct PaceProgram<'a> {
    pub chart: &'a PaceChart,
    /// Mes corrente: linha vertical, marcador e rotulo "hoje".
    pub today: bool,
    pub tokens: &'static Tokens,
    pub cache: &'a canvas::Cache,
}

impl canvas::Program<Message> for PaceProgram<'_> {
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
        let c = self.chart;
        let size = bounds.size();
        let plot = pace_plot(size);
        let geometry = self.cache.draw(renderer, size, |frame| {
            let muted = t.text_muted;
            for (k, text) in c.y_labels.iter().enumerate() {
                let y = plot.y + plot.height * k as f32 / 2.0;
                let grid = if k == 2 { t.neutral_800 } else { t.neutral_900 };
                frame.fill_rectangle(
                    Point::new(plot.x, y.round() - 0.5),
                    Size::new(plot.width, 1.0),
                    grid,
                );
                frame.fill_text(Text {
                    align_x: TextAlign::Right,
                    ..label(text.clone(), Point::new(PACE_AXIS_W, y), 11.0, muted)
                });
            }
            if let Some(income) = c.income {
                let y = value_y(income, plot).round() - 0.5;
                frame.fill_rectangle(
                    Point::new(plot.x, y),
                    Size::new(plot.width, 1.0),
                    alpha(t.income, 0.75),
                );
                if let Some(text) = &c.income_label {
                    frame.fill_text(Text {
                        align_x: TextAlign::Left,
                        ..label(
                            text.clone(),
                            Point::new(plot.x, income_label_y(y)),
                            12.0,
                            t.income_fg,
                        )
                    });
                }
            }
            if self.today && c.cutoff > 0 {
                let x = day_x(c.cutoff, c.days, plot).round() + 0.5;
                frame.stroke(
                    &polyline(&[Point::new(x, plot.y), Point::new(x, plot.y + plot.height)]),
                    dashed(t.neutral_600, 1.0, &[2.0, 3.0]),
                );
            }
            let current = pace_points(&c.current, c.days, plot);
            if let (Some(first), Some(last)) = (current.first(), current.last()) {
                let bottom = plot.y + plot.height;
                let area = Path::new(|b| {
                    b.move_to(*first);
                    for p in &current[1..] {
                        b.line_to(*p);
                    }
                    b.line_to(Point::new(last.x, bottom));
                    b.line_to(Point::new(first.x, bottom));
                    b.close();
                });
                frame.fill(&area, alpha(t.accent, 0.16));
            }
            let normal = pace_points(&c.normal, c.days, plot);
            if normal.len() > 1 {
                frame.stroke(&polyline(&normal), dashed(t.neutral_500, 1.5, &[4.0, 4.0]));
            }
            let forecast = c
                .forecast
                .map(|f| forecast_segment(c.cutoff, c.days, f, plot));
            if let Some((a, b)) = forecast {
                frame.stroke(&polyline(&[a, b]), dashed(t.accent, 1.5, &[5.0, 4.0]));
            }
            if current.len() > 1 {
                frame.stroke(
                    &polyline(&current),
                    Stroke::default()
                        .with_color(t.accent)
                        .with_width(2.0)
                        .with_line_join(LineJoin::Round),
                );
            }
            if self.today
                && let Some(p) = current.last()
            {
                frame.stroke(
                    &Path::circle(*p, 7.0),
                    Stroke::default()
                        .with_color(alpha(t.accent, 0.35))
                        .with_width(4.0),
                );
                frame.fill(&Path::circle(*p, 5.0), t.text);
            }
            // Rotulos a direita: fim da previsao e fim do normal, afastados se encostarem.
            let right = plot.x + plot.width + 10.0;
            let forecast_y = forecast.map(|(_, b)| b.y);
            let normal_y = normal.last().map(|p| p.y);
            let (fy, ny) = match (forecast_y, normal_y) {
                (Some(f), Some(n)) => {
                    let (f, n) =
                        separate_labels(f, n, PACE_LABEL_GAP, plot.y, plot.y + plot.height);
                    (Some(f), Some(n))
                }
                other => other,
            };
            for (y, text, color) in [
                (fy, c.forecast_label.as_ref(), t.accent_300),
                (ny, c.normal_label.as_ref(), muted),
            ] {
                if let (Some(y), Some(text)) = (y, text) {
                    frame.fill_text(Text {
                        align_x: TextAlign::Left,
                        ..label(text.clone(), Point::new(right, y), 12.0, color)
                    });
                }
            }
            let label_y = plot.y + plot.height + 10.0 + 8.0;
            for (day, text) in &c.x_labels {
                let is_today = self.today && *day == c.cutoff;
                let color = if is_today { t.text } else { muted };
                let last = *day == c.days;
                let x = if last {
                    plot.x + plot.width
                } else {
                    day_x(*day, c.days, plot)
                };
                frame.fill_text(Text {
                    align_x: if last {
                        TextAlign::Right
                    } else {
                        TextAlign::Center
                    },
                    ..label(text.clone(), Point::new(x, label_y), 12.0, color)
                });
            }
        });
        vec![geometry]
    }
}

// ---- medidor ----

/// Barra de progresso com marcas pontilhadas (meta nas linhas "em atenção", 50% e 70% no
/// comprometido). Sem `Cache`: sao poucos retangulos, e um cache por linha teria de viver na
/// `App` (um cache so para medidores de mesmo tamanho repetiria a geometria do primeiro).
pub struct MeterProgram {
    pub fill: f32,
    pub fill_color: Color,
    pub track: Color,
    /// Altura da barra (4 nas categorias, 6 no comprometido).
    pub bar: f32,
    /// Posicao (0..=1) e rotulo embaixo (vazio = sem rotulo).
    pub marks: Vec<(f32, String)>,
    pub mark_color: Color,
    pub label_color: Color,
}

/// A marca sobe 4 px acima da barra e desce 4 abaixo (o `top: -4px` do design).
const METER_OVERHANG: f32 = 4.0;
/// Faixa dos rotulos das marcas: 6 de espaco + 14.
const METER_LABELS: f32 = 20.0;

impl MeterProgram {
    /// Altura do canvas para este medidor.
    pub fn height(&self) -> f32 {
        let labels = self.marks.iter().any(|(_, l)| !l.is_empty());
        self.bar + 2.0 * METER_OVERHANG + if labels { METER_LABELS } else { 0.0 }
    }
}

/// Largura do preenchimento e x de cada marca numa barra de `width`.
pub fn meter_geometry(width: f32, fill: f32, marks: &[f32]) -> (f32, Vec<f32>) {
    let at = |f: f32| width * f.clamp(0.0, 1.0);
    (at(fill), marks.iter().map(|m| at(*m)).collect())
}

impl canvas::Program<Message> for MeterProgram {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let width = bounds.width;
        let y = METER_OVERHANG;
        let r = self.bar / 2.0;
        let positions: Vec<f32> = self.marks.iter().map(|(m, _)| *m).collect();
        let (fill_w, xs) = meter_geometry(width, self.fill, &positions);
        frame.fill(
            &Path::rounded_rectangle(Point::new(0.0, y), Size::new(width, self.bar), r.into()),
            self.track,
        );
        if fill_w > 0.0 {
            frame.fill(
                &Path::rounded_rectangle(
                    Point::new(0.0, y),
                    Size::new(fill_w, self.bar),
                    r.min(fill_w / 2.0).into(),
                ),
                self.fill_color,
            );
        }
        let mark_h = self.bar + 2.0 * METER_OVERHANG;
        for (x, (_, text)) in xs.into_iter().zip(&self.marks) {
            // Pontilhado de 2 px: o canvas corta a marca no limite, entao ela fica dentro.
            let x = x.clamp(1.0, (width - 1.0).max(1.0));
            frame.stroke(
                &polyline(&[Point::new(x, 0.0), Point::new(x, mark_h)]),
                dashed(self.mark_color, 2.0, &[2.0, 2.0]),
            );
            if !text.is_empty() {
                frame.fill_text(label(
                    text.clone(),
                    Point::new(x, mark_h + METER_LABELS / 2.0),
                    11.0,
                    self.label_color,
                ));
            }
        }
        vec![frame.into_geometry()]
    }
}

// ---- amostra da legenda ----

/// Traco de 14 px da legenda do ritmo (linha cheia ou tracejada).
pub struct SwatchProgram {
    pub color: Color,
    pub width: f32,
    pub dashed: bool,
}

impl canvas::Program<Message> for SwatchProgram {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let y = bounds.height / 2.0;
        let line = polyline(&[Point::new(0.0, y), Point::new(bounds.width, y)]);
        let stroke = if self.dashed {
            dashed(self.color, self.width, &[4.0, 3.0])
        } else {
            Stroke::default()
                .with_color(self.color)
                .with_width(self.width)
        };
        frame.stroke(&line, stroke);
        vec![frame.into_geometry()]
    }
}

// ---- historico de 12 meses ----

/// Eixo R$ a esquerda, eixo % a direita, respiro no topo e faixa dos meses.
const HIST_AXIS_L: f32 = 52.0;
const HIST_AXIS_R: f32 = 36.0;
const HIST_TOP: f32 = 8.0;
const HIST_LABELS: f32 = 22.0;
pub const HISTORY_HEIGHT: f32 = 200.0;
const HIST_BAR_GAP: f32 = 2.0;

pub fn history_plot(size: Size) -> Rectangle {
    Rectangle {
        x: HIST_AXIS_L,
        y: HIST_TOP,
        width: (size.width - HIST_AXIS_L - HIST_AXIS_R).max(0.0),
        height: (size.height - HIST_TOP - HIST_LABELS).max(0.0),
    }
}

/// Coluna sob o ponto; fora da area util (ou sem colunas) -> `None`.
pub fn history_column_at(point: Point, size: Size, columns: usize) -> Option<usize> {
    let plot = history_plot(size);
    if columns == 0 || plot.width <= 0.0 {
        return None;
    }
    if point.x < plot.x || point.x >= plot.x + plot.width || point.y < 0.0 || point.y > size.height
    {
        return None;
    }
    let col = ((point.x - plot.x) / (plot.width / columns as f32)).floor() as usize;
    Some(col.min(columns - 1))
}

/// Barras de receita e despesa de uma coluna; `None` antes do primeiro lancamento (coluna
/// vazia: "nao existia", nao zero).
pub fn history_bars(
    column: &HistoryColumn,
    i: usize,
    columns: usize,
    plot: Rectangle,
) -> Option<[Rectangle; 2]> {
    if !column.existed || columns == 0 {
        return None;
    }
    let slot = plot.width / columns as f32;
    let bar_w = ((slot * 0.7 - HIST_BAR_GAP) / 2.0).clamp(2.0, 10.0);
    let x0 = plot.x + i as f32 * slot + (slot - 2.0 * bar_w - HIST_BAR_GAP) / 2.0;
    let base = plot.y + plot.height;
    let bar = |k: usize, v: f32| {
        let h = plot.height * v.clamp(0.0, 1.0);
        Rectangle {
            x: x0 + k as f32 * (bar_w + HIST_BAR_GAP),
            y: base - h,
            width: bar_w,
            height: h,
        }
    };
    Some([bar(0, column.income), bar(1, column.expense)])
}

/// Barras receita x despesa dos 12 meses, linha da taxa de poupanca com eixo % a direita e a
/// faixa tracejada do alvo. O hover desenha o tooltip pronto do modelo num `Frame` sem cache.
pub struct HistoryProgram<'a> {
    pub chart: &'a HistoryChart,
    pub tokens: &'static Tokens,
    pub cache: &'a canvas::Cache,
}

impl canvas::Program<Message> for HistoryProgram<'_> {
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
            .and_then(|p| history_column_at(p, bounds.size(), self.chart.columns.len()))
            .filter(|i| {
                self.chart
                    .columns
                    .get(*i)
                    .is_some_and(|c| c.tooltip.is_some())
            });
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
        let c = self.chart;
        let size = bounds.size();
        let plot = history_plot(size);
        let n = c.columns.len();
        let slot = if n == 0 { 0.0 } else { plot.width / n as f32 };
        let chart = self.cache.draw(renderer, size, |frame| {
            let muted = t.text_muted;
            for (k, text) in c.y_labels.iter().enumerate() {
                let y = plot.y + plot.height * k as f32 / 2.0;
                let grid = if k == 2 { t.neutral_800 } else { t.neutral_900 };
                frame.fill_rectangle(
                    Point::new(plot.x, y.round() - 0.5),
                    Size::new(plot.width, 1.0),
                    grid,
                );
                frame.fill_text(Text {
                    align_x: TextAlign::Right,
                    ..label(text.clone(), Point::new(HIST_AXIS_L - 6.0, y), 11.0, muted)
                });
            }
            let right = plot.x + plot.width + 6.0;
            let [low, high] = &c.rate_labels;
            for (y, text) in [(plot.y, high), (plot.y + plot.height, low)] {
                frame.fill_text(Text {
                    align_x: TextAlign::Left,
                    ..label(text.clone(), Point::new(right, y), 11.0, muted)
                });
            }
            let target_y = value_y(c.target, plot);
            frame.stroke(
                &polyline(&[
                    Point::new(plot.x, target_y),
                    Point::new(plot.x + plot.width, target_y),
                ]),
                dashed(t.neutral_500, 2.0, &[4.0, 4.0]),
            );
            frame.fill_text(Text {
                align_x: TextAlign::Left,
                ..label(
                    c.target_label.clone(),
                    Point::new(plot.x + 4.0, target_y - 9.0),
                    11.0,
                    muted,
                )
            });
            for (i, col) in c.columns.iter().enumerate() {
                if let Some(bars) = history_bars(col, i, n, plot) {
                    for (rect, color) in bars.into_iter().zip([t.income, t.expense]) {
                        if rect.height <= 0.0 {
                            continue;
                        }
                        let path = Path::rounded_rectangle(
                            rect.position(),
                            rect.size(),
                            Radius {
                                top_left: 2.0,
                                top_right: 2.0,
                                bottom_right: 0.0,
                                bottom_left: 0.0,
                            },
                        );
                        if col.partial {
                            frame
                                .stroke(&path, Stroke::default().with_color(color).with_width(1.0));
                        } else {
                            frame.fill(&path, color);
                        }
                    }
                }
                let last = i + 1 == n;
                let color = if !col.existed {
                    alpha(muted, 0.4)
                } else if last {
                    t.text
                } else {
                    muted
                };
                frame.fill_text(Text {
                    font: if last {
                        fonts::INTER_MEDIUM
                    } else {
                        fonts::INTER
                    },
                    ..label(
                        col.label.clone(),
                        Point::new(plot.x + (i as f32 + 0.5) * slot, size.height - 9.0),
                        11.0,
                        color,
                    )
                });
            }
            // Taxa de poupanca: trechos ligam meses vizinhos com receita; ponto em cada um.
            let points: Vec<Option<Point>> = c
                .columns
                .iter()
                .enumerate()
                .map(|(i, col)| {
                    col.rate
                        .map(|r| Point::new(plot.x + (i as f32 + 0.5) * slot, value_y(r, plot)))
                })
                .collect();
            for pair in points.windows(2) {
                if let [Some(a), Some(b)] = pair {
                    frame.stroke(
                        &polyline(&[*a, *b]),
                        Stroke::default().with_color(t.accent).with_width(2.0),
                    );
                }
            }
            for p in points.iter().flatten() {
                frame.fill(&Path::circle(*p, 3.0), t.accent);
            }
        });
        let mut geometries = vec![chart];
        if let Some(i) = *state
            && let Some(col) = c.columns.get(i)
            && let Some(tip) = &col.tooltip
        {
            let mut overlay = Frame::new(renderer, size);
            let col_x = plot.x + i as f32 * slot;
            draw_column_highlight(&mut overlay, t, col_x, slot, plot);
            let top =
                history_bars(col, i, n, plot).map_or(plot.y + plot.height, |[a, b]| a.y.min(b.y));
            draw_text_tooltip(&mut overlay, t, tip, (col_x, slot), top, plot, size);
            geometries.push(overlay.into_geometry());
        }
        geometries
    }
}

/// Tooltip de texto pronto: cada trecho separado por " · " vira uma linha, a primeira em
/// Medium ("out 2026", "receitas R$ X"...). Largura estimada (o canvas nao mede texto).
fn draw_text_tooltip(
    frame: &mut Frame,
    t: &'static Tokens,
    tip: &str,
    (col_x, col_w): (f32, f32),
    bar_top: f32,
    plot: Rectangle,
    size: Size,
) {
    let lines: Vec<&str> = tip.split(" · ").collect();
    let longest = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    let line_h = 15.0;
    let gap = 3.0;
    let box_w = longest as f32 * 6.6 + 20.0;
    let box_h = lines.len() as f32 * line_h + (lines.len().saturating_sub(1)) as f32 * gap + 16.0;
    let (x, y) = tooltip_origin(
        Size::new(box_w, box_h),
        col_x,
        col_w,
        bar_top,
        plot.y,
        size.width,
    );
    let rect = Path::rounded_rectangle(Point::new(x, y), Size::new(box_w, box_h), 8.0.into());
    frame.fill(&rect, t.surface);
    frame.stroke(
        &rect,
        Stroke::default()
            .with_color(t.shadow_md_ring())
            .with_width(1.0),
    );
    for (k, line) in lines.into_iter().enumerate() {
        let ly = y + 8.0 + k as f32 * (line_h + gap) + line_h / 2.0;
        frame.fill_text(Text {
            align_x: TextAlign::Left,
            font: if k == 0 {
                fonts::INTER_MEDIUM
            } else {
                fonts::INTER
            },
            ..label(
                line.to_string(),
                Point::new(x + 10.0, ly),
                12.0,
                if k == 0 { t.text } else { t.text_muted },
            )
        });
    }
}

#[cfg(test)]
mod tests {
    use iced::widget::canvas::Program;

    use super::*;
    use crate::dashboard::money::axis_label;
    use crate::ui::theme::DARK;

    #[test]
    fn pace_points_x_por_dia_e_y_pelo_teto() {
        let plot = pace_plot(Size::new(64.0 + 300.0 + 108.0, PACE_HEIGHT));
        assert_eq!(
            (plot.x, plot.y, plot.width, plot.height),
            (64.0, 6.0, 300.0, 290.0)
        );
        let pts = pace_points(&[0.0, 0.5, 1.0, 1.4], 30, plot);
        // Dia 1 em 10 de 300 (o `viewBox` do design), dia 3 em 30.
        assert_eq!(pts[0], Point::new(74.0, 296.0));
        assert_eq!(pts[1], Point::new(84.0, 151.0));
        assert_eq!(pts[2], Point::new(94.0, 6.0));
        // Passar do teto nao sai da area.
        assert_eq!(pts[3].y, 6.0);
        assert_eq!(day_x(30, 30, plot), 364.0);
        assert_eq!(day_x(5, 0, plot), 64.0);
    }

    #[test]
    fn previsao_vai_do_corte_ao_ultimo_dia() {
        let plot = Rectangle::new(Point::new(0.0, 0.0), Size::new(300.0, 100.0));
        let (a, b) = forecast_segment(19, 30, (0.623, 0.892), plot);
        assert_eq!(a.x, 190.0);
        assert!((a.y - 37.7).abs() < 0.01);
        assert_eq!(b.x, 300.0);
        assert!((b.y - 10.8).abs() < 0.01);
    }

    #[test]
    fn rotulo_da_renda_desce_quando_a_renda_e_o_teto() {
        let plot = pace_plot(Size::new(600.0, PACE_HEIGHT));
        // Renda no meio: acima da linha.
        let mid = value_y(0.5, plot);
        assert_eq!(income_label_y(mid), mid - 12.0);
        // Renda = teto (R$ 5.000 com eixo de R$ 5 mil): a linha fica a 6 px do topo do canvas.
        let top = value_y(1.0, plot);
        assert_eq!(top, 6.0);
        assert_eq!(income_label_y(top), 18.0);
    }

    #[test]
    fn rotulos_da_direita_se_afastam() {
        // Longe: ficam.
        assert_eq!(separate_labels(30.0, 80.0, 16.0, 0.0, 290.0), (30.0, 80.0));
        // Perto: cada um anda metade do que falta.
        assert_eq!(
            separate_labels(100.0, 106.0, 16.0, 0.0, 290.0),
            (95.0, 111.0)
        );
        assert_eq!(
            separate_labels(106.0, 100.0, 16.0, 0.0, 290.0),
            (111.0, 95.0)
        );
        // Encostado no topo: o par desce junto.
        assert_eq!(separate_labels(2.0, 4.0, 16.0, 0.0, 290.0), (0.0, 16.0));
        // E no fundo, sobe.
        assert_eq!(
            separate_labels(289.0, 290.0, 16.0, 0.0, 290.0),
            (274.0, 290.0)
        );
    }

    #[test]
    fn marcas_do_medidor() {
        // Meta nas linhas "em atenção": 1 / 1,3 da escala.
        let (fill, marks) = meter_geometry(200.0, 0.5, &[1.0 / 1.3]);
        assert_eq!(fill, 100.0);
        assert!((marks[0] - 153.846).abs() < 0.01);
        // Comprometido: 50% e 70%; preenchimento nunca passa da barra.
        let (fill, marks) = meter_geometry(300.0, 1.7, &[0.5, 0.7]);
        assert_eq!(fill, 300.0);
        assert_eq!(marks, vec![150.0, 210.0]);
        let meter = |labels: bool| MeterProgram {
            fill: 0.25,
            fill_color: Color::WHITE,
            track: Color::BLACK,
            bar: 6.0,
            marks: vec![(0.5, if labels { "50%".into() } else { String::new() })],
            mark_color: Color::WHITE,
            label_color: Color::WHITE,
        };
        assert_eq!(meter(false).height(), 14.0);
        assert_eq!(meter(true).height(), 34.0);
    }

    fn column(existed: bool, income: f32, expense: f32) -> HistoryColumn {
        HistoryColumn {
            label: "out".into(),
            existed,
            partial: false,
            income,
            expense,
            rate: None,
            tooltip: existed.then(|| "out 2026 · receitas R$ 1,00".into()),
        }
    }

    #[test]
    fn doze_colunas_do_historico_vazias_antes_do_primeiro_mes() {
        let size = Size::new(52.0 + 240.0 + 36.0, HISTORY_HEIGHT);
        let plot = history_plot(size);
        assert_eq!((plot.x, plot.width), (52.0, 240.0));
        let cols: Vec<HistoryColumn> = (0..12).map(|i| column(i >= 9, 1.0, 0.5)).collect();
        let bars: Vec<_> = cols
            .iter()
            .enumerate()
            .map(|(i, c)| history_bars(c, i, 12, plot))
            .collect();
        assert!(bars[..9].iter().all(Option::is_none));
        let [inc, exp] = bars[9].expect("outubro existe");
        // Coluna 9 ocupa 52 + 9 x 20 .. +20; as duas barras centradas nela.
        assert!(inc.x >= 52.0 + 180.0 && exp.x + exp.width <= 52.0 + 200.0);
        assert_eq!(inc.height, plot.height);
        assert_eq!(exp.height, plot.height / 2.0);
        assert_eq!(inc.y + inc.height, plot.y + plot.height);
        assert_eq!(history_column_at(Point::new(60.0, 50.0), size, 12), Some(0));
        assert_eq!(history_column_at(Point::new(10.0, 50.0), size, 12), None);
        assert_eq!(
            history_column_at(Point::new(291.0, 50.0), size, 12),
            Some(11)
        );
        assert_eq!(history_column_at(Point::new(292.0, 50.0), size, 12), None);
    }

    #[test]
    fn hover_do_historico_so_em_mes_com_tooltip() {
        let cache = canvas::Cache::new();
        let chart = HistoryChart {
            columns: (0..12).map(|i| column(i >= 6, 0.8, 0.4)).collect(),
            ..HistoryChart::default()
        };
        let program = HistoryProgram {
            chart: &chart,
            tokens: &DARK,
            cache: &cache,
        };
        let bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(328.0, HISTORY_HEIGHT));
        let moved = |x: f32| {
            (
                Event::Mouse(mouse::Event::CursorMoved {
                    position: Point::new(x, 50.0),
                }),
                mouse::Cursor::Available(Point::new(x, 50.0)),
            )
        };
        let mut state = None;
        // Coluna 0 nao existia: sem tooltip, nada muda.
        let (event, cursor) = moved(60.0);
        assert!(program.update(&mut state, &event, bounds, cursor).is_none());
        assert_eq!(state, None);
        let (event, cursor) = moved(52.0 + 7.0 * 20.0 + 5.0);
        assert!(program.update(&mut state, &event, bounds, cursor).is_some());
        assert_eq!(state, Some(7));
        let (event, cursor) = moved(52.0 + 7.0 * 20.0 + 15.0);
        assert!(program.update(&mut state, &event, bounds, cursor).is_none());
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
            labels: vec!["R$ 1.000,00".into(); 12],
        }];
        let y_labels = ["R$ 6 mil".to_string(), "R$ 3 mil".into(), "R$ 0".into()];
        let totals = vec!["R$ 1.000,00".to_string(); 12];
        let program = StackedBarsProgram {
            series: &series,
            months: &months,
            axis_top: 600_000,
            y_labels: &y_labels,
            month_totals: &totals,
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
    fn tooltip_das_pilhas_usa_os_textos_do_modelo_e_mascara_com_ocultar() {
        use crate::dashboard::fixtures_with_reserves;
        use crate::dashboard::money::{MASK, leaks};
        use crate::dashboard::view::build_reserves;
        let ds = fixtures_with_reserves();
        let shown = build_reserves(&ds, "2026-09-24", false);
        let hidden = build_reserves(&ds, "2026-09-24", true);
        let last = shown.months.len() - 1;
        let tip = stack_tooltip(&shown.series, &shown.month_totals, last);
        assert!(!tip.rows.is_empty());
        assert_eq!(tip.total, "R$ 6.030,00");
        // So reservas com saldo no mes entram, com o valor do modelo.
        let with_balance = shown.series.iter().filter(|s| s.balances[last] > 0).count();
        assert_eq!(tip.rows.len(), with_balance);
        let tip = stack_tooltip(&hidden.series, &hidden.month_totals, last);
        assert_eq!(
            tip.rows.len(),
            with_balance,
            "ocultar nao some com as linhas"
        );
        assert!(tip.rows.iter().all(|(_, v, _)| *v == MASK), "{tip:?}");
        assert_eq!(tip.total, MASK);
        assert_eq!(leaks(&format!("{tip:?}")), None);
        // O eixo tambem: no cache do canvas, entao o toggle limpa o cache.
        assert_eq!(hidden.y_labels, ["•••", "•••", "•••"]);
        assert_eq!(shown.y_labels[0], "R$ 12 mil");
    }

    #[test]
    fn emergencia_em_neutral_300_na_base_da_pilha() {
        use crate::dashboard::fixtures_with_reserves;
        use crate::dashboard::view::build_reserves;
        use crate::ui::theme::LIGHT;
        for t in [&DARK, &LIGHT] {
            assert_eq!(series_color(t, &ReserveColor::Accent), t.neutral_300);
            assert_ne!(series_color(t, &ReserveColor::Accent), t.accent);
            assert_eq!(
                series_color(t, &ReserveColor::Token("amber".into())),
                t.token("amber")
            );
        }
        // A emergencia e a primeira serie: `stack_segments` empilha da primeira para cima.
        let v = build_reserves(&fixtures_with_reserves(), "2026-09-24", false);
        assert_eq!(
            v.series.first().map(|s| &s.color),
            Some(&ReserveColor::Accent)
        );
        assert!(
            v.series
                .iter()
                .skip(1)
                .all(|s| s.color != ReserveColor::Accent)
        );
        // O tooltip leva a mesma cor das barras.
        let last = v.months.len() - 1;
        let tip = stack_tooltip(&v.series, &v.month_totals, last);
        let first = tip.rows.first().map(|(_, _, c)| series_color(&DARK, c));
        assert_eq!(first, Some(DARK.neutral_300));
    }

    #[test]
    fn reticencias_por_caractere() {
        assert_eq!(clip_chars("Mercado", 20), "Mercado");
        assert_eq!(clip_chars("Alimentação e bebidas do mês", 10), "Alimentaç…");
    }
}
