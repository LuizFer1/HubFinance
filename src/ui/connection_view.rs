//! Tela Aparelhos (spec 8.4): quem sincroniza com a casa, o endereco do hub e a atividade; e o
//! fluxo "Conectar um celular" em tres passos (Wi-Fi, certificado, parear), em dialogo quando
//! ja ha aparelho e ocupando a tela quando nao ha nenhum.
//!
//! O QR e o da fatia 1 (`http://<ip>:7778/p/<token>`, so com token ativo), nao o endereco
//! puro do prototipo: um QR so com o endereco levaria o celular a uma conexao que ele nao pode
//! confiar (sem CA) nem autenticar (sem token).

use std::net::Ipv4Addr;
use std::time::{Duration, SystemTime};

use iced::widget::{Space, column, container, qr_code, row, text};
use iced::{Alignment, Border, Element, Length, Padding, Theme};

use super::app::{CopyTarget, Msg, State};
use super::theme::{self, ButtonKind, ButtonSize, Tokens, radius, size, space, type_scale};
use super::widgets::{RowEdge, SegmentedSize, SegmentedVariant};
use super::{fonts, icons, rows, shell, widgets};
use crate::dashboard::people::{Person, presence};
use crate::dashboard::periods::paired_label;
use crate::hub::snapshot::{ActivityKind, Status};
use crate::ui_model::connect::{
    ConnectFlow, ConnectStep, PAIR_STEPS, PhoneOs, certificate_steps, guide_url, hub_url, is_up,
    step_caption,
};
pub(super) use crate::ui_model::connect::{dialog_open, in_screen_case, shows_in_screen_flow};
use crate::ui_model::{measure, time};

/// Largura minima de cada card da grade antes de empilhar (`minmax(420px, 1fr)`). Na janela
/// minima (1024) o conteudo tem 728 e fica numa coluna so, com o endereco primeiro.
const GRID_MIN: f32 = 420.0;
const GAP: f32 = space::BETWEEN_CARDS;
/// Padding dos cards de endereco e do fluxo: o de todo card.
const CARD_PAD: f32 = space::CARD;
/// O QR do passo 3: 220 px (spec 8.4), contando a zona de silencio que o widget do iced
/// desenha (2 modulos de cada lado).
const QR_SIZE: f32 = 220.0;
/// O quadro claro em volta do QR: 5 px de cada lado.
const QR_FRAME: f32 = QR_SIZE + 10.0;
/// Largura do botao do campo quando mostra "Copiado" (10 + 12 + 6 + 48 + 10).
const COPIED_BUTTON: f32 = 86.0;
/// Moldura do campo em volta do valor: padding, icone, os dois espacos e o botao em "Copiado"
/// (141 px). Lida das medidas do proprio widget, para as duas contas nao ficarem defasadas.
const FIELD_CHROME: f32 = widgets::FIELD_PAD_LEFT
    + widgets::FIELD_ICON
    + 2.0 * widgets::FIELD_SPACING
    + COPIED_BUTTON
    + widgets::FIELD_PAD_RIGHT;
/// Tamanhos do valor do campo, do design (15) para baixo.
const FIELD_SIZES: [f32; 3] = [15.0, 14.0, 13.0];
/// Largura do fluxo: a mesma no dialogo e na tela, para o passo 3 caber QR e passos lado a lado.
const FLOW_WIDTH: f32 = 600.0;
/// Altura fixa do dialogo: o rodape nao pula ao trocar de passo. Cabe o passo mais alto (o
/// certificado do iPhone) e cabe na janela minima (720 de altura).
const DIALOG_HEIGHT: f32 = 580.0;

// ---- tela ----

pub fn view(app: &State) -> Element<'_, Msg> {
    if shows_in_screen_flow(app) {
        let flow = app.connect.unwrap_or_default();
        let card = widgets::card(app.iced.tokens, flow_body(app, flow, false))
            .padding(CARD_PAD)
            .max_width(FLOW_WIDTH);
        return column![card, activity_card(app)]
            .spacing(GAP)
            .width(Length::Fill)
            .into();
    }
    let width = shell::content_width(app);
    let inner = address_inner(width);
    if two_columns(width) {
        row![
            container(users_card(app)).width(Length::FillPortion(1)),
            column![address_card(app, inner), activity_card(app)]
                .spacing(GAP)
                .width(Length::FillPortion(1)),
        ]
        .spacing(GAP)
        .align_y(Alignment::Start)
        .into()
    } else {
        // Uma coluna: o endereco primeiro, que e o que se procura ao conectar.
        column![
            address_card(app, inner),
            users_card(app),
            activity_card(app)
        ]
        .spacing(GAP)
        .width(Length::Fill)
        .into()
    }
}

