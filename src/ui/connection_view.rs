//! Tela Conexao: endereco com QR e codigo de pareamento, usuarios conectados, guia do
//! certificado e atividade do hub.
//!
//! O QR e o da fatia 1 (`http://<ip>:7778/p/<token>`, so com token ativo), nao o endereco
//! puro do prototipo: um QR so com o endereco levaria o celular a uma conexao que ele nao pode
//! confiar (sem CA) nem autenticar (sem token).

use std::net::Ipv4Addr;
use std::time::{Duration, SystemTime};

use iced::widget::text::Wrapping;
use iced::widget::{
    Space, button, column, container, mouse_area, qr_code, rich_text, row, span, stack, text,
};
use iced::{Alignment, Border, Element, Length, Padding, Theme};

use super::app::{App, CopyTarget, Message};
use super::theme::{self, Tokens};
use super::{fonts, icons, shell, time, widgets};
use crate::dashboard::people::{Person, presence};
use crate::dashboard::periods::paired_label;
use crate::hub::snapshot::{ActivityKind, Snapshot, Status};

/// Largura minima de cada card da grade antes de empilhar (`minmax(420px, 1fr)`).
const GRID_MIN: f32 = 420.0;
const GAP: f32 = 14.0;
/// QR 168 + zona de silencio 14 de cada lado + contorno de 1 px.
const QR_FRAME: f32 = 198.0;
/// Coluna do endereco: minimo de 280 antes de ir para baixo do QR.
const INFO_MIN: f32 = 280.0;

/// `https://<ip>:<porta>`: o que se digita no app. Sem rede, nada.
pub fn hub_url(snap: &Snapshot) -> Option<String> {
    snap.addresses
        .first()
        .map(|ip| format!("https://{ip}:{}", snap.https_port))
}

/// Link direto da CA na porta HTTP.
pub fn ca_url(snap: &Snapshot) -> Option<String> {
    guide_url(snap).map(|g| format!("{g}/ca.crt"))
}

fn guide_url(snap: &Snapshot) -> Option<String> {
    snap.addresses
        .first()
        .map(|ip| format!("http://{ip}:{}", snap.http_port))
}

fn is_up(snap: &Snapshot) -> bool {
    matches!(snap.status, Status::Running | Status::NoNetwork)
}

pub fn view(app: &App) -> Element<'_, Message> {
    let width = shell::content_width(app);
    let two_columns = width >= 2.0 * GRID_MIN + GAP;
    let card_width = if two_columns {
        (width - GAP) / 2.0
    } else {
        width
    };
    let grid: Element<'_, Message> = if two_columns {
        row![
            container(address_card(app, card_width)).width(Length::FillPortion(1)),
            container(users_card(app)).width(Length::FillPortion(1)),
        ]
        .spacing(GAP)
        .align_y(Alignment::Start)
        .into()
    } else {
        column![address_card(app, card_width), users_card(app)]
            .spacing(GAP)
            .into()
    };
    column![grid, certificate_card(app, width), activity_card(app)]
        .spacing(GAP)
        .width(Length::Fill)
        .into()
}

// ---- card do endereco ----

fn address_card(app: &App, card_width: f32) -> Element<'_, Message> {
    let t = app.tokens;
    let inner = card_width - 48.0;
    let side_by_side = inner >= QR_FRAME + 24.0 + INFO_MIN;
    let body: Element<'_, Message> = if side_by_side {
        row![pairing_block(app), address_info(app)]
            .spacing(24)
            .align_y(Alignment::Start)
            .into()
    } else {
        column![pairing_block(app), address_info(app)]
            .spacing(24)
            .into()
    };
    container(body)
        .padding(24)
        .width(Length::Fill)
        .style(theme::card(t))
        .into()
}

