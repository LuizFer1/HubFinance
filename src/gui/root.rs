//! O estado da interface e a view raiz de cada janela.
//!
//! Duas pecas de proposito: `HubState` (dono do `ui_model::App`, vive sem janela) e
//! `HubWindow` (a view raiz, que so le o estado e repassa teclas). Uma janela GPUI entrega a
//! view raiz a si mesma e a solta ao fechar; se o estado morasse nela, fechar para a bandeja
//! perderia o "Desfazer" pendente, o tique e os eventos da bandeja.

use std::sync::Arc;

use gpui::{
    App, Context, Entity, FocusHandle, Focusable, Global, IntoElement, KeyDownEvent, Keystroke,
    Render, Window, div, prelude::*, px,
};

use super::theme::{self, Tokens};
use super::{effects, fonts};
use crate::dashboard::dataset::Dataset;
use crate::hub::HubHandle;
use crate::hub::snapshot::Snapshot;
use crate::ui_model::app::{App as Model, Msg, update};
use crate::ui_model::keys::{Key, KeyPress};
use crate::ui_model::shell;
use crate::ui_model::window::Size;

pub struct HubState {
    pub app: Model,
    /// Dataset fixo do modo demonstracao (so debug). Guardado para o `Arc` ser o mesmo em todo
    /// snapshot: o modelo reconstroi as telas quando o ponteiro muda.
    demo: Option<Arc<Dataset>>,
    /// Tema efetivo ja aplicado ao `gpui-component` (`None` = nenhum ainda).
    applied_dark: Option<bool>,
}

impl HubState {
    pub fn new(app: Model) -> HubState {
        HubState {
            app,
            demo: demo_dataset(),
            applied_dark: None,
        }
    }

    /// Em modo demonstracao troca os dados do snapshot pelos da fixture; fora dele, nada.
    fn prepare(&self, mut msg: Msg) -> Msg {
        if let (Msg::Snapshot(snapshot), Some(demo)) = (&mut msg, &self.demo) {
            snapshot.dashboard = demo.clone();
        }
        msg
    }
}

/// Dono forte do estado durante o processo inteiro; `open_window` o le daqui.
pub struct HubGlobal(pub Entity<HubState>);

impl Global for HubGlobal {}

/// `HUBFINANCE_DEMO=grafite`: ver as telas com dados sem celular. So em debug.
#[cfg(debug_assertions)]
fn demo_dataset() -> Option<Arc<Dataset>> {
    let demo = std::env::var_os("HUBFINANCE_DEMO")?;
    (demo == "grafite").then(|| Arc::new(crate::dashboard::fixtures_grafite()))
}

/// Nunca em release: um hub de verdade jamais mostra dados de exemplo.
#[cfg(not(debug_assertions))]
fn demo_dataset() -> Option<Arc<Dataset>> {
    None
}

/// O snapshot atual com os dados de demonstracao, se o modo estiver ligado.
pub fn demo_snapshot(handle: &HubHandle) -> Option<Snapshot> {
    demo_dataset().map(|dashboard| Snapshot {
        dashboard,
        ..handle.snapshot.borrow().clone()
    })
}

/// Leva o tema efetivo do modelo ao `gpui-component`, so quando muda: `Theme::change` recarrega
/// o tema inteiro e redesenha todas as janelas, caro demais para rodar a cada tique.
pub fn sync_theme(state: &Entity<HubState>, cx: &mut App) {
    let hub = state.read(cx);
    let dark = hub.app.theme.is_dark(hub.app.system_dark);
    if hub.applied_dark == Some(dark) {
        return;
    }
    theme::apply(Tokens::of(dark), cx);
    state.update(cx, |hub, _| hub.applied_dark = Some(dark));
}

/// Passa a mensagem pelo `update` do modelo, redesenha e executa os efeitos.
pub fn dispatch(state: &Entity<HubState>, msg: Msg, cx: &mut App) {
    let effects = state.update(cx, |hub, cx| {
        let msg = hub.prepare(msg);
        let effects = update(&mut hub.app, msg);
        cx.notify();
        effects
    });
    sync_theme(state, cx);
    for effect in effects {
        effects::run(effect, state, cx);
    }
}