fn two_columns(width: f32) -> bool {
    width >= 2.0 * GRID_MIN + GAP
}

/// Largura interna do card do endereco numa area de conteudo `width`: metade (menos o vao) em
/// duas colunas, inteira em uma, menos o padding.
fn address_inner(width: f32) -> f32 {
    let card = if two_columns(width) {
        (width - GAP) / 2.0
    } else {
        width
    };
    card - 2.0 * CARD_PAD
}

/// Tamanho do valor de um campo de `field_width` px: o maior que mostra o valor inteiro. O
/// iced 0.14 nao tem reticencias e um endereco cortado nao serve para digitar no app.
fn field_value_size(value: &str, field_width: f32) -> f32 {
    measure::fitting_size(value, field_width - FIELD_CHROME, &FIELD_SIZES)
}

/// "Conectar um celular" do cabecalho de Aparelhos. Some quando o fluxo ja esta na tela (sem
/// aparelho) e com o nucleo fora do ar (nao ha codigo a gerar).
pub(super) fn header_action(app: &State) -> Option<Element<'_, Msg>> {
    (is_up(&app.snapshot) && !in_screen_case(app)).then(|| {
        widgets::action_button(
            app.iced.tokens,
            ButtonKind::Primary,
            ButtonSize::Large,
            "Conectar um celular",
        )
        .on_press(Msg::ConnectPhone)
        .into()
    })
}

// ---- card do endereco ----

fn address_card(app: &State, inner: f32) -> Element<'_, Msg> {
    let t = app.iced.tokens;
    let snap = &app.snapshot;
    let url = hub_url(snap);
    let size = url
        .as_deref()
        .map_or(FIELD_SIZES[0], |u| field_value_size(u, inner));
    let mut col = column![
        widgets::kicker_accent(t, "Endereço do hub"),
        Space::new().height(10),
        field(
            app,
            icons::LOCK_SIMPLE,
            t.income_fg,
            url,
            CopyTarget::Address,
            size
        ),
    ];
    if snap.addresses.len() > 1 {
        let others: Vec<String> = snap.addresses[1..]
            .iter()
            .map(Ipv4Addr::to_string)
            .collect();
        col = col.push(
            container(
                text(format!("Outros endereços: {}", others.join(", ")))
                    .size(12)
                    .color(t.text_muted),
            )
            .padding(Padding {
                top: 8.0,
                ..Padding::ZERO
            }),
        );
    }
    col = col.push(Space::new().height(12)).push(lan_note(t));
    widgets::card(t, col.width(Length::Fill))
        .padding(CARD_PAD)
        .into()
}

fn lan_note<'a>(t: &'static Tokens) -> Element<'a, Msg> {
    row![
        icons::icon(icons::HOUSE_SIMPLE, 12.0, t.text_muted),
        text("Funciona só na rede de casa.")
            .size(12)
            .color(t.text_muted),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}

/// Campo de 44 px com icone, valor em `size` (cortado se ainda assim nao couber) e "Copiar".
fn field<'a>(
    app: &'a State,
    glyph: &'static str,
    glyph_color: iced::Color,
    value: Option<String>,
    target: CopyTarget,
    size: f32,
) -> Element<'a, Msg> {
    widgets::field_copy(
        app.iced.tokens,
        (glyph, glyph_color),
        value,
        size,
        "sem endereço de rede",
        app.is_copied(target),
        Msg::Copy(target),
    )
}

// ---- aparelhos conectados ----

