//! Icone ao lado do relogio: tooltip e desenho conforme o estado do hub, e o menu Abrir /
//! Conectar um celular / Copiar endereco / Desligar. A parte pura (texto, icone por estado,
//! ids) e testada; a criacao do icone e so Windows.

use crate::hub::snapshot::Status;

/// Ids dos itens do menu; o handler os compara com `MenuId: PartialEq<&str>`.
#[cfg_attr(not(windows), allow(dead_code, reason = "so o codigo Windows usa"))]
pub const MENU_OPEN: &str = "abrir";
#[cfg_attr(not(windows), allow(dead_code, reason = "so o codigo Windows usa"))]
pub const MENU_QUIT: &str = "desligar";
#[cfg_attr(not(windows), allow(dead_code, reason = "so o codigo Windows usa"))]
pub const MENU_CONNECT: &str = "conectar";
#[cfg_attr(not(windows), allow(dead_code, reason = "so o codigo Windows usa"))]
pub const MENU_COPY_ADDRESS: &str = "copiar-endereco";

/// Desenho do icone da bandeja. Quatro estados, cada um com desenho para barra clara e para escura
/// (ver `icon_file`): no tamanho da barra (16 px) so um ponto de cor no canto se le, e o
/// tooltip diz o resto.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TrayIcon {
    /// O icone do programa, sem ponto.
    #[default]
    Normal,
    /// Ponto laranja: o hub esta no ar mas algo pede atencao (sem rede local).
    Attention,
    /// Ponto vermelho: o hub nao subiu; a janela mostra o erro.
    Error,
    /// Servidor parado: o desenho some a 45%, sem ponto.
    #[allow(dead_code)] // usado na Tarefa 32
    Off,
}

/// Nome do PNG embutido por estado **e** pelo tema da barra de tarefas. A barra e do Windows e
/// pode divergir do tema do app: um desenho escuro sobre barra escura some, por isso os dois.
/// `None` (o sistema nao diz) vale como barra escura, o padrao do Windows.
#[cfg_attr(not(windows), allow(dead_code, reason = "so o codigo Windows usa"))]
pub fn icon_file(kind: TrayIcon, taskbar_dark: Option<bool>) -> &'static str {
    let dark = taskbar_dark.unwrap_or(true);
    match (kind, dark) {
        (TrayIcon::Normal, true) => "tray-on-dark.png",
        (TrayIcon::Normal, false) => "tray-on-light.png",
        (TrayIcon::Attention, true) => "tray-offline-dark.png",
        (TrayIcon::Attention, false) => "tray-offline-light.png",
        (TrayIcon::Error, true) => "tray-error-dark.png",
        (TrayIcon::Error, false) => "tray-error-light.png",
        (TrayIcon::Off, true) => "tray-off-dark.png",
        (TrayIcon::Off, false) => "tray-off-light.png",
    }
}

/// So o estado do nucleo decide. Aparelho removido com dados a decidir fica de fora: o "Depois"
/// do aviso vale so para a sessao, e o ponto voltaria a cada boot sem a pessoa ter como
/// tira-lo sem decidir; o aviso na janela ja cobre esse caso.
pub fn icon_for(status: &Status) -> TrayIcon {
    match status {
        Status::NoNetwork => TrayIcon::Attention,
        Status::Failed(_) => TrayIcon::Error,
        Status::Starting | Status::Running | Status::Stopping | Status::Stopped => TrayIcon::Normal,
    }
}

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
pub use win::{Tray, copy_text, install};

#[cfg(windows)]
mod win {
    use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{
        Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    };

    use super::{MENU_CONNECT, MENU_COPY_ADDRESS, MENU_OPEN, MENU_QUIT, TrayIcon as Kind, tooltip};
    use crate::desktop::{DesktopEvent, EventSender};
    use crate::hub::snapshot::Status;

    /// 32 px: o Windows reduz para 16 na barra; os PNG sao independentes do `build.rs`
    /// (o `winresource` so avisa quando falha, e ai `Icon::from_resource` nao acharia nada).
    /// Gerados por `scripts/gen-icon.py` e commitados; o teste `pngs_cobrem_icon_file` garante que
    /// todo nome devolvido por `icon_file` esta aqui.
    pub(super) const PNGS: [(&str, &[u8]); 8] = [
        (
            "tray-on-dark.png",
            include_bytes!("../../assets/icon/tray-on-dark.png"),
        ),
        (
            "tray-on-light.png",
            include_bytes!("../../assets/icon/tray-on-light.png"),
        ),
        (
            "tray-offline-dark.png",
            include_bytes!("../../assets/icon/tray-offline-dark.png"),
        ),
        (
            "tray-offline-light.png",
            include_bytes!("../../assets/icon/tray-offline-light.png"),
        ),
        (
            "tray-error-dark.png",
            include_bytes!("../../assets/icon/tray-error-dark.png"),
        ),
        (
            "tray-error-light.png",
            include_bytes!("../../assets/icon/tray-error-light.png"),
        ),
        (
            "tray-off-dark.png",
            include_bytes!("../../assets/icon/tray-off-dark.png"),
        ),
        (
            "tray-off-light.png",
            include_bytes!("../../assets/icon/tray-off-light.png"),
        ),
    ];

