//! Metadado estatico (texto `muted`) x pilula clicavel (borda `divider`, com seta para baixo).
//! O design separa os dois para que o que se clica nunca pareca texto solto.

use gpui::{
    App, ClickEvent, ElementId, FocusHandle, Hsla, IntoElement, RenderOnce, SharedString, Window,
    div, prelude::*, px,
};

use super::focus;
use crate::gui::fonts;
use crate::gui::icons::{Icon, icon};
use crate::gui::theme::Tokens;

type Handler = Box<dyn Fn(&mut Window, &mut App) + 'static>;

/// Texto de metadado, sem interacao.
#[derive(IntoElement)]
pub struct Chip {
    label: SharedString,
    tokens: &'static Tokens,
}

impl Chip {
    pub fn new(label: impl Into<SharedString>, tokens: &'static Tokens) -> Chip {
        Chip {
            label: label.into(),
            tokens,
        }
    }
}

impl RenderOnce for Chip {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .text_size(px(13.0))
            .text_color(self.tokens.muted)
            .child(self.label)
    }
}

/// Pilula clicavel de 32 px (abre um menu ou troca um filtro).
#[derive(IntoElement)]
pub struct Pill {
    id: ElementId,
    label: SharedString,
    tokens: &'static Tokens,
    ring_gap: Option<Hsla>,
    focus: Option<FocusHandle>,
    on_click: Option<Handler>,
}

impl Pill {
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        tokens: &'static Tokens,
    ) -> Pill {
        Pill {
            id: id.into(),
            label: label.into(),
            tokens,
            ring_gap: None,
            focus: None,
            on_click: None,
        }
    }

    pub fn ring_gap(mut self, gap: Hsla) -> Pill {
        self.ring_gap = Some(gap);
        self
    }

    pub fn focus(mut self, handle: &FocusHandle) -> Pill {
        self.focus = Some(handle.clone());
        self
    }

    pub fn on_click(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Pill {
        self.on_click = Some(Box::new(f));
        self
    }
}

impl RenderOnce for Pill {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let t = self.tokens;
        let ring = focus::style(t, self.ring_gap.unwrap_or(t.bg));
        let hover = t.surface_2;
        div()
            .id(self.id)
            .flex()
            .flex_none()
            .items_center()
            .gap(px(6.0))
            .h(px(32.0))
            .px(px(12.0))
            .rounded_full()
            .border_1()
            .border_color(t.divider)
            .text_size(px(13.0))
            .font_weight(fonts::weight(500))
            .text_color(t.text)
            .cursor_pointer()
            .hover(move |s| s.bg(hover))
            .when_some(self.focus, |d, h| d.track_focus(&h))
            .focus_visible(ring)
            .when_some(self.on_click, |d, f| {
                d.on_click(move |_: &ClickEvent, window, cx| f(window, cx))
            })
            .child(self.label)
            .child(icon(Icon::ChevronDown, px(14.0), t.muted))
    }
}
