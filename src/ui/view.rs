//! Desenho da janela: uma coluna rolavel com as cinco secoes da spec.

use std::net::Ipv4Addr;
use std::time::{Duration, SystemTime};

use iced::widget::{Column, button, column, container, qr_code, row, scrollable, text};
use iced::{Alignment, Color, Element, Font, Length};
use time::format_description::well_known::Rfc3339;
use time::macros::format_description;
use time::{OffsetDateTime, UtcOffset};

use super::app::{App, Message};
use super::fonts;
use crate::hub::snapshot::{ActivityKind, DeviceView, Snapshot, Status};

const ERROR: Color = Color::from_rgb(0.95, 0.4, 0.4);
const WARNING: Color = Color::from_rgb(0.95, 0.75, 0.3);
const MUTED: Color = Color::from_rgb(0.65, 0.65, 0.7);
const OK: Color = Color::from_rgb(0.45, 0.85, 0.55);

const BOLD: Font = fonts::INTER_SEMIBOLD;

pub fn view(app: &App) -> Element<'_, Message> {
    let snap = &app.snapshot;
    let content = column![
        header(snap),
        pairing_section(app),
        certificate_section(snap),
        devices_section(app),
        activity_section(app),
    ]
    .spacing(28)
    .padding(20)
    .width(Length::Fill);
    scrollable(content).height(Length::Fill).into()
}

fn title(label: &str) -> Element<'_, Message> {
    text(label).size(20).font(fonts::INTER_MEDIUM).into()
}

fn small<'a>(content: impl Into<String>) -> iced::widget::Text<'a> {
    text(content.into()).size(13).color(MUTED)
}

fn is_up(snap: &Snapshot) -> bool {
    matches!(snap.status, Status::Running | Status::NoNetwork)
}

/// Endereco que vai no tutorial e no QR; sem rede, um marcador em vez de um IP inventado.
fn host(snap: &Snapshot) -> String {
    snap.addresses
        .first()
        .map_or_else(|| "<ip-do-computador>".to_string(), Ipv4Addr::to_string)
}

fn header(snap: &Snapshot) -> Element<'_, Message> {
    let (status, color) = match &snap.status {
        Status::Starting => ("Iniciando…".to_string(), MUTED),
        Status::Running => (
            format!("Ligado em https://{}:{}", host(snap), snap.https_port),
            OK,
        ),
        Status::NoNetwork => (
            format!("Sem rede local (porta {} aberta)", snap.https_port),
            WARNING,
        ),
        Status::Stopping => ("Desligando…".to_string(), MUTED),
        Status::Stopped => ("Desligado".to_string(), MUTED),
        Status::Failed(msg) => (format!("Erro: {msg}"), ERROR),
    };
    let mut col = column![
        text("HubFinance").size(30).font(BOLD),
        text(status).size(16).color(color),
    ]
    .spacing(4);
    if snap.addresses.len() > 1 {
        let others: Vec<String> = snap.addresses[1..]
            .iter()
            .map(Ipv4Addr::to_string)
            .collect();
        col = col.push(small(format!("Outros endereços: {}", others.join(", "))));
    }
    let epoch: String = snap.epoch.chars().take(8).collect();
    if !epoch.is_empty() {
        col = col.push(small(format!("epoch {epoch}")));
    }
    col = col.push(small(format!("Dados: {}", snap.data_dir.display())));
    col.into()
}

fn pairing_section(app: &App) -> Element<'_, Message> {
    let snap = &app.snapshot;
    let mut col = Column::new().spacing(10).push(title("Parear aparelho"));
    if !is_up(snap) {
        return col
            .push(small("Disponível quando o hub estiver ligado."))
            .into();
    }
    if snap.addresses.is_empty() {
        return col
            .push(small(
                "Sem rede local: conecte o computador ao Wi-Fi ou à rede de casa. O hub tenta de \
                 novo a cada 30 s.",
            ))
            .into();
    }

    let now = SystemTime::now();
    match (&snap.pairing, &app.qr) {
        (Some(pairing), Some(qr)) if pairing.expires_at > now => {
            let left = pairing
                .expires_at
                .duration_since(now)
                .unwrap_or(Duration::ZERO)
                .as_secs();
            col = col
                .push(container(qr_code(qr).total_size(220)))
                .push(text(&pairing.display).size(34).font(BOLD))
                .push(text(format!("expira em {}:{:02}", left / 60, left % 60)).size(14))
                .push(small(format!(
                    "ou digite {}:{} no app e o código acima",
                    host(snap),
                    snap.https_port
                )))
                .push(button("Gerar outro código").on_press(Message::IssueToken));
        }
        (Some(_), _) => {
            col = col
                .push(text("Código expirado").color(WARNING))
                .push(button("Gerar código").on_press(Message::IssueToken));
        }
        (None, _) => {
            col = col
                .push(small(
                    "Gera um código de uso único, válido por 5 minutos, e o QR que leva o \
                     celular ao guia do certificado e ao app.",
                ))
                .push(button("Gerar código").on_press(Message::IssueToken));
        }
    }
    col.into()
}

