//! Interruptor 48 x 28: desligado `divider`, ligado `accent`, bolinha de 22 px `surface`.

use gpui::{
    App, BoxShadow, ClickEvent, ElementId, FocusHandle, Hsla, IntoElement, RenderOnce, Window, div,
    point, prelude::*, px,
};

use super::focus;
use crate::gui::theme::{Tokens, rgba};

pub fn track_color(on: bool, t: &Tokens) -> Hsla {
    if on { t.accent } else { t.divider }
}

type Handler = Box<dyn Fn(bool, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct Switch {
    id: ElementId,
    on: bool,
    disabled: bool,
    tokens: &'static Tokens,
    ring_gap: Option<Hsla>,
    focus: Option<FocusHandle>,
    on_toggle: Option<Handler>,
}

impl Switch {
    pub fn new(id: impl Into<ElementId>, on: bool, tokens: &'static Tokens) -> Switch {
        Switch {
            id: id.into(),
            on,
            disabled: false,
            tokens,
            ring_gap: None,
            focus: None,
            on_toggle: None,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Switch {
        self.disabled = disabled;
        self
    }

    pub fn ring_gap(mut self, gap: Hsla) -> Switch {
        self.ring_gap = Some(gap);
        self
    }

    pub fn focus(mut self, handle: &FocusHandle) -> Switch {
        self.focus = Some(handle.clone());
        self
    }

    /// Recebe o valor novo (o contrario do atual); quem desenha guarda o estado.
    pub fn on_toggle(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Switch {
        self.on_toggle = Some(Box::new(f));
        self
    }
}

impl RenderOnce for Switch {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let t = self.tokens;
        let enabled = !self.disabled;
        let next = !self.on;
        let ring = focus::style(t, self.ring_gap.unwrap_or(t.bg));
        let handler = self.on_toggle;
        div()
            .id(self.id)
            .flex()
            .flex_none()
            .items_center()
            .w(px(48.0))
            .h(px(28.0))
            .p(px(3.0))
            .rounded_full()
            .bg(track_color(self.on, t))
            .when(self.on, |d| d.justify_end())
            // Desabilitado nao entra na ordem do Tab: pararia num alvo sem anel e sem acao.
            .when_some(self.focus.filter(|_| enabled), |d, h| d.track_focus(&h))
            .when(enabled, |d| d.cursor_pointer().focus_visible(ring))
            .when(!enabled, |d| d.opacity(0.4))
            .when_some(handler.filter(|_| enabled), |d, f| {
                d.on_click(move |_: &ClickEvent, window, cx| f(next, window, cx))
            })
            .child(
                div()
                    .size(px(22.0))
                    .rounded_full()
                    .bg(t.surface)
                    .shadow(vec![BoxShadow {
                        color: rgba(0, 0, 0, 0.25),
                        offset: point(px(0.0), px(1.0)),
                        blur_radius: px(2.0),
                        spread_radius: px(0.0),
                        inset: false,
                    }]),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::theme::{DARK, LIGHT};

    #[test]
    fn trilho_ligado_e_accent_desligado_e_divider() {
        for t in [&*DARK, &*LIGHT] {
            assert_eq!(track_color(true, t), t.accent);
            assert_eq!(track_color(false, t), t.divider);
        }
    }
}
