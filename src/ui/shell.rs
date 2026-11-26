//! Casca da janela: barra de titulo propria, moldura de redimensionar, barra lateral com o
//! card de status, e a area de conteudo rolavel. O tema mora em Ajustes.

use std::net::Ipv4Addr;

use iced::widget::{
    Space, button, column, container, mouse_area, opaque, row, scrollable, stack, text, tooltip,
};
use iced::{Alignment, Element, Length, Padding, mouse, window};

use super::app::{App, Message, Screen};
use super::theme::{self, ButtonKind, ButtonSize, Tokens};
use super::widgets::{SegmentedSize, SegmentedVariant};
use super::{
    analysis_view, connection_view, dashboard_view, fonts, icons, remove_view, reserves_view,
    settings_view, time, transactions_view, widgets,
};
use crate::advisor::Severity;
use crate::dashboard::contract::Book;
use crate::dashboard::money::group_thousands;
use crate::hub::snapshot::Status;

/// Id da rolagem do conteudo: sem ele o iced guarda a posicao pela arvore e a tela nova abria
/// no meio, onde a anterior estava. `SelectScreen` volta este id ao topo.
pub(super) const CONTENT_SCROLL: &str = "conteudo";

const TITLEBAR_HEIGHT: f32 = 36.0;
const SIDEBAR_WIDTH: f32 = 232.0;
/// Espessura da borda que redimensiona; os cantos pegam 12 px ao longo de cada lado.
const EDGE: f32 = 6.0;
const CORNER: f32 = 12.0;

pub fn root(app: &App) -> Element<'_, Message> {
    let mut layers = stack![frame(app)].width(Length::Fill).height(Length::Fill);
    if !app.maximized {
        layers = layers.push(resize_frame());
    }
    if let Some(dialog) = remove_view::dialog(app) {
        layers = layers.push(dialog);
    }
    if let Some(preview) = analysis_view::envelope_dialog(app) {
        layers = layers.push(preview);
    }
    if let Some(pending) = &app.pending_removal {
        layers = layers.push(toast(app.tokens, &pending.toast));
    }
    layers.into()
}

/// Toast de 40 px centralizado embaixo, com "Desfazer".
fn toast<'a>(t: &'static Tokens, message: &'a str) -> Element<'a, Message> {
    let undo = widgets::action_button(t, ButtonKind::Ghost, ButtonSize::Compact, "Desfazer")
        .on_press(Message::UndoRemove);
    let content = row![
        icons::icon(icons::CHECK_CIRCLE, 14.0, t.income_fg),
        text(message).size(13).color(t.text),
        undo,
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    let toast_box = container(content)
        .height(40)
        .padding(Padding {
            top: 0.0,
            right: 8.0,
            bottom: 0.0,
            left: 14.0,
        })
        .align_y(Alignment::Center)
        .style(theme::toast(t));
    container(opaque(toast_box))
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::End)
        .padding(Padding {
            bottom: 24.0,
            ..Padding::ZERO
        })
        .into()
}

fn frame(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    // Lateral e conteudo no mesmo fundo, sem divisoria: no design a navegacao fica sobre o
    // fundo da pagina. Um container so para os dois leva o gradiente.
    let body = container(row![sidebar(app), content(app)].height(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(theme::page(t));
    container(column![titlebar(app), widgets::divider(t), body])
        .width(Length::Fill)
        .height(Length::Fill)
        .style(theme::fill(t.bg, 0.0))
        .into()
}

// ---- barra de titulo ----

fn titlebar(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let mark = container(icons::icon(icons::HOUSE_LINE, 11.0, t.accent_400))
        .center(18)
        .style(theme::fill(t.accent_900, 5.0));
    let brand = row![mark, text("HubFinance").size(12).color(t.text_soft()),]
        .spacing(8)
        .align_y(Alignment::Center);
    let grip = mouse_area(
        container(brand)
            .padding([0, 14])
            .width(Length::Fill)
            .height(TITLEBAR_HEIGHT)
            .align_y(Alignment::Center),
    )
    .on_press(Message::TitlePressed)
    .on_release(Message::TitleReleased)
    .on_move(|_| Message::TitleMoved)
    .on_double_click(Message::ToggleMaximize);

    // O icone pega a cor do texto do botao (que muda no hover do fechar): sem cor propria.
    let controls = row![
        titlebar_control(t, icons::MINUS, 14.0, false, Message::Minimize),
        titlebar_control(t, icons::SQUARE, 12.0, false, Message::ToggleMaximize),
        titlebar_control(t, icons::X, 14.0, true, Message::CloseRequested),
    ];
    container(row![grip, controls].align_y(Alignment::Center))
        .width(Length::Fill)
        .height(TITLEBAR_HEIGHT)
        .style(theme::fill(t.side(), 0.0))
        .into()
}

fn titlebar_control<'a>(
    t: &'static Tokens,
    glyph: &'static str,
    size: f32,
    close: bool,
    message: Message,
) -> Element<'a, Message> {
    button(
        container(
            text(glyph)
                .font(fonts::PHOSPHOR)
                .size(size)
                .line_height(1.0),
        )
        .center(Length::Fill),
    )
    .width(46)
    .height(TITLEBAR_HEIGHT)
    .padding(0)
    .style(theme::titlebar_button(t, close))
    .on_press(message)
    .into()
}

