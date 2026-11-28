//! Integracao com o sistema: bandeja, instancia unica e "iniciar com o Windows". Unico modulo
//! que conhece `tray-icon`, `image` e `windows-sys`; `ui/` continua o unico com iced e recebe
//! daqui so mensagens simples por canal. O que decide algo e funcao pura e testada; o que
//! toca o sistema fica atras de `cfg(windows)` e do trait `Desktop`, para teste nenhum criar
//! bandeja, mutex nem chave de registro.

pub mod appearance;
pub mod args;
pub mod autostart;
pub mod instance;
pub mod secret;
pub mod tray;

use std::rc::Rc;

/// O que a bandeja, o menu ou uma segunda execucao pedem a janela.
/// Fora do Windows ninguem manda eventos (sem bandeja nem segunda instancia que avise).
#[cfg_attr(not(windows), allow(dead_code, reason = "so o codigo Windows usa"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DesktopEvent {
    /// Clique no icone, "Abrir o HubFinance" no menu, ou o programa aberto de novo.
    OpenWindow,
    /// "Conectar um celular" no menu: abre a janela, se fechada, e o fluxo de Aparelhos.
    ConnectPhone,
    /// "Copiar endereco" no menu: a UI resolve o endereco e copia por `Desktop::copy_text`.
    CopyAddress,
    /// "Desligar o hub" no menu da bandeja.
    Quit,
}

/// `futures::channel`, e nao tokio: o lado que le e uma `Task` do iced, cujo executor nao e
/// tokio; o lado que escreve e um callback Win32 na thread principal, que nunca pode bloquear.
pub type EventSender = futures::channel::mpsc::UnboundedSender<DesktopEvent>;
pub type EventReceiver = futures::channel::mpsc::UnboundedReceiver<DesktopEvent>;

pub fn channel() -> (EventSender, EventReceiver) {
    futures::channel::mpsc::unbounded()
}

/// Porta da UI para o sistema. `Rc`, nao `Arc`: o `TrayIcon` nao e `Send` e vive na thread
/// principal, a mesma do `update` do iced.
pub trait Desktop {
    /// Com bandeja, fechar a janela nao desliga o hub (ha como voltar). Sem bandeja, fechar
    /// desliga, como na 1.0.0.
    fn has_tray(&self) -> bool;
    /// `false` = o sistema recusou e a UI tenta de novo depois (ela so guarda o que foi aceito).
    fn set_tooltip(&self, text: &str) -> bool;
    /// Troca o desenho do icone. A UI so chama quando o estado muda, ou quando a ultima troca
    /// falhou. `false` = recusado.
    fn set_icon(&self, icon: tray::TrayIcon) -> bool;
    /// Copia para a area de transferencia sem depender da janela (ver `tray::copy_text`).
    fn copy_text(&self, text: &str) -> Result<(), String>;
    /// `None` = sistema sem a opcao (a linha nao aparece no card).
    fn autostart(&self) -> Option<bool>;
    fn set_autostart(&self, enabled: bool) -> Result<(), String>;
    /// Tema do sistema agora (`true` = escuro), para o tema "Sistema"; `None` = nao se sabe.
    /// Leitura, sem efeito: vale com ou sem bandeja, por isso o padrao serve aos dois.
    fn system_dark(&self) -> Option<bool> {
        appearance::system_dark()
    }
    /// Tema da barra de tarefas (`true` = escura). O icone da bandeja depende dele, e ele muda
    /// com o hub ligado: a UI confere a cada tique e reaplica o icone quando muda.
    fn taskbar_dark(&self) -> Option<bool> {
        appearance::taskbar_dark()
    }
}

/// Linux, macOS, e Windows quando a bandeja nao subiu.
pub struct Headless;

impl Desktop for Headless {
    fn has_tray(&self) -> bool {
        false
    }
    /// Sem bandeja nao ha o que mudar: conta como feito, para a UI nao insistir.
    fn set_tooltip(&self, _text: &str) -> bool {
        true
    }
    fn set_icon(&self, _icon: tray::TrayIcon) -> bool {
        true
    }
    /// Sem bandeja nao ha menu que peca a copia; a janela copia pelo iced.
    fn copy_text(&self, _text: &str) -> Result<(), String> {
        Err("sem suporte neste sistema".into())
    }
    fn autostart(&self) -> Option<bool> {
        None
    }
    fn set_autostart(&self, _enabled: bool) -> Result<(), String> {
        Err("sem suporte neste sistema".into())
    }
}

/// Monta a integracao do sistema atual. No Windows cria a bandeja aqui, na thread principal;
/// se ela nao subir, o hub segue sem bandeja (fechar a janela desliga, como na 1.0.0) e o
/// motivo vai para o log.
#[cfg(windows)]
pub fn install(events: EventSender) -> Rc<dyn Desktop> {
    match tray::install(events) {
        Ok(tray) => Rc::new(Windows { tray }),
        Err(e) => {
            tracing::warn!("sem bandeja: {e}");
            Rc::new(Headless)
        }
    }
}

#[cfg(not(windows))]
pub fn install(_events: EventSender) -> Rc<dyn Desktop> {
    Rc::new(Headless)
}

/// Bandeja que nao subiu cai em `Headless`, que tambem esconde "Iniciar com o Windows": sem
/// bandeja, iniciar com o sistema abriria a janela no login, e a opcao perderia o sentido.
#[cfg(windows)]
struct Windows {
    tray: tray::Tray,
}