/// Quadro do QR (fundo `neutral_100`, raio 12, contorno `accent_700`) e o que fica embaixo:
/// codigo e contagem com token ativo, "Gerar codigo" sem ele.
fn pairing_block(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let snap = &app.snapshot;
    let now = SystemTime::now();
    let frame = |content: Element<'static, Message>, padding: f32| {
        container(content)
            .width(QR_FRAME)
            .height(QR_FRAME)
            .padding(padding)
            .center_x(QR_FRAME)
            .center_y(QR_FRAME)
            .style(move |_: &Theme| container::Style {
                background: Some(t.neutral_100.into()),
                border: Border {
                    color: t.accent_700,
                    width: 1.0,
                    radius: 12.0.into(),
                },
                ..container::Style::default()
            })
    };
    let placeholder =
        || -> Element<'static, Message> { icons::icon(icons::QR_CODE, 64.0, t.neutral_700) };

    let mut col = column![]
        .spacing(8)
        .width(QR_FRAME)
        .align_x(Alignment::Center);
    if is_up(snap) && snap.addresses.is_empty() {
        let warning: Element<'static, Message> = column![
            icons::icon(icons::WARNING_CIRCLE, 28.0, t.amber),
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

    match (&snap.pairing, &app.qr) {
        (Some(pairing), Some(qr)) if pairing.expires_at > now && is_up(snap) => {
            let left = pairing
                .expires_at
                .duration_since(now)
                .unwrap_or(Duration::ZERO)
                .as_secs();
            // O widget do iced desenha a propria zona de silencio (2 modulos); o quadro poe so
            // o que falta para os modulos ocuparem ~168 px, como a imagem do prototipo.
            let code = container(
                qr_code(qr)
                    .total_size(188)
                    .style(|_: &Theme| qr_code::Style {
                        cell: theme::DARK.bg,
                        background: theme::DARK.neutral_100,
                    }),
            )
            .width(QR_FRAME)
            .height(QR_FRAME)
            .padding(5)
            .center_x(QR_FRAME)
            .center_y(QR_FRAME)
            .style(move |_: &Theme| container::Style {
                background: Some(t.neutral_100.into()),
                border: Border {
                    color: t.accent_700,
                    width: 1.0,
                    radius: 12.0.into(),
                },
                ..container::Style::default()
            });
            col = col
                .push(code)
                .push(Space::new().height(4))
                .push(
                    text(pairing.display.clone())
                        .size(20)
                        .font(fonts::INTER_MEDIUM)
                        .color(t.text),
                )
                .push(
                    text(format!("expira em {}:{:02}", left / 60, left % 60))
                        .size(12)
                        .color(t.text_alpha(0.55)),
                )
                .push(
                    button(
                        text("Gerar outro código")
                            .size(13)
                            .font(fonts::INTER_MEDIUM),
                    )
                    .padding([6, 10])
                    .style(theme::ghost(t))
                    .on_press(Message::IssueToken),
                );
        }
        (pairing, _) => {
            col = col
                .push(frame(placeholder(), 15.0))
                .push(Space::new().height(4));
            if pairing.is_some() {
                col = col.push(text("Código expirado").size(12).color(t.amber));
            }
            let mut generate = button(
                container(text("Gerar código").size(13).font(fonts::INTER_MEDIUM))
                    .height(Length::Fill)
                    .align_y(Alignment::Center),
            )
            .height(36)
            .padding([0, 14])
            .style(theme::outline_accent(t));
            if is_up(snap) {
                generate = generate.on_press(Message::IssueToken);
            }
            col = col.push(generate);
        }
    }
    col.into()
}

