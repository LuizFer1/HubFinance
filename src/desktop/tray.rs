//! Icone ao lado do relogio: tooltip com o estado do hub e menu Abrir / Desligar. A parte
//! pura (texto, ids) e testada; a criacao do icone e so Windows.

use crate::hub::snapshot::Status;

/// Ids dos itens do menu; o handler os compara com `MenuId: PartialEq<&str>`.
#[cfg_attr(not(windows), allow(dead_code, reason = "so o codigo Windows usa"))]
pub const MENU_OPEN: &str = "abrir";
#[cfg_attr(not(windows), allow(dead_code, reason = "so o codigo Windows usa"))]
pub const MENU_QUIT: &str = "desligar";

/// `active_devices` = aparelhos nao revogados. Curto de proposito: o Windows corta em 127
/// caracteres e o tooltip e lido de relance.
pub fn tooltip(status: &Status, active_devices: usize) -> String {
    let state = match status {
        Status::Starting => "iniciando".to_string(),
        Status::Running => match active_devices {
            0 => "ligado · nenhum aparelho".to_string(),
            1 => "ligado · 1 aparelho".to_string(),
            n => format!("ligado · {n} aparelhos"),
        },
        Status::NoNetwork => "ligado, sem rede".to_string(),
        Status::Stopping => "desligando".to_string(),
        Status::Stopped => "desligado".to_string(),
        Status::Failed(_) => "com erro, abra para ver".to_string(),
    };
    format!("{} — {state}", crate::config::HUB_NAME)
}

#[cfg(windows)]
pub use win::{Tray, install};

#[cfg(windows)]
mod win {
    use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{
        Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    };

    use super::{MENU_OPEN, MENU_QUIT, tooltip};
    use crate::desktop::{DesktopEvent, EventSender};
    use crate::hub::snapshot::Status;

    /// 32 px: o Windows reduz para 16 na barra; o PNG e independente do `build.rs`
    /// (o `winresource` so avisa quando falha, e ai `Icon::from_resource` nao acharia nada).
    const ICON_PNG: &[u8] = include_bytes!("../../assets/icon/32x32.png");

    /// Guarda o `TrayIcon`: o icone some quando a ultima copia e descartada.
    pub struct Tray {
        icon: TrayIcon,
    }

    /// Chamar na thread principal, antes de `iced::daemon(..).run()`: a crate cria um HWND
    /// oculto na thread chamadora e as mensagens dele sao entregues pelo laco do winit, que
    /// roda nessa mesma thread. Os handlers rodam dentro desse laco e so empurram no canal.
    pub fn install(tx: EventSender) -> Result<Tray, String> {
        let image = image::load_from_memory(ICON_PNG)
            .map_err(|e| format!("icone da bandeja: {e}"))?
            .into_rgba8();
        let (width, height) = image.dimensions();
        let icon = Icon::from_rgba(image.into_raw(), width, height)
            .map_err(|e| format!("icone da bandeja: {e}"))?;

        let menu = Menu::new();
        menu.append_items(&[
            &MenuItem::with_id(MENU_OPEN, "Abrir o HubFinance", true, None),
            &PredefinedMenuItem::separator(),
            &MenuItem::with_id(MENU_QUIT, "Desligar o hub", true, None),
        ])
        .map_err(|e| format!("menu da bandeja: {e}"))?;

        let menu_tx = tx.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let message = if event.id == MENU_OPEN {
                DesktopEvent::OpenWindow
            } else if event.id == MENU_QUIT {
                DesktopEvent::Quit
            } else {
                return;
            };
            // Receptor caiu = UI saindo: nada a fazer.
            let _ = menu_tx.unbounded_send(message);
        }));
        // Clique simples esquerdo abre (convencao do Windows); `DoubleClick` chega depois de
        // dois `Click`, entao ja esta coberto. O menu fica no botao direito.
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = tx.unbounded_send(DesktopEvent::OpenWindow);
            }
        }));

        let icon = TrayIconBuilder::new()
            .with_icon(icon)
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .with_tooltip(tooltip(&Status::Starting, 0))
            .build()
            .map_err(|e| format!("bandeja: {e}"))?;
        Ok(Tray { icon })
    }

    impl Tray {
        pub fn set_tooltip(&self, text: &str) {
            if let Err(e) = self.icon.set_tooltip(Some(text)) {
                tracing::warn!("tooltip da bandeja: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::snapshot::Status;

    #[test]
    fn tooltip_por_estado() {
        assert_eq!(tooltip(&Status::Starting, 0), "HubFinance — iniciando");
        assert_eq!(
            tooltip(&Status::Running, 0),
            "HubFinance — ligado · nenhum aparelho"
        );
        assert_eq!(
            tooltip(&Status::Running, 1),
            "HubFinance — ligado · 1 aparelho"
        );
        assert_eq!(
            tooltip(&Status::Running, 2),
            "HubFinance — ligado · 2 aparelhos"
        );
        assert_eq!(
            tooltip(&Status::NoNetwork, 3),
            "HubFinance — ligado, sem rede"
        );
        assert_eq!(tooltip(&Status::Stopping, 1), "HubFinance — desligando");
        assert_eq!(tooltip(&Status::Stopped, 1), "HubFinance — desligado");
        assert_eq!(
            tooltip(&Status::Failed("porta ocupada".into()), 1),
            "HubFinance — com erro, abra para ver"
        );
    }

    #[test]
    fn tooltip_cabe_no_limite_do_windows() {
        // NOTIFYICONDATAW.szTip tem 128 WCHAR com o NUL; 64 deixa folga para qualquer contagem.
        for status in [
            Status::Starting,
            Status::Running,
            Status::NoNetwork,
            Status::Stopping,
            Status::Stopped,
            Status::Failed("x".repeat(500)),
        ] {
            assert!(tooltip(&status, 999_999).encode_utf16().count() <= 64);
        }
    }

    #[test]
    fn ids_do_menu_sao_distintos_e_ascii() {
        assert_ne!(MENU_OPEN, MENU_QUIT);
        assert!(MENU_OPEN.is_ascii() && MENU_QUIT.is_ascii());
    }
}
