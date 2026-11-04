//! Integracao com o sistema: bandeja, instancia unica e "iniciar com o Windows". Unico modulo
//! que conhece `tray-icon`, `image` e `windows-sys`; `ui/` continua o unico com iced e recebe
//! daqui so mensagens simples por canal. O que decide algo e funcao pura e testada; o que
//! toca o sistema fica atras de `cfg(windows)` e do trait `Desktop`, para teste nenhum criar
//! bandeja, mutex nem chave de registro.
//!
//! TEMPORARIO: removido na Tarefa 10, quando `main.rs` passa a usar tudo.
#![allow(dead_code)]

pub mod args;
pub mod autostart;
pub mod instance;
pub mod tray;

use std::rc::Rc;

/// O que a bandeja, o menu ou uma segunda execucao pedem a janela.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DesktopEvent {
    /// Clique no icone, "Abrir o HubFinance" no menu, ou o programa aberto de novo.
    OpenWindow,
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
    fn set_tooltip(&self, text: &str);
    /// `None` = sistema sem a opcao (a linha nao aparece no card).
    fn autostart(&self) -> Option<bool>;
    fn set_autostart(&self, enabled: bool) -> Result<(), String>;
}

/// Linux, macOS, e Windows quando a bandeja nao subiu.
pub struct Headless;

impl Desktop for Headless {
    fn has_tray(&self) -> bool {
        false
    }
    fn set_tooltip(&self, _text: &str) {}
    fn autostart(&self) -> Option<bool> {
        None
    }
    fn set_autostart(&self, _enabled: bool) -> Result<(), String> {
        Err("sem suporte neste sistema".into())
    }
}

/// Monta a integracao do sistema atual. Preenchido na Tarefa 6; ate la, sem bandeja.
pub fn install(_events: EventSender) -> Rc<dyn Desktop> {
    Rc::new(Headless)
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
    pub autostart: std::cell::Cell<Option<bool>>,
    pub fail_autostart: std::cell::Cell<bool>,
}

#[cfg(test)]
impl Fake {
    pub fn with_tray() -> Fake {
        Fake {
            tray: true,
            tooltip: Default::default(),
            autostart: std::cell::Cell::new(Some(false)),
            fail_autostart: Default::default(),
        }
    }
}

#[cfg(test)]
impl Desktop for Fake {
    fn has_tray(&self) -> bool {
        self.tray
    }
    fn set_tooltip(&self, text: &str) {
        *self.tooltip.borrow_mut() = text.to_string();
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
        assert!(d.set_autostart(true).is_err());
        d.set_tooltip("ignorado");
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
    fn canal_entrega_na_ordem() {
        let (tx, mut rx) = channel();
        tx.unbounded_send(DesktopEvent::OpenWindow).unwrap();
        tx.unbounded_send(DesktopEvent::Quit).unwrap();
        assert_eq!(rx.try_recv().unwrap(), DesktopEvent::OpenWindow);
        assert_eq!(rx.try_recv().unwrap(), DesktopEvent::Quit);
    }
}
