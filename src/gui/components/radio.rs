//! Radio de 20 px: borda 2 `muted`; marcado, borda `accent` e ponto de 9 px (raio 4,5).

use gpui::{
    App, ClickEvent, ElementId, FocusHandle, Hsla, IntoElement, RenderOnce, Window, div,
    prelude::*, px,
};

use super::focus;
use crate::gui::theme::Tokens;

pub fn border_color(checked: bool, t: &Tokens) -> Hsla {
    if checked { t.accent } else { t.muted }
}

type Handler = Box<dyn Fn(&mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct Radio {
    id: ElementId,
    checked: bool,
    tokens: &'static Tokens,
    ring_gap: Option<Hsla>,
    focus: Option<FocusHandle>,
    on_select: Option<Handler>,
}

impl Radio {
    pub fn new(id: impl Into<ElementId>, checked: bool, tokens: &'static Tokens) -> Radio {
        Radio {
            id: id.into(),
            checked,
            tokens,
            ring_gap: None,
            focus: None,
            on_select: None,
        }
    }

    pub fn ring_gap(mut self, gap: Hsla) -> Radio {
        self.ring_gap = Some(gap);
        self
    }

    pub fn focus(mut self, handle: &FocusHandle) -> Radio {
        self.focus = Some(handle.clone());
        self
    }

    pub fn on_select(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Radio {
        self.on_select = Some(Box::new(f));
        self
    }
}

impl RenderOnce for Radio {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let t = self.tokens;
        let ring = focus::style(t, self.ring_gap.unwrap_or(t.bg));
        let handler = self.on_select;
        div()
            .id(self.id)
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(20.0))
            .rounded_full()
            .border_2()
            .border_color(border_color(self.checked, t))
            .cursor_pointer()
            .when_some(self.focus, |d, h| d.track_focus(&h))
            .focus_visible(ring)
            .when_some(handler, |d, f| {
                d.on_click(move |_: &ClickEvent, window, cx| f(window, cx))
            })
            .when(self.checked, |d| {
                d.child(div().size(px(9.0)).rounded_full().bg(t.accent))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::theme::{DARK, LIGHT};

    #[test]
    fn marcado_usa_accent_vazio_usa_muted() {
        for t in [&*DARK, &*LIGHT] {
            assert_eq!(border_color(true, t), t.accent);
            assert_eq!(border_color(false, t), t.muted);
        }
    }
}