    /// Guarda o `TrayIcon`: o icone some quando a ultima copia e descartada. As oito imagens
    /// (4 estados x 2 temas) sao decodificadas uma vez, no `install`: trocar de estado nao le PNG de novo.
    pub struct Tray {
        icon: TrayIcon,
        icons: Vec<(&'static str, Icon)>,
    }

    fn decode(png: &[u8]) -> Result<Icon, String> {
        let image = image::load_from_memory(png)
            .map_err(|e| format!("icone da bandeja: {e}"))?
            .into_rgba8();
        let (width, height) = image.dimensions();
        Icon::from_rgba(image.into_raw(), width, height)
            .map_err(|e| format!("icone da bandeja: {e}"))
    }

    /// Chamar na thread principal, antes de `iced::daemon(..).run()`: a crate cria um HWND
    /// oculto na thread chamadora e as mensagens dele sao entregues pelo laco do winit, que
    /// roda nessa mesma thread. Os handlers rodam dentro desse laco e so empurram no canal.
    pub fn install(tx: EventSender, taskbar_dark: Option<bool>) -> Result<Tray, String> {
        let icons = PNGS
            .iter()
            .map(|(name, bytes)| decode(bytes).map(|icon| (*name, icon)))
            .collect::<Result<Vec<_>, _>>()?;
        // Mesmo criterio do `set_icon`: o primeiro desenho ja sai no tema da barra, pois so ha
        // nova troca quando o estado muda.
        let first = super::icon_file(Kind::Normal, taskbar_dark);
        let normal = icons
            .iter()
            .find(|(name, _)| *name == first)
            .map(|(_, icon)| icon.clone())
            .ok_or("icone da bandeja ausente")?;

        let menu = Menu::new();
        menu.append_items(&[
            &MenuItem::with_id(MENU_OPEN, "Abrir o HubFinance", true, None),
            &MenuItem::with_id(MENU_CONNECT, "Conectar um celular", true, None),
            &MenuItem::with_id(MENU_COPY_ADDRESS, "Copiar endereço", true, None),
            &PredefinedMenuItem::separator(),
            &MenuItem::with_id(MENU_QUIT, "Desligar o hub", true, None),
        ])
        .map_err(|e| format!("menu da bandeja: {e}"))?;

        let menu_tx = tx.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let message = if event.id == MENU_OPEN {
                DesktopEvent::OpenWindow
            } else if event.id == MENU_CONNECT {
                DesktopEvent::ConnectPhone
            } else if event.id == MENU_COPY_ADDRESS {
                DesktopEvent::CopyAddress
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
            .with_icon(normal.clone())
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .with_tooltip(tooltip(&Status::Starting, 0))
            .build()
            .map_err(|e| format!("bandeja: {e}"))?;
        Ok(Tray { icon, icons })
    }

    impl Tray {
        /// `false` = o sistema recusou; a UI tenta de novo no proximo snapshot ou tique.
        pub fn set_tooltip(&self, text: &str) -> bool {
            match self.icon.set_tooltip(Some(text)) {
                Ok(()) => true,
                Err(e) => {
                    tracing::warn!("tooltip da bandeja: {e}");
                    false
                }
            }
        }

        /// `false` = o sistema recusou (Explorer reiniciando, por exemplo).
        pub fn set_icon(&self, kind: Kind, taskbar_dark: Option<bool>) -> bool {
            // Tema recebido da UI, que o le a cada tique: a barra pode mudar de tema com o hub
            // ligado, e reler aqui deixaria o que a UI compara diferente do que desenhamos.
            let name = super::icon_file(kind, taskbar_dark);
            let Some((_, icon)) = self.icons.iter().find(|(n, _)| *n == name) else {
                return false;
            };
            match self.icon.set_icon(Some(icon.clone())) {
                Ok(()) => true,
                Err(e) => {
                    tracing::warn!("icone da bandeja: {e}");
                    false
                }
            }
        }
    }

    /// Texto na area de transferencia pelo Win32. Nao pelo iced: o `Clipboard` do iced_winit
    /// nasce desconectado e volta a isso quando a janela fecha, e ai a escrita some calada; o
    /// "Copiar endereco" da bandeja e justamente o caso sem janela.
    ///
    /// O bloco e alocado e preenchido **antes** de abrir a area: uma falha de memoria nao pode
    /// chegar ao `EmptyClipboard` e apagar o que a pessoa tinha copiado.
    ///
    /// HWND nulo: a documentacao de `OpenClipboard` diz que, aberta sem janela, o
    /// `EmptyClipboard` deixa o dono nulo e "isso faz o `SetClipboardData` falhar". Na pratica o
    /// Windows 10/11 aceita (e o que a `clipboard-win` faz por padrao), e o hub nao tem um HWND
    /// proprio a mao: o da bandeja e interno da `tray-icon`. **Conferir na validacao** (copiar
    /// com a janela fechada e colar); se falhar, o erro vai para o log e o caminho e criar uma
    /// janela so de mensagens para ser a dona.
    pub fn copy_text(text: &str) -> Result<(), String> {
        use windows_sys::Win32::Foundation::GlobalFree;
        use windows_sys::Win32::System::DataExchange::{
            CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
        };
        use windows_sys::Win32::System::Memory::{
            GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock,
        };
        use windows_sys::Win32::System::Ole::CF_UNICODETEXT;

        /// Outro programa pode estar com a area aberta por um instante (gerenciadores de
        /// historico, por exemplo): algumas tentativas curtas antes de desistir.
        const OPEN_TRIES: u32 = 5;
        const OPEN_WAIT: std::time::Duration = std::time::Duration::from_millis(10);

        let wide = crate::desktop::wide(text);
        let bytes = std::mem::size_of_val(wide.as_slice());
        // SAFETY: chamadas Win32 na ordem documentada. O bloco tem o tamanho exato do texto com
        // NUL e e copiado enquanto travado. Ate o `SetClipboardData` aceitar ele e nosso e e
        // liberado em todo caminho de erro; depois passa a ser do sistema e nao se libera. A
        // area de transferencia e sempre fechada depois de aberta.
        unsafe {
            let block = GlobalAlloc(GMEM_MOVEABLE, bytes);
            if block.is_null() {
                return Err("sem memoria para copiar".into());
            }
            let target = GlobalLock(block).cast::<u16>();
            if target.is_null() {
                GlobalFree(block);
                return Err("sem memoria para copiar".into());
            }
            std::ptr::copy_nonoverlapping(wide.as_ptr(), target, wide.len());
            GlobalUnlock(block);

            let mut opened = false;
            for attempt in 0..OPEN_TRIES {
                if attempt > 0 {
                    std::thread::sleep(OPEN_WAIT);
                }
                if OpenClipboard(std::ptr::null_mut()) != 0 {
                    opened = true;
                    break;
                }
            }
            if !opened {
                GlobalFree(block);
                return Err("area de transferencia ocupada".into());
            }
            let result = if EmptyClipboard() == 0 {
                GlobalFree(block);
                Err("nao deu para limpar a area de transferencia".to_string())
            } else if SetClipboardData(u32::from(CF_UNICODETEXT), block).is_null() {
                GlobalFree(block);
                Err("o sistema recusou a copia".to_string())
            } else {
                Ok(())
            };
            CloseClipboard();
            result
        }
    }
}

#[cfg(all(test, windows))]
mod win_tests {
    use super::{TrayIcon, icon_file, win::PNGS};

