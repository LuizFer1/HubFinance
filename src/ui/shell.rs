//! Casca da janela: barra de titulo propria, moldura de redimensionar, barra lateral com o
//! card de status, e a area de conteudo rolavel. O tema mora em Ajustes.

use iced::widget::{
    Space, button, column, container, float, mouse_area, opaque, row, scrollable, stack, text,
    tooltip,
};
use iced::{Alignment, Element, Length, Padding, Vector, mouse};

use super::app::{Msg, Screen, State};
use super::theme::{self, ButtonKind, ButtonSize, Tokens, space};
use super::widgets::{SegmentedSize, SegmentedVariant};
use super::{
    analysis_view, assistant_settings_view, connection_view, dashboard_view, fonts, icons,
    remove_view, reserves_view, settings_view, transactions_view, widgets,
};
use crate::dashboard::contract::Book;
use crate::hub::snapshot::Status;
pub(super) use crate::ui_model::shell::failure;
use crate::ui_model::shell::{OPENING_TEXT, brand_subtitle, nav_shortcut, thousands};
pub(super) use crate::ui_model::shell::{
    active_devices, address, alert_count, has_data, header_line, last_sync_line, owns_scroll,
    screen_arrival, status_card_press, transactions_count,
};

/// Id da rolagem do conteudo: sem ele o iced guarda a posicao pela arvore e a tela nova abria
/// no meio, onde a anterior estava. `SelectScreen` volta este id ao topo.
pub(super) const CONTENT_SCROLL: &str = "conteudo";

const TITLEBAR_HEIGHT: f32 = 36.0;
const SIDEBAR_WIDTH: f32 = 232.0;
/// Espessura da borda que redimensiona; os cantos pegam 12 px ao longo de cada lado.
const EDGE: f32 = 6.0;
const CORNER: f32 = 12.0;

pub fn root(app: &State) -> Element<'_, Msg> {
    let mut layers = stack![frame(app)].width(Length::Fill).height(Length::Fill);
    if !app.maximized {
        layers = layers.push(resize_frame());
    }
    if let Some(dialog) = remove_view::dialog(app) {
        layers = layers.push(dialog);
    }
    if let Some(dialog) = connection_view::connect_dialog(app) {
        layers = layers.push(dialog);
    }
    if let Some(dialog) = assistant_settings_view::dialog(app) {
        layers = layers.push(dialog);
    }
    if let Some(dialog) = analysis_view::consent_dialog(app) {
        layers = layers.push(dialog);
    }
    // O envelope por ultimo: o dialogo de configuracao tem um link para ele, que abre por cima.
    if let Some(preview) = analysis_view::envelope_dialog(app) {
        layers = layers.push(preview);
    }
    // O "Desfazer" ganha do aviso (tem acao e prazo que manda algo ao nucleo); a escolha e
    // feita em `State::scene`, e o `motion` guarda o que desce depois de sair do estado.
    if let Some((progress, shown, live)) = app.motion.toast() {
        layers = layers.push(toast(
            app.iced.tokens,
            &shown.text,
            shown.undo,
            live,
            progress,
        ));
    }
    layers.into()
}

/// Margem do toast para a borda de baixo.
const TOAST_MARGIN: f32 = 24.0;
/// Altura do toast mais a margem: o quanto ele desce para sair inteiro da janela.
const TOAST_TRAVEL: f32 = 40.0 + TOAST_MARGIN;

/// Toast de 40 px centralizado embaixo, com "Desfazer" quando `undo`. `progress` (0 a 1) e a
/// subida: o toast fica no lugar de sempre e o `float` o desloca para baixo do resto que falta,
/// desenhado numa camada recortada pela janela, entao ele desliza da borda inteiro, sem
/// espremer. Parado no lugar (deslocamento zero) o `float` desenha normal. O que desce (`live` falso) ja saiu do estado: o "Desfazer" fica sem
/// clique, porque a remocao ja foi enviada.
fn toast<'a>(
    t: &'static Tokens,
    message: &'a str,
    undo: bool,
    live: bool,
    progress: f32,
) -> Element<'a, Msg> {
    let mut content = row![
        icons::icon(icons::CHECK_CIRCLE, 14.0, t.income_fg),
        text(message).size(13).color(t.text),
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    if undo {
        let button = widgets::action_button(t, ButtonKind::Ghost, ButtonSize::Compact, "Desfazer");
        content = content.push(if live {
            button.on_press(Msg::UndoRemove)
        } else {
            button
        });
    }
    let toast_box = container(content)
        .height(40)
        .padding(Padding {
            top: 0.0,
            right: if undo { 8.0 } else { 14.0 },
            bottom: 0.0,
            left: 14.0,
        })
        .align_y(Alignment::Center)
        .style(theme::toast(t));
    let offset = TOAST_TRAVEL * (1.0 - progress.clamp(0.0, 1.0));
    let sliding = float(opaque(toast_box)).translate(move |_, _| Vector::new(0.0, offset));
    container(sliding)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::End)
        .padding(Padding {
            bottom: TOAST_MARGIN,
            ..Padding::ZERO
        })
        .into()
}

