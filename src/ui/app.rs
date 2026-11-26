//! Estado e mensagens da janela. O estado e o ultimo snapshot do nucleo mais o que e da
//! propria tela: tela atual, tema, janela (id, maximizada), QR desenhado, se esta fechando.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use iced::widget::{canvas, image, qr_code};
use iced::{Element, Size, Subscription, Task, window};
use time::macros::format_description;
use time::{OffsetDateTime, UtcOffset};
use tokio::sync::watch;

use super::assistant_settings_view::SetupStep;
use super::connection_view::{self, ConnectFlow, ConnectStep, PhoneOs};
use super::remove_view;
use super::theme::Tokens;
use crate::assistant::secret::Secret;
use crate::assistant::settings::ProviderChoice;
use crate::assistant::view::OpenConversation;
use crate::config::{self, ThemeMode, UiPrefs};
use crate::dashboard::contract::Book;
use crate::dashboard::dataset::{Dataset, avatar_bytes};
use crate::dashboard::grafite::{GrafiteView, build_grafite};
use crate::dashboard::list::{Filters, NONE_KEY, TypeFilter};
use crate::dashboard::people::{DeviceInfo, Person, people};
use crate::dashboard::periods::{month_label, month_window};
use crate::dashboard::view::{ReservesView, TransactionsView, build_reserves, build_transactions};
use crate::desktop::{Desktop, DesktopEvent, EventReceiver};
use crate::hub::snapshot::{DeviceView, Snapshot, Status};
use crate::hub::{AssistantCommand, Command, HubHandle};
use crate::store::erase::{Area, AreaSet};

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
    /// passa por `Message::Copy` (o texto depende da mensagem): sai por `CopyReply`.
    Reply,
}

/// Painel visivel da Analise na janela estreita (segmentado [Achados | Conversa]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AnalysisTab {
    #[default]
    Findings,
    Chat,
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
    pub(super) fn shows_values(self) -> bool {
        matches!(
            self,
            Screen::Dashboard | Screen::Transactions | Screen::Reserves | Screen::Analysis
        )
    }
}

pub struct App {
    pub(super) handle: HubHandle,
    pub(super) snapshot: Snapshot,
    /// Reconstruido so quando o token muda: gerar a matriz a cada quadro e desperdicio.
    pub(super) qr: Option<qr_code::Data>,
    pub(super) closing: bool,
    /// Fuso local lido uma vez no `main`, antes de existirem outras threads.
    pub(super) offset: UtcOffset,
    /// `None` enquanto nao ha janela (boot por `--autostart`, ou fechada); sem ela os botoes
    /// da barra nao fazem nada (e o `warn!` sai uma vez so).
    pub(super) window: Option<window::Id>,
    pub(super) warned_no_window: bool,
    /// Maximizada, a moldura de redimensionar some: borda de janela maximizada nao arrasta.
    pub(super) maximized: bool,
    /// Botao pressionado na barra de titulo e ainda sem arrasto: o arrasto so comeca quando o
    /// mouse anda, para o duplo clique (maximizar) nao ser engolido pelo laco de movimento do
    /// Windows.
    pub(super) title_pressed: bool,
    /// A preferencia (Escuro, Claro ou Sistema); o que se desenha e `tokens`.
    pub(super) theme: ThemeMode,
    /// O sistema esta no escuro? So pesa com `ThemeMode::System`. Nasce do registro (ou escuro,
    /// o padrao do design, quando o sistema nao diz) e segue as trocas avisadas.
    pub(super) system_dark: bool,
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
    /// Modal "Remover ...?" (ou "Dados de ...", para aparelho ja removido) aberto.
    pub(super) confirm_remove: Option<RemoveDialog>,
    pub(super) pending_removal: Option<PendingRemoval>,
    /// "Depois" no aviso de aparelhos removidos com dados: so esta sessao, nada e gravado. O
    /// aviso volta na proxima abertura porque a deteccao e derivada do banco.
    pub(super) orphans_postponed: bool,
    /// "Hoje" (`YYYY-MM-DD`) no fuso local: a unica leitura de relogio que decide algo na tela
    /// (mes corrente, tag "Parcial"). Nasce no boot e e reconferida a cada tique.
    pub(super) today: String,
    /// Os 12 meses navegaveis (`month_window(today)`), o mais antigo primeiro.
    pub(super) months: Vec<String>,
    /// Indice em `months`; 11 = mes corrente.
    pub(super) month_idx: usize,
    /// Conta do dashboard (seletor Pessoal | Comercial). So estado de UI: nada e gravado, e a
    /// janela sempre abre na pessoal.
    pub(super) book: Book,
    /// O dataset do snapshot visto por `book`: toda tela e contagem le daqui, nunca do total
    /// (`Dataset::for_book`). Recalculado em `rebuild`, junto com os modelos de tela.
    pub(super) scope: Dataset,
    /// Filtros da tela Lancamentos; sobrevivem a troca de mes.
    pub(super) filters: Filters,
    /// Modelos de tela, recalculados so quando dados, mes, filtros ou "hoje" mudam: desenhar
    /// um quadro nunca percorre o dataset.
    pub(super) dashboard: GrafiteView,
    /// "Ver todas as N" de "Próximas saídas" abriu a lista no lugar. Volta a fechar ao trocar
    /// de mes ou de conta, nao a cada sync (fecharia na cara de quem esta lendo).
    pub(super) upcoming_expanded: bool,
    pub(super) transactions: TransactionsView,
    /// Tela Reservas: sem mes escolhido (saldos sao acumulados), so depende de dados e "hoje".
    pub(super) reserves: ReservesView,
    /// Tela Analise: achados da conta aberta (`advisor`), recalculados com os dados e o "hoje".
    pub(super) analysis: crate::advisor::Analysis,
    /// Previa "Ver o que o assistente vê" aberta: o texto exato de `envelope::render`, sem
    /// mascara (a tela mascara ao desenhar; o que sai é sempre o texto inteiro). Fica guardado
    /// para `rebuild` poder refrescá-lo: dados novos com a prévia aberta não podem deixá-la
    /// mostrando um envelope que já não é o que seria enviado.
    pub(super) envelope_preview: Option<String>,
    /// Menu de categoria da tela Lancamentos aberto.
    pub(super) cat_menu_open: bool,
    /// Geometria dos graficos; limpa quando dados, mes, tema ou "Ocultar valores" mudam (os
    /// rotulos em R$ ficam gravados nela).
    pub(super) pace_cache: canvas::Cache,
    pub(super) history_cache: canvas::Cache,
    pub(super) empty_cache: canvas::Cache,
    /// Grafico empilhado e esqueleto vazio da tela Reservas.
    pub(super) reserves_cache: canvas::Cache,
    pub(super) reserves_empty_cache: canvas::Cache,
    /// `user_id` -> foto, para as linhas e os chips de autor (`avatars` e por aparelho).
    pub(super) user_avatars: HashMap<String, Avatar>,
    /// `open::that_detached` falhou: a linha da versao mostra a URL com "Copiar".
    pub(super) releases_open_failed: bool,
    /// Bandeja, "iniciar com o Windows" (ou `Headless`, sem nada disso). `Rc`: o `TrayIcon`
    /// vive na thread principal, a mesma do `update`.
    pub(super) desktop: Rc<dyn Desktop>,
    /// Ultimo texto mandado ao tooltip da bandeja.
    pub(super) tooltip: String,
    /// "Iniciar com o Windows" como o sistema diz (cache do registro, lido no boot e depois de
    /// cada troca); `None` = sistema sem a opcao, e a secao some de Ajustes.
    pub(super) autostart: Option<bool>,
    /// O que o `ui.json` lembra (`Some(false)` = desligado de proposito); vai junto em toda
    /// gravacao do arquivo para trocar o tema nao apagar a decisao.
    pub(super) autostart_pref: Option<bool>,
    /// A ultima troca da opcao falhou no sistema: a linha em Ajustes avisa.
    pub(super) autostart_failed: bool,
    /// "Ocultar valores" do Dashboard, lembrado no `ui.json` como o tema.
    pub(super) hide_values: bool,
    /// Texto digitado no chat da Analise.
    pub(super) chat_input: String,
    /// Painel da Analise no segmentado da janela estreita.
    pub(super) analysis_tab: AnalysisTab,
    /// Respostas do assistente ja interpretadas (Markdown), pelo id da mensagem. O `view` so le
    /// (o Markdown do iced desenha a partir de itens que precisam viver fora dele); quem monta
    /// e `refresh_replies`, a cada snapshot e a cada troca do olho.
    pub(super) replies: HashMap<String, super::analysis_view::CachedReply>,
    /// A resposta cujo "Copiar" virou "Copiado" (`CopyTarget::Reply`).
    pub(super) copied_reply: Option<String>,
    /// Dialogo "Configurar assistente" (8.6); `None` = fechado.
    pub(super) assistant_setup: Option<super::assistant_settings_view::AssistantSetup>,
    /// A previa de envio a nuvem aberta (8.5), pela conversa que ela mostra. Presa ao id: so
    /// aparece enquanto essa conversa for a aberta e ainda esperar aprovacao; reabrir do
    /// historico uma conversa local com a nuvem configurada nao a faz saltar sozinha.
    pub(super) consent_preview: Option<String>,
    /// "Conversar" na nuvem pediu a previa: ela abre quando o snapshot trouxer a conversa nova.
    /// Guarda a conversa aberta no clique, para nao abrir a previa de outra que o snapshot
    /// anterior ao comando ainda traga.
    pub(super) preview_after: Option<Option<String>>,
    /// A chave do OpenRouter que a pessoa esta digitando no dialogo. Vive so aqui e so ate o
    /// ultimo botao (`Configure` a leva ao nucleo e o campo esvazia na hora) ou ate o dialogo
    /// fechar por qualquer caminho (`close_setup`); nunca vai para log nem Debug (por isso a
    /// `Secret` nas mensagens e no comando).
    pub(super) key_input: String,
    /// O envelope desta conta agora (nunca mascarado), recalculado so em `rebuild`: a previa, o
    /// envio e o aviso "os dados mudaram" (spec 3) leem o mesmo texto sem refazer as agregacoes
    /// a cada quadro. O aviso compara com o envelope guardado na conversa, entao vale tambem
    /// para conversa reaberta do historico e nao dispara com sync de outra conta.
    pub(super) envelope_now: String,
    /// "Apagar todas" armado: o segundo clique antes do prazo apaga. Expira no `Tick`, para um
    /// clique esquecido nao deixar o botao perigoso armado para sempre.
    pub(super) confirm_delete_all: Option<Instant>,
    /// Conversas cujo `Delete` ja saiu para o nucleo e que o snapshot ainda traz: continuam
    /// escondidas ate um snapshot sem elas (senao reapareceriam por um instante depois do
    /// prazo do "Desfazer") ou ate um erro novo do assistente, quando o apagar falhou e a
    /// conversa volta junto com o aviso.
    pub(super) deleting: HashSet<String>,
    /// Fluxo "Conectar um celular" (spec 8.4): passo, sistema escolhido e se e o dialogo ou o
    /// fluxo em tela. `None` = fechado (o fluxo em tela, sem aparelho, nasce no passo 1).
    pub(super) connect: Option<ConnectFlow>,
    /// Aviso curto sem "Desfazer" e o prazo dele. Separado do `pending_removal` de proposito:
    /// aquele vira comando para o nucleo ao vencer, trocar de tela ou fechar.
    pub(super) notice: Option<(String, Instant)>,
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
    /// Seletor Pessoal | Comercial.
    SelectBook(Book),
    /// Olho do cabecalho do Dashboard: "Ocultar valores" / "Mostrar valores".
    ToggleHideValues,
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
    WindowOpened(window::Id),
    /// O iced fechou a janela (`close_events`): o id morreu com ela.
    WindowClosed(window::Id),
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
    Resize(window::Direction),
    SelectScreen(Screen),
    SetTheme(ThemeMode),
    /// "Configurar" do assistente (Ajustes e o painel da Analise): abre o dialogo em passos.
    ConfigureAssistant,
    /// O sistema trocou entre escuro e claro (`system::theme_changes`), ou respondeu a
    /// consulta do boot (`system::theme`).
    SystemTheme(iced::theme::Mode),
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
    KeyPressed(super::keys::KeyPress),
    /// Tab e Shift+Tab entre os campos de texto.
    FocusNext,
    FocusPrevious,
    Noop,
}

