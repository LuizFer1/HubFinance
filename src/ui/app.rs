//! Cola do iced: o estado que o `daemon` guarda (`State` = modelo + o que e do toolkit), o
//! `boot`, o `update` que converte efeitos em `Task`, as assinaturas e a `view`. A transicao de
//! estado em si mora em `ui_model::app` e nao sabe que o iced existe.

use std::cell::RefCell;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use iced::{Element, Subscription, Task, window};
use time::UtcOffset;
use tokio::sync::watch;

use super::effects;
use super::iced_side::{IcedSide, model_id};
use super::theme::Tokens;
use crate::config::UiPrefs;
use crate::desktop::{Desktop, EventReceiver};
use crate::hub::HubHandle;
use crate::hub::snapshot::{Snapshot, Status};
use crate::ui_model::app::App;
use crate::ui_model::effect::Effect;
use crate::ui_model::motion;
use crate::ui_model::window::Size;

pub(super) use crate::ui_model::analysis::awaiting_cloud_consent;
pub(super) use crate::ui_model::app::{
    AnalysisTab, CopyTarget, INITIAL_SIZE, Msg, RemoveStep, Screen,
};

/// O que o `daemon` do iced guarda: o modelo (sem tipo de toolkit) e o que so existe por causa
/// do iced. As telas leem os dois; `Deref` deixa `state.screen` e `state.snapshot` valerem como
/// no modelo, e `state.iced.tokens` e o que e do toolkit.
pub struct State {
    pub(super) model: App,
    pub(super) iced: IcedSide,
}

impl Deref for State {
    type Target = App;

    fn deref(&self) -> &App {
        &self.model
    }
}

impl DerefMut for State {
    fn deref_mut(&mut self) -> &mut App {
        &mut self.model
    }
}

impl State {
    pub(super) fn new(model: App) -> State {
        let tokens = Tokens::for_mode(model.theme, model.system_dark);
        let mut state = State {
            model,
            iced: IcedSide::new(tokens),
        };
        state.sync();
        state
    }

    /// Alinha o que e do iced com o modelo, depois de cada mensagem: o modelo decide, o toolkit
    /// so deriva (QR, texturas, tema, geometria dos graficos, respostas em Markdown, mapa de
    /// janelas). Idempotente: sem mudanca no modelo, nao refaz nada.
    pub(super) fn sync(&mut self) {
        let model = &self.model;
        let iced = &mut self.iced;
        iced.sync_qr(model.snapshot.pairing.as_ref().map(|p| p.qr_url.as_str()));
        iced.sync_device_textures(&model.avatars);
        iced.sync_user_textures(&model.user_avatars);
        let tokens = Tokens::for_mode(model.theme, model.system_dark);
        if !std::ptr::eq(tokens, iced.tokens) {
            iced.tokens = tokens;
            iced.iced_theme = tokens.iced_theme();
            // As cores dos graficos estao gravadas na geometria em cache.
            iced.clear_charts();
        }
        if iced.charts_seen != model.charts_epoch {
            iced.charts_seen = model.charts_epoch;
            iced.clear_charts();
        }
        iced.sync_replies(model);
        iced.forget_except(model.window);
    }
}

