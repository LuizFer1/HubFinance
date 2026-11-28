//! Dialogo "Configurar assistente" (spec Grafite · Mesa 8.6), no padrao de formulario em passos
//! do produto: 1) onde roda (Desligado, No meu computador, Na nuvem); 2) os detalhes do
//! escolhido. Nada e gravado antes do ultimo botao, que nomeia a acao ("Usar Ollama local"):
//! com "Cancelar" ao lado, gravar a cada clique faria o cancelar mentir, e escolher "Na nuvem"
//! ja ligaria a nuvem. O ultimo botao manda um `Configure` so (chave e configuracao juntas) e o
//! dialogo espera o nucleo: fecha se deu certo, mostra o erro se nao deu.

use iced::widget::{
    Space, button, column, container, pick_list, row, scrollable, text, text_input,
};
use iced::{Alignment, Element, Length};

use super::app::{App, Message};
use super::theme::{self, ButtonKind, ButtonSize, Tokens, Tone, space};
use super::{fonts, icons, widgets};
use crate::assistant::secret::Secret;
use crate::assistant::settings::ProviderChoice;
use crate::ui_model::assistant_setup::{
    AssistantSetup, CHOICES, CLOUD_EXPLAIN, DENY_LABEL, DENY_WARNING, MODELS_NOT_LISTED,
    NO_MODELS_HINT, PLAIN_KEY_WARNING, SetupStep, finishes_here, primary_label, settings_to_save,
    step_caption,
};

/// Com mais modelos que isto a lista de radio ficaria mais alta que o dialogo: vira o
/// `pick_list` estilizado.
const RADIO_MAX: usize = 6;

/// Largura do dialogo.
const DIALOG_WIDTH: f32 = 560.0;

/// Altura do passo 2 (o da nuvem e o mais alto, e a lista de modelos cresce): o corpo rola e
/// cabecalho, progresso e rodape ficam fixos. Numa janela mais baixa, a janela menos 24 de
/// cada lado.
const DIALOG_HEIGHT: f32 = 600.0;

pub(super) fn dialog_height(window_height: f32) -> f32 {
    DIALOG_HEIGHT.min(window_height - 48.0).max(200.0)
}

/// O ultimo botao pode gravar? Ollama precisa de porta valida e de um modelo da lista que o
/// teste trouxe **desta** porta (modelo de outra porta, ou de uma lista antiga, falharia na
/// primeira pergunta); a nuvem, de modelo e de uma chave (a salva ou a digitada agora, que nao
/// conta se for so espaco).
pub(super) fn can_finish(app: &App, setup: &AssistantSetup) -> bool {
    if setup.saving.is_some() {
        return false;
    }
    let view = &app.snapshot.assistant;
    let ok = match setup.draft.choice {
        ProviderChoice::None => true,
        ProviderChoice::Local => {
            let port = setup.draft.port();
            let listed = match &view.models {
                Some(Ok(models)) => setup
                    .draft
                    .local_model
                    .as_ref()
                    .is_some_and(|m| models.contains(m)),
                _ => false,
            };
            port.is_some() && view.models_port == port && listed
        }
        ProviderChoice::Cloud => view.has_key || !app.key_input.trim().is_empty(),
    };
    ok && settings_to_save(&view.settings, &setup.draft).is_some()
}