impl App {
    pub(super) fn new(
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
        let tokens = Tokens::for_mode(prefs.theme, system_dark);
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
            system_dark,
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
            pace_cache: canvas::Cache::new(),
            history_cache: canvas::Cache::new(),
            empty_cache: canvas::Cache::new(),
            reserves_cache: canvas::Cache::new(),
            reserves_empty_cache: canvas::Cache::new(),
            user_avatars: HashMap::new(),
            releases_open_failed: false,
            autostart: desktop.autostart(),
            autostart_pref: prefs.autostart,
            autostart_failed: false,
            hide_values: prefs.hide_values,
            chat_input: String::new(),
            analysis_tab: AnalysisTab::Findings,
            replies: HashMap::new(),
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
        };
        app.refresh_people();
        app.sync_tooltip();
        app.set_today(local_today(offset));
        app.refresh_replies();
        app
    }

    /// Interpreta as respostas da conversa aberta que ainda nao estao no cache (ou que mudaram
    /// de texto, ou de olho) e esquece as de conversas que sairam da tela. So o que muda e
    /// refeito: o parse do Markdown nao roda a cada quadro.
    pub(super) fn refresh_replies(&mut self) {
        let hidden = self.hide_values;
        let Some(open) = self.snapshot.assistant.open.as_ref() else {
            self.replies.clear();
            return;
        };
        let current: HashSet<&str> = open
            .messages
            .iter()
            .filter(|m| m.role == crate::assistant::view::MessageRole::Assistant)
            .map(|m| m.id.as_str())
            .collect();
        self.replies.retain(|id, _| current.contains(id.as_str()));
        for m in &open.messages {
            if m.role != crate::assistant::view::MessageRole::Assistant {
                continue;
            }
            let fresh = self
                .replies
                .get(&m.id)
                .is_some_and(|c| c.hidden == hidden && c.content == m.content);
            if !fresh {
                self.replies.insert(
                    m.id.clone(),
                    super::analysis_view::render_reply(&m.content, hidden),
                );
            }
        }
    }

    /// O que vai para o `ui.json`. Toda gravacao leva o arquivo inteiro: montar `UiPrefs` campo
    /// a campo em cada mensagem fazia trocar o tema apagar a escolha de outra opcao.
    fn prefs(&self) -> UiPrefs {
        UiPrefs {
            theme: self.theme,
            system_dark: self.theme == ThemeMode::System && self.system_dark,
            autostart: self.autostart_pref,
            hide_values: self.hide_values,
        }
    }

    /// Tooltip da bandeja a partir do snapshot; so manda ao sistema quando o texto muda
    /// (`Shell_NotifyIcon` a cada snapshot seria desperdicio).
    fn sync_tooltip(&mut self) {
        let active = self.snapshot.devices.iter().filter(|d| !d.revoked).count();
        let text = crate::desktop::tray::tooltip(&self.snapshot.status, active);
        if text != self.tooltip {
            self.desktop.set_tooltip(&text);
            self.tooltip = text;
        }
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

    /// Recalcula as tres telas de dados a partir do snapshot atual.
    pub(super) fn rebuild(&mut self) {
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
            self.assistant_setup = Some(super::assistant_settings_view::AssistantSetup::open(
                &self.snapshot.assistant.settings,
            ));
        }
    }

    /// Fecha o dialogo por qualquer caminho ("Cancelar", "x", scrim, Esc, gravacao que deu
    /// certo, janela fechada): a chave digitada e nao gravada nao fica na memoria.
    pub(super) fn close_setup(&mut self) {
        self.assistant_setup = None;
        self.key_input.clear();
    }

    /// Um snapshot novo chegou: o dialogo acompanha o gravado (campo que a pessoa nao mexeu) e
    /// termina a gravacao pendente (fecha se deu certo; com erro, fica aberto mostrando-o). A
    /// previa pedida abre quando a conversa nova aparece esperando aprovacao.
    fn sync_assistant_dialogs(&mut self) {
        let view = &self.snapshot.assistant;
        let mut done = false;
        if let Some(setup) = self.assistant_setup.as_mut() {
            setup.resync(&view.settings);
            if let Some(expected) = setup.saving
                && view.configured >= expected
            {
                if view.last_error.is_none() {
                    done = true;
                } else {
                    setup.saving = None;
                }
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
    pub(super) fn current_envelope(&self) -> String {
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

    /// Refaz os tokens a partir da preferencia e do sistema. So mexe no que se desenha (nunca
    /// grava o `ui.json`): a troca do sistema nao e escolha da pessoa.
    fn apply_theme(&mut self) {
        let tokens = Tokens::for_mode(self.theme, self.system_dark);
        if std::ptr::eq(tokens, self.tokens) {
            return;
        }
        self.tokens = tokens;
        self.iced_theme = tokens.iced_theme();
        // As cores dos graficos estao gravadas na geometria em cache.
        self.clear_charts();
    }

    /// O sistema trocou (ou disse pela primeira vez) entre escuro e claro.
    fn set_system_dark(&mut self, dark: bool) {
        self.system_dark = dark;
        self.apply_theme();
    }

    fn clear_charts(&self) {
        self.pace_cache.clear();
        self.history_cache.clear();
        self.empty_cache.clear();
        self.reserves_cache.clear();
        self.reserves_empty_cache.clear();
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
        let pending = self
            .pending_removal
            .as_ref()
            .and_then(PendingRemoval::device_id);
        self.people
            .iter()
            .filter(move |p| Some(p.device_id.as_str()) != pending)
    }

    pub(super) fn device(&self, device_id: &str) -> Option<&DeviceView> {
        self.snapshot
            .devices
            .iter()
            .find(|d| d.device_id == device_id)
    }

    /// Aparelhos removidos que ainda tem dados a decidir, menos o que ja tem um apagar
    /// pendente (some na hora, como a pessoa removida).
    pub(super) fn orphans(&self) -> Vec<&DeviceView> {
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
    pub(super) fn conversation_hidden(&self, id: &str) -> bool {
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
        connection_view::dialog_open(self) || connection_view::shows_in_screen_flow(self)
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
    desktop: Rc<dyn Desktop>,
    events: EventReceiver,
    open_window: bool,
) -> impl Fn() -> (App, Task<Message>) {
    // `BootFn` e `Fn`, e o receptor nao clona: o primeiro boot o leva; um segundo (presets de
    // teste do iced) nasce sem eventos da bandeja.
    let events = RefCell::new(Some(events));
    move || {
        let rx = handle.snapshot.clone();
        let mut app = App::new(
            handle.clone(),
            offset,
            prefs.clone(),
            prefs_dir.clone(),
            desktop.clone(),
        );
        let mut tasks = vec![Task::run(snapshot_stream(rx), |s| {
            Message::Snapshot(Box::new(s))
        })];
        if let Some(events) = events.borrow_mut().take() {
            tasks.push(Task::run(events, Message::Desktop));
        }
        // Fora do Windows o registro nao responde: o tema do sistema vem do iced.
        tasks.push(iced::system::theme().map(Message::SystemTheme));
        // Nucleo que ja falhou antes do boot (porta ocupada responde em milissegundos): o
        // primeiro `Snapshot` nao seria uma transicao para `Failed`, e um boot so com o icone
        // esconderia o erro.
        if open_window || matches!(app.snapshot.status, Status::Failed(_)) {
            tasks.push(open_window_task(&mut app));
        }
        (app, Task::batch(tasks))
    }
}

/// Abre a janela e guarda o id na hora: os botoes da barra de titulo nao precisam esperar
/// o `WindowOpened`.
fn open_window_task(app: &mut App) -> Task<Message> {
    let (id, task) = window::open(super::window_settings());
    app.window = Some(id);
    task.map(Message::WindowOpened)
}

/// Abre ou traz a janela. Com janela: desminimiza e pede foco (clique na bandeja e segunda
/// execucao dao ao processo o direito de foco; se o Windows recusar, o botao pisca).
fn open_or_focus(app: &mut App) -> Task<Message> {
    match app.window {
        Some(id) => Task::batch([window::minimize(id, false), window::gain_focus(id)]),
        None => open_window_task(app),
    }
}

/// `iced::exit()` mais um "despertador". O `exit` so marca o laco do winit para sair; no
/// Windows o laco confere a marca depois de acordar, e sem janela nada o acorda (nem redesenho,
/// nem evento de janela): o "Desligar o hub" com a janela fechada so terminava quando o
/// `ForceExit` de 5 s chegasse — e, se o nucleo travasse, nem ai. Uma mensagem qualquer logo
/// depois acorda o laco, que ve a marca e sai.
fn exit() -> Task<Message> {
    Task::batch([
        iced::exit(),
        Task::perform(delay(Duration::from_millis(50)), |_| Message::Noop),
    ])
}

/// Desligamento com ordem (o `CloseRequested` da 1.0.0): remocao pendente, `Shutdown`,
/// `Stopped` ou 5 s. Idempotente por `closing`.
fn begin_shutdown(app: &mut App) -> Task<Message> {
    if app.closing {
        return Task::none();
    }
    app.closing = true;
    // Antes do `Shutdown`, no mesmo canal: o nucleo processa em ordem.
    app.commit_pending();
    // Nucleo ja parado (ou morto): nao ha por que esperar o snapshot `Stopped`.
    if app.handle.commands.send(Command::Shutdown).is_err() {
        return exit();
    }
    // Garantia: se o nucleo travar, o processo sai mesmo assim em 5 s.
    Task::perform(delay(Duration::from_secs(5)), |_| Message::ForceExit)
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
    let task = handle(app, message);
    // Fluxo aberto que deixou de estar na tela (outra tela, "Desfazer" que trouxe um aparelho de
    // volta, nucleo que caiu): fecha e cancela o codigo, que nao pode valer sem ninguem ver.
    if app.connect.is_some() && !app.connect_showing() {
        app.close_connect();
    }
    task
}

fn handle(app: &mut App, message: Message) -> Task<Message> {
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
            app.refresh_replies();
            if people_changed {
                app.refresh_people();
                app.rebuild();
            }
            app.sync_tooltip();
            if stopped && app.closing {
                return exit();
            }
            // Boot por `--autostart` que falhou (porta ocupada, banco): sem janela ninguem
            // veria; o tooltip diz "com erro" e a janela abre sozinha. So na transicao para
            // `Failed`: quem fechar a janela depois de ver o erro nao a ve voltar sozinha.
            let failed = matches!(app.snapshot.status, Status::Failed(_));
            if failed && !was_failed && app.window.is_none() && !app.closing {
                return open_or_focus(app);
            }
            Task::none()
        }
        Message::IssueToken => {
            let _ = app.handle.commands.send(Command::IssuePairingToken);
            Task::none()
        }
        Message::Tick => {
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
            // No Windows a troca do tema do sistema nao chega pelo iced (ver
            // `desktop::appearance`): com "Sistema" ativo, o tique confere.
            if app.theme == ThemeMode::System
                && let Some(dark) = app.desktop.system_dark()
                && dark != app.system_dark
            {
                app.set_system_dark(dark);
            }
            Task::none()
        }
        Message::PrevMonth => {
            if app.month_idx > 0 {
                app.month_idx -= 1;
                app.upcoming_expanded = false;
                app.rebuild();
            }
            Task::none()
        }
        Message::NextMonth => {
            if app.month_idx + 1 < app.months.len() {
                app.month_idx += 1;
                app.upcoming_expanded = false;
                app.rebuild();
            }
            Task::none()
        }
        Message::SelectBook(book) => {
            // Comercial so com a conta existindo: a mensagem vem de um botao que so existe
            // nesse caso, mas o estado nao deve depender disso.
            let allowed = book == Book::Personal || app.snapshot.dashboard.has_business_book();
            if allowed && book != app.book {
                app.select_book(book);
                app.rebuild();
            }
            Task::none()
        }
        Message::ExpandUpcoming => {
            app.upcoming_expanded = true;
            Task::none()
        }
        Message::CloseRequested => {
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
            app.with_window(window::close)
        }
        Message::Desktop(DesktopEvent::Quit) => begin_shutdown(app),
        Message::Desktop(DesktopEvent::OpenWindow) => open_or_focus(app),
        Message::ForceExit => exit(),
        Message::WindowOpened(id) => {
            if app.window == Some(id) {
                window::is_maximized(id).map(Message::Maximized)
            } else {
                Task::none()
            }
        }
        Message::WindowClosed(id) => {
            if app.window == Some(id) {
                app.window = None;
                app.maximized = false;
                app.window_size = INITIAL_SIZE;
                // A geometria em cache pertence ao compositor, que morreu com a janela.
                app.clear_charts();
            }
            Task::none()
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
            let top = iced::widget::operation::snap_to(
                super::shell::CONTENT_SCROLL,
                iced::widget::operation::RelativeOffset::START,
            );
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
            // clique; isto cobre qualquer outro caminho), e a chave digitada vai junto.
            app.close_setup();
            app.screen = screen;
            app.cat_menu_open = false;
            top
        }
        Message::ConnectPhone => {
            let task = update(app, Message::SelectScreen(Screen::Connection));
            let modal = !connection_view::in_screen_case(app);
            if app.connect.is_none_or(|f| f.modal != modal) {
                app.connect = Some(ConnectFlow::new(modal));
            }
            task
        }
        Message::ConnectNext => {
            let modal = !connection_view::in_screen_case(app);
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
            Task::none()
        }
        Message::ConnectBack => {
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
            Task::none()
        }
        Message::ConnectOs(os) => {
            let modal = !connection_view::in_screen_case(app);
            app.connect
                .get_or_insert_with(|| ConnectFlow::new(modal))
                .os = os;
            Task::none()
        }
        Message::ConnectClose => {
            app.close_connect();
            Task::none()
        }
        Message::ConfigureAssistant => {
            // O dialogo e uma camada da janela: abre por cima de Ajustes ou da Analise, sem
            // trocar de tela.
            app.open_setup();
            Task::none()
        }
        Message::ToggleCategoryMenu => {
            app.cat_menu_open = !app.cat_menu_open;
            Task::none()
        }
        Message::CloseCategoryMenu => {
            app.cat_menu_open = false;
            Task::none()
        }
        Message::PickCategory(category) => {
            app.cat_menu_open = false;
            app.set_filters(|f| f.category = category);
            Task::none()
        }
        Message::ToggleAuthor(key) => {
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
            Task::none()
        }
        Message::AllAuthors => {
            app.set_filters(|f| f.authors.clear());
            Task::none()
        }
        Message::SetType(kind) => {
            app.set_filters(|f| f.kind = kind);
            Task::none()
        }
        Message::ClearFilters => {
            app.cat_menu_open = false;
            app.set_filters(Filters::clear);
            Task::none()
        }
        Message::ToggleHideValues => {
            app.hide_values = !app.hide_values;
            app.rebuild_valued();
            app.refresh_replies();
            // Os rotulos em R$ dos graficos estao gravados na geometria em cache.
            app.clear_charts();
            Task::perform(save_prefs(app.prefs(), app.prefs_dir.clone()), |_| {
                Message::Noop
            })
        }
        Message::SetTheme(mode) => {
            if mode == app.theme {
                return Task::none();
            }
            app.theme = mode;
            // Ao escolher "Sistema" o valor guardado pode ser velho (o tique so consulta com
            // "Sistema" ativo): le de novo antes de desenhar.
            if mode == ThemeMode::System
                && let Some(dark) = app.desktop.system_dark()
            {
                app.system_dark = dark;
            }
            app.apply_theme();
            Task::perform(save_prefs(app.prefs(), app.prefs_dir.clone()), |_| {
                Message::Noop
            })
        }
        Message::SystemTheme(mode) => {
            // Onde o registro responde (Windows), so ele manda: o aviso do winit la chega
            // atrasado ou com o valor anterior (ver `desktop::appearance`), e aceita-lo fazia
            // o tema piscar entre o velho e o novo, sobretudo com alto contraste.
            if app.desktop.system_dark().is_some() {
                return Task::none();
            }
            // `None` = o sistema nao tem preferencia: fica o ultimo valor conhecido.
            match mode {
                iced::theme::Mode::Dark => app.set_system_dark(true),
                iced::theme::Mode::Light => app.set_system_dark(false),
                iced::theme::Mode::None => {}
            }
            Task::none()
        }
        Message::SetAutostart(enabled) => {
            if app.autostart == Some(enabled) {
                return Task::none();
            }
            match app.desktop.set_autostart(enabled) {
                Ok(()) => {
                    app.autostart = Some(enabled);
                    app.autostart_pref = Some(enabled);
                    app.autostart_failed = false;
                    Task::perform(save_prefs(app.prefs(), app.prefs_dir.clone()), |_| {
                        Message::Noop
                    })
                }
                Err(err) => {
                    tracing::warn!("iniciar com o Windows: {err}");
                    app.autostart_failed = true;
                    Task::none()
                }
            }
        }
        Message::Copy(target) => {
            let text = match target {
                CopyTarget::Address => connection_view::hub_url(&app.snapshot),
                CopyTarget::Guide => connection_view::guide_url(&app.snapshot),
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
        Message::OpenReleases => {
            // O navegador do usuario vai ao GitHub por um clique dele; este processo nao abre
            // socket. `that_detached`, e nao `that`: em alguns Linux o `xdg-open` so volta
            // quando o navegador fecha, e `update` roda na thread da janela.
            if let Err(err) = open::that_detached(config::RELEASES_URL) {
                tracing::warn!("nao foi possivel abrir o navegador: {err}");
                app.releases_open_failed = true;
            }
            Task::none()
        }
        Message::CopyReply(id) => {
            let Some(m) = super::analysis_view::open_conversation(app)
                .and_then(|o| o.messages.iter().find(|m| m.id == id))
            else {
                return Task::none();
            };
            // O mesmo texto da tela: mascarado com o olho ligado (decisao registrada em
            // `CachedReply::copy`). Sem cache valido, monta na hora pelo mesmo caminho.
            let copy = match app
                .replies
                .get(&id)
                .filter(|c| c.hidden == app.hide_values && c.content == m.content)
            {
                Some(c) => c.copy.clone(),
                None => super::analysis_view::render_reply(&m.content, app.hide_values).copy,
            };
            app.copied = Some((CopyTarget::Reply, Instant::now() + COPIED_FOR));
            app.copied_reply = Some(id);
            Task::batch([
                iced::clipboard::write(copy),
                Task::perform(delay(COPIED_FOR), |_| Message::CopyExpired),
            ])
        }
        Message::AnalysisTab(tab) => {
            app.analysis_tab = tab;
            Task::none()
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
            app.confirm_remove = Some(RemoveDialog {
                device_id,
                step: RemoveStep::Choose,
                selected: AreaSet::default(),
            });
            Task::none()
        }
        Message::ToggleArea(area) => {
            let enabled = app
                .confirm_remove
                .as_ref()
                .and_then(|d| app.device(&d.device_id))
                .is_some_and(|d| d.data.items(area) > 0);
            if let Some(dialog) = app.confirm_remove.as_mut().filter(|_| enabled) {
                dialog.selected.toggle(area);
            }
            Task::none()
        }
        Message::SelectAllAreas => {
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
            Task::none()
        }
        Message::SelectNoAreas => {
            if let Some(dialog) = app.confirm_remove.as_mut() {
                dialog.selected = AreaSet::default();
            }
            Task::none()
        }
        Message::RemoveContinue => {
            let Some(dialog) = app.confirm_remove.as_mut() else {
                return Task::none();
            };
            if dialog.selected.is_empty() {
                return update(app, Message::ConfirmRemove);
            }
            dialog.step = RemoveStep::Confirm;
            Task::none()
        }
        Message::RemoveBack => {
            if let Some(dialog) = app.confirm_remove.as_mut() {
                dialog.step = RemoveStep::Choose;
            }
            Task::none()
        }
        Message::CancelRemove => {
            app.confirm_remove = None;
            Task::none()
        }
        Message::ShowEnvelope => {
            app.envelope_preview = Some(app.current_envelope());
            Task::none()
        }
        Message::CloseEnvelope => {
            app.envelope_preview = None;
            Task::none()
        }
        Message::ConfirmRemove => {
            let Some(dialog) = app.confirm_remove.take() else {
                return Task::none();
            };
            // Marcar algo exige ter passado pelo passo 2 (vermelho, com as contagens): sem
            // isso, uma mensagem fora de ordem apagaria sem a confirmacao explicita.
            if !dialog.selected.is_empty() && dialog.step != RemoveStep::Confirm {
                app.confirm_remove = Some(RemoveDialog {
                    step: RemoveStep::Confirm,
                    ..dialog
                });
                return Task::none();
            }
            let Some(device) = app.device(&dialog.device_id) else {
                return Task::none();
            };
            let (orphan, data, device_name) =
                (device.revoked, device.data.clone(), device.name.clone());
            if orphan && dialog.selected.is_empty() {
                // "Manter tudo" de aparelho ja removido: nada destrutivo, entra na hora.
                let _ = app
                    .handle
                    .commands
                    .send(Command::KeepDeviceData(dialog.device_id));
                return Task::none();
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
            let toast = remove_view::toast_text(&base, &data, dialog.selected);
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
            Task::none()
        }
        Message::UndoRemove => {
            // Nada foi enviado ao nucleo: desfazer e so esquecer. Vale para a remocao e para
            // o apagar, que saem juntos no mesmo comando, e para a conversa.
            app.pending_removal = None;
            Task::none()
        }
        Message::DecideOrphan => {
            let first = app.orphans().first().map(|d| d.device_id.clone());
            if let Some(device_id) = first {
                return update(app, Message::AskRemove(device_id));
            }
            Task::none()
        }
        Message::PostponeOrphans => {
            app.orphans_postponed = true;
            Task::none()
        }
        Message::StartConversation => {
            let settings = &app.snapshot.assistant.settings;
            let Some(destination) = settings
                .destination()
                .filter(|_| settings.model().is_some())
                // Nuvem sem chave so falharia na primeira resposta; a configuracao explica.
                .filter(|d| !d.is_cloud() || app.snapshot.assistant.has_key)
            else {
                app.open_setup();
                return Task::none();
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
            Task::none()
        }
        Message::ChatInput(text) => {
            app.chat_input = text;
            Task::none()
        }
        Message::ChatSubmit => {
            let text = app.chat_input.trim().to_string();
            // Sem conversa visivel (nenhuma, de outra conta ou escondida pelo apagar) a
            // pergunta iria para uma conversa que a pessoa nao esta vendo.
            if text.is_empty()
                || super::analysis_view::open_conversation(app).is_none()
                || app.snapshot.assistant.busy
                || awaiting_cloud_consent(app).is_some()
            {
                return Task::none();
            }
            app.chat_input.clear();
            send_assistant(app, AssistantCommand::Send(text));
            Task::none()
        }
        Message::ChatStop => {
            send_assistant(app, AssistantCommand::Cancel);
            Task::none()
        }
        Message::ChatRetry => {
            if !app.snapshot.assistant.busy
                && awaiting_cloud_consent(app).is_none()
                && super::analysis_view::open_conversation(app).is_some()
            {
                send_assistant(app, AssistantCommand::Resend);
            }
            Task::none()
        }
        Message::OpenConversation(id) => {
            app.analysis_tab = AnalysisTab::Chat;
            send_assistant(app, AssistantCommand::Open(id));
            Task::none()
        }
        Message::CloseConversation => {
            send_assistant(app, AssistantCommand::Close);
            Task::none()
        }
        Message::DeleteConversation(id) => {
            send_assistant(app, AssistantCommand::Delete(id));
            Task::none()
        }
        Message::AskDeleteConversation(id) => {
            if app
                .pending_removal
                .as_ref()
                .and_then(PendingRemoval::conversation_id)
                == Some(id.as_str())
            {
                return Task::none();
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
            Task::none()
        }
        Message::DeleteAllConversations => {
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
            Task::none()
        }
        Message::CloseAssistantSetup => {
            app.close_setup();
            Task::none()
        }
        Message::SetupChoice(choice) => {
            if let Some(setup) = app.assistant_setup.as_mut()
                && setup.saving.is_none()
            {
                setup.draft.choice = choice;
            }
            Task::none()
        }
        Message::SetupNext => {
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
            Task::none()
        }
        Message::SetupBack => {
            if let Some(setup) = app.assistant_setup.as_mut()
                && setup.saving.is_none()
            {
                setup.step = SetupStep::Where;
            }
            Task::none()
        }
        Message::SetupPort(text) => {
            if let Some(setup) = app.assistant_setup.as_mut() {
                setup.draft.port_input = text
                    .chars()
                    .filter(char::is_ascii_digit)
                    .take(PORT_DIGITS)
                    .collect();
            }
            Task::none()
        }
        Message::SetupTest => {
            // A lista vem da porta que esta no campo, mesmo sem gravar: e o teste dela.
            let port = app
                .assistant_setup
                .as_ref()
                .and_then(|s| s.draft.port())
                .filter(|_| !app.snapshot.assistant.listing);
            if let Some(port) = port {
                send_assistant(app, AssistantCommand::ListModels(port));
            }
            Task::none()
        }
        Message::SetupLocalModel(model) => {
            if let Some(setup) = app.assistant_setup.as_mut() {
                setup.draft.local_model = Some(model);
            }
            Task::none()
        }
        Message::SetupCloudModel(text) => {
            if let Some(setup) = app.assistant_setup.as_mut() {
                setup.draft.cloud_model = text;
            }
            Task::none()
        }
        Message::SetupDeny(deny) => {
            if let Some(setup) = app.assistant_setup.as_mut() {
                setup.draft.deny = deny;
            }
            Task::none()
        }
        Message::SetupFinish => {
            let Some(setup) = app.assistant_setup.as_ref() else {
                return Task::none();
            };
            if !super::assistant_settings_view::can_finish(app, setup) {
                return Task::none();
            }
            let saved = &app.snapshot.assistant.settings;
            let Some(settings) =
                super::assistant_settings_view::settings_to_save(saved, &setup.draft)
            else {
                return Task::none();
            };
            // `take`: o campo esvazia no mesmo instante. A chave so vai com a nuvem: escolher o
            // Ollama com uma chave digitada no passo da nuvem nao a grava.
            let key = std::mem::take(&mut app.key_input);
            let key = (setup.draft.choice == ProviderChoice::Cloud && !key.trim().is_empty())
                .then(|| Secret::new(key));
            if key.is_none() && settings == *saved {
                // Nada a gravar: o gravado ja e isto.
                app.close_setup();
                return Task::none();
            }
            let expected = app.snapshot.assistant.configured.wrapping_add(1);
            if let Some(setup) = app.assistant_setup.as_mut() {
                setup.saving = Some(expected);
            }
            send_assistant(app, AssistantCommand::Configure { settings, key });
            Task::none()
        }
        Message::KeyInput(key) => {
            app.key_input = key.expose().to_string();
            Task::none()
        }
        Message::RemoveKey => {
            app.key_input.clear();
            send_assistant(app, AssistantCommand::SaveKey(Secret::new(String::new())));
            Task::none()
        }
        Message::ShowCloudPreview => {
            if let Some(open) = awaiting_cloud_consent(app) {
                app.consent_preview = Some(open.id.clone());
            }
            Task::none()
        }
        Message::CancelPreview => {
            // So o "Cancelar": a abertura recem-criada, sem nada saido, some; o resto so fecha
            // a previa (`cancel_preview_message`).
            let busy = app.snapshot.assistant.busy;
            let cancel = super::analysis_view::shown_consent(app)
                .and_then(|open| super::analysis_view::cancel_preview_message(open, busy));
            app.consent_preview = None;
            match cancel {
                Some(message) => update(app, message),
                None => Task::none(),
            }
        }
        Message::ClosePreview => {
            // Esc e o scrim: um gesto rapido nao apaga nada. A conversa continua esperando,
            // com "Ver previa e enviar" no pe.
            app.consent_preview = None;
            Task::none()
        }
        Message::ApproveCloudSend => {
            // O id vai junto: o nucleo so libera se for a conversa ainda aberta. E so a conversa
            // que a previa mostra: um clique que chega depois de abrir outra nao aprova esta.
            if let Some(open) = super::analysis_view::shown_consent(app) {
                let id = open.id.clone();
                app.consent_preview = None;
                send_assistant(app, AssistantCommand::ApproveCloudSend(id));
            }
            Task::none()
        }
        Message::KeyPressed(press) => match super::keys::shortcut(app, &press) {
            Some(message) => update(app, message),
            None => Task::none(),
        },
        Message::FocusNext => super::keys::focus(true),
        Message::FocusPrevious => super::keys::focus(false),
        Message::Noop => Task::none(),
    }
}

/// Quanto tempo "Apagar todas" fica armado depois do primeiro clique.
pub(super) const DELETE_ALL_ARMED_FOR: Duration = Duration::from_secs(3);

/// A porta mais alta (65535) tem 5 digitos.
const PORT_DIGITS: usize = 5;

/// A conversa aberta que ainda espera a previa da nuvem ser aprovada: o destino e a nuvem e
/// nada saiu dela. Enquanto isso a tela mostra a previa no lugar do campo de pergunta.
///
/// Com resposta chegando (`busy`) nao espera nada: e o modelo local respondendo antes da troca
/// para a nuvem, e a previa (com o "Cancelar" que apaga) nao pode cair por cima disso.
pub(super) fn awaiting_cloud_consent(app: &App) -> Option<&OpenConversation> {
    super::analysis_view::open_conversation(app).filter(|o| {
        o.needs_cloud_consent
            && !app.snapshot.assistant.busy
            && app
                .snapshot
                .assistant
                .settings
                .destination()
                .is_some_and(|d| d.is_cloud())
    })
}

/// Pedido ao assistente no nucleo. Falha de envio (nucleo ja desligado) e ignorada como nos
/// outros comandos: a janela fecha junto.
fn send_assistant(app: &App, command: AssistantCommand) {
    let _ = app.handle.commands.send(Command::Assistant(command));
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
///
/// Cada gravacao leva um numero tirado na thread da UI, na ordem dos cliques, e so escreve se
/// for mais nova que a ultima escrita naquela pasta: duas threads nao tem ordem garantida, e
/// sem isso trocar o tema logo depois de "Iniciar com o Windows" (ou o tema duas vezes) podia
/// terminar com o arquivo da escolha anterior.
fn save_prefs(prefs: UiPrefs, dir: PathBuf) -> futures::channel::oneshot::Receiver<()> {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Mutex, OnceLock};

    static NEXT: AtomicU64 = AtomicU64::new(1);
    static WRITTEN: OnceLock<Mutex<HashMap<PathBuf, u64>>> = OnceLock::new();

    let generation = NEXT.fetch_add(1, Ordering::Relaxed);
    let (tx, rx) = futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        let written = WRITTEN.get_or_init(Default::default);
        // Envenenado = outra gravacao entrou em panico; a tabela continua coerente.
        let mut written = written.lock().unwrap_or_else(|e| e.into_inner());
        let last = written.entry(dir.clone()).or_insert(0);
        if generation > *last {
            if let Err(e) = prefs.save(&dir) {
                tracing::warn!("nao foi possivel gravar ui.json em {}: {e}", dir.display());
            }
            *last = generation;
        }
        drop(written);
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

/// Nucleo no ar (com ou sem rede local): o unico estado em que da para gerar codigo.
fn is_up(snap: &Snapshot) -> bool {
    matches!(snap.status, Status::Running | Status::NoNetwork)
}

pub fn subscription(_app: &App) -> Subscription<Message> {
    Subscription::batch([
        window::close_requests().map(|_| Message::CloseRequested),
        window::close_events().map(Message::WindowClosed),
        window::resize_events().map(|(_, size)| Message::Resized(size)),
        Subscription::run(ticks).map(|()| Message::Tick),
        super::keys::subscription(),
        iced::system::theme_changes().map(Message::SystemTheme),
    ])
}

pub fn view(app: &App, window: window::Id) -> Element<'_, Message> {
    super::view::view(app, window)
}

#[cfg(test)]
pub(super) mod tests {
    use tokio::sync::mpsc;

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
        app.window = Some(window::Id::unique());
        (app, fake, rx)
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
                ..UiPrefs::default()
            },
            dir.path().to_path_buf(),
            Rc::new(Headless),
        );
        assert_eq!(app.screen, Screen::Dashboard);
        assert_eq!(app.theme, ThemeMode::Light);
        assert!(std::ptr::eq(app.tokens, &super::super::theme::LIGHT));
    }

    #[test]
    fn nasce_com_o_tema_do_sistema_ou_o_da_ultima_gravacao() {
        let dir = tempfile::tempdir().unwrap();
        let boot = |system: Option<bool>, saved_dark: bool| {
            let fake = Fake::with_tray();
            fake.system_dark.set(system);
            let (cmd_tx, _rx) = mpsc::unbounded_channel();
            let (_tx, snap_rx) = watch::channel(Snapshot::default());
            App::new(
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
            )
        };
        let light = &super::super::theme::LIGHT;
        let dark = &super::super::theme::DARK;
        // O sistema manda sobre o que foi gravado.
        assert!(std::ptr::eq(boot(Some(false), true).tokens, light));
        assert!(std::ptr::eq(boot(Some(true), false).tokens, dark));
        // Sem resposta, vale o modo resolvido da ultima gravacao.
        assert!(std::ptr::eq(boot(None, true).tokens, dark));
        assert!(std::ptr::eq(boot(None, false).tokens, light));
    }

    /// Espera a thread de `save_prefs` gravar o que o teste quer ver.
    fn wait_prefs(dir: &std::path::Path, done: impl Fn(&UiPrefs) -> bool) {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !done(&UiPrefs::load(dir)) {
            assert!(std::time::Instant::now() < deadline, "ui.json nao gravado");
            std::thread::sleep(Duration::from_millis(10));
        }
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

        let _ = update(&mut app, Message::ToggleHideValues);
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
        let _ = update(&mut app, Message::SetTheme(ThemeMode::Light));
        wait_prefs(dir.path(), |p| p.theme == ThemeMode::Light);
        assert!(UiPrefs::load(dir.path()).hide_values);

        let _ = update(&mut app, Message::ToggleHideValues);
        assert!(!app.dashboard.hidden);
        assert_eq!(app.transactions, shown_transactions);
        wait_prefs(dir.path(), |p| !p.hide_values);
        assert_eq!(UiPrefs::load(dir.path()).theme, ThemeMode::Light);
    }

    #[test]
    fn alternar_ocultar_refaz_reservas_e_nao_mexe_no_que_vai_ao_assistente() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with_reserves(dir.path());
        let _ = update(&mut app, Message::SelectScreen(Screen::Transactions));
        let shown_reserves = app.reserves.clone();
        let envelope = app.current_envelope();
        let findings = format!("{:?}", app.analysis);
        assert!(leaks(&format!("{shown_reserves:#?}")).is_some());

        // Alternar em Lancamentos refaz Reservas tambem: trocar de tela depois chega mascarado.
        let _ = update(&mut app, Message::ToggleHideValues);
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

        let _ = update(&mut app, Message::ToggleHideValues);
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
    fn trocar_de_tela() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        // Toda tela da navegacao, Ajustes inclusive; cada troca volta a rolagem ao topo (a
        // tarefa do `snap_to`), porque o cabecalho fixo nao rola e so o conteudo tem posicao.
        for screen in super::super::shell::nav_entries(&app).map(|e| e.screen) {
            let task = update(&mut app, Message::SelectScreen(screen));
            assert_eq!(app.screen, screen);
            assert_eq!(task.units(), 1, "{screen:?}");
        }
        assert_eq!(app.screen, Screen::Settings);
    }

    #[test]
    fn configurar_o_assistente_abre_o_dialogo_sem_trocar_de_tela() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        let _ = update(&mut app, Message::SelectScreen(Screen::Settings));
        assert_eq!(app.screen, Screen::Settings);
        let _ = update(&mut app, Message::ConfigureAssistant);
        assert_eq!(app.screen, Screen::Settings);
        assert!(app.assistant_setup.is_some());
        assert!(super::super::assistant_settings_view::dialog(&app).is_some());
        let _ = super::super::shell::root(&app);
        // Ja aberto: nao refaz os rascunhos que a pessoa digitou.
        let _ = update(&mut app, Message::SetupPort("1234".into()));
        let _ = update(&mut app, Message::ConfigureAssistant);
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
    fn tema_sistema_segue_o_sistema_sem_gravar_a_troca() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, fake, _rx) = with_tray(dir.path());
        fake.system_dark.set(Some(false));
        let _ = update(&mut app, Message::SetTheme(ThemeMode::System));
        assert_eq!(app.theme, ThemeMode::System);
        // Escolher "Sistema" le o sistema na hora: claro, sem esperar o tique.
        assert!(std::ptr::eq(app.tokens, &super::super::theme::LIGHT));
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while UiPrefs::load(dir.path()).theme != ThemeMode::System {
            assert!(std::time::Instant::now() < deadline, "ui.json nao gravado");
            std::thread::sleep(Duration::from_millis(10));
        }

        // O sistema vai para o escuro: o tique percebe (caminho do Windows).
        fake.system_dark.set(Some(true));
        let _ = update(&mut app, Message::Tick);
        assert!(std::ptr::eq(app.tokens, &super::super::theme::DARK));
        // Com o registro respondendo, o aviso do iced e ignorado: ele chega velho no Windows.
        let _ = update(&mut app, Message::SystemTheme(iced::theme::Mode::Light));
        assert!(std::ptr::eq(app.tokens, &super::super::theme::DARK));
        // Sem registro (Linux), o aviso do iced troca; `None` nao mexe.
        fake.system_dark.set(None);
        let _ = update(&mut app, Message::SystemTheme(iced::theme::Mode::Light));
        assert!(std::ptr::eq(app.tokens, &super::super::theme::LIGHT));
        let _ = update(&mut app, Message::SystemTheme(iced::theme::Mode::None));
        let _ = update(&mut app, Message::Tick);
        assert!(std::ptr::eq(app.tokens, &super::super::theme::LIGHT));
        assert_eq!(UiPrefs::load(dir.path()).theme, ThemeMode::System);
    }

    #[test]
    fn tema_fixo_ignora_o_sistema() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, fake, _rx) = with_tray(dir.path());
        fake.system_dark.set(Some(false));
        assert_eq!(app.theme, ThemeMode::Dark);
        let _ = update(&mut app, Message::Tick);
        let _ = update(&mut app, Message::SystemTheme(iced::theme::Mode::Light));
        assert!(std::ptr::eq(app.tokens, &super::super::theme::DARK));
        // Voltar a "Sistema" usa o que o sistema disse por ultimo.
        let _ = update(&mut app, Message::SetTheme(ThemeMode::System));
        assert!(std::ptr::eq(app.tokens, &super::super::theme::LIGHT));
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
        assert!(!app.is_copied(CopyTarget::Guide));
        // Timer chegou antes do prazo (outro clique adiou): continua.
        let _ = update(&mut app, Message::CopyExpired);
        assert!(app.copied.is_some());
        app.copied = Some((CopyTarget::Address, Instant::now()));
        let _ = update(&mut app, Message::CopyExpired);
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
        let _ = update(&mut app, Message::Copy(CopyTarget::DataDir));
        assert!(app.is_copied(CopyTarget::DataDir));
        assert!(!app.is_copied(CopyTarget::Address));
        let _ = update(&mut app, Message::Copy(CopyTarget::LogsDir));
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
    fn fallback_das_releases_desenha_e_copia_a_url() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        let _ = super::super::connection_view::view(&app);
        app.releases_open_failed = true;
        let _ = super::super::connection_view::view(&app);
        let _ = update(&mut app, Message::Copy(CopyTarget::ReleasesUrl));
        assert!(app.is_copied(CopyTarget::ReleasesUrl));
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
            ..DeviceView::default()
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
            ..DeviceView::default()
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
            if let Command::RemoveDevice { device_id, .. } = cmd {
                out.push(device_id);
            }
        }
        out
    }

    // --- Conectar um celular ---

    fn phone(id: &str, name: &str) -> DeviceView {
        DeviceView {
            device_id: id.into(),
            name: name.into(),
            paired_at: "2026-10-03T10:00:00Z".into(),
            ..DeviceView::default()
        }
    }

    fn running_with(devices: Vec<DeviceView>) -> Snapshot {
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
        let _ = update(&mut app, Message::ConnectPhone);
        assert_eq!(app.screen, Screen::Connection);
        assert_eq!(app.connect, Some(ConnectFlow::new(true)));
        assert!(connection_view::dialog_open(&app));
        assert!(!connection_view::shows_in_screen_flow(&app));
    }

    #[test]
    fn conectar_sem_aparelho_abre_o_fluxo_em_tela() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(running_with(vec![]), dir.path().to_path_buf());
        let _ = update(&mut app, Message::ConnectPhone);
        assert_eq!(app.screen, Screen::Connection);
        assert_eq!(app.connect, Some(ConnectFlow::new(false)));
        assert!(connection_view::shows_in_screen_flow(&app));
        assert!(!connection_view::dialog_open(&app));
        // O botao dos vazios leva ao mesmo lugar.
        app.screen = Screen::Dashboard;
        app.connect = None;
        let _ = update(&mut app, Message::ConnectPhone);
        assert!(connection_view::shows_in_screen_flow(&app));
    }

    #[test]
    fn continuar_e_voltar_andam_e_o_codigo_e_pedido_ao_entrar_no_passo_3() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with(
            running_with(vec![phone("D1", "Pixel 7")]),
            dir.path().to_path_buf(),
        );
        let _ = update(&mut app, Message::ConnectPhone);
        let _ = update(&mut app, Message::ConnectBack);
        assert_eq!(app.connect.map(|f| f.step), Some(ConnectStep::WiFi));
        let _ = update(&mut app, Message::ConnectNext);
        assert_eq!(app.connect.map(|f| f.step), Some(ConnectStep::Certificate));
        assert!(drain(&mut rx).is_empty(), "nada sai antes do passo 3");
        let _ = update(&mut app, Message::ConnectNext);
        assert_eq!(app.connect.map(|f| f.step), Some(ConnectStep::Pair));
        assert_eq!(drain(&mut rx), vec![Command::IssuePairingToken]);
        // Continuar no ultimo passo nao faz nada.
        let _ = update(&mut app, Message::ConnectNext);
        assert!(drain(&mut rx).is_empty());
        let _ = update(&mut app, Message::ConnectBack);
        assert_eq!(app.connect.map(|f| f.step), Some(ConnectStep::Certificate));
        // Voltar do passo 3 cancela o codigo; voltar do 2 nao manda nada.
        assert_eq!(drain(&mut rx), vec![Command::CancelPairingToken]);
        let _ = update(&mut app, Message::ConnectBack);
        assert!(drain(&mut rx).is_empty());
    }

    #[test]
    fn hub_fora_do_ar_nao_pede_codigo_ao_entrar_no_passo_3() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        app.screen = Screen::Connection;
        app.connect = Some(ConnectFlow::new(true));
        let _ = update(&mut app, Message::ConnectNext);
        let _ = update(&mut app, Message::ConnectNext);
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
        let _ = update(&mut app, Message::ConnectPhone);
        let _ = update(&mut app, Message::ConnectNext);
        let _ = update(&mut app, Message::ConnectOs(PhoneOs::Iphone));
        assert_eq!(app.connect.map(|f| f.os), Some(PhoneOs::Iphone));
        assert_eq!(app.connect.map(|f| f.step), Some(ConnectStep::Certificate));
        let _ = update(&mut app, Message::ConnectOs(PhoneOs::Android));
        assert_eq!(app.connect.map(|f| f.os), Some(PhoneOs::Android));
    }

    #[test]
    fn fechar_ou_sair_da_tela_cancela_o_codigo() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with(
            running_with(vec![phone("D1", "Pixel 7")]),
            dir.path().to_path_buf(),
        );
        let _ = update(&mut app, Message::ConnectPhone);
        let _ = update(&mut app, Message::ConnectClose);
        assert_eq!(app.connect, None);
        assert_eq!(drain(&mut rx), vec![Command::CancelPairingToken]);

        let _ = update(&mut app, Message::ConnectPhone);
        let _ = update(&mut app, Message::SelectScreen(Screen::Dashboard));
        assert_eq!(app.connect, None);
        assert_eq!(drain(&mut rx), vec![Command::CancelPairingToken]);
        // Sem fluxo aberto, trocar de tela nao manda nada.
        let _ = update(&mut app, Message::SelectScreen(Screen::Connection));
        let _ = update(&mut app, Message::SelectScreen(Screen::Dashboard));
        assert!(drain(&mut rx).is_empty());
    }

    #[test]
    fn aparelho_novo_fecha_o_dialogo_e_mostra_o_aviso() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with(
            running_with(vec![phone("D1", "Pixel 7")]),
            dir.path().to_path_buf(),
        );
        let _ = update(&mut app, Message::ConnectPhone);
        let _ = update(&mut app, Message::ConnectNext);
        let _ = update(&mut app, Message::ConnectNext);
        let _ = drain(&mut rx);
        let snap = running_with(vec![phone("D1", "Pixel 7"), phone("D2", "Pixel da Ana")]);
        let _ = update(&mut app, Message::Snapshot(Box::new(snap)));
        assert_eq!(app.connect, None);
        assert_eq!(
            app.notice.as_ref().map(|(text, _)| text.as_str()),
            Some("Pixel da Ana conectado")
        );
        // Cancela mesmo assim: um "Gerar outro codigo" pode ter saido antes do snapshot.
        assert_eq!(drain(&mut rx), vec![Command::CancelPairingToken]);
        // O aviso some no tique depois do prazo.
        let _ = update(&mut app, Message::Tick);
        assert!(app.notice.is_some());
        if let Some((_, until)) = app.notice.as_mut() {
            *until = Instant::now();
        }
        let _ = update(&mut app, Message::Tick);
        assert_eq!(app.notice, None);
    }

    #[test]
    fn parear_de_novo_o_mesmo_aparelho_tambem_fecha_o_dialogo() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(
            running_with(vec![phone("D1", "Pixel 7")]),
            dir.path().to_path_buf(),
        );
        let _ = update(&mut app, Message::ConnectPhone);
        // Mesmo id, ainda ativo, `paired_at` novo (o UPSERT do pareamento).
        let mut again = phone("D1", "Pixel 7");
        again.paired_at = "2026-11-26T10:00:00Z".into();
        let _ = update(
            &mut app,
            Message::Snapshot(Box::new(running_with(vec![again]))),
        );
        assert_eq!(app.connect, None);
        assert_eq!(
            app.notice.as_ref().map(|(text, _)| text.as_str()),
            Some("Pixel 7 conectado")
        );
        // Um snapshot qualquer (mesmo `paired_at`) nao e pareamento.
        app.notice = None;
        let _ = update(&mut app, Message::ConnectPhone);
        let snap = app.snapshot.clone();
        let _ = update(&mut app, Message::Snapshot(Box::new(snap)));
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
        let _ = update(&mut app, Message::AskRemove("D1".into()));
        let _ = update(&mut app, Message::ConfirmRemove);
        // Durante o "Desfazer" o aparelho ainda conta: a lista fica, sem o fluxo em tela.
        assert!(!connection_view::in_screen_case(&app));
        assert!(!connection_view::shows_in_screen_flow(&app));
        // Fluxo em tela no passo 3 (como antes desta correcao) que o "Desfazer" esconderia.
        app.connect = Some(ConnectFlow {
            step: ConnectStep::Pair,
            ..ConnectFlow::new(false)
        });
        let _ = drain(&mut rx);
        let _ = update(&mut app, Message::UndoRemove);
        assert_eq!(app.connect, None);
        assert_eq!(drain(&mut rx), vec![Command::CancelPairingToken]);
        // Depois que a remocao sai e o snapshot a reflete, o fluxo ocupa a tela.
        let mut gone = phone("D1", "Pixel 7");
        gone.revoked = true;
        let _ = update(
            &mut app,
            Message::Snapshot(Box::new(running_with(vec![gone]))),
        );
        assert!(connection_view::shows_in_screen_flow(&app));
    }

    #[test]
    fn primeiro_aparelho_encerra_o_fluxo_em_tela() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(running_with(vec![]), dir.path().to_path_buf());
        app.screen = Screen::Connection;
        // Fluxo em tela sem nenhum clique ainda (`connect` vazio): mesmo assim fecha e avisa.
        let snap = running_with(vec![phone("D1", "Pixel 8")]);
        let _ = update(&mut app, Message::Snapshot(Box::new(snap)));
        assert_eq!(
            app.notice.as_ref().map(|(text, _)| text.as_str()),
            Some("Pixel 8 conectado")
        );
        assert!(!connection_view::shows_in_screen_flow(&app));
    }

    #[test]
    fn aparelhos_do_boot_ou_sem_fluxo_aberto_nao_viram_aviso() {
        let dir = tempfile::tempdir().unwrap();
        // Boot: o snapshot inicial (`Starting`) vem sem aparelhos e o primeiro `Running` os
        // traz todos.
        let (mut app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        app.screen = Screen::Connection;
        let snap = running_with(vec![phone("D1", "Pixel 7")]);
        let _ = update(&mut app, Message::Snapshot(Box::new(snap)));
        assert_eq!(app.notice, None);
        // Fora de Aparelhos, sem fluxo: um aparelho novo nao gera aviso.
        app.screen = Screen::Dashboard;
        let snap = running_with(vec![phone("D1", "Pixel 7"), phone("D2", "iPhone")]);
        let _ = update(&mut app, Message::Snapshot(Box::new(snap)));
        assert_eq!(app.notice, None);
    }

    #[test]
    fn remover_pede_confirmacao_e_cancelar_nao_faz_nada() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with_two_people(dir.path());
        let _ = update(&mut app, Message::AskRemove("D1".into()));
        assert_eq!(
            app.confirm_remove.as_ref().map(|d| d.device_id.as_str()),
            Some("D1")
        );
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
        assert_eq!(pending.device_id(), Some("D1"));
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
        app.screen = Screen::Connection;
        let _ = update(&mut app, Message::AskRemove("D1".into()));
        let _ = update(&mut app, Message::ConfirmRemove);
        let _ = update(&mut app, Message::AskRemove("D2".into()));
        let _ = update(&mut app, Message::ConfirmRemove);
        assert_eq!(revoked(&mut rx), vec!["D1".to_string()]);
        assert_eq!(
            app.pending_removal.as_ref().unwrap().device_id(),
            Some("D2")
        );
        let _ = update(&mut app, Message::SelectScreen(Screen::Dashboard));
        assert_eq!(revoked(&mut rx), vec!["D2".to_string()]);
    }

    #[test]
    fn clicar_na_tela_atual_so_volta_ao_topo() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with_two_people(dir.path());
        app.screen = Screen::Connection;
        let _ = update(&mut app, Message::AskRemove("D1".into()));
        let _ = update(&mut app, Message::ConfirmRemove);
        app.key_input = "sk-rascunho".into();
        let task = update(&mut app, Message::SelectScreen(Screen::Connection));
        // Volta ao topo, mas a remocao continua pendente (com "Desfazer") e nada saiu.
        assert_eq!(task.units(), 1);
        assert!(revoked(&mut rx).is_empty());
        assert_eq!(
            app.pending_removal
                .as_ref()
                .and_then(PendingRemoval::device_id),
            Some("D1")
        );
        assert_eq!(app.key_input, "sk-rascunho");
        let _ = update(&mut app, Message::UndoRemove);
        assert_eq!(app.pending_removal, None);
    }

    #[test]
    fn fechar_com_remocao_pendente_revoga_antes_do_shutdown() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with_two_people(dir.path());
        let _ = update(&mut app, Message::AskRemove("D1".into()));
        let _ = update(&mut app, Message::ConfirmRemove);
        let _ = update(&mut app, Message::CloseRequested);
        assert!(matches!(
            rx.try_recv(),
            Ok(Command::RemoveDevice { device_id, .. }) if device_id == "D1"
        ));
        assert!(matches!(rx.try_recv(), Ok(Command::Shutdown)));
    }

    // --- modal por area, desfazer e aviso de aparelho removido ---

    fn data() -> DeviceData {
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

    fn view(id: &str, name: &str, revoked: bool, kept: bool, data: DeviceData) -> DeviceView {
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

    fn app_with_data(
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

    fn drain(rx: &mut mpsc::UnboundedReceiver<Command>) -> Vec<Command> {
        let mut out = Vec::new();
        while let Ok(cmd) = rx.try_recv() {
            out.push(cmd);
        }
        out
    }

    fn only(area: Area) -> AreaSet {
        [area].into_iter().collect()
    }

    #[test]
    fn nada_marcado_manda_remove_device_vazio() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with_data(
            dir.path(),
            vec![view("D1", "Pixel 7", false, false, data())],
        );
        let _ = update(&mut app, Message::AskRemove("D1".into()));
        let _ = update(&mut app, Message::RemoveContinue);
        // Manter tudo e o padrao: nao ha passo 2.
        assert_eq!(app.confirm_remove, None);
        app.pending_removal.as_mut().unwrap().deadline = Instant::now();
        let _ = update(&mut app, Message::Tick);
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
        let _ = update(&mut app, Message::AskRemove("D1".into()));
        let _ = update(&mut app, Message::ToggleArea(Area::Budgets));
        let dialog = app.confirm_remove.clone().unwrap();
        assert_eq!(dialog.selected, only(Area::Budgets));
        assert_eq!(dialog.step, RemoveStep::Choose);
        // Mensagem fora de ordem: confirmar no passo 1 com area marcada nao apaga nada.
        let _ = update(&mut app, Message::ConfirmRemove);
        assert_eq!(app.pending_removal, None);
        assert_eq!(
            app.confirm_remove.as_ref().unwrap().step,
            RemoveStep::Confirm
        );
        let _ = update(&mut app, Message::RemoveBack);
        assert_eq!(
            app.confirm_remove.as_ref().unwrap().step,
            RemoveStep::Choose
        );
        let _ = update(&mut app, Message::RemoveContinue);
        assert_eq!(
            app.confirm_remove.as_ref().unwrap().step,
            RemoveStep::Confirm
        );
        let _ = update(&mut app, Message::RemoveBack);
        let _ = update(&mut app, Message::CancelRemove);
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
        let _ = update(&mut app, Message::AskRemove("D1".into()));
        let _ = update(&mut app, Message::ToggleArea(Area::Transactions));
        assert!(app.confirm_remove.as_ref().unwrap().selected.is_empty());
        let _ = update(&mut app, Message::ToggleArea(Area::Budgets));
        assert_eq!(
            app.confirm_remove.as_ref().unwrap().selected,
            only(Area::Budgets)
        );
        // "Apagar tudo" marca so as que tem dados.
        let _ = update(&mut app, Message::SelectNoAreas);
        let _ = update(&mut app, Message::SelectAllAreas);
        assert_eq!(
            app.confirm_remove.as_ref().unwrap().selected,
            only(Area::Budgets)
        );
    }

    #[test]
    fn apagar_tudo_marca_as_quatro_e_a_confirmacao_mostra_as_contagens() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with_data(
            dir.path(),
            vec![view("D1", "Pixel 7", false, false, data())],
        );
        let _ = update(&mut app, Message::AskRemove("D1".into()));
        let _ = update(&mut app, Message::SelectAllAreas);
        assert_eq!(
            app.confirm_remove.as_ref().unwrap().selected,
            AreaSet::all()
        );
        let _ = update(&mut app, Message::RemoveContinue);
        // O passo 2 desenha sem panico e os textos trazem as contagens.
        let _ = remove_view::dialog(&app).expect("modal aberto");
        let selected = app.confirm_remove.as_ref().unwrap().selected;
        let text = remove_view::confirm_text(&data(), selected);
        assert!(text.contains("3 metas"), "{text}");
        assert!(text.contains("100 lançamentos"), "{text}");
        assert!(text.contains("2 reservas com 14 movimentos"), "{text}");
        assert!(text.contains("1 recorrência com 2 reajustes"), "{text}");
        assert_eq!(
            remove_view::confirm_label(&data(), selected, false),
            "Remover e apagar 122 itens"
        );
        let _ = update(&mut app, Message::ConfirmRemove);
        assert!(
            app.pending_removal
                .as_ref()
                .unwrap()
                .toast
                .ends_with("122 itens serão apagados")
        );
        assert!(drain(&mut rx).is_empty(), "nada sai antes do prazo");
        app.pending_removal.as_mut().unwrap().deadline = Instant::now();
        let _ = update(&mut app, Message::Tick);
        assert_eq!(
            drain(&mut rx),
            vec![Command::RemoveDevice {
                device_id: "D1".into(),
                erase: AreaSet::all()
            }]
        );
    }

    #[test]
    fn desfazer_cancela_a_remocao_e_o_apagar() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with_data(
            dir.path(),
            vec![view("D1", "Pixel 7", false, false, data())],
        );
        let _ = update(&mut app, Message::AskRemove("D1".into()));
        let _ = update(&mut app, Message::SelectAllAreas);
        let _ = update(&mut app, Message::RemoveContinue);
        let _ = update(&mut app, Message::ConfirmRemove);
        assert_eq!(app.visible_people().count(), 0);
        let _ = update(&mut app, Message::UndoRemove);
        assert_eq!(app.pending_removal, None);
        assert_eq!(app.visible_people().count(), 1);
        let _ = update(&mut app, Message::Tick);
        let _ = update(&mut app, Message::SelectScreen(Screen::Dashboard));
        assert!(drain(&mut rx).is_empty());
    }

    #[test]
    fn aviso_so_para_removido_com_dados_sem_decisao() {
        let dir = tempfile::tempdir().unwrap();
        let devices = vec![
            view("ATIVO", "Ativo", false, false, data()),
            view("ORFAO", "Orfao", true, false, data()),
            view("MANTIDO", "Mantido", true, true, data()),
            view("VAZIO", "Vazio", true, false, DeviceData::default()),
        ];
        let (mut app, _rx, _tx) = app_with_data(dir.path(), devices);
        let ids: Vec<_> = app.orphans().iter().map(|d| d.device_id.clone()).collect();
        assert_eq!(ids, vec!["ORFAO"]);
        assert!(remove_view::orphan_banner(&app).is_some());
        // Banco sem revogados: nada.
        let (app2, _rx2, _tx2) = app_with_data(
            dir.path(),
            vec![view("ATIVO", "Ativo", false, false, data())],
        );
        assert!(app2.orphans().is_empty());
        assert!(remove_view::orphan_banner(&app2).is_none());
        // "Depois" esconde nesta sessao e nao grava nada.
        let _ = update(&mut app, Message::PostponeOrphans);
        assert!(remove_view::orphan_banner(&app).is_none());
        assert_eq!(app.orphans().len(), 1);
    }

    #[test]
    fn depois_nao_envia_comando_e_o_aviso_volta_na_reabertura() {
        let dir = tempfile::tempdir().unwrap();
        let orphan = view("ORFAO", "Orfao", true, false, data());
        let (mut app, mut rx, _tx) = app_with_data(dir.path(), vec![orphan.clone()]);
        let _ = update(&mut app, Message::PostponeOrphans);
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
            vec![view("ORFAO", "Orfao", true, false, data())],
        );
        let _ = update(&mut app, Message::DecideOrphan);
        assert_eq!(
            app.confirm_remove.as_ref().map(|d| d.device_id.as_str()),
            Some("ORFAO")
        );
        // Com o modal aberto o aviso some.
        assert!(remove_view::orphan_banner(&app).is_none());
        let _ = remove_view::dialog(&app).expect("modal aberto");
        let _ = update(&mut app, Message::RemoveContinue);
        assert_eq!(
            drain(&mut rx),
            vec![Command::KeepDeviceData("ORFAO".into())]
        );
        assert_eq!(app.confirm_remove, None);
        assert_eq!(app.pending_removal, None);
    }

    #[test]
    fn decidir_apagar_de_removido_tem_desfazer_e_nao_revoga() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with_data(
            dir.path(),
            vec![view("ORFAO", "Orfao", true, false, data())],
        );
        let _ = update(&mut app, Message::DecideOrphan);
        let _ = update(&mut app, Message::ToggleArea(Area::Reserves));
        let _ = update(&mut app, Message::RemoveContinue);
        assert_eq!(
            remove_view::confirm_label(&data(), only(Area::Reserves), true),
            "Apagar 16 itens"
        );
        let _ = update(&mut app, Message::ConfirmRemove);
        // O aparelho some do aviso na hora (apagar pendente), mas nada saiu.
        assert!(app.orphans().is_empty());
        assert!(drain(&mut rx).is_empty());
        let _ = update(&mut app, Message::UndoRemove);
        assert_eq!(app.orphans().len(), 1);
        let _ = update(&mut app, Message::DecideOrphan);
        let _ = update(&mut app, Message::ToggleArea(Area::Reserves));
        let _ = update(&mut app, Message::RemoveContinue);
        let _ = update(&mut app, Message::ConfirmRemove);
        app.pending_removal.as_mut().unwrap().deadline = Instant::now();
        let _ = update(&mut app, Message::Tick);
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
        let _ = update(&mut app, Message::NextMonth);
        assert_eq!(app.month_idx, 11, "nao passa do corrente");
        let _ = update(&mut app, Message::PrevMonth);
        assert_eq!(app.month(), "2026-08");
        assert_eq!(app.dashboard.month, "2026-08");
        assert!(!app.dashboard.is_current);
        assert_eq!(app.dashboard.header.partial, None);
        assert!(app.dashboard.whats_next.is_none());
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
        let _ = update(&mut app, Message::ExpandUpcoming);
        assert!(app.upcoming_expanded);
        // Um sync novo recalcula a tela mas nao fecha a lista de quem esta lendo.
        let snap = Snapshot {
            dashboard: std::sync::Arc::new(crate::dashboard::fixtures()),
            ..Snapshot::default()
        };
        let _ = update(&mut app, Message::Snapshot(Box::new(snap)));
        assert!(app.upcoming_expanded);
        let _ = update(&mut app, Message::PrevMonth);
        assert!(!app.upcoming_expanded);
    }

    /// Monta a arvore do Dashboard nos estados da spec 7 e nas larguras dos dois lados de 1100:
    /// pega indice fora da lista, divisao por zero em layout e secao esquecida sem abrir janela.
    fn view_all_widths(app: &mut App) {
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
        let _ = update(&mut app, Message::ExpandUpcoming);
        view_all_widths(&mut app);
        let _ = update(&mut app, Message::ToggleHideValues);
        assert!(app.dashboard.hidden);
        view_all_widths(&mut app);
        // Mes fechado: sem "O que vem".
        let _ = update(&mut app, Message::PrevMonth);
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
    fn menu_de_categoria_abre_fecha_e_escolhe() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with_fixtures(dir.path());
        let _ = update(&mut app, Message::ToggleCategoryMenu);
        assert!(app.cat_menu_open);
        let _ = update(&mut app, Message::ToggleCategoryMenu);
        assert!(!app.cat_menu_open);
        let _ = update(&mut app, Message::ToggleCategoryMenu);
        let _ = update(&mut app, Message::CloseCategoryMenu);
        assert!(!app.cat_menu_open, "clicar fora fecha");
        let _ = update(&mut app, Message::ToggleCategoryMenu);
        let _ = update(&mut app, Message::PickCategory(Some("C1".into())));
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
        let _ = update(&mut app, Message::PickCategory(Some(String::new())));
        assert_eq!(
            app.transactions.list.count, 2,
            "sem categoria: nula e apagada"
        );
        app.screen = Screen::Transactions;
        let _ = update(&mut app, Message::ToggleCategoryMenu);
        let _ = update(&mut app, Message::SelectScreen(Screen::Dashboard));
        assert!(!app.cat_menu_open, "trocar de tela fecha");
    }

    #[test]
    fn chips_de_autor_e_tipo() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with_fixtures(dir.path());
        // Setembro tem lancamento sem autor: ha tres chips (Ana, Luiz, Sem autor).
        assert!(app.transactions.unauthored_chip);
        let _ = update(&mut app, Message::ToggleAuthor("U1".into()));
        assert_eq!(app.transactions.list.count, 3);
        let _ = update(&mut app, Message::ToggleAuthor("U2".into()));
        assert_eq!(app.filters.authors.len(), 2, "falta o Sem autor");
        let _ = update(&mut app, Message::ToggleAuthor(String::new()));
        assert!(app.filters.authors.is_empty(), "todos marcados = Todos");
        assert_eq!(app.transactions.list.count, 7);
        let _ = update(&mut app, Message::ToggleAuthor("U2".into()));
        let _ = update(&mut app, Message::AllAuthors);
        assert!(app.filters.authors.is_empty());
        let _ = update(&mut app, Message::SetType(TypeFilter::Income));
        assert_eq!(app.transactions.list.count, 1);
        let _ = update(&mut app, Message::SetType(TypeFilter::All));
        assert!(!app.filters.is_active());
    }

    #[test]
    fn filtros_sobrevivem_a_troca_de_mes_e_limpar_zera() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with_fixtures(dir.path());
        let _ = update(&mut app, Message::PickCategory(Some("C2".into())));
        let _ = update(&mut app, Message::SetType(TypeFilter::Expense));
        let _ = update(&mut app, Message::PrevMonth);
        assert_eq!(app.filters.category.as_deref(), Some("C2"));
        assert_eq!(app.filters.kind, TypeFilter::Expense);
        assert_eq!(app.transactions.month, "2026-08");
        assert_eq!(app.transactions.list.count, 1, "aluguel de agosto");
        let _ = update(&mut app, Message::ToggleCategoryMenu);
        let _ = update(&mut app, Message::ClearFilters);
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
        let _ = update(&mut app, Message::SelectScreen(Screen::Reserves));
        assert_eq!(app.screen, Screen::Reserves);
        app.set_today("2026-09-24".into());
        let snap = Snapshot {
            dashboard: std::sync::Arc::new(crate::dashboard::fixtures_with_reserves()),
            ..Snapshot::default()
        };
        let _ = update(&mut app, Message::Snapshot(Box::new(snap)));
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
    fn com_bandeja_fechar_fecha_a_janela_e_nao_desliga() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _fake, mut rx) = with_tray(dir.path());
        let id = app.window.unwrap();
        let _ = update(&mut app, Message::CloseRequested);
        assert!(!app.closing);
        assert!(
            rx.try_recv().is_err(),
            "nenhum comando, muito menos Shutdown"
        );
        // A janela so some quando o iced confirma (`close_events`).
        assert_eq!(app.window, Some(id));
        let _ = update(&mut app, Message::WindowClosed(window::Id::unique()));
        assert_eq!(app.window, Some(id), "outra janela nao e a nossa");
        app.maximized = true;
        app.window_size = Size::new(800.0, 600.0);
        let _ = update(&mut app, Message::WindowClosed(id));
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
                devices: vec![view("D1", "Pixel 7", false, false, data())],
                ..Snapshot::default()
            },
            dir.path().to_path_buf(),
            fake,
        );
        app.window = Some(window::Id::unique());
        let _ = update(&mut app, Message::AskRemove("D1".into()));
        let _ = update(&mut app, Message::ConfirmRemove);
        assert!(app.pending_removal.is_some());
        let _ = update(&mut app, Message::CloseRequested);
        assert!(matches!(
            rx.try_recv(),
            Ok(Command::RemoveDevice { device_id, .. }) if device_id == "D1"
        ));
        assert!(rx.try_recv().is_err());
        assert_eq!(app.pending_removal, None);
        assert!(!app.closing);
    }

    #[test]
    fn com_bandeja_fechar_a_janela_fecha_o_conectar_um_celular() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _fake, mut rx) = with_tray(dir.path());
        app.screen = Screen::Connection;
        app.connect = Some(ConnectFlow::new(true));
        let _ = update(&mut app, Message::CloseRequested);
        assert_eq!(app.connect, None);
        assert_eq!(drain(&mut rx), vec![Command::CancelPairingToken]);
        assert!(!app.closing);
    }

    #[test]
    fn abrir_sem_janela_cria_uma_e_com_janela_mantem_a_mesma() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _fake, _rx) = with_tray(dir.path());
        app.window = None;
        let _ = update(&mut app, Message::Desktop(DesktopEvent::OpenWindow));
        let id = app.window.expect("janela aberta");
        let _ = update(&mut app, Message::Desktop(DesktopEvent::OpenWindow));
        assert_eq!(app.window, Some(id));
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
    fn sem_bandeja_fechar_continua_desligando() {
        // `fechar_manda_shutdown_uma_vez` e `fechar_com_remocao_pendente_revoga_antes_do_shutdown`
        // ja cobrem; este fixa que `Headless` e o que os mantem validos.
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        assert!(!app.desktop.has_tray());
        let _ = update(&mut app, Message::CloseRequested);
        assert!(app.closing);
        assert!(matches!(rx.try_recv(), Ok(Command::Shutdown)));
    }

    #[test]
    fn desligar_pela_bandeja_manda_shutdown_uma_vez() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _fake, mut rx) = with_tray(dir.path());
        let _ = update(&mut app, Message::Desktop(DesktopEvent::Quit));
        let _ = update(&mut app, Message::Desktop(DesktopEvent::Quit));
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
        let _ = update(&mut app, Message::Snapshot(Box::new(stopped)));
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
        let _ = update(&mut app, Message::Snapshot(Box::new(snap.clone())));
        assert_eq!(
            fake.tooltip.borrow().as_str(),
            "HubFinance — ligado · 1 aparelho"
        );
        *fake.tooltip.borrow_mut() = String::new();
        let _ = update(&mut app, Message::Snapshot(Box::new(snap)));
        assert_eq!(
            fake.tooltip.borrow().as_str(),
            "",
            "texto igual nao e reenviado"
        );
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
        let _ = update(&mut app, Message::Snapshot(Box::new(failed.clone())));
        assert!(
            app.window.is_some(),
            "falha silenciosa no login e o pior resultado"
        );
        // A pessoa viu o erro e fechou: outro snapshot ainda `Failed` nao reabre.
        app.window = None;
        let _ = update(&mut app, Message::Snapshot(Box::new(failed.clone())));
        assert!(app.window.is_none());
        // Desligando: nao reabre.
        let _ = update(
            &mut app,
            Message::Snapshot(Box::new(Snapshot {
                status: Status::Running,
                ..Snapshot::default()
            })),
        );
        app.closing = true;
        let _ = update(&mut app, Message::Snapshot(Box::new(failed)));
        assert!(app.window.is_none());
    }

    #[test]
    fn ligar_iniciar_com_o_windows_grava_no_sistema_e_no_ui_json() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, fake, _rx) = with_tray(dir.path());
        assert_eq!(app.autostart, Some(false));
        let _ = update(&mut app, Message::SetAutostart(true));
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
    fn falha_ao_gravar_mantem_o_estado_e_avisa() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, fake, _rx) = with_tray(dir.path());
        fake.fail_autostart.set(true);
        let _ = update(&mut app, Message::SetAutostart(true));
        assert_eq!(app.autostart, Some(false));
        assert_eq!(app.autostart_pref, None);
        assert!(app.autostart_failed);
        assert_eq!(UiPrefs::load(dir.path()).autostart, None);
    }

    #[test]
    fn trocar_o_tema_preserva_a_preferencia_de_autostart() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, fake, _rx) = with_tray(dir.path());
        fake.autostart.set(Some(true));
        app.autostart = Some(true);
        let _ = update(&mut app, Message::SetAutostart(false));
        let _ = update(&mut app, Message::SetTheme(ThemeMode::Light));
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
    fn fechar_manda_shutdown_uma_vez() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        let _ = update(&mut app, Message::CloseRequested);
        let _ = update(&mut app, Message::CloseRequested);
        assert!(matches!(rx.try_recv(), Ok(Command::Shutdown)));
        assert!(rx.try_recv().is_err());
    }

    // ---- seletor Pessoal | Comercial ----

    /// Fixtures (pessoais) mais a conta comercial com um lancamento de receita de R$ 900.
    fn business_snapshot(with_book: bool, book_deleted: bool) -> Snapshot {
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

    fn app_with_business(
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
        let _ = update(&mut app, Message::SelectBook(Book::Business));
        assert_eq!(app.book, Book::Business);
        assert_eq!(app.dashboard.how_am_i.income.value, "R$ 900,00");
        assert_eq!(app.dashboard.how_am_i.expense.value, "R$ 0,00");
        assert_eq!(app.transactions.list.count, 1);
        assert_eq!(super::super::shell::transactions_count(&app), 1);
        assert_eq!(
            super::super::shell::titles(&app).1,
            "Padaria · somente leitura · dados do último sync"
        );
        let _ = update(&mut app, Message::SelectBook(Book::Personal));
        assert_eq!(app.book, Book::Personal);
        assert_eq!(app.transactions.list.count, 7);
    }

    #[test]
    fn bloco_negocio_so_aparece_na_conta_comercial() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _tx) = app_with_business(dir.path(), true, false);
        assert_eq!(app.dashboard.business, None);
        let _ = update(&mut app, Message::SelectBook(Book::Business));
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
        let _ = update(&mut app, Message::SelectBook(Book::Personal));
        assert_eq!(app.dashboard.business, None);
    }

    #[test]
    fn dashboard_comercial_monta_com_o_negocio_primeiro() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _tx) = app_with_business(dir.path(), true, false);
        let _ = update(&mut app, Message::SelectBook(Book::Business));
        view_all_widths(&mut app);
    }

    #[test]
    fn tela_analise_calcula_achados_da_conta_aberta() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with_fixtures(dir.path());
        let _ = update(&mut app, Message::SelectScreen(Screen::Analysis));
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
    fn tela_analise_monta_com_e_sem_achados_e_com_valores_ocultos() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with_fixtures(dir.path());
        let _ = update(&mut app, Message::SelectScreen(Screen::Analysis));
        let _ = super::super::analysis_view::view(&app);
        let _ = update(&mut app, Message::ToggleHideValues);
        assert!(app.hide_values);
        let _ = super::super::analysis_view::view(&app);
        app.analysis = crate::advisor::Analysis::default();
        let _ = super::super::analysis_view::view(&app);
    }

    #[test]
    fn previa_mostra_exatamente_o_render_do_envelope() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with_fixtures(dir.path());
        let _ = update(&mut app, Message::SelectScreen(Screen::Analysis));
        assert!(app.envelope_preview.is_none());
        let _ = update(&mut app, Message::ShowEnvelope);
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
        let _ = update(&mut app, Message::CloseEnvelope);
        assert!(app.envelope_preview.is_none());
        assert!(super::super::analysis_view::envelope_dialog(&app).is_none());
    }

    #[test]
    fn previa_aberta_monta_pela_raiz_em_janelas_grandes_e_pequenas() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with_fixtures(dir.path());
        let _ = update(&mut app, Message::SelectScreen(Screen::Analysis));
        let _ = update(&mut app, Message::ShowEnvelope);
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
    fn previa_aberta_acompanha_a_troca_de_conta_e_os_dados() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _tx) = app_with_business(dir.path(), true, false);
        let _ = update(&mut app, Message::SelectScreen(Screen::Analysis));
        let _ = update(&mut app, Message::ShowEnvelope);
        assert!(
            app.envelope_preview
                .as_deref()
                .unwrap()
                .contains("Conta: pessoal")
        );
        let _ = update(&mut app, Message::SelectBook(Book::Business));
        let preview = app.envelope_preview.as_deref().unwrap();
        assert!(preview.contains("Conta: comercial"));
        assert_eq!(preview, app.current_envelope());
    }

    #[test]
    fn seletor_de_conta_aparece_na_analise_e_troca_os_achados() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _tx) = app_with_business(dir.path(), true, false);
        let _ = update(&mut app, Message::SelectScreen(Screen::Analysis));
        assert!(super::super::shell::book_switch(&app).is_some());
        let _ = update(&mut app, Message::SelectBook(Book::Business));
        assert_eq!(
            app.analysis,
            crate::advisor::analyze(&app.scope, &app.today)
        );
    }

    #[test]
    fn trocar_de_conta_limpa_os_filtros() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _tx) = app_with_business(dir.path(), true, false);
        let _ = update(&mut app, Message::PickCategory(Some("C1".into())));
        assert_eq!(app.filters.category.as_deref(), Some("C1"));
        let _ = update(&mut app, Message::SelectBook(Book::Business));
        assert_eq!(app.filters, Filters::default());
    }

    #[test]
    fn comercial_sem_conta_e_recusado() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _tx) = app_with_business(dir.path(), false, false);
        let _ = update(&mut app, Message::SelectBook(Book::Business));
        assert_eq!(app.book, Book::Personal);
        assert_eq!(app.transactions.list.count, 7);
    }

    #[test]
    fn conta_comercial_apagada_com_ela_aberta_volta_para_a_pessoal() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _tx) = app_with_business(dir.path(), true, false);
        let _ = update(&mut app, Message::SelectBook(Book::Business));
        assert_eq!(app.book, Book::Business);
        let _ = update(
            &mut app,
            Message::Snapshot(Box::new(business_snapshot(true, true))),
        );
        assert_eq!(app.book, Book::Personal);
        assert_eq!(app.transactions.list.count, 7);
    }

    // ---- assistente: chat e configuracao ----

    use crate::assistant::settings::Settings;
    use crate::assistant::view::{ConversationSummary, MessageRole, MessageView, OpenConversation};

    fn local_settings(model: Option<&str>) -> Settings {
        let mut s = Settings {
            provider: ProviderChoice::Local,
            ..Settings::default()
        };
        s.local.model = model.map(Into::into);
        s
    }

    /// App de exemplo na tela Analise com o assistente local configurado.
    fn assistant_app(
        dir: &std::path::Path,
        settings: Settings,
    ) -> (App, mpsc::UnboundedReceiver<Command>) {
        let (mut app, mut rx, _tx) = app_with_fixtures(dir);
        let _ = update(&mut app, Message::SelectScreen(Screen::Analysis));
        app.snapshot.assistant.settings = settings;
        assert!(drain(&mut rx).is_empty());
        (app, rx)
    }

    fn msg(role: MessageRole, content: &str) -> MessageView {
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

    fn open_conv(book: Book, messages: Vec<MessageView>) -> OpenConversation {
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
        let _ = update(&mut app, Message::StartConversation);
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
        let _ = update(&mut app, Message::ToggleHideValues);
        assert!(app.hide_values);
        let _ = update(&mut app, Message::StartConversation);
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
            let _ = update(&mut app, Message::StartConversation);
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
            let _ = update(&mut app, Message::ChatSubmit);
        }
        assert!(drain(&mut rx).is_empty());

        app.snapshot.assistant.busy = true;
        app.chat_input = "e agora?".into();
        let _ = update(&mut app, Message::ChatSubmit);
        assert!(drain(&mut rx).is_empty());
        assert_eq!(
            app.chat_input, "e agora?",
            "o texto fica para quando liberar"
        );

        app.snapshot.assistant.busy = false;
        app.chat_input = "  e agora?  ".into();
        // Sem conversa visivel nao ha para onde mandar.
        let _ = update(&mut app, Message::ChatSubmit);
        assert!(drain(&mut rx).is_empty());
        app.snapshot.assistant.open = Some(open_conv(Book::Personal, vec![]));
        let _ = update(&mut app, Message::ChatSubmit);
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
        let _ = update(&mut app, Message::ChatRetry);
        assert!(drain(&mut rx).is_empty());
        app.snapshot.assistant.open = Some(open_conv(Book::Personal, vec![]));
        let _ = update(&mut app, Message::ChatStop);
        let _ = update(&mut app, Message::ChatRetry);
        let _ = update(&mut app, Message::OpenConversation("C9".into()));
        let _ = update(&mut app, Message::CloseConversation);
        let _ = update(&mut app, Message::DeleteConversation("C9".into()));
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
        let _ = update(&mut app, Message::ChatRetry);
        assert!(drain(&mut rx).is_empty());
    }

    #[test]
    fn apagar_todas_pede_segundo_clique_dentro_do_prazo() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx) = assistant_app(dir.path(), local_settings(Some("m")));
        let _ = update(&mut app, Message::DeleteAllConversations);
        assert!(app.confirm_delete_all.is_some());
        assert!(drain(&mut rx).is_empty());
        let _ = update(&mut app, Message::DeleteAllConversations);
        assert_eq!(
            drain(&mut rx),
            vec![Command::Assistant(AssistantCommand::DeleteAll)]
        );
        assert!(app.confirm_delete_all.is_none());

        // Prazo vencido: o clique so arma de novo, e o tique desarma.
        app.confirm_delete_all = Some(Instant::now() - Duration::from_millis(1));
        let _ = update(&mut app, Message::DeleteAllConversations);
        assert!(drain(&mut rx).is_empty());
        assert!(app.confirm_delete_all.is_some());
        app.confirm_delete_all = Some(Instant::now() - Duration::from_millis(1));
        let _ = update(&mut app, Message::Tick);
        assert!(app.confirm_delete_all.is_none());
    }

    fn setup(app: &App) -> &super::super::assistant_settings_view::AssistantSetup {
        app.assistant_setup.as_ref().expect("dialogo aberto")
    }

    /// O snapshot depois de o nucleo terminar a gravacao: `configured` anda, e com sucesso a
    /// configuracao e a gravada.
    fn configured(app: &App, saved: Option<&Settings>, error: Option<&str>) -> Message {
        let mut s = app.snapshot.clone();
        s.assistant.configured += 1;
        if let Some(saved) = saved {
            s.assistant.settings = saved.clone();
        }
        s.assistant.last_error = error.map(Into::into);
        Message::Snapshot(Box::new(s))
    }

    #[test]
    fn dialogo_anda_pelos_passos_e_so_grava_no_ultimo_botao() {
        use super::super::assistant_settings_view::SetupStep;
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx) = assistant_app(dir.path(), Settings::default());
        let _ = update(&mut app, Message::ConfigureAssistant);
        assert_eq!(setup(&app).step, SetupStep::Where);
        assert_eq!(setup(&app).draft.choice, ProviderChoice::None);
        // Escolher no passo 1 nao grava nada.
        let _ = update(&mut app, Message::SetupChoice(ProviderChoice::Local));
        assert!(drain(&mut rx).is_empty());
        // Entrar no passo do Ollama ja testa a porta do campo.
        let _ = update(&mut app, Message::SetupNext);
        assert_eq!(setup(&app).step, SetupStep::Details);
        assert_eq!(
            drain(&mut rx),
            vec![Command::Assistant(AssistantCommand::ListModels(11434))]
        );
        let _ = update(&mut app, Message::SetupBack);
        assert_eq!(setup(&app).step, SetupStep::Where);
        let _ = update(&mut app, Message::SetupNext);
        let _ = drain(&mut rx);
        // A porta filtra digitos e o teste usa a porta do campo, sem gravar.
        let _ = update(&mut app, Message::SetupPort("11a5b00".into()));
        assert_eq!(setup(&app).draft.port_input, "11500");
        let _ = update(&mut app, Message::SetupTest);
        assert_eq!(
            drain(&mut rx),
            vec![Command::Assistant(AssistantCommand::ListModels(11500))]
        );
        // Testando: o botao nao manda outro pedido.
        app.snapshot.assistant.listing = true;
        let _ = update(&mut app, Message::SetupTest);
        assert!(drain(&mut rx).is_empty());
        let _ = super::super::shell::root(&app);
        app.snapshot.assistant.listing = false;
        // Porta invalida nao testa.
        let _ = update(&mut app, Message::SetupPort("0".into()));
        let _ = update(&mut app, Message::SetupTest);
        assert!(drain(&mut rx).is_empty());
        let _ = update(&mut app, Message::SetupPort("11500".into()));
        app.snapshot.assistant.models = Some(Ok(vec!["llama3.1:8b".into()]));
        app.snapshot.assistant.models_port = Some(11500);
        // Sem modelo escolhido o ultimo botao nao grava.
        let _ = update(&mut app, Message::SetupFinish);
        assert!(drain(&mut rx).is_empty());
        let _ = update(&mut app, Message::SetupLocalModel("llama3.1:8b".into()));
        let _ = super::super::shell::root(&app);
        let _ = update(&mut app, Message::SetupFinish);
        let mut expected = local_settings(Some("llama3.1:8b"));
        expected.local.port = 11500;
        assert_eq!(
            drain(&mut rx),
            vec![Command::Assistant(AssistantCommand::Configure {
                settings: expected.clone(),
                key: None,
            })]
        );
        // Espera o nucleo: aberto e "Salvando..." ate o snapshot com a gravacao.
        assert_eq!(setup(&app).saving, Some(1));
        let _ = update(&mut app, Message::SetupFinish);
        assert!(
            drain(&mut rx).is_empty(),
            "um clique a mais nao grava de novo"
        );
        let done = configured(&app, Some(&expected), None);
        let _ = update(&mut app, done);
        assert!(app.assistant_setup.is_none());
    }

    #[test]
    fn erro_ao_gravar_fica_no_dialogo_e_no_banner_da_analise() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx) = assistant_app(dir.path(), local_settings(Some("m")));
        let _ = update(&mut app, Message::ConfigureAssistant);
        let _ = update(&mut app, Message::SetupChoice(ProviderChoice::None));
        // "Desligado" termina no passo 1.
        let _ = update(&mut app, Message::SetupFinish);
        let off = Settings {
            provider: ProviderChoice::None,
            ..local_settings(Some("m"))
        };
        assert_eq!(
            drain(&mut rx),
            vec![Command::Assistant(AssistantCommand::Configure {
                settings: off,
                key: None,
            })]
        );
        let failed = configured(
            &app,
            None,
            Some("Não foi possível salvar a configuração do assistente."),
        );
        let _ = update(&mut app, failed);
        // Aberto, sem "Salvando...", com o erro; a Analise mostra o mesmo erro em banner.
        assert_eq!(setup(&app).saving, None);
        assert!(super::super::assistant_settings_view::dialog(&app).is_some());
        let _ = super::super::analysis_view::view(&app);
        let _ = super::super::shell::root(&app);
    }

    #[test]
    fn desligado_sem_nada_a_gravar_so_fecha() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx) = assistant_app(dir.path(), Settings::default());
        let _ = update(&mut app, Message::ConfigureAssistant);
        let _ = update(&mut app, Message::SetupFinish);
        assert!(drain(&mut rx).is_empty());
        assert!(app.assistant_setup.is_none());
    }

    #[test]
    fn nuvem_leva_a_chave_digitada_no_mesmo_comando_e_a_limpa_na_hora() {
        use crate::assistant::secret::Secret;
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx) = assistant_app(dir.path(), Settings::default());
        let _ = update(&mut app, Message::ConfigureAssistant);
        let _ = update(&mut app, Message::SetupChoice(ProviderChoice::Cloud));
        let _ = update(&mut app, Message::SetupNext);
        // A nuvem nao lista modelos (seria trafego antes de pergunta).
        assert!(drain(&mut rx).is_empty());
        let _ = super::super::shell::root(&app);
        // Sem chave nenhuma o ultimo botao nao grava.
        let _ = update(&mut app, Message::SetupFinish);
        assert!(drain(&mut rx).is_empty());
        let _ = update(
            &mut app,
            Message::KeyInput(Secret::new("sk-or-segredo".into())),
        );
        assert!(drain(&mut rx).is_empty(), "digitar nao salva");
        let _ = update(&mut app, Message::SetupCloudModel(" openai/gpt-4o ".into()));
        let _ = update(&mut app, Message::SetupDeny(false));
        let _ = update(&mut app, Message::SetupFinish);
        assert!(
            app.key_input.is_empty(),
            "a chave nao fica no estado da tela"
        );
        let mut expected = Settings {
            provider: ProviderChoice::Cloud,
            ..Settings::default()
        };
        expected.cloud.enabled = true;
        expected.cloud.model = "openai/gpt-4o".into();
        expected.cloud.deny_data_collection = false;
        let sent = drain(&mut rx);
        assert_eq!(
            sent,
            vec![Command::Assistant(AssistantCommand::Configure {
                settings: expected,
                key: Some(Secret::new("sk-or-segredo".into())),
            })]
        );
        // O Debug do que foi mandado (e de qualquer mensagem) nunca mostra a chave.
        assert!(!format!("{sent:?}").contains("segredo"));
        let typed = Message::KeyInput(Secret::new("sk-or-segredo".into()));
        assert!(!format!("{typed:?}").contains("segredo"));
    }

    #[test]
    fn a_chave_digitada_e_limpa_ao_fechar_por_qualquer_caminho() {
        use crate::assistant::secret::Secret;
        let dir = tempfile::tempdir().unwrap();
        let esc = super::super::keys::KeyPress {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
            physical: iced::keyboard::key::Physical::Code(iced::keyboard::key::Code::Escape),
            modifiers: iced::keyboard::Modifiers::empty(),
            repeat: false,
        };
        let closes = [
            Message::CloseAssistantSetup,
            Message::KeyPressed(esc),
            Message::SelectScreen(Screen::Dashboard),
        ];
        for close in closes {
            let (mut app, mut rx) = cloud_app(dir.path());
            app.snapshot.assistant.has_key = false;
            let _ = update(&mut app, Message::ConfigureAssistant);
            let _ = update(&mut app, Message::SetupChoice(ProviderChoice::Cloud));
            let _ = update(&mut app, Message::SetupNext);
            let _ = update(&mut app, Message::KeyInput(Secret::new("sk-or-y".into())));
            let label = format!("{close:?}");
            let _ = update(&mut app, close);
            assert!(app.assistant_setup.is_none(), "{label}");
            assert!(app.key_input.is_empty(), "{label}");
            assert!(drain(&mut rx).is_empty(), "{label}: nada e gravado");
        }
        // Janela fechada para a bandeja.
        let (mut app, _fake, mut rx) = with_tray(dir.path());
        let _ = update(&mut app, Message::ConfigureAssistant);
        let _ = update(&mut app, Message::KeyInput(Secret::new("sk-or-z".into())));
        let _ = update(&mut app, Message::CloseRequested);
        assert!(app.assistant_setup.is_none());
        assert!(app.key_input.is_empty());
        assert!(drain(&mut rx).is_empty());
        // "Remover" apaga a chave salva na hora.
        let (mut app, mut rx) = cloud_app(dir.path());
        let _ = update(&mut app, Message::ConfigureAssistant);
        let _ = update(&mut app, Message::RemoveKey);
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
        let _ = update(&mut app, Message::ConfigureAssistant);
        let _ = update(&mut app, Message::SetupPort("11500".into()));
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
        let _ = update(&mut app, Message::SetupNext);
        let _ = drain(&mut rx);
        let _ = update(&mut app, Message::SetupFinish);
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
    fn dialogos_do_assistente_montam_em_todos_os_estados() {
        use super::super::assistant_settings_view::SetupStep;
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx) = cloud_app(dir.path());
        let _ = update(&mut app, Message::ConfigureAssistant);
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

    // ---- nuvem: configuracao, chave e previa ----

    fn cloud_settings() -> Settings {
        let mut s = Settings {
            provider: ProviderChoice::Cloud,
            ..Settings::default()
        };
        s.cloud.enabled = true;
        s
    }

    fn cloud_app(dir: &std::path::Path) -> (App, mpsc::UnboundedReceiver<Command>) {
        let (mut app, rx) = assistant_app(dir, cloud_settings());
        app.snapshot.assistant.has_key = true;
        (app, rx)
    }

    fn awaiting_conv(book: Book, envelope: &str) -> OpenConversation {
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
        let _ = update(&mut app, Message::StartConversation);
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
        let _ = update(&mut app, Message::StartConversation);
        assert!(app.assistant_setup.is_some());
        assert!(drain(&mut rx).is_empty());

        let (mut app, mut rx) = cloud_app(dir.path());
        app.snapshot.assistant.settings.cloud.enabled = false;
        let _ = update(&mut app, Message::StartConversation);
        assert!(app.assistant_setup.is_some());
        assert!(drain(&mut rx).is_empty());
    }

    #[test]
    fn previa_aprovada_manda_o_id_da_conversa_e_so_quando_ha_o_que_aprovar() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx) = cloud_app(dir.path());
        // Nada aberto: o clique nao manda nada.
        let _ = update(&mut app, Message::ApproveCloudSend);
        assert!(drain(&mut rx).is_empty());

        app.snapshot.assistant.open = Some(awaiting_conv(app.book, "ENV DA CONVERSA"));
        assert!(super::awaiting_cloud_consent(&app).is_some());
        // Aprovar e so da previa aberta, presa a conversa.
        app.consent_preview = Some("C1".into());
        let _ = update(&mut app, Message::ApproveCloudSend);
        assert_eq!(
            drain(&mut rx),
            vec![Command::Assistant(AssistantCommand::ApproveCloudSend(
                "C1".into()
            ))]
        );

        // Enquanto espera, nem pergunta nova nem "Tentar de novo" saem.
        app.chat_input = "e agora?".into();
        let _ = update(&mut app, Message::ChatSubmit);
        let _ = update(&mut app, Message::ChatRetry);
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
        let _ = update(&mut app, Message::ApproveCloudSend);
        let _ = update(&mut app, Message::ChatRetry);
        assert_eq!(
            drain(&mut rx),
            vec![Command::Assistant(AssistantCommand::Resend)]
        );
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
        let body = super::super::analysis_view::preview_body(open);
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
        assert!(
            !super::super::analysis_view::PREVIEW_INTRO_EARLIER.contains("pergunta de abertura")
        );
        let _ = super::super::analysis_view::view(&app);
    }

    fn snapshot_with(app: &App, change: impl FnOnce(&mut Settings)) -> Message {
        let mut s = app.snapshot.clone();
        change(&mut s.assistant.settings);
        Message::Snapshot(Box::new(s))
    }

    #[test]
    fn previa_abre_em_dialogo_ao_conversar_na_nuvem_e_mostra_o_payload() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx) = cloud_app(dir.path());
        let _ = update(&mut app, Message::StartConversation);
        let _ = drain(&mut rx);
        assert!(app.consent_preview.is_none());
        // O snapshot com a conversa nova esperando aprovacao abre a previa.
        let snap = {
            let mut s = app.snapshot.clone();
            s.assistant.open = Some(awaiting_conv(app.book, "ENV DA CONVERSA"));
            Message::Snapshot(Box::new(s))
        };
        let _ = update(&mut app, snap);
        assert_eq!(app.consent_preview.as_deref(), Some("C1"));
        let open = super::super::analysis_view::shown_consent(&app).expect("previa aberta");
        // O texto do dialogo e o do payload (a mesma montagem do envio).
        let body = super::super::analysis_view::preview_body(open);
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
        let _ = update(&mut app, Message::CancelPreview);
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
            Message::Snapshot(Box::new(s))
        };
        let _ = update(&mut app, snap);
        assert!(app.consent_preview.is_none());
        assert!(super::super::analysis_view::consent_dialog(&app).is_none());
        // O pe da conversa leva a ela.
        let _ = update(&mut app, Message::ShowCloudPreview);
        assert_eq!(app.consent_preview.as_deref(), Some("C1"));
        let _ = update(&mut app, Message::ApproveCloudSend);
        assert!(app.consent_preview.is_none());
        assert_eq!(
            drain(&mut rx),
            vec![Command::Assistant(AssistantCommand::ApproveCloudSend(
                "C1".into()
            ))]
        );
    }

    #[test]
    fn previa_presa_a_outra_conversa_nao_aprova_a_aberta() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx) = cloud_app(dir.path());
        app.snapshot.assistant.open = Some(awaiting_conv(app.book, "ENV"));
        app.consent_preview = Some("C0".into());
        assert!(super::super::analysis_view::shown_consent(&app).is_none());
        let _ = update(&mut app, Message::ApproveCloudSend);
        assert!(drain(&mut rx).is_empty());
        // Um snapshot depois, a previa da conversa que saiu da tela fecha.
        let snap = Message::Snapshot(Box::new(app.snapshot.clone()));
        let _ = update(&mut app, snap);
        assert!(app.consent_preview.is_none());
    }

    fn summary(id: &str, book: Book) -> ConversationSummary {
        ConversationSummary {
            id: id.into(),
            book,
            title: format!("Conversa {id}"),
            created_at: String::new(),
        }
    }

    #[test]
    fn lista_de_conversas_e_so_da_conta_aberta() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx) = assistant_app(dir.path(), local_settings(Some("m")));
        app.snapshot.assistant.conversations = vec![
            summary("P1", Book::Personal),
            summary("B1", Book::Business),
            summary("P2", Book::Personal),
        ];
        let ids = |app: &App| {
            super::super::analysis_view::conversations_of_book(app)
                .iter()
                .map(|c| c.id.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(&app), ["P1", "P2"]);
        app.book = Book::Business;
        assert_eq!(ids(&app), ["B1"]);
    }

    fn conversation_ids(app: &App) -> Vec<String> {
        super::super::analysis_view::conversations_of_book(app)
            .iter()
            .map(|c| c.id.clone())
            .collect()
    }

    #[test]
    fn apagar_conversa_some_na_hora_e_desfazer_nao_envia_nada() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx) = assistant_app(dir.path(), local_settings(Some("m")));
        app.snapshot.assistant.conversations =
            vec![summary("P1", Book::Personal), summary("P2", Book::Personal)];
        let _ = update(&mut app, Message::AskDeleteConversation("P1".into()));
        assert_eq!(conversation_ids(&app), ["P2"]);
        assert!(drain(&mut rx).is_empty());
        // O toast monta pela raiz e o pendente nao esconde nenhum aparelho.
        let _ = super::super::shell::root(&app);
        assert_eq!(app.pending_removal.as_ref().unwrap().device_id(), None);
        let _ = update(&mut app, Message::UndoRemove);
        assert_eq!(conversation_ids(&app), ["P1", "P2"]);
        assert!(drain(&mut rx).is_empty());
        assert_eq!(app.pending_removal, None);
    }

    #[test]
    fn apagar_conversa_sai_no_prazo_na_troca_de_tela_e_antes_do_shutdown() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx) = assistant_app(dir.path(), local_settings(Some("m")));
        let delete = |id: &str| Command::Assistant(AssistantCommand::Delete(id.into()));
        // Prazo: antes dele nada sai.
        let _ = update(&mut app, Message::AskDeleteConversation("C1".into()));
        let _ = update(&mut app, Message::Tick);
        assert!(drain(&mut rx).is_empty());
        app.pending_removal.as_mut().unwrap().deadline = Instant::now();
        let _ = update(&mut app, Message::Tick);
        assert_eq!(drain(&mut rx), [delete("C1")]);
        // Troca de tela.
        let _ = update(&mut app, Message::AskDeleteConversation("C2".into()));
        let _ = update(&mut app, Message::SelectScreen(Screen::Dashboard));
        assert_eq!(drain(&mut rx), [delete("C2")]);
        // Outro apagar manda o anterior.
        let _ = update(&mut app, Message::AskDeleteConversation("C3".into()));
        let _ = update(&mut app, Message::AskDeleteConversation("C4".into()));
        assert_eq!(drain(&mut rx), [delete("C3")]);
        // Fechar (sem bandeja, desliga): o apagar sai antes do `Shutdown`.
        let _ = update(&mut app, Message::CloseRequested);
        assert_eq!(drain(&mut rx), [delete("C4"), Command::Shutdown]);
    }

    #[test]
    fn apagar_a_conversa_aberta_so_a_esconde_e_desfazer_a_mostra_de_novo() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx) = assistant_app(dir.path(), local_settings(Some("m")));
        let open = open_conv(Book::Personal, vec![]);
        let id = open.id.clone();
        app.snapshot.assistant.open = Some(open);
        let _ = update(&mut app, Message::AskDeleteConversation(id));
        assert!(super::super::analysis_view::open_conversation(&app).is_none());
        let _ = super::super::analysis_view::view(&app);
        // Nada sai antes do prazo: nem `Close`, que interromperia a resposta.
        assert!(drain(&mut rx).is_empty());
        let _ = update(&mut app, Message::UndoRemove);
        assert!(super::super::analysis_view::open_conversation(&app).is_some());
        assert!(drain(&mut rx).is_empty());
    }

    fn deliver(app: &mut App, change: impl FnOnce(&mut Snapshot)) {
        let mut snap = app.snapshot.clone();
        change(&mut snap);
        let _ = update(app, Message::Snapshot(Box::new(snap)));
    }

    #[test]
    fn conversa_apagada_continua_escondida_ate_o_snapshot_sem_ela() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx) = assistant_app(dir.path(), local_settings(Some("m")));
        let open = open_conv(Book::Personal, vec![]);
        let id = open.id.clone();
        app.snapshot.assistant.open = Some(open);
        app.snapshot.assistant.conversations =
            vec![summary(&id, Book::Personal), summary("P2", Book::Personal)];
        let _ = update(&mut app, Message::AskDeleteConversation(id.clone()));
        app.pending_removal.as_mut().unwrap().deadline = Instant::now();
        let _ = update(&mut app, Message::Tick);
        assert_eq!(
            drain(&mut rx),
            [Command::Assistant(AssistantCommand::Delete(id.clone()))]
        );
        // O nucleo ainda nao processou: o snapshot traz a conversa, e ela segue fora da tela.
        assert_eq!(conversation_ids(&app), ["P2"]);
        assert!(super::super::analysis_view::open_conversation(&app).is_none());
        deliver(&mut app, |_| {});
        assert_eq!(conversation_ids(&app), ["P2"]);
        assert!(super::super::analysis_view::open_conversation(&app).is_none());
        // Snapshot sem ela: o id sai do conjunto.
        deliver(&mut app, |s| {
            s.assistant.open = None;
            s.assistant.conversations.retain(|c| c.id != id);
        });
        assert!(app.deleting.is_empty());
        assert_eq!(conversation_ids(&app), ["P2"]);
    }

    #[test]
    fn apagar_que_falhou_traz_a_conversa_de_volta_com_o_aviso() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx) = assistant_app(dir.path(), local_settings(Some("m")));
        app.snapshot.assistant.conversations = vec![summary("P1", Book::Personal)];
        let _ = update(&mut app, Message::AskDeleteConversation("P1".into()));
        app.pending_removal.as_mut().unwrap().deadline = Instant::now();
        let _ = update(&mut app, Message::Tick);
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
        let _ = update(&mut app, Message::AskDeleteConversation("C1".into()));
        let deadline = app.pending_removal.as_ref().unwrap().deadline;
        let _ = update(&mut app, Message::AskDeleteConversation("C1".into()));
        assert_eq!(app.pending_removal.as_ref().unwrap().deadline, deadline);
        assert!(drain(&mut rx).is_empty());
    }

    #[test]
    fn conversa_pendente_sobrevive_a_troca_de_conta_e_sai_no_prazo() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) =
            app_with(business_snapshot(true, false), dir.path().to_path_buf());
        app.snapshot.assistant.conversations = vec![summary("P1", Book::Personal)];
        let _ = update(&mut app, Message::AskDeleteConversation("P1".into()));
        let _ = update(&mut app, Message::SelectBook(Book::Business));
        let _ = update(&mut app, Message::SelectBook(Book::Personal));
        assert!(conversation_ids(&app).is_empty());
        assert!(
            !drain(&mut rx)
                .iter()
                .any(|c| matches!(c, Command::Assistant(AssistantCommand::Delete(_))))
        );
        app.pending_removal.as_mut().unwrap().deadline = Instant::now();
        let _ = update(&mut app, Message::Tick);
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
        let _ = update(&mut app, Message::AskDeleteConversation("C1".into()));
        let _ = update(&mut app, Message::CloseRequested);
        assert_eq!(drain(&mut rx), std::slice::from_ref(&delete));
        assert!(!app.closing);
        let _ = update(&mut app, Message::AskDeleteConversation("C1".into()));
        let _ = update(&mut app, Message::Desktop(DesktopEvent::Quit));
        assert_eq!(drain(&mut rx), [delete, Command::Shutdown]);
    }

    #[test]
    fn conversa_depois_de_aparelho_manda_o_aparelho_e_o_toast_e_da_conversa() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with_two_people(dir.path());
        let _ = update(&mut app, Message::AskRemove("D1".into()));
        let _ = update(&mut app, Message::ConfirmRemove);
        let _ = update(&mut app, Message::AskDeleteConversation("C1".into()));
        assert!(matches!(
            drain(&mut rx).as_slice(),
            [Command::RemoveDevice { device_id, .. }] if device_id == "D1"
        ));
        let pending = app.pending_removal.clone().unwrap();
        assert_eq!(pending.conversation_id(), Some("C1"));
        assert_eq!(pending.toast, "Conversa apagada");
        // Desfazer vale para a conversa; a remocao ja saiu.
        let _ = update(&mut app, Message::UndoRemove);
        assert!(drain(&mut rx).is_empty());
    }

    #[test]
    fn aparelho_depois_de_conversa_manda_a_conversa_e_o_toast_e_do_aparelho() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with_two_people(dir.path());
        let _ = update(&mut app, Message::AskDeleteConversation("C1".into()));
        let _ = update(&mut app, Message::AskRemove("D1".into()));
        let _ = update(&mut app, Message::ConfirmRemove);
        assert_eq!(
            drain(&mut rx),
            [Command::Assistant(AssistantCommand::Delete("C1".into()))]
        );
        let pending = app.pending_removal.clone().unwrap();
        assert_eq!(pending.device_id(), Some("D1"));
        assert_eq!(pending.toast, "Pixel 7 foi removido do hub");
        let _ = update(&mut app, Message::UndoRemove);
        assert!(drain(&mut rx).is_empty());
        assert_eq!(app.pending_removal, None);
    }

    #[test]
    fn apagar_todas_nao_manda_a_remocao_de_aparelho_pendente() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with_two_people(dir.path());
        let _ = update(&mut app, Message::AskRemove("D1".into()));
        let _ = update(&mut app, Message::ConfirmRemove);
        let _ = update(&mut app, Message::DeleteAllConversations);
        let _ = update(&mut app, Message::DeleteAllConversations);
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
        let _ = update(&mut app, Message::AskDeleteConversation("C1".into()));
        assert_eq!(app.hover_row, None);
    }

    #[test]
    fn apagar_todas_manda_o_apagar_pendente_antes() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx) = assistant_app(dir.path(), local_settings(Some("m")));
        let _ = update(&mut app, Message::AskDeleteConversation("C1".into()));
        let _ = update(&mut app, Message::DeleteAllConversations);
        let _ = update(&mut app, Message::DeleteAllConversations);
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
    fn conversa_de_outra_conta_nao_aparece_como_aberta() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx) = assistant_app(dir.path(), local_settings(Some("m")));
        app.snapshot.assistant.open = Some(open_conv(Book::Business, vec![]));
        assert!(super::super::analysis_view::open_conversation(&app).is_none());
        app.book = Book::Business;
        assert!(super::super::analysis_view::open_conversation(&app).is_some());
    }

    #[test]
    fn aviso_de_dados_mudados_compara_o_envelope_da_conversa_com_o_de_agora() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx) = assistant_app(dir.path(), local_settings(Some("m")));
        let changed = super::super::analysis_view::data_changed;
        assert!(!changed(&app), "sem conversa aberta nao ha aviso");

        // O nucleo publica a conversa com o envelope que a UI mandou.
        let mut open = open_conv(app.book, vec![]);
        open.envelope = app.current_envelope();
        app.snapshot.assistant.open = Some(open);
        assert!(!changed(&app));

        // Sync que nao mexe nos numeros desta conta (outro `Arc`, mesmo conteudo): sem aviso.
        let mut snap = app.snapshot.clone();
        snap.dashboard = std::sync::Arc::new(crate::dashboard::fixtures());
        let _ = update(&mut app, Message::Snapshot(Box::new(snap)));
        assert!(!changed(&app));

        // Dados que mudam o envelope: aviso.
        let before = app.current_envelope();
        let mut snap = app.snapshot.clone();
        snap.dashboard = std::sync::Arc::new(crate::dashboard::fixtures_grafite());
        let _ = update(&mut app, Message::Snapshot(Box::new(snap)));
        assert_ne!(
            app.current_envelope(),
            before,
            "o teste precisa de dados que mudem o envelope"
        );
        assert!(changed(&app));

        // Conversa antiga reaberta, de quando os dados eram outros: aviso tambem.
        let mut old = open_conv(app.book, vec![]);
        old.envelope = "envelope antigo".into();
        app.snapshot.assistant.open = Some(old);
        assert!(changed(&app));

        // Conversa da outra conta nao e mostrada, entao nao avisa.
        let mut other = open_conv(Book::Business, vec![]);
        other.envelope = "envelope antigo".into();
        app.snapshot.assistant.open = Some(other);
        assert!(!changed(&app));
    }

    #[test]
    fn pergunta_sem_resposta_e_sem_nada_em_andamento_monta_com_tentar_de_novo() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx) = assistant_app(dir.path(), local_settings(Some("m")));
        app.snapshot.assistant.open =
            Some(open_conv(app.book, vec![msg(MessageRole::User, "Resuma")]));
        let _ = super::super::analysis_view::view(&app);
        // O botao manda `ChatRetry`, que reenvia a pergunta que ficou sem resposta.
        let _ = update(&mut app, Message::ChatRetry);
        assert_eq!(
            drain(&mut rx),
            vec![Command::Assistant(AssistantCommand::Resend)]
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
        let _ = update(&mut app, Message::SelectBook(Book::Personal));
        assert!(drain(&mut rx).is_empty());
        let _ = update(&mut app, Message::SelectBook(Book::Business));
        assert_eq!(app.book, Book::Business);
        assert_eq!(
            drain(&mut rx),
            vec![Command::Assistant(AssistantCommand::Close)]
        );
        // Sem conversa aberta, trocar de conta nao manda nada.
        app.snapshot.assistant.open = None;
        let _ = update(&mut app, Message::SelectBook(Book::Personal));
        assert!(drain(&mut rx).is_empty());
    }

    #[test]
    fn tela_analise_monta_com_conversa_streaming_erro_e_painel() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx) = assistant_app(dir.path(), local_settings(Some("llama3.1:8b")));
        let _ = update(&mut app, Message::StartConversation);
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
        let _ = update(&mut app, Message::ConfigureAssistant);
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
}
