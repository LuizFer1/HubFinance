//! Interface em GPUI (migracao da spec 2026-11-28). E o unico modulo com `gpui`.
//!
//! Tarefa 1: so prova que GPUI, iced e bandeja convivem no mesmo binario e abre uma janela
//! vazia. As telas chegam nas tarefas seguintes.

use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use futures::StreamExt;
use gpui::{
    App, Bounds, Context, QuitMode, Render, Window, WindowBounds, WindowOptions, div, prelude::*,
    px, rgb, size,
};

use crate::config::UiPrefs;
use crate::desktop::{Desktop, DesktopEvent, EventReceiver};
use crate::hub::snapshot::Status;
use crate::hub::{Command, HubHandle};

/// `#131313`, o fundo escuro do design.
const BG: u32 = 0x131313;

struct Empty;

impl Render for Empty {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgb(BG))
            .text_color(rgb(0xF5F5F5))
            .flex()
            .items_center()
            .justify_center()
            .child("HubFinance")
    }
}

pub fn run(
    handle: HubHandle,
    _offset: ::time::UtcOffset,
    _prefs: UiPrefs,
    _prefs_dir: PathBuf,
    desktop: Rc<dyn Desktop>,
    mut events: EventReceiver,
    open_window: bool,
) {
    // Explicit: sem janela o hub continua na bandeja; so `cx.quit()` encerra o processo.
    gpui_platform::application()
        .with_quit_mode(QuitMode::Explicit)
        .run(move |cx: &mut App| {
            gpui_component::init(cx);
            if open_window {
                open_main_window(&handle, desktop.as_ref(), cx);
            }
            // Provisorio da Tarefa 1: so reabre a janela vazia e desliga. A Tarefa 7 troca
            // isto pelo `ui_model`, que leva os eventos para as telas (Aparelhos, endereco).
            cx.spawn(async move |cx| {
                while let Some(event) = events.next().await {
                    let handle = handle.clone();
                    let desktop = desktop.clone();
                    cx.update(|cx| match event {
                        DesktopEvent::Quit => shutdown(handle, cx),
                        DesktopEvent::OpenWindow | DesktopEvent::ConnectPhone => {
                            open_main_window(&handle, desktop.as_ref(), cx);
                        }
                        DesktopEvent::CopyAddress => {}
                    });
                }
            })
            .detach();
        });
}

/// Abre a janela principal, ou traz a que ja existe para a frente (nunca duas).
fn open_main_window(handle: &HubHandle, desktop: &dyn Desktop, cx: &mut App) {
    if let Some(existing) = cx.windows().into_iter().next() {
        let _ = existing.update(cx, |_, window, _| window.activate_window());
        cx.activate(true);
        return;
    }
    let bounds = Bounds::centered(None, size(px(1440.0), px(900.0)), cx);
    let has_tray = desktop.has_tray();
    let handle = handle.clone();
    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(960.0), px(640.0))),
            ..Default::default()
        },
        |window, cx| {
            window.on_window_should_close(cx, move |_window, cx| {
                // Com bandeja so fecha a janela; sem ela fechar desliga o hub, e o
                // processo sai quando o nucleo avisa `Stopped` (ou em 5 s).
                if !has_tray {
                    shutdown(handle.clone(), cx);
                }
                has_tray
            });
            cx.new(|_| Empty)
        },
    );
    match opened {
        Ok(_) => cx.activate(true),
        Err(err) => tracing::error!("janela gpui nao abriu: {err}"),
    }
}

/// Desligamento com ordem: `Shutdown`, depois `cx.quit()` quando o snapshot vier `Stopped`.
fn shutdown(handle: HubHandle, cx: &mut App) {
    if handle.commands.send(Command::Shutdown).is_err() {
        cx.quit();
        return;
    }
    let mut snapshot = handle.snapshot.clone();
    cx.spawn(async move |cx| {
        let wait = async {
            while snapshot.borrow().status != Status::Stopped {
                if snapshot.changed().await.is_err() {
                    break;
                }
            }
        };
        let timeout = cx.background_executor().timer(Duration::from_secs(5));
        futures::future::select(Box::pin(wait), Box::pin(timeout)).await;
        cx.update(|cx| cx.quit());
    })
    .detach();
}