pub(super) fn dialog(app: &App) -> Option<Element<'_, Message>> {
    let setup = app.assistant_setup.as_ref()?;
    let t = app.iced.tokens;
    let header = row![
        text("Configurar assistente")
            .size(18)
            .font(fonts::INTER_MEDIUM)
            .color(t.text)
            .width(Length::Fill),
        widgets::icon_button(
            t,
            icons::X,
            ButtonSize::Compact,
            "Fechar",
            Some(Message::CloseAssistantSetup),
        ),
    ]
    .align_y(Alignment::Center);
    let step_index = usize::from(setup.step == SetupStep::Details);
    let progress = row![
        container(widgets::progress_steps(t, step_index, 2)).width(96),
        text(step_caption(setup)).size(12).color(t.text_muted),
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    let body = match (setup.step, setup.draft.choice) {
        (SetupStep::Details, ProviderChoice::Local) => local_step(app, setup),
        (SetupStep::Details, ProviderChoice::Cloud) => cloud_step(app, setup),
        _ => where_step(app, setup),
    };
    let details = setup.step == SetupStep::Details;
    // No passo 2 o corpo rola dentro da altura fixa; no passo 1 (tres linhas) cabe sempre.
    let body: Element<'_, Message> = if details {
        scrollable(container(body).padding(iced::Padding {
            right: 12.0,
            ..iced::Padding::ZERO
        }))
        .direction(scrollable::Direction::Vertical(
            scrollable::Scrollbar::new().width(6).scroller_width(6),
        ))
        .style(theme::scrollbar(t))
        .height(Length::Fill)
        .into()
    } else {
        body
    };
    let mut col = column![header, progress, body].spacing(16);
    if details {
        col = col.height(Length::Fill);
    }
    let error = setup
        .error
        .as_deref()
        .or(app.snapshot.assistant.last_error.as_deref());
    if let Some(err) = error {
        col = col.push(widgets::banner(
            t,
            Tone::Expense,
            t.surface,
            icons::WARNING_CIRCLE,
            err,
            Vec::new(),
        ));
    }
    let secondary = if setup.step == SetupStep::Where {
        widgets::action_button(t, ButtonKind::Secondary, ButtonSize::Regular, "Cancelar")
            .on_press(Message::CloseAssistantSetup)
    } else {
        widgets::action_button(t, ButtonKind::Secondary, ButtonSize::Regular, "Voltar")
            .on_press_maybe(setup.saving.is_none().then_some(Message::SetupBack))
    };
    let primary_message = if finishes_here(setup) {
        can_finish(app, setup).then_some(Message::SetupFinish)
    } else {
        Some(Message::SetupNext)
    };
    let primary = widgets::action_button(
        t,
        ButtonKind::Primary,
        ButtonSize::Regular,
        primary_label(setup),
    )
    .on_press_maybe(primary_message);
    col = col.push(
        row![Space::new().width(Length::Fill), secondary, primary]
            .spacing(8)
            .align_y(Alignment::Center),
    );
    Some(widgets::dialog(
        t,
        col,
        app.window_size.w,
        DIALOG_WIDTH,
        details.then(|| dialog_height(app.window_size.h)),
        Message::CloseAssistantSetup,
        app.motion.scrim(),
    ))
}

/// Linha de radio: o circulo, o rotulo e a descricao (opcional) ao lado.
fn radio_row<'a>(
    t: &'static Tokens,
    label: &'a str,
    detail: Option<&'a str>,
    selected: bool,
    on_press: Message,
) -> Element<'a, Message> {
    let ring = if selected {
        t.accent
    } else {
        t.border_control()
    };
    let inner: Element<'a, Message> = if selected {
        container(Space::new())
            .width(8)
            .height(8)
            .style(theme::fill(t.accent, 4.0))
            .into()
    } else {
        Space::new().into()
    };
    let dot = container(inner)
        .center(16)
        .style(theme::outlined(ring, 8.0));
    let mut line = row![dot, text(label).size(14).color(t.text).width(180)]
        .spacing(space::M)
        .align_y(Alignment::Center);
    if let Some(detail) = detail {
        line = line.push(
            text(detail)
                .size(13)
                .color(t.text_muted)
                .width(Length::Fill),
        );
    }
    button(line)
        .padding([8, 10])
        .width(Length::Fill)
        .style(theme::plain(t.text))
        .on_press(on_press)
        .into()
}

fn where_step<'a>(app: &'a App, setup: &'a AssistantSetup) -> Element<'a, Message> {
    let t = app.iced.tokens;
    column(CHOICES.iter().map(|(choice, label, detail)| {
        radio_row(
            t,
            label,
            Some(detail),
            setup.draft.choice == *choice,
            Message::SetupChoice(*choice),
        )
    }))
    .spacing(2)
    .into()
}