#[cfg(windows)]
impl Desktop for Windows {
    fn has_tray(&self) -> bool {
        true
    }
    fn set_tooltip(&self, text: &str) -> bool {
        self.tray.set_tooltip(text)
    }
    fn set_icon(&self, icon: tray::TrayIcon) -> bool {
        self.tray.set_icon(icon)
    }
    fn copy_text(&self, text: &str) -> Result<(), String> {
        tray::copy_text(text)
    }
    fn autostart(&self) -> Option<bool> {
        autostart::is_enabled()
    }
    fn set_autostart(&self, enabled: bool) -> Result<(), String> {
        autostart::set_enabled(enabled)
    }
}

/// UTF-16 com NUL final, como toda API `W` do Win32 pede.
#[cfg(windows)]
pub(crate) fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
pub(crate) struct Fake {
    pub tray: bool,
    pub tooltip: std::cell::RefCell<String>,
    /// Cada troca de icone pedida, na ordem: o teste confere que a UI nao repete.
    pub icons: std::cell::RefCell<Vec<tray::TrayIcon>>,
    /// O que foi copiado pela bandeja, na ordem.
    pub copied: std::cell::RefCell<Vec<String>>,
    /// A bandeja recusa tooltip e icone (o pedido fica registrado, mas a resposta e `false`).
    pub fail_tray: std::cell::Cell<bool>,
    pub autostart: std::cell::Cell<Option<bool>>,
    pub fail_autostart: std::cell::Cell<bool>,
    /// O tema do sistema que o teste escolhe; nunca le o registro da maquina.
    pub system_dark: std::cell::Cell<Option<bool>>,
    /// Tema da barra que o teste escolhe.
    pub taskbar_dark: std::cell::Cell<Option<bool>>,
}

#[cfg(test)]
impl Fake {
    pub fn with_tray() -> Fake {
        Fake {
            tray: true,
            tooltip: Default::default(),
            icons: Default::default(),
            copied: Default::default(),
            fail_tray: Default::default(),
            autostart: std::cell::Cell::new(Some(false)),
            fail_autostart: Default::default(),
            system_dark: Default::default(),
            taskbar_dark: Default::default(),
        }
    }
}

#[cfg(test)]
impl Desktop for Fake {
    fn has_tray(&self) -> bool {
        self.tray
    }
    fn set_tooltip(&self, text: &str) -> bool {
        *self.tooltip.borrow_mut() = text.to_string();
        !self.fail_tray.get()
    }
    fn set_icon(&self, icon: tray::TrayIcon) -> bool {
        self.icons.borrow_mut().push(icon);
        !self.fail_tray.get()
    }
    fn copy_text(&self, text: &str) -> Result<(), String> {
        self.copied.borrow_mut().push(text.to_string());
        Ok(())
    }
    fn autostart(&self) -> Option<bool> {
        self.autostart.get()
    }
    fn set_autostart(&self, enabled: bool) -> Result<(), String> {
        if self.fail_autostart.get() {
            return Err("falhou de proposito".into());
        }
        self.autostart.set(Some(enabled));
        Ok(())
    }
    fn system_dark(&self) -> Option<bool> {
        self.system_dark.get()
    }
    fn taskbar_dark(&self) -> Option<bool> {
        self.taskbar_dark.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evento_atravessa_threads() {
        fn assert_send<T: Send>() {}
        assert_send::<DesktopEvent>();
        assert_send::<EventSender>();
    }

    #[test]
    fn headless_nao_tem_bandeja_nem_autostart() {
        let d = Headless;
        assert!(!d.has_tray());
        assert_eq!(d.autostart(), None);
        // Em teste o `Headless` (o de `app_with`) nunca le o tema do registro.
        assert_eq!(d.system_dark(), None);
        assert!(d.set_autostart(true).is_err());
        assert!(d.set_tooltip("ignorado"));
        assert!(d.set_icon(tray::TrayIcon::Error));
        assert!(d.copy_text("x").is_err());
    }

    #[test]
    fn fake_guarda_tooltip_e_autostart() {
        let d = Fake::with_tray();
        assert!(d.has_tray());
        d.set_tooltip("x");
        assert_eq!(d.tooltip.borrow().as_str(), "x");
        d.autostart.set(Some(false));
        assert_eq!(d.autostart(), Some(false));
        d.set_autostart(true).unwrap();
        assert_eq!(d.autostart(), Some(true));
        d.fail_autostart.set(true);
        assert!(d.set_autostart(false).is_err());
        assert_eq!(d.autostart(), Some(true), "falha nao muda o estado");
    }

    #[test]
    fn fake_guarda_icones_e_copias_na_ordem() {
        let d = Fake::with_tray();
        assert!(d.set_icon(tray::TrayIcon::Attention));
        d.fail_tray.set(true);
        assert!(!d.set_icon(tray::TrayIcon::Normal));
        assert!(!d.set_tooltip("y"));
        assert_eq!(
            *d.icons.borrow(),
            [tray::TrayIcon::Attention, tray::TrayIcon::Normal]
        );
        d.copy_text("https://192.168.0.12:7777").unwrap();
        assert_eq!(*d.copied.borrow(), ["https://192.168.0.12:7777"]);
    }

    #[test]
    fn canal_entrega_na_ordem() {
        let (tx, mut rx) = channel();
        let sent = [
            DesktopEvent::OpenWindow,
            DesktopEvent::ConnectPhone,
            DesktopEvent::CopyAddress,
            DesktopEvent::Quit,
        ];
        for e in sent {
            tx.unbounded_send(e).unwrap();
        }
        for e in sent {
            assert_eq!(rx.try_recv().unwrap(), e);
        }
    }
}