/// O mesmo que `dispatch`, depois do callback atual. Callbacks da janela (tecla, fechar,
/// redimensionar) rodam com a janela emprestada, e um efeito como "fechar a janela" a
/// emprestaria de novo: o GPUI entra em panico. Adiar para a proxima volta do laco evita.
pub fn dispatch_later(state: &Entity<HubState>, msg: Msg, cx: &mut App) {
    let state = state.clone();
    cx.spawn(async move |cx| {
        cx.update(|cx| dispatch(&state, msg, cx));
    })
    .detach();
}

/// A tecla do GPUI no vocabulario do `ui_model`.
///
/// O GPUI nao informa a posicao fisica da tecla, so o caractere: em teclado AZERTY, Ctrl+1 e
/// Ctrl+& (a iced resolve pela posicao). Limite aceito; o dono confere no teclado dele.
pub fn key_press(keystroke: &Keystroke, repeat: bool) -> KeyPress {
    let key = match keystroke.key.as_str() {
        "escape" => Key::Escape,
        "tab" => Key::Tab,
        "left" => Key::ArrowLeft,
        "right" => Key::ArrowRight,
        other => {
            let mut chars = other.chars();
            match (chars.next(), chars.next()) {
                (Some(c @ '1'..='9'), None) => Key::Digit(c as u8 - b'0'),
                (Some(c), None) => Key::Letter(c.to_ascii_lowercase()),
                _ => Key::Other,
            }
        }
    };
    KeyPress {
        key,
        ctrl: keystroke.modifiers.control,
        shift: keystroke.modifiers.shift,
        alt: keystroke.modifiers.alt,
        repeat,
    }
}

pub struct HubWindow {
    state: Entity<HubState>,
    focus: FocusHandle,
}

impl HubWindow {
    pub fn new(state: Entity<HubState>, window: &mut Window, cx: &mut Context<Self>) -> HubWindow {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        let watched = state.clone();
        cx.observe_window_bounds(window, move |_, window, cx| {
            dispatch_later(&watched, Msg::Resized(viewport(window)), cx);
        })
        .detach();
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        HubWindow { state, focus }
    }
}

/// Tamanho logico da area da janela.
pub fn viewport(window: &Window) -> Size {
    let size = window.viewport_size();
    Size::new(f32::from(size.width), f32::from(size.height))
}

impl Focusable for HubWindow {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for HubWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app = &self.state.read(cx).app;
        let tokens = Tokens::of(app.theme.is_dark(app.system_dark));
        let (title, subtitle) = shell::titles(app);
        let title = title.to_string();
        let state = self.state.clone();
        div()
            .id("hub-root")
            .track_focus(&self.focus)
            .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                let press = key_press(&event.keystroke, event.is_held);
                dispatch_later(&state, Msg::KeyPressed(press), cx);
            })
            .size_full()
            .bg(tokens.bg)
            .text_color(tokens.text)
            .font_family(fonts::FAMILY)
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_2()
            .child(div().text_size(px(30.0)).child(title))
            .child(div().text_size(px(14.0)).child(subtitle))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(text: &'static str) -> KeyPress {
        key_press(&Keystroke::parse(text).expect("tecla valida"), false)
    }

    #[test]
    fn teclas_do_gpui_viram_as_do_modelo() {
        assert_eq!(press("escape").key, Key::Escape);
        assert_eq!(press("tab").key, Key::Tab);
        assert_eq!(press("left").key, Key::ArrowLeft);
        assert_eq!(press("right").key, Key::ArrowRight);
        let p = press("ctrl-3");
        assert_eq!(p.key, Key::Digit(3));
        assert!(p.ctrl && !p.shift && !p.alt && !p.repeat);
        assert_eq!(press("ctrl-h").key, Key::Letter('h'));
        // O zero nao e atalho de tela: fica como caractere, como na iced.
        assert_eq!(press("ctrl-0").key, Key::Letter('0'));
        assert_eq!(press("f5").key, Key::Other);
        let shifted = press("shift-tab");
        assert!(shifted.shift && !shifted.ctrl);
        assert!(press("alt-left").alt);
    }

    #[test]
    fn tecla_segurada_vira_repeticao() {
        let held = key_press(&Keystroke::parse("tab").expect("tecla valida"), true);
        assert!(held.repeat);
    }
}
