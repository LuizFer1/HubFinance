//! Textos e decisoes puras da casca: versao da marca, plural e milhar do pt-BR, o atalho de cada
//! item da navegacao e qual estado uma tela sem dados mostra. Sem toolkit, para as duas
//! interfaces e os testes dizerem a mesma coisa.

use crate::advisor::Severity;
use crate::dashboard::money::group_thousands;
use crate::dashboard::periods::month_long;
use crate::hub::snapshot::{Snapshot, Status};
use crate::ui_model::app::{App, Msg, Screen};
use crate::ui_model::time;
use ::time::OffsetDateTime;

/// A versao precisa de um lugar sempre a vista (Ajustes > Sobre so aparece a quem abre a
/// tela), e o subtitulo curto nao quebra em duas linhas. "Somente leitura" ficou fora dela de
/// proposito: e o subtitulo do Resumo, que no mes corrente da lugar a linha "Parcial".
pub fn brand_subtitle() -> String {
    format!("Hub de casa · v{}", crate::config::VERSION)
}

/// Texto do estado "Abrindo", o mesmo nas tres telas de dados.
pub const OPENING_TEXT: &str = "Abrindo os dados da casa";

/// Linha do Resumo no boot: so o que vale com ou sem dados. "Dados do último sync" ainda nao e
/// verdade enquanto o banco nao abriu.
pub const DASHBOARD_LINE: &str = "Somente leitura";

/// O que uma tela de dados mostra enquanto nao tem o que mostrar (spec 8.2 e A5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Arrival {
    /// A tela tem dados: desenha o que tem.
    Data,
    /// Nucleo subindo (`Starting`). O primeiro snapshot sai vazio antes de o banco abrir
    /// (`hub/run.rs`); dizer "Nenhum dado" nesse intervalo seria um vazio falso.
    Opening,
    /// Nucleo no ar, hub sem nenhuma linha e nenhum aparelho pareado: "Comece por aqui".
    FirstUse,
    /// Hub sem nenhuma linha e com aparelho pareado que nunca sincronizou: "Aguardando o
    /// primeiro sync de ...".
    Waiting(String),
    /// Sem dados para esta tela, mas o hub tem linhas (so reservas, conta comercial vazia) ou
    /// o nucleo esta desligando: o vazio de sempre, sem prometer um sync que talvez ja houve.
    Empty,
}

/// Escolhe o estado. A ordem importa: dado na tela ganha de tudo (um snapshot com dados nunca
/// mostra esqueleto), depois o boot, e so com o nucleo no ar e o hub vazio de verdade os passos
/// do primeiro uso ou a espera; o hub com erro nem chega aqui (a casca troca a tela pelo card
/// de falha). `any_synced` (algum aparelho ativo ja enviou ou recebeu) desliga a espera: o
/// primeiro sync manda perfil, categorias e formas de pagamento, que nao sao lancamento nem
/// reserva, e "Aguardando o primeiro sync" seria mentira para quem so nao lancou nada ainda.
pub fn arrival(
    status: &Status,
    screen_has_data: bool,
    hub_has_rows: bool,
    devices: &[&str],
    any_synced: bool,
) -> Arrival {
    if screen_has_data {
        return Arrival::Data;
    }
    match status {
        Status::Starting => Arrival::Opening,
        Status::Running | Status::NoNetwork if !hub_has_rows => {
            if devices.is_empty() {
                Arrival::FirstUse
            } else if any_synced {
                Arrival::Empty
            } else {
                Arrival::Waiting(waiting_line(devices))
            }
        }
        _ => Arrival::Empty,
    }
}

/// "Aguardando o primeiro sync de Pixel da Ana"; com mais de um, a contagem ("de 2 celulares"):
/// a lista de nomes numa linha so nao caberia, e qualquer um deles pode ser o primeiro.
pub fn waiting_line(devices: &[&str]) -> String {
    match devices {
        [] => "Aguardando o primeiro sync".to_string(),
        [one] => format!("Aguardando o primeiro sync de {one}"),
        many => format!(
            "Aguardando o primeiro sync de {}",
            plural(many.len(), "celular", "celulares")
        ),
    }
}

/// Texto do tooltip do item: o atalho da posicao `n` (1 a 6). Sai do mesmo indice que
/// `keys::shortcut` usa, entao nao tem como dizer um numero e a tecla fazer outro.
pub fn nav_shortcut(n: usize) -> String {
    format!("Ctrl+{n}")
}

/// A pergunta que Lancamentos responde (principio 1 da direcao Grafite · Mesa), no mes
/// escolhido. A contagem que esta linha trazia ("4.688 lançamentos sincronizados") desceu para
/// o resumo do mes, logo acima da tabela, onde conta o que a tabela mostra.
pub fn transactions_question(month: &str) -> String {
    format!("O que entrou e saiu em {}?", month_long(month))
}

/// A pergunta que Reservas responde. A tela nao tem seletor de mes, entao nunca da lugar ao
/// "Parcial".
pub const RESERVES_QUESTION: &str = "Quanto está guardado, e para quê?";

/// "1 lançamento" / "1.234 lançamentos" (milhar com ponto, como o pt-BR do app).
pub fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{} {}", thousands(n), if n == 1 { one } else { many })
}

pub fn thousands(n: usize) -> String {
    group_thousands(n as u64)
}

/// Motivo do hub que nao subiu, se a tela atual deve dar lugar a ele. Ajustes fica de fora:
/// tema, inicializacao e as pastas de dados e logs nao dependem do nucleo, e as pastas sao
/// justamente o que a pessoa procura para entender a falha.
pub fn failure(app: &App) -> Option<&str> {
    match &app.snapshot.status {
        Status::Failed(msg) if app.screen != Screen::Settings => Some(msg),
        _ => None,
    }
}