// ---- moldura de redimensionar ----

/// Bordas de 6 px (cantos de 12 px) sobre a janela inteira; o centro e um `Space`, que nao
/// tem interacao e por isso deixa os eventos passarem para a camada de baixo.
fn resize_frame<'a>() -> Element<'a, Message> {
    use window::Direction as D;
    let grip = |w: Length, h: Length, direction: D, cursor: mouse::Interaction| {
        mouse_area(Space::new().width(w).height(h))
            .interaction(cursor)
            .on_press(Message::Resize(direction))
    };
    let diag_down = mouse::Interaction::ResizingDiagonallyDown;
    let diag_up = mouse::Interaction::ResizingDiagonallyUp;
    let horizontal = mouse::Interaction::ResizingHorizontally;
    let vertical = mouse::Interaction::ResizingVertically;
    let corner = Length::Fixed(CORNER);
    let edge = Length::Fixed(EDGE);
    let top = row![
        grip(corner, edge, D::NorthWest, diag_down),
        grip(Length::Fill, edge, D::North, vertical),
        grip(corner, edge, D::NorthEast, diag_up),
    ];
    let middle = row![
        column![
            grip(edge, corner, D::NorthWest, diag_down),
            grip(edge, Length::Fill, D::West, horizontal),
            grip(edge, corner, D::SouthWest, diag_up),
        ],
        Space::new().width(Length::Fill).height(Length::Fill),
        column![
            grip(edge, corner, D::NorthEast, diag_up),
            grip(edge, Length::Fill, D::East, horizontal),
            grip(edge, corner, D::SouthEast, diag_down),
        ],
    ]
    .height(Length::Fill);
    let bottom = row![
        grip(corner, edge, D::SouthWest, diag_up),
        grip(Length::Fill, edge, D::South, vertical),
        grip(corner, edge, D::SouthEast, diag_down),
    ];
    column![top, middle, bottom]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

// ---- barra lateral ----

/// "Somente leitura" ja esta no subtitulo do Dashboard e no card "Sobre"; a versao precisa de um
/// lugar permanente, e o subtitulo curto deixa de quebrar em duas linhas.
pub(super) fn brand_subtitle() -> String {
    format!("Hub de casa · v{}", crate::config::VERSION)
}

fn sidebar(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let mark = container(icons::icon(icons::HOUSE_LINE, 20.0, t.accent_400))
        .center(36)
        .style(theme::fill(t.accent_900, 10.0));
    let brand = container(
        row![
            mark,
            column![
                text("HubFinance")
                    .size(15)
                    .font(fonts::INTER_MEDIUM)
                    .line_height(1.2)
                    .color(t.text),
                text(brand_subtitle()).size(11).color(t.text_muted),
            ]
            .spacing(2),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .padding([0, 10]);

    let mut nav = column![].spacing(2);
    for (i, entry) in nav_entries(app).into_iter().enumerate() {
        if entry.screen == Screen::Connection {
            nav = nav.push(
                container(text("Hub").size(12).color(t.text_muted)).padding(Padding {
                    top: 18.0,
                    right: 12.0,
                    bottom: 6.0,
                    left: 12.0,
                }),
            );
        }
        nav = nav.push(nav_item(app, entry, i + 1));
    }

    let col = column![
        brand,
        nav,
        Space::new().height(Length::Fill),
        status_card(app),
    ]
    .spacing(28)
    .padding([28, 14])
    .width(SIDEBAR_WIDTH)
    .height(Length::Fill);

    container(col).height(Length::Fill).into()
}

/// Uma linha da navegacao: tela, glifo, rotulo e, so na Analise, o numero de alertas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct NavEntry {
    pub screen: Screen,
    pub glyph: &'static str,
    pub label: &'static str,
    pub count: Option<usize>,
}

/// As seis linhas na ordem do design; "Aparelhos" e "Ajustes" ficam sob a secao "Hub". A
/// ordem e tambem a do Ctrl+1..6 (`keys::shortcut` le daqui) e a do tooltip de cada item.
///
/// Sem contagens (decisao do dono, 25/11): o total de lancamentos ou de aparelhos nao pede
/// nada a ninguem. A excecao e a Analise, com o numero de alertas, que e o que pede acao.
pub(super) fn nav_entries(app: &App) -> [NavEntry; 6] {
    [
        NavEntry {
            screen: Screen::Dashboard,
            glyph: icons::CHART_PIE_SLICE,
            label: "Resumo",
            count: None,
        },
        NavEntry {
            screen: Screen::Transactions,
            glyph: icons::LIST_BULLETS,
            label: "Lançamentos",
            count: None,
        },
        NavEntry {
            screen: Screen::Reserves,
            glyph: icons::VAULT,
            label: "Reservas",
            count: None,
        },
        NavEntry {
            screen: Screen::Analysis,
            glyph: icons::LIGHTBULB,
            label: "Análise",
            count: alert_count(app),
        },
        NavEntry {
            screen: Screen::Connection,
            glyph: icons::DEVICE_MOBILE,
            label: "Aparelhos",
            count: None,
        },
        NavEntry {
            screen: Screen::Settings,
            glyph: icons::GEAR,
            label: "Ajustes",
            count: None,
        },
    ]
}

/// Achados de severidade "alerta" da conta aberta, do mesmo modelo que a Analise desenha.
/// Zero nao aparece: um "0" na lateral seria ruido.
pub(super) fn alert_count(app: &App) -> Option<usize> {
    let n = app
        .analysis
        .findings
        .iter()
        .filter(|f| f.severity == Severity::Alert)
        .count();
    (n > 0).then_some(n)
}

/// Lancamentos vivos da conta escolhida (do modelo do Dashboard, que ja os contou).
pub(super) fn transactions_count(app: &App) -> usize {
    app.dashboard.sidebar.transactions
}

/// Aparelhos ativos: e o numero de linhas da lista "Usuarios conectados" (sem a remocao
/// pendente, que ja sumiu da tela).
pub(super) fn active_devices(app: &App) -> usize {
    let pending = app
        .pending_removal
        .as_ref()
        .and_then(super::app::PendingRemoval::device_id);
    app.snapshot
        .devices
        .iter()
        .filter(|d| !d.revoked && Some(d.device_id.as_str()) != pending)
        .count()
}

/// "Conectar um celular" dos estados vazios, so quando nenhum aparelho esta pareado: sem isso o
/// primeiro uso caia no Dashboard sem caminho para a Conexao. So navega; o hub nunca cria dado.
/// Com aparelho pareado o vazio e "aguardando sync" (ou filtro), e o botao nao ajudaria.
pub(super) fn connect_phone_button(app: &App) -> Option<Element<'_, Message>> {
    // So com o nucleo no ar: no `Starting` a lista de aparelhos ainda vem vazia do snapshot
    // inicial, e o botao piscaria no boot de quem ja tem celular pareado.
    let up = matches!(app.snapshot.status, Status::Running | Status::NoNetwork);
    (up && active_devices(app) == 0).then(|| {
        widgets::action_button(
            app.tokens,
            ButtonKind::Primary,
            ButtonSize::Regular,
            "Conectar um celular",
        )
        .on_press(Message::SelectScreen(Screen::Connection))
        .into()
    })
}

/// Texto do tooltip do item: o atalho da posicao `n` (1 a 6). Sai do mesmo indice que
/// `keys::shortcut` usa, entao nao tem como dizer um numero e a tecla fazer outro.
pub(super) fn nav_shortcut(n: usize) -> String {
    format!("Ctrl+{n}")
}

/// Item de 44 px, raio 8, icone 18 e rotulo 14. Ativo: fundo `surface`, barra dourada de 3x20
/// encostada a esquerda, icone `accent_400` e rotulo Medium; o inativo herda o texto de apoio
/// do botao. A barra tem lugar reservado nos dois estados, para o icone nao pular ao trocar.
/// O tooltip mostra o atalho (`n` e a posicao na navegacao).
fn nav_item(app: &App, entry: NavEntry, n: usize) -> Element<'_, Message> {
    let t = app.tokens;
    let active = app.screen == entry.screen;
    let icon = if active {
        icons::icon(entry.glyph, 18.0, t.accent_400)
    } else {
        icons::icon_inherit(entry.glyph, 18.0)
    };
    let marker = container(Space::new())
        .width(3)
        .height(20)
        .style(theme::fill(
            if active {
                t.accent
            } else {
                iced::Color::TRANSPARENT
            },
            1.5,
        ));
    let mut line = row![
        icon,
        text(entry.label)
            .size(14)
            .font(if active {
                fonts::INTER_MEDIUM
            } else {
                fonts::INTER
            })
            .width(Length::Fill),
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    if let Some(n) = entry.count {
        line = line.push(text(thousands(n)).size(12).color(t.text_muted));
    }
    let content = row![
        marker,
        container(line)
            .padding(Padding {
                left: 9.0,
                right: 12.0,
                ..Padding::ZERO
            })
            .width(Length::Fill),
    ]
    .height(44)
    .align_y(Alignment::Center);
    let item = button(content)
        .padding(0)
        .width(Length::Fill)
        .style(theme::nav_item(t, active))
        .on_press(Message::SelectScreen(entry.screen));
    tooltip(
        item,
        container(text(nav_shortcut(n)).size(12).color(t.text))
            .padding([6, 10])
            .style(theme::menu(t)),
        tooltip::Position::Right,
    )
    .gap(8)
    .delay(theme::TOOLTIP_DELAY)
    .into()
}

/// Endereco principal e porta HTTPS; sem rede, so a porta.
fn address(app: &App) -> Option<String> {
    app.snapshot
        .addresses
        .first()
        .map(|ip: &Ipv4Addr| format!("{ip}:{}", app.snapshot.https_port))
}

/// O que o card de status diz: cor do ponto, titulo, linha do endereco e linha de detalhe.
pub(super) struct StatusLines {
    pub dot: iced::Color,
    pub title: String,
    pub address: Option<String>,
    pub detail: Option<String>,
}

pub(super) fn status_lines(app: &App) -> StatusLines {
    let t = app.tokens;
    let snap = &app.snapshot;
    match &snap.status {
        Status::Running if has_data(app) => StatusLines {
            dot: t.income,
            title: "Hub ligado".into(),
            address: address(app),
            detail: Some(last_sync_line(app)),
        },
        Status::Running => StatusLines {
            dot: t.warning,
            title: "Hub ligado · sem dados".into(),
            address: address(app),
            detail: Some("Aguardando o primeiro sync de um celular pareado".into()),
        },
        Status::NoNetwork => StatusLines {
            dot: t.warning,
            title: "Sem rede local".into(),
            address: Some(format!("porta {} aberta", snap.https_port)),
            detail: Some("Conecte o computador ao Wi-Fi de casa".into()),
        },
        Status::Starting => StatusLines {
            dot: t.idle(),
            title: "Iniciando…".into(),
            address: None,
            detail: None,
        },
        Status::Stopping => StatusLines {
            dot: t.idle(),
            title: "Desligando…".into(),
            address: None,
            detail: None,
        },
        Status::Stopped => StatusLines {
            dot: t.idle(),
            title: "Desligado".into(),
            address: None,
            detail: None,
        },
        Status::Failed(msg) => StatusLines {
            dot: t.expense,
            title: "Hub com erro".into(),
            address: None,
            detail: Some(msg.clone()),
        },
    }
}

/// Ha lancamento ou reserva viva no hub: uma casa que so sincronizou reservas tem dados.
pub(super) fn has_data(app: &App) -> bool {
    has_transactions(app) || app.scope.has_reserves()
}

/// Ha lancamento vivo: o estado vazio do Dashboard e de Lancamentos e sobre lancamentos.
pub(super) fn has_transactions(app: &App) -> bool {
    app.dashboard.sidebar.has_data
}

/// "Último sync há 2 min · Pixel da Ana": o maior entre envio e recebimento de todos os
/// aparelhos. RFC 3339 em UTC compara como string, entao nao precisa parsear para escolher.
pub(super) fn last_sync_line(app: &App) -> String {
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

/// Ponto de 7 px com halo de 3 px na cor a 18 % (o `box-shadow: 0 0 0 3px` do design): o
/// iced nao tem sombra espalhada, entao o halo e um circulo de 13 px por baixo.
fn halo_dot<'a>(color: iced::Color) -> Element<'a, Message> {
    container(widgets::dot(color, 7.0))
        .center(13)
        .style(theme::fill(theme::alpha(color, 0.18), 6.5))
        .into()
}

/// Card de status: `surface`, raio 8, 14 de respiro; titulo 13 e as linhas em `text_muted`.
/// Clicavel: leva a Aparelhos, onde estao o endereco e quem esta conectado.
fn status_card(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let lines = status_lines(app);
    let mut col = column![
        row![
            halo_dot(lines.dot),
            text(lines.title).size(13).color(t.text)
        ]
        .spacing(5)
        .align_y(Alignment::Center)
    ]
    .spacing(5);
    if let Some(address) = lines.address {
        col = col.push(text(address).size(12).color(t.text_muted));
    }
    if let Some(detail) = lines.detail {
        col = col.push(text(detail).size(12).line_height(1.45).color(t.text_muted));
    }
    button(col)
        .padding(14)
        .width(Length::Fill)
        .style(theme::status_card(t))
        .on_press(Message::SelectScreen(Screen::Connection))
        .into()
}

// ---- conteudo ----

fn content(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let body: Element<'_, Message> = if let Status::Failed(msg) = &app.snapshot.status {
        failed_card(app, msg)
    } else {
        match app.screen {
            Screen::Dashboard => dashboard_view::view(app),
            Screen::Transactions => transactions_view::view(app),
            Screen::Reserves => reserves_view::view(app),
            Screen::Analysis => analysis_view::view(app),
            Screen::Connection => connection_view::view(app),
            Screen::Settings => settings_view::view(app),
        }
    };
    let mut page = column![header(app)].spacing(14);
    // O aviso vale para qualquer tela: na primeira abertura quem esta no dashboard tambem ve.
    if let Some(banner) = remove_view::orphan_banner(app) {
        page = page.push(banner);
    }
    let page = page.push(body);
    let inner = container(page).width(Length::Fill).padding(Padding {
        top: 28.0,
        right: 32.0,
        bottom: 40.0,
        left: 32.0,
    });
    let scroll = scrollable(inner)
        .id(CONTENT_SCROLL)
        .direction(scrollable::Direction::Vertical(
            scrollable::Scrollbar::new()
                .width(6)
                .scroller_width(6)
                .margin(3),
        ))
        .style(theme::scrollbar(t))
        .width(Length::Fill)
        .height(Length::Fill);
    // Sem fundo proprio: o fundo e o do container de `frame`, que cobre lateral e conteudo
    // (um fundo aqui esconderia o gradiente atras da rolagem).
    container(scroll)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Largura util do conteudo (sem a lateral e o padding de 32 + 32; a divisoria vertical saiu
/// com a lateral Grafite). Sem teto: com o maximo de 1180 da 0.0.2 a janela maximizada deixava
/// uma faixa vazia a direita. As telas usam para decidir colunas: o iced nao tem `auto-fit`.
pub(super) fn content_width(app: &App) -> f32 {
    app.window_size.width - SIDEBAR_WIDTH - 64.0
}

/// Abaixo desta largura util os controles do cabecalho descem para baixo do titulo (o
/// `flex-wrap` do design). Com chip, Pessoal | Comercial, mes e olho eles somam ~560 px, e o
/// titulo de 30 px precisa de ~240 ao lado; na janela minima (1024) a linha unica espremia o
/// titulo em duas linhas.
const HEADER_WRAP_WIDTH: f32 = 900.0;

/// Controles do cabecalho na mesma linha do titulo?
pub(super) fn header_inline(width: f32) -> bool {
    width >= HEADER_WRAP_WIDTH
}

/// Cabecalho: titulo e subtitulo a esquerda; a direita, conforme a tela, o chip "Parcial", o
/// seletor de conta, o de mes e (so no Dashboard) o olho de ocultar valores.
fn header(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let (title, subtitle) = titles(app);
    let mut controls = row![].spacing(10).align_y(Alignment::Center);
    if let Some(label) = partial_chip(app) {
        controls = controls.push(widgets::partial_tag(t, label));
    }
    if let Some(switch) = book_switch(app) {
        controls = controls.push(switch);
    }
    // Reservas nao tem seletor de mes: saldos sao acumulados ate hoje.
    if matches!(app.screen, Screen::Dashboard | Screen::Transactions) {
        let prev = (app.month_idx > 0).then_some(Message::PrevMonth);
        let next = (app.month_idx + 1 < app.months.len()).then_some(Message::NextMonth);
        controls = controls.push(widgets::month_picker(
            t,
            &app.dashboard.header.month_label,
            prev,
            next,
        ));
    }
    // Na Analise o olho tambem aparece: os cartoes mascaram os valores, e sem ele quem ocultou
    // no Dashboard nao teria como mostrar de novo sem voltar la.
    if matches!(app.screen, Screen::Dashboard | Screen::Analysis) {
        controls = controls.push(hide_values_button(app));
    }
    let title = widgets::page_title(t, title, &subtitle);
    let line: Element<'_, Message> = if header_inline(content_width(app)) {
        row![container(title).width(Length::Fill), controls]
            .spacing(16)
            .align_y(Alignment::Center)
            .into()
    } else {
        column![title, controls].spacing(16).into()
    };
    container(line)
        .padding(Padding {
            bottom: 4.0,
            ..Padding::ZERO
        })
        .into()
}

/// "Parcial · até 19 nov": so nas telas com seletor de mes, no mes corrente e com dados (o
/// modelo ja decide as duas ultimas).
pub(super) fn partial_chip(app: &App) -> Option<&str> {
    matches!(app.screen, Screen::Dashboard | Screen::Transactions)
        .then_some(app.dashboard.header.partial.as_deref())
        .flatten()
}

/// Olho de 44 px com tooltip "Ocultar valores" / "Mostrar valores" (o `aria-label` do design).
/// Com valores ocultos o glifo vira `eye-slash`, para o estado ficar visivel sem o tooltip.
fn hide_values_button(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let glyph = if app.hide_values {
        icons::EYE_SLASH
    } else {
        icons::EYE
    };
    widgets::icon_button(
        t,
        glyph,
        ButtonSize::Large,
        app.dashboard.header.hide_toggle.as_str(),
        Some(Message::ToggleHideValues),
    )
}

/// Seletor de conta. So aparece com uma linha `books` viva de conta comercial (derivado das
/// linhas, como no app) e fora da Conexao e de Ajustes, que sao do hub e nao de uma conta.
pub(super) fn book_switch(app: &App) -> Option<Element<'_, Message>> {
    let of_the_hub = matches!(app.screen, Screen::Connection | Screen::Settings);
    if of_the_hub || !app.snapshot.dashboard.has_business_book() {
        return None;
    }
    let option = |book: Book| (book.label(), app.book == book, Message::SelectBook(book));
    Some(widgets::segmented(
        app.tokens,
        SegmentedVariant::Pill,
        SegmentedSize::Regular,
        false,
        [option(Book::Personal), option(Book::Business)],
    ))
}

/// Titulo e subtitulo de cada tela. O do Dashboard vem pronto do modelo Grafite ("Resumo da
/// casa"); a navegacao diz so "Resumo".
pub(super) fn titles(app: &App) -> (&str, String) {
    let empty = !has_transactions(app);
    match app.screen {
        Screen::Dashboard => (
            app.dashboard.header.title.as_str(),
            app.dashboard.header.subtitle.clone(),
        ),
        Screen::Transactions => (
            "Lançamentos",
            if empty {
                "Nada sincronizado ainda".into()
            } else {
                synced_line(transactions_count(app), app.transactions.synced_devices)
            },
        ),
        Screen::Reserves => (
            "Reservas",
            if app.reserves.is_empty {
                "Nada sincronizado ainda".into()
            } else {
                "Dinheiro separado do saldo do mês · somente leitura".into()
            },
        ),
        Screen::Analysis => (
            "Análise",
            "Onde dá para melhorar · calculado no seu PC".into(),
        ),
        Screen::Connection => (
            "Aparelhos",
            "Abra este endereço no celular para conectar ao hub".into(),
        ),
        Screen::Settings => ("Ajustes", "Preferências deste computador".into()),
    }
}

/// "1 lançamento" / "1.234 lançamentos" (milhar com ponto, como o pt-BR do app).
/// "4.688 lançamentos sincronizados de 2 aparelhos". Sem aparelho conhecido (todos removidos,
/// ou linhas sem origem), sem o "de 0 aparelhos", que dizia que nada tinha vindo de lugar
/// nenhum. O particípio concorda com o numero: "1 lançamento sincronizado".
fn synced_line(rows: usize, devices: usize) -> String {
    let rows = format!(
        "{} {}",
        plural(rows, "lançamento", "lançamentos"),
        if rows == 1 {
            "sincronizado"
        } else {
            "sincronizados"
        }
    );
    if devices == 0 {
        rows
    } else {
        format!("{rows} de {}", plural(devices, "aparelho", "aparelhos"))
    }
}

pub(super) fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{} {}", thousands(n), if n == 1 { one } else { many })
}

