//! Adaptador `Effect` -> `Task` do iced. Tudo o que o `handle` pede ao mundo de fora passa
//! por aqui, para a mesma descricao poder ser executada por outro toolkit sem mexer na
//! transicao de estado.

use std::path::PathBuf;
use std::time::Duration;

use iced::{Task, window};

use super::app::{Msg, State};
use super::iced_side::model_id;
use super::shell::CONTENT_SCROLL;
use crate::config::UiPrefs;
use crate::ui_model::effect::Effect;
use crate::ui_model::window::ResizeEdge;

pub(super) fn task(effect: Effect, app: &mut State) -> Task<Msg> {
    match effect {
        Effect::OpenWindow => {
            let (id, task) = window::open(super::window_settings());
            // O id vai ao estado na hora: os botoes da barra de titulo nao esperam o `WindowOpened`.
            app.window = Some(app.iced.register(id));
            task.map(|id| Msg::WindowOpened(model_id(id)))
        }
        Effect::FocusWindow => match iced_window(app) {
            // Clique na bandeja e segunda execucao dao ao processo o direito de foco; se o
            // Windows recusar, o botao da barra de tarefas pisca.
            Some(id) => Task::batch([window::minimize(id, false), window::gain_focus(id)]),
            None => Task::none(),
        },
        Effect::CloseWindow => with_window(app, window::close),
        Effect::Minimize => with_window(app, |id| window::minimize(id, true)),
        Effect::ToggleMaximize => with_window(app, |id| {
            window::toggle_maximize(id).chain(window::is_maximized(id).map(Msg::Maximized))
        }),
        Effect::QueryMaximized => {
            with_window(app, |id| window::is_maximized(id).map(Msg::Maximized))
        }
        Effect::StartDrag => with_window(app, window::drag),
        Effect::After(duration, message) => {
            Task::perform(delay(duration), move |_| message.clone())
        }
        Effect::SavePrefs(prefs) => {
            Task::perform(save_prefs(prefs, app.prefs_dir.clone()), |_| Msg::Noop)
        }
        Effect::CopyToClipboard(text) => iced::clipboard::write(text),
        Effect::OpenUrl(url) => {
            if let Err(err) = open::that_detached(url) {
                // `that_detached`, e nao `that`: em alguns Linux o `xdg-open` so volta quando o
                // navegador fecha, e a thread da janela nao pode esperar.
                tracing::warn!("nao foi possivel abrir o navegador: {err}");
                return Task::done(Msg::OpenUrlFailed);
            }
            Task::none()
        }
        Effect::OpenFolder(path) => {
            if let Err(err) = open::that_detached(&path) {
                tracing::warn!("nao foi possivel abrir a pasta {}: {err}", path.display());
            }
            Task::none()
        }
        Effect::FocusNext => iced::widget::operation::focus_next(),
        Effect::FocusPrevious => iced::widget::operation::focus_previous(),
        Effect::ScrollToTop => iced::widget::operation::snap_to(
            CONTENT_SCROLL,
            iced::widget::operation::RelativeOffset::START,
        ),
        // `iced::exit()` so marca o laco do winit para sair; no Windows o laco confere a marca
        // depois de acordar, e sem janela nada o acorda (nem redesenho, nem evento de janela):
        // o "Desligar o hub" com a janela fechada so terminava quando o `ForceExit` de 5 s
        // chegasse — e, se o nucleo travasse, nem ai. Uma mensagem qualquer logo depois acorda
        // o laco, que ve a marca e sai.
        Effect::Exit => Task::batch([
            iced::exit(),
            Task::perform(delay(Duration::from_millis(50)), |_| Msg::Noop),
        ]),
    }
}

/// Arrastar a borda para redimensionar e coisa so do iced (o GPUI usa o teste de posicao do
/// sistema), por isso nao e um `Effect`; some com a iced.
pub(super) fn resize(app: &mut State, edge: ResizeEdge) -> Task<Msg> {
    let direction = match edge {
        ResizeEdge::North => window::Direction::North,
        ResizeEdge::South => window::Direction::South,
        ResizeEdge::East => window::Direction::East,
        ResizeEdge::West => window::Direction::West,
        ResizeEdge::NorthEast => window::Direction::NorthEast,
        ResizeEdge::NorthWest => window::Direction::NorthWest,
        ResizeEdge::SouthEast => window::Direction::SouthEast,
        ResizeEdge::SouthWest => window::Direction::SouthWest,
    };
    with_window(app, |id| window::drag_resize(id, direction))
}

/// O id do iced da janela atual, pelo mapa do adaptador.
fn iced_window(app: &State) -> Option<window::Id> {
    app.window.and_then(|id| app.iced.iced_id(id))
}

/// Grava o `ui.json` fora da thread da UI; a ordem das gravacoes e garantida em
/// `config::save_prefs_ordered`, que a interface GPUI usa igual.
pub(super) fn save_prefs(prefs: UiPrefs, dir: PathBuf) -> futures::channel::oneshot::Receiver<()> {
    crate::config::save_prefs_ordered(prefs, dir)
}

/// Timer sem runtime: o executor da UI nao e tokio, entao o timer e uma thread comum + oneshot
/// do `futures`, que e um Future em qualquer executor.
fn delay(duration: Duration) -> futures::channel::oneshot::Receiver<()> {
    let (tx, rx) = futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        std::thread::sleep(duration);
        let _ = tx.send(());
    });
    rx
}

/// Acao sobre a janela; sem id ainda, nada (com um aviso so, para nao inundar o log).
fn with_window(app: &mut State, action: impl FnOnce(window::Id) -> Task<Msg>) -> Task<Msg> {
    match iced_window(app) {
        Some(id) => action(id),
        None => {
            if !app.warned_no_window {
                app.warned_no_window = true;
                tracing::warn!("janela sem id: botoes da barra de titulo sem efeito");
            }
            Task::none()
        }
    }
}