/// A ordem da navegacao lateral, que e tambem a do Ctrl+1..6. Mora aqui para o atalho nao
/// depender dos glifos e rotulos da barra; a lateral de cada toolkit desenha nesta ordem.
pub const NAV_ORDER: [Screen; 6] = [
    Screen::Dashboard,
    Screen::Transactions,
    Screen::Reserves,
    Screen::Analysis,
    Screen::Connection,
    Screen::Settings,
];

/// Para onde o card de status leva: Aparelhos, onde estao o endereco e quem esta conectado.
pub const STATUS_CARD_TARGET: Screen = Screen::Connection;

/// Achados de severidade "alerta" da conta aberta, do mesmo modelo que a Analise desenha.
/// Zero nao aparece: um "0" na lateral seria ruido.
pub fn alert_count(app: &App) -> Option<usize> {
    let n = app
        .analysis
        .findings
        .iter()
        .filter(|f| f.severity == Severity::Alert)
        .count();
    (n > 0).then_some(n)
}

/// Lancamentos vivos da conta escolhida (do modelo do Dashboard, que ja os contou).
pub fn transactions_count(app: &App) -> usize {
    app.dashboard.sidebar.transactions
}

/// Aparelhos ativos: e o numero de linhas da lista "Usuarios conectados" (sem a remocao
/// pendente, que ja sumiu da tela).
pub fn active_devices(app: &App) -> usize {
    active_device_names(app).len()
}

/// Nomes dos aparelhos ativos, na ordem do snapshot. A mesma regra de `active_devices`, para o
/// "Aguardando o primeiro sync de ..." nunca citar um aparelho que a contagem nao conta.
pub fn active_device_names(app: &App) -> Vec<&str> {
    let pending = app
        .pending_removal
        .as_ref()
        .and_then(crate::ui_model::app::PendingRemoval::device_id);
    app.snapshot
        .devices
        .iter()
        .filter(|d| !d.revoked && Some(d.device_id.as_str()) != pending)
        .map(|d| d.name.as_str())
        .collect()
}

/// Algum aparelho ativo (a regra de `active_device_names`) ja fez um envio ou recebimento.
pub fn any_active_synced(app: &App) -> bool {
    let pending = app
        .pending_removal
        .as_ref()
        .and_then(crate::ui_model::app::PendingRemoval::device_id);
    app.snapshot.devices.iter().any(|d| {
        !d.revoked
            && Some(d.device_id.as_str()) != pending
            && (d.last_push_at.is_some() || d.last_pull_at.is_some())
    })
}

/// Ha alguma linha viva no hub, em qualquer conta: lancamento ou reserva. E o que separa o hub
/// que nunca recebeu um sync (passos, espera) da tela que so esta vazia nesta conta.
pub fn hub_has_rows(app: &App) -> bool {
    let ds = &app.snapshot.dashboard;
    !ds.is_empty() || ds.has_reserves()
}

/// O estado de uma tela de dados que tem (ou nao) o que mostrar.
pub fn screen_arrival(app: &App, screen_has_data: bool) -> Arrival {
    arrival(
        &app.snapshot.status,
        screen_has_data,
        hub_has_rows(app),
        &active_device_names(app),
        any_active_synced(app),
    )
}

/// Endereco principal e porta HTTPS; sem rede, so a porta.
pub fn address(app: &App) -> Option<String> {
    app.snapshot
        .addresses
        .first()
        .map(|ip| format!("{ip}:{}", app.snapshot.https_port))
}

/// Ha lancamento ou reserva viva no hub: uma casa que so sincronizou reservas tem dados.
pub fn has_data(app: &App) -> bool {
    has_transactions(app) || app.scope.has_reserves()
}

/// Ha lancamento vivo: o estado vazio do Dashboard e de Lancamentos e sobre lancamentos.
pub fn has_transactions(app: &App) -> bool {
    app.dashboard.sidebar.has_data
}

/// "Último sync há 2 min · Pixel da Ana": o maior entre envio e recebimento de todos os
/// aparelhos. RFC 3339 em UTC compara como string, entao nao precisa parsear para escolher.
pub fn last_sync_line(app: &App) -> String {
    let latest = app
        .snapshot
        .devices
        .iter()
        .flat_map(|d| {
            [d.last_push_at.as_deref(), d.last_pull_at.as_deref()]
                .into_iter()
                .flatten()
                .map(move |at| (at, d.name.as_str()))
        })
        .max_by(|a, b| a.0.cmp(b.0));
    match latest {
        Some((at, name)) => format!("Último sync {} · {name}", time::relative(Some(at))),
        None => "Nenhum sync ainda".into(),
    }
}

/// A mensagem do clique no card, a unica que o card de status da lateral usa.
pub fn status_card_press() -> Msg {
    Msg::SelectScreen(STATUS_CARD_TARGET)
}

/// A tela atual gerencia a propria rolagem? Lancamentos (B7: so as linhas rolam) e Analise
/// (8.5: cada painel com a sua), e so quando aparecem de fato: com o hub em erro o card de falha
/// entra no lugar e volta a rolagem geral.
pub fn owns_scroll(app: &App) -> bool {
    matches!(app.screen, Screen::Transactions | Screen::Analysis) && failure(app).is_none()
}