fn users_card(app: &State) -> Element<'_, Msg> {
    let t = app.iced.tokens;
    let count = app.visible_people().count();
    let title = container(widgets::card_title(
        t,
        "Conectados",
        Some(count.to_string()),
    ))
    .padding([0, 8]);
    let mut col = column![title, Space::new().height(8)];
    if count == 0 {
        // Nucleo fora do ar, ou o ultimo aparelho com a remocao pendente (o fluxo so ocupa a
        // tela depois que ela sai).
        let body = widgets::empty_state(
            t,
            widgets::empty_icon(t, icons::DEVICE_MOBILE),
            "Nenhum aparelho conectado",
            "Quando um celular parear, ele aparece aqui com o nome e a cor que a pessoa \
             escolheu no app.",
            None,
        );
        col = col.push(widgets::divider(t)).push(
            container(body)
                .padding(Padding {
                    top: 32.0,
                    right: 8.0,
                    bottom: 28.0,
                    left: 8.0,
                })
                .center_x(Length::Fill),
        );
    } else {
        let now = SystemTime::now();
        for person in app.visible_people() {
            col = col
                .push(widgets::divider(t))
                .push(person_row(app, person, now));
        }
    }
    // Padding lateral de 12 (e nao 20) para o hover da linha passar 8 px do conteudo, como o
    // `margin: 0 -8px` do prototipo.
    widgets::card(t, col)
        .padding(Padding {
            top: 18.0,
            right: 12.0,
            bottom: 8.0,
            left: 12.0,
        })
        .into()
}

fn person_row<'a>(app: &'a State, person: &'a Person, now: SystemTime) -> Element<'a, Msg> {
    let t = app.iced.tokens;
    let photo = app.iced.avatars.get(&person.device_id);
    let avatar = widgets::avatar(
        t,
        &person.display_name,
        person.color.as_deref(),
        photo,
        40.0,
    );

    let mut name = row![
        text(person.display_name.clone())
            .size(14)
            .font(fonts::INTER_MEDIUM)
            .color(t.text)
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    if let Some(token) = &person.color {
        name = name.push(widgets::color_tag(t, token));
    }

    let seen = presence(person.last_seen_at.as_deref(), now);
    let dot = if seen.is_online() { t.income } else { t.idle() };
    let muted = t.text_muted;
    let meta = row![
        row![
            icons::icon(icons::DEVICE_MOBILE, 12.0, muted),
            text(person.device_name.clone()).size(12).color(muted),
        ]
        .spacing(space::XS)
        .align_y(Alignment::Center),
        text(format!(
            "desde {}",
            paired_label(&person.paired_at, app.offset)
        ))
        .size(12)
        .color(muted),
        row![
            widgets::dot(dot, 6.0),
            text(seen.label(person.user_known)).size(12).color(muted),
        ]
        .spacing(space::XS)
        .align_y(Alignment::Center),
    ]
    .spacing(space::M)
    .align_y(Alignment::Center)
    .wrap()
    .vertical_spacing(4);

    let remove = widgets::action_button_icon(
        t,
        ButtonKind::Danger,
        ButtonSize::Compact,
        icons::USER_MINUS,
        "Remover",
    )
    .on_press(Msg::AskRemove(person.device_id.clone()));

    let content = row![
        // Altura minima da linha (68 com o padding): o iced nao tem `min-height`.
        Space::new().width(0).height(48),
        avatar,
        column![name, meta].spacing(space::XS).width(Length::Fill),
        remove,
    ]
    .spacing(space::L)
    .align_y(Alignment::Center);

    rows::hover_row(
        app,
        &person.device_id,
        content,
        [10, 8],
        RowEdge::Inset,
        None,
    )
}

// ---- atividade ----

/// O "status detalhado" que o design nao desenhou, reduzido ao essencial. Sem o "epoch": e
/// jargao, e quem precisa dele acha em Ajustes > Sobre (a pasta dos dados e os logs).
fn activity_card(app: &State) -> Element<'_, Msg> {
    let t = app.iced.tokens;
    let snap = &app.snapshot;
    let mut col = column![
        widgets::card_title(t, "Atividade", Some("últimas 20".into())),
        Space::new().height(8),
    ];
    if snap.activity.is_empty() {
        col = col.push(container(text("Nada ainda").size(13).color(t.text_muted)).height(28));
    }
    for entry in snap.activity.iter().take(20) {
        let at = time::format_time(::time::OffsetDateTime::from(entry.at), app.offset);
        let color = match entry.kind {
            ActivityKind::Warning => t.warning,
            ActivityKind::Error => t.expense_fg,
            ActivityKind::Info | ActivityKind::Sync => t.text,
        };
        col = col.push(
            row![
                text(at).size(12).color(t.text_muted).width(44),
                text(entry.text.clone()).size(13).color(color),
            ]
            .spacing(space::M)
            .height(28)
            .align_y(Alignment::Center),
        );
    }
    widgets::card(t, col)
        .padding(Padding {
            top: 18.0,
            right: 20.0,
            bottom: 12.0,
            left: 20.0,
        })
        .into()
}