pub(super) fn thousands(n: usize) -> String {
    group_thousands(n as u64)
}

/// Hub que nao subiu: no lugar de qualquer tela, o motivo e onde ficam os dados.
fn failed_card<'a>(app: &'a App, msg: &'a str) -> Element<'a, Message> {
    let t = app.tokens;
    widgets::card(
        t,
        column![
            icons::icon(icons::WARNING_CIRCLE, 28.0, t.expense_fg),
            text("O hub não iniciou")
                .size(16)
                .font(fonts::INTER_MEDIUM)
                .color(t.text),
            text(msg).size(13).color(t.text_muted),
            text(format!("Dados em {}", app.snapshot.data_dir.display()))
                .size(12)
                .color(t.text_muted),
        ]
        .spacing(8),
    )
    .padding(24)
    .into()
}

#[cfg(test)]
mod tests {
    use super::super::app::tests::app_with;
    use super::*;
    use crate::hub::snapshot::{DeviceView, Snapshot};

    #[test]
    fn marca_mostra_a_versao_numa_linha_curta() {
        let subtitle = brand_subtitle();
        assert!(subtitle.starts_with("Hub de casa · v"), "{subtitle}");
        assert!(subtitle.contains(crate::config::VERSION), "{subtitle}");
        assert!(!subtitle.contains("somente leitura"), "{subtitle}");
        // Cabe numa linha de 11 px nos ~174 px uteis da lateral.
        assert!(subtitle.chars().count() <= 24, "{subtitle}");
    }

