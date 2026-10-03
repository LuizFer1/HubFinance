//! Casca da janela: barra de titulo propria, moldura de redimensionar, barra lateral com o
//! card de status e o seletor de tema, e a area de conteudo rolavel.

use std::net::Ipv4Addr;

use iced::widget::{
    Space, button, column, container, mouse_area, opaque, row, scrollable, stack, text,
};
use iced::{Alignment, Element, Length, Padding, mouse, window};

use super::app::{App, Message, Screen};
use super::theme::{self, Tokens};
use super::{connection_view, dashboard_view, fonts, icons, time, transactions_view, widgets};
use crate::config::ThemeMode;
use crate::dashboard::money::group_thousands;
use crate::hub::snapshot::Status;

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
    if let Some(dialog) = remove_dialog(app) {
        layers = layers.push(dialog);
    }
    if let Some(pending) = &app.pending_removal {
        layers = layers.push(toast(app.tokens, &pending.toast));
    }
    layers.into()
}

/// Dialogo "Remover Ana?": fundo preto a 55 % que cancela ao clicar e a caixa por cima.
fn remove_dialog(app: &App) -> Option<Element<'_, Message>> {
    let t = app.tokens;
    let device_id = app.confirm_remove.as_deref()?;
    let person = app.people.iter().find(|p| p.device_id == device_id)?;
    let photo = app.avatars.get(&person.device_id).map(|a| &a.handle);
    let name = person.display_name.as_str();
    let header = row![
        widgets::avatar(t, name, person.color.as_deref(), photo, 40.0),
        text(format!("Remover {name}?"))
            .size(18)
            .font(fonts::INTER_MEDIUM)
            .color(t.text),
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    let body = text(format!(
        "O {} para de sincronizar com este computador. Os lançamentos que {name} já criou \
         continuam no histórico. Para voltar, basta conectar de novo pelo QR code.",
        person.device_name
    ))
    .size(14)
    .line_height(1.55)
    .color(t.text_alpha(0.70));
    let action = |content: Element<'static, Message>| {
        button(
            container(content)
                .height(Length::Fill)
                .align_y(Alignment::Center),
        )
        .height(38)
        .padding([0, 16])
    };
    let cancel = action(text("Cancelar").size(13).font(fonts::INTER_MEDIUM).into())
        .style(theme::outline_divider(t))
        .on_press(Message::CancelRemove);
    let confirm = action(
        row![
            icons::icon_inherit(icons::USER_MINUS, 13.0),
            text("Remover").size(13).font(fonts::INTER_MEDIUM),
        ]
        .spacing(6)
        .align_y(Alignment::Center)
        .into(),
    )
    .style(theme::danger_filled(t))
    .on_press(Message::ConfirmRemove);
    let buttons = row![Space::new().width(Length::Fill), cancel, confirm].spacing(10);
    let width = (app.window_size.width - 48.0).clamp(240.0, 420.0);
    let dialog_box = container(
        column![
            header,
            Space::new().height(14),
            body,
            Space::new().height(22),
            buttons
        ]
        .width(Length::Fill),
    )
    .width(width)
    .padding(24)
    .style(theme::dialog(t));
    // A caixa engole o clique: sem isso, clicar dentro dela chegaria ao fundo e cancelaria.
    let dialog_box = opaque(mouse_area(dialog_box).on_press(Message::Noop));
    let scrim = opaque(
        mouse_area(
            container(Space::new())
                .width(Length::Fill)
                .height(Length::Fill)
                .style(theme::scrim()),
        )
        .on_press(Message::CancelRemove),
    );
    Some(
        stack![scrim, container(dialog_box).center(Length::Fill)]
            .width(Length::Fill)
            .height(Length::Fill)
            .into(),
    )
}

