//! Estado e mensagens da janela. O estado e o ultimo snapshot do nucleo mais o que e da
//! propria tela: tela atual, tema, janela (id, maximizada), QR desenhado, se esta fechando.

use std::path::PathBuf;
use std::time::Duration;

use iced::widget::qr_code;
use iced::{Element, Subscription, Task, window};
use time::UtcOffset;
use tokio::sync::watch;

use super::theme::Tokens;
use crate::config::{ThemeMode, UiPrefs};
use crate::hub::snapshot::{Snapshot, Status};
use crate::hub::{Command, HubHandle};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Screen {
    #[default]
    Dashboard,
    Transactions,
    Connection,
}

pub struct App {
    pub(super) handle: HubHandle,
    pub(super) snapshot: Snapshot,
    /// Reconstruido so quando o token muda: gerar a matriz a cada quadro e desperdicio.
    pub(super) qr: Option<qr_code::Data>,
    pub(super) closing: bool,
    /// Fuso local lido uma vez no `main`, antes de existirem outras threads.
    pub(super) offset: UtcOffset,
    /// `None` ate o `window::oldest()` do boot responder; sem ele os botoes da barra de titulo
    /// nao fazem nada (e o `warn!` sai uma vez so).
    pub(super) window: Option<window::Id>,
    pub(super) warned_no_window: bool,
    /// Maximizada, a moldura de redimensionar some: borda de janela maximizada nao arrasta.
    pub(super) maximized: bool,
    /// Botao pressionado na barra de titulo e ainda sem arrasto: o arrasto so comeca quando o
    /// mouse anda, para o duplo clique (maximizar) nao ser engolido pelo laco de movimento do
    /// Windows.
    pub(super) title_pressed: bool,
    pub(super) theme: ThemeMode,
    pub(super) tokens: &'static Tokens,
    /// `Theme::custom` gera a paleta estendida a cada chamada; guardado para nao refazer isso
    /// a cada quadro.
    pub(super) iced_theme: iced::Theme,
    pub(super) screen: Screen,
    /// Onde fica o `ui.json` (o diretorio de dados do hub).
    pub(super) prefs_dir: PathBuf,
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
    WindowId(Option<window::Id>),
    TitlePressed,
    TitleReleased,
    TitleMoved,
    Minimize,
    ToggleMaximize,
    Maximized(bool),
    /// A janela mudou de tamanho por fora (Win+seta, encaixe): reconsulta se esta maximizada.
    Resized,
    Resize(window::Direction),
    SelectScreen(Screen),
    SetTheme(ThemeMode),
    Noop,
}

impl App {
    pub(super) fn new(
        handle: HubHandle,
        offset: UtcOffset,
        prefs: UiPrefs,
        prefs_dir: PathBuf,
    ) -> App {
        let snapshot = handle.snapshot.borrow().clone();
        let tokens = Tokens::for_mode(prefs.theme);
        App {
            handle,
            snapshot,
            qr: None,
            closing: false,
            offset,
            window: None,
            warned_no_window: false,
            maximized: false,
            title_pressed: false,
            theme: prefs.theme,
            tokens,
            iced_theme: tokens.iced_theme(),
            screen: Screen::default(),
            prefs_dir,
        }
    }

    /// Acao sobre a janela; sem id ainda, nada (com um aviso so, para nao inundar o log).
    fn with_window(&mut self, action: impl FnOnce(window::Id) -> Task<Message>) -> Task<Message> {
        match self.window {
            Some(id) => action(id),
            None => {
                if !self.warned_no_window {
                    self.warned_no_window = true;
                    tracing::warn!("janela sem id: botoes da barra de titulo sem efeito");
                }
                Task::none()
            }
        }
    }
}

pub fn boot(
    handle: HubHandle,
    offset: UtcOffset,
    prefs: UiPrefs,
    prefs_dir: PathBuf,
) -> impl Fn() -> (App, Task<Message>) {
    move || {
        let rx = handle.snapshot.clone();
        let app = App::new(handle.clone(), offset, prefs.clone(), prefs_dir.clone());
        let task = Task::batch([
            Task::run(snapshot_stream(rx), |s| Message::Snapshot(Box::new(s))),
            window::oldest().map(Message::WindowId),
        ]);
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
        Message::WindowId(id) => {
            app.window = id;
            app.with_window(|id| window::is_maximized(id).map(Message::Maximized))
        }
        Message::TitlePressed => {
            app.title_pressed = true;
            Task::none()
        }
        Message::TitleReleased => {
            app.title_pressed = false;
            Task::none()
        }
        Message::TitleMoved => {
            if !app.title_pressed {
                return Task::none();
            }
            // O laco de movimento do sistema consome o soltar do botao: sem zerar aqui, o
            // proximo movimento sem botao comecaria outro arrasto.
            app.title_pressed = false;
            app.with_window(window::drag)
        }
        Message::Minimize => app.with_window(|id| window::minimize(id, true)),
        Message::ToggleMaximize => {
            app.title_pressed = false;
            app.with_window(|id| {
                window::toggle_maximize(id).chain(window::is_maximized(id).map(Message::Maximized))
            })
        }
        Message::Maximized(maximized) => {
            app.maximized = maximized;
            Task::none()
        }
        Message::Resized => app.with_window(|id| window::is_maximized(id).map(Message::Maximized)),
        Message::Resize(direction) => app.with_window(|id| window::drag_resize(id, direction)),
        Message::SelectScreen(screen) => {
            app.screen = screen;
            Task::none()
        }
        Message::SetTheme(mode) => {
            if mode == app.theme {
                return Task::none();
            }
            app.theme = mode;
            app.tokens = Tokens::for_mode(mode);
            app.iced_theme = app.tokens.iced_theme();
            Task::perform(
                save_prefs(UiPrefs { theme: mode }, app.prefs_dir.clone()),
                |_| Message::Noop,
            )
        }
        Message::Noop => Task::none(),
    }
}