// ---- fluxo "Conectar um celular" ----

/// O dialogo, quando aberto (`dialog_open`). O scrim e o "x" fecham pelo mesmo caminho do Esc,
/// que cancela o codigo pendente.
pub(super) fn connect_dialog(app: &State) -> Option<Element<'_, Msg>> {
    if !dialog_open(app) {
        return None;
    }
    let flow = app.connect?;
    Some(widgets::dialog(
        app.iced.tokens,
        flow_body(app, flow, true),
        app.window_size.w,
        FLOW_WIDTH,
        Some(dialog_height(app.window_size.h)),
        Msg::ConnectClose,
        app.motion.scrim(),
    ))
}

/// Altura do dialogo: a fixa, ou a janela menos a margem de 24 de cada lado se ela for menor.
fn dialog_height(window_height: f32) -> f32 {
    DIALOG_HEIGHT.min(window_height - 48.0)
}

/// Titulo, progresso, o passo e o rodape. No dialogo o rodape fica embaixo (altura fixa) e o
/// titulo ganha o "x"; na tela nao ha o que fechar.
fn flow_body(app: &State, flow: ConnectFlow, modal: bool) -> Element<'_, Msg> {
    let t = app.iced.tokens;
    let mut head = row![
        text("Conectar um celular")
            .size(type_scale::LEAD)
            .font(fonts::INTER_MEDIUM)
            .color(t.text)
            .width(Length::Fill)
    ]
    .align_y(Alignment::Center);
    if modal {
        head = head.push(widgets::icon_button(
            t,
            icons::X,
            ButtonSize::Compact,
            "Fechar",
            Some(Msg::ConnectClose),
        ));
    }
    let progress = column![
        widgets::progress_steps(t, flow.step.index(), ConnectStep::TOTAL),
        text(step_caption(flow.step))
            .size(type_scale::META)
            .color(t.text_muted),
    ]
    .spacing(8);
    let body = match flow.step {
        ConnectStep::WiFi => wifi_step(app),
        ConnectStep::Certificate => certificate_step(app, flow.os),
        ConnectStep::Pair => pair_step(app),
    };
    let mut col = column![
        head,
        Space::new().height(space::L),
        progress,
        Space::new().height(space::XL),
        body
    ]
    .width(Length::Fill);
    col = if modal {
        // A caixa tem altura fixa: a coluna a preenche e o rodape fica sempre no mesmo lugar.
        col.height(Length::Fill)
            .push(Space::new().height(Length::Fill))
    } else {
        col.push(Space::new().height(24))
    };
    col.push(footer(t, flow.step)).into()
}

/// Rodape a direita, o secundario antes do primario.
fn footer<'a>(t: &'static Tokens, step: ConnectStep) -> Element<'a, Msg> {
    let size = ButtonSize::Regular;
    let back = || {
        widgets::action_button(t, ButtonKind::Secondary, size, "Voltar").on_press(Msg::ConnectBack)
    };
    let mut line = row![Space::new().width(Length::Fill)]
        .spacing(space::M)
        .align_y(Alignment::Center);
    match step {
        ConnectStep::WiFi => {
            line = line.push(
                widgets::action_button(t, ButtonKind::Primary, size, "Continuar")
                    .on_press(Msg::ConnectNext),
            );
        }
        ConnectStep::Certificate => {
            line = line.push(back()).push(
                widgets::action_button(t, ButtonKind::Primary, size, "Já instalei")
                    .on_press(Msg::ConnectNext),
            );
        }
        // O passo 3 termina sozinho: o dialogo fecha quando o aparelho aparece.
        ConnectStep::Pair => line = line.push(back()),
    }
    line.into()
}

fn body_text<'a>(t: &'static Tokens, content: impl text::IntoFragment<'a>) -> text::Text<'a> {
    text(content)
        .size(type_scale::BODY)
        .line_height(1.5)
        .color(t.text_soft())
}