/// Toast de 40 px centralizado embaixo, com "Desfazer".
fn toast<'a>(t: &'static Tokens, message: &'a str) -> Element<'a, Message> {
    let undo = button(
        container(text("Desfazer").size(13).font(fonts::INTER_MEDIUM))
            .height(Length::Fill)
            .align_y(Alignment::Center),
    )
    .height(28)
    .padding([0, 10])
    .style(theme::ghost(t))
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
    let body = row![sidebar(app), vline(t), content(app)].height(Length::Fill);
    container(column![titlebar(app), hline(t), body])
        .width(Length::Fill)
        .height(Length::Fill)
        .style(theme::fill(t.bg, 0.0))
        .into()
}

/// Linha de 1 px no divisor: o iced nao tem borda de um lado so.
fn hline<'a>(t: &'static Tokens) -> Element<'a, Message> {
    container(Space::new())
        .width(Length::Fill)
        .height(1)
        .style(theme::fill(t.divider(), 0.0))
        .into()
}

fn vline<'a>(t: &'static Tokens) -> Element<'a, Message> {
    container(Space::new())
        .width(1)
        .height(Length::Fill)
        .style(theme::fill(t.divider(), 0.0))
        .into()
}

// ---- barra de titulo ----

fn titlebar(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let mark = container(icons::icon(icons::HOUSE_LINE, 11.0, t.accent_300))
        .center(18)
        .style(theme::outlined(t.accent, 5.0));
    let brand = row![mark, text("HubFinance").size(12).color(t.text_alpha(0.75)),]
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
        .style(theme::fill(t.side, 0.0))
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
    let mark = container(icons::icon(icons::HOUSE_LINE, 17.0, t.accent_300))
        .center(32)
        .style(theme::outlined(t.accent, 8.0));
    let brand = container(
        row![
            mark,
            column![
                text("HubFinance")
                    .size(15)
                    .font(fonts::INTER_MEDIUM)
                    .line_height(1.2)
                    .color(t.text),
                text(brand_subtitle()).size(11).color(t.text_alpha(0.55)),
            ],
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .padding(Padding {
        top: 0.0,
        right: 8.0,
        bottom: 18.0,
        left: 8.0,
    });

    let group = container(
        text("HUB")
            .size(11)
            .font(fonts::INTER_MEDIUM)
            .color(t.text_alpha(0.45)),
    )
    .padding(Padding {
        top: 14.0,
        right: 10.0,
        bottom: 6.0,
        left: 10.0,
    });

    let col = column![
        brand,
        nav_item(
            app,
            Screen::Dashboard,
            icons::CHART_PIE_SLICE,
            "Dashboard",
            None
        ),
        nav_item(
            app,
            Screen::Transactions,
            icons::LIST_BULLETS,
            "Lançamentos",
            Some(transactions_count(app)),
        ),
        group,
        nav_item(
            app,
            Screen::Connection,
            icons::QR_CODE,
            "Conexão",
            Some(active_devices(app)),
        ),
        Space::new().height(Length::Fill),
        status_card(app),
        container(theme_switch(app)).padding(Padding {
            top: 8.0,
            ..Padding::ZERO
        }),
    ]
    .spacing(4)
    .padding(Padding {
        top: 18.0,
        right: 12.0,
        bottom: 14.0,
        left: 12.0,
    })
    .width(SIDEBAR_WIDTH)
    .height(Length::Fill);

    container(col)
        .height(Length::Fill)
        .style(theme::fill(t.side, 0.0))
        .into()
}

/// Lancamentos vivos no total.
pub(super) fn transactions_count(app: &App) -> usize {
    app.snapshot.dashboard.alive_transactions().count()
}

/// Aparelhos ativos: e o numero de linhas da lista "Usuarios conectados" (sem a remocao
/// pendente, que ja sumiu da tela).
pub(super) fn active_devices(app: &App) -> usize {
    let pending = app.pending_removal.as_ref().map(|p| p.device_id.as_str());
    app.snapshot
        .devices
        .iter()
        .filter(|d| !d.revoked && Some(d.device_id.as_str()) != pending)
        .count()
}

fn nav_item<'a>(
    app: &'a App,
    screen: Screen,
    glyph: &'static str,
    label: &'a str,
    count: Option<usize>,
) -> Element<'a, Message> {
    let t = app.tokens;
    let active = app.screen == screen;
    let icon_color = if active {
        t.accent_300
    } else {
        t.text_alpha(0.55)
    };
    let mut line = row![
        icons::icon(glyph, 18.0, icon_color),
        text(label)
            .size(13)
            .font(fonts::INTER_MEDIUM)
            .width(Length::Fill),
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    if let Some(n) = count {
        line = line.push(text(thousands(n)).size(11).color(t.text_alpha(0.50)));
    }
    button(container(line).height(36).align_y(Alignment::Center))
        .padding([0, 10])
        .width(Length::Fill)
        .style(theme::nav_item(t, active))
        .on_press(Message::SelectScreen(screen))
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
            dot: t.amber,
            title: "Hub ligado · sem dados".into(),
            address: address(app),
            detail: Some("Aguardando o primeiro sync de um celular pareado.".into()),
        },
        Status::NoNetwork => StatusLines {
            dot: t.amber,
            title: "Sem rede local".into(),
            address: Some(format!("porta {} aberta", snap.https_port)),
            detail: Some("Conecte o computador ao Wi-Fi de casa.".into()),
        },
        Status::Starting => StatusLines {
            dot: t.neutral_600,
            title: "Iniciando…".into(),
            address: None,
            detail: None,
        },
        Status::Stopping => StatusLines {
            dot: t.neutral_600,
            title: "Desligando…".into(),
            address: None,
            detail: None,
        },
        Status::Stopped => StatusLines {
            dot: t.neutral_600,
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

/// Ha lancamento vivo no hub.
pub(super) fn has_data(app: &App) -> bool {
    !app.snapshot.dashboard.is_empty()
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

fn status_card(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let lines = status_lines(app);
    let mut col = column![
        row![
            widgets::dot(lines.dot, 7.0),
            text(lines.title)
                .size(12)
                .font(fonts::INTER_MEDIUM)
                .color(t.text),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
    ]
    .spacing(6);
    if let Some(address) = lines.address {
        col = col.push(text(address).size(11).color(t.text_alpha(0.70)));
    }
    if let Some(detail) = lines.detail {
        col = col.push(
            text(detail)
                .size(11)
                .line_height(1.45)
                .color(t.text_alpha(0.55)),
        );
    }
    container(col)
        .padding(12)
        .width(Length::Fill)
        .style(theme::card_inset(t))
        .into()
}

fn theme_switch(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let option = |mode: ThemeMode, glyph: &'static str, label: &'static str| {
        let active = app.theme == mode;
        button(
            container(
                row![
                    text(glyph).font(fonts::PHOSPHOR).size(12).line_height(1.0),
                    text(label).size(12).font(fonts::INTER_MEDIUM),
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            )
            .center_x(Length::Fill)
            .height(26)
            .align_y(Alignment::Center),
        )
        .padding(0)
        .width(Length::Fill)
        .style(theme::seg_option(t, active))
        .on_press(Message::SetTheme(mode))
    };
    container(
        row![
            option(ThemeMode::Dark, icons::MOON, "Escuro"),
            option(ThemeMode::Light, icons::SUN, "Claro"),
        ]
        .width(Length::Fill),
    )
    .padding(3)
    .width(Length::Fill)
    .style(theme::outlined(t.divider(), 8.0))
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
            Screen::Connection => connection_view::view(app),
        }
    };
    let page = column![header(app), body].spacing(14);
    let inner = container(page).width(Length::Fill).padding(Padding {
        top: 28.0,
        right: 32.0,
        bottom: 40.0,
        left: 32.0,
    });
    let scroll = scrollable(inner)
        .direction(scrollable::Direction::Vertical(
            scrollable::Scrollbar::new()
                .width(6)
                .scroller_width(6)
                .margin(3),
        ))
        .style(theme::scrollbar(t))
        .width(Length::Fill)
        .height(Length::Fill);
    container(scroll)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(theme::fill(t.bg, 0.0))
        .into()
}

/// Largura util do conteudo (sem a lateral, a borda e o padding de 32 + 32). Sem teto: com o
/// maximo de 1180 da 0.0.2 a janela maximizada deixava uma faixa vazia a direita. As telas usam
/// para decidir colunas: o iced nao tem `auto-fit`.
pub(super) fn content_width(app: &App) -> f32 {
    app.window_size.width - SIDEBAR_WIDTH - 1.0 - 64.0
}

fn header(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let (title, subtitle) = titles(app);
    let mut line = row![container(widgets::page_title(t, title, &subtitle)).width(Length::Fill)]
        .spacing(24)
        .align_y(Alignment::End);
    if app.screen != Screen::Connection {
        line = line.push(month_controls(app));
    }
    container(line)
        .padding(Padding {
            bottom: 8.0,
            ..Padding::ZERO
        })
        .into()
}

/// Tag "Parcial · até 24 set" (so no mes corrente e com dados) e o seletor de mes. As setas
/// ficam sem acao nos limites da janela de 12 meses.
fn month_controls(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let v = &app.dashboard;
    let mut controls = row![].spacing(10).align_y(Alignment::Center);
    if let Some(until) = v.partial_until.as_deref().filter(|_| !v.is_empty) {
        controls = controls.push(widgets::partial_tag(t, until));
    }
    let prev = (app.month_idx > 0).then_some(Message::PrevMonth);
    let next = (app.month_idx + 1 < app.months.len()).then_some(Message::NextMonth);
    controls
        .push(widgets::month_picker(t, &v.month_label, prev, next))
        .into()
}

pub(super) fn titles(app: &App) -> (&'static str, String) {
    let empty = !has_data(app);
    match app.screen {
        Screen::Dashboard => (
            "Dashboard",
            if empty {
                "Nada sincronizado ainda".into()
            } else {
                "Somente leitura · dados do último sync".into()
            },
        ),
        Screen::Transactions => (
            "Lançamentos",
            if empty {
                "Nada sincronizado ainda".into()
            } else {
                format!(
                    "{} sincronizados de {}",
                    plural(transactions_count(app), "lançamento", "lançamentos"),
                    plural(app.transactions.synced_devices, "aparelho", "aparelhos"),
                )
            },
        ),
        Screen::Connection => (
            "Conexão",
            "Abra este endereço no celular para conectar ao hub".into(),
        ),
    }
}

/// "1 lançamento" / "1.234 lançamentos" (milhar com ponto, como o pt-BR do app).
pub(super) fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{} {}", thousands(n), if n == 1 { one } else { many })
}

pub(super) fn thousands(n: usize) -> String {
    group_thousands(n as u64)
}

/// Hub que nao subiu: no lugar de qualquer tela, o motivo e onde ficam os dados.
fn failed_card<'a>(app: &'a App, msg: &'a str) -> Element<'a, Message> {
    let t = app.tokens;
    container(
        column![
            icons::icon(icons::WARNING_CIRCLE, 28.0, t.expense_fg),
            text("O hub não iniciou")
                .size(16)
                .font(fonts::INTER_MEDIUM)
                .color(t.text),
            text(msg).size(13).color(t.text_alpha(0.60)),
            text(format!("Dados em {}", app.snapshot.data_dir.display()))
                .size(12)
                .color(t.text_alpha(0.50)),
        ]
        .spacing(8),
    )
    .padding(24)
    .width(Length::Fill)
    .style(theme::card(t))
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
        assert_eq!(lines.dot, app.tokens.amber);

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
    fn subtitulos() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        assert_eq!(titles(&app).1, "Nada sincronizado ainda");
        app.screen = Screen::Connection;
        assert_eq!(
            titles(&app),
            (
                "Conexão",
                "Abra este endereço no celular para conectar ao hub".to_string()
            )
        );
    }
}