    fn device(name: &str, push: Option<&str>, pull: Option<&str>, revoked: bool) -> DeviceView {
        DeviceView {
            device_id: format!("ID-{name}"),
            name: name.into(),
            paired_at: "2026-09-01T10:00:00Z".into(),
            last_seen_at: None,
            last_push_at: push.map(Into::into),
            last_pull_at: pull.map(Into::into),
            revoked,
            user_id: None,
            ..DeviceView::default()
        }
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
    fn card_de_status_por_estado() {
        let dir = tempfile::tempdir().unwrap();
        let snap = Snapshot {
            status: Status::Running,
            addresses: vec![Ipv4Addr::new(192, 168, 0, 12)],
            https_port: 7777,
            ..Snapshot::default()
        };
        let (mut app, _rx, _tx) = app_with(snap.clone(), dir.path().to_path_buf());
        let lines = status_lines(&app);
        assert_eq!(lines.title, "Hub ligado · sem dados");
        assert_eq!(lines.address.as_deref(), Some("192.168.0.12:7777"));
        assert_eq!(lines.dot, app.tokens.warning);

        app.snapshot.status = Status::NoNetwork;
        app.snapshot.addresses.clear();
        let lines = status_lines(&app);
        assert_eq!(lines.title, "Sem rede local");
        assert_eq!(lines.address.as_deref(), Some("porta 7777 aberta"));

        app.snapshot.status = Status::Failed("Porta 7777 em uso".into());
        let lines = status_lines(&app);
        assert_eq!(lines.title, "Hub com erro");
        assert_eq!(lines.detail.as_deref(), Some("Porta 7777 em uso"));
        assert_eq!(lines.dot, app.tokens.expense);

        app.snapshot.status = Status::Starting;
        assert_eq!(status_lines(&app).title, "Iniciando…");
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
    fn subtitulo_dos_lancamentos_conta_aparelhos_que_enviaram() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = super::super::app::tests::app_with_fixtures(dir.path());
        app.screen = Screen::Transactions;
        assert_eq!(titles(&app).1, "13 lançamentos sincronizados de 1 aparelho");
        app.screen = Screen::Dashboard;
        assert_eq!(titles(&app).1, "Somente leitura · dados do último sync");
    }

    #[test]
    fn linha_de_sincronizados_concorda_e_omite_zero_aparelho() {
        assert_eq!(synced_line(4688, 0), "4.688 lançamentos sincronizados");
        assert_eq!(
            synced_line(4688, 1),
            "4.688 lançamentos sincronizados de 1 aparelho"
        );
        assert_eq!(
            synced_line(4688, 3),
            "4.688 lançamentos sincronizados de 3 aparelhos"
        );
        assert_eq!(synced_line(1, 1), "1 lançamento sincronizado de 1 aparelho");
        assert_eq!(synced_line(1, 0), "1 lançamento sincronizado");
    }

    #[test]
    fn subtitulos_das_reservas() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        app.screen = Screen::Reserves;
        assert_eq!(
            titles(&app),
            ("Reservas", "Nada sincronizado ainda".to_string())
        );
        let (mut app, _rx, _tx) = super::super::app::tests::app_with_reserves(dir.path());
        app.screen = Screen::Reserves;
        assert_eq!(
            titles(&app),
            (
                "Reservas",
                "Dinheiro separado do saldo do mês · somente leitura".to_string()
            )
        );
        assert_eq!(app.dashboard.sidebar.reserves, 6);
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
        assert_eq!(status_lines(&app).title, "Hub ligado");
        app.screen = Screen::Dashboard;
        assert_eq!(titles(&app).1, "Nada sincronizado ainda");
        app.screen = Screen::Reserves;
        assert_eq!(
            titles(&app).1,
            "Dinheiro separado do saldo do mês · somente leitura"
        );
    }