fn certificate_section(snap: &Snapshot) -> Element<'_, Message> {
    let ip = host(snap);
    let guide = format!("http://{ip}:{}", snap.http_port);
    column![
        title("Certificado"),
        small("Uma vez por celular: o certificado do hub precisa ser instalado para o app confiar na conexão."),
        text(format!("Link direto: {guide}/ca.crt")).size(14),
        text("Android").font(BOLD),
        text(format!(
            "1. No celular, abra {guide} e toque em Baixar certificado.\n\
             2. Configurações → Segurança → Criptografia e credenciais → Instalar um certificado \
             → Certificado de CA → Instalar mesmo assim → escolha HubFinance-CA.crt.\n\
             3. Abra o HomeFinance e leia o QR (ou digite o endereço e o código)."
        ))
        .size(14),
        text("iPhone").font(BOLD),
        text(format!(
            "1. No Safari, abra {guide} e toque em Baixar certificado → Permitir.\n\
             2. Ajustes → Geral → VPN e Gerenciamento de Dispositivo → HubFinance CA → Instalar.\n\
             3. Ajustes → Geral → Sobre → Ajustes de Confiança de Certificado → ative HubFinance CA.\n\
             4. Abra o HomeFinance e leia o QR."
        ))
        .size(14),
    ]
    .spacing(8)
    .into()
}

fn devices_section(app: &App) -> Element<'_, Message> {
    let mut col = Column::new().spacing(12).push(title("Aparelhos"));
    if app.snapshot.devices.is_empty() {
        return col.push(small("Nenhum aparelho pareado.")).into();
    }
    for device in &app.snapshot.devices {
        col = col.push(device_row(device, app.offset));
    }
    col.into()
}

fn device_row(device: &DeviceView, offset: UtcOffset) -> Element<'_, Message> {
    let short_id: String = device.device_id.chars().take(10).collect();
    let paired = parse(&device.paired_at)
        .map(|t| format_date(t, offset))
        .unwrap_or_else(|| device.paired_at.clone());
    let info = column![
        text(&device.name).size(16).font(BOLD),
        small(format!("{short_id}… · pareado em {paired}")),
        small(format!(
            "último envio {} · recebimento {}",
            relative(device.last_push_at.as_deref()),
            relative(device.last_pull_at.as_deref())
        )),
    ]
    .spacing(2)
    .width(Length::Fill);
    let action: Element<'_, Message> = if device.revoked {
        text("Revogado").color(MUTED).into()
    } else {
        button("Revogar")
            .style(button::danger)
            .on_press(Message::Revoke(device.device_id.clone()))
            .into()
    };
    row![info, action]
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
}

fn activity_section(app: &App) -> Element<'_, Message> {
    let mut col = Column::new().spacing(4).push(title("Atividade"));
    if app.snapshot.activity.is_empty() {
        return col.push(small("Nada ainda.")).into();
    }
    for entry in &app.snapshot.activity {
        let at = format_time(OffsetDateTime::from(entry.at), app.offset);
        let line = text(format!("{at}  {}", entry.text)).size(14);
        let line = match entry.kind {
            ActivityKind::Error => line.color(ERROR),
            ActivityKind::Warning => line.color(WARNING),
            ActivityKind::Info | ActivityKind::Sync => line,
        };
        col = col.push(line);
    }
    col.into()
}

fn parse(rfc3339: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(rfc3339, &Rfc3339).ok()
}

fn format_date(t: OffsetDateTime, offset: UtcOffset) -> String {
    t.to_offset(offset)
        .format(format_description!("[day]/[month]/[year]"))
        .unwrap_or_default()
}

fn format_time(t: OffsetDateTime, offset: UtcOffset) -> String {
    t.to_offset(offset)
        .format(format_description!("[hour]:[minute]"))
        .unwrap_or_default()
}

/// "há 3 min". `SystemTime::now()` aqui e so exibicao, nunca decisao.
fn relative(rfc3339: Option<&str>) -> String {
    let Some(t) = rfc3339.and_then(parse) else {
        return "nunca".to_string();
    };
    let secs = (OffsetDateTime::now_utc() - t).whole_seconds().max(0);
    match secs {
        0..60 => "agora há pouco".to_string(),
        60..3600 => format!("há {} min", secs / 60),
        3600..86_400 => format!("há {} h", secs / 3600),
        _ => format!("há {} dias", secs / 86_400),
    }
}
