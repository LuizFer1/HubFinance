//! Interface em GPUI (migracao da spec 2026-11-28). E o unico modulo com `gpui`.
//!
//! O estado da janela e o `update` moram em `ui_model` e sao os mesmos da iced: aqui so se
//! recebe snapshot, tique e eventos da bandeja, se executam os efeitos (`effects`) e se
//! desenha (`root`). As telas chegam nas tarefas seguintes.

mod assets;
mod charts;
mod components;
mod effects;
mod fonts;
mod icons;
mod motion;
mod root;
mod screens;
mod shell;
mod theme;

use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use futures::StreamExt;
use gpui::{App, AppContext as _, QuitMode};

use crate::config::UiPrefs;
use crate::desktop::{Desktop, EventReceiver};
use crate::hub::HubHandle;
use crate::hub::snapshot::Status;
use crate::ui_model::app::Msg;
use crate::ui_model::effect::Effect;

use self::root::{HubGlobal, HubState};

pub fn run(
    handle: HubHandle,
    offset: ::time::UtcOffset,
    prefs: UiPrefs,
    prefs_dir: PathBuf,
    desktop: Rc<dyn Desktop>,
    mut events: EventReceiver,
    open_window: bool,
) {
    // Explicit: sem janela o hub continua na bandeja; so `cx.quit()` encerra o processo.
    gpui_platform::application()
        .with_assets(assets::HubAssets)
        .with_quit_mode(QuitMode::Explicit)
        .run(move |cx: &mut App| {
            gpui_component::init(cx);
            fonts::register(cx);
            // O estado nasce aqui, antes de qualquer janela, e o `Global` o segura a vida
            // toda: a view raiz morre com a janela, e fechar na bandeja nao pode perder o
            // "Desfazer" pendente, o tique nem os eventos da bandeja.
            let state = cx.new(|_| {
                HubState::new(crate::ui_model::app::App::new(
                    handle.clone(),
                    offset,
                    prefs,
                    prefs_dir,
                    desktop,
                ))
            });
            cx.set_global(HubGlobal(state.clone()));
            // Antes da primeira janela: ela ja nasce no tema certo, sem piscar o padrao.
            root::sync_theme(&state, cx);

            // O nucleo que ja falhou antes do boot (porta ocupada responde em milissegundos)
            // nao geraria a transicao para `Failed`: sem janela o erro ficaria escondido.
            let failed = matches!(handle.snapshot.borrow().status, Status::Failed(_));
            if let Some(demo) = root::demo_snapshot(&handle) {
                root::dispatch(&state, Msg::Snapshot(Box::new(demo)), cx);
            }
            if open_window || failed {
                effects::run(Effect::OpenWindow, &state, cx);
            }

            // (1) snapshot do nucleo.
            let mut snapshot = handle.snapshot.clone();
            let task_state = state.clone();
            cx.spawn(async move |cx| {
                // Sender caiu = nucleo terminou.
                while snapshot.changed().await.is_ok() {
                    let next = snapshot.borrow_and_update().clone();
                    cx.update(|cx| {
                        root::dispatch(&task_state, Msg::Snapshot(Box::new(next)), cx);
                    });
                }
            })
            .detach();

            // (2) tique de 1 s: contagens regressivas, "ha 3 min", virada do dia, tema do sistema.
            let task_state = state.clone();
            cx.spawn(async move |cx| {
                loop {
                    cx.background_executor().timer(Duration::from_secs(1)).await;
                    cx.update(|cx| root::dispatch(&task_state, Msg::Tick, cx));
                }
            })
            .detach();

            // (3) eventos da bandeja e da segunda execucao.
            let task_state = state;
            cx.spawn(async move |cx| {
                while let Some(event) = events.next().await {
                    cx.update(|cx| root::dispatch(&task_state, Msg::Desktop(event), cx));
                }
            })
            .detach();
        });
}