/// "Parcial · até 19 nov": so nas telas com seletor de mes, no mes corrente e com dados (o
/// modelo ja decide as duas ultimas). Vira a linha do cabecalho (`header_line`).
pub fn partial_chip(app: &App) -> Option<&str> {
    matches!(app.screen, Screen::Dashboard | Screen::Transactions)
        .then_some(app.dashboard.header.partial.as_deref())
        .flatten()
}

/// Titulo e subtitulo de cada tela. O do Dashboard vem pronto do modelo Grafite ("Resumo da
/// casa"); a navegacao diz so "Resumo".
pub fn titles(app: &App) -> (&str, String) {
    let empty = !has_transactions(app);
    // No boot o vazio ainda nao quer dizer nada: "Nada sincronizado ainda" la seria o mesmo
    // vazio falso do corpo. A linha volta ao que a tela e; o corpo diz que esta abrindo.
    let opening = app.snapshot.status == Status::Starting;
    match app.screen {
        Screen::Dashboard => (
            app.dashboard.header.title.as_str(),
            if opening && app.dashboard.is_empty {
                DASHBOARD_LINE.into()
            } else {
                app.dashboard.header.subtitle.clone()
            },
        ),
        Screen::Transactions => (
            "Lançamentos",
            if empty && !opening {
                "Nada sincronizado ainda".into()
            } else {
                transactions_question(&app.transactions.month)
            },
        ),
        Screen::Reserves => (
            "Reservas",
            if app.reserves.is_empty && !opening {
                "Nada sincronizado ainda".into()
            } else {
                RESERVES_QUESTION.into()
            },
        ),
        Screen::Analysis => ("Análise", crate::ui_model::analysis::QUESTION.into()),
        Screen::Connection => ("Aparelhos", "Quem sincroniza com esta casa?".into()),
        Screen::Settings => ("Ajustes", "Preferências deste computador".into()),
    }
}

/// Titulo e a linha de baixo do cabecalho. A regra, igual nas telas de dados:
/// - mes corrente com dados, nas telas com seletor de mes: o "Parcial · até 24 set" do modelo
///   ganha da pergunta, porque e o que muda a leitura dos numeros (o mes ainda nao fechou) e o
///   Resumo ja faz assim. Em Lancamentos ele leva tambem quantos aparelhos enviaram ("· 2
///   aparelhos"): sao eles que ainda podem mandar o resto do mes;
/// - fora disso, o subtitulo de `titles`, que nas telas de dados e a pergunta que a tela
///   responde ("O que entrou e saiu em agosto?", "Quanto está guardado, e para quê?");
/// - sem nada sincronizado, "Nada sincronizado ainda": uma pergunta sobre dado que nao existe
///   nao faz sentido.
pub fn header_line(app: &App) -> (&str, String) {
    let (title, subtitle) = titles(app);
    let Some(partial) = partial_chip(app) else {
        return (title, subtitle);
    };
    let devices = app.transactions.synced_devices;
    let line = if app.screen == Screen::Transactions && devices > 0 {
        format!("{partial} · {}", plural(devices, "aparelho", "aparelhos"))
    } else {
        partial.to_string()
    };
    (title, line)
}

/// Icone do card de status: forma + cor, nunca so cor (spec §5). O anel girando do design e
/// `Spinner`; quem desenha decide a animacao (e a corta com "Reduzir movimento").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusIcon {
    /// Disco cheio com check: hub ligado e com dados.
    Ok,
    /// Check contornado: hub ligado, ainda sem dados. Forma propria para nao ser so mais um "Ok".
    OkEmpty,
    /// Losango: sem rede local.
    Warning,
    /// Anel com o arco de cima em `accent`: iniciando.
    Spinner,
    /// Anel com o arco de cima em `muted`: desligando. Outra cor sobre a mesma forma do
    /// `Spinner` seria "so cor"; a forma muda do "Desligado" (anel inteiro, Tarefa 32).
    Stopping,
    Danger,
}

/// Botao do card. So o erro tem um hoje; "Ligar hub" entra com o `Off` (Tarefa 32).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardAction {
    /// Leva a Ajustes, onde estao as pastas de dados e de logs que explicam a falha.
    Details,
}

impl CardAction {
    pub fn label(self) -> &'static str {
        match self {
            CardAction::Details => "Ver detalhes",
        }
    }

    pub fn msg(self) -> Msg {
        match self {
            CardAction::Details => Msg::SelectScreen(Screen::Settings),
        }
    }
}

/// O card de status da lateral: `lines[0]` fica sob o titulo e o resto abaixo do fio.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusCard {
    pub icon: StatusIcon,
    pub title: String,
    pub lines: Vec<String>,
    pub action: Option<CardAction>,
}

