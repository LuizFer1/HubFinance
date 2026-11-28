//! A casca da janela: barra de titulo (Tarefa 14), lateral, card de status e cabecalho
//! (Tarefa 15).

pub mod header;
pub mod sidebar;
pub mod status_card;
pub mod titlebar;

use gpui::{App, ClickEvent, Context, Entity, FocusHandle, Window};

use super::root::{HubState, dispatch_later};
use crate::ui_model::app::Msg;

/// Os `FocusHandle`s da casca. Guardados pela view raiz (e nao criados a cada quadro): um handle
/// novo por render perderia o foco a cada redesenho, e sem handle o Tab nao chega ao controle.
pub struct ShellFocus {
    pub nav: Vec<FocusHandle>,
    pub card: FocusHandle,
    pub details: FocusHandle,
    pub theme: Vec<FocusHandle>,
    pub prev: FocusHandle,
    pub next: FocusHandle,
    pub eye: FocusHandle,
    pub book: Vec<FocusHandle>,
}

impl ShellFocus {
    pub fn new<T: 'static>(cx: &mut Context<T>) -> ShellFocus {
        ShellFocus {
            nav: (0..crate::ui_model::shell::NAV_ORDER.len())
                .map(|_| cx.focus_handle())
                .collect(),
            card: cx.focus_handle(),
            details: cx.focus_handle(),
            theme: (0..2).map(|_| cx.focus_handle()).collect(),
            prev: cx.focus_handle(),
            next: cx.focus_handle(),
            eye: cx.focus_handle(),
            book: (0..2).map(|_| cx.focus_handle()).collect(),
        }
    }
}

/// Clique que manda uma mensagem ao modelo. `dispatch_later`: o clique roda com a janela
/// emprestada e um efeito (fechar, foco) a emprestaria de novo.
pub fn send(
    state: &Entity<HubState>,
    msg: impl Fn() -> Msg + 'static,
) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    let state = state.clone();
    move |_, _, cx| dispatch_later(&state, msg(), cx)
}