fn frame(app: &State) -> Element<'_, Msg> {
    let t = app.iced.tokens;
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

fn titlebar(app: &State) -> Element<'_, Msg> {
    let t = app.iced.tokens;
    let mark = container(icons::icon(icons::HOUSE_LINE, 11.0, t.accent_400))
        .center(18)
        .style(theme::fill(t.accent_900, theme::radius::INSET));
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
    .on_press(Msg::TitlePressed)
    .on_release(Msg::TitleReleased)
    .on_move(|_| Msg::TitleMoved)
    .on_double_click(Msg::ToggleMaximize);

    // O icone pega a cor do texto do botao (que muda no hover do fechar): sem cor propria.
    let controls = row![
        titlebar_control(t, icons::MINUS, 14.0, false, Msg::Minimize),
        titlebar_control(t, icons::SQUARE, 12.0, false, Msg::ToggleMaximize),
        titlebar_control(t, icons::X, 14.0, true, Msg::CloseRequested),
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
    message: Msg,
) -> Element<'a, Msg> {
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
fn resize_frame<'a>() -> Element<'a, Msg> {
    use crate::ui_model::window::ResizeEdge as D;
    let grip = |w: Length, h: Length, direction: D, cursor: mouse::Interaction| {
        mouse_area(Space::new().width(w).height(h))
            .interaction(cursor)
            .on_press(Msg::Resize(direction))
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

fn sidebar(app: &State) -> Element<'_, Msg> {
    let t = app.iced.tokens;
    let mark = container(icons::icon(icons::HOUSE_LINE, 20.0, t.accent_400))
        .center(36)
        .style(theme::fill(t.accent_900, theme::radius::CONTROL));
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
/// ordem e tambem a do Ctrl+1..6 (`keys::shortcut` le `NAV_ORDER`, e o teste
/// `lateral_desenha_na_ordem_do_ctrl_numero` garante que esta lista a segue) e a do tooltip de cada item.
///
/// Sem contagens (decisao do dono, 25/11): o total de lancamentos ou de aparelhos nao pede
/// nada a ninguem. A excecao e a Analise, com o numero de alertas, que e o que pede acao.
pub(super) fn nav_entries(app: &State) -> [NavEntry; 6] {
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

/// Esqueleto do boot, o mesmo em Resumo, Lancamentos e Reservas: o texto e blocos de
/// `surface_2` no lugar das secoes (uma faixa larga, duas lado a lado na proporcao 2:1 do
/// Resumo e mais uma larga). Nao imita cada tela: dura o tempo de abrir o banco. A altura total
/// (~425: texto, 100 + 160 + 100 e tres espacos de 16) cabe nos ~470 que sobram em 1024x720 com
/// o cabecalho quebrado em duas linhas, porque em Lancamentos a casca nao rola (`owns_scroll`).
pub(super) fn opening(app: &State) -> Element<'_, Msg> {
    let t = app.iced.tokens;
    let block = |height: f32| {
        container(Space::new())
            .width(Length::Fill)
            .height(height)
            .style(theme::fill(t.surface_2, theme::radius::CARD))
    };
    column![
        text(OPENING_TEXT).size(13).color(t.text_muted),
        block(100.0),
        row![
            container(block(160.0)).width(Length::FillPortion(2)),
            container(block(160.0)).width(Length::FillPortion(1)),
        ]
        .spacing(space::BETWEEN_CARDS),
        block(100.0),
    ]
    .spacing(space::BETWEEN_CARDS)
    .width(Length::Fill)
    .into()
}

/// "Conectar um celular" dos estados vazios, so quando nenhum aparelho esta pareado: sem isso o
/// primeiro uso caia no Resumo sem caminho para Aparelhos. So navega (a Aparelhos, com o
/// fluxo de pareamento aberto); o hub nunca cria dado.
/// Com aparelho pareado o vazio e "aguardando sync" (ou filtro), e o botao nao ajudaria.
pub(super) fn connect_phone_button(app: &State) -> Option<Element<'_, Msg>> {
    // So com o nucleo no ar: no `Starting` a lista de aparelhos ainda vem vazia do snapshot
    // inicial, e o botao piscaria no boot de quem ja tem celular pareado.
    let up = matches!(app.snapshot.status, Status::Running | Status::NoNetwork);
    (up && active_devices(app) == 0).then(|| {
        widgets::action_button(
            app.iced.tokens,
            ButtonKind::Primary,
            ButtonSize::Regular,
            "Conectar um celular",
        )
        .on_press(Msg::ConnectPhone)
        .into()
    })
}

/// Item de 44 px, raio 8, icone 18 e rotulo 14. Ativo: fundo `surface`, barra dourada de 3x20
/// encostada a esquerda, icone `accent_400` e rotulo Medium; o inativo herda o texto de apoio
/// do botao. A barra tem lugar reservado nos dois estados, para o icone nao pular ao trocar.
/// O tooltip mostra o atalho (`n` e a posicao na navegacao).
fn nav_item(app: &State, entry: NavEntry, n: usize) -> Element<'_, Msg> {
    let t = app.iced.tokens;
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
        .on_press(Msg::SelectScreen(entry.screen));
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

/// O que o card de status diz: cor do ponto, titulo, linha do endereco e linha de detalhe.
pub(super) struct StatusLines {
    pub dot: iced::Color,
    pub title: String,
    pub address: Option<String>,
    pub detail: Option<String>,
}

pub(super) fn status_lines(app: &State) -> StatusLines {
    let t = app.iced.tokens;
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

/// Ponto de 7 px com halo de 3 px na cor a 18 % (o `box-shadow: 0 0 0 3px` do design): o
/// iced nao tem sombra espalhada, entao o halo e um circulo de 13 px por baixo.
fn halo_dot<'a>(color: iced::Color) -> Element<'a, Msg> {
    container(widgets::dot(color, 7.0))
        .center(13)
        .style(theme::fill(theme::alpha(color, 0.18), 6.5))
        .into()
}

/// Card de status: `surface`, raio 8, 14 de respiro; titulo 13 e as linhas em `text_muted`.
/// Clicavel: leva a Aparelhos, onde estao o endereco e quem esta conectado.
fn status_card(app: &State) -> Element<'_, Msg> {
    let t = app.iced.tokens;
    let lines = status_lines(app);
    let mut col = column![
        row![
            halo_dot(lines.dot),
            text(lines.title).size(13).color(t.text)
        ]
        // Validado; fora da escala de proposito.
        .spacing(5)
        .align_y(Alignment::Center)
    ]
    // Validado; fora da escala de proposito.
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
        .on_press(status_card_press())
        .into()
}

// ---- conteudo ----

fn content(app: &State) -> Element<'_, Msg> {
    let t = app.iced.tokens;
    let body: Element<'_, Msg> = if let Some(msg) = failure(app) {
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
    // O aviso vale para qualquer tela: na primeira abertura quem esta no dashboard tambem ve.
    // Fica no topo da rolagem, e nao no cabecalho fixo: e um aviso de uma vez, e preso em cima
    // ele comeria a altura de toda tela ate a pessoa decidir. Excecao: em Lancamentos, que rola
    // so as linhas (`owns_scroll`), ele fica fixo acima dos filtros e tira altura das linhas ate
    // a pessoa decidir.
    // Validado; fora da escala de proposito.
    let mut page = column![].spacing(14);
    if let Some(banner) = remove_view::orphan_banner(app) {
        page = page.push(banner);
    }
    let page = page.push(body);
    let scroll: Element<'_, Msg> = if owns_scroll(app) {
        // A tela tem rolagem propria (Lancamentos: filtros, resumo e cabecalho da tabela fixos,
        // so as linhas rolam; Analise: os achados com `CONTENT_SCROLL` e a conversa com o seu
        // id). Aqui ela so ganha o respiro e a altura que sobra; a rolagem dela
        // usa o mesmo `CONTENT_SCROLL`, entao o `snap_to` da troca de tela continua valendo. A
        // roda do mouse so rola sobre as linhas (nao mais sobre as margens), e o respiro de baixo
        // e 32 (a margem da pagina) e nao os 40 da rolagem geral: aqui ele nao e fim de rolagem,
        // e o espaco entre o card e a borda da janela.
        container(page)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(Padding {
                top: 20.0,
                right: 32.0,
                bottom: space::PAGE,
                left: 32.0,
            })
            .into()
    } else {
        let inner = container(page).width(Length::Fill).padding(Padding {
            top: 20.0,
            right: 32.0,
            bottom: 40.0,
            left: 32.0,
        });
        // So o conteudo rola (B7): titulo, conta, mes e olho ficam a vista enquanto a pessoa le
        // a tela. A rolagem ocupa a largura toda, com o respiro dentro dela, para a barra ficar
        // na borda da janela e a roda do mouse valer tambem sobre as margens.
        scrollable(inner)
            .id(CONTENT_SCROLL)
            .direction(scrollable::Direction::Vertical(
                scrollable::Scrollbar::new()
                    .width(6)
                    .scroller_width(6)
                    .margin(3),
            ))
            .style(theme::scrollbar(t))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    };
    // Sem fundo proprio: o fundo e o do container de `frame`, que cobre lateral e conteudo
    // (um fundo aqui esconderia o gradiente atras da rolagem).
    container(column![header(app), widgets::divider(t), scroll])
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Largura util do conteudo (sem a lateral e o padding de 32 + 32; a divisoria vertical saiu
/// com a lateral Grafite). Sem teto: com o maximo de 1180 da 0.0.2 a janela maximizada deixava
/// uma faixa vazia a direita. As telas usam para decidir colunas: o iced nao tem `auto-fit`.
pub(super) fn content_width(app: &State) -> f32 {
    app.window_size.w - SIDEBAR_WIDTH - 64.0
}

/// Abaixo desta largura util os controles do cabecalho descem para baixo do titulo (o
/// `flex-wrap` do design). Com o chip, Pessoal | Comercial, mes e olho somavam ~560 px, e o
/// titulo de 30 px precisa de ~240 ao lado; na janela minima (1024) a linha unica espremia o
/// titulo em duas linhas. O chip virou a linha de baixo do titulo, mas o limite fica o mesmo
/// (8.1: "os controles descem, como hoje").
const HEADER_WRAP_WIDTH: f32 = 900.0;

/// Controles do cabecalho na mesma linha do titulo?
pub(super) fn header_inline(width: f32) -> bool {
    width >= HEADER_WRAP_WIDTH
}

/// Quais controles da direita do cabecalho uma tela mostra (o seletor de conta depende tambem
/// dos dados: `book_switch`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct HeaderControls {
    pub month: bool,
    pub eye: bool,
}

