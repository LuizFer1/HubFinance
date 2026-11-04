//! Janela iced: so renderiza o snapshot do nucleo e envia comandos; unico modulo com iced.

mod app;
mod charts;
mod connection_view;
mod dashboard_view;
mod fonts;
mod icons;
mod remove_view;
mod reserves_view;
mod shell;
mod theme;
mod time;
mod transactions_view;
mod view;
mod widgets;

use std::path::PathBuf;
use std::rc::Rc;

use iced::{Size, window};

use crate::config::UiPrefs;
use crate::desktop::{Desktop, EventReceiver};
use crate::hub::HubHandle;

pub fn run(
    handle: HubHandle,
    offset: ::time::UtcOffset,
    prefs: UiPrefs,
    prefs_dir: PathBuf,
    desktop: Rc<dyn Desktop>,
    events: EventReceiver,
    open_window: bool,
) -> iced::Result {
    // `daemon`, nao `application`: o programa e o hub, e a janela e opcional. O daemon nao
    // abre janela sozinho (o `boot` abre, se for o caso) nem termina quando a ultima fecha;
    // so `iced::exit()` — o "Desligar o hub" — encerra o processo.
    iced::daemon(
        app::boot(
            handle,
            offset,
            prefs,
            prefs_dir,
            desktop,
            events,
            open_window,
        ),
        app::update,
        app::view,
    )
    .title("HubFinance")
    .subscription(app::subscription)
    .font(fonts::INTER_REGULAR_BYTES)
    .font(fonts::INTER_MEDIUM_BYTES)
    .font(fonts::INTER_SEMIBOLD_BYTES)
    .font(fonts::PHOSPHOR_BYTES)
    .default_font(fonts::INTER)
    .antialiasing(true)
    .theme(|app: &app::App, _window: window::Id| app.iced_theme.clone())
    .run()
}

/// Usado por `window::open` toda vez que a janela e (re)aberta.
pub(super) fn window_settings() -> window::Settings {
    window::Settings {
        size: app::INITIAL_SIZE,
        min_size: Some(Size::new(1024.0, 720.0)),
        // Barra de titulo propria (shell.rs), como o design desenha.
        decorations: false,
        // O X da barra manda `CloseRequested`; com bandeja a UI fecha so a janela, sem
        // bandeja desliga o hub com ordem (`Shutdown` -> `Stopped` ou 5 s). O iced nunca
        // fecha por conta propria.
        exit_on_close_request: false,
        icon: window_icon(),
        ..window::Settings::default()
    }
}

/// Gerado por `scripts/gen-icon.py`, o mesmo desenho dos instaladores.
const WINDOW_ICON_PNG: &[u8] = include_bytes!("../../assets/icon/64x64.png");

/// Icone que nao decodifica so custa o icone padrao do sistema; nunca impede a janela de abrir.
fn window_icon() -> Option<window::Icon> {
    match window::icon::from_file_data(WINDOW_ICON_PNG, None) {
        Ok(icon) => Some(icon),
        Err(err) => {
            tracing::warn!("icone da janela invalido: {err}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icone_da_janela_decodifica() {
        assert!(window_icon().is_some());
    }

    #[test]
    fn janela_nunca_fecha_sozinha() {
        assert!(!window_settings().exit_on_close_request);
        assert!(!window_settings().decorations);
    }
}
