//! Estado e mensagens da janela. O estado e o ultimo snapshot do nucleo mais o que e da
//! propria tela: tela atual, tema, janela (id, maximizada), se esta fechando. Nada aqui e tipo de
//! toolkit: o que o toolkit precisa guardar (QR, caches, texturas) mora ao lado, no adaptador.

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use time::macros::format_description;
use time::{OffsetDateTime, UtcOffset};

use crate::assistant::secret::Secret;
use crate::assistant::settings::ProviderChoice;
use crate::config::{self, ThemeMode, UiPrefs};
use crate::dashboard::contract::Book;
use crate::dashboard::dataset::{Dataset, avatar_bytes};
use crate::dashboard::grafite::{GrafiteView, build_grafite};
use crate::dashboard::list::{Filters, NONE_KEY, TypeFilter};
use crate::dashboard::people::{DeviceInfo, Person, people};
use crate::dashboard::periods::{month_label, month_window};
use crate::dashboard::view::{ReservesView, TransactionsView, build_reserves, build_transactions};
use crate::desktop::{Desktop, DesktopEvent};
use crate::hub::snapshot::{DeviceView, Snapshot, Status};
use crate::hub::{AssistantCommand, Command, HubHandle};
use crate::store::erase::{Area, AreaSet};
use crate::ui_model::analysis::{self, awaiting_cloud_consent};
use crate::ui_model::assistant_setup::SetupStep;
use crate::ui_model::connect::{self, ConnectFlow, ConnectStep, PhoneOs, is_up};
use crate::ui_model::effect::Effect;
use crate::ui_model::motion::{Motion, Scene, ShownToast};
use crate::ui_model::window::{ResizeEdge, Size, WindowId};
use crate::ui_model::{assistant_setup, keys, shell};

/// Quanto tempo "Copiado" fica no botao, como no prototipo.
pub const COPIED_FOR: Duration = Duration::from_millis(1600);

/// Quanto tempo o toast de "Desfazer" fica na tela antes de a revogacao ser enviada.
pub const UNDO_FOR: Duration = Duration::from_secs(5);

/// Quanto tempo um aviso sem acao ("Pixel da Ana conectado") fica na tela.
pub const NOTICE_FOR: Duration = Duration::from_secs(4);

/// Remocao confirmada mas ainda nao enviada ao nucleo.
///
/// Revogacao adiada, e nao "revogar e des-revogar": durante os 5 s o celular nunca recebe
/// `401` (que a fatia 3 do app poderia tratar descartando a chave, e ai "Desfazer" mentiria).
/// Custo aceito: se o hub cair nesses 5 s, a remocao nao aconteceu e a pessoa reaparece.
///
/// Apagar uma conversa da Analise usa o mesmo adiamento: um "Desfazer" so na tela, e o mesmo
/// caminho de envio (prazo, troca de tela, fechar, desligar).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingRemoval {
    /// Id do aparelho, ou da conversa em `DeleteConversation`.
    pub target_id: String,
    pub toast: String,
    pub deadline: Instant,
    pub action: PendingAction,
}

impl PendingRemoval {
    /// O aparelho que esta saindo (some da lista na hora); `None` para conversa. Pelo tipo da
    /// acao, e nao so pelo id: um id de conversa nunca pode esconder um aparelho.
    pub fn device_id(&self) -> Option<&str> {
        match self.action {
            PendingAction::Remove(_) | PendingAction::Erase(_) => Some(&self.target_id),
            PendingAction::DeleteConversation => None,
        }
    }

    /// A conversa que esta sendo apagada (some da lista na hora).
    pub fn conversation_id(&self) -> Option<&str> {
        match self.action {
            PendingAction::DeleteConversation => Some(&self.target_id),
            PendingAction::Remove(_) | PendingAction::Erase(_) => None,
        }
    }
}

/// O que sai para o nucleo quando o prazo do "Desfazer" vence. Remocao e apagar andam juntos:
/// desfazer cancela os dois, e nada foi enviado antes disso.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingAction {
    /// Aparelho ativo: revoga e apaga `AreaSet` (vazio = so remove, mantendo os dados).
    Remove(AreaSet),
    /// Aparelho ja removido: so apaga as areas.
    Erase(AreaSet),
    /// "Apagar" de uma conversa da Analise. Se era a aberta, ela so some da tela
    /// (`analysis_view::open_conversation`): nenhum `Close` nosso sai antes do prazo, porque
    /// fechar interromperia uma resposta chegando e "Desfazer" nao a traria de volta. Outras
    /// acoes da pessoa nesse meio tempo ainda interrompem a resposta escondida, como sempre
    /// fizeram: `NewConversation` ("Conversar sobre isso"), `Open` de outra conversa e a troca
    /// de conta (que fecha a conversa da outra conta).
    DeleteConversation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemoveStep {
    Choose,
    Confirm,
}

/// Modal aberto para este aparelho. `selected` vazio e "Manter tudo" (o padrao): marcar
/// qualquer area leva ao passo `Confirm`, em vermelho, antes de qualquer envio.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoveDialog {
    pub device_id: String,
    pub step: RemoveStep,
    pub selected: AreaSet,
}

/// Tamanho inicial da janela (`ui::run`); vale ate o primeiro evento de redimensionamento.
pub const INITIAL_SIZE: Size = Size::new(1440.0, 900.0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CopyTarget {
    Address,
    /// Pagina do guia do certificado (`http://<ip>:7778`), no passo 2 do "Conectar um celular".
    Guide,
    /// Pastas de Ajustes > Sobre: copiaveis mesmo sem rede, ao contrario dos enderecos.
    DataDir,
    LogsDir,
    /// Fallback quando o navegador nao abre: a URL das releases com "Copiar".
    ReleasesUrl,
    /// "Copiar" de uma resposta do assistente; qual delas fica em `App::copied_reply`. Nao
    /// passa por `Msg::Copy` (o texto depende da mensagem): sai por `CopyReply`.
    Reply,
}

/// Painel visivel da Analise na janela estreita (segmentado [Achados | Conversa]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AnalysisTab {
    #[default]
    Findings,
    Chat,
}

/// Foto decodificada e o data URI de onde veio: so decodifica de novo quando o URI muda. A
/// textura do toolkit mora em `IcedSide`, com a mesma chave.
pub struct Avatar {
    pub uri: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Screen {
    #[default]
    Dashboard,
    Transactions,
    Reserves,
    Analysis,
    Connection,
    /// Ajustes: tema, ocultar valores, iniciar com o Windows, assistente e sobre.
    Settings,
}

impl Screen {
    /// Tela de dados da conta, com valores em R$: e onde o olho "Ocultar valores" aparece e
    /// onde Ctrl+H alterna (decisao do dono de 25/11: ocultar vale em toda tela de dados).
    /// Aparelhos e Ajustes sao do hub e nao mostram dinheiro. Um lugar so para o botao e o
    /// atalho nao divergirem.
    pub(crate) fn shows_values(self) -> bool {
        matches!(
            self,
            Screen::Dashboard | Screen::Transactions | Screen::Reserves | Screen::Analysis
        )
    }
}

pub struct App {
    pub(crate) handle: HubHandle,
    pub(crate) snapshot: Snapshot,
    /// Sobe a cada vez que o que os graficos desenharam deixa de valer (dados, mes, tema, olho,
    /// janela nova). O toolkit guarda a geometria em cache e a descarta quando o numero muda:
    /// o modelo nao conhece o cache, so avisa que ele envelheceu. `Cell` porque quem avisa
    /// (`clear_charts`) so tem `&self`.
    pub(crate) charts_epoch: Cell<u32>,
    pub(crate) closing: bool,
    /// Fuso local lido uma vez no `main`, antes de existirem outras threads.
    pub(crate) offset: UtcOffset,
    /// `None` enquanto nao ha janela (boot por `--autostart`, ou fechada); sem ela os botoes
    /// da barra nao fazem nada (e o `warn!` sai uma vez so).
    pub(crate) window: Option<WindowId>,
    pub(crate) warned_no_window: bool,
    /// Maximizada, a moldura de redimensionar some: borda de janela maximizada nao arrasta.
    pub(crate) maximized: bool,
    /// Botao pressionado na barra de titulo e ainda sem arrasto: o arrasto so comeca quando o
    /// mouse anda, para o duplo clique (maximizar) nao ser engolido pelo laco de movimento do
    /// Windows.
    pub(crate) title_pressed: bool,
    /// A preferencia (Escuro, Claro ou Sistema); o que se desenha sai de `theme` + `system_dark`.
    pub(crate) theme: ThemeMode,
    /// O sistema esta no escuro? So pesa com `ThemeMode::System`. Nasce do registro (ou escuro,
    /// o padrao do design, quando o sistema nao diz) e segue as trocas avisadas.
    pub(crate) system_dark: bool,
    pub(crate) screen: Screen,
    /// Onde fica o `ui.json` (o diretorio de dados do hub).
    pub(crate) prefs_dir: PathBuf,
    /// Tamanho da janela: o toolkit nao tem `auto-fit`, entao a tela decide colunas por ele.
    pub(crate) window_size: Size,
    /// Linhas de "Usuarios conectados", recalculadas so quando aparelhos ou dataset mudam.
    pub(crate) people: Vec<Person>,
    /// `device_id` -> foto.
    pub(crate) avatars: HashMap<String, Avatar>,
    /// Qual campo mostra "Copiado" e ate quando.
    pub(crate) copied: Option<(CopyTarget, Instant)>,
    /// Linha sob o mouse em "Usuarios conectados" (hover de 4 %).
    pub(crate) hover_row: Option<String>,
    /// Modal "Remover ...?" (ou "Dados de ...", para aparelho ja removido) aberto.
    pub(crate) confirm_remove: Option<RemoveDialog>,
    pub(crate) pending_removal: Option<PendingRemoval>,
    /// "Depois" no aviso de aparelhos removidos com dados: so esta sessao, nada e gravado. O
    /// aviso volta na proxima abertura porque a deteccao e derivada do banco.
    pub(crate) orphans_postponed: bool,
    /// "Hoje" (`YYYY-MM-DD`) no fuso local: a unica leitura de relogio que decide algo na tela
    /// (mes corrente, tag "Parcial"). Nasce no boot e e reconferida a cada tique.
    pub(crate) today: String,
    /// Os 12 meses navegaveis (`month_window(today)`), o mais antigo primeiro.
    pub(crate) months: Vec<String>,
    /// Indice em `months`; 11 = mes corrente.
    pub(crate) month_idx: usize,
    /// Conta do dashboard (seletor Pessoal | Comercial). So estado de UI: nada e gravado, e a
    /// janela sempre abre na pessoal.
    pub(crate) book: Book,
    /// O dataset do snapshot visto por `book`: toda tela e contagem le daqui, nunca do total
    /// (`Dataset::for_book`). Recalculado em `rebuild`, junto com os modelos de tela.
    pub(crate) scope: Dataset,
    /// Filtros da tela Lancamentos; sobrevivem a troca de mes.
    pub(crate) filters: Filters,
    /// Modelos de tela, recalculados so quando dados, mes, filtros ou "hoje" mudam: desenhar
    /// um quadro nunca percorre o dataset.
    pub(crate) dashboard: GrafiteView,
    /// "Ver todas as N" de "Próximas saídas" abriu a lista no lugar. Volta a fechar ao trocar
    /// de mes ou de conta, nao a cada sync (fecharia na cara de quem esta lendo).
    pub(crate) upcoming_expanded: bool,
    pub(crate) transactions: TransactionsView,
    /// Tela Reservas: sem mes escolhido (saldos sao acumulados), so depende de dados e "hoje".
    pub(crate) reserves: ReservesView,
    /// Tela Analise: achados da conta aberta (`advisor`), recalculados com os dados e o "hoje".
    pub(crate) analysis: crate::advisor::Analysis,
    /// Previa "Ver o que o assistente vê" aberta: o texto exato de `envelope::render`, sem
    /// mascara (a tela mascara ao desenhar; o que sai é sempre o texto inteiro). Fica guardado
    /// para `rebuild` poder refrescá-lo: dados novos com a prévia aberta não podem deixá-la
    /// mostrando um envelope que já não é o que seria enviado.
    pub(crate) envelope_preview: Option<String>,
    /// Menu de categoria da tela Lancamentos aberto.
    pub(crate) cat_menu_open: bool,
    /// `user_id` -> foto, para as linhas e os chips de autor (`avatars` e por aparelho).
    pub(crate) user_avatars: HashMap<String, Avatar>,
    /// `open::that_detached` falhou: a linha da versao mostra a URL com "Copiar".
    pub(crate) releases_open_failed: bool,
    /// Bandeja, "iniciar com o Windows" (ou `Headless`, sem nada disso). `Rc`: o `TrayIcon`
    /// vive na thread principal, a mesma do `update`.
    pub(crate) desktop: Rc<dyn Desktop>,
    /// Ultimo texto mandado ao tooltip da bandeja.
    pub(crate) tooltip: String,
    /// Ultimo desenho mandado ao icone da bandeja; nasce `Normal`, que e o do `install`.
    pub(crate) tray_icon: crate::desktop::tray::TrayIcon,
    /// As tres animacoes (scrim, toast, cursor) e o "Reduzir movimento".
    pub(crate) motion: Motion,
    /// "Iniciar com o Windows" como o sistema diz (cache do registro, lido no boot e depois de
    /// cada troca); `None` = sistema sem a opcao, e a secao some de Ajustes.
    pub(crate) autostart: Option<bool>,
    /// O que o `ui.json` lembra (`Some(false)` = desligado de proposito); vai junto em toda
    /// gravacao do arquivo para trocar o tema nao apagar a decisao.
    pub(crate) autostart_pref: Option<bool>,
    /// A ultima troca da opcao falhou no sistema: a linha em Ajustes avisa.
    pub(crate) autostart_failed: bool,
    /// "Ocultar valores" (todas as telas de dados), lembrado no `ui.json` como o tema.
    pub(crate) hide_values: bool,
    /// Texto digitado no chat da Analise.
    pub(crate) chat_input: String,
    /// Painel da Analise no segmentado da janela estreita.
    pub(crate) analysis_tab: AnalysisTab,
    /// A resposta cujo "Copiar" virou "Copiado" (`CopyTarget::Reply`).
    pub(crate) copied_reply: Option<String>,
    /// Dialogo "Configurar assistente" (8.6); `None` = fechado.
    pub(crate) assistant_setup: Option<crate::ui_model::assistant_setup::AssistantSetup>,
    /// A previa de envio a nuvem aberta (8.5), pela conversa que ela mostra. Presa ao id: so
    /// aparece enquanto essa conversa for a aberta e ainda esperar aprovacao; reabrir do
    /// historico uma conversa local com a nuvem configurada nao a faz saltar sozinha.
    pub(crate) consent_preview: Option<String>,
    /// "Conversar" na nuvem pediu a previa: ela abre quando o snapshot trouxer a conversa nova.
    /// Guarda a conversa aberta no clique, para nao abrir a previa de outra que o snapshot
    /// anterior ao comando ainda traga.
    pub(crate) preview_after: Option<Option<String>>,
    /// A chave do OpenRouter que a pessoa esta digitando no dialogo. Vive so aqui e so ate o
    /// ultimo botao (`Configure` a leva ao nucleo e o campo esvazia na hora) ou ate o dialogo
    /// fechar por qualquer caminho (`close_setup`); nunca vai para log nem Debug (por isso a
    /// `Secret` nas mensagens e no comando).
    pub(crate) key_input: String,
    /// O envelope desta conta agora (nunca mascarado), recalculado so em `rebuild`: a previa, o
    /// envio e o aviso "os dados mudaram" (spec 3) leem o mesmo texto sem refazer as agregacoes
    /// a cada quadro. O aviso compara com o envelope guardado na conversa, entao vale tambem
    /// para conversa reaberta do historico e nao dispara com sync de outra conta.
    pub(crate) envelope_now: String,
    /// "Apagar todas" armado: o segundo clique antes do prazo apaga. Expira no `Tick`, para um
    /// clique esquecido nao deixar o botao perigoso armado para sempre.
    pub(crate) confirm_delete_all: Option<Instant>,
    /// Conversas cujo `Delete` ja saiu para o nucleo e que o snapshot ainda traz: continuam
    /// escondidas ate um snapshot sem elas (senao reapareceriam por um instante depois do
    /// prazo do "Desfazer") ou ate um erro novo do assistente, quando o apagar falhou e a
    /// conversa volta junto com o aviso.
    pub(crate) deleting: HashSet<String>,
    /// Fluxo "Conectar um celular" (spec 8.4): passo, sistema escolhido e se e o dialogo ou o
    /// fluxo em tela. `None` = fechado (o fluxo em tela, sem aparelho, nasce no passo 1).
    pub(crate) connect: Option<ConnectFlow>,
    /// Aviso curto sem "Desfazer" e o prazo dele. Separado do `pending_removal` de proposito:
    /// aquele vira comando para o nucleo ao vencer, trocar de tela ou fechar.
    pub(crate) notice: Option<(String, Instant)>,
}

#[derive(Debug, Clone)]
pub enum Msg {
    /// Em `Box`: o snapshot e grande e as outras mensagens nao precisam carregar o tamanho dele.
    Snapshot(Box<Snapshot>),
    IssueToken,
    /// Redesenha a contagem regressiva e os "ha 3 min"; confere a virada do dia.
    Tick,
    PrevMonth,
    NextMonth,
    /// Seletor Pessoal | Comercial.
    SelectBook(Book),
    /// Olho do cabecalho (e Ctrl+H): "Ocultar valores" / "Mostrar valores".
    ToggleHideValues,
    /// "Reduzir movimento" de Ajustes.
    ToggleReduceMotion,
    /// Um quadro de `window::frames()`, so enquanto alguma animacao anda.
    Frame(Instant),
    /// "Ver todas as N" de "Próximas saídas": abre a lista inteira no card.
    ExpandUpcoming,
    /// Abre ou fecha o menu de categoria (o botao alterna, como no prototipo).
    ToggleCategoryMenu,
    CloseCategoryMenu,
    /// `None` = todas; `Some("")` = "Sem categoria".
    PickCategory(Option<String>),
    /// Chip de autor; `""` = "Sem autor".
    ToggleAuthor(String),
    /// "Todos" dos chips de autor.
    AllAuthors,
    SetType(TypeFilter),
    ClearFilters,
    CloseRequested,
    ForceExit,
    /// `window::open` respondeu: a janela existe e pode ser consultada.
    WindowOpened(WindowId),
    /// O toolkit fechou a janela (`close_events`): o id morreu com ela.
    WindowClosed(WindowId),
    /// Bandeja, menu ou segunda execucao: abrir a janela (ou traze-la) e "Desligar o hub", o
    /// unico caminho que encerra o processo quando ha bandeja.
    Desktop(DesktopEvent),
    TitlePressed,
    TitleReleased,
    TitleMoved,
    Minimize,
    ToggleMaximize,
    Maximized(bool),
    /// A janela mudou de tamanho (arrasto, Win+seta, encaixe): guarda a largura e reconsulta
    /// se esta maximizada.
    Resized(Size),
    Resize(ResizeEdge),
    SelectScreen(Screen),
    SetTheme(ThemeMode),
    /// "Configurar" do assistente (Ajustes e o painel da Analise): abre o dialogo em passos.
    ConfigureAssistant,
    /// O sistema trocou entre escuro e claro (`system::theme_changes`), ou respondeu a
    /// consulta do boot (`system::theme`).
    SystemTheme(bool),
    /// "Iniciar com o Windows" de Ajustes.
    SetAutostart(bool),
    Copy(CopyTarget),
    CopyExpired,
    /// "Copiar" de uma resposta do assistente (id da mensagem).
    CopyReply(String),
    /// Segmentado [Achados | Conversa] da Analise na janela estreita.
    AnalysisTab(AnalysisTab),
    /// "Versoes no GitHub" de Ajustes > Sobre.
    OpenReleases,
    /// O sistema recusou abrir o navegador (devolvida por `Effect::OpenUrl`).
    OpenUrlFailed,
    HoverRow(String),
    UnhoverRow(String),
    AskRemove(String),
    /// Marca ou desmarca uma area no passo 1 (area sem dados nao faz nada).
    ToggleArea(Area),
    /// "Apagar tudo": marca as areas que tem dados.
    SelectAllAreas,
    /// "Manter tudo": desmarca tudo (o padrao).
    SelectNoAreas,
    /// Botao primario do passo 1: sem area marcada confirma direto; com area vai ao passo 2.
    RemoveContinue,
    RemoveBack,
    CancelRemove,
    ConfirmRemove,
    UndoRemove,
    /// "Decidir" do aviso: abre o modal do primeiro aparelho removido com dados.
    DecideOrphan,
    /// "Depois" do aviso.
    PostponeOrphans,
    /// "Ver o que o assistente vê" (tela Análise).
    ShowEnvelope,
    CloseEnvelope,
    /// "Conversar sobre isso": nova conversa com o envelope atual.
    StartConversation,
    ChatInput(String),
    ChatSubmit,
    ChatStop,
    ChatRetry,
    OpenConversation(String),
    CloseConversation,
    DeleteConversation(String),
    /// "Apagar" da lista de conversas: some na hora, com "Desfazer" de 5 s, e so entao sai.
    AskDeleteConversation(String),
    /// Dois cliques: o primeiro arma (`confirm_delete_all`), o segundo apaga.
    DeleteAllConversations,
    /// "x", "Cancelar", Esc ou scrim do dialogo "Configurar assistente".
    CloseAssistantSetup,
    /// Radio do passo 1: onde roda (so rascunho).
    SetupChoice(ProviderChoice),
    /// "Continuar" do passo 1.
    SetupNext,
    /// "Voltar" do passo 2.
    SetupBack,
    /// Campo "Porta" do Ollama (so rascunho; filtra digitos).
    SetupPort(String),
    /// "Testar conexao": lista os modelos do Ollama na porta do campo.
    SetupTest,
    SetupLocalModel(String),
    /// Campo "Modelo" da nuvem (so rascunho).
    SetupCloudModel(String),
    /// "Nao permitir que o provedor guarde ou treine" (so rascunho).
    SetupDeny(bool),
    /// O ultimo botao ("Usar Ollama local", "Usar OpenRouter", "Desligar assistente").
    SetupFinish,
    /// Texto do campo da chave; `Secret` para um `{:?}` de mensagem nao vazar a chave.
    KeyInput(crate::assistant::secret::Secret),
    /// "Remover" da chave salva (grava na hora: apagar nao precisa de previa).
    RemoveKey,
    /// "Ver previa e enviar" do pe da conversa que espera aprovacao.
    ShowCloudPreview,
    /// "Cancelar" da previa: apaga so a abertura recem-criada; o resto so fecha.
    CancelPreview,
    /// Esc ou scrim da previa: so fecha, nunca apaga.
    ClosePreview,
    /// "Enviar ao OpenRouter" da previa: a pessoa viu o que sai e libera o envio desta conversa.
    ApproveCloudSend,
    /// "Conectar um celular" (cabecalho de Aparelhos, Resumo e vazios): vai a Aparelhos com o
    /// dialogo aberto, ou com o fluxo em tela quando nao ha aparelho.
    ConnectPhone,
    /// "Continuar" / "Já instalei": entrar no passo 3 ja pede o codigo.
    ConnectNext,
    ConnectBack,
    /// Segmentado Android | iPhone do passo 2.
    ConnectOs(PhoneOs),
    /// "x", Esc ou clique no scrim: fecha o dialogo e cancela o codigo pendente.
    ConnectClose,
    /// Tecla que nenhum widget consumiu; `keys::shortcut` decide o que ela faz.
    KeyPressed(crate::ui_model::keys::KeyPress),
    /// Tab e Shift+Tab entre os campos de texto.
    FocusNext,
    FocusPrevious,
    Noop,
}

impl App {
    pub(crate) fn new(
        handle: HubHandle,
        offset: UtcOffset,
        prefs: UiPrefs,
        prefs_dir: PathBuf,
        desktop: Rc<dyn Desktop>,
    ) -> App {
        let snapshot = handle.snapshot.borrow().clone();
        // Sem resposta do sistema: o que ele dizia na ultima gravacao, ou escuro (o padrao do
        // design) se nunca houve.
        let saved = prefs.theme != ThemeMode::System || prefs.system_dark;
        let system_dark = desktop.system_dark().unwrap_or(saved);
        let mut app = App {
            handle,
            snapshot,
            charts_epoch: Cell::new(0),
            closing: false,
            offset,
            window: None,
            warned_no_window: false,
            maximized: false,
            title_pressed: false,
            theme: prefs.theme,
            system_dark,
            screen: Screen::default(),
            prefs_dir,
            window_size: INITIAL_SIZE,
            people: Vec::new(),
            avatars: HashMap::new(),
            copied: None,
            hover_row: None,
            confirm_remove: None,
            pending_removal: None,
            orphans_postponed: false,
            today: String::new(),
            months: Vec::new(),
            month_idx: 0,
            book: Book::Personal,
            scope: Dataset::default(),
            filters: Filters::default(),
            dashboard: GrafiteView::default(),
            upcoming_expanded: false,
            transactions: TransactionsView::default(),
            reserves: ReservesView::default(),
            analysis: crate::advisor::Analysis::default(),
            envelope_preview: None,
            cat_menu_open: false,
            user_avatars: HashMap::new(),
            releases_open_failed: false,
            autostart: desktop.autostart(),
            autostart_pref: prefs.autostart,
            autostart_failed: false,
            hide_values: prefs.hide_values,
            chat_input: String::new(),
            analysis_tab: AnalysisTab::Findings,
            copied_reply: None,
            assistant_setup: None,
            consent_preview: None,
            preview_after: None,
            key_input: String::new(),
            envelope_now: String::new(),
            confirm_delete_all: None,
            deleting: HashSet::new(),
            connect: None,
            notice: None,
            desktop,
            tooltip: String::new(),
            tray_icon: crate::desktop::tray::TrayIcon::Normal,
            motion: Motion::new(prefs.reduce_motion, Instant::now()),
        };
        app.refresh_people();
        app.sync_tray();
        app.set_today(local_today(offset));
        app
    }

