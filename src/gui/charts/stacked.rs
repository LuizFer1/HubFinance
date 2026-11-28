//! Area empilhada da "Evolucao" das reservas; o trecho do mes parcial e tracejado.

use gpui::{
    App, BorderStyle, Bounds, Corners, Edges, ElementId, Hsla, IntoElement, LineCap, LineJoin,
    PathBuilder, PathStyle, RenderOnce, Role, SharedString, Size, StrokeOptions, Styled, Window,
    canvas, div, linear_color_stop, linear_gradient, prelude::*, px, quad,
};

use super::geometry::{self, Pt};
use super::{at, paint, plot_of};

#[derive(IntoElement)]
pub struct StackedArea {
    label: SharedString,
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
        label: impl Into<SharedString>,
        series: Vec<Vec<i64>>,
        colors: Vec<Hsla>,
        top: i64,
        height: f32,
        surface: Hsla,
    ) -> Self {
        StackedArea {
            label: label.into(),
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

/// Pontos da borda de cima de um poligono (metade; a outra volta pela base).
fn top_edge_len(poly_len: usize) -> usize {
    poly_len / 2
}

/// Area e linha precisam de ao menos 2 pontos na borda de cima; com 1 so o ponto e pintado.
fn has_area(top_len: usize) -> bool {
    top_len >= 2
}

impl RenderOnce for StackedArea {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let StackedArea {
            label,
            series,
            colors,
            top,
            height,
            partial,
            surface,
        } = self;
        let layer = canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let polys = geometry::stacked_polygons(&series, top, plot_of(bounds));
                for (poly, color) in polys.iter().zip(colors.iter()) {
                    let top_len = top_edge_len(poly.len());
                    if top_len == 0 {
                        continue;
                    }
                    // Borda de cima: a primeira metade do poligono (a outra volta pela base).
                    let top_edge: &[Pt] = &poly[..top_len];
                    // Um mes so: sem area nem linha (nao ha segmento), mas o marcador final fica.
                    if has_area(top_len) {
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
                    }
                    let (solid, dashed) = if has_area(top_len) {
                        geometry::partial_segments(top_len, partial)
                    } else {
                        (0..0, None)
                    };
                    let line = |range: std::ops::Range<usize>, dash: bool| {
                        let mut b = stroke(dash);
                        let pts: Vec<_> = top_edge[range]
                            .iter()
                            .map(|p| at(bounds.origin, *p))
                            .collect();
                        b.add_polygon(&pts, false);
                        b
                    };
                    if has_area(top_len) {
                        paint(window, line(solid, false), *color);
                        if let Some(d) = dashed {
                            paint(window, line(d, true), *color);
                        }
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
        .size_full();
        div()
            .id(ElementId::Name(format!("stacked-{}", label).into()))
            .role(Role::Image)
            .aria_label(label)
            .w_full()
            .h(px(height))
            .child(layer)
    }
}

#[cfg(test)]
mod pintura_tests {
    use super::*;

    #[test]
    fn um_mes_pinta_so_o_marcador() {
        // stacked_polygons devolve 2 pontos para n == 1.
        assert_eq!(top_edge_len(2), 1);
        assert!(!has_area(top_edge_len(2)));
        assert_eq!(top_edge_len(0), 0);
        assert!(has_area(top_edge_len(6)));
    }
}
