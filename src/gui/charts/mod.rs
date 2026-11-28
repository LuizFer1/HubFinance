//! Graficos do design. A geometria e pura (`geometry`); aqui so se pinta com `canvas()`.
//!
//! Rotulos (eixo, valores, meses) sao texto GPUI por cima do canvas, nunca pintados: assim
//! "Ocultar valores" so troca o texto e nao ha cache de desenho para invalidar (spec §8).
//! Nenhum grafico formata dinheiro: os textos chegam prontos do `dashboard/`.
#![allow(dead_code)] // usado na Tarefa 19 em diante (a 18 ja usa parte)

pub mod bars;
pub mod donut;
pub mod geometry;
pub mod ring;
pub mod stacked;

use gpui::{Bounds, Pixels, Point, Window, point, px};

use geometry::Pt;

/// Ponto do grafico (origem no canvas) para a janela.
pub(crate) fn at(origin: Point<Pixels>, p: Pt) -> Point<Pixels> {
    point(origin.x + px(p.x), origin.y + px(p.y))
}

/// Caminho que nao constroi (coordenada degenerada) some em vez de derrubar a janela.
pub(crate) fn paint(
    window: &mut Window,
    builder: gpui::PathBuilder,
    color: impl Into<gpui::Background>,
) {
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

/// Area util = o canvas inteiro: os rotulos do eixo sao texto ao lado, nao dentro dele.
pub(crate) fn plot_of(bounds: Bounds<Pixels>) -> geometry::Rect {
    geometry::Rect {
        x: 0.0,
        y: 0.0,
        w: f32::from(bounds.size.width),
        h: f32::from(bounds.size.height),
    }
}