/// Passo 1. O hub nao sabe o nome da rede (nao le o SSID), entao nao o mostra: diz o endereco
/// do computador na rede, que ele sabe, e avisa quando nao ha rede local.
fn wifi_step(app: &State) -> Element<'_, Msg> {
    let t = app.iced.tokens;
    let snap = &app.snapshot;
    let mut col = column![
        row![
            icons::icon(icons::WIFI_HIGH, 20.0, t.text_accent()),
            body_text(t, "Conecte o celular no mesmo Wi-Fi deste computador.").width(Length::Fill),
        ]
        .spacing(12)
        .align_y(Alignment::Center)
    ]
    .spacing(12);
    if snap.status == Status::NoNetwork || snap.addresses.is_empty() {
        col = col.push(
            text("Este computador está sem rede local. Conecte-o ao Wi-Fi de casa.")
                .size(type_scale::LABEL)
                .color(t.warning),
        );
    } else if let Some(ip) = snap.addresses.first() {
        col = col.push(
            text(format!("Este computador está na rede em {ip}."))
                .size(type_scale::LABEL)
                .color(t.text_muted),
        );
    }
    col.push(lan_note(t)).into()
}

/// Passo 2: segmentado Android | iPhone e os passos do sistema; o endereco do guia vai com
/// "Copiar" dentro do passo que manda abri-lo.
fn certificate_step(app: &State, os: PhoneOs) -> Element<'_, Msg> {
    let t = app.iced.tokens;
    let option = |label: &'static str, value: PhoneOs| (label, os == value, Msg::ConnectOs(value));
    let switch = widgets::segmented(
        t,
        SegmentedVariant::PillInset,
        SegmentedSize::Regular,
        false,
        [
            option("Android", PhoneOs::Android),
            option("iPhone", PhoneOs::Iphone),
        ],
    );
    let mut steps = column![].spacing(12);
    for (i, line) in certificate_steps(os).iter().enumerate() {
        let n = u8::try_from(i + 1).unwrap_or(u8::MAX);
        let content: Element<'_, Msg> = if i == 0 {
            column![
                widgets::step_text(t, *line),
                field(
                    app,
                    icons::CERTIFICATE,
                    t.accent_300,
                    guide_url(&app.snapshot),
                    CopyTarget::Guide,
                    FIELD_SIZES[0],
                ),
            ]
            .spacing(8)
            .into()
        } else {
            widgets::step_text(t, *line).into()
        };
        steps = steps.push(widgets::step(t, n, content));
    }
    column![
        body_text(
            t,
            "Uma vez por celular: o app só confia na conexão depois que o certificado do hub está \
             instalado."
        ),
        switch,
        steps,
    ]
    // Validado; fora da escala de proposito.
    .spacing(14)
    .width(Length::Fill)
    .into()
}

/// Passo 3: QR e codigo a esquerda, os passos e o endereco a direita.
fn pair_step(app: &State) -> Element<'_, Msg> {
    let t = app.iced.tokens;
    // Validado; fora da escala de proposito.
    let mut info = column![].spacing(10).width(Length::Fill);
    for (i, line) in PAIR_STEPS.iter().enumerate() {
        let n = u8::try_from(i + 1).unwrap_or(u8::MAX);
        info = info.push(widgets::step(t, n, widgets::step_text(t, *line).into()));
    }
    if let Some(url) = hub_url(&app.snapshot) {
        info = info.push(Space::new().height(4)).push(
            column![
                text("Endereço").size(type_scale::META).color(t.text_muted),
                text(url)
                    .size(type_scale::LABEL)
                    .font(fonts::INTER_MEDIUM)
                    .color(t.text),
            ]
            .spacing(2),
        );
    }
    row![pairing_block(app), info]
        .spacing(24)
        .align_y(Alignment::Start)
        .into()
}

