//! Estado e mensagens da janela. O estado e o ultimo snapshot do nucleo mais o que e da
//! propria tela: tela atual, tema, janela (id, maximizada), QR desenhado, se esta fechando.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use iced::widget::{image, qr_code};
use iced::{Element, Size, Subscription, Task, window};
use time::UtcOffset;
use tokio::sync::watch;

use super::connection_view;
use super::theme::Tokens;
use crate::config::{ThemeMode, UiPrefs};
use crate::dashboard::dataset::avatar_bytes;
use crate::dashboard::people::{DeviceInfo, Person, people};
use crate::hub::snapshot::{Snapshot, Status};
use crate::hub::{Command, HubHandle};

/// Quanto tempo "Copiado" fica no botao, como no prototipo.
pub const COPIED_FOR: Duration = Duration::from_millis(1600);

/// Tamanho inicial da janela (`ui::run`); vale ate o primeiro evento de redimensionamento.
pub const INITIAL_SIZE: Size = Size::new(1440.0, 900.0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CopyTarget {
    Address,
    CaLink,
}

/// Foto decodificada e o data URI de onde veio: so decodifica de novo quando o URI muda.
pub struct Avatar {
    pub uri: String,
    pub handle: image::Handle,
}

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
    /// Largura da janela: o iced nao tem `auto-fit`, entao a tela decide colunas por ela.
    pub(super) window_size: Size,
    /// Linhas de "Usuarios conectados", recalculadas so quando aparelhos ou dataset mudam.
    pub(super) people: Vec<Person>,
    /// `device_id` -> foto.
    pub(super) avatars: HashMap<String, Avatar>,
    /// Qual campo mostra "Copiado" e ate quando.
    pub(super) copied: Option<(CopyTarget, Instant)>,
    /// Linha sob o mouse em "Usuarios conectados" (hover de 4 %).
    pub(super) hover_row: Option<String>,
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
    /// A janela mudou de tamanho (arrasto, Win+seta, encaixe): guarda a largura e reconsulta
    /// se esta maximizada.
    Resized(Size),
    Resize(window::Direction),
    SelectScreen(Screen),
    SetTheme(ThemeMode),
    Copy(CopyTarget),
    CopyExpired,
    HoverRow(String),
    UnhoverRow(String),
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
        let mut app = App {
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
            window_size: INITIAL_SIZE,
            people: Vec::new(),
            avatars: HashMap::new(),
            copied: None,
            hover_row: None,
        };
        app.refresh_people();
        app
    }

    /// Pessoas e fotos a partir do snapshot atual. A foto so e decodificada de novo quando o
    /// data URI muda: decodificar base64 de 60 KB a cada push seria desperdicio.
    pub(super) fn refresh_people(&mut self) {
        let infos: Vec<DeviceInfo> = self.snapshot.devices.iter().map(DeviceInfo::from).collect();
        self.people = people(&self.snapshot.dashboard, &infos);
        let mut avatars = HashMap::new();
        for person in &self.people {
            let Some(uri) = &person.avatar_uri else {
                continue;
            };
            let avatar = match self.avatars.remove(&person.device_id) {
                Some(old) if &old.uri == uri => old,
                _ => match avatar_bytes(uri) {
                    Some(bytes) => Avatar {
                        uri: uri.clone(),
                        handle: image::Handle::from_bytes(bytes),
                    },
                    None => continue,
                },
            };
            avatars.insert(person.device_id.clone(), avatar);
        }
        self.avatars = avatars;
    }

    /// "Copiado" ainda vale para este campo?
    pub(super) fn is_copied(&self, target: CopyTarget) -> bool {
        self.copied
            .is_some_and(|(t, until)| t == target && Instant::now() < until)
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
            let people_changed = !std::sync::Arc::ptr_eq(&s.dashboard, &app.snapshot.dashboard)
                || s.devices != app.snapshot.devices;
            app.snapshot = *s;
            if people_changed {
                app.refresh_people();
            }
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
        Message::Resized(size) => {
            app.window_size = size;
            app.with_window(|id| window::is_maximized(id).map(Message::Maximized))
        }
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
        Message::Copy(target) => {
            let text = match target {
                CopyTarget::Address => connection_view::hub_url(&app.snapshot),
                CopyTarget::CaLink => connection_view::ca_url(&app.snapshot),
            };
            let Some(text) = text else {
                return Task::none();
            };
            app.copied = Some((target, Instant::now() + COPIED_FOR));
            // O iced nao devolve falha da area de transferencia; o rotulo vira "Copiado" mesmo
            // assim (limite aceito na spec).
            Task::batch([
                iced::clipboard::write(text),
                Task::perform(delay(COPIED_FOR), |_| Message::CopyExpired),
            ])
        }
        Message::CopyExpired => {
            // Um segundo clique adiou o prazo: o timer do primeiro nao apaga o do segundo.
            if app.copied.is_some_and(|(_, until)| Instant::now() >= until) {
                app.copied = None;
            }
            Task::none()
        }
        Message::HoverRow(id) => {
            app.hover_row = Some(id);
            Task::none()
        }
        Message::UnhoverRow(id) => {
            // Saida de uma linha pode chegar depois da entrada na vizinha.
            if app.hover_row.as_ref() == Some(&id) {
                app.hover_row = None;
            }
            Task::none()
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
        window::resize_events().map(|(_, size)| Message::Resized(size)),
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
    fn copiar_marca_o_campo_e_expira() {
        let dir = tempfile::tempdir().unwrap();
        let snap = Snapshot {
            addresses: vec![std::net::Ipv4Addr::new(192, 168, 0, 12)],
            https_port: 7777,
            http_port: 7778,
            ..Snapshot::default()
        };
        let (mut app, _rx, _tx) = app_with(snap, dir.path().to_path_buf());
        let _ = update(&mut app, Message::Copy(CopyTarget::Address));
        assert!(app.is_copied(CopyTarget::Address));
        assert!(!app.is_copied(CopyTarget::CaLink));
        // Timer chegou antes do prazo (outro clique adiou): continua.
        let _ = update(&mut app, Message::CopyExpired);
        assert!(app.copied.is_some());
        app.copied = Some((CopyTarget::Address, Instant::now()));
        let _ = update(&mut app, Message::CopyExpired);
        assert!(app.copied.is_none());
    }

    #[test]
    fn copiar_sem_rede_nao_faz_nada() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        let _ = update(&mut app, Message::Copy(CopyTarget::Address));
        assert!(app.copied.is_none());
    }

    #[test]
    fn hover_de_linha_tolera_saida_atrasada() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        let _ = update(&mut app, Message::HoverRow("A".into()));
        let _ = update(&mut app, Message::HoverRow("B".into()));
        let _ = update(&mut app, Message::UnhoverRow("A".into()));
        assert_eq!(app.hover_row.as_deref(), Some("B"));
        let _ = update(&mut app, Message::UnhoverRow("B".into()));
        assert_eq!(app.hover_row, None);
    }

    #[test]
    fn snapshot_novo_recalcula_pessoas_e_fotos() {
        use crate::dashboard::dataset::{Dataset, RawRow};
        use crate::hub::snapshot::DeviceView;

        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        assert!(app.people.is_empty());

        let user = "01HZZZZZZZZZZZZZZZZZZZZZP1";
        let mut ds = Dataset::default();
        ds.apply([RawRow {
            table: "users".into(),
            id: user.into(),
            deleted_at: None,
            seq: 1,
            data: serde_json::json!({
                "name": "Ana",
                "color": "fuchsia",
                "avatar": "data:image/png;base64,iVBORw0KGgo="
            })
            .to_string(),
        }]);
        let device = DeviceView {
            device_id: "D1".into(),
            name: "Pixel da Ana".into(),
            paired_at: "2026-09-01T10:00:00Z".into(),
            last_seen_at: None,
            last_push_at: None,
            last_pull_at: None,
            revoked: false,
            user_id: Some(user.into()),
        };
        let snap = Snapshot {
            devices: vec![device],
            dashboard: std::sync::Arc::new(ds),
            ..Snapshot::default()
        };
        let _ = update(&mut app, Message::Snapshot(Box::new(snap)));
        assert_eq!(app.people.len(), 1);
        assert_eq!(app.people[0].display_name, "Ana");
        assert!(app.avatars.contains_key("D1"));
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
