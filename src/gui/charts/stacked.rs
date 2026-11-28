//! Area empilhada da "Evolucao" das reservas; o trecho do mes parcial e tracejado.

use gpui::{
    App, BorderStyle, Bounds, Corners, Edges, Hsla, IntoElement, LineCap, LineJoin, PathBuilder,
    PathStyle, RenderOnce, Size, StrokeOptions, Styled, Window, canvas, linear_color_stop,
    linear_gradient, px, quad,
};

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
    /// Fundo do cartao: miolo dos pontos e traco do marcador final.
    surface: Hsla,
}

/// Linha 2,5 px com ponta e juncao redondas (Reservas).
fn stroke(dash: bool) -> PathBuilder {
    let b = PathBuilder::stroke(px(2.5)).with_style(PathStyle::Stroke(StrokeOptions {
        line_width: 2.5,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Default::default()
    }));
    if dash {
        b.dash_array(&[px(4.0), px(4.0)])
    } else {
        b
    }
}

/// Bolinha: raio externo `r + borda/2`, miolo `fill`, borda `edge` por dentro do quad.
fn dot(
    window: &mut Window,
    center: gpui::Point<gpui::Pixels>,
    r: f32,
    border: f32,
    fill: Hsla,
    edge: Hsla,
) {
    let outer = r + border / 2.0;
    window.paint_quad(quad(
        Bounds {
            origin: gpui::point(center.x - px(outer), center.y - px(outer)),
            size: Size {
                width: px(2.0 * outer),
                height: px(2.0 * outer),
            },
        },
        Corners::all(px(outer)),
        fill,
        Edges::all(px(border)),
        edge,
        BorderStyle::default(),
    ));
}

impl StackedArea {
    pub fn new(
        series: Vec<Vec<i64>>,
        colors: Vec<Hsla>,
        top: i64,
        height: f32,
        surface: Hsla,
    ) -> Self {
        StackedArea {
            series,
            colors,
            top,
            height,
            partial: false,
            surface,
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
            surface,
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
                    // Gradiente vertical do design: 0,26 no topo, 0,02 na base.
                    paint(
                        window,
                        fill,
                        linear_gradient(
                            180.0,
                            linear_color_stop(color.opacity(0.26), 0.0),
                            linear_color_stop(color.opacity(0.02), 1.0),
                        ),
                    );
                    // Borda de cima: a primeira metade do poligono (a outra volta pela base).
                    let top_edge: &[Pt] = &poly[..poly.len() / 2];
                    let (solid, dashed) = geometry::partial_segments(top_edge.len(), partial);
                    let line = |range: std::ops::Range<usize>, dash: bool| {
                        let mut b = stroke(dash);
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
                    // Pontos (r 3, miolo surface, traco accent 2) e marcador final (r 6, accent,
                    // traco surface 3).
                    let last = top_edge.len().saturating_sub(1);
                    for (i, p) in top_edge.iter().enumerate() {
                        let c = at(bounds.origin, *p);
                        if i == last {
                            dot(window, c, 6.0, 3.0, *color, surface);
                        } else {
                            dot(window, c, 3.0, 2.0, surface, *color);
                        }
                    }
                }
            },
        )
        .w_full()
        .h(px(height))
    }
}