/// Quadro do QR (fundo `neutral_100` do escuro nos dois temas, raio 12, contorno `accent_700`)
/// e o que fica embaixo: codigo em 24 Regular, contagem e "Gerar outro codigo" com token
/// ativo; "Gerar codigo" sem ele (expirou, tentativas demais, ou o nucleo nao estava no ar ao
/// entrar no passo).
///
/// O quadro e sempre claro porque o leitor de QR do celular espera modulos escuros sobre fundo
/// claro. Na paleta Nocturne o `neutral_100` era igual nos dois temas; na Grafite a rampa do
/// claro e invertida (`neutral_100` claro e quase preto), entao o quadro fixa o do escuro.
fn pairing_block(app: &State) -> Element<'_, Msg> {
    let t = app.iced.tokens;
    let snap = &app.snapshot;
    let now = SystemTime::now();
    let frame = |content, padding| qr_frame(t, content, padding);

    let mut col = column![]
        .spacing(8)
        .width(QR_FRAME)
        .align_x(Alignment::Center);
    if is_up(snap) && snap.addresses.is_empty() {
        let warning: Element<'static, Msg> = column![
            // `warning` do claro: o quadro e sempre claro, e o laranja do escuro some nele.
            icons::icon(icons::WARNING_CIRCLE, 28.0, theme::LIGHT.warning),
            text("Sem rede local")
                .size(13)
                .font(fonts::INTER_MEDIUM)
                .color(theme::DARK.bg),
        ]
        .spacing(8)
        .align_x(Alignment::Center)
        .into();
        return col.push(frame(warning, 15.0)).into();
    }

    match (&snap.pairing, &app.iced.qr) {
        (Some(pairing), Some(qr)) if pairing.expires_at > now && is_up(snap) => {
            let left = pairing
                .expires_at
                .duration_since(now)
                .unwrap_or(Duration::ZERO)
                .as_secs();
            // O widget do iced desenha a propria zona de silencio (2 modulos); o quadro poe so
            // a borda de 5 px em volta.
            let code: Element<'_, Msg> = qr_code(qr)
                .total_size(QR_SIZE)
                .style(|_: &Theme| qr_code::Style {
                    cell: theme::DARK.bg,
                    background: theme::DARK.neutral_100,
                })
                .into();
            col = col
                .push(frame(code, 5.0))
                .push(Space::new().height(4))
                .push(
                    text(pairing.display.clone())
                        .size(type_scale::VALUE_M)
                        .font(fonts::INTER)
                        .color(t.text),
                )
                .push(
                    text(format!("expira em {}:{:02}", left / 60, left % 60))
                        .size(12)
                        .color(t.text_muted),
                )
                .push(
                    widgets::action_button(
                        t,
                        ButtonKind::Ghost,
                        ButtonSize::Compact,
                        "Gerar outro código",
                    )
                    .on_press(Msg::IssueToken),
                );
        }
        (pairing, _) => {
            col = col
                .push(frame(
                    icons::icon(icons::QR_CODE, 64.0, theme::DARK.neutral_700),
                    15.0,
                ))
                .push(Space::new().height(4));
            // So o que venceu de fato: um codigo valido sem QR (hub fora do ar) nao e expirado.
            if pairing.as_ref().is_some_and(|p| p.expires_at <= now) {
                col = col.push(text("Código expirado").size(12).color(t.warning));
            }
            let mut generate =
                widgets::action_button(t, ButtonKind::Primary, ButtonSize::Regular, "Gerar código");
            if is_up(snap) {
                generate = generate.on_press(Msg::IssueToken);
            }
            col = col.push(generate);
        }
    }
    col.into()
}

