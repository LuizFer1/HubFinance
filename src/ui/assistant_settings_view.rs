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
use crate::assistant::settings::{ProviderChoice, Settings};

/// Dica da lista vazia; constante para o teste conferir o texto.
pub(super) const NO_MODELS_HINT: &str =
    "Nenhum modelo instalado. Rode ollama pull llama3.1:8b no terminal e clique em Testar conexão.";

/// Antes do primeiro teste nesta porta. "Clique", e nao "Toque": o hub e programa de mesa.
pub(super) const MODELS_NOT_LISTED: &str =
    "Clique em Testar conexão para ver o que o Ollama tem instalado.";

/// Explicacao da nuvem; constante para o teste conferir o texto.
pub(super) const CLOUD_EXPLAIN: &str = "Ligado, o resumo de achados e totais desta conta vai para o OpenRouter quando você conversar. Lançamentos, descrições, pessoas e datas exatas nunca saem.";

pub(super) const PLAIN_KEY_WARNING: &str =
    "Neste sistema a chave fica em texto puro na pasta de dados do hub.";

pub(super) const DENY_LABEL: &str = "Não permitir que o provedor guarde ou treine";

pub(super) const DENY_WARNING: &str =
    "Alguns provedores podem guardar ou treinar com o que você enviar.";

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetupStep {
    /// Passo 1: onde roda.
    Where,
    /// Passo 2: porta e modelo do Ollama, ou chave, modelo e retencao da nuvem.
    Details,
}

/// O que a pessoa escolheu no dialogo, ainda nao gravado.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetupDraft {
    pub choice: ProviderChoice,
    /// So digitos, ate 5. Pode estar invalido (vazio, 0, 70000): o ultimo botao fica desligado.
    pub port_input: String,
    pub local_model: Option<String>,
    pub cloud_model: String,
    pub deny: bool,
}

impl SetupDraft {
    /// O rascunho que reflete o gravado. Nuvem escolhida com o interruptor desligado conta
    /// como "Desligado": e o que vale de fato (nada sai).
    pub fn from_settings(s: &Settings) -> SetupDraft {
        let choice = match s.provider {
            ProviderChoice::Cloud if !s.cloud.enabled => ProviderChoice::None,
            other => other,
        };
        SetupDraft {
            choice,
            port_input: s.local.port.to_string(),
            local_model: s.local.model.clone(),
            cloud_model: s.cloud.model.clone(),
            deny: s.cloud.deny_data_collection,
        }
    }

    /// A porta digitada, se for valida.
    pub fn port(&self) -> Option<u16> {
        self.port_input.parse::<u16>().ok().filter(|p| *p > 0)
    }
}

/// O dialogo aberto.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssistantSetup {
    pub step: SetupStep,
    pub draft: SetupDraft,
    /// O gravado de onde o rascunho saiu. Um snapshot novo (o do boot do nucleo, com o dialogo
    /// aberto antes dele) atualiza o campo que a pessoa nao tocou (ainda igual a base); o que
    /// ela mexeu fica. Sem isto o dialogo aberto cedo regravaria o padrao por cima do salvo.
    pub base: SetupDraft,
    /// `Some(n)`: o `Configure` numero `n` saiu e o dialogo espera `configure_result` com ele.
    pub saving: Option<u64>,
    /// Por que a ultima gravacao deste dialogo nao valeu (do `configure_result`, ou o hub fora
    /// do ar). Mostrado no dialogo; some ao tentar de novo.
    pub error: Option<String>,
}

impl AssistantSetup {
    pub fn open(saved: &Settings) -> AssistantSetup {
        let draft = SetupDraft::from_settings(saved);
        AssistantSetup {
            step: SetupStep::Where,
            base: draft.clone(),
            draft,
            saving: None,
            error: None,
        }
    }

    /// Campo a campo: o que ainda e igual a base acompanha o gravado novo.
    pub fn resync(&mut self, saved: &Settings) {
        fn follow<T: PartialEq + Clone>(draft: &mut T, base: &T, fresh: &T) {
            if draft == base {
                *draft = fresh.clone();
            }
        }
        let fresh = SetupDraft::from_settings(saved);
        let (d, b) = (&mut self.draft, &self.base);
        follow(&mut d.choice, &b.choice, &fresh.choice);
        follow(&mut d.port_input, &b.port_input, &fresh.port_input);
        follow(&mut d.local_model, &b.local_model, &fresh.local_model);
        follow(&mut d.cloud_model, &b.cloud_model, &fresh.cloud_model);
        follow(&mut d.deny, &b.deny, &fresh.deny);
        self.base = fresh;
    }
}