/// A tabela da spec §5. A porta vem do snapshot (`https_port`), nunca o 8443 do design: o hub
/// usa 7777 e um numero fixo na tela mentiria. `now` entra como parametro para o "ha 4 min" ser
/// testavel sem relogio.
pub fn status_card(snapshot: &Snapshot, now: OffsetDateTime) -> StatusCard {
    let card = |icon, title: &str, lines: Vec<String>| StatusCard {
        icon,
        title: title.to_string(),
        lines,
        action: None,
    };
    let addr = snapshot
        .addresses
        .first()
        .map(|ip| format!("{ip}:{}", snapshot.https_port));
    match &snapshot.status {
        Status::Running => {
            let rows = !snapshot.dashboard.is_empty() || snapshot.dashboard.has_reserves();
            match (rows, last_sync(snapshot)) {
                (true, Some((at, name))) => card(
                    StatusIcon::Ok,
                    "Hub ligado",
                    addr.into_iter()
                        .chain([
                            format!("Último sync {}", time::relative_to(at, now)),
                            name.to_string(),
                        ])
                        .collect(),
                ),
                // Linhas no hub sem nenhum carimbo de sync (aparelho removido, banco antigo):
                // nao inventa um "ha X min", mostra so onde o hub esta.
                (true, None) => card(StatusIcon::Ok, "Hub ligado", addr.into_iter().collect()),
                (false, _) => card(
                    StatusIcon::OkEmpty,
                    "Hub ligado",
                    vec!["Sem dados · aguardando o primeiro sync".into()],
                ),
            }
        }
        Status::NoNetwork => card(
            StatusIcon::Warning,
            "Sem rede local",
            vec!["Conecte o computador a um Wi-Fi".into()],
        ),
        Status::Starting => card(
            StatusIcon::Spinner,
            "Iniciando…",
            vec![match snapshot.https_port {
                0 => "Abrindo a porta de rede".to_string(),
                port => format!("Abrindo a porta {port}"),
            }],
        ),
        // `Stopped` so aparece ao sair: o mesmo texto de quem esta salvando.
        Status::Stopping | Status::Stopped => card(
            StatusIcon::Stopping,
            "Desligando…",
            vec!["Salvando os dados".into()],
        ),
        Status::Failed(msg) => StatusCard {
            icon: StatusIcon::Danger,
            title: "Hub com erro".into(),
            lines: vec![msg.clone()],
            action: Some(CardAction::Details),
        },
    }
}

/// O sync mais recente entre todos os aparelhos ativos (instante e nome). Aparelho removido nao
/// conta: o card falaria de quem ja nao sincroniza.
fn last_sync(snapshot: &Snapshot) -> Option<(OffsetDateTime, &str)> {
    snapshot
        .devices
        .iter()
        .filter(|d| !d.revoked)
        .flat_map(|d| {
            [d.last_push_at.as_deref(), d.last_pull_at.as_deref()]
                .into_iter()
                .flatten()
                .filter_map(time::parse)
                .map(move |at| (at, d.name.as_str()))
        })
        .max_by_key(|(at, _)| *at)
}

/// Titulo e subtitulo do cabecalho da interface GPUI, exatamente como nos artboards (spec §5).
/// O "Parcial" nao entra: e um chip a parte. Fica separado de `titles`, que a iced ainda usa.
pub fn header_texts(app: &App) -> (&'static str, &'static str) {
    match app.screen {
        Screen::Dashboard => (
            "Dashboard",
            // No boot o banco ainda nao abriu: "dados do ultimo sync" seria falso.
            if app.snapshot.status == Status::Starting && app.dashboard.is_empty {
                "Resumo da casa · Somente leitura"
            } else {
                "Resumo da casa · Somente leitura · dados do último sync"
            },
        ),
        Screen::Transactions => (
            "Lançamentos",
            "Tudo o que entrou e saiu no mês · Somente leitura",
        ),
        Screen::Reserves => ("Reservas", "Dinheiro separado da casa · Somente leitura"),
        Screen::Analysis => (
            "Análise",
            "Onde dá para melhorar? · calculado neste computador",
        ),
        Screen::Connection => ("Conexão", "Pareie aparelhos e acompanhe o hub"),
        Screen::Settings => ("Ajustes", "Preferências deste computador"),
    }
}

/// Telas com seletor de mes no cabecalho (as que leem um mes escolhido).
pub fn has_month_picker(screen: Screen) -> bool {
    matches!(
        screen,
        Screen::Dashboard | Screen::Transactions | Screen::Reserves | Screen::Analysis
    )
}

/// O mes escolhido existe depois dele? O "›" do seletor fica desabilitado no corrente.
pub fn can_go_next_month(app: &App) -> bool {
    app.month_idx + 1 < app.months.len()
}

/// Abaixo desta largura a lateral vira o trilho de icones de 76 px (spec §4.3).
pub const RAIL_BELOW: f32 = 1024.0;

