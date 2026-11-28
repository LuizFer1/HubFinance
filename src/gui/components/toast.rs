//! Toast do design (artboard Estados): fixo embaixo, centrado na area de conteudo, com icone de
//! confirmacao, texto, acao ("Desfazer (6)"), fechar e a barra do tempo que falta.
//!
//! Aqui so se desenha. O prazo e o "pausa com mouse ou foco" vivem em `ui_model::countdown`, e
//! o componente avisa pelo `on_pause(bool)` quando o mouse entra e sai; o foco entra por
//! `watch_focus`. Ligar os dois a um `Msg` e a remocao pendente e da Tarefa 26.

use std::rc::Rc;

use gpui::{
    App, BoxShadow, ClickEvent, ElementId, FocusHandle, IntoElement, RenderOnce, Role,
    SharedString, Subscription, Window, div, point, prelude::*, px,
};

use super::button::IconButton;
use super::focus;
use crate::gui::fonts;
use crate::gui::icons::{Icon, icon};
use crate::gui::motion;
use crate::gui::theme::{Tokens, rgba};

pub const MIN_WIDTH: f32 = 400.0;
pub const MAX_WIDTH: f32 = 520.0;
/// Distancia do toast ate a borda de baixo da area de conteudo.
pub const BOTTOM: f32 = 24.0;
/// Lado do X do toast (artboard Estados).
pub const CLOSE_SIZE: f32 = 32.0;

type Action = Rc<dyn Fn(&mut Window, &mut App) + 'static>;
type Pause = Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>;

/// Largura da barra de tempo (0 a 1 do espaco disponivel); valores fora disso sao cortados.
pub fn bar_fraction(left: f32) -> f32 {
    left.clamp(0.0, 1.0)
}

/// Avisa `f(true)` quando o foco entra no toast e `f(false)` quando sai. As `Subscription`
/// precisam ficar vivas com o toast: soltas, o foco deixaria de pausar o prazo sem erro nenhum.
pub fn watch_focus(
    handle: &FocusHandle,
    window: &mut Window,
    cx: &mut App,
    f: impl Fn(bool, &mut Window, &mut App) + Clone + 'static,
) -> [Subscription; 2] {
    let on_in = f.clone();
    [
        window.on_focus_in(handle, cx, move |window, cx| on_in(true, window, cx)),
        window.on_focus_out(handle, cx, move |_, window, cx| f(false, window, cx)),
    ]
}

/// Os tres focos do toast: o corpo (para `watch_focus` saber que o foco esta dentro) e os dois
/// controles. Quem monta o toast cria uma vez e guarda: handle novo a cada quadro perderia o foco.
#[derive(Clone)]
pub struct ToastFocus {
    pub root: FocusHandle,
    pub action: FocusHandle,
    pub close: FocusHandle,
}

impl ToastFocus {
    pub fn new(cx: &mut App) -> ToastFocus {
        ToastFocus {
            root: cx.focus_handle(),
            action: cx.focus_handle().tab_stop(true),
            close: cx.focus_handle().tab_stop(true),
        }
    }
}

#[derive(IntoElement)]
pub struct Toast {
    id: ElementId,
    tokens: &'static Tokens,
    title: SharedString,
    detail: Option<SharedString>,
    action: Option<(SharedString, Action)>,
    seconds_left: Option<u64>,
    bar: Option<f32>,
    on_close: Option<Action>,
    on_pause: Option<Pause>,
    reduce_motion: bool,
    /// `Some(true)` sobe, `Some(false)` desce; `None` = parado (sem animacao).
    entering: Option<bool>,
    focus: Option<ToastFocus>,
}

