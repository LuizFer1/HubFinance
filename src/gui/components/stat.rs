//! Estatistica: rotulo (muted), valor e nota opcional.

use gpui::{App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div, px};

use super::money::Money;
use crate::gui::theme::Tokens;

#[derive(IntoElement)]
pub struct Stat {
    label: SharedString,
    value: SharedString,
    note: Option<SharedString>,
    size: f32,
    hidden: bool,
    tokens: &'static Tokens,
}

impl Stat {
    pub fn new(
        label: impl Into<SharedString>,
        value: impl Into<SharedString>,
        tokens: &'static Tokens,
    ) -> Stat {
        Stat {
            label: label.into(),
            value: value.into(),
            note: None,
            size: 30.0,
            hidden: false,
            tokens,
        }
    }

    pub fn note(mut self, note: impl Into<SharedString>) -> Stat {
        self.note = Some(note.into());
        self
    }

    pub fn size(mut self, size: f32) -> Stat {
        self.size = size;
        self
    }

    pub fn hidden(mut self, hidden: bool) -> Stat {
        self.hidden = hidden;
        self
    }
}

impl RenderOnce for Stat {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let t = self.tokens;
        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(t.muted)
                    .child(self.label),
            )
            .child(Money::new(self.value, self.size, t).hidden(self.hidden))
            .children(
                self.note
                    .map(|n| div().text_size(px(12.0)).text_color(t.muted).child(n)),
            )
    }
}
