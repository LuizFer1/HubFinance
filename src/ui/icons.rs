//! Icones da iced: o mapa de glifos mora em `ui_model::icons` (os dois toolkits o usam);
//! aqui ficam so os widgets `text` na fonte Phosphor.

use iced::widget::text;
use iced::{Color, Element};

use super::fonts;

pub use crate::ui_model::icons::*;

/// `text(glyph)` na fonte Phosphor. Tamanho em px e cor explicitos: icone nunca herda.
/// Altura de linha 1.0 para a caixa do icone ter exatamente `size`, como o `font-size` do
/// protótipo; com a altura padrao (1.3) o icone desceria em relacao ao texto ao lado.
pub fn icon<'a, M: 'a>(glyph: &'static str, size: f32, color: Color) -> Element<'a, M> {
    text(glyph)
        .font(fonts::PHOSPHOR)
        .size(size)
        .line_height(1.0)
        .color(color)
        .into()
}

/// Icone sem cor propria: herda a cor de texto do botao em volta (que muda no hover), como o
/// `<i>` do prototipo herda o `color` do pai.
pub fn icon_inherit<'a, M: 'a>(glyph: &'static str, size: f32) -> Element<'a, M> {
    text(glyph)
        .font(fonts::PHOSPHOR)
        .size(size)
        .line_height(1.0)
        .into()
}
