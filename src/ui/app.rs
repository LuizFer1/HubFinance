//! Estado e mensagens da janela. O estado e o ultimo snapshot do nucleo mais o que e da
//! propria tela: tela atual, tema, janela (id, maximizada), QR desenhado, se esta fechando.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use iced::widget::{canvas, image, qr_code};
use iced::{Element, Size, Subscription, Task, window};
use time::macros::format_description;
use time::{OffsetDateTime, UtcOffset};
use tokio::sync::watch;

use super::connection_view;
use super::theme::Tokens;
use crate::config::{ThemeMode, UiPrefs};
use crate::dashboard::dataset::avatar_bytes;
use crate::dashboard::list::Filters;
use crate::dashboard::people::{DeviceInfo, Person, people};
use crate::dashboard::periods::month_window;
use crate::dashboard::view::{
    DashboardView, TransactionsView, build_dashboard, build_transactions,
};
use crate::hub::snapshot::{Snapshot, Status};
use crate::hub::{Command, HubHandle};

/// Quanto tempo "Copiado" fica no botao, como no prototipo.
pub const COPIED_FOR: Duration = Duration::from_millis(1600);

/// Quanto tempo o toast de "Desfazer" fica na tela antes de a revogacao ser enviada.
pub const UNDO_FOR: Duration = Duration::from_secs(5);

/// Remocao confirmada mas ainda nao enviada ao nucleo.
///
/// Revogacao adiada, e nao "revogar e des-revogar": durante os 5 s o celular nunca recebe
/// `401` (que a fatia 3 do app poderia tratar descartando a chave, e ai "Desfazer" mentiria).
/// Custo aceito: se o hub cair nesses 5 s, a remocao nao aconteceu e a pessoa reaparece.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingRemoval {
    pub device_id: String,
    pub toast: String,
    pub deadline: Instant,
}

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
    /// Dialogo "Remover ...?" aberto para este `device_id`.
    pub(super) confirm_remove: Option<String>,
    pub(super) pending_removal: Option<PendingRemoval>,
    /// "Hoje" (`YYYY-MM-DD`) no fuso local: a unica leitura de relogio que decide algo na tela
    /// (mes corrente, tag "Parcial"). Nasce no boot e e reconferida a cada tique.
    pub(super) today: String,
    /// Os 12 meses navegaveis (`month_window(today)`), o mais antigo primeiro.
    pub(super) months: Vec<String>,
    /// Indice em `months`; 11 = mes corrente.
    pub(super) month_idx: usize,
    /// Filtros da tela Lancamentos; sobrevivem a troca de mes.
    pub(super) filters: Filters,
    /// Modelos de tela, recalculados so quando dados, mes, filtros ou "hoje" mudam: desenhar
    /// um quadro nunca percorre o dataset.
    pub(super) dashboard: DashboardView,
    pub(super) transactions: TransactionsView,
    /// Categoria sob o mouse na legenda da rosca.
    pub(super) hover_category: Option<String>,
    /// Geometria dos graficos; limpa quando dados, mes, hover da legenda ou tema mudam.
    pub(super) donut_cache: canvas::Cache,
    pub(super) bars_cache: canvas::Cache,
    pub(super) empty_cache: canvas::Cache,
    /// `user_id` -> foto, para as linhas e os chips de autor (`avatars` e por aparelho).
    pub(super) user_avatars: HashMap<String, Avatar>,
}