pub fn is_rail(width: f32) -> bool {
    width < RAIL_BELOW
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::snapshot::Snapshot;
    use crate::ui_model::app::tests::app_with;

    #[test]
    fn marca_mostra_a_versao_numa_linha_curta() {
        let subtitle = brand_subtitle();
        assert!(subtitle.starts_with("Hub de casa · v"), "{subtitle}");
        assert!(subtitle.contains(crate::config::VERSION), "{subtitle}");
        assert!(!subtitle.contains("somente leitura"), "{subtitle}");
        // Cabe numa linha de 11 px nos ~174 px uteis da lateral.
        assert!(subtitle.chars().count() <= 24, "{subtitle}");
    }

    #[test]
    fn milhar_com_ponto() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1234), "1.234");
        assert_eq!(thousands(1_234_567), "1.234.567");
        assert_eq!(plural(1, "pessoa", "pessoas"), "1 pessoa");
        assert_eq!(plural(2, "pessoa", "pessoas"), "2 pessoas");
    }

    #[test]
    fn tooltip_da_nav_mostra_o_atalho_da_posicao() {
        assert_eq!(nav_shortcut(1), "Ctrl+1");
        assert_eq!(nav_shortcut(6), "Ctrl+6");
    }

    #[test]
    fn estado_da_tela_sem_dados_por_status_aparelho_e_hub() {
        let ana = ["Pixel da Ana"];
        let at = |status: Status, data: bool, rows: bool, devices: &[&str], synced: bool| {
            arrival(&status, data, rows, devices, synced)
        };
        let waiting = Arrival::Waiting("Aguardando o primeiro sync de Pixel da Ana".into());
        // Com dados na tela, dados, ate no boot.
        assert_eq!(at(Status::Starting, true, true, &ana, true), Arrival::Data);
        assert_eq!(at(Status::Running, true, true, &[], false), Arrival::Data);
        // Boot: abrindo, mesmo sem aparelho no snapshot inicial.
        assert_eq!(
            at(Status::Starting, false, false, &[], false),
            Arrival::Opening
        );
        // Nucleo no ar, hub vazio e nenhum aparelho: primeiro uso (tambem sem rede).
        assert_eq!(
            at(Status::Running, false, false, &[], false),
            Arrival::FirstUse
        );
        assert_eq!(
            at(Status::NoNetwork, false, false, &[], false),
            Arrival::FirstUse
        );
        // Aparelho pareado que nunca sincronizou: aguardando (tambem sem rede).
        assert_eq!(at(Status::Running, false, false, &ana, false), waiting);
        assert_eq!(at(Status::NoNetwork, false, false, &ana, false), waiting);
        // I1: o aparelho ja sincronizou (perfil, categorias), so nao ha lancamento nem reserva:
        // nada de "Aguardando o primeiro sync".
        assert_eq!(
            at(Status::Running, false, false, &ana, true),
            Arrival::Empty
        );
        assert_eq!(
            at(Status::NoNetwork, false, false, &ana, true),
            Arrival::Empty
        );
        // O hub tem linhas (so reservas, outra conta): o vazio de sempre, sem prometer sync.
        assert_eq!(
            at(Status::Running, false, true, &ana, false),
            Arrival::Empty
        );
        assert_eq!(at(Status::Running, false, true, &[], false), Arrival::Empty);
        // Desligando, desligado ou com erro: nem passos, nem espera, nem esqueleto.
        assert_eq!(
            at(Status::Stopped, false, false, &[], false),
            Arrival::Empty
        );
        assert_eq!(
            at(Status::Stopping, false, false, &ana, false),
            Arrival::Empty
        );
        let failed = || Status::Failed("Porta 7777 em uso".into());
        assert_eq!(at(failed(), false, false, &ana, false), Arrival::Empty);
        assert_eq!(at(failed(), false, false, &[], false), Arrival::Empty);
    }

    #[test]
    fn texto_do_aguardando_com_um_e_com_varios_aparelhos() {
        assert_eq!(
            waiting_line(&["Pixel da Ana"]),
            "Aguardando o primeiro sync de Pixel da Ana"
        );
        assert_eq!(
            waiting_line(&["Pixel da Ana", "iPhone do Luiz"]),
            "Aguardando o primeiro sync de 2 celulares"
        );
        assert_eq!(waiting_line(&[]), "Aguardando o primeiro sync");
    }

    fn device(
        name: &str,
        push: Option<&str>,
        pull: Option<&str>,
        revoked: bool,
    ) -> crate::hub::snapshot::DeviceView {
        crate::hub::snapshot::DeviceView {
            device_id: format!("ID-{name}"),
            name: name.into(),
            paired_at: "2026-09-01T10:00:00Z".into(),
            last_seen_at: None,
            last_push_at: push.map(Into::into),
            last_pull_at: pull.map(Into::into),
            revoked,
            user_id: None,
            ..crate::hub::snapshot::DeviceView::default()
        }
    }

    #[test]
    fn ultimo_sync_e_o_maior_entre_envio_e_recebimento() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        assert_eq!(last_sync_line(&app), "Nenhum sync ainda");
        app.snapshot.devices = vec![
            device("iPhone do Luiz", Some("2026-09-01T10:00:00Z"), None, false),
            device("Pixel da Ana", None, Some("2026-09-02T10:00:00Z"), false),
        ];
        assert!(
            last_sync_line(&app).ends_with("· Pixel da Ana"),
            "{}",
            last_sync_line(&app)
        );
    }

    #[test]
    fn contagem_de_aparelhos_ignora_revogados() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        app.snapshot.devices = vec![
            device("A", None, None, false),
            device("B", None, None, true),
        ];
        assert_eq!(active_devices(&app), 1);
    }

    #[test]
    fn subtitulo_dos_lancamentos_e_a_pergunta_do_mes() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = crate::ui_model::app::tests::app_with_fixtures(dir.path());
        app.screen = Screen::Transactions;
        assert_eq!(titles(&app).1, "O que entrou e saiu em setembro?");
        // A pergunta acompanha o mes escolhido.
        let _ = crate::ui_model::app::update(&mut app, Msg::PrevMonth);
        assert_eq!(titles(&app).1, "O que entrou e saiu em agosto?");
        app.screen = Screen::Dashboard;
        assert_eq!(titles(&app).1, "Somente leitura · dados do último sync");
    }

    #[test]
    fn subtitulos_das_reservas() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        app.screen = Screen::Reserves;
        // Boot: a pergunta da tela, nunca o vazio.
        assert_eq!(titles(&app).1, "Quanto está guardado, e para quê?");
        app.snapshot.status = Status::Running;
        assert_eq!(
            titles(&app),
            ("Reservas", "Nada sincronizado ainda".to_string())
        );
        app.screen = Screen::Transactions;
        assert_eq!(titles(&app).1, "Nada sincronizado ainda");
        app.snapshot.status = Status::Starting;
        assert!(titles(&app).1.starts_with("O que entrou e saiu em "));
        let (mut app, _rx, _tx) = crate::ui_model::app::tests::app_with_reserves(dir.path());
        app.screen = Screen::Reserves;
        assert_eq!(
            titles(&app),
            ("Reservas", "Quanto está guardado, e para quê?".to_string())
        );
        assert_eq!(app.dashboard.sidebar.reserves, 6);
    }

    #[test]
    fn chip_parcial_so_no_mes_corrente_com_dados() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = crate::ui_model::app::tests::app_with_fixtures(dir.path());
        assert_eq!(partial_chip(&app), Some("Parcial · até 24 set"));
        assert_eq!(app.dashboard.header.month_label, "Set 2026");
        app.screen = Screen::Transactions;
        assert_eq!(partial_chip(&app), Some("Parcial · até 24 set"));
        // Reservas e Aparelhos nao tem seletor de mes.
        app.screen = Screen::Reserves;
        assert_eq!(partial_chip(&app), None);
        app.screen = Screen::Connection;
        assert_eq!(partial_chip(&app), None);

        app.screen = Screen::Dashboard;
        let _ = crate::ui_model::app::update(&mut app, Msg::PrevMonth);
        assert_eq!(partial_chip(&app), None);
        assert_eq!(app.dashboard.header.month_label, "Ago 2026");

        let (mut empty, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        empty.set_today("2026-09-24".into());
        assert_eq!(partial_chip(&empty), None);
    }

    #[test]
    fn parcial_vira_a_linha_do_cabecalho() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = crate::ui_model::app::tests::app_with_fixtures(dir.path());
        assert_eq!(
            header_line(&app),
            ("Resumo da casa", "Parcial · até 24 set".to_string())
        );
        app.screen = Screen::Transactions;
        assert_eq!(
            header_line(&app),
            (
                "Lançamentos",
                "Parcial · até 24 set · 1 aparelho".to_string()
            )
        );
        app.transactions.synced_devices = 2;
        assert_eq!(header_line(&app).1, "Parcial · até 24 set · 2 aparelhos");
        // Nenhum aparelho conhecido: sem o "· 0 aparelhos".
        app.transactions.synced_devices = 0;
        assert_eq!(header_line(&app).1, "Parcial · até 24 set");
        // Mes fechado: volta o subtitulo.
        let _ = crate::ui_model::app::update(&mut app, Msg::PrevMonth);
        assert_eq!(header_line(&app), titles(&app));
        // Telas sem seletor de mes: sempre o subtitulo.
        for screen in [
            Screen::Reserves,
            Screen::Analysis,
            Screen::Connection,
            Screen::Settings,
        ] {
            app.screen = screen;
            assert_eq!(header_line(&app), titles(&app));
        }
    }

    #[test]
    fn subtitulos() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        // No boot o vazio nao e verdade ainda: a linha nao diz "Nada sincronizado", nem promete
        // "dados do último sync" antes de o banco abrir.
        assert_eq!(titles(&app).1, "Somente leitura");
        app.snapshot.status = Status::Running;
        assert_eq!(titles(&app).1, "Nada sincronizado ainda");
        app.screen = Screen::Connection;
        assert_eq!(
            titles(&app),
            ("Aparelhos", "Quem sincroniza com esta casa?".to_string())
        );
    }

    #[test]
    fn titulo_do_dashboard_e_resumo_da_casa_e_a_nav_diz_resumo() {
        let dir = tempfile::tempdir().unwrap();
        let (app, _rx, _tx) = crate::ui_model::app::tests::app_with_fixtures(dir.path());
        assert_eq!(
            titles(&app),
            (
                "Resumo da casa",
                "Somente leitura · dados do último sync".to_string()
            )
        );
        let snap = Snapshot {
            status: Status::Running,
            ..Snapshot::default()
        };
        let (empty, _rx, _tx) = app_with(snap, dir.path().to_path_buf());
        assert_eq!(
            titles(&empty),
            ("Resumo da casa", "Nada sincronizado ainda".to_string())
        );
    }

    #[test]
    fn hub_com_erro_cobre_as_telas_menos_ajustes() {
        let dir = tempfile::tempdir().unwrap();
        let snap = Snapshot {
            status: Status::Failed("Porta 7777 em uso".into()),
            ..Snapshot::default()
        };
        let (mut app, _rx, _tx) = app_with(snap, dir.path().to_path_buf());
        for screen in NAV_ORDER {
            app.screen = screen;
            let want = (screen != Screen::Settings).then_some("Porta 7777 em uso");
            assert_eq!(failure(&app), want, "{screen:?}");
        }
        app.snapshot.status = Status::Running;
        app.screen = Screen::Dashboard;
        assert_eq!(failure(&app), None);
    }

    #[test]
    fn lancamentos_e_analise_rolam_por_conta_propria_e_o_erro_devolve_a_rolagem_geral() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = crate::ui_model::app::tests::app_with_fixtures(dir.path());
        for screen in NAV_ORDER {
            app.screen = screen;
            assert_eq!(
                owns_scroll(&app),
                matches!(screen, Screen::Transactions | Screen::Analysis),
                "{screen:?}"
            );
        }
        // Hub em erro: o card de falha entra no lugar e precisa da rolagem da casca.
        app.screen = Screen::Transactions;
        app.snapshot.status = Status::Failed("Porta 7777 em uso".into());
        assert!(!owns_scroll(&app));
    }

    #[test]
    fn so_reservas_liga_o_status_mas_nao_o_dashboard() {
        use crate::dashboard::dataset::{Dataset, RESERVES, RawRow};
        let dir = tempfile::tempdir().unwrap();
        let mut ds = Dataset::default();
        ds.apply([RawRow {
            table: RESERVES.into(),
            id: "RP1".into(),
            deleted_at: None,
            seq: 1,
            data: serde_json::json!({ "kind": "goal", "name": "X", "icon": "tag", "color": "sky" })
                .to_string(),
        }]);
        let snap = Snapshot {
            status: Status::Running,
            dashboard: std::sync::Arc::new(ds),
            ..Snapshot::default()
        };
        let (mut app, _rx, _tx) = app_with(snap, dir.path().to_path_buf());
        app.screen = Screen::Dashboard;
        assert_eq!(titles(&app).1, "Nada sincronizado ainda");
        app.screen = Screen::Reserves;
        assert_eq!(titles(&app).1, "Quanto está guardado, e para quê?");
    }

    #[test]
    fn nav_sem_contagens_exceto_os_alertas_da_analise() {
        use crate::advisor::{Finding, FindingKind};
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = crate::ui_model::app::tests::app_with_fixtures(dir.path());
        // Com dois aparelhos ativos, a conta de aparelhos e a dos ativos.
        assert_eq!(active_devices(&app), 2);
        let finding = |severity| Finding {
            kind: FindingKind::Pace,
            severity,
            title: "x".into(),
            detail: String::new(),
            action: None,
            figures: Vec::new(),
            impact_minor: 0,
        };
        // A Analise conta so os alertas; zero nao mostra nada.
        app.analysis.findings = vec![finding(Severity::Attention), finding(Severity::GoodSign)];
        assert_eq!(alert_count(&app), None);
        app.analysis.findings = vec![
            finding(Severity::Alert),
            finding(Severity::Attention),
            finding(Severity::Alert),
        ];
        assert_eq!(alert_count(&app), Some(2));
    }

    #[test]
    fn card_de_status_leva_a_aparelhos_e_a_lateral_monta_em_toda_tela() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = crate::ui_model::app::tests::app_with_fixtures(dir.path());
        assert_eq!(STATUS_CARD_TARGET, Screen::Connection);
        assert!(matches!(
            status_card_press(),
            Msg::SelectScreen(STATUS_CARD_TARGET)
        ));
        app.screen = Screen::Dashboard;
        let _ = crate::ui_model::app::update(&mut app, status_card_press());
        assert_eq!(app.screen, STATUS_CARD_TARGET);
        assert_eq!(titles(&app).0, "Aparelhos");
    }

    #[test]
    fn estados_sem_dados_escolhidos_pelo_app_e_montados_nas_tres_telas() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        assert_eq!(screen_arrival(&app, false), Arrival::Opening);
        app.snapshot.status = Status::Running;
        assert_eq!(screen_arrival(&app, false), Arrival::FirstUse);
        // Revogado nao conta; com um ativo, o nome dele.
        app.snapshot.devices = vec![
            device("Pixel da Ana", None, None, false),
            device("Velho", None, None, true),
        ];
        assert_eq!(
            screen_arrival(&app, false),
            Arrival::Waiting("Aguardando o primeiro sync de Pixel da Ana".into())
        );
        app.snapshot
            .devices
            .push(device("iPhone do Luiz", None, None, false));
        assert_eq!(
            screen_arrival(&app, false),
            Arrival::Waiting("Aguardando o primeiro sync de 2 celulares".into())
        );
        // I1: um deles ja sincronizou (so perfil e categorias): o vazio de sempre.
        app.snapshot.devices.push(device(
            "Moto da Bia",
            Some("2026-09-24T10:00:00Z"),
            None,
            false,
        ));
        assert!(any_active_synced(&app));
        assert_eq!(screen_arrival(&app, false), Arrival::Empty);
        // Revogado que sincronizou nao conta.
        if let Some(d) = app.snapshot.devices.last_mut() {
            d.revoked = true;
        }
        assert!(!any_active_synced(&app));
        // Com dados: o Resumo do dono, intocado.
        let (app, _rx, _tx) = crate::ui_model::app::tests::app_with_fixtures(dir.path());
        assert_eq!(screen_arrival(&app, !app.dashboard.is_empty), Arrival::Data);
        assert!(hub_has_rows(&app));
    }

    // ---- Card de status (spec §5): um teste por linha da tabela ----

    fn card_of(snapshot: Snapshot) -> StatusCard {
        status_card(&snapshot, time::parse("2026-11-27T15:30:00Z").unwrap())
    }

    fn running_with_data() -> Snapshot {
        Snapshot {
            status: Status::Running,
            addresses: vec!["192.168.0.12".parse().unwrap()],
            https_port: 7777,
            dashboard: std::sync::Arc::new(crate::dashboard::fixtures_grafite()),
            devices: vec![
                device("Pixel 8 da Ana", Some("2026-11-27T15:26:00Z"), None, false),
                device("Velho", Some("2026-11-27T10:00:00Z"), None, false),
            ],
            ..Snapshot::default()
        }
    }

    #[test]
    fn card_rodando_com_dados_traz_endereco_sync_e_aparelho() {
        let card = card_of(running_with_data());
        assert_eq!(card.icon, StatusIcon::Ok);
        assert_eq!(card.title, "Hub ligado");
        // A porta e a do snapshot (7777), nunca o 8443 do design.
        assert_eq!(
            card.lines,
            [
                "192.168.0.12:7777",
                "Último sync há 4 min",
                "Pixel 8 da Ana"
            ]
        );
        assert_eq!(card.action, None);
    }

    #[test]
    fn card_do_sync_ignora_aparelho_removido() {
        let mut snap = running_with_data();
        snap.devices
            .push(device("Removido", Some("2026-11-27T15:29:00Z"), None, true));
        assert_eq!(card_of(snap).lines[2], "Pixel 8 da Ana");
    }

    #[test]
    fn card_com_dados_e_sem_carimbo_nao_inventa_sync() {
        let mut snap = running_with_data();
        snap.devices.clear();
        assert_eq!(card_of(snap).lines, ["192.168.0.12:7777"]);
    }

    #[test]
    fn card_rodando_sem_dados_espera_o_primeiro_sync() {
        let card = card_of(Snapshot {
            status: Status::Running,
            addresses: vec!["192.168.0.12".parse().unwrap()],
            https_port: 7777,
            ..Snapshot::default()
        });
        // Check contornado, nao o disco cheio do estado com dados.
        assert_eq!(card.icon, StatusIcon::OkEmpty);
        assert_eq!(card.title, "Hub ligado");
        assert_eq!(card.lines, ["Sem dados · aguardando o primeiro sync"]);
    }

    #[test]
    fn card_sem_rede_pede_wifi() {
        let card = card_of(Snapshot {
            status: Status::NoNetwork,
            ..Snapshot::default()
        });
        assert_eq!(card.icon, StatusIcon::Warning);
        assert_eq!(card.title, "Sem rede local");
        assert_eq!(card.lines, ["Conecte o computador a um Wi-Fi"]);
    }

    #[test]
    fn card_iniciando_diz_a_porta_do_snapshot() {
        let card = card_of(Snapshot {
            status: Status::Starting,
            https_port: 7777,
            ..Snapshot::default()
        });
        assert_eq!(card.icon, StatusIcon::Spinner);
        assert_eq!(card.title, "Iniciando…");
        assert_eq!(card.lines, ["Abrindo a porta 7777"]);
        // Antes de o servidor bindar a porta ainda e 0: nada de "porta 0".
        let boot = card_of(Snapshot::default());
        assert_eq!(boot.lines, ["Abrindo a porta de rede"]);
    }

    #[test]
    fn card_desligando_e_parado_salvam_os_dados() {
        for status in [Status::Stopping, Status::Stopped] {
            let card = card_of(Snapshot {
                status,
                ..Snapshot::default()
            });
            assert_eq!(card.icon, StatusIcon::Stopping);
            assert_eq!(card.title, "Desligando…");
            assert_eq!(card.lines, ["Salvando os dados"]);
        }
    }

    #[test]
    fn cada_estado_do_card_tem_forma_propria() {
        let icon = |status, rows: bool| {
            let mut snap = Snapshot {
                status,
                ..Snapshot::default()
            };
            if rows {
                snap = running_with_data();
            }
            card_of(snap).icon
        };
        let shapes = [
            icon(Status::Running, true),
            icon(Status::Running, false),
            icon(Status::NoNetwork, false),
            icon(Status::Starting, false),
            icon(Status::Stopping, false),
            icon(Status::Failed("x".into()), false),
        ];
        for (i, a) in shapes.iter().enumerate() {
            for b in &shapes[i + 1..] {
                assert_ne!(a, b, "forma + cor, nunca so cor");
            }
        }
    }

    #[test]
    fn card_com_erro_mostra_a_mensagem_e_ver_detalhes() {
        let card = card_of(Snapshot {
            status: Status::Failed("Porta 7777 em uso por outro programa".into()),
            ..Snapshot::default()
        });
        assert_eq!(card.icon, StatusIcon::Danger);
        assert_eq!(card.title, "Hub com erro");
        assert_eq!(card.lines, ["Porta 7777 em uso por outro programa"]);
        let action = card.action.expect("acao");
        assert_eq!(action.label(), "Ver detalhes");
        assert!(matches!(action.msg(), Msg::SelectScreen(Screen::Settings)));
    }

    #[test]
    fn cabecalho_usa_os_textos_do_design() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        let expected = [
            (
                Screen::Transactions,
                "Lançamentos",
                "Tudo o que entrou e saiu no mês · Somente leitura",
            ),
            (
                Screen::Reserves,
                "Reservas",
                "Dinheiro separado da casa · Somente leitura",
            ),
            (
                Screen::Analysis,
                "Análise",
                "Onde dá para melhorar? · calculado neste computador",
            ),
            (
                Screen::Connection,
                "Conexão",
                "Pareie aparelhos e acompanhe o hub",
            ),
            (Screen::Settings, "Ajustes", "Preferências deste computador"),
        ];
        for (screen, title, subtitle) in expected {
            app.screen = screen;
            assert_eq!(header_texts(&app), (title, subtitle));
        }
        app.screen = Screen::Dashboard;
        // No boot ainda nao ha "ultimo sync" a citar.
        assert_eq!(
            header_texts(&app),
            ("Dashboard", "Resumo da casa · Somente leitura")
        );
        app.snapshot.status = Status::Running;
        assert_eq!(
            header_texts(&app),
            (
                "Dashboard",
                "Resumo da casa · Somente leitura · dados do último sync"
            )
        );
    }

    #[test]
    fn seletor_de_mes_so_nas_telas_de_mes_e_proximo_so_antes_do_corrente() {
        assert!(has_month_picker(Screen::Dashboard));
        assert!(has_month_picker(Screen::Reserves));
        assert!(!has_month_picker(Screen::Connection));
        assert!(!has_month_picker(Screen::Settings));
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        assert!(!can_go_next_month(&app));
        let _ = crate::ui_model::app::update(&mut app, Msg::PrevMonth);
        assert!(can_go_next_month(&app));
    }

    #[test]
    fn trilho_abaixo_de_1024() {
        assert!(is_rail(1023.0));
        assert!(!is_rail(1024.0));
    }
}
