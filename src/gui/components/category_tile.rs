//! Tile de categoria (artboard Main, "Recentes"): circulo de 36 px, cor do token a 14 % sobre
//! transparente (o fundo do card aparece) e glifo Phosphor de 16 px.

use gpui::{
    App, Hsla, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div, px,
};

use super::avatar::profile_color;
use crate::gui::fonts;
use crate::gui::theme::Tokens;
use crate::ui_model::icons::glyph;

/// Opacidade do fundo (`color-mix(in srgb, cor 14%, transparent)` do design).
pub const BG_OPACITY: f32 = 0.14;
pub const SIZE: f32 = 36.0;
pub const GLYPH: f32 = 16.0;

/// Fundo do tile: o token a 14 %, sobre transparente.
pub fn tile_bg(color: Hsla) -> Hsla {
    color.opacity(BG_OPACITY)
}

#[derive(IntoElement)]
pub struct CategoryTile {
    icon_key: SharedString,
    color_token: SharedString,
    size: f32,
    tokens: &'static Tokens,
}

impl CategoryTile {
    pub fn new(
        icon_key: impl Into<SharedString>,
        color_token: impl Into<SharedString>,
        tokens: &'static Tokens,
    ) -> CategoryTile {
        CategoryTile {
            icon_key: icon_key.into(),
            color_token: color_token.into(),
            size: SIZE,
            tokens,
        }
    }

    pub fn size(mut self, size: f32) -> CategoryTile {
        self.size = size;
        self
    }
}

impl RenderOnce for CategoryTile {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let t = self.tokens;
        let color = profile_color(&self.color_token, t);
        div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(self.size))
            .rounded_full()
            .bg(tile_bg(color))
            .text_color(color)
            .text_size(px(self.size * GLYPH / SIZE))
            .font_family(fonts::ICON_FAMILY)
            // Chave desconhecida cai no glifo neutro de `glyph`, nunca em quadrado vazio.
            .child(glyph(&self.icon_key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::theme::DARK;

    #[test]
    fn fundo_e_a_cor_a_14_por_cento_sobre_transparente() {
        let t = &*DARK;
        let c = profile_color("rose", t);
        let bg = tile_bg(c);
        assert_eq!(bg.a, BG_OPACITY);
        assert_eq!((bg.h, bg.s, bg.l), (c.h, c.s, c.l));
    }
}