/// Grava o `ui.json` fora da thread da UI (disco lento nao trava a janela). Falha so vira
/// aviso: a preferencia e conforto, o tema ja trocou na tela.
fn save_prefs(prefs: UiPrefs, dir: PathBuf) -> futures::channel::oneshot::Receiver<()> {
    let (tx, rx) = futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        if let Err(e) = prefs.save(&dir) {
            tracing::warn!("nao foi possivel gravar ui.json em {}: {e}", dir.display());
        }
        let _ = tx.send(());
    });
    rx
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
        window::resize_events().map(|_| Message::Resized),
        Subscription::run(ticks).map(|()| Message::Tick),
    ])
}

pub fn view(app: &App) -> Element<'_, Message> {
    super::view::view(app)
}

#[cfg(test)]
pub(super) mod tests {
    use tokio::sync::mpsc;

    use super::*;

    /// App de teste com canais de verdade: o receptor de comandos fica com o teste.
    pub(crate) fn app_with(
        snapshot: Snapshot,
        prefs_dir: PathBuf,
    ) -> (
        App,
        mpsc::UnboundedReceiver<Command>,
        watch::Sender<Snapshot>,
    ) {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (snap_tx, snap_rx) = watch::channel(snapshot);
        let handle = HubHandle {
            commands: cmd_tx,
            snapshot: snap_rx,
        };
        let app = App::new(handle, UtcOffset::UTC, UiPrefs::default(), prefs_dir);
        (app, cmd_rx, snap_tx)
    }

    #[test]
    fn nasce_no_dashboard_com_o_tema_das_preferencias() {
        let dir = tempfile::tempdir().unwrap();
        let (cmd_tx, _rx) = mpsc::unbounded_channel();
        let (_tx, snap_rx) = watch::channel(Snapshot::default());
        let app = App::new(
            HubHandle {
                commands: cmd_tx,
                snapshot: snap_rx,
            },
            UtcOffset::UTC,
            UiPrefs {
                theme: ThemeMode::Light,
            },
            dir.path().to_path_buf(),
        );
        assert_eq!(app.screen, Screen::Dashboard);
        assert_eq!(app.theme, ThemeMode::Light);
        assert!(std::ptr::eq(app.tokens, &super::super::theme::LIGHT));
    }

    #[test]
    fn trocar_de_tela() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        let _ = update(&mut app, Message::SelectScreen(Screen::Connection));
        assert_eq!(app.screen, Screen::Connection);
    }

    #[test]
    fn trocar_o_tema_troca_os_tokens_e_grava_ui_json() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        let _ = update(&mut app, Message::SetTheme(ThemeMode::Light));
        assert_eq!(app.theme, ThemeMode::Light);
        assert!(std::ptr::eq(app.tokens, &super::super::theme::LIGHT));
        // A gravacao corre numa thread; espera com prazo.
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while UiPrefs::load(dir.path()).theme != ThemeMode::Light {
            assert!(std::time::Instant::now() < deadline, "ui.json nao gravado");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn barra_de_titulo_so_arrasta_depois_de_pressionar() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        // Mover sem botao: nada.
        let _ = update(&mut app, Message::TitleMoved);
        assert!(!app.title_pressed);
        let _ = update(&mut app, Message::TitlePressed);
        assert!(app.title_pressed);
        // Mover com botao: arrasta uma vez e esquece o botao (o sistema engole o soltar).
        let _ = update(&mut app, Message::TitleMoved);
        assert!(!app.title_pressed);
        // Duplo clique tambem esquece o botao.
        let _ = update(&mut app, Message::TitlePressed);
        let _ = update(&mut app, Message::ToggleMaximize);
        assert!(!app.title_pressed);
    }

    #[test]
    fn sem_id_de_janela_avisa_uma_vez() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        let _ = update(&mut app, Message::Minimize);
        assert!(app.warned_no_window);
        let _ = update(&mut app, Message::Maximized(true));
        assert!(app.maximized);
    }

    #[test]
    fn fechar_manda_shutdown_uma_vez() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        let _ = update(&mut app, Message::CloseRequested);
        let _ = update(&mut app, Message::CloseRequested);
        assert!(matches!(rx.try_recv(), Ok(Command::Shutdown)));
        assert!(rx.try_recv().is_err());
    }
}