    /// O que vai para o `ui.json`. Toda gravacao leva o arquivo inteiro: montar `UiPrefs` campo
    /// a campo em cada mensagem fazia trocar o tema apagar a escolha de outra opcao.
    fn prefs(&self) -> UiPrefs {
        UiPrefs {
            theme: self.theme,
            system_dark: self.theme == ThemeMode::System && self.system_dark,
            autostart: self.autostart_pref,
            hide_values: self.hide_values,
            reduce_motion: self.motion.reduce(),
        }
    }

    /// Algum dialogo sobre o scrim. O mesmo criterio do Esc e dos atalhos bloqueados
    /// (`keys::shortcut`) e da pilha de `shell::root`.
    pub(crate) fn any_dialog_open(&self) -> bool {
        self.confirm_remove.is_some()
            || self.envelope_preview.is_some()
            || connect::dialog_open(self)
            || self.assistant_setup.is_some()
            || analysis::shown_consent(self).is_some()
    }

    /// O que as animacoes olham: dialogo, toast (o "Desfazer" ganha do aviso, como na tela) e
    /// resposta chegando com a Analise na tela.
    fn scene(&self) -> Scene {
        let toast = match (&self.pending_removal, &self.notice) {
            (Some(pending), _) => Some(ShownToast {
                text: pending.toast.clone(),
                undo: true,
            }),
            (None, Some((notice, _))) => Some(ShownToast {
                text: notice.clone(),
                undo: false,
            }),
            (None, None) => None,
        };
        let assistant = &self.snapshot.assistant;
        Scene {
            dialog: self.any_dialog_open(),
            toast,
            streaming: self.screen == Screen::Analysis
                && assistant.busy
                && assistant.streaming.is_some(),
        }
    }

    /// Tooltip e icone da bandeja a partir do snapshot; cada um so vai ao sistema quando muda
    /// (`Shell_NotifyIcon` a cada snapshot seria desperdicio, e trocar o icone pisca a barra).
    fn sync_tray(&mut self) {
        let active = self.snapshot.devices.iter().filter(|d| !d.revoked).count();
        let text = crate::desktop::tray::tooltip(&self.snapshot.status, active);
        // O cache so guarda o que o sistema aceitou: recusado, o proximo snapshot ou tique
        // tenta de novo, em vez de a bandeja ficar presa no estado velho.
        if text != self.tooltip && self.desktop.set_tooltip(&text) {
            self.tooltip = text;
        }
        let icon = crate::desktop::tray::icon_for(&self.snapshot.status);
        if icon != self.tray_icon && self.desktop.set_icon(icon) {
            self.tray_icon = icon;
        }
    }

    /// Mes escolhido no seletor.
    pub(crate) fn month(&self) -> &str {
        self.months.get(self.month_idx).map_or("", String::as_str)
    }