fn address_info(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let snap = &app.snapshot;
    let mut col = column![
        widgets::kicker_accent(t, "Endereço do hub"),
        Space::new().height(10),
        field(
            app,
            icons::LOCK_SIMPLE,
            t.income_fg,
            hub_url(snap),
            CopyTarget::Address
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
                    .color(t.text_alpha(0.55)),
            )
            .padding(Padding {
                top: 8.0,
                ..Padding::ZERO
            }),
        );
    }
    let cert_step = rich_text::<(), _, _, _>([
        span("Instale o certificado do hub no celular (uma vez). "),
        span("Ver como ↓").color(t.accent_300),
    ])
    .size(13)
    .line_height(1.45)
    .color(t.text_alpha(0.75));
    let steps = column![
        widgets::step(
            t,
            1,
            widgets::step_text(t, "Conecte o celular no mesmo Wi-Fi deste computador.").into()
        ),
        widgets::step(t, 2, cert_step.into()),
        widgets::step(
            t,
            3,
            widgets::step_text(
                t,
                "Aponte a câmera para o QR code ou digite o endereço e o código no app."
            )
            .into()
        ),
        widgets::step(
            t,
            4,
            widgets::step_text(t, "No app, toque em Ajustes → Sincronizar com o hub.").into()
        ),
    ]
    .spacing(10);
    let footer = row![
        icons::icon(icons::HOUSE_SIMPLE, 12.0, t.text_alpha(0.55)),
        text("Funciona só na rede de casa. Nada passa pela internet.")
            .size(12)
            .color(t.text_alpha(0.55)),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    col.push(Space::new().height(18))
        .push(steps)
        .push(Space::new().height(16))
        .push(footer)
        .width(Length::Fill)
        .into()
}

