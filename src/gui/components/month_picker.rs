//! Seletor de mês de 44 px: "‹ Novembro 2026 ›". O "›" fica desabilitado no mês corrente (não
//! há mês futuro com dados) e o chip "Parcial · até 27 nov" avisa que o mês ainda corre.

use gpui::{
    App, ClickEvent, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div,
    prelude::*, px,
};

use crate::gui::fonts;
use crate::gui::icons::{Icon, icon};
use crate::gui::theme::Tokens;

type Handler = Box<dyn Fn(&mut Window, &mut App) + 'static>;

/// Opacidade da seta desabilitada (design: 0,35).
pub const DISABLED_OPACITY: f32 = 0.35;

/// O mês seguinte só existe se o mostrado for anterior ao corrente (`YYYY-MM` compara por
/// string porque tem largura fixa).
pub fn can_go_next(shown: &str, current: &str) -> bool {
    shown < current
}

#[derive(IntoElement)]
pub struct MonthPicker {
    label: SharedString,
    can_next: bool,
    on_prev: Option<Handler>,
    on_next: Option<Handler>,
    tokens: &'static Tokens,
}

impl MonthPicker {
    pub fn new(label: impl Into<SharedString>, can_next: bool, tokens: &'static Tokens) -> Self {
        MonthPicker {
            label: label.into(),
            can_next,
            on_prev: None,
            on_next: None,
            tokens,
        }
    }

    pub fn on_prev(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_prev = Some(Box::new(f));
        self
    }

    pub fn on_next(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_next = Some(Box::new(f));
        self
    }
}

impl RenderOnce for MonthPicker {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let t = self.tokens;
        let arrow = |id: &'static str, glyph: Icon, enabled: bool, on: Option<Handler>| {
            let el = div()
                .id(id)
                .flex()
                .items_center()
                .justify_center()
                .size(px(36.0))
                .rounded_full();
            if enabled {
                let hover = t.surface;
                el.cursor_pointer()
                    .hover(move |s| s.bg(hover))
                    .when_some(on, |d, f| d.on_click(move |_: &ClickEvent, w, cx| f(w, cx)))
                    .child(icon(glyph, px(16.0), t.text))
            } else {
                el.opacity(DISABLED_OPACITY)
                    .child(icon(glyph, px(16.0), t.text))
            }
        };
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(4.0))
            .h(px(44.0))
            .px(px(4.0))
            .rounded_full()
            .bg(t.surface_2)
            .text_color(t.text)
            .font_weight(fonts::weight(600))
            .child(arrow("month-prev", Icon::ChevronLeft, true, self.on_prev))
            .child(self.label)
            .child(arrow(
                "month-next",
                Icon::ChevronRight,
                self.can_next,
                self.on_next,
            ))
    }
}

/// Chip "Parcial · até 27 nov": meio círculo cheio em `warning` + texto `muted`.
#[derive(IntoElement)]
pub struct PartialChip {
    label: SharedString,
    tokens: &'static Tokens,
}

impl PartialChip {
    pub fn new(label: impl Into<SharedString>, tokens: &'static Tokens) -> PartialChip {
        PartialChip {
            label: label.into(),
            tokens,
        }
    }
}

impl RenderOnce for PartialChip {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let t = self.tokens;
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(8.0))
            .h(px(32.0))
            .px(px(12.0))
            .rounded_full()
            .border_1()
            .border_color(t.divider)
            .text_size(px(12.0))
            .font_weight(fonts::weight(500))
            .text_color(t.muted)
            .child(icon(Icon::HalfCircleWarning, px(14.0), t.warning))
            .child(self.label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proximo_so_antes_do_mes_corrente() {
        assert!(can_go_next("2026-10", "2026-11"));
        assert!(!can_go_next("2026-11", "2026-11"));
        assert!(can_go_next("2025-12", "2026-01"));
    }
}