fn local_step<'a>(app: &'a App, setup: &'a AssistantSetup) -> Element<'a, Message> {
    let t = app.iced.tokens;
    let view = &app.snapshot.assistant;
    let port = setup.draft.port();
    let test_label = if view.listing {
        "Testando…"
    } else {
        "Testar conexão"
    };
    let port_row = row![
        text("Porta").size(13).color(t.text_soft()),
        text_input("11434", &setup.draft.port_input)
            .on_input(Message::SetupPort)
            .on_submit(Message::SetupTest)
            .size(13)
            .width(100)
            .padding([10, 10])
            .style(theme::input(t)),
        Space::new().width(Length::Fill),
        widgets::action_button(t, ButtonKind::Secondary, ButtonSize::Regular, test_label)
            .on_press_maybe((!view.listing && port.is_some()).then_some(Message::SetupTest)),
    ]
    // Validado; fora da escala de proposito.
    .spacing(10)
    .align_y(Alignment::Center);
    let mut section = column![port_row].spacing(12);
    let listed_here = port.is_some() && view.models_port == port;
    match (&view.models, listed_here) {
        (Some(Err(msg)), true) => {
            section = section.push(text(msg.as_str()).size(13).color(t.expense_fg));
        }
        (Some(Ok(models)), true) if models.is_empty() => {
            section = section.push(text(NO_MODELS_HINT).size(13).color(t.text_muted));
        }
        (Some(Ok(models)), true) => {
            section = section.push(widgets::kicker(t, "Modelo"));
            let current = setup.draft.local_model.as_deref();
            if models.len() <= RADIO_MAX {
                section = section.push(
                    column(models.iter().map(|m| {
                        radio_row(
                            t,
                            m.as_str(),
                            None,
                            current == Some(m.as_str()),
                            Message::SetupLocalModel(m.clone()),
                        )
                    }))
                    .spacing(2),
                );
            } else {
                section = section.push(
                    pick_list(
                        models.clone(),
                        setup.draft.local_model.clone(),
                        Message::SetupLocalModel,
                    )
                    .placeholder("Escolha um modelo")
                    .text_size(13)
                    .padding([10, 10])
                    .style(theme::pick(t))
                    .menu_style(theme::pick_menu(t))
                    .width(Length::Fill),
                );
            }
        }
        _ if view.listing => {
            section = section.push(
                text("Procurando o Ollama nesta porta…")
                    .size(13)
                    .color(t.text_muted),
            );
        }
        _ => {
            section = section.push(text(MODELS_NOT_LISTED).size(13).color(t.text_muted));
        }
    }
    section.into()
}

fn cloud_step<'a>(app: &'a App, setup: &'a AssistantSetup) -> Element<'a, Message> {
    let t = app.iced.tokens;
    let view = &app.snapshot.assistant;
    let mut section = column![
        text(CLOUD_EXPLAIN)
            .size(13)
            .line_height(1.45)
            .color(t.text_muted),
    ]
    .spacing(12);

    // Chave: salva (so "Remover") ou campo para digitar. O campo e `secure`: a chave nunca
    // aparece na tela, e so sai do `App` no ultimo botao.
    let key: Element<'_, Message> = if view.has_key {
        row![
            text("Chave salva").size(13).color(t.text),
            Space::new().width(Length::Fill),
            // Desligado enquanto grava: apagar a chave no meio do `Configure` desfaria o que ele
            // acabou de salvar.
            widgets::action_button(t, ButtonKind::Danger, ButtonSize::Compact, "Remover")
                .on_press_maybe(setup.saving.is_none().then_some(Message::RemoveKey)),
        ]
        .align_y(Alignment::Center)
        .into()
    } else {
        text_input("sk-or-...", &app.key_input)
            .secure(true)
            .on_input(|k| Message::KeyInput(Secret::new(k)))
            .on_submit(Message::SetupFinish)
            .size(13)
            .width(Length::Fill)
            .padding([10, 10])
            .style(theme::input(t))
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
                &setup.draft.cloud_model
            )
            .on_input(Message::SetupCloudModel)
            .size(13)
            .width(Length::Fill)
            .padding([10, 10])
            .style(theme::input(t)),
        ]
        .spacing(6),
    );
    section = section.push(
        row![
            text(DENY_LABEL).size(13).color(t.text).width(Length::Fill),
            widgets::toggle(t, setup.draft.deny, Some(Message::SetupDeny)),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    );
    if !setup.draft.deny {
        section = section.push(text(DENY_WARNING).size(13).color(t.warning));
    }
    section = section.push(
        widgets::action_button(
            t,
            ButtonKind::Link,
            ButtonSize::Compact,
            "Ver o que o assistente vê",
        )
        .on_press(Message::ShowEnvelope),
    );
    section.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passo_2_tem_altura_fixa_que_cabe_na_janela() {
        assert_eq!(dialog_height(900.0), DIALOG_HEIGHT);
        assert_eq!(dialog_height(500.0), 452.0);
        assert_eq!(dialog_height(100.0), 200.0);
    }
}