pub(super) fn header_controls(screen: Screen) -> HeaderControls {
    HeaderControls {
        // Reservas nao tem seletor de mes: saldos sao acumulados ate hoje.
        month: matches!(screen, Screen::Dashboard | Screen::Transactions),
        // Em toda tela de dados: com o olho so no Resumo, "Ver reservas" e o link dos ultimos
        // lancamentos levavam a telas sem como mostrar de novo sem voltar.
        eye: screen.shows_values(),
    }
}

/// Cabecalho fixo, fora da rolagem: titulo 30 e uma linha 13 `text_muted` a esquerda
/// (`header_line`); a direita, conforme a tela, o seletor de conta, o de mes e o olho de
/// ocultar valores. Abaixo de `HEADER_WRAP_WIDTH` os controles descem para baixo do titulo.
fn header(app: &State) -> Element<'_, Msg> {
    let t = app.iced.tokens;
    let (title, line) = header_line(app);
    // Validado; fora da escala de proposito.
    let mut controls = row![].spacing(10).align_y(Alignment::Center);
    if let Some(switch) = book_switch(app) {
        controls = controls.push(switch);
    }
    let shown = header_controls(app.screen);
    if shown.month {
        let prev = (app.month_idx > 0).then_some(Msg::PrevMonth);
        let next = (app.month_idx + 1 < app.months.len()).then_some(Msg::NextMonth);
        controls = controls.push(widgets::month_picker(
            t,
            &app.dashboard.header.month_label,
            prev,
            next,
        ));
    }
    if shown.eye {
        controls = controls.push(hide_values_button(app));
    }
    if app.screen == Screen::Connection
        && let Some(connect) = connection_view::header_action(app)
    {
        controls = controls.push(connect);
    }
    let title = widgets::page_title(t, title, &line);
    let head: Element<'_, Msg> = if header_inline(content_width(app)) {
        row![container(title).width(Length::Fill), controls]
            .spacing(16)
            .align_y(Alignment::Center)
            .into()
    } else {
        column![title, controls].spacing(16).into()
    };
    container(head)
        .width(Length::Fill)
        .padding(Padding {
            top: 28.0,
            right: 32.0,
            bottom: 18.0,
            left: 32.0,
        })
        .into()
}