pub fn boot(
    handle: HubHandle,
    offset: UtcOffset,
    prefs: UiPrefs,
    prefs_dir: PathBuf,
    desktop: Rc<dyn Desktop>,
    events: EventReceiver,
    open_window: bool,
) -> impl Fn() -> (State, Task<Msg>) {
    // `BootFn` e `Fn`, e o receptor nao clona: o primeiro boot o leva; um segundo (presets de
    // teste do iced) nasce sem eventos da bandeja.
    let events = RefCell::new(Some(events));
    move || {
        let rx = handle.snapshot.clone();
        let mut state = State::new(App::new(
            handle.clone(),
            offset,
            prefs.clone(),
            prefs_dir.clone(),
            desktop.clone(),
        ));
        let mut tasks = vec![Task::run(snapshot_stream(rx), |s| {
            Msg::Snapshot(Box::new(s))
        })];
        if let Some(events) = events.borrow_mut().take() {
            tasks.push(Task::run(events, Msg::Desktop));
        }
        // Fora do Windows o registro nao responde: o tema do sistema vem do iced.
        tasks.push(iced::system::theme().map(system_theme));
        // Nucleo que ja falhou antes do boot (porta ocupada responde em milissegundos): o
        // primeiro `Snapshot` nao seria uma transicao para `Failed`, e um boot so com o icone
        // esconderia o erro.
        if open_window || matches!(state.snapshot.status, Status::Failed(_)) {
            tasks.push(effects::task(Effect::OpenWindow, &mut state));
        }
        (state, Task::batch(tasks))
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

/// O `update` do iced: o modelo descreve os efeitos e so aqui eles viram `Task`.
pub fn update(state: &mut State, message: Msg) -> Task<Msg> {
    // So da iced: o arrastar da borda e do sistema de janelas dela, nao e um `Effect`.
    if let Msg::Resize(edge) = message {
        return effects::resize(state, edge);
    }
    let effects = crate::ui_model::app::update(&mut state.model, message);
    // Convertido na hora (e em ordem): `OpenWindow` guarda o id da janela no estado.
    let task = Task::batch(effects.into_iter().map(|e| effects::task(e, state)));
    state.sync();
    task
}

/// Uma mensagem sem converter os efeitos em `Task`: o que os testes conferem (e mais
/// verificavel olhar o `Vec<Effect>` do que o `Task`).
#[cfg(test)]
pub(super) fn step(state: &mut State, message: Msg) -> Vec<Effect> {
    let effects = crate::ui_model::app::update(&mut state.model, message);
    state.sync();
    effects
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

/// Um instante a cada `BLINK`, enquanto a assinatura existir. Os prazos saem do inicio
/// (`inicio + k * BLINK`), nao de `sleep` encadeado: o atraso de cada volta nao se acumula e a
/// amostra nao escorrega ate a troca do cursor numa resposta longa.
fn blinks() -> futures::channel::mpsc::Receiver<Instant> {
    let (mut tx, rx) = futures::channel::mpsc::channel(1);
    std::thread::spawn(move || {
        let start = Instant::now();
        let mut k: u32 = 0;
        loop {
            k = k.saturating_add(1);
            let deadline = start + motion::BLINK * k;
            std::thread::sleep(deadline.saturating_duration_since(Instant::now()));
            // Cheio = a UI ainda nao consumiu o anterior; pular um nao importa. Desconectado =
            // a assinatura saiu (a resposta terminou): a thread acaba.
            if let Err(e) = tx.try_send(Instant::now())
                && e.is_disconnected()
            {
                break;
            }
        }
    });
    rx
}

pub fn subscription(state: &State) -> Subscription<Msg> {
    let mut subs = vec![
        window::close_requests().map(|_| Msg::CloseRequested),
        window::close_events().map(|id| Msg::WindowClosed(model_id(id))),
        window::resize_events().map(|(_, size)| Msg::Resized(Size::new(size.width, size.height))),
        Subscription::run(ticks).map(|()| Msg::Tick),
        super::keys::subscription(),
        iced::system::theme_changes().map(system_theme),
    ];
    // Quadros so enquanto algo anda: ligado o tempo todo, a janela redesenharia a 60 por
    // segundo parada.
    if state.motion.animating() {
        subs.push(window::frames().map(Msg::Frame));
    }
    // O cursor da resposta tem relogio proprio: quadros a toda hora durante uma resposta de
    // minutos seriam 60 redesenhos por segundo para piscar um caractere.
    if blink_running(state) {
        subs.push(Subscription::run(blinks).map(Msg::Frame));
    }
    Subscription::batch(subs)
}

/// O relogio do cursor so roda com janela: sem ela nao ha cursor para piscar, e o relogio
/// acordaria o laco de eventos a toda piscada durante uma resposta inteira.
fn blink_running(state: &State) -> bool {
    state.motion.blinking() && state.window.is_some()
}

/// O aviso de tema do iced como mensagem. `Mode::None` = o sistema nao tem preferencia: fica o
/// ultimo valor conhecido, entao nao vira mensagem de tema.
fn system_theme(mode: iced::theme::Mode) -> Msg {
    match mode {
        iced::theme::Mode::Dark => Msg::SystemTheme(true),
        iced::theme::Mode::Light => Msg::SystemTheme(false),
        iced::theme::Mode::None => Msg::Noop,
    }
}

pub fn view(state: &State, window: window::Id) -> Element<'_, Msg> {
    super::view::view(state, window)
}

#[cfg(test)]
pub(super) mod tests {
    use tokio::sync::{mpsc, watch};

    use super::super::effects::save_prefs;
    use super::super::remove_view;
    use super::*;
    use crate::assistant::settings::{ProviderChoice, Settings};
    use crate::assistant::view::MessageRole;
    use crate::config::ThemeMode;
    use crate::dashboard::contract::Book;
    use crate::dashboard::money::{MASK, leaks};
    use crate::desktop::{DesktopEvent, Fake, Headless};
    use crate::hub::snapshot::DeviceView;
    use crate::hub::{AssistantCommand, Command};
    use crate::store::erase::{AreaSet, DeviceData};
    use crate::ui_model::app::DELETE_ALL_ARMED_FOR;
    use crate::ui_model::app::tests as m;
    use crate::ui_model::app::tests::wait_prefs;
    use crate::ui_model::app::tests::{
        awaiting_conv, configured, data, drain, local_settings, msg, open_conv, phone,
        running_with, summary, view as device_view,
    };
    use crate::ui_model::connect::{self, ConnectFlow};
    use crate::ui_model::motion::{self, Scene};

    type Fixture = (
        State,
        mpsc::UnboundedReceiver<Command>,
        watch::Sender<Snapshot>,
    );

    fn registered(mut app: State) -> State {
        app.window = Some(app.iced.register(window::Id::unique()));
        app
    }

    pub(crate) fn app_with_desktop(
        snapshot: Snapshot,
        dir: PathBuf,
        desktop: Rc<dyn Desktop>,
    ) -> Fixture {
        let (a, rx, tx) = m::app_with_desktop(snapshot, dir, desktop);
        (State::new(a), rx, tx)
    }

    pub(crate) fn app_with(snapshot: Snapshot, dir: PathBuf) -> Fixture {
        let (a, rx, tx) = m::app_with(snapshot, dir);
        (State::new(a), rx, tx)
    }

    pub(crate) fn with_tray(
        dir: &std::path::Path,
    ) -> (State, Rc<Fake>, mpsc::UnboundedReceiver<Command>) {
        let (a, fake, rx) = m::with_tray(dir);
        let mut state = State::new(a);
        // O modelo ja tem janela; o iced precisa do id dele para os efeitos de janela.
        state.window = None;
        (registered(state), fake, rx)
    }

    pub(crate) fn app_with_fixtures(dir: &std::path::Path) -> Fixture {
        let (a, rx, tx) = m::app_with_fixtures(dir);
        (State::new(a), rx, tx)
    }

    pub(crate) fn app_with_reserves(dir: &std::path::Path) -> Fixture {
        let (a, rx, tx) = m::app_with_reserves(dir);
        (State::new(a), rx, tx)
    }

    pub(crate) fn app_with_data(dir: &std::path::Path, devices: Vec<DeviceView>) -> Fixture {
        let (a, rx, tx) = m::app_with_data(dir, devices);
        (State::new(a), rx, tx)
    }

    pub(crate) fn app_with_business(
        dir: &std::path::Path,
        with_book: bool,
        book_deleted: bool,
    ) -> (State, watch::Sender<Snapshot>) {
        let (a, tx) = m::app_with_business(dir, with_book, book_deleted);
        (State::new(a), tx)
    }

    pub(crate) fn assistant_app(
        dir: &std::path::Path,
        settings: crate::assistant::settings::Settings,
    ) -> (State, mpsc::UnboundedReceiver<Command>) {
        let (a, rx) = m::assistant_app(dir, settings);
        (State::new(a), rx)
    }

    pub(crate) fn cloud_app(dir: &std::path::Path) -> (State, mpsc::UnboundedReceiver<Command>) {
        let (a, rx) = m::cloud_app(dir);
        (State::new(a), rx)
    }

    #[test]
    fn nasce_no_dashboard_com_o_tema_das_preferencias() {
        let dir = tempfile::tempdir().unwrap();
        let (cmd_tx, _rx) = mpsc::unbounded_channel();
        let (_tx, snap_rx) = watch::channel(Snapshot::default());
        let app = State::new(App::new(
            HubHandle {
                commands: cmd_tx,
                snapshot: snap_rx,
            },
            UtcOffset::UTC,
            UiPrefs {
                theme: ThemeMode::Light,
                ..UiPrefs::default()
            },
            dir.path().to_path_buf(),
            Rc::new(Headless),
        ));
        assert_eq!(app.screen, Screen::Dashboard);
        assert_eq!(app.theme, ThemeMode::Light);
        assert!(std::ptr::eq(app.iced.tokens, &super::super::theme::LIGHT));
    }

    #[test]
    fn nasce_com_o_tema_do_sistema_ou_o_da_ultima_gravacao() {
        let dir = tempfile::tempdir().unwrap();
        let boot = |system: Option<bool>, saved_dark: bool| {
            let fake = Fake::with_tray();
            fake.system_dark.set(system);
            let (cmd_tx, _rx) = mpsc::unbounded_channel();
            let (_tx, snap_rx) = watch::channel(Snapshot::default());
            State::new(App::new(
                HubHandle {
                    commands: cmd_tx,
                    snapshot: snap_rx,
                },
                UtcOffset::UTC,
                UiPrefs {
                    theme: ThemeMode::System,
                    system_dark: saved_dark,
                    ..UiPrefs::default()
                },
                dir.path().to_path_buf(),
                Rc::new(fake),
            ))
        };
        let light = &super::super::theme::LIGHT;
        let dark = &super::super::theme::DARK;
        // O sistema manda sobre o que foi gravado.
        assert!(std::ptr::eq(boot(Some(false), true).iced.tokens, light));
        assert!(std::ptr::eq(boot(Some(true), false).iced.tokens, dark));
        // Sem resposta, vale o modo resolvido da ultima gravacao.
        assert!(std::ptr::eq(boot(None, true).iced.tokens, dark));
        assert!(std::ptr::eq(boot(None, false).iced.tokens, light));
    }

    #[test]
    fn dialogo_liga_os_quadros_so_durante_a_entrada() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with_fixtures(dir.path());
        // Com janela: sem ela as animacoes terminam na hora.
        app.window = Some(app.iced.register(window::Id::unique()));
        assert!(!app.motion.animating(), "parada, sem quadros");
        let _ = update(&mut app, Msg::ShowEnvelope);
        assert!(app.any_dialog_open());
        assert!(app.motion.animating());
        let _ = update(&mut app, Msg::Frame(Instant::now() + motion::SCRIM_IN));
        assert!(!app.motion.animating());
        assert_eq!(app.motion.scrim(), 1.0);
    }

    #[test]
    fn trocar_de_tela() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        // Toda tela da navegacao, Ajustes inclusive; cada troca volta a rolagem ao topo (a
        // tarefa do `snap_to`), porque o cabecalho fixo nao rola e so o conteudo tem posicao.
        for screen in super::super::shell::nav_entries(&app).map(|e| e.screen) {
            let effects = step(&mut app, Msg::SelectScreen(screen));
            assert_eq!(app.screen, screen);
            assert!(
                matches!(effects.as_slice(), [Effect::ScrollToTop]),
                "{screen:?}"
            );
        }
        assert_eq!(app.screen, Screen::Settings);
    }

    #[test]
    fn configurar_o_assistente_abre_o_dialogo_sem_trocar_de_tela() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        let _ = update(&mut app, Msg::SelectScreen(Screen::Settings));
        assert_eq!(app.screen, Screen::Settings);
        let _ = update(&mut app, Msg::ConfigureAssistant);
        assert_eq!(app.screen, Screen::Settings);
        assert!(app.assistant_setup.is_some());
        assert!(super::super::assistant_settings_view::dialog(&app).is_some());
        let _ = super::super::shell::root(&app);
        // Ja aberto: nao refaz os rascunhos que a pessoa digitou.
        let _ = update(&mut app, Msg::SetupPort("1234".into()));
        let _ = update(&mut app, Msg::ConfigureAssistant);
        assert_eq!(
            app.assistant_setup
                .as_ref()
                .map(|s| s.draft.port_input.as_str()),
            Some("1234")
        );
    }

    #[test]
    fn trocar_o_tema_troca_os_tokens_e_grava_ui_json() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        let _ = update(&mut app, Msg::SetTheme(ThemeMode::Light));
        assert_eq!(app.theme, ThemeMode::Light);
        assert!(std::ptr::eq(app.iced.tokens, &super::super::theme::LIGHT));
        // A gravacao corre numa thread; espera com prazo.
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while UiPrefs::load(dir.path()).theme != ThemeMode::Light {
            assert!(std::time::Instant::now() < deadline, "ui.json nao gravado");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn tema_sistema_segue_o_sistema_sem_gravar_a_troca() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, fake, _rx) = with_tray(dir.path());
        fake.system_dark.set(Some(false));
        let _ = update(&mut app, Msg::SetTheme(ThemeMode::System));
        assert_eq!(app.theme, ThemeMode::System);
        // Escolher "Sistema" le o sistema na hora: claro, sem esperar o tique.
        assert!(std::ptr::eq(app.iced.tokens, &super::super::theme::LIGHT));
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while UiPrefs::load(dir.path()).theme != ThemeMode::System {
            assert!(std::time::Instant::now() < deadline, "ui.json nao gravado");
            std::thread::sleep(Duration::from_millis(10));
        }

        // O sistema vai para o escuro: o tique percebe (caminho do Windows).
        fake.system_dark.set(Some(true));
        let _ = update(&mut app, Msg::Tick);
        assert!(std::ptr::eq(app.iced.tokens, &super::super::theme::DARK));
        // Com o registro respondendo, o aviso do iced e ignorado: ele chega velho no Windows.
        let _ = update(&mut app, Msg::SystemTheme(false));
        assert!(std::ptr::eq(app.iced.tokens, &super::super::theme::DARK));
        // Sem registro (Linux), o aviso do iced troca; `None` nao mexe.
        fake.system_dark.set(None);
        let _ = update(&mut app, Msg::SystemTheme(false));
        assert!(std::ptr::eq(app.iced.tokens, &super::super::theme::LIGHT));
        let _ = update(&mut app, system_theme(iced::theme::Mode::None));
        let _ = update(&mut app, Msg::Tick);
        assert!(std::ptr::eq(app.iced.tokens, &super::super::theme::LIGHT));
        assert_eq!(UiPrefs::load(dir.path()).theme, ThemeMode::System);
    }

    #[test]
    fn tema_fixo_ignora_o_sistema() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, fake, _rx) = with_tray(dir.path());
        fake.system_dark.set(Some(false));
        assert_eq!(app.theme, ThemeMode::Dark);
        let _ = update(&mut app, Msg::Tick);
        let _ = update(&mut app, Msg::SystemTheme(false));
        assert!(std::ptr::eq(app.iced.tokens, &super::super::theme::DARK));
        // Voltar a "Sistema" usa o que o sistema disse por ultimo.
        let _ = update(&mut app, Msg::SetTheme(ThemeMode::System));
        assert!(std::ptr::eq(app.iced.tokens, &super::super::theme::LIGHT));
    }

    #[test]
    fn fallback_das_releases_desenha_e_copia_a_url() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        let _ = super::super::connection_view::view(&app);
        app.releases_open_failed = true;
        let _ = super::super::connection_view::view(&app);
        let _ = update(&mut app, Msg::Copy(CopyTarget::ReleasesUrl));
        assert!(app.is_copied(CopyTarget::ReleasesUrl));
    }

    #[test]
    fn apagar_tudo_marca_as_quatro_e_a_confirmacao_mostra_as_contagens() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with_data(
            dir.path(),
            vec![device_view("D1", "Pixel 7", false, false, data())],
        );
        let _ = update(&mut app, Msg::AskRemove("D1".into()));
        let _ = update(&mut app, Msg::SelectAllAreas);
        assert_eq!(
            app.confirm_remove.as_ref().unwrap().selected,
            AreaSet::all()
        );
        let _ = update(&mut app, Msg::RemoveContinue);
        // O passo 2 desenha sem panico e os textos trazem as contagens.
        let _ = remove_view::dialog(&app).expect("modal aberto");
        let selected = app.confirm_remove.as_ref().unwrap().selected;
        let text = crate::ui_model::remove::confirm_text(&data(), selected);
        assert!(text.contains("3 metas"), "{text}");
        assert!(text.contains("100 lançamentos"), "{text}");
        assert!(text.contains("2 reservas com 14 movimentos"), "{text}");
        assert!(text.contains("1 recorrência com 2 reajustes"), "{text}");
        assert_eq!(
            crate::ui_model::remove::confirm_label(&data(), selected, false),
            "Remover e apagar 122 itens"
        );
        let _ = update(&mut app, Msg::ConfirmRemove);
        assert!(
            app.pending_removal
                .as_ref()
                .unwrap()
                .toast
                .ends_with("122 itens serão apagados")
        );
        assert!(drain(&mut rx).is_empty(), "nada sai antes do prazo");
        app.pending_removal.as_mut().unwrap().deadline = Instant::now();
        let _ = update(&mut app, Msg::Tick);
        assert_eq!(
            drain(&mut rx),
            vec![Command::RemoveDevice {
                device_id: "D1".into(),
                erase: AreaSet::all()
            }]
        );
    }

    #[test]
    fn aviso_so_para_removido_com_dados_sem_decisao() {
        let dir = tempfile::tempdir().unwrap();
        let devices = vec![
            device_view("ATIVO", "Ativo", false, false, data()),
            device_view("ORFAO", "Orfao", true, false, data()),
            device_view("MANTIDO", "Mantido", true, true, data()),
            device_view("VAZIO", "Vazio", true, false, DeviceData::default()),
        ];
        let (mut app, _rx, _tx) = app_with_data(dir.path(), devices);
        let ids: Vec<_> = app.orphans().iter().map(|d| d.device_id.clone()).collect();
        assert_eq!(ids, vec!["ORFAO"]);
        assert!(remove_view::orphan_banner(&app).is_some());
        // Banco sem revogados: nada.
        let (app2, _rx2, _tx2) = app_with_data(
            dir.path(),
            vec![device_view("ATIVO", "Ativo", false, false, data())],
        );
        assert!(app2.orphans().is_empty());
        assert!(remove_view::orphan_banner(&app2).is_none());
        // "Depois" esconde nesta sessao e nao grava nada.
        let _ = update(&mut app, Msg::PostponeOrphans);
        assert!(remove_view::orphan_banner(&app).is_none());
        assert_eq!(app.orphans().len(), 1);
    }

    #[test]
    fn depois_nao_envia_comando_e_o_aviso_volta_na_reabertura() {
        let dir = tempfile::tempdir().unwrap();
        let orphan = device_view("ORFAO", "Orfao", true, false, data());
        let (mut app, mut rx, _tx) = app_with_data(dir.path(), vec![orphan.clone()]);
        let _ = update(&mut app, Msg::PostponeOrphans);
        assert!(drain(&mut rx).is_empty());
        // Nova abertura: estado novo, mesmo banco.
        let (app2, _rx2, _tx2) = app_with_data(dir.path(), vec![orphan]);
        assert!(remove_view::orphan_banner(&app2).is_some());
    }

    #[test]
    fn decidir_manter_tudo_de_removido_envia_keep_na_hora() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with_data(
            dir.path(),
            vec![device_view("ORFAO", "Orfao", true, false, data())],
        );
        let _ = update(&mut app, Msg::DecideOrphan);
        assert_eq!(
            app.confirm_remove.as_ref().map(|d| d.device_id.as_str()),
            Some("ORFAO")
        );
        // Com o modal aberto o aviso some.
        assert!(remove_view::orphan_banner(&app).is_none());
        let _ = remove_view::dialog(&app).expect("modal aberto");
        let _ = update(&mut app, Msg::RemoveContinue);
        assert_eq!(
            drain(&mut rx),
            vec![Command::KeepDeviceData("ORFAO".into())]
        );
        assert_eq!(app.confirm_remove, None);
        assert_eq!(app.pending_removal, None);
    }

    /// Monta a arvore do Dashboard nos estados da spec 7 e nas larguras dos dois lados de 1100:
    /// pega indice fora da lista, divisao por zero em layout e secao esquecida sem abrir janela.
    fn view_all_widths(app: &mut State) {
        for content in [1400.0, 1100.0, 1099.0, 900.0, 700.0] {
            app.window_size = Size::new(content + 232.0 + 64.0, 900.0);
            let _ = super::super::dashboard_view::view(app);
        }
    }

    #[test]
    fn dashboard_monta_em_todos_os_estados_e_larguras() {
        use crate::dashboard::grafite::{HistoryBody, HistoryChart, HistoryColumn, Section};
        let dir = tempfile::tempdir().unwrap();
        let snap = Snapshot {
            dashboard: std::sync::Arc::new(crate::dashboard::fixtures_grafite()),
            ..Snapshot::default()
        };
        let (mut app, _rx, _tx) = app_with(snap, dir.path().to_path_buf());
        app.set_today("2026-11-19".into());
        assert!(app.dashboard.sections().contains(&Section::WhatsNext));
        view_all_widths(&mut app);
        let _ = update(&mut app, Msg::ExpandUpcoming);
        view_all_widths(&mut app);
        let _ = update(&mut app, Msg::ToggleHideValues);
        assert!(app.dashboard.hidden);
        view_all_widths(&mut app);
        // Mes fechado: sem "O que vem".
        let _ = update(&mut app, Msg::PrevMonth);
        assert!(!app.dashboard.sections().contains(&Section::WhatsNext));
        view_all_widths(&mut app);
        // Doze meses em grafico (o modelo decide a partir do 4o mes).
        app.dashboard.history.body = HistoryBody::Chart(HistoryChart {
            columns: (0..12)
                .map(|i| HistoryColumn {
                    label: "out".into(),
                    existed: i > 3,
                    partial: i == 11,
                    income: 0.8,
                    expense: 0.5,
                    rate: (i > 3).then_some(0.4),
                    tooltip: (i > 3).then(|| "out 2026 · receitas R$ •••".into()),
                })
                .collect(),
            ..HistoryChart::default()
        });
        view_all_widths(&mut app);
        // Hub sem dados.
        let (mut empty, _rx2, _tx2) = app_with(Snapshot::default(), dir.path().to_path_buf());
        empty.set_today("2026-11-19".into());
        assert_eq!(empty.dashboard.sections(), vec![Section::Empty]);
        view_all_widths(&mut empty);
    }

    #[test]
    fn com_bandeja_fechar_fecha_a_janela_e_nao_desliga() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _fake, mut rx) = with_tray(dir.path());
        let id = app.window.unwrap();
        let _ = update(&mut app, Msg::CloseRequested);
        assert!(!app.closing);
        assert!(
            rx.try_recv().is_err(),
            "nenhum comando, muito menos Shutdown"
        );
        // A janela so some quando o iced confirma (`close_events`).
        assert_eq!(app.window, Some(id));
        let _ = update(&mut app, Msg::WindowClosed(model_id(window::Id::unique())));
        assert_eq!(app.window, Some(id), "outra janela nao e a nossa");
        app.maximized = true;
        app.window_size = Size::new(800.0, 600.0);
        let _ = update(&mut app, Msg::WindowClosed(id));
        assert_eq!(app.window, None);
        assert!(!app.maximized);
        assert_eq!(app.window_size, INITIAL_SIZE);
    }

    #[test]
    fn com_bandeja_fechar_envia_a_remocao_pendente_sem_shutdown() {
        let dir = tempfile::tempdir().unwrap();
        let fake = Rc::new(Fake::with_tray());
        let (mut app, mut rx, _tx) = app_with_desktop(
            Snapshot {
                devices: vec![device_view("D1", "Pixel 7", false, false, data())],
                ..Snapshot::default()
            },
            dir.path().to_path_buf(),
            fake,
        );
        app.window = Some(app.iced.register(window::Id::unique()));
        let _ = update(&mut app, Msg::AskRemove("D1".into()));
        let _ = update(&mut app, Msg::ConfirmRemove);
        assert!(app.pending_removal.is_some());
        let _ = update(&mut app, Msg::CloseRequested);
        assert!(matches!(
            rx.try_recv(),
            Ok(Command::RemoveDevice { device_id, .. }) if device_id == "D1"
        ));
        assert!(rx.try_recv().is_err());
        assert_eq!(app.pending_removal, None);
        assert!(!app.closing);
    }

    #[test]
    fn boot_abre_a_janela_so_quando_pedido() {
        let dir = tempfile::tempdir().unwrap();
        for open in [true, false] {
            let (cmd_tx, _rx) = mpsc::unbounded_channel();
            let (_tx, snap_rx) = watch::channel(Snapshot::default());
            let handle = HubHandle {
                commands: cmd_tx,
                snapshot: snap_rx,
            };
            let (_etx, erx) = crate::desktop::channel();
            let (app, _task) = boot(
                handle,
                UtcOffset::UTC,
                UiPrefs::default(),
                dir.path().to_path_buf(),
                Rc::new(Fake::with_tray()),
                erx,
                open,
            )();
            assert_eq!(app.window.is_some(), open);
        }
    }

    #[test]
    fn boot_so_com_icone_abre_a_janela_se_o_nucleo_ja_falhou() {
        let dir = tempfile::tempdir().unwrap();
        let (cmd_tx, _rx) = mpsc::unbounded_channel();
        let (_tx, snap_rx) = watch::channel(Snapshot {
            status: Status::Failed("porta ocupada".into()),
            ..Snapshot::default()
        });
        let handle = HubHandle {
            commands: cmd_tx,
            snapshot: snap_rx,
        };
        let (_etx, erx) = crate::desktop::channel();
        let (app, _task) = boot(
            handle,
            UtcOffset::UTC,
            UiPrefs::default(),
            dir.path().to_path_buf(),
            Rc::new(Fake::with_tray()),
            erx,
            false,
        )();
        assert!(app.window.is_some());
    }

    #[test]
    fn sem_a_opcao_o_card_desenha_sem_ela() {
        let dir = tempfile::tempdir().unwrap();
        let (app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        assert_eq!(app.autostart, None);
        let _ = super::super::connection_view::view(&app);
        let (mut app, _fake, _rx) = with_tray(dir.path());
        assert_eq!(app.autostart, Some(false));
        let _ = super::super::connection_view::view(&app);
        app.autostart_failed = true;
        let _ = super::super::connection_view::view(&app);
    }

    #[test]
    fn gravacao_antiga_que_chega_depois_nao_sobrescreve_a_nova() {
        let dir = tempfile::tempdir().unwrap();
        let first = save_prefs(
            UiPrefs {
                theme: ThemeMode::Dark,
                ..UiPrefs::default()
            },
            dir.path().to_path_buf(),
        );
        let second = save_prefs(
            UiPrefs {
                theme: ThemeMode::Light,
                ..UiPrefs::default()
            },
            dir.path().to_path_buf(),
        );
        futures::executor::block_on(async {
            let _ = second.await;
            let _ = first.await;
        });
        assert_eq!(UiPrefs::load(dir.path()).theme, ThemeMode::Light);
    }

    #[test]
    fn seletor_de_conta_so_existe_com_linha_books_viva() {
        let dir = tempfile::tempdir().unwrap();
        for (with_book, deleted, visible) in [
            (false, false, false),
            (true, true, false),
            (true, false, true),
        ] {
            let (app, _tx) = app_with_business(dir.path(), with_book, deleted);
            assert_eq!(
                super::super::shell::book_switch(&app).is_some(),
                visible,
                "books={with_book} apagada={deleted}"
            );
        }
    }

    #[test]
    fn abre_na_pessoal_sem_misturar_o_comercial() {
        let dir = tempfile::tempdir().unwrap();
        let (app, _tx) = app_with_business(dir.path(), true, false);
        assert_eq!(app.book, Book::Personal);
        // As fixtures tem 7 lancamentos; o comercial nao entra na contagem nem na lista.
        assert_eq!(app.transactions.list.count, 7);
        assert_eq!(
            super::super::shell::transactions_count(&app),
            crate::dashboard::fixtures().alive_transactions().count()
        );
        let plain = crate::dashboard::grafite::build_grafite(
            &crate::dashboard::fixtures(),
            "2026-09-24",
            "2026-09",
            Book::Personal,
            false,
        );
        assert_eq!(app.dashboard.how_am_i, plain.how_am_i);
    }

    #[test]
    fn escolher_comercial_mostra_so_a_conta_comercial_e_volta() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _tx) = app_with_business(dir.path(), true, false);
        let _ = update(&mut app, Msg::SelectBook(Book::Business));
        assert_eq!(app.book, Book::Business);
        assert_eq!(app.dashboard.how_am_i.income.value, "R$ 900,00");
        assert_eq!(app.dashboard.how_am_i.expense.value, "R$ 0,00");
        assert_eq!(app.transactions.list.count, 1);
        assert_eq!(super::super::shell::transactions_count(&app), 1);
        assert_eq!(
            crate::ui_model::shell::titles(&app).1,
            "Padaria · somente leitura · dados do último sync"
        );
        let _ = update(&mut app, Msg::SelectBook(Book::Personal));
        assert_eq!(app.book, Book::Personal);
        assert_eq!(app.transactions.list.count, 7);
    }

    #[test]
    fn dashboard_comercial_monta_com_o_negocio_primeiro() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _tx) = app_with_business(dir.path(), true, false);
        let _ = update(&mut app, Msg::SelectBook(Book::Business));
        view_all_widths(&mut app);
    }

    #[test]
    fn tela_analise_monta_com_e_sem_achados_e_com_valores_ocultos() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with_fixtures(dir.path());
        let _ = update(&mut app, Msg::SelectScreen(Screen::Analysis));
        let _ = super::super::analysis_view::view(&app);
        let _ = update(&mut app, Msg::ToggleHideValues);
        assert!(app.hide_values);
        let _ = super::super::analysis_view::view(&app);
        app.analysis = crate::advisor::Analysis::default();
        let _ = super::super::analysis_view::view(&app);
    }

    #[test]
    fn previa_mostra_exatamente_o_render_do_envelope() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with_fixtures(dir.path());
        let _ = update(&mut app, Msg::SelectScreen(Screen::Analysis));
        assert!(app.envelope_preview.is_none());
        let _ = update(&mut app, Msg::ShowEnvelope);
        let expected = crate::assistant::envelope::render(&crate::assistant::envelope::build(
            &app.analysis,
            &app.scope,
            &app.today,
            app.book,
        ));
        assert_eq!(app.envelope_preview.as_deref(), Some(expected.as_str()));
        assert_eq!(
            app.envelope_preview.as_deref(),
            Some(app.current_envelope().as_str())
        );
        assert!(
            expected.contains("R$ "),
            "o exemplo traz dinheiro no envelope"
        );
        // Aberta, a prévia monta (sobreposição) e fecha.
        assert!(super::super::analysis_view::envelope_dialog(&app).is_some());
        let _ = update(&mut app, Msg::CloseEnvelope);
        assert!(app.envelope_preview.is_none());
        assert!(super::super::analysis_view::envelope_dialog(&app).is_none());
    }

    #[test]
    fn previa_aberta_monta_pela_raiz_em_janelas_grandes_e_pequenas() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with_fixtures(dir.path());
        let _ = update(&mut app, Msg::SelectScreen(Screen::Analysis));
        let _ = update(&mut app, Msg::ShowEnvelope);
        for hidden in [false, true] {
            app.hide_values = hidden;
            for (w, h) in [
                (1400.0, 900.0),
                (900.0, 600.0),
                (420.0, 300.0),
                (120.0, 80.0),
            ] {
                app.window_size = Size::new(w, h);
                let _ = super::super::shell::root(&app);
            }
        }
    }

    #[test]
    fn seletor_de_conta_aparece_na_analise_e_troca_os_achados() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _tx) = app_with_business(dir.path(), true, false);
        let _ = update(&mut app, Msg::SelectScreen(Screen::Analysis));
        assert!(super::super::shell::book_switch(&app).is_some());
        let _ = update(&mut app, Msg::SelectBook(Book::Business));
        assert_eq!(
            app.analysis,
            crate::advisor::analyze(&app.scope, &app.today)
        );
    }

    /// So a montagem da view em dois passos; a navegacao e a gravacao no ultimo botao vivem em
    /// `ui_model::app` (`dialogo_anda_pelos_passos_e_so_grava_no_ultimo_botao`).
    #[test]
    fn dialogo_do_assistente_monta_nos_passos() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx) = assistant_app(dir.path(), Settings::default());
        let _ = update(&mut app, Msg::ConfigureAssistant);
        let _ = update(&mut app, Msg::SetupChoice(ProviderChoice::Local));
        let _ = super::super::shell::root(&app);
        let _ = update(&mut app, Msg::SetupNext);
        app.snapshot.assistant.listing = true;
        let _ = super::super::shell::root(&app);
        app.snapshot.assistant.listing = false;
        app.snapshot.assistant.models = Some(Ok(vec!["llama3.1:8b".into()]));
        let _ = update(&mut app, Msg::SetupLocalModel("llama3.1:8b".into()));
        let _ = super::super::shell::root(&app);
    }

    /// So a montagem; o `Configure` enviado e o "Salvando" que some vivem em `ui_model::app`
    /// (`erro_ao_gravar_fica_no_dialogo_e_no_banner_da_analise`).
    #[test]
    fn erro_ao_gravar_monta_no_dialogo_e_no_banner_da_analise() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx) = assistant_app(dir.path(), local_settings(Some("m")));
        let _ = update(&mut app, Msg::ConfigureAssistant);
        let _ = update(&mut app, Msg::SetupChoice(ProviderChoice::None));
        let _ = update(&mut app, Msg::SetupFinish);
        let failed = configured(
            &app,
            None,
            Some("Não foi possível salvar a configuração do assistente."),
        );
        let _ = update(&mut app, failed);
        assert!(super::super::assistant_settings_view::dialog(&app).is_some());
        let _ = super::super::analysis_view::view(&app);
        let _ = super::super::shell::root(&app);
    }

    /// So a montagem da view: a chave limpa na hora e o comando vivem em
    /// `ui_model::app` (`nuvem_leva_a_chave_digitada_no_mesmo_comando_e_a_limpa_na_hora`).
    #[test]
    fn nuvem_monta() {
        use crate::assistant::settings::ProviderChoice;
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx) = assistant_app(dir.path(), Settings::default());
        let _ = update(&mut app, Msg::ConfigureAssistant);
        let _ = update(&mut app, Msg::SetupChoice(ProviderChoice::Cloud));
        let _ = update(&mut app, Msg::SetupNext);
        let _ = super::super::shell::root(&app);
    }

    /// So a montagem da view: o "Salvando" que nao fica preso vive em `ui_model::app`
    /// (`hub_fora_do_ar_nao_deixa_o_salvando_preso`).
    #[test]
    fn hub_fora_do_ar_monta() {
        use crate::assistant::settings::ProviderChoice;
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx) = assistant_app(dir.path(), local_settings(Some("m")));
        let _ = update(&mut app, Msg::ConfigureAssistant);
        let _ = update(&mut app, Msg::SetupChoice(ProviderChoice::None));
        let _ = update(&mut app, Msg::SetupFinish);
        let mut down = app.snapshot.clone();
        down.status = Status::Failed("porta ocupada".into());
        let _ = update(&mut app, Msg::Snapshot(Box::new(down)));
        let _ = super::super::shell::root(&app);
    }

    #[test]
    fn dialogos_do_assistente_montam_em_todos_os_estados() {
        use crate::ui_model::assistant_setup::SetupStep;
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx) = cloud_app(dir.path());
        let _ = update(&mut app, Msg::ConfigureAssistant);
        let models = [
            None,
            Some(Err("Não encontrei o Ollama.".to_string())),
            Some(Ok(vec![])),
            Some(Ok(vec!["llama3.1:8b".to_string(), "qwen3:4b".to_string()])),
            Some(Ok((0..9).map(|i| format!("modelo-{i}")).collect())),
        ];
        for choice in [
            ProviderChoice::None,
            ProviderChoice::Local,
            ProviderChoice::Cloud,
        ] {
            for step in [SetupStep::Where, SetupStep::Details] {
                for listed in &models {
                    for (has_key, deny, listing) in [
                        (false, false, false),
                        (true, true, true),
                        (false, true, false),
                    ] {
                        if let Some(s) = app.assistant_setup.as_mut() {
                            s.draft.choice = choice;
                            s.step = step;
                            s.draft.deny = deny;
                            s.saving = listing.then_some(1);
                        }
                        app.snapshot.assistant.models = listed.clone();
                        app.snapshot.assistant.models_port = Some(11434);
                        app.snapshot.assistant.listing = listing;
                        app.snapshot.assistant.has_key = has_key;
                        app.snapshot.assistant.key_encrypted = deny;
                        app.snapshot.assistant.last_error =
                            listing.then(|| "Falha ao gravar".to_string());
                        app.key_input = if has_key { String::new() } else { "sk".into() };
                        for (w, h) in [(1400.0, 900.0), (1024.0, 720.0), (300.0, 200.0)] {
                            app.window_size = Size::new(w, h);
                            let _ = super::super::shell::root(&app);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn previa_mostra_o_envelope_da_propria_conversa_e_mascara_so_na_tela() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx) = cloud_app(dir.path());
        // O envelope guardado na conversa (nao o de agora) e o que a previa mostra.
        let env = "Gasto R$ 1.234,56 em mercado.";
        app.snapshot.assistant.open = Some(awaiting_conv(app.book, env));
        app.hide_values = false;
        let open = super::awaiting_cloud_consent(&app).unwrap();
        let shown = super::super::analysis_view::preview_shown(&app, open);
        assert!(shown.starts_with(env), "{shown}");
        assert_eq!(open.envelope, env, "o que sai nunca e mascarado");
        app.hide_values = true;
        let open = super::awaiting_cloud_consent(&app).unwrap();
        let shown = super::super::analysis_view::preview_shown(&app, open);
        assert!(!shown.contains("1.234"), "{shown}");
        assert_eq!(open.envelope, env);
        let _ = super::super::analysis_view::view(&app);
    }

    #[test]
    fn previa_mostra_o_mesmo_que_vai_no_pedido_incluindo_as_mensagens_anteriores() {
        use crate::assistant::prompt::{MAX_TURNS, Role};
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx) = cloud_app(dir.path());
        // Conversa respondida no modelo local e agora na nuvem: 12 pares e a pergunta pendente.
        let mut conv = awaiting_conv(app.book, "ENV DA CONVERSA");
        conv.messages.clear();
        for i in 0..12 {
            conv.messages
                .push(msg(MessageRole::User, &format!("pergunta-{i}")));
            conv.messages
                .push(msg(MessageRole::Assistant, &format!("resposta-{i}")));
        }
        conv.messages
            .push(msg(MessageRole::Error, "falha-que-nao-vai"));
        conv.messages
            .push(msg(MessageRole::User, "pergunta-pendente"));
        app.snapshot.assistant.open = Some(conv);
        let open = super::awaiting_cloud_consent(&app).unwrap();
        let body = crate::ui_model::analysis::preview_body(open);
        let payload = open.payload();
        // A janela corta o comeco (e avanca ate uma pergunta): nem tudo cabe.
        assert!(payload.len() > 2 && payload.len() <= 1 + MAX_TURNS * 2);
        assert!(body.starts_with("ENV DA CONVERSA"));
        // Cada turno do pedido aparece na previa, na mesma ordem e com o mesmo texto.
        let mut from = 0;
        for turn in payload.iter().skip(1) {
            let who = if turn.role == Role::Assistant {
                "Assistente"
            } else {
                "Você"
            };
            let line = format!("{who}: {}", turn.content);
            let at = body[from..].find(&line).unwrap_or_else(|| panic!("{line}"));
            from += at + line.len();
        }
        // O que a janela corta e o erro (que nunca vai ao modelo) tambem nao aparecem.
        assert!(!body.contains("pergunta-0\n") && !body.contains("pergunta-0:"));
        assert!(!body.contains("falha-que-nao-vai"));
        assert!(body.ends_with("Você: pergunta-pendente"));
        // Com mensagens anteriores a introducao deixa de falar so da pergunta de abertura.
        assert!(!crate::ui_model::analysis::PREVIEW_INTRO_EARLIER.contains("pergunta de abertura"));
        let _ = super::super::analysis_view::view(&app);
    }

    #[test]
    fn previa_abre_em_dialogo_ao_conversar_na_nuvem_e_mostra_o_payload() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx) = cloud_app(dir.path());
        let _ = update(&mut app, Msg::StartConversation);
        let _ = drain(&mut rx);
        assert!(app.consent_preview.is_none());
        // O snapshot com a conversa nova esperando aprovacao abre a previa.
        let snap = {
            let mut s = app.snapshot.clone();
            s.assistant.open = Some(awaiting_conv(app.book, "ENV DA CONVERSA"));
            Msg::Snapshot(Box::new(s))
        };
        let _ = update(&mut app, snap);
        assert_eq!(app.consent_preview.as_deref(), Some("C1"));
        let open = crate::ui_model::analysis::shown_consent(&app).expect("previa aberta");
        // O texto do dialogo e o do payload (a mesma montagem do envio).
        let body = crate::ui_model::analysis::preview_body(open);
        for turn in open.payload().iter().skip(1) {
            assert!(body.contains(&turn.content), "{}", turn.content);
        }
        assert!(body.starts_with("ENV DA CONVERSA"));
        assert!(super::super::analysis_view::consent_dialog(&app).is_some());
        for hidden in [false, true] {
            app.hide_values = hidden;
            for (w, h) in [(1400.0, 900.0), (1024.0, 720.0), (300.0, 200.0)] {
                app.window_size = Size::new(w, h);
                let _ = super::super::shell::root(&app);
            }
        }
        // "Cancelar": a abertura recem-criada (nada saiu) some e a previa fecha.
        let _ = update(&mut app, Msg::CancelPreview);
        assert!(app.consent_preview.is_none());
        assert_eq!(
            drain(&mut rx),
            vec![Command::Assistant(AssistantCommand::Delete("C1".into()))]
        );
    }

    #[test]
    fn conversa_reaberta_esperando_aprovacao_nao_abre_a_previa_sozinha() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx) = cloud_app(dir.path());
        let snap = {
            let mut s = app.snapshot.clone();
            s.assistant.open = Some(awaiting_conv(app.book, "ENV"));
            Msg::Snapshot(Box::new(s))
        };
        let _ = update(&mut app, snap);
        assert!(app.consent_preview.is_none());
        assert!(super::super::analysis_view::consent_dialog(&app).is_none());
        // O pe da conversa leva a ela.
        let _ = update(&mut app, Msg::ShowCloudPreview);
        assert_eq!(app.consent_preview.as_deref(), Some("C1"));
        let _ = update(&mut app, Msg::ApproveCloudSend);
        assert!(app.consent_preview.is_none());
        assert_eq!(
            drain(&mut rx),
            vec![Command::Assistant(AssistantCommand::ApproveCloudSend(
                "C1".into()
            ))]
        );
    }

    /// So a montagem da view: o toast do "Desfazer" monta pela raiz; o comportamento vive em
    /// `ui_model::app` (`apagar_conversa_some_na_hora_e_desfazer_nao_envia_nada`).
    #[test]
    fn apagar_conversa_monta() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx) = assistant_app(dir.path(), local_settings(Some("m")));
        app.snapshot.assistant.conversations =
            vec![summary("P1", Book::Personal), summary("P2", Book::Personal)];
        let _ = update(&mut app, Msg::AskDeleteConversation("P1".into()));
        let _ = super::super::shell::root(&app);
    }

    /// So a montagem da Analise sem a conversa escondida; o Desfazer vive em `ui_model::app`
    /// (`apagar_a_conversa_aberta_so_a_esconde_e_desfazer_a_mostra_de_novo`).
    #[test]
    fn apagar_a_conversa_aberta_monta() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx) = assistant_app(dir.path(), local_settings(Some("m")));
        let open = open_conv(Book::Personal, vec![]);
        let id = open.id.clone();
        app.snapshot.assistant.open = Some(open);
        let _ = update(&mut app, Msg::AskDeleteConversation(id));
        let _ = super::super::analysis_view::view(&app);
    }

    #[test]
    fn lista_de_conversas_monta_com_data_e_sem_data() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx) = assistant_app(dir.path(), local_settings(Some("m")));
        let mut dated = summary("P1", Book::Personal);
        dated.created_at = "2026-11-25T10:00:00.000Z".into();
        app.snapshot.assistant.conversations = vec![dated, summary("P2", Book::Personal)];
        app.hover_row = Some("P1".into());
        let _ = super::super::analysis_view::view(&app);
    }

    #[test]
    fn pergunta_sem_resposta_e_sem_nada_em_andamento_monta_com_tentar_de_novo() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx) = assistant_app(dir.path(), local_settings(Some("m")));
        app.snapshot.assistant.open =
            Some(open_conv(app.book, vec![msg(MessageRole::User, "Resuma")]));
        let _ = super::super::analysis_view::view(&app);
        // O botao manda `ChatRetry`, que reenvia a pergunta que ficou sem resposta.
        let _ = update(&mut app, Msg::ChatRetry);
        assert_eq!(
            drain(&mut rx),
            vec![Command::Assistant(AssistantCommand::Resend)]
        );
    }

    #[test]
    fn tela_analise_monta_com_conversa_streaming_erro_e_painel() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx) = assistant_app(dir.path(), local_settings(Some("llama3.1:8b")));
        let _ = update(&mut app, Msg::StartConversation);
        let mut answered = msg(
            MessageRole::Assistant,
            "Você gastou R$ 1.234,56 em mercado.",
        );
        answered.tokens_in = Some(120);
        answered.tokens_out = Some(30);
        answered.sent_to_cloud = true;
        answered.deny_data_collection = Some(true);
        answered.provider = Some("cloud".into());
        answered.model = Some("openai/gpt-4o".into());
        let mut cut = msg(MessageRole::Assistant, "parcial");
        cut.interrupted = true;
        app.snapshot.assistant.open = Some(open_conv(
            app.book,
            vec![
                msg(MessageRole::User, "Resuma"),
                answered,
                cut,
                msg(
                    MessageRole::Error,
                    "Não encontrei o Ollama em 127.0.0.1:11434.",
                ),
            ],
        ));
        app.snapshot.assistant.conversations = vec![summary("C1", app.book)];
        app.snapshot.assistant.last_error = Some("Falha ao gravar".into());
        let _ = update(&mut app, Msg::ConfigureAssistant);
        app.confirm_delete_all = Some(Instant::now() + DELETE_ALL_ARMED_FOR);
        app.chat_input = "e depois?".into();
        // Envelope da conversa diferente do de agora: a faixa aparece.

        let models = [
            None,
            Some(Err("Não encontrei o Ollama.".to_string())),
            Some(Ok(vec![])),
            Some(Ok(vec!["llama3.1:8b".to_string(), "qwen3:4b".to_string()])),
        ];
        for provider in [
            ProviderChoice::None,
            ProviderChoice::Local,
            ProviderChoice::Cloud,
        ] {
            app.snapshot.assistant.settings.provider = provider;
            for listed in &models {
                app.snapshot.assistant.models = listed.clone();
                for (busy, streaming) in [(false, None), (true, Some("R$ 10,00 e ".to_string()))] {
                    app.snapshot.assistant.busy = busy;
                    app.snapshot.assistant.streaming = streaming;
                    for hidden in [false, true] {
                        app.hide_values = hidden;
                        for (w, h) in [(1400.0, 900.0), (1024.0, 720.0), (300.0, 200.0)] {
                            app.window_size = Size::new(w, h);
                            let _ = super::super::analysis_view::view(&app);
                            let _ = super::super::shell::root(&app);
                        }
                    }
                }
            }
        }
        // Hub que nao subiu: no lugar do chat, o aviso; a tela ainda monta.
        app.snapshot.status = Status::Failed("porta ocupada".into());
        let _ = super::super::analysis_view::view(&app);
    }
    #[test]
    fn com_bandeja_fechar_zera_previa_envelope_e_remocao() {
        let dir = tempfile::tempdir().unwrap();
        let fake = Rc::new(Fake::with_tray());
        let (mut app, _rx, _tx) = app_with_desktop(
            Snapshot {
                devices: vec![device_view("D1", "Pixel 7", false, false, data())],
                ..Snapshot::default()
            },
            dir.path().to_path_buf(),
            fake,
        );
        app.window = Some(app.iced.register(window::Id::unique()));
        let _ = update(&mut app, Msg::AskRemove("D1".into()));
        let _ = update(&mut app, Msg::ShowEnvelope);
        app.preview_after = Some(Some("C1".into()));
        assert!(app.confirm_remove.is_some());
        assert!(app.envelope_preview.is_some());
        let _ = update(&mut app, Msg::CloseRequested);
        assert!(!app.closing);
        assert!(app.confirm_remove.is_none());
        assert!(app.envelope_preview.is_none());
        assert!(app.preview_after.is_none());
        assert!(!app.any_dialog_open());
    }

    #[test]
    fn conectar_um_celular_nao_aparece_com_o_hub_em_falha() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with(
            running_with(vec![phone("D1", "Pixel 7")]),
            dir.path().to_path_buf(),
        );
        let _ = update(&mut app, Msg::ConnectPhone);
        assert!(connect::dialog_open(&app));
        let _ = drain(&mut rx);
        // O hub cai com o dialogo aberto: ele fecha e o codigo e cancelado.
        let mut failed = running_with(vec![phone("D1", "Pixel 7")]);
        failed.status = Status::Failed("porta ocupada".into());
        let _ = update(&mut app, Msg::Snapshot(Box::new(failed)));
        assert_eq!(app.connect, None);
        assert!(drain(&mut rx).contains(&Command::CancelPairingToken));
        assert!(super::super::connection_view::connect_dialog(&app).is_none());
        // Pedir de novo (bandeja, estado vazio) leva a Aparelhos, com o card de falha, sem fluxo.
        let _ = update(&mut app, Msg::ConnectPhone);
        assert_eq!(app.screen, Screen::Connection);
        assert_eq!(app.connect, None);
        // Mesmo com um fluxo modal sobrando, o dialogo nao monta por cima da falha.
        app.connect = Some(ConnectFlow::new(true));
        assert!(!connect::dialog_open(&app));
        assert!(!app.any_dialog_open());
        assert!(super::super::connection_view::connect_dialog(&app).is_none());
        let _ = super::super::shell::root(&app);
    }

    #[test]
    fn cursor_so_pisca_com_janela_aberta() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _fake, _rx) = with_tray(dir.path());
        assert!(!blink_running(&app), "sem resposta chegando nao ha piscada");
        app.motion.sync(
            Scene {
                streaming: true,
                ..Default::default()
            },
            Instant::now(),
        );
        assert!(app.motion.blinking());
        assert!(blink_running(&app));
        // A janela fecha com a resposta ainda chegando (o `update` recalcularia a cena a partir
        // do estado, sem resposta de verdade aqui; o que importa e so a janela).
        app.window = None;
        assert!(app.motion.blinking(), "a resposta segue chegando");
        assert!(!blink_running(&app), "sem janela o relogio do cursor para");
    }

    #[test]
    fn abrir_sem_janela_cria_uma_e_com_janela_mantem_a_mesma() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _fake, _rx) = with_tray(dir.path());
        app.window = None;
        let _ = update(&mut app, Msg::Desktop(DesktopEvent::OpenWindow));
        let id = app.window.expect("janela aberta");
        let _ = update(&mut app, Msg::Desktop(DesktopEvent::OpenWindow));
        assert_eq!(app.window, Some(id));
    }

    #[test]
    fn conectar_pela_bandeja_com_dialogo_aberto_so_traz_a_janela() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _fake, _rx) = with_tray(dir.path());
        app.screen = Screen::Analysis;
        app.envelope_preview = Some("envelope".into());
        assert!(app.any_dialog_open());
        app.window = None;
        let _ = update(&mut app, Msg::Desktop(DesktopEvent::ConnectPhone));
        assert!(app.window.is_some(), "a janela abre");
        assert_eq!(app.screen, Screen::Analysis, "a tela por baixo nao muda");
        assert!(app.connect.is_none());
        assert!(app.envelope_preview.is_some(), "o dialogo continua");
    }

    #[test]
    fn conectar_pela_bandeja_abre_a_janela_e_o_fluxo() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _fake, _rx) = with_tray(dir.path());
        app.window = None;
        app.screen = Screen::Dashboard;
        let _ = update(&mut app, Msg::Desktop(DesktopEvent::ConnectPhone));
        let id = app.window.expect("janela aberta");
        assert_eq!(app.screen, Screen::Connection);
        assert!(app.connect.is_some());
        // Com a janela aberta, a mesma janela, e o fluxo nao recomeca.
        let _ = update(&mut app, Msg::Desktop(DesktopEvent::ConnectPhone));
        assert_eq!(app.window, Some(id));
        assert!(app.connect.is_some());
    }

    #[test]
    fn falha_do_nucleo_abre_a_janela_quando_nao_ha_uma() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _fake, _rx) = with_tray(dir.path());
        app.window = None;
        let failed = Snapshot {
            status: Status::Failed("porta ocupada".into()),
            ..Snapshot::default()
        };
        let _ = update(&mut app, Msg::Snapshot(Box::new(failed.clone())));
        assert!(
            app.window.is_some(),
            "falha silenciosa no login e o pior resultado"
        );
        // A pessoa viu o erro e fechou: outro snapshot ainda `Failed` nao reabre.
        app.window = None;
        let _ = update(&mut app, Msg::Snapshot(Box::new(failed.clone())));
        assert!(app.window.is_none());
        // Desligando: nao reabre.
        let _ = update(
            &mut app,
            Msg::Snapshot(Box::new(Snapshot {
                status: Status::Running,
                ..Snapshot::default()
            })),
        );
        app.closing = true;
        let _ = update(&mut app, Msg::Snapshot(Box::new(failed)));
        assert!(app.window.is_none());
    }

    #[test]
    fn sem_id_de_janela_avisa_uma_vez() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        let _ = update(&mut app, Msg::Minimize);
        assert!(app.warned_no_window);
        let _ = update(&mut app, Msg::Maximized(true));
        assert!(app.maximized);
    }

    #[test]
    fn alternar_ocultar_grava_preferencia_e_refaz_o_dashboard() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with_fixtures(dir.path());
        assert!(!app.hide_values);
        assert!(!app.dashboard.hidden);
        assert_eq!(app.dashboard.header.hide_toggle, "Ocultar valores");
        let shown = format!("{:?}", app.dashboard.how_am_i);
        assert!(shown.contains("R$ "), "{shown}");
        assert_eq!(app.transactions.list.income_label, "+R$ 6.500,00");
        assert_eq!(app.transactions.synced_devices, 1);
        let shown_transactions = app.transactions.clone();

        let _ = update(&mut app, Msg::ToggleHideValues);
        assert!(app.hide_values);
        assert!(app.dashboard.hidden);
        assert_eq!(app.dashboard.header.hide_toggle, "Mostrar valores");
        // O mes e a conta continuam: so o texto dos valores muda.
        assert_eq!(app.dashboard.month, app.month());
        let hidden = format!("{:?}", app.dashboard.how_am_i);
        assert!(hidden.contains("•••"), "{hidden}");
        // Lancamentos tambem, mesmo sem estar na tela: o modelo e refeito junto, e o
        // "· N aparelhos" do cabecalho (que vem do snapshot) nao se perde.
        assert_eq!(app.screen, Screen::Dashboard);
        assert_eq!(leaks(&format!("{:#?}", app.transactions)), None);
        assert_eq!(app.transactions.list.income_label, MASK);
        assert_eq!(app.transactions.synced_devices, 1);
        assert_eq!(app.transactions.list.count, shown_transactions.list.count);
        wait_prefs(dir.path(), |p| p.hide_values);

        // Trocar o tema leva a escolha junto (a gravacao e do arquivo inteiro).
        let _ = update(&mut app, Msg::SetTheme(ThemeMode::Light));
        wait_prefs(dir.path(), |p| p.theme == ThemeMode::Light);
        assert!(UiPrefs::load(dir.path()).hide_values);

        let _ = update(&mut app, Msg::ToggleHideValues);
        assert!(!app.dashboard.hidden);
        assert_eq!(app.transactions, shown_transactions);
        wait_prefs(dir.path(), |p| !p.hide_values);
        assert_eq!(UiPrefs::load(dir.path()).theme, ThemeMode::Light);
    }

    #[test]
    fn ligar_iniciar_com_o_windows_grava_no_sistema_e_no_ui_json() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, fake, _rx) = with_tray(dir.path());
        assert_eq!(app.autostart, Some(false));
        let _ = update(&mut app, Msg::SetAutostart(true));
        assert_eq!(fake.autostart.get(), Some(true));
        assert_eq!(app.autostart, Some(true));
        assert_eq!(app.autostart_pref, Some(true));
        assert!(!app.autostart_failed);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while UiPrefs::load(dir.path()).autostart != Some(true) {
            assert!(std::time::Instant::now() < deadline, "ui.json nao gravado");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn reduzir_movimento_grava_e_sobrevive_a_outra_preferencia() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with_fixtures(dir.path());
        let _ = update(&mut app, Msg::ToggleReduceMotion);
        assert!(app.motion.reduce());
        wait_prefs(dir.path(), |p| p.reduce_motion);
        let _ = update(&mut app, Msg::ShowEnvelope);
        assert!(!app.motion.animating(), "sem animacao com a opcao ligada");
        assert_eq!(app.motion.scrim(), 1.0);
        // A gravacao e do arquivo inteiro: trocar o olho nao apaga a escolha.
        let _ = update(&mut app, Msg::ToggleHideValues);
        wait_prefs(dir.path(), |p| p.hide_values);
        assert!(UiPrefs::load(dir.path()).reduce_motion);
        let _ = update(&mut app, Msg::ToggleReduceMotion);
        wait_prefs(dir.path(), |p| !p.reduce_motion);
    }

    #[test]
    fn trocar_o_tema_preserva_a_preferencia_de_autostart() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, fake, _rx) = with_tray(dir.path());
        fake.autostart.set(Some(true));
        app.autostart = Some(true);
        let _ = update(&mut app, Msg::SetAutostart(false));
        let _ = update(&mut app, Msg::SetTheme(ThemeMode::Light));
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            let prefs = UiPrefs::load(dir.path());
            if prefs.theme == ThemeMode::Light && prefs.autostart == Some(false) {
                break;
            }
            assert!(std::time::Instant::now() < deadline, "ui.json: {prefs:?}");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
