//! Painel "Configurar assistente" da tela Analise: Nenhum, Local (Ollama) ou Nuvem (OpenRouter).
//! Tudo que muda sai como `SaveSettings` (ou `SaveKey`) para o nucleo; o painel so mostra o que
//! o snapshot diz estar gravado, fora os rascunhos de porta, modelo e chave.

use iced::widget::{Space, button, column, container, pick_list, row, text, text_input};
use iced::{Alignment, Element, Length, Padding};

use super::app::{App, Message};
use super::theme::{self, Tokens};
use super::{fonts, widgets};
use crate::assistant::secret::Secret;
use crate::assistant::settings::ProviderChoice;

/// Dica da lista vazia; constante para o teste conferir o texto.
pub(super) const NO_MODELS_HINT: &str = "Nenhum modelo instalado. Rode ollama pull llama3.1:8b no terminal e clique em Atualizar modelos.";

/// Com o assistente desligado. O painel fica acima dos achados, entao eles estao "abaixo".
pub(super) const ASSISTANT_OFF: &str =
    "O assistente está desligado. Os achados abaixo continuam valendo.";

/// Antes da primeira listagem. "Clique", e nao "Toque": o hub e programa de mesa.
pub(super) const MODELS_NOT_LISTED: &str =
    "Clique em Atualizar modelos para ver o que o Ollama tem instalado.";

/// Explicacao do interruptor da nuvem; constante para o teste conferir o texto.
pub(super) const CLOUD_EXPLAIN: &str = "Ligado, o resumo de achados e totais desta conta vai para o OpenRouter quando você conversar. Lançamentos, descrições, pessoas e datas exatas nunca saem.";

pub(super) const PLAIN_KEY_WARNING: &str =
    "Neste sistema a chave fica em texto puro na pasta de dados do hub.";

pub(super) const DENY_LABEL: &str = "Não permitir que o provedor guarde ou treine com os dados";

pub(super) const DENY_WARNING: &str =
    "Alguns provedores podem guardar ou treinar com o que você enviar.";

pub fn view(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let view = &app.snapshot.assistant;
    let mut body = column![
        text("Configurar assistente")
            .size(16)
            .font(fonts::INTER_SEMIBOLD)
            .color(t.text),
        provider_picker(t, view.settings.provider),
    ]
    .spacing(12);
    match view.settings.provider {
        ProviderChoice::None => {
            body = body.push(text(ASSISTANT_OFF).size(13).color(t.text_muted));
        }
        ProviderChoice::Local => body = body.push(local_section(app)),
        ProviderChoice::Cloud => body = body.push(cloud_section(app)),
    }
    if let Some(err) = &view.last_error {
        body = body.push(text(err.as_str()).size(13).color(t.expense_fg));
    }
    widgets::card(t, body)
        .padding(Padding::from([18, 20]))
        .into()
}

