//! Painel "Configurar assistente" da tela Analise. Nesta versao so Nenhum e Local (Ollama): a
//! nuvem aparece desligada ate a Issue 05. Tudo que muda sai como `SaveSettings` para o nucleo;
//! o painel so mostra o que o snapshot diz estar gravado.

use iced::widget::{Space, button, column, container, pick_list, row, text, text_input};
use iced::{Alignment, Element, Length, Padding};

use super::app::{App, Message};
use super::theme::{self, Tokens};
use super::{fonts, widgets};
use crate::assistant::settings::ProviderChoice;

/// Dica da lista vazia; constante para o teste conferir o texto.
pub(super) const NO_MODELS_HINT: &str =
    "Nenhum modelo instalado. Rode `ollama pull llama3.1:8b` e atualize.";

pub(super) const CLOUD_SOON: &str = "na próxima versão";

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
            body = body.push(
                text("O assistente está desligado. A análise acima continua valendo.")
                    .size(13)
                    .color(t.text_muted),
            );
        }
        ProviderChoice::Local => body = body.push(local_section(app)),
        // Sem caminho na UI ate a Issue 05; so chega aqui por um `assistant.json` editado a mao.
        ProviderChoice::Cloud => {
            body = body.push(
                text("A nuvem chega na próxima versão. Escolha Nenhum ou Local.")
                    .size(13)
                    .color(t.text_muted),
            );
        }
    }
    if let Some(err) = &view.last_error {
        body = body.push(text(err.as_str()).size(13).color(t.expense_fg));
    }
    container(body)
        .padding(Padding::from([18, 20]))
        .width(Length::Fill)
        .style(theme::card(t))
        .into()
}

/// Nenhum / Local / Nuvem. `widgets::segmented` so tem duas opcoes, e a terceira precisa
/// aparecer desabilitada com o aviso, entao o trilho e montado aqui com o mesmo estilo.
fn provider_picker(t: &'static Tokens, current: ProviderChoice) -> Element<'static, Message> {
    let option = |label: &'static str, choice: ProviderChoice| {
        segment(
            t,
            label,
            current == choice,
            Some(Message::SettingsProvider(choice)),
        )
    };
    let cloud = row![
        segment(t, "Nuvem (OpenRouter)", false, None),
        text(CLOUD_SOON).size(12).color(t.text_muted),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    let rail = container(
        row![
            option("Nenhum", ProviderChoice::None),
            option("Local (Ollama)", ProviderChoice::Local),
            cloud,
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
    on_press: Option<Message>,
) -> Element<'static, Message> {
    let content = container(text(label).size(13).line_height(1.0))
        .height(36)
        .padding([0, 14])
        .align_y(Alignment::Center);
    let enabled = on_press.is_some();
    button(content)
        .padding(0)
        .style(move |theme: &iced::Theme, status| {
            if enabled {
                theme::segment(t, active)(theme, status)
            } else {
                theme::plain(t.text_muted)(theme, status)
            }
        })
        .on_press_maybe(on_press)
        .into()
}

fn local_section(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let view = &app.snapshot.assistant;
    let port = row![
        text("Porta").size(13).color(t.text_soft()),
        text_input("11434", &app.port_input)
            .on_input(Message::SettingsPort)
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
            section = section.push(
                text("Toque em Atualizar modelos para ver o que o Ollama tem instalado.")
                    .size(13)
                    .color(t.text_muted),
            );
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
                    .width(Length::Fill),
                ]
                .spacing(6),
            );
        }
    }
    section.into()
}