/// O quadro do QR (ou do aviso no lugar dele).
fn qr_frame<'a>(t: &'static Tokens, content: Element<'a, Msg>, padding: f32) -> Element<'a, Msg> {
    container(content)
        .width(QR_FRAME)
        .height(QR_FRAME)
        .padding(padding)
        .center_x(QR_FRAME)
        .center_y(QR_FRAME)
        .style(move |_: &Theme| container::Style {
            background: Some(theme::DARK.neutral_100.into()),
            border: Border {
                color: t.accent_700,
                width: size::BORDER,
                radius: radius::DIALOG.into(),
            },
            ..container::Style::default()
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::super::app::Screen;
    use super::super::app::tests::app_with;
    use super::*;
    use crate::hub::snapshot::{DeviceView, Snapshot};

    #[test]
    fn endereco_mais_longo_cabe_inteiro_de_1024_a_2560() {
        let url = "https://192.168.100.200:7777";
        // A moldura validada: o espaco do widget e o que a conta usa sao a mesma constante.
        assert_eq!(FIELD_CHROME, 141.0);
        assert_eq!(widgets::FIELD_SPACING, 10.0);
        for window in 1024..=2560 {
            // Mesma conta de `shell::content_width`; `address_inner` e a que a tela usa, nas
            // duas disposicoes (uma coluna ate 855 de conteudo, duas dai em diante).
            let content = window as f32 - 232.0 - 64.0;
            let inner = address_inner(content);
            let size = field_value_size(url, inner);
            assert!(
                measure::medium_width(url, size) <= inner - FIELD_CHROME,
                "janela {window}: {size} px nao cabe em {inner}"
            );
        }
        // Na janela minima (728 de conteudo) e uma coluna so, com o card na largura toda.
        assert!(!two_columns(728.0));
        assert_eq!(address_inner(728.0), 728.0 - 2.0 * space::CARD);
        assert!(!two_columns(855.0));
        assert!(two_columns(856.0));
        // Com espaco, fica no tamanho do design.
        assert_eq!(field_value_size(url, 600.0), 15.0);
    }

    #[test]
    fn dialogo_nunca_passa_da_janela() {
        assert_eq!(dialog_height(900.0), DIALOG_HEIGHT);
        assert_eq!(dialog_height(600.0), 552.0);
    }

    fn running(devices: Vec<DeviceView>) -> Snapshot {
        Snapshot {
            status: Status::Running,
            addresses: vec![Ipv4Addr::new(192, 168, 0, 12)],
            https_port: 7777,
            http_port: 7778,
            devices,
            ..Snapshot::default()
        }
    }

    fn device(id: &str) -> DeviceView {
        DeviceView {
            device_id: id.into(),
            name: "Pixel 8".into(),
            paired_at: "2026-10-03T10:00:00Z".into(),
            ..DeviceView::default()
        }
    }

    /// So a acao do cabecalho; quando o fluxo ocupa a tela e quando vira dialogo vive em
    /// `ui_model::connect` (`sem_aparelho_o_fluxo_ocupa_a_tela_e_com_aparelho_vira_dialogo`).
    #[test]
    fn acao_do_cabecalho_so_com_aparelho_pareado() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(running(vec![]), dir.path().to_path_buf());
        app.refresh_people();
        app.screen = Screen::Connection;
        assert!(header_action(&app).is_none());
        app.snapshot = running(vec![device("D1")]);
        app.refresh_people();
        assert!(header_action(&app).is_some());
    }

    #[test]
    fn tela_e_dialogo_montam_em_todo_passo_sistema_e_largura() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(running(vec![]), dir.path().to_path_buf());
        app.screen = Screen::Connection;
        for devices in [vec![], vec![device("D1")]] {
            app.snapshot.devices = devices;
            app.refresh_people();
            for width in [1024.0, 1440.0, 2560.0] {
                app.window_size.w = width;
                let _ = view(&app);
                for step in [
                    ConnectStep::WiFi,
                    ConnectStep::Certificate,
                    ConnectStep::Pair,
                ] {
                    for os in [PhoneOs::Android, PhoneOs::Iphone] {
                        for modal in [false, true] {
                            app.connect = Some(ConnectFlow { step, os, modal });
                            let _ = view(&app);
                            assert_eq!(connect_dialog(&app).is_some(), modal);
                        }
                    }
                }
                app.connect = None;
            }
        }
        // Passo 3 com codigo valido (QR, codigo, contagem) e com codigo expirado, na tela e no
        // dialogo: o snapshot passa pelo `update`, que monta o QR.
        for (expires_at, devices) in [
            (SystemTime::now() + Duration::from_secs(300), vec![]),
            (
                SystemTime::now() - Duration::from_secs(1),
                vec![device("D1")],
            ),
        ] {
            let mut snap = running(devices);
            snap.pairing = Some(crate::hub::snapshot::PairingView {
                token: "ABC123".into(),
                display: "ABC-123".into(),
                expires_at,
                qr_url: "http://192.168.0.12:7778/p/ABC123".into(),
            });
            let _ = super::super::app::update(&mut app, Msg::Snapshot(Box::new(snap)));
            assert!(app.iced.qr.is_some());
            app.connect = Some(ConnectFlow {
                step: ConnectStep::Pair,
                ..ConnectFlow::new(!app.snapshot.devices.is_empty())
            });
            let _ = view(&app);
            let _ = connect_dialog(&app);
        }
        // Sem rede local: o passo 3 mostra o aviso no quadro.
        app.snapshot.addresses.clear();
        app.snapshot.status = Status::NoNetwork;
        app.connect = Some(ConnectFlow {
            step: ConnectStep::Pair,
            ..ConnectFlow::new(true)
        });
        assert!(connect_dialog(&app).is_some());
    }
}
