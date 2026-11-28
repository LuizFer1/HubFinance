//! Area empilhada da "Evolucao" das reservas; o trecho do mes parcial e tracejado.

use gpui::{App, Hsla, IntoElement, PathBuilder, RenderOnce, Styled, Window, canvas, px};

use super::geometry::{self, Pt};
use super::{at, paint, plot_of};

#[derive(IntoElement)]
pub struct StackedArea {
    /// `series[k][i]` = saldo da reserva `k` no mes `i`, de baixo para cima.
    series: Vec<Vec<i64>>,
    colors: Vec<Hsla>,
    top: i64,
    height: f32,
    partial: bool,
}

impl StackedArea {
    pub fn new(series: Vec<Vec<i64>>, colors: Vec<Hsla>, top: i64, height: f32) -> Self {
        StackedArea {
            series,
            colors,
            top,
            height,
            partial: false,
        }
    }

    /// Ultimo mes ainda em andamento: o trecho que chega nele vai tracejado.
    pub fn partial(mut self, partial: bool) -> Self {
        self.partial = partial;
        self
    }
}

impl RenderOnce for StackedArea {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let StackedArea {
            series,
            colors,
            top,
            height,
            partial,
        } = self;
        canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let polys = geometry::stacked_polygons(&series, top, plot_of(bounds));
                for (poly, color) in polys.iter().zip(colors.iter()) {
                    if poly.len() < 4 {
                        continue;
                    }
                    let mut fill = PathBuilder::fill();
                    let pts: Vec<_> = poly.iter().map(|p| at(bounds.origin, *p)).collect();
                    fill.add_polygon(&pts, true);
                    paint(window, fill, color.opacity(0.35));
                    // Borda de cima: a primeira metade do poligono (a outra volta pela base).
                    let top_edge: &[Pt] = &poly[..poly.len() / 2];
                    let (solid, dashed) = geometry::partial_segments(top_edge.len(), partial);
                    let line = |range: std::ops::Range<usize>, dash: bool| {
                        let mut b = PathBuilder::stroke(px(2.0));
                        if dash {
                            b = b.dash_array(&[px(4.0), px(4.0)]);
                        }
                        let pts: Vec<_> = top_edge[range]
                            .iter()
                            .map(|p| at(bounds.origin, *p))
                            .collect();
                        b.add_polygon(&pts, false);
                        b
                    };
                    paint(window, line(solid, false), *color);
                    if let Some(d) = dashed {
                        paint(window, line(d, true), *color);
                    }
                }
            },
        )
        .w_full()
        .h(px(height))
    }
}