/// O que o ultimo botao grava: o gravado de agora com o que o dialogo decide. A nuvem so fica
/// ligada com "Usar OpenRouter"; sair dela (Desligado ou Ollama) desliga o interruptor, para a
/// nuvem nunca ficar armada sem a pessoa ter escolhido de novo. Os campos de um provedor que a
/// pessoa nao escolheu ficam como estavam. `None` = falta algo (porta invalida, sem modelo).
pub(super) fn settings_to_save(saved: &Settings, draft: &SetupDraft) -> Option<Settings> {
    let mut next = saved.clone();
    next.provider = draft.choice;
    next.cloud.enabled = draft.choice == ProviderChoice::Cloud;
    match draft.choice {
        ProviderChoice::None => {}
        ProviderChoice::Local => {
            next.local.port = draft.port()?;
            let model = draft.local_model.clone().filter(|m| !m.is_empty())?;
            next.local.model = Some(model);
        }
        ProviderChoice::Cloud => {
            let model = draft.cloud_model.trim();
            if model.is_empty() {
                return None;
            }
            next.cloud.model = model.to_string();
            next.cloud.deny_data_collection = draft.deny;
        }
    }
    Some(next)
}

/// O ultimo botao nomeia a acao.
pub(super) fn finish_label(choice: ProviderChoice) -> &'static str {
    match choice {
        ProviderChoice::None => "Desligar assistente",
        ProviderChoice::Local => "Usar Ollama local",
        ProviderChoice::Cloud => "Usar OpenRouter",
    }
}

/// O passo 1 termina ali? So com "Desligado", que nao tem passo 2.
fn finishes_here(setup: &AssistantSetup) -> bool {
    setup.step == SetupStep::Details || setup.draft.choice == ProviderChoice::None
}

/// O botao primario do passo: "Continuar" no passo 1 (menos com "Desligado") e a acao no fim.
pub(super) fn primary_label(setup: &AssistantSetup) -> &'static str {
    if setup.saving.is_some() {
        "Salvando…"
    } else if finishes_here(setup) {
        finish_label(setup.draft.choice)
    } else {
        "Continuar"
    }
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

/// Rotulo da linha de progresso.
fn step_caption(setup: &AssistantSetup) -> &'static str {
    match (setup.step, setup.draft.choice) {
        (SetupStep::Where, _) => "Onde roda, passo 1 de 2",
        (SetupStep::Details, ProviderChoice::Local) => "No meu computador, passo 2 de 2",
        (SetupStep::Details, _) => "Na nuvem, passo 2 de 2",
    }
}

pub(super) fn dialog(app: &App) -> Option<Element<'_, Message>> {
    let setup = app.assistant_setup.as_ref()?;
    let t = app.tokens;
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
        app.window_size.width,
        DIALOG_WIDTH,
        details.then(|| dialog_height(app.window_size.height)),
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

/// As tres opcoes do passo 1, com a descricao de cada uma.
pub(super) const CHOICES: [(ProviderChoice, &str, &str); 3] = [
    (
        ProviderChoice::None,
        "Desligado",
        "Só os achados, sem conversa.",
    ),
    (
        ProviderChoice::Local,
        "No meu computador",
        "Ollama. Nada sai da máquina.",
    ),
    (
        ProviderChoice::Cloud,
        "Na nuvem",
        "OpenRouter. O resumo sai da sua rede.",
    ),
];

