//! Executa os `Effect` do `ui_model` no GPUI. Mesmo contrato do `ui/effects.rs` da iced.
//!
//! A janela e resolvida na hora da execucao (`cx.windows()`), e nao recebida: o efeito tambem
//! chega sem janela aberta (`OpenWindow` da bandeja, `SavePrefs`, `Exit`), e quem o pediu
//! pode estar dentro de um callback da propria janela.

use gpui::{
    AnyWindowHandle, App, AppContext as _, Bounds, ClipboardItem, Entity, WindowBounds,
    WindowDecorations, WindowOptions, px, size,
};

use super::root::{self, HubState, HubWindow, dispatch, dispatch_later};
use crate::ui_model::app::Msg;
use crate::ui_model::effect::Effect;
use crate::ui_model::window::WindowId;

pub fn run(effect: Effect, state: &Entity<HubState>, cx: &mut App) {
    match effect {
        Effect::OpenWindow => open_window(state, cx),
        Effect::FocusWindow => {
            with_window(cx, |window, _| window.activate_window());
            cx.activate(true);
        }
        Effect::CloseWindow => {
            let closed = state.read(cx).app.window;
            with_window(cx, |window, _| window.remove_window());
            // O estado precisa saber que a janela morreu para a bandeja reabrir uma nova.
            if let Some(id) = closed {
                dispatch_later(state, Msg::WindowClosed(id), cx);
            }
        }
        Effect::Minimize => {
            // Na barra propria o botao e nativo (`window_control_area(Min)`); isto serve ao atalho.
            with_window(cx, |window, _| window.minimize_window());
        }
        Effect::ToggleMaximize => {
            // Windows: o `zoom_window` do gpui so manda SW_MAXIMIZE (gpui-pre-windows 0.3.14,
            // window.rs:949), entao uma janela ja maximizada nao volta ao normal por aqui.
            // O botao da barra propria e nativo (`window_control_area(Max)`).
            with_window(cx, |window, _| window.zoom_window());
            // A resposta do sistema chega depois do `zoom`; o `Resized` consulta de novo.
            query_maximized(state, cx);
        }
        Effect::QueryMaximized => query_maximized(state, cx),
        Effect::StartDrag => {
            // Windows: `start_window_move` nao e sobrescrito na plataforma (vale o default vazio
            // de platform.rs:1461), logo nao faz nada. Arrastar na barra propria vem de
            // `window_control_area(Drag)`, que o sistema resolve por hit-test (HTCAPTION).
            with_window(cx, |window, _| window.start_window_move());
        }
        Effect::After(duration, msg) => {
            let state = state.clone();
            cx.spawn(async move |cx| {
                cx.background_executor().timer(duration).await;
                cx.update(|cx| dispatch(&state, msg, cx));
            })
            .detach();
        }
        Effect::SavePrefs(prefs) => {
            let dir = state.read(cx).app.prefs_dir.clone();
            // A thread grava sozinha; ninguem espera o resultado (falha so vira aviso no log).
            drop(crate::config::save_prefs_ordered(prefs, dir));
        }
        Effect::CopyToClipboard(text) => cx.write_to_clipboard(ClipboardItem::new_string(text)),
        Effect::OpenUrl(url) => {
            // `that_detached`: em alguns Linux o `xdg-open` so volta quando o navegador fecha.
            if let Err(err) = open::that_detached(url) {
                tracing::warn!("nao foi possivel abrir o navegador: {err}");
                dispatch_later(state, Msg::OpenUrlFailed, cx);
            }
        }
        Effect::OpenFolder(path) => {
            if let Err(err) = open::that_detached(&path) {
                tracing::warn!("nao foi possivel abrir a pasta {}: {err}", path.display());
            }
        }
        // Tab/Shift+Tab percorrem os controles com `track_focus` na ordem de leitura. O campo de
        // texto do `gpui-component` e o botao entram na mesma cadeia de foco; sem isto o atalho
        // do `ui_model` nao teria efeito na janela.
        Effect::FocusNext => {
            with_window(cx, |window, cx| window.focus_next(cx));
        }
        Effect::FocusPrevious => {
            with_window(cx, |window, cx| window.focus_prev(cx));
        }
        // Sem rolagem propria ainda: "voltar ao topo" fica para a casca (Tarefa 15).
        Effect::ScrollToTop => {}
        Effect::Exit => cx.quit(),
    }
}

/// Abre a janela principal, ou traz a que ja existe para a frente (nunca duas).
fn open_window(state: &Entity<HubState>, cx: &mut App) {
    if !cx.windows().is_empty() {
        with_window(cx, |window, _| window.activate_window());
        cx.activate(true);
        return;
    }
    let bounds = Bounds::centered(None, size(px(1440.0), px(900.0)), cx);
    let hub = cx.global::<root::HubGlobal>().0.clone();
    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(960.0), px(640.0))),
            // Barra de titulo propria (Tarefa 14): o sistema nao desenha a dele.
            titlebar: None,
            window_decorations: Some(WindowDecorations::Client),
            ..Default::default()
        },
        move |window, cx| {
            let closing = hub.clone();
            // Quem fecha e o efeito `CloseWindow`: com bandeja so a janela, sem ela o hub.
            window.on_window_should_close(cx, move |_window, cx| {
                dispatch_later(&closing, Msg::CloseRequested, cx);
                false
            });
            let hub = hub.clone();
            cx.new(|cx| HubWindow::new(hub, window, cx))
        },
    );
    match opened {
        Ok(handle) => {
            let id = WindowId(AnyWindowHandle::from(handle).window_id().as_u64());
            // O id vai ao estado na hora: os botoes da barra nao esperam o `WindowOpened`.
            state.update(cx, |hub, cx| {
                hub.app.window = Some(id);
                cx.notify();
            });
            let first = handle
                .update(cx, |_, window, _| root::viewport(window))
                .ok();
            dispatch_later(state, Msg::WindowOpened(id), cx);
            if let Some(first) = first {
                dispatch_later(state, Msg::Resized(first), cx);
            }
            cx.activate(true);
        }
        Err(err) => tracing::error!("janela gpui nao abriu: {err}"),
    }
}

fn query_maximized(state: &Entity<HubState>, cx: &mut App) {
    let maximized = with_window(cx, |window, _| window.is_maximized());
    if let Some(maximized) = maximized {
        dispatch_later(state, Msg::Maximized(maximized), cx);
    }
}

/// Acao sobre a janela aberta; sem janela, nada.
fn with_window<R>(
    cx: &mut App,
    action: impl FnOnce(&mut gpui::Window, &mut App) -> R,
) -> Option<R> {
    let handle = cx.windows().into_iter().next()?;
    handle.update(cx, |_, window, cx| action(window, cx)).ok()
}
