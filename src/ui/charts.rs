//! Graficos do Dashboard em `canvas`: rosca de gastos por categoria, barras de receita x
//! despesa e o esqueleto tracejado do estado "Sem dados".
//!
//! A geometria fica num `canvas::Cache` que a `App` limpa quando dados, mes, hover da legenda
//! ou tema mudam; o hover das barras e estado do proprio `Program` e e desenhado num segundo
//! `Frame`, sem cache, para passar o mouse nao refazer o grafico inteiro.

// Usado pela tela Dashboard na tarefa seguinte do plano 2b; sai quando ela entrar.
#![allow(dead_code)]

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
use crate::dashboard::periods::{month_label, month_short};

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
const AXIS_W: f32 = 52.0;
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
                    ..label(text, Point::new(AXIS_W - 8.0, y), 11.0, muted)
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

/// Fundo a 5 % na coluna e o tooltip acima das barras. O tooltip do prototipo sai por cima do
/// grafico; o canvas recorta no proprio limite, entao aqui ele fica dentro, encostado no topo
/// da barra mais alta (ou no topo do canvas).
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
    let y = (bar_top - 6.0 - box_h).max(0.0);
    let cx = col_x + col_w / 2.0;
    let x = (cx - box_w / 2.0).clamp(0.0, (size.width - box_w).max(0.0));
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
    fn coluna_sob_o_cursor() {
        let size = Size::new(652.0, 210.0);
        // Area util: x de 52 a 652, seis colunas de 100.
        assert_eq!(bar_column(Point::new(10.0, 100.0), size, 6), None, "eixo");
        assert_eq!(bar_column(Point::new(52.0, 100.0), size, 6), Some(0));
        assert_eq!(bar_column(Point::new(151.9, 100.0), size, 6), Some(0));
        assert_eq!(bar_column(Point::new(152.0, 100.0), size, 6), Some(1));
        assert_eq!(bar_column(Point::new(651.0, 205.0), size, 6), Some(5));
        assert_eq!(bar_column(Point::new(652.0, 100.0), size, 6), None);
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
        let bounds = Rectangle::new(Point::new(100.0, 300.0), Size::new(652.0, 210.0));
        let moved = |x: f32, y: f32| {
            (
                Event::Mouse(mouse::Event::CursorMoved {
                    position: Point::new(x, y),
                }),
                mouse::Cursor::Available(Point::new(x, y)),
            )
        };
        let mut state = None;
        let (event, cursor) = moved(100.0 + 160.0, 400.0);
        assert!(program.update(&mut state, &event, bounds, cursor).is_some());
        assert_eq!(state, Some(1));
        // Mesma coluna: nada a redesenhar.
        let (event, cursor) = moved(100.0 + 170.0, 410.0);
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
    fn altura_da_barra_sem_estourar() {
        assert_eq!(bar_height(100, 200, 180.0), 90.0);
        assert_eq!(bar_height(500, 200, 180.0), 180.0);
        assert_eq!(bar_height(-5, 200, 180.0), 0.0);
        assert_eq!(bar_height(5, 0, 180.0), 0.0);
    }

    #[test]
    fn reticencias_por_caractere() {
        assert_eq!(clip_chars("Mercado", 20), "Mercado");
        assert_eq!(clip_chars("Alimentação e bebidas do mês", 10), "Alimentaç…");
    }
}