impl Toast {
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        tokens: &'static Tokens,
    ) -> Toast {
        Toast {
            id: id.into(),
            tokens,
            title: title.into(),
            detail: None,
            action: None,
            seconds_left: None,
            bar: None,
            on_close: None,
            on_pause: None,
            reduce_motion: false,
            entering: None,
            focus: None,
        }
    }

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Toast {
        self.detail = Some(detail.into());
        self
    }

    pub fn action(
        mut self,
        label: impl Into<SharedString>,
        f: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Toast {
        self.action = Some((label.into(), Rc::new(f)));
        self
    }

    /// Contador do "(6)" no botao de acao.
    pub fn seconds_left(mut self, seconds: u64) -> Toast {
        self.seconds_left = Some(seconds);
        self
    }

    /// Fracao do tempo que falta, 0 a 1 (a barra de baixo).
    pub fn bar(mut self, left: f32) -> Toast {
        self.bar = Some(bar_fraction(left));
        self
    }

    pub fn on_close(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Toast {
        self.on_close = Some(Rc::new(f));
        self
    }

    /// Mouse entrou (`true`) ou saiu (`false`). Sem ligacao a nada ate a Tarefa 26.
    pub fn on_pause(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Toast {
        self.on_pause = Some(Rc::new(f));
        self
    }

    pub fn reduce_motion(mut self, reduce: bool) -> Toast {
        self.reduce_motion = reduce;
        self
    }

    /// Focos do corpo, da acao e do X. Passe `focus.root` a `watch_focus` para pausar o prazo.
    pub fn focus(mut self, focus: &ToastFocus) -> Toast {
        self.focus = Some(focus.clone());
        self
    }

    pub fn entering(mut self, rising: bool) -> Toast {
        self.entering = Some(rising);
        self
    }
}

impl RenderOnce for Toast {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let t = self.tokens;
        let check = div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(24.0))
            .rounded_full()
            .bg(t.ok)
            // Texto escuro fixo do design sobre o verde `ok`, nos dois temas.
            .child(icon(Icon::CheckW32, 13.0, rgba(0x13, 0x13, 0x13, 1.0)));

        let text = div()
            .flex_1()
            .min_w(px(0.0))
            .text_size(px(14.0))
            .child(div().font_weight(fonts::weight(600)).child(self.title))
            .when_some(self.detail, |d, detail| {
                d.child(
                    div()
                        .mt(px(1.0))
                        .text_size(px(12.0))
                        .text_color(t.toast_muted)
                        .child(detail),
                )
            });

        let secs = self.seconds_left;
        let focus = self.focus;
        let ring = focus::style(t, t.toast_bg);
        let action = self.action.map(|(label, f)| {
            let mut d = div().id("toast-action");
            if let Some(f) = &focus {
                d = d.track_focus(&f.action);
            }
            d.flex()
                .items_center()
                .h(px(40.0))
                .px(px(14.0))
                .rounded_full()
                .cursor_pointer()
                .text_size(px(14.0))
                .font_weight(fonts::weight(700))
                .text_color(t.toast_accent)
                .whitespace_nowrap()
                .hover(|s| s.bg(t.toast_fg.opacity(0.10)))
                .focus_visible(ring)
                .on_click(move |_: &ClickEvent, window, cx| f(window, cx))
                .child(label)
                .when_some(secs, |d, s| {
                    d.child(
                        div()
                            .ml(px(4.0))
                            .font_weight(fonts::weight(500))
                            .child(SharedString::from(format!("({s})"))),
                    )
                })
        });

        let mut close = IconButton::new("toast-close", Icon::Close, t)
            .box_size(CLOSE_SIZE)
            .glyph_size(16.0)
            // Sobre o fundo invertido do toast `text` some; o design pinta o X em `toast_muted`.
            .glyph_color(t.toast_muted)
            .aria_label("Fechar aviso")
            .hover_bg(t.toast_fg.opacity(0.10))
            .ring_gap(t.toast_bg);
        if let Some(f) = &focus {
            close = close.focus(&f.close);
        }
        if let Some(f) = self.on_close {
            close = close.on_click(move |window, cx| f(window, cx));
        }

        let bar = self.bar.map(|left| {
            div()
                .absolute()
                .left(px(16.0))
                .right(px(16.0))
                .bottom(px(4.0))
                .h(px(3.0))
                .rounded(px(2.0))
                .bg(t.toast_fg.opacity(0.14))
                .child(
                    div()
                        .h_full()
                        .w(gpui::relative(left))
                        .rounded(px(2.0))
                        .bg(t.toast_accent),
                )
        });

        let pause = self.on_pause;
        let body = div()
            .id(self.id)
            .role(Role::Status)
            .relative()
            .overflow_hidden()
            .flex()
            .items_center()
            .gap(px(12.0))
            .min_w(px(MIN_WIDTH))
            .max_w(px(MAX_WIDTH))
            .pt(px(10.0))
            .pr(px(10.0))
            .pb(px(14.0))
            .pl(px(16.0))
            .rounded(px(16.0))
            .bg(t.toast_bg)
            .text_color(t.toast_fg)
            .shadow(vec![BoxShadow {
                color: rgba(0, 0, 0, 0.3),
                offset: point(px(0.0), px(12.0)),
                blur_radius: px(32.0),
                spread_radius: px(0.0),
                inset: false,
            }])
            .when_some(focus.as_ref(), |d, f| d.track_focus(&f.root))
            .when_some(pause, |d, f| {
                d.on_hover(move |hovered: &bool, window, cx| f(*hovered, window, cx))
            })
            .child(check)
            .child(text)
            .children(action)
            .child(close)
            .children(bar);

        let body = match self.entering {
            Some(rising) => motion::toast_slide(body, self.reduce_motion, rising),
            None => body.into_any_element(),
        };
        // Posicao e de quem desenha o toast: fixo embaixo, centrado na area de conteudo (o
        // ancestral posicionado). O invólucro nao captura o mouse fora do corpo.
        div()
            .absolute()
            .left_0()
            .right_0()
            .bottom(px(BOTTOM))
            .flex()
            .justify_center()
            .child(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn barra_fica_entre_zero_e_um() {
        assert_eq!(bar_fraction(0.62), 0.62);
        assert_eq!(bar_fraction(-1.0), 0.0);
        assert_eq!(bar_fraction(4.0), 1.0);
    }
}