    /// Troca "hoje". A janela de 12 meses anda junto; o mes escolhido continua o mesmo se
    /// ainda estiver nela (quem olhava agosto continua em agosto), senao vai para o corrente.
    pub(crate) fn set_today(&mut self, today: String) {
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

    /// Recalcula as tres telas de dados a partir do snapshot atual.
    pub(crate) fn rebuild(&mut self) {
        // A conta comercial pode sumir do hub (linha apagada) com ela selecionada: sem a
        // linha o seletor desaparece, entao quem ficou nela nao teria como voltar.
        if self.book == Book::Business && !self.snapshot.dashboard.has_business_book() {
            self.select_book(Book::Personal);
        }
        self.scope = self.snapshot.dashboard.for_book(self.book);
        self.analysis = crate::advisor::analyze(&self.scope, &self.today);
        self.envelope_now = self.compute_envelope();
        if self.envelope_preview.is_some() {
            self.envelope_preview = Some(self.current_envelope());
        }
        self.rebuild_valued();
        self.refresh_user_avatars();
        self.clear_charts();
        tracing::trace!("telas recalculadas para {}", self.month());
    }

    /// As telas cujo modelo traz o dinheiro ja formatado: Resumo, Lancamentos e Reservas.
    /// "Ocultar valores" refaz as tres juntas, nao so a da vez: trocar de tela com o olho ligado
    /// tem de chegar numa tela ja mascarada. A Analise fica de fora de proposito: os achados
    /// guardam o texto real (vao no envelope do assistente) e a tela os mascara ao desenhar.
    fn rebuild_valued(&mut self) {
        let month = self.month().to_string();
        self.transactions = build_transactions(
            &self.scope,
            &self.today,
            &month,
            &self.filters,
            self.hide_values,
        );
        // O "· N aparelhos" da linha Parcial dos Lancamentos: so aparelhos ativos que ja
        // enviaram. Vem do snapshot, nao do dataset; refazer o modelo sem isto o zeraria.
        self.transactions.synced_devices = self
            .snapshot
            .devices
            .iter()
            .filter(|d| !d.revoked && d.last_push_at.is_some())
            .count();
        self.reserves = build_reserves(&self.scope, &self.today, self.hide_values);
        self.rebuild_dashboard();
    }

    /// Abre o dialogo "Configurar assistente" com o que esta gravado. Ja aberto, fica como esta
    /// (nao perde o que a pessoa digitou).
    fn open_setup(&mut self) {
        if self.assistant_setup.is_none() {
            self.assistant_setup = Some(crate::ui_model::assistant_setup::AssistantSetup::open(
                &self.snapshot.assistant.settings,
            ));
        }
    }

    /// Fecha o dialogo por qualquer caminho ("Cancelar", "x", scrim, Esc, gravacao que deu
    /// certo, janela fechada): a chave digitada e nao gravada nao fica na memoria.
    pub(crate) fn close_setup(&mut self) {
        self.assistant_setup = None;
        self.key_input.clear();
    }

    /// Um snapshot novo chegou: o dialogo acompanha o gravado (campo que a pessoa nao mexeu) e
    /// termina a gravacao pendente (fecha se deu certo; com erro, fica aberto mostrando-o). A
    /// previa pedida abre quando a conversa nova aparece esperando aprovacao.
    fn sync_assistant_dialogs(&mut self) {
        let view = &self.snapshot.assistant;
        let mut done = false;
        let hub_down = matches!(self.snapshot.status, Status::Failed(_) | Status::Stopped);
        if let Some(setup) = self.assistant_setup.as_mut() {
            setup.resync(&view.settings);
            match (&view.configure_result, setup.saving) {
                (Some((n, result)), Some(expected)) if *n == expected => {
                    setup.saving = None;
                    match result {
                        Ok(()) => done = true,
                        Err(msg) => setup.error = Some(msg.clone()),
                    }
                }
                // O nucleo caiu com a gravacao no ar: a resposta nunca vira. O botao volta e o
                // dialogo diz por que, em vez de "Salvando..." para sempre.
                (_, Some(_)) if hub_down => {
                    setup.saving = None;
                    setup.error = Some(HUB_DOWN_SAVE.into());
                }
                _ => {}
            }
        }
        if done {
            self.close_setup();
        }
        if let Some(before) = self.preview_after.clone() {
            let open_id = self.snapshot.assistant.open.as_ref().map(|o| o.id.clone());
            if open_id.is_some() && open_id != before {
                self.preview_after = None;
                if let Some(open) = awaiting_cloud_consent(self) {
                    self.consent_preview = Some(open.id.clone());
                }
            }
        }
        // A conversa da previa saiu da tela (fechada, apagada, aprovada em outro lugar): a
        // previa fecha junto, e nunca passa a mostrar outra conversa.
        if self
            .consent_preview
            .as_ref()
            .is_some_and(|id| awaiting_cloud_consent(self).is_none_or(|o| &o.id != id))
        {
            self.consent_preview = None;
        }
    }

    /// O texto que o assistente recebe nesta conta agora; a prévia e o envio usam este mesmo.
    pub(crate) fn current_envelope(&self) -> String {
        self.envelope_now.clone()
    }

    fn compute_envelope(&self) -> String {
        crate::assistant::envelope::render(&crate::assistant::envelope::build(
            &self.analysis,
            &self.scope,
            &self.today,
            self.book,
        ))
    }

    /// So o Resumo (o `grafite`): o modelo depende do mes, da conta e de "Ocultar valores".
    fn rebuild_dashboard(&mut self) {
        self.dashboard = build_grafite(
            &self.scope,
            &self.today,
            self.month(),
            self.book,
            self.hide_values,
        );
    }

    /// Troca a conta. Os filtros de categoria apontam para linhas da conta anterior e a
    /// escolha de categoria/autor nao faz sentido na outra, entao voltam ao padrao.
    fn select_book(&mut self, book: Book) {
        if self.book == book {
            return;
        }
        // A conversa aberta e de uma conta so; ficar aberta na outra deixaria uma resposta
        // chegando para numeros que sumiram da tela. Fechar cancela o que estiver em andamento
        // e a guarda na lista. Aqui (e nao no clique) porque `rebuild` tambem troca de conta
        // quando a comercial some do hub.
        if self
            .snapshot
            .assistant
            .open
            .as_ref()
            .is_some_and(|c| c.book != book)
        {
            send_assistant(self, AssistantCommand::Close);
        }
        self.book = book;
        self.filters = Filters::default();
        self.cat_menu_open = false;
        self.upcoming_expanded = false;
    }

    /// Muda os filtros e recalcula so se mudaram de fato.
    fn set_filters(&mut self, change: impl FnOnce(&mut Filters)) {
        let before = self.filters.clone();
        change(&mut self.filters);
        if self.filters != before {
            self.rebuild();
        }
    }

    /// Fotos dos perfis vivos, decodificadas so quando o data URI muda.
    fn refresh_user_avatars(&mut self) {
        let wanted: Vec<(String, String)> = self
            .snapshot
            .dashboard
            .alive_users()
            .into_iter()
            .filter_map(|user| Some((user.id.clone(), user.avatar.clone()?)))
            .collect();
        self.user_avatars = sync_avatars(std::mem::take(&mut self.user_avatars), wanted);
    }

    /// O sistema trocou (ou disse pela primeira vez) entre escuro e claro.
    fn set_system_dark(&mut self, dark: bool) {
        self.system_dark = dark;
    }

    /// As cores e os rotulos dos graficos estao gravados na geometria em cache do toolkit;
    /// subir o numero faz o toolkit descarta-la antes do proximo quadro.
    fn clear_charts(&self) {
        self.charts_epoch
            .set(self.charts_epoch.get().wrapping_add(1));
    }

    /// Pessoas e fotos a partir do snapshot atual. A foto so e decodificada de novo quando o
    /// data URI muda: decodificar base64 de 60 KB a cada push seria desperdicio.
    pub(crate) fn refresh_people(&mut self) {
        let infos: Vec<DeviceInfo> = self.snapshot.devices.iter().map(DeviceInfo::from).collect();
        self.people = people(&self.snapshot.dashboard, &infos);
        let wanted: Vec<(String, String)> = self
            .people
            .iter()
            .filter_map(|p| Some((p.device_id.clone(), p.avatar_uri.clone()?)))
            .collect();
        self.avatars = sync_avatars(std::mem::take(&mut self.avatars), wanted);
    }

    /// Pessoas que a tela mostra: sem a da remocao pendente (ela some na hora, como no design).
    pub(crate) fn visible_people(&self) -> impl Iterator<Item = &Person> {
        let pending = self
            .pending_removal
            .as_ref()
            .and_then(PendingRemoval::device_id);
        self.people
            .iter()
            .filter(move |p| Some(p.device_id.as_str()) != pending)
    }

    pub(crate) fn device(&self, device_id: &str) -> Option<&DeviceView> {
        self.snapshot
            .devices
            .iter()
            .find(|d| d.device_id == device_id)
    }

    /// Aparelhos removidos que ainda tem dados a decidir, menos o que ja tem um apagar
    /// pendente (some na hora, como a pessoa removida).
    pub(crate) fn orphans(&self) -> Vec<&DeviceView> {
        let pending = self
            .pending_removal
            .as_ref()
            .and_then(PendingRemoval::device_id);
        self.snapshot
            .devices
            .iter()
            .filter(|d| d.is_orphan() && Some(d.device_id.as_str()) != pending)
            .collect()
    }

    /// Envia a remocao (e o apagar) pendente agora (prazo vencido, outra remocao, troca de
    /// tela ou fechamento).
    fn commit_pending(&mut self) {
        if let Some(pending) = self.pending_removal.take() {
            let target_id = pending.target_id;
            let command = match pending.action {
                PendingAction::Remove(erase) => Command::RemoveDevice {
                    device_id: target_id,
                    erase,
                },
                PendingAction::Erase(areas) => Command::EraseDeviceData {
                    device_id: target_id,
                    areas,
                },
                PendingAction::DeleteConversation => {
                    self.deleting.insert(target_id.clone());
                    Command::Assistant(AssistantCommand::Delete(target_id))
                }
            };
            let _ = self.handle.commands.send(command);
        }
    }

    /// Conversa fora da tela: com "Desfazer" pendente ou com o `Delete` ja enviado e ainda nao
    /// refletido no snapshot.
    pub(crate) fn conversation_hidden(&self, id: &str) -> bool {
        self.pending_removal
            .as_ref()
            .and_then(PendingRemoval::conversation_id)
            == Some(id)
            || self.deleting.contains(id)
    }

    /// Fecha o fluxo e cancela o codigo no nucleo. Sempre manda o cancelamento, mesmo sem
    /// codigo no snapshot: um `IssuePairingToken` recem-enviado pode nao ter voltado ainda, e o
    /// canal e ordenado, entao o cancelamento chega depois dele.
    fn close_connect(&mut self) {
        self.connect = None;
        let _ = self.handle.commands.send(Command::CancelPairingToken);
    }

    /// O fluxo esta na tela agora (dialogo ou em tela)? So entao um aparelho novo o encerra.
    fn connect_showing(&self) -> bool {
        connect::dialog_open(self) || connect::shows_in_screen_flow(self)
    }

    /// "Copiado" ainda vale para este campo?
    pub(crate) fn is_copied(&self, target: CopyTarget) -> bool {
        self.copied
            .is_some_and(|(t, until)| t == target && Instant::now() < until)
    }
}

/// Abre ou traz a janela. Com janela: desminimiza e pede foco (clique na bandeja e segunda
/// execucao dao ao processo o direito de foco; se o Windows recusar, o botao pisca).
fn open_or_focus(app: &App) -> Vec<Effect> {
    match app.window {
        Some(_) => vec![Effect::FocusWindow],
        None => vec![Effect::OpenWindow],
    }
}

/// Desligamento com ordem (o `CloseRequested` da 1.0.0): remocao pendente, `Shutdown`,
/// `Stopped` ou 5 s. Idempotente por `closing`.
fn begin_shutdown(app: &mut App) -> Vec<Effect> {
    if app.closing {
        return vec![];
    }
    app.closing = true;
    // Antes do `Shutdown`, no mesmo canal: o nucleo processa em ordem.
    app.commit_pending();
    // Nucleo ja parado (ou morto): nao ha por que esperar o snapshot `Stopped`.
    if app.handle.commands.send(Command::Shutdown).is_err() {
        return vec![Effect::Exit];
    }
    // Garantia: se o nucleo travar, o processo sai mesmo assim em 5 s.
    vec![Effect::After(Duration::from_secs(5), Msg::ForceExit)]
}

/// O `update` do modelo: a transicao e os efeitos que ela pede. Quem executa os efeitos e o
/// toolkit; aqui nada abre janela, grava arquivo nem fala com o sistema.
pub fn update(app: &mut App, message: Msg) -> Vec<Effect> {
    let effects = step(app, message);
    // Sem janela nao chegam quadros: o que comecou agora (um toast que expirou com a janela
    // fechada, por exemplo) termina aqui, e nao aparece pela metade ao reabrir. Um
    // `OpenWindow` no mesmo passo conta como janela: ela abre ja com a animacao andando.
    if app.window.is_none() && !effects.iter().any(|e| matches!(e, Effect::OpenWindow)) {
        app.motion.settle();
    }
    effects
}

/// Uma mensagem: transicao de estado mais a limpeza posterior, devolvendo os efeitos. As
/// mensagens que disparam outras (`ConnectPhone`, atalhos) usam este, e nao `update`: o
/// `settle` so vale depois de todos os efeitos do passo inteiro.
pub(crate) fn step(app: &mut App, message: Msg) -> Vec<Effect> {
    let effects = handle(app, message);
    // Fluxo aberto que deixou de estar na tela (outra tela, "Desfazer" que trouxe um aparelho de
    // volta, nucleo que caiu): fecha e cancela o codigo, que nao pode valer sem ninguem ver.
    if app.connect.is_some() && !app.connect_showing() {
        app.close_connect();
    }
    // Uma vez por mensagem, depois de tudo: a deteccao de "abriu agora" nao depende de qual
    // dos cinco dialogos (ou qual caminho, snapshot incluso) mudou o estado.
    let scene = app.scene();
    app.motion.sync(scene, Instant::now());
    effects
}

fn handle(app: &mut App, message: Msg) -> Vec<Effect> {
    match message {
        Msg::Snapshot(s) => {
            // Aparelho novo enquanto o fluxo esta na tela: o pareamento terminou. Pelos ids ativos
            // (um aparelho removido pode parear de novo) e so com o nucleo ja no ar antes: no
            // primeiro `Running` depois do boot todo aparelho seria "novo".
            let paired = (is_up(&app.snapshot) && app.connect_showing())
                .then(|| {
                    s.devices
                        .iter()
                        // Parear de novo com o mesmo `deviceId` ainda ativo mantem o id (UPSERT)
                        // e so troca o `paired_at`: conta como novo tambem.
                        .find(|d| {
                            !d.revoked
                                && !app.snapshot.devices.iter().any(|o| {
                                    !o.revoked
                                        && o.device_id == d.device_id
                                        && o.paired_at == d.paired_at
                                })
                        })
                        .map(|d| d.name.clone())
                })
                .flatten();
            if let Some(name) = paired {
                // O codigo foi consumido, mas um "Gerar outro codigo" pode ter saido antes do
                // snapshot chegar: cancela mesmo assim, para nenhum codigo ficar valendo.
                app.close_connect();
                app.notice = Some((format!("{name} conectado"), Instant::now() + NOTICE_FOR));
            }
            let stopped = s.status == Status::Stopped;
            let was_failed = matches!(app.snapshot.status, Status::Failed(_));
            let people_changed = !std::sync::Arc::ptr_eq(&s.dashboard, &app.snapshot.dashboard)
                || s.devices != app.snapshot.devices;
            // Apagar que falhou volta com um erro novo: a conversa reaparece junto do aviso.
            let new_error = s.assistant.last_error.is_some()
                && s.assistant.last_error != app.snapshot.assistant.last_error;
            if new_error {
                app.deleting.clear();
            } else {
                let still_there = |id: &String| {
                    s.assistant.conversations.iter().any(|c| &c.id == id)
                        || s.assistant.open.as_ref().is_some_and(|o| &o.id == id)
                };
                app.deleting.retain(still_there);
            }
            app.snapshot = *s;
            app.sync_assistant_dialogs();
            if people_changed {
                app.refresh_people();
                app.rebuild();
            }
            app.sync_tray();
            if stopped && app.closing {
                return vec![Effect::Exit];
            }
            // Boot por `--autostart` que falhou (porta ocupada, banco): sem janela ninguem
            // veria; o tooltip diz "com erro" e a janela abre sozinha. So na transicao para
            // `Failed`: quem fechar a janela depois de ver o erro nao a ve voltar sozinha.
            let failed = matches!(app.snapshot.status, Status::Failed(_));
            // O "Conectar um celular" aberto quando o hub cai fecha: o codigo nao vale mais, e
            // o dialogo nao reaparece sozinho por cima do card de falha.
            if failed && !was_failed && app.connect.is_some() {
                app.close_connect();
            }
            if failed && !was_failed && app.window.is_none() && !app.closing {
                return open_or_focus(app);
            }
            vec![]
        }
        Msg::IssueToken => {
            let _ = app.handle.commands.send(Command::IssuePairingToken);
            vec![]
        }
        Msg::Tick => {
            if app
                .notice
                .as_ref()
                .is_some_and(|(_, until)| Instant::now() >= *until)
            {
                app.notice = None;
            }
            if app
                .pending_removal
                .as_ref()
                .is_some_and(|p| Instant::now() >= p.deadline)
            {
                app.commit_pending();
            }
            if app
                .confirm_delete_all
                .is_some_and(|deadline| Instant::now() >= deadline)
            {
                app.confirm_delete_all = None;
            }
            app.set_today(local_today(app.offset));
            // Tooltip ou icone que a bandeja recusou antes tenta de novo; sem pendencia, nada vai
            // ao sistema.
            app.sync_tray();
            // No Windows a troca do tema do sistema nao chega pelo toolkit (ver
            // `desktop::appearance`): com "Sistema" ativo, o tique confere.
            if app.theme == ThemeMode::System
                && let Some(dark) = app.desktop.system_dark()
                && dark != app.system_dark
            {
                app.set_system_dark(dark);
            }
            vec![]
        }
        Msg::PrevMonth => {
            if app.month_idx > 0 {
                app.month_idx -= 1;
                app.upcoming_expanded = false;
                app.rebuild();
            }
            vec![]
        }
        Msg::NextMonth => {
            if app.month_idx + 1 < app.months.len() {
                app.month_idx += 1;
                app.upcoming_expanded = false;
                app.rebuild();
            }
            vec![]
        }
        Msg::SelectBook(book) => {
            // Comercial so com a conta existindo: a mensagem vem de um botao que so existe
            // nesse caso, mas o estado nao deve depender disso.
            let allowed = book == Book::Personal || app.snapshot.dashboard.has_business_book();
            if allowed && book != app.book {
                app.select_book(book);
                app.rebuild();
            }
            vec![]
        }
        Msg::ExpandUpcoming => {
            app.upcoming_expanded = true;
            vec![]
        }
        Msg::CloseRequested => {
            // Sem bandeja (Linux/macOS, ou bandeja que nao subiu) nao haveria como voltar:
            // fechar desliga, como na 1.0.0.
            if !app.desktop.has_tray() {
                return begin_shutdown(app);
            }
            // Com bandeja, fechar e so fechar: o hub segue sincronizando. A remocao pendente
            // sai agora para nao depender de a janela voltar.
            app.commit_pending();
            // O "Conectar um celular" fecha junto: o codigo nao fica valendo sem janela, e o
            // dialogo nao reaparece sozinho quando ela voltar.
            if app.connect.is_some() {
                app.close_connect();
            }
            // Os dialogos do assistente tambem: a chave digitada nao fica na memoria de uma
            // janela fechada, e a previa nao reaparece sozinha.
            app.close_setup();
            app.consent_preview = None;
            // Os outros dialogos tambem, como na troca de tela: o envelope e a remocao nao
            // reaparecem sozinhos quando a janela voltar, nem a previa agendada para depois.
            app.preview_after = None;
            app.envelope_preview = None;
            app.confirm_remove = None;
            vec![Effect::CloseWindow]
        }
        Msg::Desktop(DesktopEvent::Quit) => begin_shutdown(app),
        Msg::Desktop(DesktopEvent::OpenWindow) => open_or_focus(app),
        Msg::Desktop(DesktopEvent::ConnectPhone) => {
            // Desligando: nao abre janela nem gera codigo que ninguem vai usar.
            if app.closing {
                return vec![];
            }
            // A janela primeiro: o `ConnectPhone` le o estado da tela, nao da janela, entao
            // vale com ela fechada, e o dialogo ja esta montado quando ela aparece.
            let open = open_or_focus(app);
            // Com um dialogo aberto (remocao, previa, assistente) a pessoa esta no meio de algo:
            // so traz a janela, sem trocar de tela por baixo dele nem fecha-lo.
            if app.any_dialog_open() {
                return open;
            }
            let connect = step(app, Msg::ConnectPhone);
            open.into_iter().chain(connect).collect()
        }
        Msg::Desktop(DesktopEvent::CopyAddress) => {
            // Sem endereco (sem rede local) nao ha o que copiar; o icone ja esta com o ponto
            // de atencao e o tooltip diz "sem rede".
            if let Some(url) = connect::hub_url(&app.snapshot)
                && let Err(err) = app.desktop.copy_text(&url)
            {
                tracing::warn!("copiar endereco pela bandeja: {err}");
            }
            vec![]
        }
        // O porque do "despertador" esta em `effects::task`.
        Msg::ForceExit => vec![Effect::Exit],
        Msg::WindowOpened(id) => {
            if app.window == Some(id) {
                vec![Effect::QueryMaximized]
            } else {
                vec![]
            }
        }
        Msg::WindowClosed(id) => {
            if app.window == Some(id) {
                app.window = None;
                app.maximized = false;
                app.window_size = INITIAL_SIZE;
                // A geometria em cache pertence ao compositor, que morreu com a janela.
                app.clear_charts();
                // Sem janela nao ha quadros: o que andava termina aqui, nao ao reabrir.
                app.motion.settle();
            }
            vec![]
        }
        Msg::TitlePressed => {
            app.title_pressed = true;
            vec![]
        }
        Msg::TitleReleased => {
            app.title_pressed = false;
            vec![]
        }
        Msg::TitleMoved => {
            if !app.title_pressed {
                return vec![];
            }
            // O laco de movimento do sistema consome o soltar do botao: sem zerar aqui, o
            // proximo movimento sem botao comecaria outro arrasto.
            app.title_pressed = false;
            vec![Effect::StartDrag]
        }
        Msg::Minimize => vec![Effect::Minimize],
        Msg::ToggleMaximize => {
            app.title_pressed = false;
            vec![Effect::ToggleMaximize]
        }
        Msg::Maximized(maximized) => {
            app.maximized = maximized;
            vec![]
        }
        Msg::Resized(size) => {
            app.window_size = size;
            vec![Effect::QueryMaximized]
        }
        // So de um toolkit: o adaptador dele executa o arrasto antes do modelo, porque a borda e do
        // sistema de janelas dele e nao e um `Effect` (no GPUI nao existe esta mensagem).
        Msg::Resize(_) => vec![],
        Msg::SelectScreen(screen) => {
            let top = vec![Effect::ScrollToTop];
            // Clicar na tela em que ja se esta so volta ao topo: a pessoa nao saiu dela, entao
            // a remocao pendente segue com o "Desfazer" e a chave digitada fica no campo.
            if screen == app.screen {
                return top;
            }
            app.commit_pending();
            // Sair de Aparelhos com o fluxo aberto e fecha-lo: o codigo nao fica valendo sem
            // ninguem olhando para ele.
            if app.connect.is_some() {
                app.close_connect();
            }
            // O dialogo do assistente nao sobrevive a troca de tela (o scrim ja impede o
            // clique; isto cobre qualquer outro caminho), e a chave digitada vai junto. A
            // previa tambem: ela e da conversa da Analise, e nao reaparece sozinha ao voltar.
            app.close_setup();
            app.consent_preview = None;
            app.preview_after = None;
            app.screen = screen;
            app.cat_menu_open = false;
            top
        }
        Msg::ConnectPhone => {
            let task = step(app, Msg::SelectScreen(Screen::Connection));
            // Hub que nao subiu: a casca mostra o card de falha no lugar de Aparelhos, e um
            // codigo nao teria quem o emitir. O fluxo nem abre.
            if shell::failure(app).is_some() {
                return task;
            }
            let modal = !connect::in_screen_case(app);
            if app.connect.is_none_or(|f| f.modal != modal) {
                app.connect = Some(ConnectFlow::new(modal));
            }
            task
        }
        Msg::ConnectNext => {
            let modal = !connect::in_screen_case(app);
            let up = is_up(&app.snapshot);
            let flow = app.connect.get_or_insert_with(|| ConnectFlow::new(modal));
            if let Some(step) = flow.step.next() {
                flow.step = step;
                // O codigo nasce ao entrar no passo, sem clique extra. Com o nucleo fora do ar o
                // passo mostra "Gerar codigo" desabilitado ate ele subir.
                if step == ConnectStep::Pair && up {
                    let _ = app.handle.commands.send(Command::IssuePairingToken);
                }
            }
            vec![]
        }
        Msg::ConnectBack => {
            let mut left_pair = false;
            if let Some(flow) = app.connect.as_mut()
                && let Some(step) = flow.step.back()
            {
                left_pair = flow.step == ConnectStep::Pair;
                flow.step = step;
            }
            // Sair do passo 3 tira o QR da tela: o codigo deixa de valer; voltar ao passo 3
            // pede outro.
            if left_pair {
                let _ = app.handle.commands.send(Command::CancelPairingToken);
            }
            vec![]
        }
        Msg::ConnectOs(os) => {
            let modal = !connect::in_screen_case(app);
            app.connect
                .get_or_insert_with(|| ConnectFlow::new(modal))
                .os = os;
            vec![]
        }
        Msg::ConnectClose => {
            app.close_connect();
            vec![]
        }
        Msg::ConfigureAssistant => {
            // O dialogo e uma camada da janela: abre por cima de Ajustes ou da Analise, sem
            // trocar de tela.
            app.open_setup();
            vec![]
        }
        Msg::ToggleCategoryMenu => {
            app.cat_menu_open = !app.cat_menu_open;
            vec![]
        }
        Msg::CloseCategoryMenu => {
            app.cat_menu_open = false;
            vec![]
        }
        Msg::PickCategory(category) => {
            app.cat_menu_open = false;
            app.set_filters(|f| f.category = category);
            vec![]
        }
        Msg::ToggleAuthor(key) => {
            // Todos os chips da tela: perfis vivos e, se aparecer, "Sem autor".
            let mut all: Vec<String> = app
                .transactions
                .authors
                .iter()
                .map(|a| a.id.clone())
                .collect();
            if app.transactions.unauthored_chip {
                all.push(NONE_KEY.to_string());
            }
            app.set_filters(|f| f.toggle_author(&key, &all));
            vec![]
        }
        Msg::AllAuthors => {
            app.set_filters(|f| f.authors.clear());
            vec![]
        }
        Msg::SetType(kind) => {
            app.set_filters(|f| f.kind = kind);
            vec![]
        }
        Msg::ClearFilters => {
            app.cat_menu_open = false;
            app.set_filters(Filters::clear);
            vec![]
        }
        Msg::ToggleHideValues => {
            app.hide_values = !app.hide_values;
            app.rebuild_valued();
            // Os rotulos em R$ dos graficos estao gravados na geometria em cache.
            app.clear_charts();
            vec![Effect::SavePrefs(app.prefs())]
        }
        Msg::SetTheme(mode) => {
            if mode == app.theme {
                return vec![];
            }
            app.theme = mode;
            // Ao escolher "Sistema" o valor guardado pode ser velho (o tique so consulta com
            // "Sistema" ativo): le de novo antes de desenhar.
            if mode == ThemeMode::System
                && let Some(dark) = app.desktop.system_dark()
            {
                app.system_dark = dark;
            }
            vec![Effect::SavePrefs(app.prefs())]
        }
        Msg::SystemTheme(dark) => {
            // Onde o registro responde (Windows), so ele manda: o aviso do winit la chega
            // atrasado ou com o valor anterior (ver `desktop::appearance`), e aceita-lo fazia
            // o tema piscar entre o velho e o novo, sobretudo com alto contraste.
            if app.desktop.system_dark().is_some() {
                return vec![];
            }
            app.set_system_dark(dark);
            vec![]
        }
        Msg::SetAutostart(enabled) => {
            if app.autostart == Some(enabled) {
                return vec![];
            }
            match app.desktop.set_autostart(enabled) {
                Ok(()) => {
                    app.autostart = Some(enabled);
                    app.autostart_pref = Some(enabled);
                    app.autostart_failed = false;
                    vec![Effect::SavePrefs(app.prefs())]
                }
                Err(err) => {
                    tracing::warn!("iniciar com o Windows: {err}");
                    app.autostart_failed = true;
                    vec![]
                }
            }
        }
        Msg::Copy(target) => {
            let text = match target {
                CopyTarget::Address => connect::hub_url(&app.snapshot),
                CopyTarget::Guide => connect::guide_url(&app.snapshot),
                CopyTarget::DataDir => Some(app.snapshot.data_dir.display().to_string()),
                CopyTarget::LogsDir => Some(
                    config::logs_dir(&app.snapshot.data_dir)
                        .display()
                        .to_string(),
                ),
                CopyTarget::ReleasesUrl => Some(config::RELEASES_URL.to_string()),
                // Sai por `CopyReply`, que sabe qual resposta.
                CopyTarget::Reply => None,
            };
            let Some(text) = text else {
                return vec![];
            };
            app.copied = Some((target, Instant::now() + COPIED_FOR));
            // O toolkit nao devolve falha da area de transferencia; o rotulo vira "Copiado" mesmo
            // assim (limite aceito na spec).
            vec![
                Effect::CopyToClipboard(text),
                Effect::After(COPIED_FOR, Msg::CopyExpired),
            ]
        }
        Msg::OpenReleases => {
            // O navegador do usuario vai ao GitHub por um clique dele; este processo nao abre
            // socket. A falha volta como `OpenUrlFailed` para virar estado na tela.
            vec![Effect::OpenUrl(config::RELEASES_URL)]
        }
        Msg::OpenUrlFailed => {
            app.releases_open_failed = true;
            vec![]
        }
        Msg::CopyReply(id) => {
            let Some(m) = analysis::open_conversation(app)
                .and_then(|o| o.messages.iter().find(|m| m.id == id))
            else {
                return vec![];
            };
            // O mesmo texto da tela: mascarado com o olho ligado (decisao registrada em
            // `MaskedReply::copy`). Calculado aqui, pela mesma regra que o desenho usa, para o
            // modelo nao depender do cache que o toolkit guarda.
            let copy = analysis::masked_reply(&m.content, app.hide_values).copy;
            app.copied = Some((CopyTarget::Reply, Instant::now() + COPIED_FOR));
            app.copied_reply = Some(id);
            vec![
                Effect::CopyToClipboard(copy),
                Effect::After(COPIED_FOR, Msg::CopyExpired),
            ]
        }
        Msg::AnalysisTab(tab) => {
            app.analysis_tab = tab;
            vec![]
        }
        Msg::CopyExpired => {
            // Um segundo clique adiou o prazo: o timer do primeiro nao apaga o do segundo.
            if app.copied.is_some_and(|(_, until)| Instant::now() >= until) {
                app.copied = None;
            }
            vec![]
        }
        Msg::HoverRow(id) => {
            app.hover_row = Some(id);
            vec![]
        }
        Msg::UnhoverRow(id) => {
            // Saida de uma linha pode chegar depois da entrada na vizinha.
            if app.hover_row.as_ref() == Some(&id) {
                app.hover_row = None;
            }
            vec![]
        }
        Msg::AskRemove(device_id) => {
            app.confirm_remove = Some(RemoveDialog {
                device_id,
                step: RemoveStep::Choose,
                selected: AreaSet::default(),
            });
            vec![]
        }
        Msg::ToggleArea(area) => {
            let enabled = app
                .confirm_remove
                .as_ref()
                .and_then(|d| app.device(&d.device_id))
                .is_some_and(|d| d.data.items(area) > 0);
            if let Some(dialog) = app.confirm_remove.as_mut().filter(|_| enabled) {
                dialog.selected.toggle(area);
            }
            vec![]
        }
        Msg::SelectAllAreas => {
            let with_data: AreaSet = app
                .confirm_remove
                .as_ref()
                .and_then(|d| app.device(&d.device_id))
                .map(|d| {
                    Area::ALL
                        .into_iter()
                        .filter(|a| d.data.items(*a) > 0)
                        .collect()
                })
                .unwrap_or_default();
            if let Some(dialog) = app.confirm_remove.as_mut() {
                dialog.selected = with_data;
            }
            vec![]
        }
        Msg::SelectNoAreas => {
            if let Some(dialog) = app.confirm_remove.as_mut() {
                dialog.selected = AreaSet::default();
            }
            vec![]
        }
        Msg::RemoveContinue => {
            let Some(dialog) = app.confirm_remove.as_mut() else {
                return vec![];
            };
            if dialog.selected.is_empty() {
                return step(app, Msg::ConfirmRemove);
            }
            dialog.step = RemoveStep::Confirm;
            vec![]
        }
        Msg::RemoveBack => {
            if let Some(dialog) = app.confirm_remove.as_mut() {
                dialog.step = RemoveStep::Choose;
            }
            vec![]
        }
        Msg::CancelRemove => {
            app.confirm_remove = None;
            vec![]
        }
        Msg::ShowEnvelope => {
            app.envelope_preview = Some(app.current_envelope());
            vec![]
        }
        Msg::CloseEnvelope => {
            app.envelope_preview = None;
            vec![]
        }
        Msg::ConfirmRemove => {
            let Some(dialog) = app.confirm_remove.take() else {
                return vec![];
            };
            // Marcar algo exige ter passado pelo passo 2 (vermelho, com as contagens): sem
            // isso, uma mensagem fora de ordem apagaria sem a confirmacao explicita.
            if !dialog.selected.is_empty() && dialog.step != RemoveStep::Confirm {
                app.confirm_remove = Some(RemoveDialog {
                    step: RemoveStep::Confirm,
                    ..dialog
                });
                return vec![];
            }
            let Some(device) = app.device(&dialog.device_id) else {
                return vec![];
            };
            let (orphan, data, device_name) =
                (device.revoked, device.data.clone(), device.name.clone());
            if orphan && dialog.selected.is_empty() {
                // "Manter tudo" de aparelho ja removido: nada destrutivo, entra na hora.
                let _ = app
                    .handle
                    .commands
                    .send(Command::KeepDeviceData(dialog.device_id));
                return vec![];
            }
            let base = if orphan {
                format!("Dados de {device_name}")
            } else {
                app.people
                    .iter()
                    .find(|p| p.device_id == dialog.device_id)
                    .map_or_else(|| "Aparelho removido do hub".to_string(), removal_toast)
            };
            // So uma remocao desfazivel por vez: a anterior vai agora.
            app.commit_pending();
            let toast = crate::ui_model::remove::toast_text(&base, &data, dialog.selected);
            let action = if orphan {
                PendingAction::Erase(dialog.selected)
            } else {
                PendingAction::Remove(dialog.selected)
            };
            app.pending_removal = Some(PendingRemoval {
                target_id: dialog.device_id,
                toast,
                deadline: Instant::now() + UNDO_FOR,
                action,
            });
            vec![]
        }
        Msg::UndoRemove => {
            // Nada foi enviado ao nucleo: desfazer e so esquecer. Vale para a remocao e para
            // o apagar, que saem juntos no mesmo comando, e para a conversa.
            app.pending_removal = None;
            vec![]
        }
        Msg::DecideOrphan => {
            let first = app.orphans().first().map(|d| d.device_id.clone());
            if let Some(device_id) = first {
                return step(app, Msg::AskRemove(device_id));
            }
            vec![]
        }
        Msg::PostponeOrphans => {
            app.orphans_postponed = true;
            vec![]
        }
        Msg::StartConversation => {
            let settings = &app.snapshot.assistant.settings;
            let Some(destination) = settings
                .destination()
                .filter(|_| settings.model().is_some())
                // Nuvem sem chave so falharia na primeira resposta; a configuracao explica.
                .filter(|d| !d.is_cloud() || app.snapshot.assistant.has_key)
            else {
                app.open_setup();
                return vec![];
            };
            app.chat_input.clear();
            // Na janela estreita a conversa nova fica a vista, e nao atras do segmentado.
            app.analysis_tab = AnalysisTab::Chat;
            let title = format!("Análise de {}", month_label(&app.analysis.month));
            // O envelope sai inteiro, nunca o mascarado: "Ocultar valores" e so de quem olha.
            let envelope = app.current_envelope();
            send_assistant(
                app,
                AssistantCommand::NewConversation {
                    book: app.book,
                    title,
                    envelope,
                },
            );
            // A nuvem so envia depois da previa aprovada na tela (`ApproveCloudSend`); o modelo
            // local nao precisa de consentimento e ja pede a resposta de abertura.
            if destination.is_cloud() {
                // A previa abre em dialogo quando a conversa nova chegar no snapshot.
                app.preview_after =
                    Some(app.snapshot.assistant.open.as_ref().map(|o| o.id.clone()));
            } else {
                send_assistant(app, AssistantCommand::Resend);
            }
            vec![]
        }
        Msg::ChatInput(text) => {
            app.chat_input = text;
            vec![]
        }
        Msg::ChatSubmit => {
            let text = app.chat_input.trim().to_string();
            // Sem conversa visivel (nenhuma, de outra conta ou escondida pelo apagar) a
            // pergunta iria para uma conversa que a pessoa nao esta vendo.
            if text.is_empty()
                || analysis::open_conversation(app).is_none()
                || app.snapshot.assistant.busy
                || awaiting_cloud_consent(app).is_some()
            {
                return vec![];
            }
            app.chat_input.clear();
            send_assistant(app, AssistantCommand::Send(text));
            vec![]
        }
        Msg::ChatStop => {
            send_assistant(app, AssistantCommand::Cancel);
            vec![]
        }
        Msg::ChatRetry => {
            if !app.snapshot.assistant.busy
                && awaiting_cloud_consent(app).is_none()
                && analysis::open_conversation(app).is_some()
            {
                send_assistant(app, AssistantCommand::Resend);
            }
            vec![]
        }
        Msg::OpenConversation(id) => {
            app.analysis_tab = AnalysisTab::Chat;
            send_assistant(app, AssistantCommand::Open(id));
            vec![]
        }
        Msg::CloseConversation => {
            send_assistant(app, AssistantCommand::Close);
            vec![]
        }
        Msg::DeleteConversation(id) => {
            send_assistant(app, AssistantCommand::Delete(id));
            vec![]
        }
        Msg::AskDeleteConversation(id) => {
            if app
                .pending_removal
                .as_ref()
                .and_then(PendingRemoval::conversation_id)
                == Some(id.as_str())
            {
                return vec![];
            }
            // So um "Desfazer" por vez: o anterior (de conversa ou de aparelho) vai agora, e o
            // toast passa a ser desta.
            app.commit_pending();
            if app.hover_row.as_deref() == Some(id.as_str()) {
                app.hover_row = None;
            }
            app.pending_removal = Some(PendingRemoval {
                target_id: id,
                toast: "Conversa apagada".to_string(),
                deadline: Instant::now() + UNDO_FOR,
                action: PendingAction::DeleteConversation,
            });
            vec![]
        }
        Msg::DeleteAllConversations => {
            let armed = app
                .confirm_delete_all
                .take()
                .is_some_and(|deadline| Instant::now() < deadline);
            if armed {
                // A conversa com "Desfazer" pendente some junto; o pendente sai antes para nao
                // chegar depois do `DeleteAll` com um id que ja nao existe. Remocao de aparelho
                // pendente nao tem nada com isso e continua com o seu "Desfazer".
                if app
                    .pending_removal
                    .as_ref()
                    .is_some_and(|p| p.conversation_id().is_some())
                {
                    app.commit_pending();
                }
                send_assistant(app, AssistantCommand::DeleteAll);
            } else {
                app.confirm_delete_all = Some(Instant::now() + DELETE_ALL_ARMED_FOR);
            }
            vec![]
        }
        Msg::CloseAssistantSetup => {
            app.close_setup();
            vec![]
        }
        Msg::SetupChoice(choice) => {
            if let Some(setup) = app.assistant_setup.as_mut()
                && setup.saving.is_none()
            {
                setup.draft.choice = choice;
            }
            vec![]
        }
        Msg::SetupNext => {
            let view = &app.snapshot.assistant;
            let mut test = None;
            if let Some(setup) = app.assistant_setup.as_mut()
                && setup.saving.is_none()
                && setup.draft.choice != ProviderChoice::None
            {
                setup.step = SetupStep::Details;
                // Entrar no passo do Ollama ja testa a porta, se ninguem testou esta: sem a
                // lista o passo mostraria so "clique em Testar conexao".
                if setup.draft.choice == ProviderChoice::Local
                    && !view.listing
                    && setup
                        .draft
                        .port()
                        .is_some_and(|p| view.models_port != Some(p))
                {
                    test = setup.draft.port();
                }
            }
            if let Some(port) = test {
                send_assistant(app, AssistantCommand::ListModels(port));
            }
            vec![]
        }
        Msg::SetupBack => {
            if let Some(setup) = app.assistant_setup.as_mut()
                && setup.saving.is_none()
            {
                setup.step = SetupStep::Where;
            }
            vec![]
        }
        Msg::SetupPort(text) => {
            if let Some(setup) = app.assistant_setup.as_mut() {
                setup.draft.port_input = text
                    .chars()
                    .filter(char::is_ascii_digit)
                    .take(PORT_DIGITS)
                    .collect();
            }
            vec![]
        }
        Msg::SetupTest => {
            // A lista vem da porta que esta no campo, mesmo sem gravar: e o teste dela.
            let port = app
                .assistant_setup
                .as_ref()
                .and_then(|s| s.draft.port())
                .filter(|_| !app.snapshot.assistant.listing);
            if let Some(port) = port {
                send_assistant(app, AssistantCommand::ListModels(port));
            }
            vec![]
        }
        Msg::SetupLocalModel(model) => {
            if let Some(setup) = app.assistant_setup.as_mut() {
                setup.draft.local_model = Some(model);
            }
            vec![]
        }
        Msg::SetupCloudModel(text) => {
            if let Some(setup) = app.assistant_setup.as_mut() {
                setup.draft.cloud_model = text;
            }
            vec![]
        }
        Msg::SetupDeny(deny) => {
            if let Some(setup) = app.assistant_setup.as_mut() {
                setup.draft.deny = deny;
            }
            vec![]
        }
        Msg::SetupFinish => {
            let Some(setup) = app.assistant_setup.as_ref() else {
                return vec![];
            };
            if !assistant_setup::can_finish(app, setup) {
                return vec![];
            }
            let saved = &app.snapshot.assistant.settings;
            let Some(settings) =
                crate::ui_model::assistant_setup::settings_to_save(saved, &setup.draft)
            else {
                return vec![];
            };
            // `take`: o campo esvazia no mesmo instante. A chave so vai com a nuvem: escolher o
            // Ollama com uma chave digitada no passo da nuvem nao a grava.
            let key = std::mem::take(&mut app.key_input);
            let key = (setup.draft.choice == ProviderChoice::Cloud && !key.trim().is_empty())
                .then(|| Secret::new(key));
            if key.is_none() && settings == *saved {
                // Nada a gravar: o gravado ja e isto.
                app.close_setup();
                return vec![];
            }
            // Hub fora do ar: o comando nao teria quem o grave. A chave digitada ja saiu do
            // campo (`take`) e nao volta: digitar de novo e mais seguro que guarda-la.
            if matches!(app.snapshot.status, Status::Failed(_) | Status::Stopped) {
                if let Some(setup) = app.assistant_setup.as_mut() {
                    setup.error = Some(HUB_DOWN_SAVE.into());
                }
                return vec![];
            }
            let expected = app
                .snapshot
                .assistant
                .configure_result
                .as_ref()
                .map_or(0, |(n, _)| *n)
                .wrapping_add(1);
            if let Some(setup) = app.assistant_setup.as_mut() {
                setup.saving = Some(expected);
                setup.error = None;
            }
            send_assistant(app, AssistantCommand::Configure { settings, key });
            vec![]
        }
        Msg::KeyInput(key) => {
            app.key_input = key.expose().to_string();
            vec![]
        }
        Msg::RemoveKey => {
            if app
                .assistant_setup
                .as_ref()
                .is_some_and(|s| s.saving.is_some())
            {
                return vec![];
            }
            app.key_input.clear();
            send_assistant(app, AssistantCommand::SaveKey(Secret::new(String::new())));
            vec![]
        }
        Msg::ShowCloudPreview => {
            if let Some(open) = awaiting_cloud_consent(app) {
                app.consent_preview = Some(open.id.clone());
            }
            vec![]
        }
        Msg::CancelPreview => {
            // So o "Cancelar": a abertura recem-criada, sem nada saido, some; o resto so fecha
            // a previa (`cancel_preview_message`).
            let busy = app.snapshot.assistant.busy;
            let cancel = analysis::shown_consent(app)
                .and_then(|open| analysis::cancel_preview_message(open, busy));
            app.consent_preview = None;
            match cancel {
                Some(message) => step(app, message),
                None => vec![],
            }
        }
        Msg::ClosePreview => {
            // Esc e o scrim: um gesto rapido nao apaga nada. A conversa continua esperando,
            // com "Ver previa e enviar" no pe.
            app.consent_preview = None;
            vec![]
        }
        Msg::ApproveCloudSend => {
            // O id vai junto: o nucleo so libera se for a conversa ainda aberta. E so a conversa
            // que a previa mostra: um clique que chega depois de abrir outra nao aprova esta.
            if let Some(open) = analysis::shown_consent(app) {
                let id = open.id.clone();
                app.consent_preview = None;
                send_assistant(app, AssistantCommand::ApproveCloudSend(id));
            }
            vec![]
        }
        Msg::KeyPressed(press) => match keys::shortcut(app, &press) {
            Some(message) => step(app, message),
            None => vec![],
        },
        Msg::FocusNext => vec![Effect::FocusNext],
        Msg::FocusPrevious => vec![Effect::FocusPrevious],
        Msg::Noop => vec![],
        Msg::Frame(at) => {
            app.motion.frame(at);
            vec![]
        }
        Msg::ToggleReduceMotion => {
            let reduce = !app.motion.reduce();
            app.motion.set_reduce(reduce);
            vec![Effect::SavePrefs(app.prefs())]
        }
    }
}

/// Quanto tempo "Apagar todas" fica armado depois do primeiro clique.
pub(crate) const DELETE_ALL_ARMED_FOR: Duration = Duration::from_secs(3);

/// Por que o dialogo nao gravou com o nucleo fora do ar.
pub(crate) const HUB_DOWN_SAVE: &str =
    "O hub não está rodando, então a configuração não foi gravada.";

/// A porta mais alta (65535) tem 5 digitos.
const PORT_DIGITS: usize = 5;

/// Pedido ao assistente no nucleo. Falha de envio (nucleo ja desligado) e ignorada como nos
/// outros comandos: a janela fecha junto.
fn send_assistant(app: &App, command: AssistantCommand) {
    let _ = app.handle.commands.send(Command::Assistant(command));
}

/// "Ana foi removida do hub". O genero e uma heuristica (a do prototipo): nome de pessoa
/// terminado em "a" e feminino; nome de aparelho (sem perfil conhecido) e sempre masculino
/// ("o Pixel 7").
pub(crate) fn removal_toast(person: &Person) -> String {
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

/// Fotos de um conjunto (aparelhos ou perfis) a partir dos pares `(chave, data URI)`: reaproveita
/// a foto quando o URI nao mudou, e so decodifica o que e novo ou trocou. A textura do toolkit
/// e refeita em `IcedSide`, a partir destes bytes.
fn sync_avatars(
    mut old: HashMap<String, Avatar>,
    wanted: Vec<(String, String)>,
) -> HashMap<String, Avatar> {
    let mut avatars = HashMap::new();
    for (key, uri) in wanted {
        match old.remove(&key) {
            Some(prev) if prev.uri == uri => {
                avatars.insert(key, prev);
            }
            _ => {
                let Some(bytes) = avatar_bytes(&uri) else {
                    continue;
                };
                avatars.insert(key, Avatar { uri, bytes });
            }
        }
    }
    avatars
}

#[cfg(test)]
pub(crate) mod tests {
    use tokio::sync::{mpsc, watch};

    use crate::dashboard::money::{MASK, leaks};
    use crate::desktop::{Fake, Headless};
    use crate::store::erase::DeviceData;

    use super::*;

    /// App de teste com canais de verdade e a integracao do sistema escolhida: o receptor de
    /// comandos fica com o teste.
    pub(crate) fn app_with_desktop(
        snapshot: Snapshot,
        prefs_dir: PathBuf,
        desktop: Rc<dyn Desktop>,
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
        let app = App::new(
            handle,
            UtcOffset::UTC,
            UiPrefs::default(),
            prefs_dir,
            desktop,
        );
        (app, cmd_rx, snap_tx)
    }

    /// Sem bandeja: o comportamento da 1.0.0 (fechar desliga). Os testes antigos usam este.
    pub(crate) fn app_with(
        snapshot: Snapshot,
        prefs_dir: PathBuf,
    ) -> (
        App,
        mpsc::UnboundedReceiver<Command>,
        watch::Sender<Snapshot>,
    ) {
        app_with_desktop(snapshot, prefs_dir, Rc::new(Headless))
    }

    /// Com bandeja (o `Fake`) e janela aberta.
    pub(crate) fn with_tray(
        dir: &std::path::Path,
    ) -> (App, Rc<Fake>, mpsc::UnboundedReceiver<Command>) {
        let fake = Rc::new(Fake::with_tray());
        let (mut app, rx, _tx) =
            app_with_desktop(Snapshot::default(), dir.to_path_buf(), fake.clone());
        app.window = Some(WindowId(1));
        (app, fake, rx)
    }

    /// Espera a thread de `save_prefs` gravar o que o teste quer ver.
    pub(crate) fn wait_prefs(dir: &std::path::Path, done: impl Fn(&UiPrefs) -> bool) {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !done(&UiPrefs::load(dir)) {
            assert!(std::time::Instant::now() < deadline, "ui.json nao gravado");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn alternar_ocultar_refaz_reservas_e_nao_mexe_no_que_vai_ao_assistente() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with_reserves(dir.path());
        let _ = update(&mut app, Msg::SelectScreen(Screen::Transactions));
        let shown_reserves = app.reserves.clone();
        let envelope = app.current_envelope();
        let findings = format!("{:?}", app.analysis);
        assert!(leaks(&format!("{shown_reserves:#?}")).is_some());

        // Alternar em Lancamentos refaz Reservas tambem: trocar de tela depois chega mascarado.
        let _ = update(&mut app, Msg::ToggleHideValues);
        assert_eq!(leaks(&format!("{:#?}", app.reserves)), None);
        assert_eq!(
            app.reserves.movements.first().map(|m| m.amount.as_str()),
            Some(MASK)
        );
        assert_eq!(app.reserves.totals, shown_reserves.totals);
        // O envelope e os achados guardam os valores reais: a Analise mascara so ao desenhar.
        // `rebuild` com o olho ligado e o caminho de snapshot novo, mes ou conta: recalcula os
        // achados e o envelope, que tem de sair igual (a meta da emergencia no envelope e o
        // "Faltam R$ ..." do achado vem do `emergency_view`, que eles pedem sempre visivel).
        assert!(findings.contains("R$ "), "{findings}");
        assert!(envelope.contains("R$ "), "{envelope}");
        app.rebuild();
        assert_eq!(app.current_envelope(), envelope);
        assert!(!envelope.contains('•'));
        assert_eq!(format!("{:?}", app.analysis), findings);
        assert_eq!(leaks(&format!("{:#?}", app.reserves)), None);
        assert_eq!(leaks(&format!("{:#?}", app.transactions)), None);

        let _ = update(&mut app, Msg::ToggleHideValues);
        assert_eq!(app.reserves, shown_reserves);
    }

    #[test]
    fn nasce_com_valores_ocultos_das_preferencias() {
        let dir = tempfile::tempdir().unwrap();
        let (cmd_tx, _rx) = mpsc::unbounded_channel();
        let (_tx, snap_rx) = watch::channel(Snapshot {
            dashboard: std::sync::Arc::new(crate::dashboard::fixtures()),
            ..Snapshot::default()
        });
        let app = App::new(
            HubHandle {
                commands: cmd_tx,
                snapshot: snap_rx,
            },
            UtcOffset::UTC,
            UiPrefs {
                hide_values: true,
                ..UiPrefs::default()
            },
            dir.path().to_path_buf(),
            Rc::new(Headless),
        );
        assert!(app.hide_values);
        assert!(app.dashboard.hidden);
        assert_eq!(app.dashboard.header.hide_toggle, "Mostrar valores");
    }

    #[test]
    fn barra_de_titulo_so_arrasta_depois_de_pressionar() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        // Mover sem botao: nada.
        let _ = update(&mut app, Msg::TitleMoved);
        assert!(!app.title_pressed);
        let _ = update(&mut app, Msg::TitlePressed);
        assert!(app.title_pressed);
        // Mover com botao: arrasta uma vez e esquece o botao (o sistema engole o soltar).
        let _ = update(&mut app, Msg::TitleMoved);
        assert!(!app.title_pressed);
        // Duplo clique tambem esquece o botao.
        let _ = update(&mut app, Msg::TitlePressed);
        let _ = update(&mut app, Msg::ToggleMaximize);
        assert!(!app.title_pressed);
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
        let _ = update(&mut app, Msg::Copy(CopyTarget::Address));
        assert!(app.is_copied(CopyTarget::Address));
        assert!(!app.is_copied(CopyTarget::Guide));
        // Timer chegou antes do prazo (outro clique adiou): continua.
        let _ = update(&mut app, Msg::CopyExpired);
        assert!(app.copied.is_some());
        app.copied = Some((CopyTarget::Address, Instant::now()));
        let _ = update(&mut app, Msg::CopyExpired);
        assert!(app.copied.is_none());
    }

    #[test]
    fn copiar_as_pastas_funciona_sem_rede() {
        let dir = tempfile::tempdir().unwrap();
        let snap = Snapshot {
            data_dir: PathBuf::from("C:/x/HubFinance/data"),
            ..Snapshot::default()
        };
        assert!(snap.addresses.is_empty());
        let (mut app, _rx, _tx) = app_with(snap, dir.path().to_path_buf());
        let _ = update(&mut app, Msg::Copy(CopyTarget::DataDir));
        assert!(app.is_copied(CopyTarget::DataDir));
        assert!(!app.is_copied(CopyTarget::Address));
        let _ = update(&mut app, Msg::Copy(CopyTarget::LogsDir));
        assert!(app.is_copied(CopyTarget::LogsDir));
        assert!(!app.is_copied(CopyTarget::DataDir));
    }

    #[test]
    fn nasce_sem_falha_ao_abrir_o_navegador() {
        let dir = tempfile::tempdir().unwrap();
        let (app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        assert!(!app.releases_open_failed);
    }

    #[test]
    fn falha_ao_abrir_o_navegador_liga_o_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        let _ = update(&mut app, Msg::OpenUrlFailed);
        assert!(app.releases_open_failed);
    }

    #[test]
    fn copiar_sem_rede_nao_faz_nada() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        let _ = update(&mut app, Msg::Copy(CopyTarget::Address));
        assert!(app.copied.is_none());
    }

    #[test]
    fn hover_de_linha_tolera_saida_atrasada() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        let _ = update(&mut app, Msg::HoverRow("A".into()));
        let _ = update(&mut app, Msg::HoverRow("B".into()));
        let _ = update(&mut app, Msg::UnhoverRow("A".into()));
        assert_eq!(app.hover_row.as_deref(), Some("B"));
        let _ = update(&mut app, Msg::UnhoverRow("B".into()));
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
            ..DeviceView::default()
        };
        let snap = Snapshot {
            devices: vec![device],
            dashboard: std::sync::Arc::new(ds),
            ..Snapshot::default()
        };
        let _ = update(&mut app, Msg::Snapshot(Box::new(snap)));
        assert_eq!(app.people.len(), 1);
        assert_eq!(app.people[0].display_name, "Ana");
        assert!(app.avatars.contains_key("D1"));
    }