    #[test]
    fn titulo_do_dashboard_e_resumo_da_casa_e_a_nav_diz_resumo() {
        let dir = tempfile::tempdir().unwrap();
        let (app, _rx, _tx) = super::super::app::tests::app_with_fixtures(dir.path());
        assert_eq!(
            titles(&app),
            (
                "Resumo da casa",
                "Somente leitura · dados do último sync".to_string()
            )
        );
        assert_eq!(nav_entries(&app)[0].label, "Resumo");
        let (empty, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        assert_eq!(
            titles(&empty),
            ("Resumo da casa", "Nada sincronizado ainda".to_string())
        );
    }

    #[test]
    fn chip_parcial_so_no_mes_corrente_com_dados() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = super::super::app::tests::app_with_fixtures(dir.path());
        assert_eq!(partial_chip(&app), Some("Parcial · até 24 set"));
        assert_eq!(app.dashboard.header.month_label, "Set 2026");
        app.screen = Screen::Transactions;
        assert_eq!(partial_chip(&app), Some("Parcial · até 24 set"));
        // Reservas e Conexao nao tem seletor de mes.
        app.screen = Screen::Reserves;
        assert_eq!(partial_chip(&app), None);
        app.screen = Screen::Connection;
        assert_eq!(partial_chip(&app), None);

        app.screen = Screen::Dashboard;
        let _ = super::super::app::update(&mut app, Message::PrevMonth);
        assert_eq!(partial_chip(&app), None);
        assert_eq!(app.dashboard.header.month_label, "Ago 2026");

        let (mut empty, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        empty.set_today("2026-09-24".into());
        assert_eq!(partial_chip(&empty), None);
    }

    #[test]
    fn status_sem_dados_usa_warning_e_com_dados_income() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = super::super::app::tests::app_with_fixtures(dir.path());
        app.snapshot.status = Status::Running;
        let lines = status_lines(&app);
        assert_eq!(lines.title, "Hub ligado");
        assert_eq!(lines.dot, app.tokens.income);

        let snap = Snapshot {
            status: Status::Running,
            ..Snapshot::default()
        };
        let (empty, _rx, _tx) = app_with(snap, dir.path().to_path_buf());
        let lines = status_lines(&empty);
        assert_eq!(lines.title, "Hub ligado · sem dados");
        assert_eq!(lines.dot, empty.tokens.warning);
        assert_ne!(lines.dot, empty.tokens.token("amber"));
    }