    /// Nome fora de `PNGS` faria `install` falhar ou `set_icon` devolver `false` para sempre.
    #[test]
    fn pngs_cobrem_icon_file() {
        for kind in [
            TrayIcon::Normal,
            TrayIcon::Attention,
            TrayIcon::Error,
            TrayIcon::Off,
        ] {
            for dark in [None, Some(true), Some(false)] {
                let name = icon_file(kind, dark);
                assert!(PNGS.iter().any(|(n, _)| *n == name), "{name}");
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
        let ids = [MENU_OPEN, MENU_CONNECT, MENU_COPY_ADDRESS, MENU_QUIT];
        for (i, a) in ids.iter().enumerate() {
            assert!(a.is_ascii(), "{a}");
            for b in &ids[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn icone_por_estado() {
        assert_eq!(icon_for(&Status::Starting), TrayIcon::Normal);
        assert_eq!(icon_for(&Status::Running), TrayIcon::Normal);
        assert_eq!(icon_for(&Status::NoNetwork), TrayIcon::Attention);
        assert_eq!(icon_for(&Status::Stopping), TrayIcon::Normal);
        assert_eq!(icon_for(&Status::Stopped), TrayIcon::Normal);
        assert_eq!(
            icon_for(&Status::Failed("porta ocupada".into())),
            TrayIcon::Error
        );
    }

    /// Todo estado x tema da barra tem PNG embutido, 32 px, e o nome vem de `icon_file`.
    #[test]
    fn icone_por_estado_e_tema_da_barra() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icon");
        let mut seen = std::collections::HashSet::new();
        for kind in [
            TrayIcon::Normal,
            TrayIcon::Attention,
            TrayIcon::Error,
            TrayIcon::Off,
        ] {
            for dark in [Some(true), Some(false)] {
                let name = icon_file(kind, dark);
                assert!(seen.insert(name), "{name} repetido");
                let bytes = std::fs::read(dir.join(name)).unwrap();
                assert!(
                    bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]),
                    "{name}"
                );
                assert_eq!(&bytes[16..24], &[0, 0, 0, 32, 0, 0, 0, 32], "{name}");
            }
        }
        assert_eq!(seen.len(), 8);
    }

    #[test]
    fn barra_sem_resposta_vale_escura() {
        for kind in [
            TrayIcon::Normal,
            TrayIcon::Error,
            TrayIcon::Attention,
            TrayIcon::Off,
        ] {
            assert_eq!(icon_file(kind, None), icon_file(kind, Some(true)));
        }
        assert_eq!(
            icon_file(TrayIcon::Normal, Some(false)),
            "tray-on-light.png"
        );
    }
}
