//! Badge de contagem (alertas da Analise na barra lateral): `danger` a 18 % no fundo, numero
//! `danger` em 700 (SemiBold, spec §4.2). No trilho de icones vira um ponto de 9 px.

use gpui::{App, Hsla, IntoElement, RenderOnce, SharedString, Window, div, prelude::*, px};

use crate::gui::fonts;
use crate::gui::theme::Tokens;

pub fn bg(t: &Tokens) -> Hsla {
    t.danger.opacity(0.18)
}

#[derive(IntoElement)]
pub struct Badge {
    count: u32,
    dot: bool,
    tokens: &'static Tokens,
}

impl Badge {
    pub fn new(count: u32, tokens: &'static Tokens) -> Badge {
        Badge {
            count,
            dot: false,
            tokens,
        }
    }

    /// Ponto de 9 px, sem numero (trilho abaixo de 1024 px).
    pub fn dot(mut self) -> Badge {
        self.dot = true;
        self
    }
}

impl RenderOnce for Badge {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let t = self.tokens;
        if self.dot {
            return div().size(px(9.0)).rounded_full().bg(t.danger);
        }
        let label: SharedString = self.count.to_string().into();
        div()
            .flex()
            .items_center()
            .justify_center()
            .min_w(px(22.0))
            .h(px(22.0))
            .px(px(7.0))
            .rounded_full()
            .bg(bg(t))
            .text_size(px(12.0))
            .font_weight(fonts::weight(700))
            .text_color(t.danger)
            .child(label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::theme::{DARK, LIGHT};

    #[test]
    fn fundo_e_danger_a_18_por_cento() {
        for t in [&*DARK, &*LIGHT] {
            let c = bg(t);
            assert!((c.a - 0.18).abs() < 1e-6);
            assert_eq!((c.h, c.s, c.l), (t.danger.h, t.danger.s, t.danger.l));
        }
    }
}
