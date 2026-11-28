//! Tile de categoria: glifo Phosphor sobre a cor do token a 18 % (README do handoff).

use gpui::{
    App, Hsla, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div, px,
};

use super::avatar::profile_color;
use super::mix;
use crate::gui::fonts;
use crate::gui::theme::Tokens;
use crate::ui_model::icons::glyph;

/// Fundo do tile: o token misturado a 18 % sobre a superficie.
pub fn tile_bg(color: Hsla, t: &Tokens) -> Hsla {
    mix(t.surface, color, 0.18)
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
            size: 40.0,
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
            .rounded(px(self.size * 0.3))
            .bg(tile_bg(color, t))
            .text_color(color)
            .text_size(px(self.size * 0.5))
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
    fn fundo_e_a_mistura_de_18_por_cento() {
        let t = &*DARK;
        let c = profile_color("rose", t);
        assert_eq!(tile_bg(c, t), mix(t.surface, c, 0.18));
        assert_ne!(tile_bg(c, t), t.surface);
    }
}
