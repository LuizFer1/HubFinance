//! Estado vazio: ícone num círculo `accent_900`, título, texto e botão opcional.
//!
//! **Contrato: sem padding nem card próprio.** No artboard Estados o estado vazio É o card
//! (`padding: 32px 24px` no lugar dos 24 padrão), então quem usa envolve em
//! `Card::new(t).px(px(24.0)).py(px(32.0))`. Se o componente também tivesse padding, ele se
//! somaria ao do `Card` (48/56 px) e os títulos quebrariam de linha.

use gpui::{
    AnyElement, App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div, px,
};

use crate::gui::fonts;
use crate::gui::icons::{Icon, icon};
use crate::gui::theme::Tokens;

#[derive(IntoElement)]
pub struct EmptyState {
    icon: Icon,
    title: SharedString,
    text: SharedString,
    action: Option<AnyElement>,
    tokens: &'static Tokens,
}

impl EmptyState {
    pub fn new(
        icon: Icon,
        title: impl Into<SharedString>,
        text: impl Into<SharedString>,
        tokens: &'static Tokens,
    ) -> EmptyState {
        EmptyState {
            icon,
            title: title.into(),
            text: text.into(),
            action: None,
            tokens,
        }
    }

    pub fn action(mut self, button: impl IntoElement) -> EmptyState {
        self.action = Some(button.into_any_element());
        self
    }
}

impl RenderOnce for EmptyState {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let t = self.tokens;
        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(12.0))
            .text_center()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(48.0))
                    .rounded_full()
                    .bg(t.accent_900)
                    .child(icon(self.icon, px(22.0), t.accent_200)),
            )
            .child(
                div()
                    .text_size(px(15.0))
                    .font_weight(fonts::weight(600))
                    .child(self.title),
            )
            .child(
                div()
                    .max_w(px(380.0))
                    .text_size(px(13.0))
                    .line_height(px(20.0))
                    .text_color(t.muted)
                    .child(self.text),
            )
            .children(self.action)
    }
}