    pub(crate) fn app_with_two_people(
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
            ..DeviceView::default()
        };
        let snap = Snapshot {
            devices: vec![device("D1", "Pixel 7"), device("D2", "iPhone 13")],
            ..Snapshot::default()
        };
        app_with(snap, dir.to_path_buf())
    }

    pub(crate) fn revoked(rx: &mut mpsc::UnboundedReceiver<Command>) -> Vec<String> {
        let mut out = Vec::new();
        while let Ok(cmd) = rx.try_recv() {
            if let Command::RemoveDevice { device_id, .. } = cmd {
                out.push(device_id);
            }
        }
        out
    }

    // --- Conectar um celular ---

    pub(crate) fn phone(id: &str, name: &str) -> DeviceView {
        DeviceView {
            device_id: id.into(),
            name: name.into(),
            paired_at: "2026-10-03T10:00:00Z".into(),
            ..DeviceView::default()
        }
    }

    pub(crate) fn running_with(devices: Vec<DeviceView>) -> Snapshot {
        Snapshot {
            status: Status::Running,
            addresses: vec![std::net::Ipv4Addr::new(192, 168, 0, 12)],
            https_port: 7777,
            http_port: 7778,
            devices,
            ..Snapshot::default()
        }
    }

    #[test]
    fn conectar_com_aparelho_abre_o_dialogo_em_aparelhos_no_passo_1() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(
            running_with(vec![phone("D1", "Pixel 7")]),
            dir.path().to_path_buf(),
        );
        let _ = update(&mut app, Msg::ConnectPhone);
        assert_eq!(app.screen, Screen::Connection);
        assert_eq!(app.connect, Some(ConnectFlow::new(true)));
        assert!(connect::dialog_open(&app));
        assert!(!connect::shows_in_screen_flow(&app));
    }

    #[test]
    fn conectar_sem_aparelho_abre_o_fluxo_em_tela() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(running_with(vec![]), dir.path().to_path_buf());
        let _ = update(&mut app, Msg::ConnectPhone);
        assert_eq!(app.screen, Screen::Connection);
        assert_eq!(app.connect, Some(ConnectFlow::new(false)));
        assert!(connect::shows_in_screen_flow(&app));
        assert!(!connect::dialog_open(&app));
        // O botao dos vazios leva ao mesmo lugar.
        app.screen = Screen::Dashboard;
        app.connect = None;
        let _ = update(&mut app, Msg::ConnectPhone);
        assert!(connect::shows_in_screen_flow(&app));
    }

    #[test]
    fn continuar_e_voltar_andam_e_o_codigo_e_pedido_ao_entrar_no_passo_3() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with(
            running_with(vec![phone("D1", "Pixel 7")]),
            dir.path().to_path_buf(),
        );
        let _ = update(&mut app, Msg::ConnectPhone);
        let _ = update(&mut app, Msg::ConnectBack);
        assert_eq!(app.connect.map(|f| f.step), Some(ConnectStep::WiFi));
        let _ = update(&mut app, Msg::ConnectNext);
        assert_eq!(app.connect.map(|f| f.step), Some(ConnectStep::Certificate));
        assert!(drain(&mut rx).is_empty(), "nada sai antes do passo 3");
        let _ = update(&mut app, Msg::ConnectNext);
        assert_eq!(app.connect.map(|f| f.step), Some(ConnectStep::Pair));
        assert_eq!(drain(&mut rx), vec![Command::IssuePairingToken]);
        // Continuar no ultimo passo nao faz nada.
        let _ = update(&mut app, Msg::ConnectNext);
        assert!(drain(&mut rx).is_empty());
        let _ = update(&mut app, Msg::ConnectBack);
        assert_eq!(app.connect.map(|f| f.step), Some(ConnectStep::Certificate));
        // Voltar do passo 3 cancela o codigo; voltar do 2 nao manda nada.
        assert_eq!(drain(&mut rx), vec![Command::CancelPairingToken]);
        let _ = update(&mut app, Msg::ConnectBack);
        assert!(drain(&mut rx).is_empty());
    }

    #[test]
    fn hub_fora_do_ar_nao_pede_codigo_ao_entrar_no_passo_3() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        app.screen = Screen::Connection;
        app.connect = Some(ConnectFlow::new(true));
        let _ = update(&mut app, Msg::ConnectNext);
        let _ = update(&mut app, Msg::ConnectNext);
        assert_eq!(app.connect.map(|f| f.step), Some(ConnectStep::Pair));
        assert!(drain(&mut rx).is_empty());
    }

    #[test]
    fn segmentado_troca_o_sistema_do_passo_2() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(
            running_with(vec![phone("D1", "Pixel 7")]),
            dir.path().to_path_buf(),
        );
        let _ = update(&mut app, Msg::ConnectPhone);
        let _ = update(&mut app, Msg::ConnectNext);
        let _ = update(&mut app, Msg::ConnectOs(PhoneOs::Iphone));
        assert_eq!(app.connect.map(|f| f.os), Some(PhoneOs::Iphone));
        assert_eq!(app.connect.map(|f| f.step), Some(ConnectStep::Certificate));
        let _ = update(&mut app, Msg::ConnectOs(PhoneOs::Android));
        assert_eq!(app.connect.map(|f| f.os), Some(PhoneOs::Android));
    }

    #[test]
    fn fechar_ou_sair_da_tela_cancela_o_codigo() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with(
            running_with(vec![phone("D1", "Pixel 7")]),
            dir.path().to_path_buf(),
        );
        let _ = update(&mut app, Msg::ConnectPhone);
        let _ = update(&mut app, Msg::ConnectClose);
        assert_eq!(app.connect, None);
        assert_eq!(drain(&mut rx), vec![Command::CancelPairingToken]);

        let _ = update(&mut app, Msg::ConnectPhone);
        let _ = update(&mut app, Msg::SelectScreen(Screen::Dashboard));
        assert_eq!(app.connect, None);
        assert_eq!(drain(&mut rx), vec![Command::CancelPairingToken]);
        // Sem fluxo aberto, trocar de tela nao manda nada.
        let _ = update(&mut app, Msg::SelectScreen(Screen::Connection));
        let _ = update(&mut app, Msg::SelectScreen(Screen::Dashboard));
        assert!(drain(&mut rx).is_empty());
    }

    #[test]
    fn aparelho_novo_fecha_o_dialogo_e_mostra_o_aviso() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with(
            running_with(vec![phone("D1", "Pixel 7")]),
            dir.path().to_path_buf(),
        );
        let _ = update(&mut app, Msg::ConnectPhone);
        let _ = update(&mut app, Msg::ConnectNext);
        let _ = update(&mut app, Msg::ConnectNext);
        let _ = drain(&mut rx);
        let snap = running_with(vec![phone("D1", "Pixel 7"), phone("D2", "Pixel da Ana")]);
        let _ = update(&mut app, Msg::Snapshot(Box::new(snap)));
        assert_eq!(app.connect, None);
        assert_eq!(
            app.notice.as_ref().map(|(text, _)| text.as_str()),
            Some("Pixel da Ana conectado")
        );
        // Cancela mesmo assim: um "Gerar outro codigo" pode ter saido antes do snapshot.
        assert_eq!(drain(&mut rx), vec![Command::CancelPairingToken]);
        // O aviso some no tique depois do prazo.
        let _ = update(&mut app, Msg::Tick);
        assert!(app.notice.is_some());
        if let Some((_, until)) = app.notice.as_mut() {
            *until = Instant::now();
        }
        let _ = update(&mut app, Msg::Tick);
        assert_eq!(app.notice, None);
    }

    #[test]
    fn parear_de_novo_o_mesmo_aparelho_tambem_fecha_o_dialogo() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(
            running_with(vec![phone("D1", "Pixel 7")]),
            dir.path().to_path_buf(),
        );
        let _ = update(&mut app, Msg::ConnectPhone);
        // Mesmo id, ainda ativo, `paired_at` novo (o UPSERT do pareamento).
        let mut again = phone("D1", "Pixel 7");
        again.paired_at = "2026-11-26T10:00:00Z".into();
        let _ = update(&mut app, Msg::Snapshot(Box::new(running_with(vec![again]))));
        assert_eq!(app.connect, None);
        assert_eq!(
            app.notice.as_ref().map(|(text, _)| text.as_str()),
            Some("Pixel 7 conectado")
        );
        // Um snapshot qualquer (mesmo `paired_at`) nao e pareamento.
        app.notice = None;
        let _ = update(&mut app, Msg::ConnectPhone);
        let snap = app.snapshot.clone();
        let _ = update(&mut app, Msg::Snapshot(Box::new(snap)));
        assert!(app.connect.is_some());
        assert_eq!(app.notice, None);
    }

    #[test]
    fn remocao_pendente_nao_troca_a_lista_pelo_fluxo_e_desfazer_cancela_o_codigo() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with(
            running_with(vec![phone("D1", "Pixel 7")]),
            dir.path().to_path_buf(),
        );
        app.refresh_people();
        app.screen = Screen::Connection;
        let _ = update(&mut app, Msg::AskRemove("D1".into()));
        let _ = update(&mut app, Msg::ConfirmRemove);
        // Durante o "Desfazer" o aparelho ainda conta: a lista fica, sem o fluxo em tela.
        assert!(!connect::in_screen_case(&app));
        assert!(!connect::shows_in_screen_flow(&app));
        // Fluxo em tela no passo 3 (como antes desta correcao) que o "Desfazer" esconderia.
        app.connect = Some(ConnectFlow {
            step: ConnectStep::Pair,
            ..ConnectFlow::new(false)
        });
        let _ = drain(&mut rx);
        let _ = update(&mut app, Msg::UndoRemove);
        assert_eq!(app.connect, None);
        assert_eq!(drain(&mut rx), vec![Command::CancelPairingToken]);
        // Depois que a remocao sai e o snapshot a reflete, o fluxo ocupa a tela.
        let mut gone = phone("D1", "Pixel 7");
        gone.revoked = true;
        let _ = update(&mut app, Msg::Snapshot(Box::new(running_with(vec![gone]))));
        assert!(connect::shows_in_screen_flow(&app));
    }

    #[test]
    fn primeiro_aparelho_encerra_o_fluxo_em_tela() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(running_with(vec![]), dir.path().to_path_buf());
        app.screen = Screen::Connection;
        // Fluxo em tela sem nenhum clique ainda (`connect` vazio): mesmo assim fecha e avisa.
        let snap = running_with(vec![phone("D1", "Pixel 8")]);
        let _ = update(&mut app, Msg::Snapshot(Box::new(snap)));
        assert_eq!(
            app.notice.as_ref().map(|(text, _)| text.as_str()),
            Some("Pixel 8 conectado")
        );
        assert!(!connect::shows_in_screen_flow(&app));
    }

    #[test]
    fn aparelhos_do_boot_ou_sem_fluxo_aberto_nao_viram_aviso() {
        let dir = tempfile::tempdir().unwrap();
        // Boot: o snapshot inicial (`Starting`) vem sem aparelhos e o primeiro `Running` os
        // traz todos.
        let (mut app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        app.screen = Screen::Connection;
        let snap = running_with(vec![phone("D1", "Pixel 7")]);
        let _ = update(&mut app, Msg::Snapshot(Box::new(snap)));
        assert_eq!(app.notice, None);
        // Fora de Aparelhos, sem fluxo: um aparelho novo nao gera aviso.
        app.screen = Screen::Dashboard;
        let snap = running_with(vec![phone("D1", "Pixel 7"), phone("D2", "iPhone")]);
        let _ = update(&mut app, Msg::Snapshot(Box::new(snap)));
        assert_eq!(app.notice, None);
    }

    #[test]
    fn remover_pede_confirmacao_e_cancelar_nao_faz_nada() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with_two_people(dir.path());
        let _ = update(&mut app, Msg::AskRemove("D1".into()));
        assert_eq!(
            app.confirm_remove.as_ref().map(|d| d.device_id.as_str()),
            Some("D1")
        );
        let _ = update(&mut app, Msg::CancelRemove);
        assert_eq!(app.confirm_remove, None);
        assert_eq!(app.pending_removal, None);
        assert!(revoked(&mut rx).is_empty());
        assert_eq!(app.visible_people().count(), 2);
    }

    #[test]
    fn confirmar_esconde_a_linha_e_desfazer_nao_envia_nada() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with_two_people(dir.path());
        let _ = update(&mut app, Msg::AskRemove("D1".into()));
        let _ = update(&mut app, Msg::ConfirmRemove);
        assert_eq!(app.confirm_remove, None);
        let pending = app.pending_removal.clone().unwrap();
        assert_eq!(pending.device_id(), Some("D1"));
        assert_eq!(pending.toast, "Pixel 7 foi removido do hub");
        assert_eq!(
            app.visible_people()
                .map(|p| p.device_id.as_str())
                .collect::<Vec<_>>(),
            vec!["D2"]
        );
        // Tique antes do prazo: nada sai.
        let _ = update(&mut app, Msg::Tick);
        assert!(revoked(&mut rx).is_empty());
        let _ = update(&mut app, Msg::UndoRemove);
        assert_eq!(app.pending_removal, None);
        assert_eq!(app.visible_people().count(), 2);
        assert!(revoked(&mut rx).is_empty());
    }

    #[test]
    fn prazo_vencido_envia_a_revogacao() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with_two_people(dir.path());
        let _ = update(&mut app, Msg::AskRemove("D1".into()));
        let _ = update(&mut app, Msg::ConfirmRemove);
        app.pending_removal.as_mut().unwrap().deadline = Instant::now();
        let _ = update(&mut app, Msg::Tick);
        assert_eq!(revoked(&mut rx), vec!["D1".to_string()]);
        assert_eq!(app.pending_removal, None);
    }

    #[test]
    fn outra_remocao_ou_trocar_de_tela_envia_a_pendente() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with_two_people(dir.path());
        app.screen = Screen::Connection;
        let _ = update(&mut app, Msg::AskRemove("D1".into()));
        let _ = update(&mut app, Msg::ConfirmRemove);
        let _ = update(&mut app, Msg::AskRemove("D2".into()));
        let _ = update(&mut app, Msg::ConfirmRemove);
        assert_eq!(revoked(&mut rx), vec!["D1".to_string()]);
        assert_eq!(
            app.pending_removal.as_ref().unwrap().device_id(),
            Some("D2")
        );
        let _ = update(&mut app, Msg::SelectScreen(Screen::Dashboard));
        assert_eq!(revoked(&mut rx), vec!["D2".to_string()]);
    }

    #[test]
    fn clicar_na_tela_atual_so_volta_ao_topo() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with_two_people(dir.path());
        app.screen = Screen::Connection;
        let _ = update(&mut app, Msg::AskRemove("D1".into()));
        let _ = update(&mut app, Msg::ConfirmRemove);
        app.key_input = "sk-rascunho".into();
        let effects = step(&mut app, Msg::SelectScreen(Screen::Connection));
        // Volta ao topo, mas a remocao continua pendente (com "Desfazer") e nada saiu.
        assert!(matches!(effects.as_slice(), [Effect::ScrollToTop]));
        assert!(revoked(&mut rx).is_empty());
        assert_eq!(
            app.pending_removal
                .as_ref()
                .and_then(PendingRemoval::device_id),
            Some("D1")
        );
        assert_eq!(app.key_input, "sk-rascunho");
        let _ = update(&mut app, Msg::UndoRemove);
        assert_eq!(app.pending_removal, None);
    }

    #[test]
    fn fechar_com_remocao_pendente_revoga_antes_do_shutdown() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with_two_people(dir.path());
        let _ = update(&mut app, Msg::AskRemove("D1".into()));
        let _ = update(&mut app, Msg::ConfirmRemove);
        let _ = update(&mut app, Msg::CloseRequested);
        assert!(matches!(
            rx.try_recv(),
            Ok(Command::RemoveDevice { device_id, .. }) if device_id == "D1"
        ));
        assert!(matches!(rx.try_recv(), Ok(Command::Shutdown)));
    }

    // --- modal por area, desfazer e aviso de aparelho removido ---

    pub(crate) fn data() -> DeviceData {
        DeviceData {
            budgets: 3,
            transactions: 100,
            internal_transfers: 4,
            reserves: 2,
            reserve_movements: 14,
            recurrences: 1,
            adjustments: 2,
            ..DeviceData::default()
        }
    }

    pub(crate) fn view(
        id: &str,
        name: &str,
        revoked: bool,
        kept: bool,
        data: DeviceData,
    ) -> DeviceView {
        DeviceView {
            device_id: id.into(),
            name: name.into(),
            paired_at: "2026-09-01T10:00:00Z".into(),
            revoked,
            data_kept: kept,
            data,
            ..DeviceView::default()
        }
    }

    pub(crate) fn app_with_data(
        dir: &std::path::Path,
        devices: Vec<DeviceView>,
    ) -> (
        App,
        mpsc::UnboundedReceiver<Command>,
        watch::Sender<Snapshot>,
    ) {
        let snap = Snapshot {
            devices,
            ..Snapshot::default()
        };
        app_with(snap, dir.to_path_buf())
    }

    pub(crate) fn drain(rx: &mut mpsc::UnboundedReceiver<Command>) -> Vec<Command> {
        let mut out = Vec::new();
        while let Ok(cmd) = rx.try_recv() {
            out.push(cmd);
        }
        out
    }

    pub(crate) fn only(area: Area) -> AreaSet {
        [area].into_iter().collect()
    }

    #[test]
    fn nada_marcado_manda_remove_device_vazio() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with_data(
            dir.path(),
            vec![view("D1", "Pixel 7", false, false, data())],
        );
        let _ = update(&mut app, Msg::AskRemove("D1".into()));
        let _ = update(&mut app, Msg::RemoveContinue);
        // Manter tudo e o padrao: nao ha passo 2.
        assert_eq!(app.confirm_remove, None);
        app.pending_removal.as_mut().unwrap().deadline = Instant::now();
        let _ = update(&mut app, Msg::Tick);
        assert_eq!(
            drain(&mut rx),
            vec![Command::RemoveDevice {
                device_id: "D1".into(),
                erase: AreaSet::default()
            }]
        );
    }

    #[test]
    fn marcar_area_exige_o_passo_2_e_voltar_nao_envia_nada() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with_data(
            dir.path(),
            vec![view("D1", "Pixel 7", false, false, data())],
        );
        let _ = update(&mut app, Msg::AskRemove("D1".into()));
        let _ = update(&mut app, Msg::ToggleArea(Area::Budgets));
        let dialog = app.confirm_remove.clone().unwrap();
        assert_eq!(dialog.selected, only(Area::Budgets));
        assert_eq!(dialog.step, RemoveStep::Choose);
        // Mensagem fora de ordem: confirmar no passo 1 com area marcada nao apaga nada.
        let _ = update(&mut app, Msg::ConfirmRemove);
        assert_eq!(app.pending_removal, None);
        assert_eq!(
            app.confirm_remove.as_ref().unwrap().step,
            RemoveStep::Confirm
        );
        let _ = update(&mut app, Msg::RemoveBack);
        assert_eq!(
            app.confirm_remove.as_ref().unwrap().step,
            RemoveStep::Choose
        );
        let _ = update(&mut app, Msg::RemoveContinue);
        assert_eq!(
            app.confirm_remove.as_ref().unwrap().step,
            RemoveStep::Confirm
        );
        let _ = update(&mut app, Msg::RemoveBack);
        let _ = update(&mut app, Msg::CancelRemove);
        assert_eq!(app.confirm_remove, None);
        assert_eq!(app.pending_removal, None);
        assert!(drain(&mut rx).is_empty());
    }

    #[test]
    fn area_sem_dados_fica_desabilitada() {
        let dir = tempfile::tempdir().unwrap();
        let only_budgets = DeviceData {
            budgets: 2,
            ..DeviceData::default()
        };
        let (mut app, _rx, _tx) = app_with_data(
            dir.path(),
            vec![view("D1", "Pixel 7", false, false, only_budgets)],
        );
        let _ = update(&mut app, Msg::AskRemove("D1".into()));
        let _ = update(&mut app, Msg::ToggleArea(Area::Transactions));
        assert!(app.confirm_remove.as_ref().unwrap().selected.is_empty());
        let _ = update(&mut app, Msg::ToggleArea(Area::Budgets));
        assert_eq!(
            app.confirm_remove.as_ref().unwrap().selected,
            only(Area::Budgets)
        );
        // "Apagar tudo" marca so as que tem dados.
        let _ = update(&mut app, Msg::SelectNoAreas);
        let _ = update(&mut app, Msg::SelectAllAreas);
        assert_eq!(
            app.confirm_remove.as_ref().unwrap().selected,
            only(Area::Budgets)
        );
    }

    #[test]
    fn desfazer_cancela_a_remocao_e_o_apagar() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with_data(
            dir.path(),
            vec![view("D1", "Pixel 7", false, false, data())],
        );
        let _ = update(&mut app, Msg::AskRemove("D1".into()));
        let _ = update(&mut app, Msg::SelectAllAreas);
        let _ = update(&mut app, Msg::RemoveContinue);
        let _ = update(&mut app, Msg::ConfirmRemove);
        assert_eq!(app.visible_people().count(), 0);
        let _ = update(&mut app, Msg::UndoRemove);
        assert_eq!(app.pending_removal, None);
        assert_eq!(app.visible_people().count(), 1);
        let _ = update(&mut app, Msg::Tick);
        let _ = update(&mut app, Msg::SelectScreen(Screen::Dashboard));
        assert!(drain(&mut rx).is_empty());
    }

    #[test]
    fn decidir_apagar_de_removido_tem_desfazer_e_nao_revoga() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with_data(
            dir.path(),
            vec![view("ORFAO", "Orfao", true, false, data())],
        );
        let _ = update(&mut app, Msg::DecideOrphan);
        let _ = update(&mut app, Msg::ToggleArea(Area::Reserves));
        let _ = update(&mut app, Msg::RemoveContinue);
        assert_eq!(
            crate::ui_model::remove::confirm_label(&data(), only(Area::Reserves), true),
            "Apagar 16 itens"
        );
        let _ = update(&mut app, Msg::ConfirmRemove);
        // O aparelho some do aviso na hora (apagar pendente), mas nada saiu.
        assert!(app.orphans().is_empty());
        assert!(drain(&mut rx).is_empty());
        let _ = update(&mut app, Msg::UndoRemove);
        assert_eq!(app.orphans().len(), 1);
        let _ = update(&mut app, Msg::DecideOrphan);
        let _ = update(&mut app, Msg::ToggleArea(Area::Reserves));
        let _ = update(&mut app, Msg::RemoveContinue);
        let _ = update(&mut app, Msg::ConfirmRemove);
        app.pending_removal.as_mut().unwrap().deadline = Instant::now();
        let _ = update(&mut app, Msg::Tick);
        assert_eq!(
            drain(&mut rx),
            vec![Command::EraseDeviceData {
                device_id: "ORFAO".into(),
                areas: only(Area::Reserves)
            }]
        );
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
            ..DeviceView::default()
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
        assert_eq!(
            app.dashboard.header.partial.as_deref(),
            Some("Parcial · até 24 set")
        );
        assert_eq!(app.transactions.list.count, 7);
        // So D1: D2 nunca enviou, D3 foi revogado.
        assert_eq!(app.transactions.synced_devices, 1);
    }

    #[test]
    fn seletor_de_mes_satura_nos_limites() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with_fixtures(dir.path());
        let _ = update(&mut app, Msg::NextMonth);
        assert_eq!(app.month_idx, 11, "nao passa do corrente");
        let _ = update(&mut app, Msg::PrevMonth);
        assert_eq!(app.month(), "2026-08");
        assert_eq!(app.dashboard.month, "2026-08");
        assert!(!app.dashboard.is_current);
        assert_eq!(app.dashboard.header.partial, None);
        assert!(app.dashboard.whats_next.is_none());
        assert_eq!(app.transactions.month, "2026-08");
        for _ in 0..20 {
            let _ = update(&mut app, Msg::PrevMonth);
        }
        assert_eq!(app.month_idx, 0);
        assert_eq!(app.month(), "2025-10");
        let _ = update(&mut app, Msg::NextMonth);
        assert_eq!(app.month(), "2025-11");
    }

    #[test]
    fn virada_do_dia_anda_a_janela_e_mantem_o_mes() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with_fixtures(dir.path());
        let _ = update(&mut app, Msg::PrevMonth);
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
        let _ = update(&mut app, Msg::Snapshot(Box::new(snap)));
        assert!(!app.dashboard.is_empty);
        assert_eq!(
            app.dashboard.recent.lines.len(),
            crate::dashboard::grafite::RECENT_LINES
        );
    }

    #[test]
    fn ver_todas_abre_no_lugar_sobrevive_ao_sync_e_fecha_ao_trocar_de_mes() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with_fixtures(dir.path());
        assert!(!app.upcoming_expanded);
        let _ = update(&mut app, Msg::ExpandUpcoming);
        assert!(app.upcoming_expanded);
        // Um sync novo recalcula a tela mas nao fecha a lista de quem esta lendo.
        let snap = Snapshot {
            dashboard: std::sync::Arc::new(crate::dashboard::fixtures()),
            ..Snapshot::default()
        };
        let _ = update(&mut app, Msg::Snapshot(Box::new(snap)));
        assert!(app.upcoming_expanded);
        let _ = update(&mut app, Msg::PrevMonth);
        assert!(!app.upcoming_expanded);
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
        let _ = update(&mut app, Msg::Snapshot(Box::new(snap)));
        assert!(app.user_avatars.contains_key("U1"));
        assert!(!app.user_avatars.contains_key("U2"));
    }

    #[test]
    fn menu_de_categoria_abre_fecha_e_escolhe() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with_fixtures(dir.path());
        let _ = update(&mut app, Msg::ToggleCategoryMenu);
        assert!(app.cat_menu_open);
        let _ = update(&mut app, Msg::ToggleCategoryMenu);
        assert!(!app.cat_menu_open);
        let _ = update(&mut app, Msg::ToggleCategoryMenu);
        let _ = update(&mut app, Msg::CloseCategoryMenu);
        assert!(!app.cat_menu_open, "clicar fora fecha");
        let _ = update(&mut app, Msg::ToggleCategoryMenu);
        let _ = update(&mut app, Msg::PickCategory(Some("C1".into())));
        assert!(!app.cat_menu_open, "escolher fecha");
        assert_eq!(app.filters.category.as_deref(), Some("C1"));
        assert_eq!(app.transactions.list.count, 1);
        assert_eq!(
            app.transactions
                .selected_category
                .as_ref()
                .map(|c| c.name.as_str()),
            Some("Alimentação")
        );
        let _ = update(&mut app, Msg::PickCategory(Some(String::new())));
        assert_eq!(
            app.transactions.list.count, 2,
            "sem categoria: nula e apagada"
        );
        app.screen = Screen::Transactions;
        let _ = update(&mut app, Msg::ToggleCategoryMenu);
        let _ = update(&mut app, Msg::SelectScreen(Screen::Dashboard));
        assert!(!app.cat_menu_open, "trocar de tela fecha");
    }

    #[test]
    fn chips_de_autor_e_tipo() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with_fixtures(dir.path());
        // Setembro tem lancamento sem autor: ha tres chips (Ana, Luiz, Sem autor).
        assert!(app.transactions.unauthored_chip);
        let _ = update(&mut app, Msg::ToggleAuthor("U1".into()));
        assert_eq!(app.transactions.list.count, 3);
        let _ = update(&mut app, Msg::ToggleAuthor("U2".into()));
        assert_eq!(app.filters.authors.len(), 2, "falta o Sem autor");
        let _ = update(&mut app, Msg::ToggleAuthor(String::new()));
        assert!(app.filters.authors.is_empty(), "todos marcados = Todos");
        assert_eq!(app.transactions.list.count, 7);
        let _ = update(&mut app, Msg::ToggleAuthor("U2".into()));
        let _ = update(&mut app, Msg::AllAuthors);
        assert!(app.filters.authors.is_empty());
        let _ = update(&mut app, Msg::SetType(TypeFilter::Income));
        assert_eq!(app.transactions.list.count, 1);
        let _ = update(&mut app, Msg::SetType(TypeFilter::All));
        assert!(!app.filters.is_active());
    }

    #[test]
    fn filtros_sobrevivem_a_troca_de_mes_e_limpar_zera() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with_fixtures(dir.path());
        let _ = update(&mut app, Msg::PickCategory(Some("C2".into())));
        let _ = update(&mut app, Msg::SetType(TypeFilter::Expense));
        let _ = update(&mut app, Msg::PrevMonth);
        assert_eq!(app.filters.category.as_deref(), Some("C2"));
        assert_eq!(app.filters.kind, TypeFilter::Expense);
        assert_eq!(app.transactions.month, "2026-08");
        assert_eq!(app.transactions.list.count, 1, "aluguel de agosto");
        let _ = update(&mut app, Msg::ToggleCategoryMenu);
        let _ = update(&mut app, Msg::ClearFilters);
        assert_eq!(app.filters, Filters::default());
        assert!(!app.cat_menu_open);
        assert_eq!(app.transactions.list.count, 3);
    }

    /// App com `fixtures_with_reserves()` e "hoje" fixo em 24 de setembro de 2026.
    pub(crate) fn app_with_reserves(
        dir: &std::path::Path,
    ) -> (
        App,
        mpsc::UnboundedReceiver<Command>,
        watch::Sender<Snapshot>,
    ) {
        let snap = Snapshot {
            dashboard: std::sync::Arc::new(crate::dashboard::fixtures_with_reserves()),
            ..Snapshot::default()
        };
        let (mut app, rx, tx) = app_with(snap, dir.to_path_buf());
        app.set_today("2026-09-24".into());
        (app, rx, tx)
    }

    #[test]
    fn tela_reservas_nasce_vazia_e_recalcula_com_o_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        assert!(app.reserves.is_empty);
        let _ = update(&mut app, Msg::SelectScreen(Screen::Reserves));
        assert_eq!(app.screen, Screen::Reserves);
        app.set_today("2026-09-24".into());
        let snap = Snapshot {
            dashboard: std::sync::Arc::new(crate::dashboard::fixtures_with_reserves()),
            ..Snapshot::default()
        };
        let _ = update(&mut app, Msg::Snapshot(Box::new(snap)));
        assert_eq!(app.reserves.reserve_count, 6);
        assert_eq!(app.reserves.current_month, "2026-09");
    }

    #[test]
    fn app_nasce_com_reservas_padrao() {
        let dir = tempfile::tempdir().unwrap();
        let (cmd_tx, _rx) = mpsc::unbounded_channel();
        let (_tx, snap_rx) = watch::channel(Snapshot::default());
        let app = App::new(
            HubHandle {
                commands: cmd_tx,
                snapshot: snap_rx,
            },
            UtcOffset::UTC,
            UiPrefs::default(),
            dir.path().to_path_buf(),
            Rc::new(Headless),
        );
        // `new` ja chama `set_today` (que recalcula): o modelo nao e o `default()`, e o de um
        // hub sem reservas, com a janela de hoje.
        assert!(app.reserves.is_empty);
        assert_eq!(app.reserves.reserve_count, 0);
        assert_eq!(app.reserves.months.len(), 12);
        assert_eq!(app.reserves.emergency, None);
    }

    #[test]
    fn com_bandeja_fechar_a_janela_fecha_o_conectar_um_celular() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _fake, mut rx) = with_tray(dir.path());
        app.screen = Screen::Connection;
        app.connect = Some(ConnectFlow::new(true));
        let _ = update(&mut app, Msg::CloseRequested);
        assert_eq!(app.connect, None);
        assert_eq!(drain(&mut rx), vec![Command::CancelPairingToken]);
        assert!(!app.closing);
    }

    #[test]
    fn sem_bandeja_fechar_continua_desligando() {
        // `fechar_manda_shutdown_uma_vez` e `fechar_com_remocao_pendente_revoga_antes_do_shutdown`
        // ja cobrem; este fixa que `Headless` e o que os mantem validos.
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        assert!(!app.desktop.has_tray());
        let _ = update(&mut app, Msg::CloseRequested);
        assert!(app.closing);
        assert!(matches!(rx.try_recv(), Ok(Command::Shutdown)));
    }

    #[test]
    fn desligar_pela_bandeja_manda_shutdown_uma_vez() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _fake, mut rx) = with_tray(dir.path());
        let _ = update(&mut app, Msg::Desktop(DesktopEvent::Quit));
        let _ = update(&mut app, Msg::Desktop(DesktopEvent::Quit));
        assert!(app.closing);
        assert!(matches!(rx.try_recv(), Ok(Command::Shutdown)));
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn stopped_so_sai_quando_esta_desligando() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _fake, _rx) = with_tray(dir.path());
        let stopped = Snapshot {
            status: Status::Stopped,
            ..Snapshot::default()
        };
        // Sem `closing`, um `Stopped` (nucleo caiu) nao encerra a bandeja: a pessoa ve o estado.
        let _ = update(&mut app, Msg::Snapshot(Box::new(stopped)));
        assert!(!app.closing);
    }

    #[test]
    fn tooltip_segue_o_snapshot_e_nao_repete() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, fake, _rx) = with_tray(dir.path());
        assert_eq!(fake.tooltip.borrow().as_str(), "HubFinance — iniciando");
        let snap = Snapshot {
            status: Status::Running,
            devices: vec![
                view("D1", "Pixel 7", false, false, DeviceData::default()),
                view("D2", "Antigo", true, true, DeviceData::default()),
            ],
            ..Snapshot::default()
        };
        let _ = update(&mut app, Msg::Snapshot(Box::new(snap.clone())));
        assert_eq!(
            fake.tooltip.borrow().as_str(),
            "HubFinance — ligado · 1 aparelho"
        );
        *fake.tooltip.borrow_mut() = String::new();
        let _ = update(&mut app, Msg::Snapshot(Box::new(snap)));
        assert_eq!(
            fake.tooltip.borrow().as_str(),
            "",
            "texto igual nao e reenviado"
        );
    }

    #[test]
    fn icone_da_bandeja_segue_o_estado_e_so_troca_quando_muda() {
        use crate::desktop::tray::TrayIcon;
        let dir = tempfile::tempdir().unwrap();
        let (mut app, fake, _rx) = with_tray(dir.path());
        assert!(
            fake.icons.borrow().is_empty(),
            "o boot ja nasce com o normal"
        );
        let snap = |status| Snapshot {
            status,
            ..Snapshot::default()
        };
        for status in [
            Status::Running,
            Status::NoNetwork,
            Status::NoNetwork,
            Status::Failed("porta".into()),
            Status::Failed("outra".into()),
            Status::Running,
        ] {
            let _ = update(&mut app, Msg::Snapshot(Box::new(snap(status))));
        }
        assert_eq!(
            *fake.icons.borrow(),
            [TrayIcon::Attention, TrayIcon::Error, TrayIcon::Normal]
        );
    }

    #[test]
    fn bandeja_que_recusa_tenta_de_novo_no_tique() {
        use crate::desktop::tray::TrayIcon;
        let dir = tempfile::tempdir().unwrap();
        let (mut app, fake, _rx) = with_tray(dir.path());
        fake.fail_tray.set(true);
        let snap = Snapshot {
            status: Status::NoNetwork,
            ..Snapshot::default()
        };
        let _ = update(&mut app, Msg::Snapshot(Box::new(snap)));
        assert_eq!(*fake.icons.borrow(), [TrayIcon::Attention]);
        assert_eq!(
            app.tray_icon,
            TrayIcon::Normal,
            "recusado nao entra no cache"
        );
        // Recusou de novo no tique: tenta, sem guardar.
        let _ = update(&mut app, Msg::Tick);
        assert_eq!(fake.icons.borrow().len(), 2);
        fake.fail_tray.set(false);
        let _ = update(&mut app, Msg::Tick);
        assert_eq!(app.tray_icon, TrayIcon::Attention);
        assert_eq!(
            fake.tooltip.borrow().as_str(),
            "HubFinance — ligado, sem rede"
        );
        assert_eq!(app.tooltip, "HubFinance — ligado, sem rede");
        // Aceito, o tique seguinte nao manda nada.
        let _ = update(&mut app, Msg::Tick);
        assert_eq!(fake.icons.borrow().len(), 3);
    }

    #[test]
    fn sem_janela_as_animacoes_terminam_na_hora() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _fake, _rx) = with_tray(dir.path());
        app.window = None;
        app.notice = Some(("Copiado".into(), Instant::now() + Duration::from_secs(5)));
        let _ = update(&mut app, Msg::Noop);
        assert!(!app.motion.animating());
        app.notice = None;
        let _ = update(&mut app, Msg::Noop);
        assert!(app.motion.toast().is_none(), "nada descendo para reabrir");
    }

    #[test]
    fn conectar_pela_bandeja_desligando_nao_faz_nada() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _fake, _rx) = with_tray(dir.path());
        app.window = None;
        app.closing = true;
        let _ = update(&mut app, Msg::Desktop(DesktopEvent::ConnectPhone));
        assert!(app.window.is_none());
        assert!(app.connect.is_none());
    }

    #[test]
    fn copiar_endereco_pela_bandeja_usa_o_sistema_e_sem_rede_nao_copia() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, fake, _rx) = with_tray(dir.path());
        app.window = None;
        let _ = update(&mut app, Msg::Desktop(DesktopEvent::CopyAddress));
        assert!(fake.copied.borrow().is_empty(), "sem endereco, nada");
        let snap = Snapshot {
            status: Status::Running,
            addresses: vec![std::net::Ipv4Addr::new(192, 168, 0, 12)],
            https_port: 7777,
            ..Snapshot::default()
        };
        let _ = update(&mut app, Msg::Snapshot(Box::new(snap)));
        let _ = update(&mut app, Msg::Desktop(DesktopEvent::CopyAddress));
        assert_eq!(*fake.copied.borrow(), ["https://192.168.0.12:7777"]);
        assert!(app.window.is_none(), "copiar nao abre a janela");
    }

    #[test]
    fn falha_ao_gravar_mantem_o_estado_e_avisa() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, fake, _rx) = with_tray(dir.path());
        fake.fail_autostart.set(true);
        let _ = update(&mut app, Msg::SetAutostart(true));
        assert_eq!(app.autostart, Some(false));
        assert_eq!(app.autostart_pref, None);
        assert!(app.autostart_failed);
        assert_eq!(UiPrefs::load(dir.path()).autostart, None);
    }

    #[test]
    fn fechar_manda_shutdown_uma_vez() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        let _ = update(&mut app, Msg::CloseRequested);
        let _ = update(&mut app, Msg::CloseRequested);
        assert!(matches!(rx.try_recv(), Ok(Command::Shutdown)));
        assert!(rx.try_recv().is_err());
    }

    // ---- seletor Pessoal | Comercial ----

    /// Fixtures (pessoais) mais a conta comercial com um lancamento de receita de R$ 900.
    pub(crate) fn business_snapshot(with_book: bool, book_deleted: bool) -> Snapshot {
        use crate::dashboard::dataset::{ASSETS, BOOKS, DEBTS, RawRow, TRANSACTIONS};
        let mut ds = crate::dashboard::fixtures();
        let row =
            |table: &str, id: &str, deleted: Option<&str>, seq: i64, data: serde_json::Value| {
                RawRow {
                    table: table.into(),
                    id: id.into(),
                    deleted_at: deleted.map(Into::into),
                    seq,
                    data: data.to_string(),
                }
            };
        let mut rows = vec![row(
            TRANSACTIONS,
            "TB1",
            None,
            1000,
            serde_json::json!({
                "kind": "income", "description": "Venda", "amountMinor": 90000,
                "occurredOn": "2026-09-10", "book": "business"
            }),
        )];
        // Capital 3.000 (2.000 equipamento + 1.000 estoque) e divida 500 do negocio; o bem sem
        // `book` e pessoal e nao pode aparecer no painel.
        rows.extend([
            row(
                ASSETS,
                "AB1",
                None,
                1002,
                serde_json::json!({
                    "kind": "equipment", "name": "Forno", "valueMinor": 200000,
                    "acquiredOn": "2026-08-01", "book": "business"
                }),
            ),
            row(
                ASSETS,
                "AB2",
                None,
                1003,
                serde_json::json!({
                    "kind": "inventory", "name": "Farinha", "valueMinor": 100000,
                    "acquiredOn": "2026-09-01", "book": "business"
                }),
            ),
            row(
                ASSETS,
                "AP1",
                None,
                1004,
                serde_json::json!({
                    "kind": "investment", "name": "CDB", "valueMinor": 777700,
                    "acquiredOn": "2026-09-01"
                }),
            ),
            row(
                DEBTS,
                "DB1",
                None,
                1005,
                serde_json::json!({
                    "name": "Banco", "principalMinor": 900000, "balanceMinor": 50000,
                    "book": "business"
                }),
            ),
        ]);
        if with_book {
            rows.push(row(
                BOOKS,
                "B1",
                book_deleted.then_some("x"),
                1001,
                serde_json::json!({
                    "kind": "business", "name": "Padaria", "color": "amber", "icon": "storefront"
                }),
            ));
        }
        ds.apply(rows);
        Snapshot {
            dashboard: std::sync::Arc::new(ds),
            ..Snapshot::default()
        }
    }

    pub(crate) fn app_with_business(
        dir: &std::path::Path,
        with_book: bool,
        book_deleted: bool,
    ) -> (App, watch::Sender<Snapshot>) {
        let (mut app, _rx, tx) = app_with(
            business_snapshot(with_book, book_deleted),
            dir.to_path_buf(),
        );
        app.set_today("2026-09-24".into());
        // O `_rx` cai aqui: nenhum teste deste grupo envia comando.
        (app, tx)
    }

    #[test]
    fn bloco_negocio_so_aparece_na_conta_comercial() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _tx) = app_with_business(dir.path(), true, false);
        assert_eq!(app.dashboard.business, None);
        let _ = update(&mut app, Msg::SelectBook(Book::Business));
        let block = app
            .dashboard
            .business
            .clone()
            .expect("bloco na conta comercial");
        // Mesmos numeros do resumo da conta, e bens/dividas fora de receita e despesa.
        assert_eq!(block.income.value, app.dashboard.how_am_i.income.value);
        assert_eq!(block.expense.value, app.dashboard.how_am_i.expense.value);
        assert_eq!(
            app.dashboard.sections().first(),
            Some(&crate::dashboard::grafite::Section::Business)
        );
        let panel = crate::dashboard::business::business_panel(&app.scope, app.month());
        assert_eq!((panel.income_minor, panel.result_minor), (90_000, 90_000));
        assert_eq!(panel.equipment_minor, 200_000);
        assert_eq!(panel.inventory_minor, 100_000);
        assert_eq!(panel.investment_minor, 0);
        assert_eq!(panel.capital_minor, 300_000);
        assert_eq!(panel.debt_minor, 50_000);
        let _ = update(&mut app, Msg::SelectBook(Book::Personal));
        assert_eq!(app.dashboard.business, None);
    }

    #[test]
    fn tela_analise_calcula_achados_da_conta_aberta() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with_fixtures(dir.path());
        let _ = update(&mut app, Msg::SelectScreen(Screen::Analysis));
        assert_eq!(app.screen, Screen::Analysis);
        assert_eq!(
            app.analysis,
            crate::advisor::analyze(&app.scope, &app.today)
        );
        assert!(
            !app.analysis.findings.is_empty(),
            "o dataset de exemplo gera achados"
        );
    }

    #[test]
    fn previa_aberta_acompanha_a_troca_de_conta_e_os_dados() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _tx) = app_with_business(dir.path(), true, false);
        let _ = update(&mut app, Msg::SelectScreen(Screen::Analysis));
        let _ = update(&mut app, Msg::ShowEnvelope);
        assert!(
            app.envelope_preview
                .as_deref()
                .unwrap()
                .contains("Conta: pessoal")
        );
        let _ = update(&mut app, Msg::SelectBook(Book::Business));
        let preview = app.envelope_preview.as_deref().unwrap();
        assert!(preview.contains("Conta: comercial"));
        assert_eq!(preview, app.current_envelope());
    }

    #[test]
    fn trocar_de_conta_limpa_os_filtros() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _tx) = app_with_business(dir.path(), true, false);
        let _ = update(&mut app, Msg::PickCategory(Some("C1".into())));
        assert_eq!(app.filters.category.as_deref(), Some("C1"));
        let _ = update(&mut app, Msg::SelectBook(Book::Business));
        assert_eq!(app.filters, Filters::default());
    }

    #[test]
    fn comercial_sem_conta_e_recusado() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _tx) = app_with_business(dir.path(), false, false);
        let _ = update(&mut app, Msg::SelectBook(Book::Business));
        assert_eq!(app.book, Book::Personal);
        assert_eq!(app.transactions.list.count, 7);
    }

    #[test]
    fn conta_comercial_apagada_com_ela_aberta_volta_para_a_pessoal() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _tx) = app_with_business(dir.path(), true, false);
        let _ = update(&mut app, Msg::SelectBook(Book::Business));
        assert_eq!(app.book, Book::Business);
        let _ = update(
            &mut app,
            Msg::Snapshot(Box::new(business_snapshot(true, true))),
        );
        assert_eq!(app.book, Book::Personal);
        assert_eq!(app.transactions.list.count, 7);
    }

    // ---- assistente: chat e configuracao ----

    use crate::assistant::settings::Settings;
    use crate::assistant::view::{ConversationSummary, MessageRole, MessageView, OpenConversation};

    pub(crate) fn local_settings(model: Option<&str>) -> Settings {
        let mut s = Settings {
            provider: ProviderChoice::Local,
            ..Settings::default()
        };
        s.local.model = model.map(Into::into);
        s
    }

    /// App de exemplo na tela Analise com o assistente local configurado.
    pub(crate) fn assistant_app(
        dir: &std::path::Path,
        settings: Settings,
    ) -> (App, mpsc::UnboundedReceiver<Command>) {
        let (mut app, mut rx, _tx) = app_with_fixtures(dir);
        let _ = update(&mut app, Msg::SelectScreen(Screen::Analysis));
        app.snapshot.assistant.settings = settings;
        assert!(drain(&mut rx).is_empty());
        (app, rx)
    }

    pub(crate) fn msg(role: MessageRole, content: &str) -> MessageView {
        MessageView {
            id: format!("m-{content}"),
            role,
            content: content.into(),
            sent_to_cloud: false,
            deny_data_collection: None,
            provider: None,
            model: None,
            tokens_in: None,
            tokens_out: None,
            interrupted: false,
        }
    }

    pub(crate) fn open_conv(book: Book, messages: Vec<MessageView>) -> OpenConversation {
        OpenConversation {
            id: "C1".into(),
            book,
            envelope: String::new(),
            messages,
            needs_cloud_consent: false,
        }
    }

    #[test]
    fn conversar_local_manda_o_envelope_da_previa_e_ja_pede_a_abertura() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx) = assistant_app(dir.path(), local_settings(Some("llama3.1:8b")));
        let env = app.current_envelope();
        let _ = update(&mut app, Msg::StartConversation);
        let sent = drain(&mut rx);
        assert_eq!(
            sent,
            vec![
                Command::Assistant(AssistantCommand::NewConversation {
                    book: app.book,
                    title: format!("Análise de {}", month_label(&app.analysis.month)),
                    envelope: env,
                }),
                Command::Assistant(AssistantCommand::Resend),
            ]
        );
        assert!(app.assistant_setup.is_none());
    }

    #[test]
    fn conversar_com_valores_ocultos_continua_mandando_o_envelope_inteiro() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx) = assistant_app(dir.path(), local_settings(Some("m")));
        let _ = update(&mut app, Msg::ToggleHideValues);
        assert!(app.hide_values);
        let _ = update(&mut app, Msg::StartConversation);
        let Some(Command::Assistant(AssistantCommand::NewConversation { envelope, .. })) =
            drain(&mut rx).into_iter().next()
        else {
            panic!("esperava NewConversation");
        };
        assert_eq!(envelope, app.current_envelope());
        assert!(envelope.contains("R$ "), "o envelope nunca vai mascarado");
    }

    #[test]
    fn conversar_sem_destino_ou_sem_modelo_abre_a_configuracao_e_nao_manda_nada() {
        let dir = tempfile::tempdir().unwrap();
        for settings in [Settings::default(), local_settings(None)] {
            let (mut app, mut rx) = assistant_app(dir.path(), settings);
            assert!(app.assistant_setup.is_none());
            let _ = update(&mut app, Msg::StartConversation);
            assert!(app.assistant_setup.is_some());
            assert!(drain(&mut rx).is_empty());
        }
    }

    #[test]
    fn enviar_vazio_ou_ocupado_nao_manda_nada_e_enviar_manda_aparado() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx) = assistant_app(dir.path(), local_settings(Some("m")));
        for blank in ["", "   ", "\n\t "] {
            app.chat_input = blank.into();
            let _ = update(&mut app, Msg::ChatSubmit);
        }
        assert!(drain(&mut rx).is_empty());

        app.snapshot.assistant.busy = true;
        app.chat_input = "e agora?".into();
        let _ = update(&mut app, Msg::ChatSubmit);
        assert!(drain(&mut rx).is_empty());
        assert_eq!(
            app.chat_input, "e agora?",
            "o texto fica para quando liberar"
        );

        app.snapshot.assistant.busy = false;
        app.chat_input = "  e agora?  ".into();
        // Sem conversa visivel nao ha para onde mandar.
        let _ = update(&mut app, Msg::ChatSubmit);
        assert!(drain(&mut rx).is_empty());
        app.snapshot.assistant.open = Some(open_conv(Book::Personal, vec![]));
        let _ = update(&mut app, Msg::ChatSubmit);
        assert_eq!(
            drain(&mut rx),
            vec![Command::Assistant(AssistantCommand::Send(
                "e agora?".into()
            ))]
        );
        assert!(app.chat_input.is_empty());
    }

    #[test]
    fn parar_tentar_de_novo_abrir_fechar_e_apagar_mandam_o_comando_certo() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx) = assistant_app(dir.path(), local_settings(Some("m")));
        // "Tentar de novo" sem conversa visivel nao reenvia nada.
        let _ = update(&mut app, Msg::ChatRetry);
        assert!(drain(&mut rx).is_empty());
        app.snapshot.assistant.open = Some(open_conv(Book::Personal, vec![]));
        let _ = update(&mut app, Msg::ChatStop);
        let _ = update(&mut app, Msg::ChatRetry);
        let _ = update(&mut app, Msg::OpenConversation("C9".into()));
        let _ = update(&mut app, Msg::CloseConversation);
        let _ = update(&mut app, Msg::DeleteConversation("C9".into()));
        assert_eq!(
            drain(&mut rx),
            [
                AssistantCommand::Cancel,
                AssistantCommand::Resend,
                AssistantCommand::Open("C9".into()),
                AssistantCommand::Close,
                AssistantCommand::Delete("C9".into()),
            ]
            .map(Command::Assistant)
        );
        // "Tentar de novo" com resposta chegando nao reenvia.
        app.snapshot.assistant.busy = true;
        let _ = update(&mut app, Msg::ChatRetry);
        assert!(drain(&mut rx).is_empty());
    }

    #[test]
    fn apagar_todas_pede_segundo_clique_dentro_do_prazo() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx) = assistant_app(dir.path(), local_settings(Some("m")));
        let _ = update(&mut app, Msg::DeleteAllConversations);
        assert!(app.confirm_delete_all.is_some());
        assert!(drain(&mut rx).is_empty());
        let _ = update(&mut app, Msg::DeleteAllConversations);
        assert_eq!(
            drain(&mut rx),
            vec![Command::Assistant(AssistantCommand::DeleteAll)]
        );
        assert!(app.confirm_delete_all.is_none());

        // Prazo vencido: o clique so arma de novo, e o tique desarma.
        app.confirm_delete_all = Some(Instant::now() - Duration::from_millis(1));
        let _ = update(&mut app, Msg::DeleteAllConversations);
        assert!(drain(&mut rx).is_empty());
        assert!(app.confirm_delete_all.is_some());
        app.confirm_delete_all = Some(Instant::now() - Duration::from_millis(1));
        let _ = update(&mut app, Msg::Tick);
        assert!(app.confirm_delete_all.is_none());
    }

    pub(crate) fn setup(app: &App) -> &crate::ui_model::assistant_setup::AssistantSetup {
        app.assistant_setup.as_ref().expect("dialogo aberto")
    }

    /// O snapshot depois de o nucleo terminar a gravacao: `configure_result` com o numero
    /// seguinte, e com sucesso a configuracao e a gravada.
    pub(crate) fn configured(app: &App, saved: Option<&Settings>, error: Option<&str>) -> Msg {
        let mut s = app.snapshot.clone();
        let n = s.assistant.configure_result.as_ref().map_or(0, |(n, _)| *n) + 1;
        s.assistant.configure_result = Some((n, error.map_or(Ok(()), |e| Err(e.to_string()))));
        if let Some(saved) = saved {
            s.assistant.settings = saved.clone();
        }
        s.assistant.last_error = error.map(Into::into);
        Msg::Snapshot(Box::new(s))
    }

    #[test]
    fn desligado_sem_nada_a_gravar_so_fecha() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx) = assistant_app(dir.path(), Settings::default());
        let _ = update(&mut app, Msg::ConfigureAssistant);
        let _ = update(&mut app, Msg::SetupFinish);
        assert!(drain(&mut rx).is_empty());
        assert!(app.assistant_setup.is_none());
    }

    #[test]
    fn a_chave_digitada_e_limpa_ao_fechar_por_qualquer_caminho() {
        use crate::assistant::secret::Secret;
        let dir = tempfile::tempdir().unwrap();
        let esc = crate::ui_model::keys::KeyPress {
            key: crate::ui_model::keys::Key::Escape,
            ctrl: false,
            shift: false,
            alt: false,
            repeat: false,
        };
        let closes = [
            Msg::CloseAssistantSetup,
            Msg::KeyPressed(esc),
            Msg::SelectScreen(Screen::Dashboard),
        ];
        for close in closes {
            let (mut app, mut rx) = cloud_app(dir.path());
            app.snapshot.assistant.has_key = false;
            let _ = update(&mut app, Msg::ConfigureAssistant);
            let _ = update(&mut app, Msg::SetupChoice(ProviderChoice::Cloud));
            let _ = update(&mut app, Msg::SetupNext);
            let _ = update(&mut app, Msg::KeyInput(Secret::new("sk-or-y".into())));
            let label = format!("{close:?}");
            let _ = update(&mut app, close);
            assert!(app.assistant_setup.is_none(), "{label}");
            assert!(app.key_input.is_empty(), "{label}");
            assert!(drain(&mut rx).is_empty(), "{label}: nada e gravado");
        }
        // Janela fechada para a bandeja.
        let (mut app, _fake, mut rx) = with_tray(dir.path());
        let _ = update(&mut app, Msg::ConfigureAssistant);
        let _ = update(&mut app, Msg::KeyInput(Secret::new("sk-or-z".into())));
        let _ = update(&mut app, Msg::CloseRequested);
        assert!(app.assistant_setup.is_none());
        assert!(app.key_input.is_empty());
        assert!(drain(&mut rx).is_empty());
        // "Remover" apaga a chave salva na hora.
        let (mut app, mut rx) = cloud_app(dir.path());
        let _ = update(&mut app, Msg::ConfigureAssistant);
        let _ = update(&mut app, Msg::RemoveKey);
        assert_eq!(
            drain(&mut rx),
            vec![Command::Assistant(AssistantCommand::SaveKey(Secret::new(
                String::new()
            )))]
        );
    }

    #[test]
    fn rascunho_digitado_sobrevive_ao_snapshot_do_boot_e_o_intocado_acompanha() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx) = assistant_app(dir.path(), Settings::default());
        // Dialogo aberto com o snapshot padrao (o nucleo ainda nao subiu).
        let _ = update(&mut app, Msg::ConfigureAssistant);
        let _ = update(&mut app, Msg::SetupPort("11500".into()));
        let mut boot = local_settings(Some("llama"));
        boot.local.port = 12345;
        boot.cloud.model = "x/modelo".into();
        let snap = snapshot_with(&app, |s| *s = boot.clone());
        let _ = update(&mut app, snap);
        let draft = &setup(&app).draft;
        assert_eq!(draft.port_input, "11500", "o digitado fica");
        assert_eq!(draft.choice, ProviderChoice::Local, "o intocado acompanha");
        assert_eq!(draft.local_model.as_deref(), Some("llama"));
        assert_eq!(draft.cloud_model, "x/modelo");
        let _ = update(&mut app, Msg::SetupNext);
        let _ = drain(&mut rx);
        app.snapshot.assistant.models = Some(Ok(vec!["llama".into()]));
        app.snapshot.assistant.models_port = Some(11500);
        let _ = update(&mut app, Msg::SetupFinish);
        let mut expected = boot.clone();
        expected.local.port = 11500;
        assert_eq!(
            drain(&mut rx),
            vec![Command::Assistant(AssistantCommand::Configure {
                settings: expected,
                key: None,
            })]
        );
    }

    #[test]
    fn porta_do_dialogo_tem_no_maximo_5_digitos() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx) = assistant_app(dir.path(), Settings::default());
        let _ = update(&mut app, Msg::ConfigureAssistant);
        let _ = update(&mut app, Msg::SetupPort("1234567".into()));
        assert_eq!(setup(&app).draft.port_input, "12345");
    }

    #[test]
    fn ollama_so_grava_com_modelo_da_lista_desta_porta() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx) = assistant_app(dir.path(), Settings::default());
        let _ = update(&mut app, Msg::ConfigureAssistant);
        let _ = update(&mut app, Msg::SetupChoice(ProviderChoice::Local));
        let _ = update(&mut app, Msg::SetupNext);
        let _ = update(&mut app, Msg::SetupLocalModel("llama".into()));
        let _ = drain(&mut rx);
        // Lista de outra porta, ou sem o modelo escolhido: nao grava.
        app.snapshot.assistant.models = Some(Ok(vec!["llama".into()]));
        app.snapshot.assistant.models_port = Some(11500);
        let _ = update(&mut app, Msg::SetupFinish);
        app.snapshot.assistant.models_port = Some(11434);
        app.snapshot.assistant.models = Some(Ok(vec!["qwen".into()]));
        let _ = update(&mut app, Msg::SetupFinish);
        app.snapshot.assistant.models = Some(Err("sem Ollama".into()));
        let _ = update(&mut app, Msg::SetupFinish);
        assert!(drain(&mut rx).is_empty());
        app.snapshot.assistant.models = Some(Ok(vec!["qwen".into(), "llama".into()]));
        let _ = update(&mut app, Msg::SetupFinish);
        assert_eq!(drain(&mut rx).len(), 1);
    }

    #[test]
    fn chave_so_com_espacos_nao_conta_e_remover_limpa_o_campo() {
        use crate::assistant::secret::Secret;
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx) = assistant_app(dir.path(), Settings::default());
        let _ = update(&mut app, Msg::ConfigureAssistant);
        let _ = update(&mut app, Msg::SetupChoice(ProviderChoice::Cloud));
        let _ = update(&mut app, Msg::SetupNext);
        let _ = update(&mut app, Msg::KeyInput(Secret::new("   ".into())));
        let _ = update(&mut app, Msg::SetupFinish);
        assert!(drain(&mut rx).is_empty());
        assert!(app.assistant_setup.is_some());
        let _ = update(&mut app, Msg::KeyInput(Secret::new("sk-or-x".into())));
        let _ = update(&mut app, Msg::RemoveKey);
        assert!(app.key_input.is_empty());
        assert_eq!(
            drain(&mut rx),
            vec![Command::Assistant(AssistantCommand::SaveKey(Secret::new(
                String::new()
            )))]
        );
        // Gravando: "Remover" nao faz nada.
        if let Some(s) = app.assistant_setup.as_mut() {
            s.saving = Some(1);
        }
        let _ = update(&mut app, Msg::RemoveKey);
        assert!(drain(&mut rx).is_empty());
    }

    #[test]
    fn nada_mudou_nada_e_gravado_no_ollama_e_na_nuvem() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx) = assistant_app(dir.path(), local_settings(Some("m")));
        app.snapshot.assistant.models = Some(Ok(vec!["m".into()]));
        app.snapshot.assistant.models_port = Some(11434);
        let _ = update(&mut app, Msg::ConfigureAssistant);
        let _ = update(&mut app, Msg::SetupNext);
        let _ = update(&mut app, Msg::SetupFinish);
        assert!(drain(&mut rx).is_empty());
        assert!(app.assistant_setup.is_none());

        let (mut app, mut rx) = cloud_app(dir.path());
        let _ = update(&mut app, Msg::ConfigureAssistant);
        let _ = update(&mut app, Msg::SetupNext);
        let _ = update(&mut app, Msg::SetupFinish);
        assert!(drain(&mut rx).is_empty());
        assert!(app.assistant_setup.is_none());
    }

    #[test]
    fn chave_digitada_com_ollama_escolhido_nao_vai() {
        use crate::assistant::secret::Secret;
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx) = assistant_app(dir.path(), Settings::default());
        let _ = update(&mut app, Msg::ConfigureAssistant);
        let _ = update(&mut app, Msg::SetupChoice(ProviderChoice::Cloud));
        let _ = update(&mut app, Msg::SetupNext);
        let _ = update(&mut app, Msg::KeyInput(Secret::new("sk-or-x".into())));
        let _ = update(&mut app, Msg::SetupBack);
        let _ = update(&mut app, Msg::SetupChoice(ProviderChoice::Local));
        let _ = update(&mut app, Msg::SetupNext);
        let _ = update(&mut app, Msg::SetupLocalModel("m".into()));
        app.snapshot.assistant.models = Some(Ok(vec!["m".into()]));
        app.snapshot.assistant.models_port = Some(11434);
        let _ = drain(&mut rx);
        let _ = update(&mut app, Msg::SetupFinish);
        assert_eq!(
            drain(&mut rx),
            vec![Command::Assistant(AssistantCommand::Configure {
                settings: local_settings(Some("m")),
                key: None,
            })]
        );
        assert!(app.key_input.is_empty());
    }

    #[test]
    fn resultado_de_outra_gravacao_nao_fecha_o_dialogo() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx) = assistant_app(dir.path(), local_settings(Some("m")));
        // Uma gravacao anterior ja terminou com erro (numero 1).
        app.snapshot.assistant.configure_result = Some((1, Err("antigo".into())));
        let _ = update(&mut app, Msg::ConfigureAssistant);
        let _ = update(&mut app, Msg::SetupChoice(ProviderChoice::None));
        let _ = update(&mut app, Msg::SetupFinish);
        assert_eq!(setup(&app).saving, Some(2));
        let _ = drain(&mut rx);
        // Snapshot que ainda traz o resultado 1: o dialogo continua esperando o 2.
        let same = Msg::Snapshot(Box::new(app.snapshot.clone()));
        let _ = update(&mut app, same);
        assert_eq!(setup(&app).saving, Some(2));
        assert_eq!(setup(&app).error, None);
        let off = Settings {
            provider: ProviderChoice::None,
            ..local_settings(Some("m"))
        };
        let done = configured(&app, Some(&off), None);
        let _ = update(&mut app, done);
        assert!(app.assistant_setup.is_none());
    }

    // ---- nuvem: configuracao, chave e previa ----

    pub(crate) fn cloud_settings() -> Settings {
        let mut s = Settings {
            provider: ProviderChoice::Cloud,
            ..Settings::default()
        };
        s.cloud.enabled = true;
        s
    }

    pub(crate) fn cloud_app(dir: &std::path::Path) -> (App, mpsc::UnboundedReceiver<Command>) {
        let (mut app, rx) = assistant_app(dir, cloud_settings());
        app.snapshot.assistant.has_key = true;
        (app, rx)
    }

    pub(crate) fn awaiting_conv(book: Book, envelope: &str) -> OpenConversation {
        OpenConversation {
            id: "C1".into(),
            book,
            envelope: envelope.into(),
            messages: vec![msg(
                MessageRole::User,
                crate::assistant::prompt::OPENING_REQUEST,
            )],
            needs_cloud_consent: true,
        }
    }

    #[test]
    fn conversar_na_nuvem_cria_a_conversa_mas_nao_pede_a_abertura() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx) = cloud_app(dir.path());
        let env = app.current_envelope();
        let _ = update(&mut app, Msg::StartConversation);
        // Sem `Resend`: o primeiro envio so sai depois da previa aprovada na tela.
        assert_eq!(
            drain(&mut rx),
            vec![Command::Assistant(AssistantCommand::NewConversation {
                book: app.book,
                title: format!("Análise de {}", month_label(&app.analysis.month)),
                envelope: env,
            })]
        );
    }

    #[test]
    fn nuvem_desligada_ou_sem_chave_abre_a_configuracao_em_vez_de_conversar() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx) = cloud_app(dir.path());
        app.snapshot.assistant.has_key = false;
        let _ = update(&mut app, Msg::StartConversation);
        assert!(app.assistant_setup.is_some());
        assert!(drain(&mut rx).is_empty());

        let (mut app, mut rx) = cloud_app(dir.path());
        app.snapshot.assistant.settings.cloud.enabled = false;
        let _ = update(&mut app, Msg::StartConversation);
        assert!(app.assistant_setup.is_some());
        assert!(drain(&mut rx).is_empty());
    }

    #[test]
    fn previa_aprovada_manda_o_id_da_conversa_e_so_quando_ha_o_que_aprovar() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx) = cloud_app(dir.path());
        // Nada aberto: o clique nao manda nada.
        let _ = update(&mut app, Msg::ApproveCloudSend);
        assert!(drain(&mut rx).is_empty());

        app.snapshot.assistant.open = Some(awaiting_conv(app.book, "ENV DA CONVERSA"));
        assert!(super::awaiting_cloud_consent(&app).is_some());
        // Aprovar e so da previa aberta, presa a conversa.
        app.consent_preview = Some("C1".into());
        let _ = update(&mut app, Msg::ApproveCloudSend);
        assert_eq!(
            drain(&mut rx),
            vec![Command::Assistant(AssistantCommand::ApproveCloudSend(
                "C1".into()
            ))]
        );

        // Enquanto espera, nem pergunta nova nem "Tentar de novo" saem.
        app.chat_input = "e agora?".into();
        let _ = update(&mut app, Msg::ChatSubmit);
        let _ = update(&mut app, Msg::ChatRetry);
        assert!(drain(&mut rx).is_empty());
        assert_eq!(app.chat_input, "e agora?");

        // Conversa da outra conta, ja aprovada, ou destino local: nao ha o que aprovar.
        app.snapshot.assistant.open = Some(awaiting_conv(Book::Business, "E"));
        assert!(super::awaiting_cloud_consent(&app).is_none());
        let mut approved = awaiting_conv(app.book, "E");
        approved.needs_cloud_consent = false;
        app.snapshot.assistant.open = Some(approved);
        assert!(super::awaiting_cloud_consent(&app).is_none());
        app.snapshot.assistant.open = Some(awaiting_conv(app.book, "E"));
        app.snapshot.assistant.settings = local_settings(Some("m"));
        assert!(super::awaiting_cloud_consent(&app).is_none());
        let _ = update(&mut app, Msg::ApproveCloudSend);
        let _ = update(&mut app, Msg::ChatRetry);
        assert_eq!(
            drain(&mut rx),
            vec![Command::Assistant(AssistantCommand::Resend)]
        );
    }

    pub(crate) fn snapshot_with(app: &App, change: impl FnOnce(&mut Settings)) -> Msg {
        let mut s = app.snapshot.clone();
        change(&mut s.assistant.settings);
        Msg::Snapshot(Box::new(s))
    }

    #[test]
    fn previa_nao_abre_com_snapshot_que_ainda_traz_a_conversa_anterior() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx) = cloud_app(dir.path());
        // A conversa anterior (C1) esta aberta e tambem espera aprovacao.
        app.snapshot.assistant.open = Some(awaiting_conv(app.book, "ENV"));
        let _ = update(&mut app, Msg::StartConversation);
        let _ = drain(&mut rx);
        assert_eq!(app.preview_after, Some(Some("C1".into())));
        // O snapshot de antes do comando ainda traz C1: a previa nao abre para ela.
        let old = Msg::Snapshot(Box::new(app.snapshot.clone()));
        let _ = update(&mut app, old);
        assert!(app.consent_preview.is_none());
        // Chega a conversa nova: abre para ela.
        let snap = {
            let mut s = app.snapshot.clone();
            let mut fresh = awaiting_conv(app.book, "ENV");
            fresh.id = "C2".into();
            s.assistant.open = Some(fresh);
            Msg::Snapshot(Box::new(s))
        };
        let _ = update(&mut app, snap);
        assert_eq!(app.consent_preview.as_deref(), Some("C2"));
        // Trocar de tela fecha a previa e esquece o pedido.
        let _ = update(&mut app, Msg::SelectScreen(Screen::Dashboard));
        assert!(app.consent_preview.is_none());
        assert!(app.preview_after.is_none());
    }

    pub(crate) fn summary(id: &str, book: Book) -> ConversationSummary {
        ConversationSummary {
            id: id.into(),
            book,
            title: format!("Conversa {id}"),
            created_at: String::new(),
        }
    }

    #[test]
    fn apagar_conversa_sai_no_prazo_na_troca_de_tela_e_antes_do_shutdown() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx) = assistant_app(dir.path(), local_settings(Some("m")));
        let delete = |id: &str| Command::Assistant(AssistantCommand::Delete(id.into()));
        // Prazo: antes dele nada sai.
        let _ = update(&mut app, Msg::AskDeleteConversation("C1".into()));
        let _ = update(&mut app, Msg::Tick);
        assert!(drain(&mut rx).is_empty());
        app.pending_removal.as_mut().unwrap().deadline = Instant::now();
        let _ = update(&mut app, Msg::Tick);
        assert_eq!(drain(&mut rx), [delete("C1")]);
        // Troca de tela.
        let _ = update(&mut app, Msg::AskDeleteConversation("C2".into()));
        let _ = update(&mut app, Msg::SelectScreen(Screen::Dashboard));
        assert_eq!(drain(&mut rx), [delete("C2")]);
        // Outro apagar manda o anterior.
        let _ = update(&mut app, Msg::AskDeleteConversation("C3".into()));
        let _ = update(&mut app, Msg::AskDeleteConversation("C4".into()));
        assert_eq!(drain(&mut rx), [delete("C3")]);
        // Fechar (sem bandeja, desliga): o apagar sai antes do `Shutdown`.
        let _ = update(&mut app, Msg::CloseRequested);
        assert_eq!(drain(&mut rx), [delete("C4"), Command::Shutdown]);
    }

    pub(crate) fn deliver(app: &mut App, change: impl FnOnce(&mut Snapshot)) {
        let mut snap = app.snapshot.clone();
        change(&mut snap);
        let _ = update(app, Msg::Snapshot(Box::new(snap)));
    }

    #[test]
    fn apagar_que_falhou_traz_a_conversa_de_volta_com_o_aviso() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx) = assistant_app(dir.path(), local_settings(Some("m")));
        app.snapshot.assistant.conversations = vec![summary("P1", Book::Personal)];
        let _ = update(&mut app, Msg::AskDeleteConversation("P1".into()));
        app.pending_removal.as_mut().unwrap().deadline = Instant::now();
        let _ = update(&mut app, Msg::Tick);
        assert!(conversation_ids(&app).is_empty());
        deliver(&mut app, |s| {
            s.assistant.last_error = Some("Não foi possível apagar.".into());
        });
        assert_eq!(conversation_ids(&app), ["P1"]);
        assert!(app.deleting.is_empty());
    }

    #[test]
    fn apagar_de_novo_a_mesma_conversa_nao_reinicia_o_prazo() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx) = assistant_app(dir.path(), local_settings(Some("m")));
        let _ = update(&mut app, Msg::AskDeleteConversation("C1".into()));
        let deadline = app.pending_removal.as_ref().unwrap().deadline;
        let _ = update(&mut app, Msg::AskDeleteConversation("C1".into()));
        assert_eq!(app.pending_removal.as_ref().unwrap().deadline, deadline);
        assert!(drain(&mut rx).is_empty());
    }

    #[test]
    fn conversa_pendente_sobrevive_a_troca_de_conta_e_sai_no_prazo() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) =
            app_with(business_snapshot(true, false), dir.path().to_path_buf());
        app.snapshot.assistant.conversations = vec![summary("P1", Book::Personal)];
        let _ = update(&mut app, Msg::AskDeleteConversation("P1".into()));
        let _ = update(&mut app, Msg::SelectBook(Book::Business));
        let _ = update(&mut app, Msg::SelectBook(Book::Personal));
        assert!(conversation_ids(&app).is_empty());
        assert!(
            !drain(&mut rx)
                .iter()
                .any(|c| matches!(c, Command::Assistant(AssistantCommand::Delete(_))))
        );
        app.pending_removal.as_mut().unwrap().deadline = Instant::now();
        let _ = update(&mut app, Msg::Tick);
        assert!(
            drain(&mut rx).contains(&Command::Assistant(AssistantCommand::Delete("P1".into())))
        );
        assert!(conversation_ids(&app).is_empty());
    }

    #[test]
    fn com_bandeja_fechar_e_desligar_mandam_a_conversa_pendente() {
        let dir = tempfile::tempdir().unwrap();
        let delete = Command::Assistant(AssistantCommand::Delete("C1".into()));
        let (mut app, _fake, mut rx) = with_tray(dir.path());
        let _ = update(&mut app, Msg::AskDeleteConversation("C1".into()));
        let _ = update(&mut app, Msg::CloseRequested);
        assert_eq!(drain(&mut rx), std::slice::from_ref(&delete));
        assert!(!app.closing);
        let _ = update(&mut app, Msg::AskDeleteConversation("C1".into()));
        let _ = update(&mut app, Msg::Desktop(DesktopEvent::Quit));
        assert_eq!(drain(&mut rx), [delete, Command::Shutdown]);
    }

    #[test]
    fn conversa_depois_de_aparelho_manda_o_aparelho_e_o_toast_e_da_conversa() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with_two_people(dir.path());
        let _ = update(&mut app, Msg::AskRemove("D1".into()));
        let _ = update(&mut app, Msg::ConfirmRemove);
        let _ = update(&mut app, Msg::AskDeleteConversation("C1".into()));
        assert!(matches!(
            drain(&mut rx).as_slice(),
            [Command::RemoveDevice { device_id, .. }] if device_id == "D1"
        ));
        let pending = app.pending_removal.clone().unwrap();
        assert_eq!(pending.conversation_id(), Some("C1"));
        assert_eq!(pending.toast, "Conversa apagada");
        // Desfazer vale para a conversa; a remocao ja saiu.
        let _ = update(&mut app, Msg::UndoRemove);
        assert!(drain(&mut rx).is_empty());
    }

    #[test]
    fn aparelho_depois_de_conversa_manda_a_conversa_e_o_toast_e_do_aparelho() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with_two_people(dir.path());
        let _ = update(&mut app, Msg::AskDeleteConversation("C1".into()));
        let _ = update(&mut app, Msg::AskRemove("D1".into()));
        let _ = update(&mut app, Msg::ConfirmRemove);
        assert_eq!(
            drain(&mut rx),
            [Command::Assistant(AssistantCommand::Delete("C1".into()))]
        );
        let pending = app.pending_removal.clone().unwrap();
        assert_eq!(pending.device_id(), Some("D1"));
        assert_eq!(pending.toast, "Pixel 7 foi removido do hub");
        let _ = update(&mut app, Msg::UndoRemove);
        assert!(drain(&mut rx).is_empty());
        assert_eq!(app.pending_removal, None);
    }

    #[test]
    fn apagar_todas_nao_manda_a_remocao_de_aparelho_pendente() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with_two_people(dir.path());
        let _ = update(&mut app, Msg::AskRemove("D1".into()));
        let _ = update(&mut app, Msg::ConfirmRemove);
        let _ = update(&mut app, Msg::DeleteAllConversations);
        let _ = update(&mut app, Msg::DeleteAllConversations);
        assert_eq!(
            drain(&mut rx),
            [Command::Assistant(AssistantCommand::DeleteAll)]
        );
        assert_eq!(
            app.pending_removal.as_ref().unwrap().device_id(),
            Some("D1")
        );
    }

    #[test]
    fn esconder_a_conversa_limpa_o_hover_da_linha() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx) = assistant_app(dir.path(), local_settings(Some("m")));
        app.hover_row = Some("C1".into());
        let _ = update(&mut app, Msg::AskDeleteConversation("C1".into()));
        assert_eq!(app.hover_row, None);
    }

    #[test]
    fn apagar_todas_manda_o_apagar_pendente_antes() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx) = assistant_app(dir.path(), local_settings(Some("m")));
        let _ = update(&mut app, Msg::AskDeleteConversation("C1".into()));
        let _ = update(&mut app, Msg::DeleteAllConversations);
        let _ = update(&mut app, Msg::DeleteAllConversations);
        assert_eq!(
            drain(&mut rx),
            [
                AssistantCommand::Delete("C1".into()),
                AssistantCommand::DeleteAll
            ]
            .map(Command::Assistant)
        );
    }

    #[test]
    fn trocar_de_conta_fecha_a_conversa_aberta_da_outra() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _tx) = app_with_business(dir.path(), true, false);
        // `app_with_business` descarta o receptor; troca por um canal nosso.
        let (cmd_tx, mut rx) = mpsc::unbounded_channel();
        app.handle.commands = cmd_tx;
        app.snapshot.assistant.open = Some(open_conv(Book::Personal, vec![]));
        // Conversa da conta que fica: nada a fechar.
        let _ = update(&mut app, Msg::SelectBook(Book::Personal));
        assert!(drain(&mut rx).is_empty());
        let _ = update(&mut app, Msg::SelectBook(Book::Business));
        assert_eq!(app.book, Book::Business);
        assert_eq!(
            drain(&mut rx),
            vec![Command::Assistant(AssistantCommand::Close)]
        );
        // Sem conversa aberta, trocar de conta nao manda nada.
        app.snapshot.assistant.open = None;
        let _ = update(&mut app, Msg::SelectBook(Book::Personal));
        assert!(drain(&mut rx).is_empty());
    }

    pub(crate) fn conversation_ids(app: &App) -> Vec<String> {
        analysis::conversations_of_book(app)
            .iter()
            .map(|c| c.id.clone())
            .collect()
    }
}