fn where_step<'a>(app: &'a App, setup: &'a AssistantSetup) -> Element<'a, Message> {
    let t = app.tokens;
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
    let t = app.tokens;
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
    let t = app.tokens;
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

    fn local(port: u16, model: Option<&str>) -> Settings {
        let mut s = Settings {
            provider: ProviderChoice::Local,
            ..Settings::default()
        };
        s.local.port = port;
        s.local.model = model.map(Into::into);
        s
    }

    #[test]
    fn rascunho_reflete_o_gravado_e_nuvem_desligada_conta_como_desligado() {
        let d = SetupDraft::from_settings(&local(11500, Some("m")));
        assert_eq!(d.choice, ProviderChoice::Local);
        assert_eq!(d.port_input, "11500");
        assert_eq!(d.port(), Some(11500));
        let mut cloud_off = Settings {
            provider: ProviderChoice::Cloud,
            ..Settings::default()
        };
        assert_eq!(
            SetupDraft::from_settings(&cloud_off).choice,
            ProviderChoice::None
        );
        cloud_off.cloud.enabled = true;
        assert_eq!(
            SetupDraft::from_settings(&cloud_off).choice,
            ProviderChoice::Cloud
        );
        for bad in ["", "0", "70000"] {
            let mut d = d.clone();
            d.port_input = bad.into();
            assert_eq!(d.port(), None, "{bad}");
        }
    }

    #[test]
    fn o_que_grava_depende_da_escolha_e_so_a_nuvem_liga_o_interruptor() {
        let saved = local(11434, Some("m"));
        let mut d = SetupDraft::from_settings(&saved);
        // Ollama com porta nova.
        d.port_input = "11500".into();
        let next = settings_to_save(&saved, &d).unwrap();
        assert_eq!(next.local.port, 11500);
        assert!(!next.cloud.enabled);
        // Sem modelo ou com porta invalida nao grava.
        d.local_model = None;
        assert_eq!(settings_to_save(&saved, &d), None);
        d.local_model = Some("m".into());
        d.port_input = "0".into();
        assert_eq!(settings_to_save(&saved, &d), None);
        // Nuvem: liga o interruptor, grava modelo e retencao e deixa a porta como estava.
        d.choice = ProviderChoice::Cloud;
        d.cloud_model = "  openai/gpt-4o ".into();
        d.deny = false;
        let next = settings_to_save(&saved, &d).unwrap();
        assert_eq!(next.provider, ProviderChoice::Cloud);
        assert!(next.cloud.enabled);
        assert_eq!(next.cloud.model, "openai/gpt-4o");
        assert!(!next.cloud.deny_data_collection);
        assert_eq!(next.local.port, 11434);
        d.cloud_model = "   ".into();
        assert_eq!(settings_to_save(&saved, &d), None);
        // Desligado desliga a nuvem tambem.
        let mut on = saved.clone();
        on.provider = ProviderChoice::Cloud;
        on.cloud.enabled = true;
        d.choice = ProviderChoice::None;
        let next = settings_to_save(&on, &d).unwrap();
        assert_eq!(next.provider, ProviderChoice::None);
        assert!(!next.cloud.enabled);
    }

    #[test]
    fn ultimo_botao_nomeia_a_acao() {
        assert_eq!(finish_label(ProviderChoice::None), "Desligar assistente");
        assert_eq!(finish_label(ProviderChoice::Local), "Usar Ollama local");
        assert_eq!(finish_label(ProviderChoice::Cloud), "Usar OpenRouter");
        let mut setup = AssistantSetup::open(&Settings::default());
        // Desligado termina no passo 1; os outros continuam.
        assert_eq!(primary_label(&setup), "Desligar assistente");
        setup.draft.choice = ProviderChoice::Local;
        assert_eq!(primary_label(&setup), "Continuar");
        setup.step = SetupStep::Details;
        assert_eq!(primary_label(&setup), "Usar Ollama local");
        setup.draft.choice = ProviderChoice::Cloud;
        assert_eq!(primary_label(&setup), "Usar OpenRouter");
        setup.saving = Some(1);
        assert_eq!(primary_label(&setup), "Salvando…");
    }

    #[test]
    fn snapshot_novo_atualiza_so_o_que_a_pessoa_nao_mexeu() {
        let mut setup = AssistantSetup::open(&Settings::default());
        setup.draft.port_input = "11500".into();
        let mut boot = local(12345, Some("llama"));
        boot.cloud.model = "x/modelo".into();
        setup.resync(&boot);
        assert_eq!(setup.draft.port_input, "11500", "digitado fica");
        assert_eq!(setup.draft.cloud_model, "x/modelo");
        assert_eq!(setup.draft.choice, ProviderChoice::Local);
        assert_eq!(setup.draft.local_model.as_deref(), Some("llama"));
        assert_eq!(setup.base, SetupDraft::from_settings(&boot));
    }

    #[test]
    fn passo_2_tem_altura_fixa_que_cabe_na_janela() {
        assert_eq!(dialog_height(900.0), DIALOG_HEIGHT);
        assert_eq!(dialog_height(500.0), 452.0);
        assert_eq!(dialog_height(100.0), 200.0);
    }

    #[test]
    fn textos_das_opcoes_e_da_nuvem() {
        assert_eq!(
            CHOICES.map(|c| c.1),
            ["Desligado", "No meu computador", "Na nuvem"]
        );
        for (_, label, detail) in CHOICES {
            assert!(!label.contains("  ") && !detail.contains("  "));
        }
        assert_eq!(DENY_LABEL, "Não permitir que o provedor guarde ou treine");
    }
}