/// Campo de 44 px com icone, valor (cortado se nao couber) e "Copiar".
fn field<'a>(
    app: &'a App,
    glyph: &'static str,
    glyph_color: iced::Color,
    value: Option<String>,
    target: CopyTarget,
) -> Element<'a, Message> {
    let t = app.tokens;
    let enabled = value.is_some();
    let shown = match value {
        Some(v) => text(v).size(15).font(fonts::INTER_MEDIUM).color(t.text),
        None => text("sem endereço de rede")
            .size(15)
            .color(t.text_alpha(0.55)),
    };
    // O iced 0.14 nao tem reticencias: texto que nao cabe e cortado na borda do campo.
    let value = container(shown.wrapping(Wrapping::None))
        .width(Length::Fill)
        .clip(true);
    let copied = app.is_copied(target);
    let mut copy = button(
        container(
            row![
                icons::icon_inherit(if copied { icons::CHECK } else { icons::COPY }, 12.0),
                text(if copied { "Copiado" } else { "Copiar" })
                    .size(12)
                    .font(fonts::INTER_MEDIUM),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .height(Length::Fill)
        .align_y(Alignment::Center),
    )
    .height(32)
    .padding([0, 10])
    .style(theme::ghost(t));
    if enabled {
        copy = copy.on_press(Message::Copy(target));
    }
    container(
        row![icons::icon(glyph, 15.0, glyph_color), value, copy]
            .spacing(10)
            .align_y(Alignment::Center),
    )
    .height(44)
    .width(Length::Fill)
    .padding(Padding {
        top: 0.0,
        right: 6.0,
        bottom: 0.0,
        left: 14.0,
    })
    .align_y(Alignment::Center)
    .style(theme::field(t))
    .into()
}

// ---- usuarios conectados ----

fn users_card(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let count = app.visible_people().count();
    let title = container(widgets::card_title(
        t,
        "Usuários conectados",
        Some(shell::plural(count, "pessoa", "pessoas")),
    ))
    .padding([0, 8]);
    let mut col = column![title, Space::new().height(8)];
    if count == 0 {
        col = col.push(empty_users(t));
    } else {
        let now = SystemTime::now();
        for person in app.visible_people() {
            col = col
                .push(widgets::hdivider(t))
                .push(person_row(app, person, now));
        }
    }
    // Padding lateral de 12 (e nao 20) para o hover da linha passar 8 px do conteudo, como o
    // `margin: 0 -8px` do prototipo.
    container(col)
        .padding(Padding {
            top: 18.0,
            right: 12.0,
            bottom: 8.0,
            left: 12.0,
        })
        .width(Length::Fill)
        .style(theme::card(t))
        .into()
}

fn person_row<'a>(app: &'a App, person: &'a Person, now: SystemTime) -> Element<'a, Message> {
    let t = app.tokens;
    let photo = app.avatars.get(&person.device_id).map(|a| &a.handle);
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
    let dot = if seen.is_online() {
        t.income
    } else {
        t.neutral_600
    };
    let muted = t.text_alpha(0.55);
    let meta = row![
        row![
            icons::icon(icons::DEVICE_MOBILE, 12.0, muted),
            text(person.device_name.clone()).size(12).color(muted),
        ]
        .spacing(5)
        .align_y(Alignment::Center),
        text(format!(
            "Conectado em {}",
            paired_label(&person.paired_at, app.offset)
        ))
        .size(12)
        .color(muted),
        row![
            widgets::dot(dot, 6.0),
            text(seen.label(person.user_known)).size(12).color(muted),
        ]
        .spacing(5)
        .align_y(Alignment::Center),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .wrap()
    .vertical_spacing(4);

    let remove = button(
        container(
            row![
                icons::icon_inherit(icons::USER_MINUS, 13.0),
                text("Remover").size(13).font(fonts::INTER_MEDIUM),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .height(Length::Fill)
        .align_y(Alignment::Center),
    )
    .height(32)
    .padding([0, 12])
    .style(theme::danger_outline(t))
    .on_press(Message::AskRemove(person.device_id.clone()));

    let content = row![
        // Altura minima da linha (68 com o padding): o iced nao tem `min-height`.
        Space::new().width(0).height(48),
        avatar,
        column![name, meta].spacing(3).width(Length::Fill),
        remove,
    ]
    .spacing(14)
    .align_y(Alignment::Center);

    let hovered = app.hover_row.as_deref() == Some(person.device_id.as_str());
    let body = container(content)
        .padding([10, 8])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: hovered.then(|| t.text_alpha(0.04).into()),
            border: Border {
                radius: 8.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    mouse_area(body)
        .on_enter(Message::HoverRow(person.device_id.clone()))
        .on_exit(Message::UnhoverRow(person.device_id.clone()))
        .into()
}

fn empty_users<'a>(t: &'static Tokens) -> Element<'a, Message> {
    // Borda solida: o iced nao traceja borda de container (desvio aceito na spec).
    let circle = move |alpha: f32| {
        container(Space::new())
            .width(36)
            .height(36)
            .style(move |_: &Theme| container::Style {
                border: Border {
                    color: theme::alpha(t.neutral_700, alpha),
                    width: 1.5,
                    radius: 18.0.into(),
                },
                ..container::Style::default()
            })
    };
    let circles = stack![
        circle(1.0),
        container(circle(0.6)).padding(Padding {
            left: 26.0,
            ..Padding::ZERO
        }),
    ]
    .width(62)
    .height(36);
    let body = column![
        circles,
        Space::new().height(6),
        text("Nenhum usuário conectado")
            .size(15)
            .font(fonts::INTER_MEDIUM)
            .color(t.text),
        container(
            text(
                "Quando um celular abrir o endereço ao lado, a pessoa aparece aqui com o nome e \
                 a cor que escolheu no app."
            )
            .size(13)
            .line_height(1.5)
            .color(t.text_alpha(0.60))
            .align_x(Alignment::Center),
        )
        .max_width(320),
    ]
    .spacing(8)
    .align_x(Alignment::Center);
    column![
        widgets::hdivider(t),
        container(body)
            .padding(Padding {
                top: 36.0,
                right: 8.0,
                bottom: 32.0,
                left: 8.0,
            })
            .center_x(Length::Fill),
    ]
    .into()
}

// ---- certificado ----

fn certificate_card(app: &App, width: f32) -> Element<'_, Message> {
    let t = app.tokens;
    let snap = &app.snapshot;
    let guide = guide_url(snap).unwrap_or_else(|| "http://<ip-do-computador>:7778".into());
    let step = |n: u8, s: String| widgets::step(t, n, widgets::step_text(t, s).into());
    let android = column![
        text("Android")
            .size(14)
            .font(fonts::INTER_MEDIUM)
            .color(t.text),
        step(
            1,
            format!("No celular, abra {guide} e toque em Baixar certificado.")
        ),
        step(
            2,
            "Configurações → Segurança → Criptografia e credenciais → Instalar um certificado → \
             Certificado de CA → Instalar mesmo assim → escolha HubFinance-CA.crt."
                .into()
        ),
        step(
            3,
            "Abra o HomeFinance e leia o QR (ou digite o endereço e o código).".into()
        ),
    ]
    .spacing(10)
    .width(Length::Fill);
    let iphone = column![
        text("iPhone")
            .size(14)
            .font(fonts::INTER_MEDIUM)
            .color(t.text),
        step(
            1,
            format!("No Safari, abra {guide} e toque em Baixar certificado → Permitir.")
        ),
        step(
            2,
            "Ajustes → Geral → VPN e Gerenciamento de Dispositivo → HubFinance CA → Instalar."
                .into()
        ),
        step(
            3,
            "Ajustes → Geral → Sobre → Ajustes de Confiança de Certificado → ative HubFinance CA."
                .into()
        ),
        step(4, "Abra o HomeFinance e leia o QR.".into()),
    ]
    .spacing(10)
    .width(Length::Fill);
    // Duas colunas so com espaco para as duas (>= 280 cada); senao, uma embaixo da outra.
    let guides: Element<'_, Message> = if width - 48.0 >= 2.0 * 280.0 + 32.0 {
        row![android, iphone]
            .spacing(32)
            .align_y(Alignment::Start)
            .into()
    } else {
        column![android, iphone].spacing(20).into()
    };
    container(
        column![
            widgets::kicker_accent(t, "Certificado do hub"),
            Space::new().height(10),
            text(
                "Uma vez por celular: o app só confia na conexão depois que o certificado do hub \
                 está instalado."
            )
            .size(13)
            .line_height(1.45)
            .color(t.text_alpha(0.75)),
            Space::new().height(14),
            field(
                app,
                icons::CERTIFICATE,
                t.accent_300,
                ca_url(snap),
                CopyTarget::CaLink
            ),
            Space::new().height(20),
            guides,
        ]
        .width(Length::Fill),
    )
    .padding(24)
    .width(Length::Fill)
    .style(theme::card(t))
    .into()
}

// ---- atividade ----

/// O "status detalhado" que o design nao desenhou, reduzido ao essencial.
fn activity_card(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let snap = &app.snapshot;
    let mut col = column![
        widgets::card_title(t, "Atividade", Some("últimas 20".into())),
        Space::new().height(8),
    ];
    if snap.activity.is_empty() {
        col =
            col.push(container(text("Nada ainda.").size(13).color(t.text_alpha(0.55))).height(28));
    }
    for entry in snap.activity.iter().take(20) {
        let at = time::format_time(::time::OffsetDateTime::from(entry.at), app.offset);
        let color = match entry.kind {
            ActivityKind::Warning => t.amber,
            ActivityKind::Error => t.expense_fg,
            ActivityKind::Info | ActivityKind::Sync => t.text,
        };
        col = col.push(
            row![
                text(at).size(12).color(t.text_alpha(0.50)).width(44),
                text(entry.text.clone()).size(13).color(color),
            ]
            .spacing(10)
            .height(28)
            .align_y(Alignment::Center),
        );
    }
    let epoch: String = snap.epoch.chars().take(8).collect();
    col = col.push(
        container(
            text(format!("epoch {epoch} · {}", snap.data_dir.display()))
                .size(11)
                .color(t.text_alpha(0.45)),
        )
        .padding(Padding {
            top: 10.0,
            right: 0.0,
            bottom: 10.0,
            left: 0.0,
        }),
    );
    container(col)
        .padding(Padding {
            top: 18.0,
            right: 20.0,
            bottom: 8.0,
            left: 20.0,
        })
        .width(Length::Fill)
        .style(theme::card(t))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enderecos_so_com_rede() {
        let mut snap = Snapshot {
            https_port: 7777,
            http_port: 7778,
            ..Snapshot::default()
        };
        assert_eq!(hub_url(&snap), None);
        assert_eq!(ca_url(&snap), None);
        snap.addresses = vec![Ipv4Addr::new(192, 168, 0, 12)];
        assert_eq!(hub_url(&snap).as_deref(), Some("https://192.168.0.12:7777"));
        assert_eq!(
            ca_url(&snap).as_deref(),
            Some("http://192.168.0.12:7778/ca.crt")
        );
    }
}
