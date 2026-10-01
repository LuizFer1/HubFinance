//! Estado e mensagens da janela. O estado e so o ultimo snapshot do nucleo mais o que e da
//! propria tela (QR desenhado, se esta fechando).

use std::time::Duration;

use iced::widget::qr_code;
use iced::{Element, Subscription, Task, window};
use time::UtcOffset;
use tokio::sync::watch;

use crate::hub::snapshot::{Snapshot, Status};
use crate::hub::{Command, HubHandle};

pub struct App {
    pub(super) handle: HubHandle,
    pub(super) snapshot: Snapshot,
    /// Reconstruido so quando o token muda: gerar a matriz a cada quadro e desperdicio.
    pub(super) qr: Option<qr_code::Data>,
    pub(super) closing: bool,
    /// Fuso local lido uma vez no `main`, antes de existirem outras threads.
    pub(super) offset: UtcOffset,
}

#[derive(Debug, Clone)]
pub enum Message {
    /// Em `Box`: o snapshot e grande e as outras mensagens nao precisam carregar o tamanho dele.
    Snapshot(Box<Snapshot>),
    IssueToken,
    Revoke(String),
    /// Redesenha a contagem regressiva e os "ha 3 min".
    Tick,
    CloseRequested,
    ForceExit,
}

pub fn boot(handle: HubHandle, offset: UtcOffset) -> impl Fn() -> (App, Task<Message>) {
    move || {
        let rx = handle.snapshot.clone();
        let app = App {
            snapshot: rx.borrow().clone(),
            handle: handle.clone(),
            qr: None,
            closing: false,
            offset,
        };
        let task = Task::run(snapshot_stream(rx), |s| Message::Snapshot(Box::new(s)));
        (app, task)
    }
}

/// `watch` -> Stream. `Task::run` aceita qualquer `Stream + Send`; nao precisa de
/// `Subscription::run_with` (que exigiria `Hash`).
fn snapshot_stream(rx: watch::Receiver<Snapshot>) -> impl futures::Stream<Item = Snapshot> + Send {
    futures::stream::unfold(rx, |mut rx| async move {
        // Sender caiu = nucleo terminou: o stream acaba.
        rx.changed().await.ok()?;
        let snap = rx.borrow_and_update().clone();
        Some((snap, rx))
    })
}

pub fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::Snapshot(s) => {
            let new_token = s.pairing.as_ref().map(|p| &p.token);
            let old_token = app.snapshot.pairing.as_ref().map(|p| &p.token);
            if new_token != old_token {
                app.qr = s
                    .pairing
                    .as_ref()
                    .and_then(|p| qr_code::Data::new(&p.qr_url).ok());
            }
            let stopped = s.status == Status::Stopped;
            app.snapshot = *s;
            if stopped && app.closing {
                iced::exit()
            } else {
                Task::none()
            }
        }
        Message::IssueToken => {
            let _ = app.handle.commands.send(Command::IssuePairingToken);
            Task::none()
        }
        Message::Revoke(id) => {
            let _ = app.handle.commands.send(Command::RevokeDevice(id));
            Task::none()
        }
        Message::Tick => Task::none(),
        Message::CloseRequested => {
            if app.closing {
                return Task::none();
            }
            app.closing = true;
            // Nucleo ja parado (ou morto): nao ha por que esperar o snapshot `Stopped`.
            if app.handle.commands.send(Command::Shutdown).is_err() {
                return iced::exit();
            }
            // Garantia: se o nucleo travar, a janela fecha mesmo assim em 5 s.
            Task::perform(delay(Duration::from_secs(5)), |_| Message::ForceExit)
        }
        Message::ForceExit => iced::exit(),
    }
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

/// Um tique por segundo, numa thread so. A thread termina quando o iced solta o receiver.
/// (`iced::time::every` exige o executor tokio ou smol; o padrao do iced e thread-pool.)
fn ticks() -> futures::channel::mpsc::Receiver<()> {
    let (mut tx, rx) = futures::channel::mpsc::channel(1);
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_secs(1));
            // Cheio = a UI ainda nao consumiu o anterior; pular um tique nao importa.
            if let Err(e) = tx.try_send(())
                && e.is_disconnected()
            {
                break;
            }
        }
    });
    rx
}

pub fn subscription(_app: &App) -> Subscription<Message> {
    Subscription::batch([
        window::close_requests().map(|_| Message::CloseRequested),
        Subscription::run(ticks).map(|()| Message::Tick),
    ])
}

pub fn view(app: &App) -> Element<'_, Message> {
    super::view::view(app)
}
