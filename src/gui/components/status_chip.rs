//! Chip de status: sempre ícone ou texto além da cor (nunca só cor).

use gpui::{
    App, Hsla, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div, px,
};

use crate::gui::fonts;
use crate::gui::icons::{Icon, icon};
use crate::gui::theme::Tokens;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Verde com check.
    Ok,
    /// Laranja com "!".
    Warning,
}

/// Cor do texto e do fundo (16 % da mesma cor).
pub fn colors(kind: Kind, t: &Tokens) -> (Hsla, Hsla) {
    let c = match kind {
        Kind::Ok => t.ok,
        Kind::Warning => t.warning,
    };
    (c, c.opacity(0.16))
}

#[derive(IntoElement)]
pub struct StatusChip {
    kind: Kind,
    label: SharedString,
    tokens: &'static Tokens,
}

impl StatusChip {
    pub fn new(kind: Kind, label: impl Into<SharedString>, tokens: &'static Tokens) -> StatusChip {
        StatusChip {
            kind,
            label: label.into(),
            tokens,
        }
    }
}

impl RenderOnce for StatusChip {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let t = self.tokens;
        let (fg, bg) = colors(self.kind, t);
        let mark = match self.kind {
            Kind::Ok => icon(Icon::CheckW3, px(12.0), fg).into_any_element(),
            Kind::Warning => div()
                .font_weight(fonts::weight(800))
                .child("!")
                .into_any_element(),
        };
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(6.0))
            .h(px(26.0))
            .px(px(10.0))
            .rounded_full()
            .bg(bg)
            .text_color(fg)
            .text_size(px(12.0))
            .font_weight(fonts::weight(600))
            .child(mark)
            .child(self.label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::theme::{DARK, LIGHT};

    #[test]
    fn cores_por_tipo_nos_dois_temas() {
        for t in [&*DARK, &*LIGHT] {
            assert_eq!(colors(Kind::Ok, t).0, t.ok);
            assert_eq!(colors(Kind::Warning, t).0, t.warning);
            assert_eq!(colors(Kind::Ok, t).1.a, 0.16);
        }
    }
}