    #[test]
    fn conectar_um_celular_so_sem_aparelho_pareado() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        // Boot: sem aparelho no snapshot inicial, mas ainda nao se sabe.
        assert_eq!(app.snapshot.status, Status::Starting);
        assert!(connect_phone_button(&app).is_none());
        app.snapshot.status = Status::NoNetwork;
        assert!(connect_phone_button(&app).is_some());
        app.snapshot.status = Status::Running;
        assert!(connect_phone_button(&app).is_some());
        // Os vazios montam com o botao nas tres telas.
        for screen in [Screen::Dashboard, Screen::Transactions, Screen::Reserves] {
            app.screen = screen;
            let _ = root(&app);
        }
        let (mut app, _rx, _tx) = super::super::app::tests::app_with_fixtures(dir.path());
        app.snapshot.status = Status::Running;
        assert!(connect_phone_button(&app).is_none());
    }

    #[test]
    fn nav_sem_contagens_exceto_os_alertas_da_analise() {
        use crate::advisor::{Finding, FindingKind};
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = super::super::app::tests::app_with_fixtures(dir.path());
        let entries = nav_entries(&app);
        let screens: Vec<Screen> = entries.iter().map(|e| e.screen).collect();
        assert_eq!(
            screens,
            [
                Screen::Dashboard,
                Screen::Transactions,
                Screen::Reserves,
                Screen::Analysis,
                Screen::Connection,
                Screen::Settings
            ]
        );
        let labels: Vec<&str> = entries.iter().map(|e| e.label).collect();
        assert_eq!(
            labels,
            [
                "Resumo",
                "Lançamentos",
                "Reservas",
                "Análise",
                "Aparelhos",
                "Ajustes"
            ]
        );
        // Com lancamentos, reservas e dois aparelhos ativos, nenhuma contagem aparece.
        assert_eq!(app.dashboard.sidebar.transactions, 13);
        assert_eq!(active_devices(&app), 2);
        for entry in entries.iter().filter(|e| e.screen != Screen::Analysis) {
            assert_eq!(entry.count, None, "{}", entry.label);
        }

        // A Analise conta so os alertas; zero nao mostra nada.
        let finding = |severity| Finding {
            kind: FindingKind::Pace,
            severity,
            title: "x".into(),
            detail: String::new(),
            action: None,
            figures: Vec::new(),
            impact_minor: 0,
        };
        app.analysis.findings = vec![finding(Severity::Attention), finding(Severity::GoodSign)];
        assert_eq!(nav_entries(&app)[3].count, None);
        app.analysis.findings = vec![
            finding(Severity::Alert),
            finding(Severity::Attention),
            finding(Severity::Alert),
        ];
        assert_eq!(alert_count(&app), Some(2));
        assert_eq!(nav_entries(&app)[3].count, Some(2));
    }

    #[test]
    fn tooltip_da_nav_mostra_o_atalho_da_posicao() {
        assert_eq!(nav_shortcut(1), "Ctrl+1");
        assert_eq!(nav_shortcut(6), "Ctrl+6");
    }

    #[test]
    fn card_de_status_leva_a_aparelhos_e_a_lateral_monta_em_toda_tela() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = super::super::app::tests::app_with_fixtures(dir.path());
        for screen in nav_entries(&app).map(|e| e.screen) {
            app.screen = screen;
            let _ = root(&app);
        }
        app.screen = Screen::Dashboard;
        let _ = super::super::app::update(&mut app, Message::SelectScreen(Screen::Connection));
        assert_eq!(titles(&app).0, "Aparelhos");
    }

    #[test]
    fn cabecalho_quebra_abaixo_de_900_de_largura_util() {
        assert!(header_inline(1144.0)); // janela inicial de 1440
        assert!(!header_inline(1024.0 - SIDEBAR_WIDTH - 64.0)); // janela minima
        assert!(header_inline(900.0));
    }

    #[test]
    fn subtitulos() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        assert_eq!(titles(&app).1, "Nada sincronizado ainda");
        app.screen = Screen::Connection;
        assert_eq!(
            titles(&app),
            (
                "Aparelhos",
                "Abra este endereço no celular para conectar ao hub".to_string()
            )
        );
    }
}