#[derive(Debug, Clone)]
pub enum Message {
    /// Em `Box`: o snapshot e grande e as outras mensagens nao precisam carregar o tamanho dele.
    Snapshot(Box<Snapshot>),
    IssueToken,
    /// Redesenha a contagem regressiva e os "ha 3 min"; confere a virada do dia.
    Tick,
    PrevMonth,
    NextMonth,
    /// Mouse entrou numa linha da legenda da rosca.
    HoverCategory(String),
    /// Mouse saiu: so limpa se ainda for a mesma (a saida pode chegar depois da entrada na
    /// vizinha).
    UnhoverCategory(String),
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
    AskRemove(String),
    CancelRemove,
    ConfirmRemove,
    UndoRemove,
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
            confirm_remove: None,
            pending_removal: None,
            today: String::new(),
            months: Vec::new(),
            month_idx: 0,
            filters: Filters::default(),
            dashboard: DashboardView::default(),
            transactions: TransactionsView::default(),
            hover_category: None,
            donut_cache: canvas::Cache::new(),
            bars_cache: canvas::Cache::new(),
            empty_cache: canvas::Cache::new(),
            user_avatars: HashMap::new(),
        };
        app.refresh_people();
        app.set_today(local_today(offset));
        app
    }

    /// Mes escolhido no seletor.
    pub(super) fn month(&self) -> &str {
        self.months.get(self.month_idx).map_or("", String::as_str)
    }

    /// Troca "hoje". A janela de 12 meses anda junto; o mes escolhido continua o mesmo se
    /// ainda estiver nela (quem olhava agosto continua em agosto), senao vai para o corrente.
    pub(super) fn set_today(&mut self, today: String) {
        if today == self.today {
            return;
        }
        let previous = self.month().to_string();
        self.months = month_window(&today);
        let last = self.months.len().saturating_sub(1);
        self.month_idx = self
            .months
            .iter()
            .position(|m| *m == previous)
            .unwrap_or(last);
        self.today = today;
        self.rebuild();
    }

    /// Recalcula as duas telas a partir do snapshot atual.
    pub(super) fn rebuild(&mut self) {
        let dataset = &self.snapshot.dashboard;
        let month = self.month().to_string();
        self.dashboard = build_dashboard(dataset, &self.today, &month);
        self.transactions = build_transactions(dataset, &self.today, &month, &self.filters);
        // "N lançamentos sincronizados de K aparelhos": so aparelhos ativos que ja enviaram.
        self.transactions.synced_devices = self
            .snapshot
            .devices
            .iter()
            .filter(|d| !d.revoked && d.last_push_at.is_some())
            .count();
        self.refresh_user_avatars();
        self.clear_charts();
        tracing::trace!("telas recalculadas para {month}");
    }

    /// Fotos dos perfis vivos, decodificadas so quando o data URI muda.
    fn refresh_user_avatars(&mut self) {
        let mut avatars = HashMap::new();
        for user in self.snapshot.dashboard.alive_users() {
            let Some(uri) = &user.avatar else {
                continue;
            };
            let avatar = match self.user_avatars.remove(&user.id) {
                Some(old) if &old.uri == uri => old,
                _ => match avatar_bytes(uri) {
                    Some(bytes) => Avatar {
                        uri: uri.clone(),
                        handle: image::Handle::from_bytes(bytes),
                    },
                    None => continue,
                },
            };
            avatars.insert(user.id.clone(), avatar);
        }
        self.user_avatars = avatars;
    }

    fn clear_charts(&self) {
        self.donut_cache.clear();
        self.bars_cache.clear();
        self.empty_cache.clear();
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

    /// Pessoas que a tela mostra: sem a da remocao pendente (ela some na hora, como no design).
    pub(super) fn visible_people(&self) -> impl Iterator<Item = &Person> {
        let pending = self.pending_removal.as_ref().map(|p| p.device_id.as_str());
        self.people
            .iter()
            .filter(move |p| Some(p.device_id.as_str()) != pending)
    }

    /// Envia a revogacao pendente agora (prazo vencido, outra remocao, troca de tela ou
    /// fechamento).
    fn commit_pending(&mut self) {
        if let Some(pending) = self.pending_removal.take() {
            let _ = self
                .handle
                .commands
                .send(Command::RevokeDevice(pending.device_id));
        }
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
                app.rebuild();
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
        Message::Tick => {
            if app
                .pending_removal
                .as_ref()
                .is_some_and(|p| Instant::now() >= p.deadline)
            {
                app.commit_pending();
            }
            app.set_today(local_today(app.offset));
            Task::none()
        }
        Message::PrevMonth => {
            if app.month_idx > 0 {
                app.month_idx -= 1;
                app.hover_category = None;
                app.rebuild();
            }
            Task::none()
        }
        Message::NextMonth => {
            if app.month_idx + 1 < app.months.len() {
                app.month_idx += 1;
                app.hover_category = None;
                app.rebuild();
            }
            Task::none()
        }
        Message::HoverCategory(key) => {
            app.hover_category = Some(key);
            app.donut_cache.clear();
            Task::none()
        }
        Message::UnhoverCategory(key) => {
            if app.hover_category.as_ref() == Some(&key) {
                app.hover_category = None;
                app.donut_cache.clear();
            }
            Task::none()
        }
        Message::CloseRequested => {
            if app.closing {
                return Task::none();
            }
            app.closing = true;
            // Antes do `Shutdown`, no mesmo canal: o nucleo processa em ordem.
            app.commit_pending();
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
            app.commit_pending();
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
            // As cores dos graficos estao gravadas na geometria em cache.
            app.clear_charts();
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
        Message::AskRemove(device_id) => {
            app.confirm_remove = Some(device_id);
            Task::none()
        }
        Message::CancelRemove => {
            app.confirm_remove = None;
            Task::none()
        }
        Message::ConfirmRemove => {
            let Some(device_id) = app.confirm_remove.take() else {
                return Task::none();
            };
            // So uma remocao desfazivel por vez: a anterior vai agora.
            app.commit_pending();
            let toast = app
                .people
                .iter()
                .find(|p| p.device_id == device_id)
                .map_or_else(|| "Aparelho removido do hub".to_string(), removal_toast);
            app.pending_removal = Some(PendingRemoval {
                device_id,
                toast,
                deadline: Instant::now() + UNDO_FOR,
            });
            Task::none()
        }
        Message::UndoRemove => {
            // Nada foi enviado ao nucleo: desfazer e so esquecer.
            app.pending_removal = None;
            Task::none()
        }
        Message::Noop => Task::none(),
    }
}

/// "Ana foi removida do hub". O genero e uma heuristica (a do prototipo): nome de pessoa
/// terminado em "a" e feminino; nome de aparelho (sem perfil conhecido) e sempre masculino
/// ("o Pixel 7").
pub(super) fn removal_toast(person: &Person) -> String {
    let feminine = person.user_known && person.display_name.trim_end().ends_with(['a', 'A']);
    format!(
        "{} foi {} do hub",
        person.display_name,
        if feminine { "removida" } else { "removido" }
    )
}

/// "Hoje" no fuso local, `YYYY-MM-DD`. Relogio so para exibicao e recorte, nunca para merge.
fn local_today(offset: UtcOffset) -> String {
    OffsetDateTime::now_utc()
        .to_offset(offset)
        .date()
        .format(format_description!("[year]-[month]-[day]"))
        .unwrap_or_default()
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

    fn app_with_two_people(
        dir: &std::path::Path,
    ) -> (
        App,
        mpsc::UnboundedReceiver<Command>,
        watch::Sender<Snapshot>,
    ) {
        use crate::hub::snapshot::DeviceView;
        let device = |id: &str, name: &str| DeviceView {
            device_id: id.into(),
            name: name.into(),
            paired_at: "2026-09-01T10:00:00Z".into(),
            last_seen_at: None,
            last_push_at: None,
            last_pull_at: None,
            revoked: false,
            user_id: None,
        };
        let snap = Snapshot {
            devices: vec![device("D1", "Pixel 7"), device("D2", "iPhone 13")],
            ..Snapshot::default()
        };
        app_with(snap, dir.to_path_buf())
    }

    fn revoked(rx: &mut mpsc::UnboundedReceiver<Command>) -> Vec<String> {
        let mut out = Vec::new();
        while let Ok(cmd) = rx.try_recv() {
            if let Command::RevokeDevice(id) = cmd {
                out.push(id);
            }
        }
        out
    }

    #[test]
    fn remover_pede_confirmacao_e_cancelar_nao_faz_nada() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with_two_people(dir.path());
        let _ = update(&mut app, Message::AskRemove("D1".into()));
        assert_eq!(app.confirm_remove.as_deref(), Some("D1"));
        let _ = update(&mut app, Message::CancelRemove);
        assert_eq!(app.confirm_remove, None);
        assert_eq!(app.pending_removal, None);
        assert!(revoked(&mut rx).is_empty());
        assert_eq!(app.visible_people().count(), 2);
    }

    #[test]
    fn confirmar_esconde_a_linha_e_desfazer_nao_envia_nada() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with_two_people(dir.path());
        let _ = update(&mut app, Message::AskRemove("D1".into()));
        let _ = update(&mut app, Message::ConfirmRemove);
        assert_eq!(app.confirm_remove, None);
        let pending = app.pending_removal.clone().unwrap();
        assert_eq!(pending.device_id, "D1");
        assert_eq!(pending.toast, "Pixel 7 foi removido do hub");
        assert_eq!(
            app.visible_people()
                .map(|p| p.device_id.as_str())
                .collect::<Vec<_>>(),
            vec!["D2"]
        );
        // Tique antes do prazo: nada sai.
        let _ = update(&mut app, Message::Tick);
        assert!(revoked(&mut rx).is_empty());
        let _ = update(&mut app, Message::UndoRemove);
        assert_eq!(app.pending_removal, None);
        assert_eq!(app.visible_people().count(), 2);
        assert!(revoked(&mut rx).is_empty());
    }

    #[test]
    fn prazo_vencido_envia_a_revogacao() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with_two_people(dir.path());
        let _ = update(&mut app, Message::AskRemove("D1".into()));
        let _ = update(&mut app, Message::ConfirmRemove);
        app.pending_removal.as_mut().unwrap().deadline = Instant::now();
        let _ = update(&mut app, Message::Tick);
        assert_eq!(revoked(&mut rx), vec!["D1".to_string()]);
        assert_eq!(app.pending_removal, None);
    }

    #[test]
    fn outra_remocao_ou_trocar_de_tela_envia_a_pendente() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with_two_people(dir.path());
        let _ = update(&mut app, Message::AskRemove("D1".into()));
        let _ = update(&mut app, Message::ConfirmRemove);
        let _ = update(&mut app, Message::AskRemove("D2".into()));
        let _ = update(&mut app, Message::ConfirmRemove);
        assert_eq!(revoked(&mut rx), vec!["D1".to_string()]);
        assert_eq!(app.pending_removal.as_ref().unwrap().device_id, "D2");
        let _ = update(&mut app, Message::SelectScreen(Screen::Dashboard));
        assert_eq!(revoked(&mut rx), vec!["D2".to_string()]);
    }

    #[test]
    fn fechar_com_remocao_pendente_revoga_antes_do_shutdown() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with_two_people(dir.path());
        let _ = update(&mut app, Message::AskRemove("D1".into()));
        let _ = update(&mut app, Message::ConfirmRemove);
        let _ = update(&mut app, Message::CloseRequested);
        assert!(matches!(rx.try_recv(), Ok(Command::RevokeDevice(id)) if id == "D1"));
        assert!(matches!(rx.try_recv(), Ok(Command::Shutdown)));
    }

    #[test]
    fn genero_do_toast() {
        use crate::dashboard::people::Person;
        let person = |name: &str, known: bool| Person {
            device_id: "D".into(),
            display_name: name.into(),
            color: None,
            color_name: None,
            avatar_uri: None,
            device_name: "Pixel".into(),
            paired_at: String::new(),
            last_seen_at: None,
            user_known: known,
        };
        assert_eq!(
            removal_toast(&person("Ana", true)),
            "Ana foi removida do hub"
        );
        assert_eq!(
            removal_toast(&person("Luiz", true)),
            "Luiz foi removido do hub"
        );
        assert_eq!(
            removal_toast(&person("Pixel da Ana", false)),
            "Pixel da Ana foi removido do hub"
        );
    }

    /// App com o dataset de exemplo e "hoje" fixo em 24 de setembro de 2026.
    pub(crate) fn app_with_fixtures(
        dir: &std::path::Path,
    ) -> (
        App,
        mpsc::UnboundedReceiver<Command>,
        watch::Sender<Snapshot>,
    ) {
        use crate::hub::snapshot::DeviceView;
        let device = |id: &str, push: Option<&str>, revoked: bool| DeviceView {
            device_id: id.into(),
            name: format!("Aparelho {id}"),
            paired_at: "2026-09-01T10:00:00Z".into(),
            last_seen_at: None,
            last_push_at: push.map(Into::into),
            last_pull_at: None,
            revoked,
            user_id: None,
        };
        let snap = Snapshot {
            devices: vec![
                device("D1", Some("2026-09-24T10:00:00Z"), false),
                device("D2", None, false),
                device("D3", Some("2026-09-01T10:00:00Z"), true),
            ],
            dashboard: std::sync::Arc::new(crate::dashboard::fixtures()),
            ..Snapshot::default()
        };
        let (mut app, rx, tx) = app_with(snap, dir.to_path_buf());
        app.set_today("2026-09-24".into());
        (app, rx, tx)
    }

    #[test]
    fn nasce_no_mes_corrente_com_as_telas_calculadas() {
        let dir = tempfile::tempdir().unwrap();
        let (app, _rx, _tx) = app_with_fixtures(dir.path());
        assert_eq!(app.months.len(), 12);
        assert_eq!(app.month_idx, 11);
        assert_eq!(app.month(), "2026-09");
        assert_eq!(app.dashboard.month, "2026-09");
        assert!(app.dashboard.is_current);
        assert_eq!(app.dashboard.partial_until.as_deref(), Some("24 set"));
        assert_eq!(app.transactions.list.count, 7);
        // So D1: D2 nunca enviou, D3 foi revogado.
        assert_eq!(app.transactions.synced_devices, 1);
    }

    #[test]
    fn seletor_de_mes_satura_nos_limites() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with_fixtures(dir.path());
        let _ = update(&mut app, Message::NextMonth);
        assert_eq!(app.month_idx, 11, "nao passa do corrente");
        let _ = update(&mut app, Message::PrevMonth);
        assert_eq!(app.month(), "2026-08");
        assert_eq!(app.dashboard.month, "2026-08");
        assert!(!app.dashboard.is_current);
        assert_eq!(app.dashboard.partial_until, None);
        assert_eq!(app.transactions.month, "2026-08");
        for _ in 0..20 {
            let _ = update(&mut app, Message::PrevMonth);
        }
        assert_eq!(app.month_idx, 0);
        assert_eq!(app.month(), "2025-10");
        let _ = update(&mut app, Message::NextMonth);
        assert_eq!(app.month(), "2025-11");
    }

    #[test]
    fn virada_do_dia_anda_a_janela_e_mantem_o_mes() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with_fixtures(dir.path());
        let _ = update(&mut app, Message::PrevMonth);
        app.set_today("2026-10-01".into());
        assert_eq!(app.months.last().map(String::as_str), Some("2026-10"));
        assert_eq!(app.month(), "2026-08");
        assert_eq!(app.month_idx, 9);
        // O mes escolhido saiu da janela: volta ao corrente.
        app.set_today("2027-12-01".into());
        assert_eq!(app.month_idx, 11);
        assert_eq!(app.month(), "2027-12");
        assert_eq!(app.dashboard.month, "2027-12");
        assert!(app.dashboard.is_current);
    }

    #[test]
    fn snapshot_com_dataset_novo_recalcula_as_telas() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        app.set_today("2026-09-24".into());
        assert!(app.dashboard.is_empty);
        let snap = Snapshot {
            dashboard: std::sync::Arc::new(crate::dashboard::fixtures()),
            ..Snapshot::default()
        };
        let _ = update(&mut app, Message::Snapshot(Box::new(snap)));
        assert!(!app.dashboard.is_empty);
        assert_eq!(app.dashboard.recent.len(), 6);
    }

    #[test]
    fn hover_da_legenda_tolera_saida_atrasada_e_some_ao_trocar_de_mes() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with_fixtures(dir.path());
        let _ = update(&mut app, Message::HoverCategory("C1".into()));
        let _ = update(&mut app, Message::HoverCategory("C2".into()));
        let _ = update(&mut app, Message::UnhoverCategory("C1".into()));
        assert_eq!(app.hover_category.as_deref(), Some("C2"));
        let _ = update(&mut app, Message::UnhoverCategory("C2".into()));
        assert_eq!(app.hover_category, None);
        let _ = update(&mut app, Message::HoverCategory("C2".into()));
        let _ = update(&mut app, Message::PrevMonth);
        assert_eq!(app.hover_category, None);
    }

    #[test]
    fn fotos_dos_perfis_por_user_id() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with_fixtures(dir.path());
        assert!(app.user_avatars.is_empty());
        let mut ds = (*app.snapshot.dashboard).clone();
        ds.apply([crate::dashboard::dataset::RawRow {
            table: "users".into(),
            id: "U1".into(),
            deleted_at: None,
            seq: 1000,
            data: serde_json::json!({
                "name": "Ana",
                "color": "fuchsia",
                "avatar": "data:image/png;base64,iVBORw0KGgo="
            })
            .to_string(),
        }]);
        let snap = Snapshot {
            dashboard: std::sync::Arc::new(ds),
            ..app.snapshot.clone()
        };
        let _ = update(&mut app, Message::Snapshot(Box::new(snap)));
        assert!(app.user_avatars.contains_key("U1"));
        assert!(!app.user_avatars.contains_key("U2"));
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
