//! Dialogo do design (artboards Remover, Assistente): scrim sobre a janela e cartao de 520 px.
//!
//! O `Modal` preenche o ancestral posicionado (a view raiz da janela deve ser `relative`). Fecha
//! pelo X, pelo clique no scrim e por Esc, sempre chamando o mesmo `on_close`: quem desenha
//! decide o `Msg` (fechar, voltar um passo, cancelar). O overlay inteiro captura o mouse, para o
//! clique no scrim nunca "vazar" para um botao da tela por baixo.

use std::rc::Rc;

use gpui::{
    AnyElement, App, ClickEvent, ElementId, FocusHandle, IntoElement, KeyDownEvent, ParentElement,
    RenderOnce, SharedString, Styled as _, Window, div, prelude::*, px,
};

use super::button::IconButton;
use crate::gui::fonts;
use crate::gui::icons::Icon;
use crate::gui::motion;
use crate::gui::theme::Tokens;

/// Largura do cartao, raio e preenchimento: medidas do artboard Remover.
pub const WIDTH: f32 = 520.0;
pub const RADIUS: f32 = 24.0;
pub const PADDING: f32 = 28.0;

type Close = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct Modal {
    id: ElementId,
    tokens: &'static Tokens,
    title: Option<SharedString>,
    header: Option<AnyElement>,
    on_close: Option<Close>,
    reduce_motion: bool,
    focus: Option<FocusHandle>,
    children: Vec<AnyElement>,
}

impl Modal {
    pub fn new(id: impl Into<ElementId>, tokens: &'static Tokens) -> Modal {
        Modal {
            id: id.into(),
            tokens,
            title: None,
            header: None,
            on_close: None,
            reduce_motion: false,
            focus: None,
            children: Vec::new(),
        }
    }

    /// `h2` de 22 px logo abaixo do cabecalho.
    pub fn title(mut self, title: impl Into<SharedString>) -> Modal {
        self.title = Some(title.into());
        self
    }

    /// Ao lado do X (normalmente um `Stepper`); sem ele o X vai para a direita sozinho.
    pub fn header(mut self, header: impl IntoElement) -> Modal {
        self.header = Some(header.into_any_element());
        self
    }

    pub fn on_close(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Modal {
        self.on_close = Some(Rc::new(f));
        self
    }

    pub fn reduce_motion(mut self, reduce: bool) -> Modal {
        self.reduce_motion = reduce;
        self
    }

    /// Foco do dialogo: sem ele o Esc so chega se um filho (campo, botao) estiver focado.
    pub fn focus(mut self, handle: &FocusHandle) -> Modal {
        self.focus = Some(handle.clone());
        self
    }
}

impl ParentElement for Modal {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

/// `escape` fecha; qualquer outra tecla segue para os filhos.
pub fn closes_on(key: &str) -> bool {
    key == "escape"
}

impl RenderOnce for Modal {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let t = self.tokens;
        let close_on_scrim = self.on_close.clone();
        let close_on_x = self.on_close.clone();
        let close_on_key = self.on_close;

        let scrim = div()
            .id("modal-scrim")
            .absolute()
            .inset_0()
            .bg(t.scrim)
            .when_some(close_on_scrim, |d, f| {
                d.on_click(move |_: &ClickEvent, window, cx| f(window, cx))
            });

        let mut close = IconButton::new("modal-close", Icon::Close, t)
            .glyph_size(18.0)
            .ring_gap(t.surface);
        if let Some(f) = close_on_x {
            close = close.on_click(move |window, cx| f(window, cx));
        }

        let top = div()
            .flex()
            .items_center()
            .gap(px(16.0))
            .child(
                self.header
                    .unwrap_or_else(|| div().flex_1().into_any_element()),
            )
            .child(div().flex_none().child(close));

        let dialog = div()
            .id("modal-dialog")
            .flex()
            .flex_col()
            .gap(px(22.0))
            .w(px(WIDTH))
            .max_w_full()
            .p(px(PADDING))
            .rounded(px(RADIUS))
            .bg(t.surface)
            .border_1()
            .border_color(t.divider)
            .shadow(vec![t.dialog_shadow.clone()])
            .text_color(t.text)
            // O cartao e irmao do scrim: sem `occlude` o clique dentro dele tambem chegaria ao
            // scrim e fecharia o dialogo.
            .occlude()
            .child(top)
            .when_some(self.title, |d, title| {
                d.child(
                    div()
                        .text_size(px(22.0))
                        .line_height(px(27.5))
                        .font_weight(fonts::weight(600))
                        .child(title),
                )
            })
            .children(self.children);

        div()
            .id(self.id)
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .occlude()
            .when_some(self.focus, |d, h| d.track_focus(&h))
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                if let Some(f) = &close_on_key
                    && closes_on(&event.keystroke.key)
                {
                    cx.stop_propagation();
                    f(window, cx);
                }
            })
            .child(motion::scrim_in(
                scrim,
                "modal-scrim-in",
                self.reduce_motion,
            ))
            .child(dialog)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn so_escape_fecha() {
        assert!(closes_on("escape"));
        assert!(!closes_on("enter"));
        assert!(!closes_on("tab"));
    }

    #[test]
    fn medidas_do_artboard_remover() {
        assert_eq!((WIDTH, RADIUS, PADDING), (520.0, 24.0, 28.0));
    }
}