/// Nenhum / Local / Nuvem. `widgets::segmented` so tem duas opcoes, entao o trilho de tres e
/// montado aqui com o mesmo estilo.
fn provider_picker(t: &'static Tokens, current: ProviderChoice) -> Element<'static, Message> {
    let option = |label: &'static str, choice: ProviderChoice| {
        segment(
            t,
            label,
            current == choice,
            Message::SettingsProvider(choice),
        )
    };
    let rail = container(
        row![
            option("Nenhum", ProviderChoice::None),
            option("Local (Ollama)", ProviderChoice::Local),
            option("Nuvem (OpenRouter)", ProviderChoice::Cloud),
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .padding(4)
    .style(theme::fill(t.surface_2, 8.0));
    row![rail, Space::new().width(Length::Fill)].into()
}

fn segment(
    t: &'static Tokens,
    label: &'static str,
    active: bool,
    on_press: Message,
) -> Element<'static, Message> {
    let content = container(text(label).size(13).line_height(1.0))
        .height(36)
        .padding([0, 14])
        .align_y(Alignment::Center);
    button(content)
        .padding(0)
        .style(theme::segment(t, active))
        .on_press(on_press)
        .into()
}

fn local_section(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let view = &app.snapshot.assistant;
    let port = row![
        text("Porta").size(13).color(t.text_soft()),
        text_input("11434", &app.port_input)
            .on_input(Message::SettingsPort)
            .on_submit(Message::SettingsPortSubmit)
            .size(13)
            .width(90)
            .padding([8, 10])
            .style(theme::input(t)),
        Space::new().width(Length::Fill),
        button(text("Atualizar modelos").size(13).font(fonts::INTER_MEDIUM))
            .padding([8, 14])
            .style(theme::outline_divider(t))
            .on_press(Message::RefreshModels),
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    let mut section = column![port].spacing(10);
    match &view.models {
        None => {
            section = section.push(text(MODELS_NOT_LISTED).size(13).color(t.text_muted));
        }
        Some(Err(msg)) => {
            section = section.push(text(msg.as_str()).size(13).color(t.expense_fg));
        }
        Some(Ok(models)) if models.is_empty() => {
            section = section.push(text(NO_MODELS_HINT).size(13).color(t.text_muted));
        }
        Some(Ok(models)) => {
            section = section.push(
                column![
                    widgets::kicker(t, "Modelo"),
                    pick_list(
                        models.clone(),
                        view.settings.local.model.clone(),
                        Message::SettingsLocalModel
                    )
                    .placeholder("Escolha um modelo")
                    .text_size(13)
                    .padding([8, 10])
                    .style(theme::pick(t))
                    .menu_style(theme::pick_menu(t))
                    .width(Length::Fill),
                ]
                .spacing(6),
            );
        }
    }
    section.into()
}

/// Sim / Nao no estilo do hub (`seg_switch`), com o rotulo a esquerda.
fn yes_no<'a>(
    t: &'static Tokens,
    label: &'static str,
    on: bool,
    message: impl Fn(bool) -> Message,
) -> Element<'a, Message> {
    row![
        text(label).size(13).color(t.text).width(Length::Fill),
        container(widgets::seg_switch(
            t,
            [
                (None, "Sim", on, message(true)),
                (None, "Não", !on, message(false)),
            ],
        ))
        .width(140),
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .into()
}

fn cloud_section(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let view = &app.snapshot.assistant;
    let cloud = &view.settings.cloud;
    let mut section = column![
        yes_no(
            t,
            "Enviar para a nuvem",
            cloud.enabled,
            Message::SettingsCloudEnabled
        ),
        text(CLOUD_EXPLAIN)
            .size(13)
            .line_height(1.4)
            .color(t.text_muted),
    ]
    .spacing(8);

    // Chave: salva (so "Remover") ou campo para digitar. O campo e `secure`: a chave nunca
    // aparece na tela.
    let key: Element<'_, Message> = if view.has_key {
        row![
            text("Chave salva").size(13).color(t.text),
            Space::new().width(Length::Fill),
            button(text("Remover").size(13).font(fonts::INTER_MEDIUM))
                .padding([8, 14])
                .style(theme::danger_outline(t))
                .on_press(Message::RemoveKey),
        ]
        .align_y(Alignment::Center)
        .into()
    } else {
        row![
            text_input("sk-or-...", &app.key_input)
                .secure(true)
                .on_input(|k| Message::KeyInput(Secret::new(k)))
                .on_submit(Message::SaveKey)
                .size(13)
                .width(Length::Fill)
                .padding([8, 10])
                .style(theme::input(t)),
            button(text("Salvar").size(13).font(fonts::INTER_MEDIUM))
                .padding([8, 14])
                .style(theme::outline_accent(t))
                .on_press_maybe((!app.key_input.trim().is_empty()).then_some(Message::SaveKey)),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .into()
    };
    section = section.push(column![widgets::kicker(t, "Chave do OpenRouter"), key].spacing(6));
    if !view.key_encrypted {
        section = section.push(text(PLAIN_KEY_WARNING).size(13).color(t.warning));
    }

    section = section.push(
        column![
            widgets::kicker(t, "Modelo"),
            text_input(
                crate::assistant::settings::SUGGESTED_CLOUD_MODEL,
                &app.model_input
            )
            .on_input(Message::SettingsCloudModel)
            .on_submit(Message::SettingsCloudModelSubmit)
            .size(13)
            .width(Length::Fill)
            .padding([8, 10])
            .style(theme::input(t)),
        ]
        .spacing(6),
    );

    section = section.push(yes_no(
        t,
        DENY_LABEL,
        cloud.deny_data_collection,
        Message::SettingsDeny,
    ));
    if !cloud.deny_data_collection {
        section = section.push(text(DENY_WARNING).size(13).color(t.warning));
    }
    section.into()
}