/// Olho de 44 px com tooltip "Ocultar valores" / "Mostrar valores" (o `aria-label` do design).
/// Com valores ocultos o glifo vira `eye-slash`, para o estado ficar visivel sem o tooltip.
fn hide_values_button(app: &State) -> Element<'_, Msg> {
    let t = app.iced.tokens;
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
        Some(Msg::ToggleHideValues),
    )
}

/// Seletor de conta. So aparece com uma linha `books` viva de conta comercial (derivado das
/// linhas, como no app) e fora de Aparelhos e de Ajustes, que sao do hub e nao de uma conta.
pub(super) fn book_switch(app: &State) -> Option<Element<'_, Msg>> {
    let of_the_hub = matches!(app.screen, Screen::Connection | Screen::Settings);
    if of_the_hub || !app.snapshot.dashboard.has_business_book() {
        return None;
    }
    let option = |book: Book| (book.label(), app.book == book, Msg::SelectBook(book));
    Some(widgets::segmented(
        app.iced.tokens,
        SegmentedVariant::Pill,
        SegmentedSize::Regular,
        false,
        [option(Book::Personal), option(Book::Business)],
    ))
}

/// Hub que nao subiu: no lugar de qualquer tela (menos Ajustes), o motivo e onde ficam os
/// dados.
fn failed_card<'a>(app: &'a State, msg: &'a str) -> Element<'a, Msg> {
    let t = app.iced.tokens;
    // O padrao dos estados vazios (titulo 18, corpo 14), com o glifo na cor do erro e a pasta
    // de dados como nota: e ela que a pessoa procura para entender a falha.
    let col = column![
        icons::icon(icons::WARNING_CIRCLE, 28.0, t.expense_fg),
        Space::new().height(space::M),
        widgets::empty_copy(t, "O hub não iniciou", msg, true),
        Space::new().height(space::S),
        text(format!("Dados em {}", app.snapshot.data_dir.display()))
            .size(theme::type_scale::META)
            .color(t.text_muted),
    ]
    .align_x(Alignment::Center);
    widgets::card(t, container(col).center_x(Length::Fill))
        .padding([space::GIANT, space::XXL])
        .into()
}

#[cfg(test)]
mod tests {
    use super::super::app::tests::app_with;
    use super::*;
    use crate::advisor::Severity;
    use crate::hub::snapshot::{DeviceView, Snapshot};
    use crate::ui_model::shell::{
        Arrival, STATUS_CARD_TARGET, any_active_synced, hub_has_rows, titles,
    };
    use std::net::Ipv4Addr;

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
        assert_eq!(lines.dot, app.iced.tokens.warning);

        app.snapshot.status = Status::NoNetwork;
        app.snapshot.addresses.clear();
        let lines = status_lines(&app);
        assert_eq!(lines.title, "Sem rede local");
        assert_eq!(lines.address.as_deref(), Some("porta 7777 aberta"));

        app.snapshot.status = Status::Failed("Porta 7777 em uso".into());
        let lines = status_lines(&app);
        assert_eq!(lines.title, "Hub com erro");
        assert_eq!(lines.detail.as_deref(), Some("Porta 7777 em uso"));
        assert_eq!(lines.dot, app.iced.tokens.expense);

        app.snapshot.status = Status::Starting;
        assert_eq!(status_lines(&app).title, "Iniciando…");
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
        assert_eq!(titles(&app).1, "Quanto está guardado, e para quê?");
    }

    #[test]
    fn status_sem_dados_usa_warning_e_com_dados_income() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = super::super::app::tests::app_with_fixtures(dir.path());
        app.snapshot.status = Status::Running;
        let lines = status_lines(&app);
        assert_eq!(lines.title, "Hub ligado");
        assert_eq!(lines.dot, app.iced.tokens.income);

        let snap = Snapshot {
            status: Status::Running,
            ..Snapshot::default()
        };
        let (empty, _rx, _tx) = app_with(snap, dir.path().to_path_buf());
        let lines = status_lines(&empty);
        assert_eq!(lines.title, "Hub ligado · sem dados");
        assert_eq!(lines.dot, empty.iced.tokens.warning);
        assert_ne!(lines.dot, empty.iced.tokens.token("amber"));
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
    fn card_de_status_leva_a_aparelhos_e_a_lateral_monta_em_toda_tela() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = super::super::app::tests::app_with_fixtures(dir.path());
        for screen in nav_entries(&app).map(|e| e.screen) {
            app.screen = screen;
            let _ = root(&app);
        }
        assert_eq!(STATUS_CARD_TARGET, Screen::Connection);
        assert!(matches!(
            status_card_press(),
            Msg::SelectScreen(STATUS_CARD_TARGET)
        ));
        app.screen = Screen::Dashboard;
        let _ = super::super::app::update(&mut app, status_card_press());
        assert_eq!(app.screen, STATUS_CARD_TARGET);
        assert_eq!(titles(&app).0, "Aparelhos");
    }

    #[test]
    fn cabecalho_fixo_monta_nas_duas_larguras() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = super::super::app::tests::app_with_fixtures(dir.path());
        for width in [1024.0, 1440.0] {
            app.window_size.w = width;
            for screen in nav_entries(&app).map(|e| e.screen) {
                app.screen = screen;
                let _ = root(&app);
            }
        }
    }

    #[test]
    fn olho_nas_quatro_telas_de_dados_e_fora_de_aparelhos_e_ajustes() {
        let eye = |s: Screen| header_controls(s).eye;
        for s in [
            Screen::Dashboard,
            Screen::Transactions,
            Screen::Reserves,
            Screen::Analysis,
        ] {
            assert!(eye(s), "{s:?}");
        }
        for s in [Screen::Connection, Screen::Settings] {
            assert!(!eye(s), "{s:?}");
        }
        assert!(header_controls(Screen::Transactions).month);
        assert!(!header_controls(Screen::Reserves).month);
    }

    #[test]
    fn cabecalho_monta_em_toda_tela_com_valores_ocultos() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = super::super::app::tests::app_with_fixtures(dir.path());
        let _ = crate::ui::app::update(&mut app, Msg::ToggleHideValues);
        assert!(app.hide_values);
        for screen in nav_entries(&app).map(|e| e.screen) {
            app.screen = screen;
            let _ = root(&app);
        }
    }

    #[test]
    fn cabecalho_quebra_abaixo_de_900_de_largura_util() {
        assert!(header_inline(1144.0)); // janela inicial de 1440
        assert!(!header_inline(1024.0 - SIDEBAR_WIDTH - 64.0)); // janela minima
        assert!(header_inline(900.0));
    }

    #[test]
    fn hub_com_erro_mostra_o_card_de_falha_e_nao_o_esqueleto() {
        let dir = tempfile::tempdir().unwrap();
        let snap = Snapshot {
            status: Status::Failed("Porta 7777 em uso".into()),
            ..Snapshot::default()
        };
        let (mut app, _rx, _tx) = app_with(snap, dir.path().to_path_buf());
        for screen in [Screen::Dashboard, Screen::Transactions, Screen::Reserves] {
            app.screen = screen;
            // A casca troca a tela pelo card de falha antes de qualquer estado de chegada.
            assert_eq!(failure(&app), Some("Porta 7777 em uso"));
            assert!(!owns_scroll(&app));
            assert_ne!(screen_arrival(&app, false), Arrival::Opening);
            let _ = root(&app);
        }
    }
    #[test]
    fn estados_sem_dados_escolhidos_pelo_app_e_montados_nas_tres_telas() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        let draw = |app: &mut State| {
            for screen in [Screen::Dashboard, Screen::Transactions, Screen::Reserves] {
                app.screen = screen;
                let _ = root(app);
            }
            app.screen = Screen::Dashboard;
        };
        assert_eq!(screen_arrival(&app, false), Arrival::Opening);
        draw(&mut app);
        app.snapshot.status = Status::Running;
        assert_eq!(screen_arrival(&app, false), Arrival::FirstUse);
        draw(&mut app);
        // Revogado nao conta; com um ativo, o nome dele.
        app.snapshot.devices = vec![
            device("Pixel da Ana", None, None, false),
            device("Velho", None, None, true),
        ];
        assert_eq!(
            screen_arrival(&app, false),
            Arrival::Waiting("Aguardando o primeiro sync de Pixel da Ana".into())
        );
        draw(&mut app);
        app.snapshot
            .devices
            .push(device("iPhone do Luiz", None, None, false));
        assert_eq!(
            screen_arrival(&app, false),
            Arrival::Waiting("Aguardando o primeiro sync de 2 celulares".into())
        );
        // Sem rede: os cards montam com o aviso do Wi-Fi.
        app.snapshot.status = Status::NoNetwork;
        draw(&mut app);
        app.snapshot.status = Status::Running;
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
        let (app, _rx, _tx) = super::super::app::tests::app_with_fixtures(dir.path());
        assert_eq!(screen_arrival(&app, !app.dashboard.is_empty), Arrival::Data);
        assert!(hub_has_rows(&app));
    }

    /// A lateral diz "Resumo" mesmo quando o titulo do cabecalho e "Resumo da casa" (o modelo
    /// do titulo vive em `ui_model::shell`: `titulo_do_dashboard_e_resumo_da_casa_e_a_nav_diz_resumo`).
    #[test]
    fn a_nav_diz_resumo() {
        let dir = tempfile::tempdir().unwrap();
        let (app, _rx, _tx) = super::super::app::tests::app_with_fixtures(dir.path());
        assert_eq!(nav_entries(&app)[0].label, "Resumo");
    }

    /// A lateral desenha na ordem do Ctrl+1..6: `keys::shortcut` le `NAV_ORDER`, entao a ordem
    /// de `nav_entries` precisa ser a mesma ou o tooltip diria um numero e a tecla faria outro.
    #[test]
    fn lateral_desenha_na_ordem_do_ctrl_numero() {
        let dir = tempfile::tempdir().unwrap();
        let (app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        assert_eq!(
            nav_entries(&app).map(|e| e.screen),
            crate::ui_model::shell::NAV_ORDER
        );
    }

    /// So a montagem: o hub com erro troca a tela pelo card de falha em toda tela da lateral
    /// (a regra vive em `ui_model::shell`: `hub_com_erro_cobre_as_telas_menos_ajustes`).
    #[test]
    fn hub_com_erro_monta_em_toda_tela() {
        let dir = tempfile::tempdir().unwrap();
        let snap = Snapshot {
            status: Status::Failed("Porta 7777 em uso".into()),
            ..Snapshot::default()
        };
        let (mut app, _rx, _tx) = app_with(snap, dir.path().to_path_buf());
        for screen in nav_entries(&app).map(|e| e.screen) {
            app.screen = screen;
            let _ = root(&app);
        }
    }
    #[test]
    fn rolagem_monta_em_toda_tela_e_com_o_menu_de_categoria_aberto() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = super::super::app::tests::app_with_fixtures(dir.path());
        for screen in nav_entries(&app).map(|e| e.screen) {
            app.screen = screen;
            assert_eq!(
                owns_scroll(&app),
                matches!(screen, Screen::Transactions | Screen::Analysis),
                "{screen:?}"
            );
            let _ = root(&app);
        }
        // Com o menu de categoria aberto a tela continua montando, nas duas larguras.
        app.screen = Screen::Transactions;
        app.cat_menu_open = true;
        for width in [1024.0, 1440.0] {
            app.window_size.w = width;
            let _ = root(&app);
        }
        // Hub em erro: o card de falha entra no lugar e precisa da rolagem da casca.
        app.snapshot.status = Status::Failed("Porta 7777 em uso".into());
        assert!(!owns_scroll(&app));
        let _ = root(&app);
    }
}
