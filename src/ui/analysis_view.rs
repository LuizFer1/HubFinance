//! Tela Analise em dois paineis (spec Grafite · Mesa 8.5): os achados de `advisor` a esquerda
//! (sempre, sem rede) e o assistente a direita, cada um com a propria rolagem. Na janela minima
//! (728 de largura util) os paineis viram um segmentado [Achados | Conversa]. Tudo vem de
//! `app.analysis` (calculado em `App::rebuild`) e do snapshot; aqui so se monta a arvore.

use std::borrow::Cow;

use iced::widget::{
    Space, button, column, container, markdown, rich_text, row, scrollable, span, text, text_input,
};
use iced::{Alignment, Color, Element, Length, Padding};

use super::app::{AnalysisTab, App, CopyTarget, Message, awaiting_cloud_consent};
use super::theme::{self, ButtonKind, ButtonSize, Tokens, Tone, radius, size, space, type_scale};
use super::widgets::{RowEdge, SegmentedSize, SegmentedVariant};
use super::{fonts, icons, rows, shell, time, widgets};
use crate::advisor::{FigureValue, Finding, FindingKind, Severity};
use crate::assistant::prompt::{OPENING_REQUEST, Role};
use crate::assistant::settings::{ProviderChoice, Settings};
use crate::assistant::view::{ConversationSummary, MessageRole, MessageView, OpenConversation};
use crate::dashboard::money::{MASK, MASK_DIGITS, MINUS, group_thousands};

/// Entre os cards da tela.
const GAP: f32 = space::BETWEEN_CARDS;

/// Largura do painel do assistente (8.5).
const ASSISTANT_WIDTH: f32 = 400.0;

/// Teto do texto dos achados: linha longa demais cansa a leitura na janela maximizada.
const FINDINGS_MAX: f32 = 720.0;

/// Largura maxima da resposta: perto de 56 caracteres de 14 px, a medida de leitura (8.5).
/// No painel de 400 ela nunca chega a valer; vale no segmentado da janela minima.
const REPLY_MAX: f32 = 440.0;

/// Abaixo desta largura util os dois paineis nao cabem (achados precisam de ~320 ao lado dos
/// 400 do assistente e do respiro): a tela vira o segmentado. A janela minima (1024) da 728.
const TWO_PANELS_MIN: f32 = 760.0;

/// Rolagem da conversa: id proprio, porque `CONTENT_SCROLL` (o que a troca de tela volta ao
/// topo) e o dos achados, e so uma rolagem pode ter cada id.
pub(super) const CHAT_SCROLL: &str = "conversa";

/// A pergunta que a tela responde, no cabecalho fixo da casca.
pub(super) const QUESTION: &str = "Onde dá para melhorar?";

/// Texto da sobreposicao; constante para um teste conferir que a quebra de linha no fonte nao
/// deixou espacos soltos no meio da frase.
const INTRO: &str = "Este é o texto exato que o assistente recebe: resumos e totais, sem \
lançamentos, descrições, nomes de pessoas, aparelhos ou formas de pagamento, nem datas \
exatas.";

/// Os dois paineis cabem lado a lado nesta largura util?
pub(super) fn two_panels(content_width: f32) -> bool {
    content_width >= TWO_PANELS_MIN
}

pub fn view(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let mut page = column![
        text(summary_line(&app.analysis.findings))
            .size(13)
            .color(t.text_muted)
    ]
    .spacing(GAP)
    .width(Length::Fill)
    .height(Length::Fill);
    // Erro do assistente (gravar a configuracao, a chave ou o historico) na tela, e nao so
    // dentro do dialogo: quem fechou o dialogo ou nem o abriu tambem precisa ver (A7).
    if let Some(err) = &app.snapshot.assistant.last_error {
        page = page.push(widgets::banner(
            t,
            Tone::Expense,
            t.bg,
            icons::WARNING_CIRCLE,
            err.as_str(),
            vec![
                widgets::action_button(t, ButtonKind::Secondary, ButtonSize::Compact, "Configurar")
                    .on_press(Message::ConfigureAssistant)
                    .into(),
            ],
        ));
    }
    if two_panels(shell::content_width(app)) {
        page = page.push(
            row![
                findings_panel(app),
                container(assistant_panel(app)).width(ASSISTANT_WIDTH)
            ]
            .spacing(16)
            .height(Length::Fill),
        );
    } else {
        page = page.push(tab_switch(app));
        page = page.push(match app.analysis_tab {
            AnalysisTab::Findings => findings_panel(app),
            AnalysisTab::Chat => assistant_panel(app),
        });
    }
    page.into()
}

/// [Achados | Conversa] da janela estreita.
fn tab_switch(app: &App) -> Element<'_, Message> {
    let current = app.analysis_tab;
    let option =
        |label: &'static str, tab: AnalysisTab| (label, current == tab, Message::AnalysisTab(tab));
    row![
        widgets::segmented(
            app.tokens,
            SegmentedVariant::Pill,
            SegmentedSize::Regular,
            false,
            [
                option("Achados", AnalysisTab::Findings),
                option("Conversa", AnalysisTab::Chat),
            ],
        ),
        Space::new().width(Length::Fill),
    ]
    .into()
}

/// "2 alertas, 3 atenções, 1 bom sinal. Calculado no seu PC." Grupo vazio nao entra: "0
/// alertas" chamaria atencao para o que nao existe.
pub(super) fn summary_line(findings: &[Finding]) -> String {
    let count = |s: Severity| findings.iter().filter(|f| f.severity == s).count();
    let parts: Vec<String> = [
        (count(Severity::Alert), "alerta", "alertas"),
        (count(Severity::Attention), "atenção", "atenções"),
        (count(Severity::GoodSign), "bom sinal", "bons sinais"),
    ]
    .into_iter()
    .filter(|(n, ..)| *n > 0)
    .map(|(n, one, many)| format!("{n} {}", if n == 1 { one } else { many }))
    .collect();
    if parts.is_empty() {
        "Nada a apontar. Calculado no seu PC.".to_string()
    } else {
        format!("{}. Calculado no seu PC.", parts.join(", "))
    }
}

/// Rolagem vertical de painel: cursor fino, longe da borda do card.
fn panel_scroll<'a>(
    t: &'static Tokens,
    content: impl Into<Element<'a, Message>>,
) -> scrollable::Scrollable<'a, Message> {
    scrollable(content)
        .direction(scrollable::Direction::Vertical(
            scrollable::Scrollbar::new()
                .width(6)
                .scroller_width(6)
                .margin(3),
        ))
        .style(theme::scrollbar(t))
        .width(Length::Fill)
        .height(Length::Fill)
}

// ---- achados ----

fn severity_tone(s: Severity) -> Tone {
    match s {
        Severity::Alert => Tone::Expense,
        Severity::Attention => Tone::Warning,
        Severity::GoodSign => Tone::Income,
    }
}

/// Cor do icone de cada gravidade: a de texto do tom (o `expense` cru nao vira icone).
fn severity_fg(t: &'static Tokens, s: Severity) -> Color {
    match s {
        Severity::Alert => t.expense_fg,
        Severity::Attention => t.warning,
        Severity::GoodSign => t.income_fg,
    }
}

fn severity_glyph(s: Severity) -> &'static str {
    match s {
        Severity::Alert => icons::WARNING_CIRCLE,
        Severity::Attention => icons::INFO,
        Severity::GoodSign => icons::CHECK_CIRCLE,
    }
}

/// Titulo do grupo: plural, como na linha-resumo ("Alertas 2").
fn group_title(s: Severity) -> &'static str {
    match s {
        Severity::Alert => "Alertas",
        Severity::Attention => "Atenção",
        Severity::GoodSign => "Bons sinais",
    }
}

const SEVERITIES: [Severity; 3] = [Severity::Alert, Severity::Attention, Severity::GoodSign];

fn findings_panel(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let findings = &app.analysis.findings;
    let content: Element<'_, Message> = if findings.is_empty() {
        container(widgets::empty_state(
            t,
            widgets::empty_icon(t, icons::CHECK_CIRCLE),
            "Nada a apontar",
            "Ainda não há dados suficientes nesta conta, ou está tudo dentro do seu normal.",
            None,
        ))
        .center_x(Length::Fill)
        .padding([40, 20])
        .into()
    } else {
        let mut groups = column![].spacing(space::XXL).max_width(FINDINGS_MAX);
        for severity in SEVERITIES {
            let items: Vec<&Finding> = findings.iter().filter(|f| f.severity == severity).collect();
            if items.is_empty() {
                continue;
            }
            let mut group = column![
                row![
                    text(group_title(severity))
                        .size(13)
                        .font(fonts::INTER_MEDIUM)
                        .color(t.text_soft()),
                    text(items.len().to_string()).size(13).color(t.text_muted),
                ]
                .spacing(8)
            ]
            .spacing(12);
            for (i, f) in items.into_iter().enumerate() {
                if i > 0 {
                    group = group.push(widgets::divider(t));
                }
                group = group.push(finding_row(app, f));
            }
            groups = groups.push(group);
        }
        container(groups)
            .padding(space::CARD)
            .width(Length::Fill)
            .into()
    };
    widgets::card(t, panel_scroll(t, content).id(super::shell::CONTENT_SCROLL))
        .padding(0)
        .height(Length::Fill)
        .into()
}

/// Um achado: tile de 28 na tinta de 10 % do tom, titulo 16 Medium, detalhe 14 `text_soft` e a
/// acao 14 em `fg` (nao dourada: nao e clicavel).
fn finding_row<'a>(app: &'a App, f: &'a Finding) -> Element<'a, Message> {
    let t = app.tokens;
    let tile = container(icons::icon(
        severity_glyph(f.severity),
        16.0,
        severity_fg(t, f.severity),
    ))
    .center(size::CTRL_S)
    .style(theme::fill(
        t.tint(severity_tone(f.severity), t.surface),
        radius::INSET,
    ));
    let mut body = column![
        text(mask(app, &f.title))
            .size(16)
            .font(fonts::INTER_MEDIUM)
            .color(t.text),
        text(mask(app, &f.detail))
            .size(14)
            .line_height(1.45)
            .color(t.text_soft()),
    ]
    .spacing(4)
    .width(Length::Fill);
    if let Some(action) = &f.action {
        body = body.push(
            text(mask(app, action))
                .size(14)
                .line_height(1.45)
                .color(t.text),
        );
    }
    if let Some(c) = comparison(f) {
        body = body.push(comparison_view(app, f.severity, &c));
    }
    row![tile, body].spacing(12).into()
}

/// Gasto contra a referencia de um achado (spec 10: "barra de real contra normal"), lido das
/// cifras que ja vao ao envelope, sem mudar `Finding`. So os achados que comparam um gasto com
/// outro valor do mesmo tipo; os de taxa, meses ou previsao nao tem um par assim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Comparison {
    pub real: i64,
    pub reference: i64,
    pub reference_label: &'static str,
}

/// A ordem das cifras nao e a mesma em toda regra (`advisor/rules.rs`): ritmo e categoria vem
/// `[gasto, normal]`, meta vem `[meta, gasto]`. Por isso o tipo decide, num lugar so.
pub(super) fn comparison(f: &Finding) -> Option<Comparison> {
    let money = |i: usize| match f.figures.get(i)?.value {
        FigureValue::Money(v) => Some(v),
        _ => None,
    };
    let (real, reference, reference_label) = match f.kind {
        FindingKind::Pace | FindingKind::CategoryAboveNormal => (money(0)?, money(1)?, "Normal"),
        FindingKind::Budget => (money(1)?, money(0)?, "Meta"),
        _ => return None,
    };
    // Sem escala (os dois zero ou negativos) a barra nao diz nada.
    (real.max(reference) > 0).then_some(Comparison {
        real,
        reference,
        reference_label,
    })
}

/// Duas barras de 6 px na mesma escala: o gasto na cor da gravidade e a referencia em neutro,
/// cada uma com o valor a direita (mascarado com "Ocultar valores", como as barras do Resumo:
/// o tamanho fica, o numero some).
fn comparison_view<'a>(app: &App, severity: Severity, c: &Comparison) -> Element<'a, Message> {
    use crate::dashboard::money::format_brl;
    let t = app.tokens;
    let scale = c.real.max(c.reference) as f32;
    let line = |label: &'static str, value: i64, color: Color| -> Element<'a, Message> {
        let shown = if app.hide_values {
            MASK.to_string()
        } else {
            format_brl(value)
        };
        row![
            text(label)
                .size(type_scale::META)
                .color(t.text_muted)
                .width(COMPARISON_LABEL),
            widgets::bar(
                widgets::Bar::Fraction {
                    value: value.max(0) as f32 / scale,
                    color,
                    track: Some(t.neutral_800),
                    outlined: false,
                },
                6.0,
            ),
            text(shown)
                .size(type_scale::META)
                .color(t.text_soft())
                .width(COMPARISON_VALUE)
                .align_x(Alignment::End),
        ]
        .spacing(space::S)
        .align_y(Alignment::Center)
        .into()
    };
    column![
        line("No mês", c.real, severity_fg(t, severity)),
        line(c.reference_label, c.reference, t.neutral_500),
    ]
    .spacing(space::XS)
    .padding(Padding {
        top: space::XS,
        ..Padding::ZERO
    })
    .into()
}

/// Coluna do rotulo ("No mês", "Normal", "Meta") e do valor das barras do achado.
const COMPARISON_LABEL: f32 = 56.0;
const COMPARISON_VALUE: f32 = 96.0;

// ---- assistente ----

/// "llama3.1:8b, local" sob o titulo do painel: onde o assistente roda agora.
pub(super) fn assistant_status(settings: &Settings) -> String {
    match settings.provider {
        ProviderChoice::None => "Desligado".to_string(),
        ProviderChoice::Local => match settings.local.model.as_deref() {
            Some(m) if !m.is_empty() => format!("{m}, local"),
            _ => "Ollama, sem modelo".to_string(),
        },
        ProviderChoice::Cloud if !settings.cloud.enabled => "Nuvem desligada".to_string(),
        ProviderChoice::Cloud => format!("{}, nuvem", settings.cloud.model.trim()),
    }
}

fn assistant_panel(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let open = open_conversation(app);
    let mut header = row![
        column![
            text("Assistente")
                .size(15)
                .font(fonts::INTER_MEDIUM)
                .color(t.text),
            text(assistant_status(&app.snapshot.assistant.settings))
                .size(12)
                .color(t.text_muted),
        ]
        .spacing(2)
        .width(Length::Fill),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    if let Some(open) = open {
        header = header.push(text(tokens_total_label(open)).size(12).color(t.text_muted));
        header = header.push(
            widgets::action_button(t, ButtonKind::Secondary, ButtonSize::Compact, "Fechar")
                .on_press(Message::CloseConversation),
        );
    }
    header = header.push(widgets::icon_button(
        t,
        icons::GEAR,
        ButtonSize::Compact,
        "Configurar assistente",
        Some(Message::ConfigureAssistant),
    ));
    // Sem `Status::Failed` aqui: com o nucleo fora do ar o card de falha da casca toma a tela
    // inteira (`shell::failure`), e a Analise nem e montada.
    let body: Element<'_, Message> = if let Some(open) = open {
        conversation_body(app, open)
    } else {
        idle_body(app)
    };
    widgets::card(
        t,
        column![
            container(header).padding(Padding {
                top: 14.0,
                right: 12.0,
                bottom: 12.0,
                left: 16.0,
            }),
            widgets::divider(t),
            body,
        ]
        .height(Length::Fill),
    )
    .padding(0)
    .height(Length::Fill)
    .into()
}

/// Sem conversa aberta: conversar, ver o envelope e as conversas anteriores.
fn idle_body(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let mut col = column![
        text("Converse sobre os achados desta conta. O assistente recebe só resumos e totais.")
            .size(13)
            .line_height(1.45)
            .color(t.text_soft()),
        widgets::action_button(
            t,
            ButtonKind::Primary,
            ButtonSize::Regular,
            "Conversar sobre estes achados",
        )
        .on_press(Message::StartConversation),
        widgets::action_button(
            t,
            ButtonKind::Link,
            ButtonSize::Compact,
            "Ver o que o assistente vê"
        )
        .on_press(Message::ShowEnvelope),
    ]
    .spacing(12)
    .width(Length::Fill);
    if let Some(list) = conversation_list(app) {
        col = col.push(Space::new().height(6)).push(list);
    }
    panel_scroll(t, container(col).padding([16, 16])).into()
}

/// A conversa aberta, so se for da conta que a tela mostra: com a outra conta aberta ela falaria
/// de numeros que nao estao na tela.
pub(super) fn open_conversation(app: &App) -> Option<&OpenConversation> {
    app.snapshot
        .assistant
        .open
        .as_ref()
        .filter(|c| c.book == app.book && !app.conversation_hidden(&c.id))
}

/// Os dados da conta mudaram desde que a conversa comecou (spec 3): o envelope guardado nela
/// difere do de agora. Comparar o texto (e nao o ponteiro do `Arc` do dataset) faz valer
/// tambem para conversa reaberta do historico e ignora sync que nao mexeu nesta conta.
pub(super) fn data_changed(app: &App) -> bool {
    open_conversation(app).is_some_and(|c| c.envelope != app.envelope_now)
}

/// Conversas da conta que a tela mostra (cada uma e presa a uma conta).
pub(super) fn conversations_of_book(app: &App) -> Vec<&ConversationSummary> {
    app.snapshot
        .assistant
        .conversations
        .iter()
        .filter(|c| c.book == app.book && !app.conversation_hidden(&c.id))
        .collect()
}

fn tokens_label((tokens_in, tokens_out): (u32, u32)) -> String {
    format!(
        "{} → {} tokens",
        group_thousands(u64::from(tokens_in)),
        group_thousands(u64::from(tokens_out))
    )
}

/// Total do cabecalho. Sem nenhuma resposta que tenha informado uso (modelo local que nao manda
/// `usage`, ou conversa so com a abertura), "0 → 0" afirmaria um zero que ninguem mediu.
fn tokens_total_label(open: &OpenConversation) -> String {
    if open
        .messages
        .iter()
        .any(|m| m.tokens_in.is_some() || m.tokens_out.is_some())
    {
        tokens_label(open.tokens_total())
    } else {
        "—".to_string()
    }
}

/// "Cancelar" na previa da nuvem. So apaga a conversa que e exatamente a abertura automatica
/// recem-criada (uma pergunta, a `OPENING_REQUEST`) e sem nada chegando: nada nunca foi enviado
/// e guarda-la deixaria lixo no historico. Qualquer outra (reaberta do historico com erro, com
/// resposta, com pergunta da pessoa, ou com uma resposta local ainda chegando) tem o que
/// guardar: o "Cancelar" so fecha a previa. Esc e o scrim nunca apagam (`ClosePreview`).
pub(super) fn cancel_preview_message(open: &OpenConversation, busy: bool) -> Option<Message> {
    let only_opening = matches!(
        open.messages.as_slice(),
        [m] if m.role == MessageRole::User && m.content == OPENING_REQUEST
    );
    (only_opening && !busy).then(|| Message::DeleteConversation(open.id.clone()))
}

/// Aviso acima da previa com "Ocultar valores" ligado: a tela mascara, o envio nao.
const HIDDEN_VALUES_NOTE: &str = "Os valores estão ocultos na tela; o envio leva os valores reais.";

fn hidden_values_note(app: &App) -> Option<&'static str> {
    app.hide_values.then_some(HIDDEN_VALUES_NOTE)
}

/// A conversa: mensagens na rolagem propria (ancorada embaixo, acompanha o texto que chega) e o
/// campo de pergunta fixo no pe do painel.
fn conversation_body<'a>(app: &'a App, open: &'a OpenConversation) -> Element<'a, Message> {
    let t = app.tokens;
    let view = &app.snapshot.assistant;
    // Nuvem sem a previa aprovada: nada saiu, e o "Tentar de novo" sob a pergunta de abertura
    // reenviaria sem a pessoa ver o que vai. A previa toma o lugar dele e do campo de pergunta.
    let awaiting = awaiting_cloud_consent(app).is_some();
    let last = open.messages.len().saturating_sub(1);
    let mut messages = column![].spacing(space::L).width(Length::Fill);
    for (i, m) in open.messages.iter().enumerate() {
        messages = messages.push(message_view(app, m, i == last && !view.busy && !awaiting));
    }
    if let (true, Some(partial)) = (view.busy, view.streaming.as_deref()) {
        messages = messages.push(streaming_view(app, partial));
    }
    let scroll = scrollable(container(messages).padding([14, 16]))
        .id(CHAT_SCROLL)
        .direction(scrollable::Direction::Vertical(
            scrollable::Scrollbar::new()
                .width(6)
                .scroller_width(6)
                .margin(3)
                .anchor(scrollable::Anchor::End),
        ))
        .style(theme::scrollbar(t))
        .width(Length::Fill)
        .height(Length::Fill);
    let mut col = column![].width(Length::Fill).height(Length::Fill);
    if data_changed(app) {
        col = col.push(
            container(widgets::banner(
                t,
                Tone::Warning,
                t.surface,
                icons::WARNING_CIRCLE,
                "Os dados mudaram desde o início desta conversa",
                vec![
                    widgets::action_button(
                        t,
                        ButtonKind::Primary,
                        ButtonSize::Compact,
                        "Nova conversa",
                    )
                    .on_press(Message::StartConversation)
                    .into(),
                ],
            ))
            .padding(Padding {
                top: 12.0,
                right: 12.0,
                bottom: 0.0,
                left: 12.0,
            }),
        );
    }
    col = col.push(scroll).push(widgets::divider(t));
    let foot: Element<'a, Message> = if awaiting {
        awaiting_foot(t)
    } else {
        chat_input(app)
    };
    col.push(container(foot).padding([12, 16])).into()
}

/// Campo de pergunta com "Enviar", fixo no pe do painel.
fn chat_input(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let view = &app.snapshot.assistant;
    // Sem `on_input` o campo fica desabilitado enquanto ha resposta chegando.
    let mut input = text_input("Pergunte sobre estes números", &app.chat_input)
        .size(14)
        .padding([9, 12])
        .style(theme::input(t))
        .width(Length::Fill);
    if !view.busy {
        input = input
            .on_input(Message::ChatInput)
            .on_submit(Message::ChatSubmit);
    }
    // "Enviar" ao lado do Enter: quem nao sabe do Enter nao fica sem saida. Desabilitado nas
    // mesmas condicoes em que `ChatSubmit` nao faria nada.
    let can_send = !view.busy && !app.chat_input.trim().is_empty();
    // Botao padrao de 36 e o campo na mesma altura: texto de 14 com a altura de linha padrao e
    // 9 de respiro (14 x 1,3 + 18 = 36,2).
    let send = widgets::action_button(t, ButtonKind::Primary, ButtonSize::Regular, "Enviar")
        .on_press_maybe(can_send.then_some(Message::ChatSubmit));
    row![input, send]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
}

fn message_view<'a>(app: &'a App, m: &'a MessageView, is_last: bool) -> Element<'a, Message> {
    let t = app.tokens;
    match m.role {
        MessageRole::User => {
            let bubble = container(text(mask(app, &m.content)).size(14).color(t.text))
                .padding([8, 12])
                .max_width(320)
                .style(theme::fill(t.surface_2, radius::CONTROL));
            // Pergunta sem resposta e sem nada em andamento (a abertura nao saiu, ou o envio
            // morreu antes do erro virar mensagem): sem este botao a conversa ficaria parada.
            let mut side = column![bubble].spacing(6).align_x(Alignment::End);
            if is_last {
                side = side.push(retry_button(t));
            }
            row![Space::new().width(Length::Fill), side].into()
        }
        MessageRole::Assistant => {
            let notes = reply_notes(m);
            let copied = app.is_copied(CopyTarget::Reply)
                && app.copied_reply.as_deref() == Some(m.id.as_str());
            column![
                reply_body(app, m),
                row![
                    text(notes.join(" · "))
                        .size(11)
                        .color(t.text_muted)
                        .width(Length::Fill),
                    widgets::copy_button(t, copied, Some(Message::CopyReply(m.id.clone()))),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            ]
            .spacing(6)
            .max_width(REPLY_MAX)
            .into()
        }
        MessageRole::Error => {
            let mut body =
                column![text(mask(app, &m.content)).size(14).color(t.expense_fg)].spacing(8);
            if is_last {
                body = body.push(retry_button(t));
            }
            container(body)
                .padding([8, 12])
                .max_width(REPLY_MAX)
                .style(theme::outlined(
                    theme::alpha(t.expense, 0.55),
                    radius::CONTROL,
                ))
                .into()
        }
    }
}

/// O corpo da resposta: o Markdown ja interpretado do cache (`App::replies`), ou o texto puro.
/// O cache so vale para o mesmo texto e o mesmo estado do olho: um cache montado com os valores
/// a mostra nunca e desenhado com "Ocultar valores" ligado.
fn reply_body<'a>(app: &'a App, m: &'a MessageView) -> Element<'a, Message> {
    let t = app.tokens;
    // Sem cache valido (snapshot que ainda nao passou por `refresh_replies`): o mesmo caminho
    // do cache, em texto puro. O fonte so mascarado deixaria passar o que o Markdown remonta.
    let fallback = || -> Element<'a, Message> {
        let r = render_reply(&m.content, app.hide_values);
        let shown = match r.shown {
            ReplyShown::Plain(s) => s,
            ReplyShown::Markdown(_) => r.copy,
        };
        text(shown).size(14).line_height(1.5).color(t.text).into()
    };
    let Some(cached) = app
        .replies
        .get(&m.id)
        .filter(|c| c.hidden == app.hide_values && c.content == m.content)
    else {
        return fallback();
    };
    match &cached.shown {
        ReplyShown::Markdown(items) => container(markdown::view_with(
            items,
            markdown_settings(t),
            &ReplyViewer { t },
        ))
        .style(move |_| container::Style {
            text_color: Some(t.text),
            ..container::Style::default()
        })
        .into(),
        ReplyShown::Plain(s) => text(s.as_str())
            .size(14)
            .line_height(1.5)
            .color(t.text)
            .into(),
    }
}

/// Resposta chegando: texto puro (mascarado). O Markdown so entra quando ela termina: um numero
/// cortado entre dois pedacos nao seria reconhecido pela mascara num parse incremental.
fn streaming_view<'a>(app: &'a App, partial: &'a str) -> Element<'a, Message> {
    let t = app.tokens;
    let shown = if partial.is_empty() {
        Cow::Borrowed("…")
    } else if app.hide_values {
        // A mascara olha o texto desenhado, nao o Markdown cru: "R$ **1500**" e "1.**234**,56"
        // so viram dinheiro depois de interpretados.
        Cow::Owned(mask_streaming(&plain_text(partial)))
    } else {
        Cow::Borrowed(partial)
    };
    // Cursor que pisca no fim do texto (`ui::motion`). Apagado fica transparente, e nao some:
    // o texto nao muda de largura a cada piscada. O "|" e o que a Inter tem; os blocos (U+258x)
    // cairiam numa fonte do sistema.
    let cursor = if app.motion.cursor_on() {
        t.text_accent()
    } else {
        Color::TRANSPARENT
    };
    column![
        rich_text![
            span::<(), _>(shown).color(t.text),
            span(" |").color(cursor).font(fonts::INTER_MEDIUM),
        ]
        .size(14)
        .line_height(1.5),
        widgets::action_button(t, ButtonKind::Secondary, ButtonSize::Compact, "Parar")
            .on_press(Message::ChatStop),
    ]
    .spacing(8)
    .max_width(REPLY_MAX)
    .into()
}

/// A mascara do texto chegando (`plain_text` do parcial). O numero no fim pode estar pela
/// metade ("1500,0" vira "1500,00"; "5 " vira "5 mil"; "1 " vira "1 234,56"): incompleto, a
/// mascara nao o reconhece como dinheiro. Por isso o fim que ainda pode virar valor fica
/// escondido ate o texto seguir (`pending_number_start`).
pub(super) fn mask_streaming(drawn: &str) -> String {
    let drawn = drawn.trim_end();
    match pending_number_start(drawn) {
        Some(cut) => mask_str(&drawn[..cut]),
        None => mask_str(drawn),
    }
}

/// Inicio do numero que encosta no fim de `text` e ainda pode crescer, se ha um. Andando de tras
/// para frente: um comeco de unidade ("m", "mi", "re"), espacos, e a sequencia de digitos,
/// separadores (".", ",", espaco de milhar) e marcas de Markdown ainda abertas ("**", "~~"),
/// com o sinal colado. Sem digito nessa sequencia nao ha numero pendente.
fn pending_number_start(text: &str) -> Option<usize> {
    let mut end = text.len();
    let word = text[..end]
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_alphabetic())
        .last()
        .map(|(i, _)| i);
    if let Some(start) = word {
        let w = text[start..end].to_lowercase();
        let unit = MAGNITUDES
            .iter()
            .chain(UNITS.iter())
            .any(|u| u.starts_with(&w));
        let before = text[..start].chars().next_back();
        if unit && before.is_some_and(|c| c.is_whitespace() || c.is_ascii_digit()) {
            end = start;
        }
    }
    let end = text[..end].trim_end().len();
    let mut start = end;
    let mut digit = false;
    for (i, c) in text[..end].char_indices().rev() {
        let part = c.is_ascii_digit()
            || matches!(c, '.' | ',' | '*' | '_' | '~' | '`' | '\\')
            || thousands_space(text, i).is_some();
        if !part {
            break;
        }
        digit |= c.is_ascii_digit();
        start = i;
    }
    if !digit {
        return None;
    }
    // Comeca no primeiro digito ou marca, nao no espaco que separa da palavra anterior.
    start = end - text[start..end].trim_start().len();
    let head = strip_sign(&text[..start]);
    Some(head.len())
}

// ---- Markdown da resposta ----

/// Como a resposta aparece: Markdown interpretado ou, quando a mascara nao pode garantir o
/// Markdown, o texto puro ja mascarado.
pub(super) enum ReplyShown {
    Markdown(Vec<markdown::Item>),
    Plain(String),
}

/// Resposta pronta para desenhar, guardada em `App::replies` pelo id da mensagem. `hidden` e
/// `content` sao a chave de validade: o `view` so usa o cache se os dois baterem com agora.
pub(super) struct CachedReply {
    pub hidden: bool,
    pub content: String,
    pub shown: ReplyShown,
    /// O que "Copiar" leva: o texto da resposta; com o olho ligado, o texto desenhado ja
    /// mascarado (coerente com a tela: quem esconde os valores nao os quer colados num chat ou
    /// num e-mail, nem o fonte que o Markdown remontaria).
    pub copy: String,
}

/// Monta a resposta. Com os valores a mostra, e o Markdown da resposta.
///
/// Com "Ocultar valores", a mascara e aplicada ao texto **antes** do parse, e o resultado e
/// conferido **depois**: o texto desenhado precisa ser exatamente a mascara do texto que o
/// Markdown original desenharia. Isso pega o que o parse remonta e a mascara do fonte nao ve
/// ("1.**234**,56", "1\.234,56", "&#49;.234,56"); nesses casos sai o texto puro, mascarado depois
/// de interpretado. Tabela nao pode ser conferida (o iced nao expoe as celulas): sai em texto
/// puro, sem as marcas de enfase.
pub(super) fn render_reply(content: &str, hidden: bool) -> CachedReply {
    let cached = |shown, copy| CachedReply {
        hidden,
        content: content.to_string(),
        shown,
        copy,
    };
    if !hidden {
        return cached(
            ReplyShown::Markdown(markdown::parse(content).collect()),
            content.to_string(),
        );
    }
    let raw: Vec<markdown::Item> = markdown::parse(content).collect();
    let source = mask_str(content);
    let masked: Vec<markdown::Item> = markdown::parse(&source).collect();
    match (rendered_text(&raw), rendered_text(&masked)) {
        (Some(raw_text), Some(masked_text)) => {
            let expected = mask_str(&raw_text);
            let copy = expected.trim_end().to_string();
            if masked_text == expected {
                cached(ReplyShown::Markdown(masked), copy)
            } else {
                cached(ReplyShown::Plain(copy.clone()), copy)
            }
        }
        _ => {
            let plain = mask_str(&plain_text(content));
            let plain = plain.trim_end().to_string();
            cached(ReplyShown::Plain(plain.clone()), plain)
        }
    }
}

/// O texto que o Markdown desenharia, lido dos eventos do `pulldown-cmark` com as mesmas opcoes
/// do iced: texto e codigo (celulas de tabela inclusive), quebras e fim de bloco viram quebra de
/// linha, HTML some (o iced nao o desenha). E a reserva da tabela, que o iced nao deixa ler: a
/// mascara roda sobre o texto ja interpretado, onde "1.~~234~~,56" ja e "1.234,56".
pub(super) fn plain_text(content: &str) -> String {
    use pulldown_cmark::{Event, Options, Parser, TagEnd};
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let mut out = String::new();
    for event in Parser::new_ext(content, options) {
        match event {
            Event::Text(t) | Event::Code(t) => out.push_str(&t),
            Event::SoftBreak | Event::HardBreak => out.push('\n'),
            Event::End(
                TagEnd::Paragraph
                | TagEnd::Heading(_)
                | TagEnd::Item
                | TagEnd::CodeBlock
                | TagEnd::TableRow
                | TagEnd::TableHead,
            ) => out.push('\n'),
            Event::End(TagEnd::TableCell) => out.push_str(" | "),
            _ => {}
        }
    }
    out
}

/// O texto que o Markdown desenha, bloco por bloco. `None` com tabela (celulas privadas no iced).
pub(super) fn rendered_text(items: &[markdown::Item]) -> Option<String> {
    let style = markdown::Style::from_palette(iced::theme::Palette::DARK);
    let mut out = String::new();
    for item in items {
        push_item(item, style, &mut out)?;
    }
    Some(out)
}

fn push_text(text: &markdown::Text, style: markdown::Style, out: &mut String) {
    for span in text.spans(style).iter() {
        out.push_str(&span.text);
    }
    out.push('\n');
}

fn push_item(item: &markdown::Item, style: markdown::Style, out: &mut String) -> Option<()> {
    match item {
        markdown::Item::Heading(_, text) | markdown::Item::Paragraph(text) => {
            push_text(text, style, out);
        }
        markdown::Item::CodeBlock { lines, .. } => {
            for line in lines {
                push_text(line, style, out);
            }
        }
        markdown::Item::List { bullets, .. } => {
            for bullet in bullets {
                let (markdown::Bullet::Point { items } | markdown::Bullet::Task { items, .. }) =
                    bullet;
                for item in items {
                    push_item(item, style, out)?;
                }
            }
        }
        markdown::Item::Image { alt, .. } => push_text(alt, style, out),
        markdown::Item::Quote(items) => {
            for item in items {
                push_item(item, style, out)?;
            }
        }
        markdown::Item::Rule => {}
        markdown::Item::Table { .. } => return None,
    }
    Some(())
}

/// Corpo 14 em Inter Hub; titulos contidos (o padrao do iced faz o h1 com o dobro do corpo, alto
/// demais num painel de 400) e codigo na mesma fonte, a 12: o app nao embute monoespacada.
fn markdown_settings(t: &'static Tokens) -> markdown::Settings {
    let style = markdown::Style {
        font: fonts::INTER,
        inline_code_highlight: markdown::Highlight {
            background: t.surface_2.into(),
            border: iced::border::rounded(4),
        },
        inline_code_padding: Padding::from([0, 3]),
        inline_code_color: t.text,
        inline_code_font: fonts::INTER,
        code_block_font: fonts::INTER,
        // Link nao abre (a UI so abre URL constante): na cor de apoio, sem cara de botao.
        link_color: t.text_soft(),
    };
    markdown::Settings {
        h1_size: 18.into(),
        h2_size: 16.into(),
        h3_size: 15.into(),
        h4_size: 14.into(),
        h5_size: 14.into(),
        h6_size: 14.into(),
        code_size: 12.into(),
        spacing: 10.into(),
        ..markdown::Settings::with_text_size(14, style)
    }
}

/// Desenho do Markdown da resposta: link nao faz nada (`Noop`), codigo numa caixa interna e
/// imagem so com o texto alternativo (nada e baixado).
struct ReplyViewer {
    t: &'static Tokens,
}

impl<'a> markdown::Viewer<'a, Message> for ReplyViewer {
    fn on_link_click(_url: markdown::Uri) -> Message {
        Message::Noop
    }

    fn image(
        &self,
        settings: markdown::Settings,
        _url: &'a markdown::Uri,
        _title: &'a str,
        alt: &markdown::Text,
    ) -> Element<'a, Message> {
        rich_text(alt.spans(settings.style))
            .size(settings.text_size)
            .into()
    }

    fn code_block(
        &self,
        settings: markdown::Settings,
        _language: Option<&'a str>,
        _code: &'a str,
        lines: &'a [markdown::Text],
    ) -> Element<'a, Message> {
        widgets::inset(
            self.t,
            column(lines.iter().map(|line| {
                rich_text(line.spans(settings.style))
                    .size(settings.code_size)
                    .font(settings.style.code_block_font)
                    .into()
            })),
        )
        .padding(10)
        .into()
    }
}

/// Rodape da resposta: local = "modelo · tokens"; nuvem = "tokens · selo e retencao · modelo",
/// com o modelo por ultimo para o selo de privacidade ficar junto do que ele qualifica.
/// Registro antigo, sem modelo, simplesmente nao o mostra.
pub(super) fn reply_notes(m: &MessageView) -> Vec<String> {
    let mut notes = Vec::new();
    let model = m.model.as_deref().filter(|s| !s.is_empty());
    if let Some(model) = model
        && !m.sent_to_cloud
    {
        notes.push(model.to_string());
    }
    notes.push(match (m.tokens_in, m.tokens_out) {
        (Some(i), Some(o)) => tokens_label((i, o)),
        _ => "—".to_string(),
    });
    if m.interrupted {
        notes.push("interrompida".to_string());
    }
    if m.sent_to_cloud {
        notes.push(cloud_badge(m.deny_data_collection).to_string());
        if let Some(model) = model {
            notes.push(model.to_string());
        }
    }
    notes
}

/// Selo da resposta que passou pela nuvem, com a politica de retencao valendo no envio (spec 7).
/// `None` = registro sem a politica: so diz que foi a nuvem.
pub(super) fn cloud_badge(deny_data_collection: Option<bool>) -> &'static str {
    match deny_data_collection {
        Some(true) => "enviado à nuvem · sem retenção",
        Some(false) => "enviado à nuvem · retenção permitida",
        None => "enviado à nuvem",
    }
}

pub(super) const PREVIEW_TITLE: &str = "Enviar para a nuvem?";

/// O botao da previa diz para onde vai: "Enviar" sozinho nao diz que sai do computador.
pub(super) const PREVIEW_SEND: &str = "Enviar ao OpenRouter";

pub(super) const PREVIEW_INTRO: &str = "Este é o texto exato que vai para o OpenRouter: o resumo desta conta e a pergunta de abertura, além das instruções fixas do assistente. Só depois de você enviar é que algo sai do computador.";

/// Conversa que ja tinha mensagens (por exemplo, respondidas pelo modelo local antes de trocar
/// para a nuvem): elas vao junto, e a previa diz isso.
pub(super) const PREVIEW_INTRO_EARLIER: &str = "Este é o texto exato que vai para o OpenRouter: o resumo desta conta e as mensagens anteriores desta conversa (só as últimas que cabem no pedido), além das instruções fixas do assistente. Só depois de você enviar é que algo sai do computador.";

/// A retencao da previa, a partir da politica configurada agora (e a que vale no envio).
pub(super) fn retention_value(deny_data_collection: bool) -> &'static str {
    if deny_data_collection {
        "não permitida"
    } else {
        "permitida"
    }
}

/// O envelope da conversa como aparece na previa: o da propria conversa (e o que de fato vai),
/// com a mascara de "Ocultar valores" so na tela. Com o olho desligado e o texto exato do envio.
/// Com ele ligado, as respostas anteriores do assistente passam antes por `plain_text`: sao
/// Markdown, e "R$ **1500**" ou "R\$ 1.500" so viram dinheiro que a mascara reconhece depois de
/// interpretados.
pub(super) fn preview_shown(app: &App, open: &OpenConversation) -> String {
    if !app.hide_values {
        return preview_body(open);
    }
    mask_str(&preview_text_with(open, |turn| {
        plain_text(turn).trim_end().to_string()
    }))
}

/// Tudo que o dono dos dados autoriza ao aprovar: o envelope e as mensagens que vao no pedido.
/// Sai de `OpenConversation::payload`, a mesma montagem do envio (janela de turnos incluida),
/// para a previa nao poder divergir do que de fato sai. As regras fixas do assistente (sem
/// dado nenhum) so sao citadas na introducao.
pub(super) fn preview_body(open: &OpenConversation) -> String {
    preview_text_with(open, str::to_string)
}

/// A montagem da previa, com `assistant` aplicado ao texto de cada resposta anterior.
fn preview_text_with(open: &OpenConversation, assistant: impl Fn(&str) -> String) -> String {
    let payload = open.payload();
    let mut out = open.envelope.clone();
    out.push_str("\n\n— Conversa —");
    for turn in payload.iter().skip(1) {
        out.push('\n');
        match turn.role {
            Role::Assistant => {
                out.push_str("Assistente: ");
                out.push_str(&assistant(&turn.content));
            }
            _ => {
                out.push_str("Você: ");
                out.push_str(&turn.content);
            }
        }
    }
    out
}

/// A introducao conforme haja mensagens anteriores alem da pergunta pendente.
fn preview_intro(open: &OpenConversation) -> &'static str {
    if open.payload().len() > 2 {
        PREVIEW_INTRO_EARLIER
    } else {
        PREVIEW_INTRO
    }
}

/// Pe da conversa de nuvem que espera aprovacao: nada saiu, e o campo de pergunta da lugar ao
/// caminho para a previa (que tambem abre sozinha ao comecar a conversa).
fn awaiting_foot(t: &'static Tokens) -> Element<'static, Message> {
    column![
        text("Nada saiu ainda. Veja o que vai para o OpenRouter antes de enviar.")
            .size(13)
            .line_height(1.4)
            .color(t.text_muted),
        row![
            Space::new().width(Length::Fill),
            widgets::action_button(
                t,
                ButtonKind::Primary,
                ButtonSize::Regular,
                "Ver prévia e enviar"
            )
            .on_press(Message::ShowCloudPreview),
        ],
    ]
    .spacing(space::M)
    .into()
}

/// A conversa da previa aberta: so enquanto ela for a aberta e esperar aprovacao. A previa e
/// presa ao id, entao nunca mostra (nem aprova) outra conversa.
pub(super) fn shown_consent(app: &App) -> Option<&OpenConversation> {
    let id = app.consent_preview.as_deref()?;
    awaiting_cloud_consent(app).filter(|o| o.id == id)
}

/// Dialogo "Enviar para a nuvem?" (8.5): o envelope **desta conversa** e as mensagens que vao
/// no pedido (o que de fato sai; o `Resend` do nucleo le o mesmo texto), modelo, retencao e o
/// aviso de valores ocultos. Na tela o texto segue o "Ocultar valores"; o que sai nunca e
/// mascarado. Esc e o scrim so fecham; so o "Cancelar" pode apagar a abertura recem-criada.
pub(super) fn consent_dialog(app: &App) -> Option<Element<'_, Message>> {
    let open = shown_consent(app)?;
    let t = app.tokens;
    let cloud = &app.snapshot.assistant.settings.cloud;
    // O `padding(4)` mantem o cursor da rolagem longe do canto arredondado (o texto fica a 12
    // px da borda: 4 + 8).
    let envelope = widgets::inset(
        t,
        scrollable(
            container(
                text(preview_shown(app, open))
                    .size(12)
                    .font(fonts::INTER)
                    .color(t.text),
            )
            .padding(8)
            .width(Length::Fill),
        )
        .style(theme::scrollbar(t)),
    )
    .height(Length::Fill)
    .padding(4);
    let fact = |label: &'static str, value: String| {
        row![
            text(label).size(13).color(t.text_muted),
            text(value).size(13).color(t.text),
        ]
        .spacing(8)
    };
    let mut col = column![
        text(PREVIEW_TITLE)
            .size(18)
            .font(fonts::INTER_MEDIUM)
            .color(t.text),
        text(preview_intro(open))
            .size(13)
            .line_height(1.45)
            .color(t.text_soft()),
        envelope,
        row![
            fact("Modelo", cloud.model.trim().to_string()),
            Space::new().width(24),
            fact(
                "Retenção",
                retention_value(cloud.deny_data_collection).to_string()
            ),
        ]
        .align_y(Alignment::Center),
    ]
    .spacing(12)
    .height(Length::Fill);
    if let Some(note) = hidden_values_note(app) {
        col = col.push(
            row![
                icons::icon(icons::INFO, 14.0, t.text_soft()),
                text(note).size(12).color(t.text_soft()),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        );
    }
    col = col.push(
        row![
            Space::new().width(Length::Fill),
            widgets::action_button(t, ButtonKind::Secondary, ButtonSize::Regular, "Cancelar")
                .on_press(Message::CancelPreview),
            widgets::action_button(t, ButtonKind::Primary, ButtonSize::Regular, PREVIEW_SEND)
                .on_press(Message::ApproveCloudSend),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    );
    let height = (app.window_size.height - 96.0).clamp(240.0, 600.0);
    Some(widgets::dialog(
        t,
        col,
        app.window_size.width,
        640.0,
        Some(height),
        Message::ClosePreview,
        app.motion.scrim(),
    ))
}

/// Secundario, e nao de perigo: tentar de novo nao apaga nem envia nada novo.
fn retry_button(t: &'static Tokens) -> Element<'static, Message> {
    widgets::action_button(
        t,
        ButtonKind::Secondary,
        ButtonSize::Compact,
        "Tentar de novo",
    )
    .on_press(Message::ChatRetry)
    .into()
}

/// Rotulo do botao "Apagar todas" conforme esteja armado ou nao.
pub(super) fn delete_all_label(armed: bool) -> &'static str {
    if armed {
        "Apagar todas? Clique de novo"
    } else {
        "Apagar todas"
    }
}

/// Conversas anteriores da conta que a tela mostra, com data e hora (todas se chamam "Análise
/// de <mês>"), abrir e apagar com "Desfazer".
fn conversation_list(app: &App) -> Option<Element<'_, Message>> {
    let t = app.tokens;
    let items = conversations_of_book(app);
    if items.is_empty() {
        return None;
    }
    let armed = app.confirm_delete_all.is_some();
    let delete_kind = if armed {
        ButtonKind::DangerFilled
    } else {
        ButtonKind::Danger
    };
    let delete_all =
        widgets::action_button(t, delete_kind, ButtonSize::Compact, delete_all_label(armed))
            .on_press(Message::DeleteAllConversations);
    let mut col = column![
        row![
            widgets::kicker(t, "Conversas anteriores"),
            Space::new().width(Length::Fill),
            delete_all,
        ]
        .align_y(Alignment::Center)
    ]
    .spacing(6)
    .width(Length::Fill);
    for c in items {
        let mut label = column![text(c.title.as_str()).size(14).color(t.text)].spacing(2);
        if let Some(when) = time::date_time(&c.created_at, app.offset) {
            label = label.push(text(when).size(12).color(t.text_muted));
        }
        let title = button(label)
            .padding([6, 8])
            .width(Length::Fill)
            .style(theme::plain(t.text))
            .on_press(Message::OpenConversation(c.id.clone()));
        let delete = widgets::action_button(t, ButtonKind::Danger, ButtonSize::Compact, "Apagar")
            .on_press(Message::AskDeleteConversation(c.id.clone()));
        let line = row![title, delete].spacing(8).align_y(Alignment::Center);
        col = col.push(rows::hover_row(
            app,
            &c.id,
            line,
            [0, 4],
            RowEdge::Inset,
            Some(size::ROW_RICH),
        ));
    }
    Some(col.into())
}

/// O texto da prévia como aparece na tela: com "Ocultar valores" ligado cada R$ vira a máscara
/// (quem esconde os valores não os quer à vista nem aqui). O que SAI para o assistente é
/// sempre `App::current_envelope`, sem máscara; a máscara é só de quem olha.
pub(super) fn preview_text(app: &App) -> Option<Cow<'_, str>> {
    app.envelope_preview.as_deref().map(|s| mask(app, s))
}

/// Sobreposição "O que o assistente vê": o envelope inteiro, rolável, com "Fechar". Mesmo
/// padrão do modal de remoção: fundo escurecido que fecha ao clicar e a caixa por cima, que
/// engole o clique.
pub(super) fn envelope_dialog(app: &App) -> Option<Element<'_, Message>> {
    let t = app.tokens;
    let preview = preview_text(app)?;
    let header = text("O que o assistente vê")
        .size(18)
        .font(fonts::INTER_MEDIUM)
        .color(t.text);
    let intro = text(INTRO).size(13).line_height(1.5).color(t.text_soft());
    // Fonte padrão do app (Inter Hub, com `tnum`: os dígitos alinham). O app não embute uma
    // monoespaçada, e a do sistema teria cobertura incerta para "—" e "−".
    // Caixa interna dentro do dialogo, com o mesmo respiro da previa para o cursor da rolagem.
    let content = widgets::inset(
        t,
        scrollable(
            container(text(preview).size(12).font(fonts::INTER).color(t.text))
                .padding(8)
                .width(Length::Fill),
        )
        .style(theme::scrollbar(t)),
    )
    .height(Length::Fill)
    .padding(4);
    let close = widgets::action_button(t, ButtonKind::Secondary, ButtonSize::Regular, "Fechar")
        .on_press(Message::CloseEnvelope);
    let height = (app.window_size.height - 96.0).clamp(240.0, 640.0);
    Some(widgets::dialog(
        t,
        column![
            header,
            intro,
            content,
            row![Space::new().width(Length::Fill), close]
        ]
        .spacing(12)
        .height(Length::Fill),
        app.window_size.width,
        720.0,
        Some(height),
        Message::CloseEnvelope,
        app.motion.scrim(),
    ))
}

/// "Ocultar valores": os textos dos achados trazem o dinheiro ja formatado ("R$ 1.234,56"),
/// entao a tela troca cada valor pela mascara em vez de formatar de novo. Sem isto o numero
/// vazaria por cima do botao de ocultar.
fn mask<'a>(app: &App, s: &'a str) -> Cow<'a, str> {
    if app.hide_values {
        Cow::Owned(mask_str(s))
    } else {
        Cow::Borrowed(s)
    }
}

/// Troca todo valor em dinheiro do texto pela mascara. Os achados trazem "R$ 1.234,56", mas a
/// resposta do modelo pode vir sem o "R$" ("1.500 reais", "5 mil", "1234.56"), entao ha duas
/// regras:
///
/// - "R$" seguido de numero (depois de qualquer espaco Unicode: NBSP, U+202F, tab) vira `MASK`.
///   O fim do numero sai de `scan_number`: um ponto ou virgula so pertence ao valor quando vem
///   um digito depois, senao "R$ 960,00." comeria o ponto final e "R$ 5 de R$ 9" comeria o
///   " de". "R$" sem digito depois (parte do nome de uma categoria, por ex.) fica literal.
/// - Numero solto com cara de dinheiro vira `MASK_DIGITS`: parte inteira de 3 ou mais digitos
///   ("1500", "1500,5", "1234.56"; um ano tambem cai aqui, e e aceito: vazar valor e pior),
///   milhar com ponto ou espaco ("1.500", "1 234,56"), duas casas ("12,50") ou unidade depois
///   ("3 reais", "5 mil", "1,5 mil", "5k"). Porcentagem ("12,5%", "150%"), meses ("0,5 mês"),
///   datas ("19 nov", "24/09") e numero pequeno ("3 meses") ficam.
///
/// A ordem de grandeza ("mil", "milhões", "k") some junto com o numero: "••••• mil" ainda diria
/// o tamanho do valor. "reais" fica, como o resto da frase.
///
/// O sinal colado ao valor ("−", "-" ou "+") sai junto: oculto nao mostra o sinal, como o
/// `signed_brl` do dashboard. O "~" de estimativa e a pontuacao em volta ficam.
fn mask_str(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut copied = 0;
    let mut i = 0;
    while i < b.len() {
        if s[i..].starts_with("R$") {
            let after = i + 2;
            let number = s.len() - s[after..].trim_start().len();
            if b.get(number).is_some_and(u8::is_ascii_digit) {
                let n = scan_number(s, number);
                let end = magnitude_end(s, n.end).unwrap_or(n.end);
                out.push_str(strip_sign(&s[copied..i]));
                out.push_str(MASK);
                copied = end;
                i = end;
            } else {
                i = after;
            }
            continue;
        }
        if b[i].is_ascii_digit() {
            let word = s[..i]
                .chars()
                .next_back()
                .is_some_and(char::is_alphanumeric);
            let n = scan_number(s, i);
            let mut run_end = n.end;
            if !word && let Some(end) = bare_money_end(s, &n) {
                out.push_str(strip_sign(&s[copied..i]));
                out.push_str(MASK_DIGITS);
                copied = end;
                run_end = end;
            }
            // Pula o numero inteiro: recomecar no meio dele acharia "500,00" em "x1500,00".
            i = run_end.max(i + 1);
            continue;
        }
        i += s[i..].chars().next().map_or(1, char::len_utf8);
    }
    out.push_str(&s[copied..]);
    out
}

/// Um numero lido por `scan_number`.
struct Number {
    /// Fim do numero (byte); termina sempre num digito.
    end: usize,
    /// Digitos da parte inteira, milhares incluidos.
    int_digits: usize,
    /// Tem separador de milhar ("1.500", "1 234").
    grouped: bool,
    /// Termina em virgula ou ponto com exatamente duas casas ("12,50", "15.90").
    cents: bool,
}

/// Espaco que separa milhar ("1 234,56"): o comum, o NBSP, o fino e o U+202F que alguns
/// formatadores usam. Devolve o tamanho em bytes.
fn thousands_space(s: &str, at: usize) -> Option<usize> {
    match s.get(at..)?.chars().next()? {
        c @ (' ' | '\u{a0}' | '\u{2009}' | '\u{202f}') => Some(c.len_utf8()),
        _ => None,
    }
}

/// Le o numero que comeca no digito `start`: parte inteira, grupos de milhar (ponto ou espaco
/// e exatamente 3 digitos, so se a primeira parte tem ate 3) e parte decimal (virgula ou ponto
/// seguido de digito). Separador sem digito depois nao entra: e pontuacao da frase.
fn scan_number(s: &str, start: usize) -> Number {
    let b = s.as_bytes();
    let digits = |from: usize| {
        b.get(from..)
            .map_or(0, |r| r.iter().take_while(|c| c.is_ascii_digit()).count())
    };
    let lead = digits(start);
    let mut end = start + lead;
    let mut int_digits = lead;
    let mut grouped = false;
    if lead <= 3 {
        loop {
            let sep = if b.get(end) == Some(&b'.') {
                Some(1)
            } else {
                thousands_space(s, end)
            };
            match sep {
                Some(n) if digits(end + n) == 3 => {
                    end += n + 3;
                    int_digits += 3;
                    grouped = true;
                }
                _ => break,
            }
        }
    }
    let mut cents = false;
    if matches!(b.get(end), Some(b',' | b'.')) {
        let frac = digits(end + 1);
        if frac > 0 {
            cents = frac == 2;
            end += 1 + frac;
        }
    }
    Number {
        end,
        int_digits,
        grouped,
        cents,
    }
}

/// A palavra logo depois de `at` (pulando espacos), em minusculas, e onde ela termina. Palavra
/// inteira: "5 kg" le "kg", e nao "k".
fn next_word(s: &str, at: usize) -> Option<(String, usize)> {
    let rest = s.get(at..)?;
    let start = at + (rest.len() - rest.trim_start().len());
    let len: usize = s[start..]
        .chars()
        .take_while(|c| c.is_alphanumeric())
        .map(char::len_utf8)
        .sum();
    (len > 0).then(|| (s[start..start + len].to_lowercase(), start + len))
}

/// Ordem de grandeza depois do numero: some junto com ele.
const MAGNITUDES: [&str; 4] = ["mil", "milhão", "milhões", "k"];
/// Unidade depois do numero: faz dele dinheiro, mas fica na frase.
const UNITS: [&str; 2] = ["reais", "real"];

/// Fim da ordem de grandeza ("mil", "k") depois do numero que termina em `at`.
fn magnitude_end(s: &str, at: usize) -> Option<usize> {
    next_word(s, at).and_then(|(w, end)| MAGNITUDES.contains(&w.as_str()).then_some(end))
}

/// Fim do trecho a mascarar se o numero solto `n` tem cara de dinheiro; `None` para o resto
/// ("12,5%", "0,5 mês", "3 meses").
fn bare_money_end(s: &str, n: &Number) -> Option<usize> {
    if s[n.end..].trim_start().starts_with('%') {
        return None;
    }
    if let Some(end) = magnitude_end(s, n.end) {
        return Some(end);
    }
    let unit = next_word(s, n.end).is_some_and(|(w, _)| UNITS.contains(&w.as_str()));
    (n.int_digits >= 3 || n.grouped || n.cents || unit).then_some(n.end)
}

/// Tira o sinal colado ao fim de `before` ("−" tipografico, "-" ou "+"), so quando ele nao
/// emenda numa palavra: "item-1.500" nao e sinal.
fn strip_sign(before: &str) -> &str {
    for sign in [MINUS, "-", "+"] {
        if let Some(rest) = before.strip_suffix(sign)
            && !rest.chars().next_back().is_some_and(char::is_alphanumeric)
        {
            return rest;
        }
    }
    before
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::assistant_settings_view;

    #[test]
    fn mascara_troca_valores_e_mantem_o_resto() {
        assert_eq!(
            mask_str("Gasto R$ 1.234,56 de R$ 2.000,00 (passou R$ 0,00)."),
            format!("Gasto {MASK} de {MASK} (passou {MASK}).")
        );
    }

    #[test]
    fn mascara_tira_o_sinal_de_menos_e_mantem_o_til() {
        assert_eq!(
            mask_str("Saldo \u{2212}R$ 5,00 hoje"),
            format!("Saldo {MASK} hoje")
        );
        assert_eq!(
            mask_str("faltar ~R$ 960,00 no fim do mês"),
            format!("faltar ~{MASK} no fim do mês")
        );
    }

    #[test]
    fn mascara_preserva_ponto_final() {
        assert_eq!(mask_str("sobrou R$ 960,00."), format!("sobrou {MASK}."));
    }

    #[test]
    fn mascara_deixa_r_cifrao_sem_digito_como_texto() {
        assert_eq!(mask_str("Fundo R$ extra"), "Fundo R$ extra");
        assert_eq!(
            mask_str("Fundo R$ extra: R$ 5,00"),
            format!("Fundo R$ extra: {MASK}")
        );
    }

    #[test]
    fn barras_dos_achados_leem_gasto_e_referencia_na_ordem_de_cada_regra() {
        use crate::dashboard::{GRAFITE_TODAY, fixtures_grafite};
        let analysis = crate::advisor::analyze(&fixtures_grafite(), GRAFITE_TODAY);
        // Pelo indice da cifra, e nao pelo valor: dois valores iguais (gasto igual a meta)
        // fariam a busca por valor achar o rotulo errado e o teste passar por acaso.
        let money_at = |f: &Finding, i: usize| match f.figures[i].value {
            FigureValue::Money(v) => v,
            ref other => panic!("{:?}: cifra {i} nao e dinheiro: {other:?}", f.kind),
        };
        let index_of = |f: &Finding, pick: &dyn Fn(&str) -> bool| {
            let found: Vec<usize> = (0..f.figures.len())
                .filter(|&i| pick(&f.figures[i].label))
                .collect();
            assert_eq!(found.len(), 1, "{:?}: {:?}", f.kind, f.figures);
            found[0]
        };
        let mut seen = 0;
        for f in &analysis.findings {
            let Some(c) = comparison(f) else {
                assert!(
                    !matches!(
                        f.kind,
                        FindingKind::Pace | FindingKind::CategoryAboveNormal | FindingKind::Budget
                    ),
                    "{:?}",
                    f.kind
                );
                continue;
            };
            seen += 1;
            // O indice do gasto e o da cifra cujo rotulo fala de gasto, pelo texto da regra.
            let (real, reference): (usize, usize) = match f.kind {
                FindingKind::Pace => (
                    index_of(f, &|l| l == "Gasto até hoje"),
                    index_of(f, &|l| l == "Normal até hoje"),
                ),
                FindingKind::CategoryAboveNormal => (
                    index_of(f, &|l| l.ends_with(" no mês")),
                    index_of(f, &|l| l.ends_with(" normal")),
                ),
                FindingKind::Budget => {
                    assert_eq!(c.reference_label, "Meta");
                    (
                        index_of(f, &|l| l.starts_with("Gasto ")),
                        index_of(f, &|l| l.starts_with("Meta ")),
                    )
                }
                other => panic!("{other:?}"),
            };
            assert_eq!(c.real, money_at(f, real), "{:?}", f.kind);
            assert_eq!(c.reference, money_at(f, reference), "{:?}", f.kind);
        }
        assert!(seen >= 3, "a fixture tem ritmo, categoria e metas");
    }

    #[test]
    fn barras_dos_achados_montam_com_e_sem_valores_ocultos() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = crate::ui::app::tests::app_with_fixtures(dir.path());
        for hidden in [false, true] {
            app.hide_values = hidden;
            let f = app
                .analysis
                .findings
                .iter()
                .find(|f| f.kind == FindingKind::Pace)
                .cloned()
                .unwrap();
            let c = comparison(&f).unwrap();
            let _ = comparison_view(&app, f.severity, &c);
        }
    }

    #[test]
    fn mascara_nao_deixa_valor_nenhum_nos_achados_nem_na_resposta() {
        use crate::dashboard::money::leaks;
        use crate::dashboard::{GRAFITE_TODAY, fixtures_grafite};
        let analysis = crate::advisor::analyze(&fixtures_grafite(), GRAFITE_TODAY);
        let mut texts: Vec<&str> = analysis
            .findings
            .iter()
            .flat_map(|f| {
                [
                    Some(f.title.as_str()),
                    Some(f.detail.as_str()),
                    f.action.as_deref(),
                ]
            })
            .flatten()
            .collect();
        assert!(
            texts.iter().any(|t| leaks(t).is_some()),
            "o teste so vale se algum texto trouxer dinheiro"
        );
        // Resposta do modelo sem "R$": o prompt pede o formato, mas a mascara nao confia nele.
        let reply = "Você gastou 1.500 reais em mercado e faltam 5.500,00 para a meta; \
                     no mês passado foram 1500,00 e o saldo ficou -R$ 3,00.";
        texts.push(reply);
        for t in texts {
            let masked = mask_str(t);
            assert_eq!(leaks(&masked), None, "{t:?} -> {masked:?}");
        }
        assert_eq!(
            mask_str(reply),
            format!(
                "Você gastou {MASK_DIGITS} reais em mercado e faltam {MASK_DIGITS} para a meta; \
                 no mês passado foram {MASK_DIGITS} e o saldo ficou {MASK}."
            )
        );
    }

    #[test]
    fn mascara_tira_o_hifen_antes_do_valor_so_quando_e_sinal() {
        assert_eq!(mask_str("saldo -R$ 3,00"), format!("saldo {MASK}"));
        assert_eq!(mask_str("saldo -1.500"), format!("saldo {MASK_DIGITS}"));
        assert_eq!(mask_str("item-1.500"), format!("item-{MASK_DIGITS}"));
    }

    #[test]
    fn mascara_deixa_numeros_que_nao_sao_dinheiro() {
        for t in [
            "Você guarda 80% da renda",
            "cobre só 0,5 mês",
            "subiu 12,5% no mês",
            "subiu 150% no ano",
            "3 meses, dia 24/09",
            "fechou em 19 nov",
            "A1234",
            "5 kg de arroz e 5 km",
            "5 mililitros",
        ] {
            assert_eq!(mask_str(t), t);
        }
        // Um ano tem 4 digitos e cai na regra do inteiro longo: aceito, vazar valor e pior.
        assert_eq!(
            mask_str("subiu 12,5% em 2026"),
            format!("subiu 12,5% em {MASK_DIGITS}")
        );
    }

    /// Os formatos que o modelo usa sem "R$": cada um vira mascara, sem digito nenhum sobrando
    /// (o `leaks` so ve "d,dd" e "R$ d", entao o texto exato e conferido tambem).
    #[test]
    fn mascara_pega_valor_sem_cifrao_em_qualquer_formato() {
        use crate::dashboard::money::leaks;
        let d = MASK_DIGITS;
        let cases = [
            ("gastou 1500 reais", format!("gastou {d} reais")),
            ("gastou 3 reais", format!("gastou {d} reais")),
            ("1 real a mais", format!("{d} real a mais")),
            ("sobram 5 mil", format!("sobram {d}")),
            ("sobram 1,5 mil.", format!("sobram {d}.")),
            ("sobram 5k no mês", format!("sobram {d} no mês")),
            ("perto de 2 milhões", format!("perto de {d}")),
            ("total 1500 no mês", format!("total {d} no mês")),
            ("total 1234.56", format!("total {d}")),
            ("total 1500,5 hoje", format!("total {d} hoje")),
            ("total 1 234,56 hoje", format!("total {d} hoje")),
            ("total 1\u{a0}234,56", format!("total {d}")),
            ("total 1\u{202f}234,56", format!("total {d}")),
            ("total 12,50 hoje", format!("total {d} hoje")),
            ("saldo +1.500", format!("saldo {d}")),
            ("saldo +R$ 3,00", format!("saldo {MASK}")),
            ("R$ 5 mil no mês", format!("{MASK} no mês")),
            ("R$ 1 234,56.", format!("{MASK}.")),
        ];
        for (input, expected) in cases {
            let masked = mask_str(input);
            assert_eq!(masked, expected, "{input:?}");
            assert_eq!(leaks(&masked), None, "{input:?} -> {masked:?}");
            assert!(
                !masked.chars().any(|c| c.is_ascii_digit()),
                "{input:?} -> {masked:?}"
            );
        }
    }

    #[test]
    fn mascara_apara_todo_espaco_unicode_depois_do_cifrao() {
        use crate::dashboard::money::leaks;
        for space in [" ", "\u{a0}", "\u{202f}", "\t", "\u{2009}", "  "] {
            let input = format!("custou R${space}1.234,56 hoje");
            let masked = mask_str(&input);
            assert_eq!(masked, format!("custou {MASK} hoje"), "{input:?}");
            assert_eq!(leaks(&masked), None, "{input:?}");
        }
    }

    #[test]
    fn previa_mascara_valores_so_na_tela_e_o_envio_continua_inteiro() {
        use crate::ui::app::tests::app_with_fixtures;
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with_fixtures(dir.path());
        let _ = crate::ui::app::update(&mut app, Message::ShowEnvelope);
        let full = app.current_envelope();
        assert!(full.contains("R$ "), "o envelope do exemplo traz dinheiro");

        app.hide_values = false;
        assert_eq!(preview_text(&app).as_deref(), Some(full.as_str()));

        app.hide_values = true;
        let shown = preview_text(&app).expect("previa aberta");
        let leaked = shown
            .match_indices("R$ ")
            .any(|(i, _)| shown[i + 3..].starts_with(|c: char| c.is_ascii_digit()));
        assert!(!leaked, "valor apareceu na previa com valores ocultos");
        assert!(shown.contains(MASK));
        // O texto que sai (e o guardado na previa) nunca e o mascarado.
        assert_eq!(app.current_envelope(), full);
        assert_eq!(app.envelope_preview.as_deref(), Some(full.as_str()));
        assert!(envelope_dialog(&app).is_some());
    }

    #[test]
    fn selo_da_nuvem_diz_a_politica_do_envio() {
        assert_eq!(cloud_badge(Some(true)), "enviado à nuvem · sem retenção");
        assert_eq!(
            cloud_badge(Some(false)),
            "enviado à nuvem · retenção permitida"
        );
        assert_eq!(cloud_badge(None), "enviado à nuvem");
    }

    #[test]
    fn rodape_da_resposta_mostra_o_modelo_local_antes_e_o_da_nuvem_depois() {
        let mut m = MessageView {
            id: "m".into(),
            role: MessageRole::Assistant,
            content: String::new(),
            sent_to_cloud: false,
            deny_data_collection: None,
            provider: Some("local".into()),
            model: Some("llama3.1:8b".into()),
            tokens_in: Some(1240),
            tokens_out: Some(380),
            interrupted: false,
        };
        assert_eq!(
            reply_notes(&m).join(" · "),
            "llama3.1:8b · 1.240 → 380 tokens"
        );
        m.sent_to_cloud = true;
        m.provider = Some("cloud".into());
        m.model = Some("openai/gpt-4o".into());
        m.deny_data_collection = Some(true);
        assert_eq!(
            reply_notes(&m).join(" · "),
            "1.240 → 380 tokens · enviado à nuvem · sem retenção · openai/gpt-4o"
        );
        m.interrupted = true;
        assert!(reply_notes(&m).contains(&"interrompida".to_string()));
        // Registro antigo, sem modelo: nada de "None" na tela.
        m.model = None;
        m.provider = None;
        assert_eq!(
            reply_notes(&m).join(" · "),
            "1.240 → 380 tokens · interrompida · enviado à nuvem · sem retenção"
        );
    }

    fn conversation_with(messages: Vec<MessageView>) -> OpenConversation {
        OpenConversation {
            id: "C1".into(),
            book: crate::dashboard::contract::Book::Personal,
            envelope: String::new(),
            messages,
            needs_cloud_consent: true,
        }
    }

    fn msg(role: MessageRole, tokens: Option<u32>) -> MessageView {
        MessageView {
            id: "m".into(),
            role,
            content: String::new(),
            sent_to_cloud: false,
            deny_data_collection: None,
            provider: None,
            model: None,
            tokens_in: tokens,
            tokens_out: tokens,
            interrupted: false,
        }
    }

    #[test]
    fn total_do_cabecalho_agrupa_milhares_e_some_sem_uso_informado() {
        assert_eq!(tokens_label((1240, 380)), "1.240 → 380 tokens");
        assert_eq!(tokens_label((1_234_567, 0)), "1.234.567 → 0 tokens");
        let none = conversation_with(vec![
            msg(MessageRole::User, None),
            msg(MessageRole::Assistant, None),
        ]);
        assert_eq!(tokens_total_label(&none), "—");
        let some = conversation_with(vec![
            msg(MessageRole::User, None),
            msg(MessageRole::Assistant, Some(1500)),
        ]);
        assert_eq!(tokens_total_label(&some), "1.500 → 1.500 tokens");
        // Uso informado como zero e uma medida, nao ausencia.
        let zero = conversation_with(vec![msg(MessageRole::Assistant, Some(0))]);
        assert_eq!(tokens_total_label(&zero), "0 → 0 tokens");
    }

    #[test]
    fn cancelar_a_previa_apaga_a_conversa_que_nunca_foi_respondida() {
        let opening = || {
            let mut m = msg(MessageRole::User, None);
            m.content = OPENING_REQUEST.into();
            m
        };
        // So a abertura recem-criada, sem nada chegando, e apagada.
        let fresh = conversation_with(vec![opening()]);
        assert!(matches!(
            cancel_preview_message(&fresh, false),
            Some(Message::DeleteConversation(id)) if id == "C1"
        ));
        assert!(cancel_preview_message(&fresh, true).is_none());
        // Conversa local com erro, reaberta do historico com a nuvem configurada: tem o que
        // guardar, o "Cancelar" so fecha a previa.
        let failed = conversation_with(vec![opening(), msg(MessageRole::Error, None)]);
        assert!(cancel_preview_message(&failed, false).is_none());
        // Pergunta da propria pessoa (nao a abertura) tambem fica.
        let asked = conversation_with(vec![msg(MessageRole::User, None)]);
        assert!(cancel_preview_message(&asked, false).is_none());
        let answered = conversation_with(vec![
            opening(),
            msg(MessageRole::Assistant, None),
            msg(MessageRole::User, None),
        ]);
        assert!(cancel_preview_message(&answered, false).is_none());

        // Pela tela.
        use crate::assistant::settings::{ProviderChoice, Settings};
        use crate::hub::{AssistantCommand, Command};
        use crate::ui::app::tests::app_with_fixtures;
        use crate::ui::app::{Screen, update};
        let dir = tempfile::tempdir().unwrap();
        let (mut app, mut rx, _tx) = app_with_fixtures(dir.path());
        let _ = update(&mut app, Message::SelectScreen(Screen::Analysis));
        let mut cloud = Settings {
            provider: ProviderChoice::Cloud,
            ..Settings::default()
        };
        cloud.cloud.enabled = true;
        app.snapshot.assistant.settings = cloud;
        let mut failed = failed;
        failed.book = app.book;
        app.snapshot.assistant.open = Some(failed);
        app.consent_preview = Some("C1".into());
        assert!(consent_dialog(&app).is_some());
        let _ = shell::root(&app);
        while rx.try_recv().is_ok() {}
        // Esc/scrim e "Cancelar" so fecham a previa: nada sai para o nucleo.
        let _ = update(&mut app, Message::ClosePreview);
        assert!(app.consent_preview.is_none());
        app.consent_preview = Some("C1".into());
        let _ = update(&mut app, Message::CancelPreview);
        assert!(app.consent_preview.is_none());
        assert!(
            rx.try_recv().is_err(),
            "a conversa com erro nao foi apagada"
        );

        // Resposta local chegando e a troca para a nuvem: nao ha o que aprovar ainda, a previa
        // nao aparece e o "Cancelar" nao apaga nada.
        let mut streaming = fresh;
        streaming.book = app.book;
        app.snapshot.assistant.open = Some(streaming);
        app.snapshot.assistant.busy = true;
        app.consent_preview = Some("C1".into());
        assert!(crate::ui::app::awaiting_cloud_consent(&app).is_none());
        assert!(consent_dialog(&app).is_none());
        let _ = update(&mut app, Message::CancelPreview);
        assert!(
            rx.try_recv().is_err(),
            "a conversa com resposta chegando ficou"
        );
        // Terminada a resposta, a abertura sozinha e apagada pelo "Cancelar".
        app.snapshot.assistant.busy = false;
        app.consent_preview = Some("C1".into());
        let _ = update(&mut app, Message::CancelPreview);
        assert!(matches!(
            rx.try_recv(),
            Ok(Command::Assistant(AssistantCommand::Delete(id))) if id == "C1"
        ));
    }

    #[test]
    fn previa_com_valores_ocultos_avisa_que_o_envio_leva_os_reais() {
        use crate::ui::app::tests::app_with_fixtures;
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with_fixtures(dir.path());
        app.hide_values = false;
        assert_eq!(hidden_values_note(&app), None);
        app.hide_values = true;
        assert_eq!(
            hidden_values_note(&app),
            Some("Os valores estão ocultos na tela; o envio leva os valores reais.")
        );
    }

    #[test]
    fn linha_de_retencao_da_previa() {
        assert_eq!(retention_value(true), "não permitida");
        assert_eq!(retention_value(false), "permitida");
    }

    #[test]
    fn textos_da_nuvem_nao_tem_espaco_duplo() {
        for text in [
            PREVIEW_INTRO,
            PREVIEW_INTRO_EARLIER,
            assistant_settings_view::CLOUD_EXPLAIN,
            assistant_settings_view::DENY_LABEL,
            assistant_settings_view::DENY_WARNING,
            assistant_settings_view::PLAIN_KEY_WARNING,
        ] {
            assert!(!text.contains("  "), "{text:?}");
        }
        assert!(
            assistant_settings_view::CLOUD_EXPLAIN.starts_with(
                "Ligado, o resumo de achados e totais desta conta vai para o OpenRouter"
            )
        );
    }

    /// Lista "Baixo" do relatorio de 25/11: programa de mesa, comando sem crase literal e o
    /// destino no botao de envio.
    #[test]
    fn textos_do_assistente_falam_de_programa_de_mesa() {
        use assistant_settings_view::{MODELS_NOT_LISTED, NO_MODELS_HINT};
        for text in [MODELS_NOT_LISTED, NO_MODELS_HINT, PREVIEW_SEND] {
            assert!(!text.contains("  "), "{text:?}");
            assert!(!text.contains('`'), "{text:?}");
            assert!(!text.contains("Toque"), "{text:?}");
        }
        assert!(MODELS_NOT_LISTED.starts_with("Clique em Testar conexão"));
        assert!(NO_MODELS_HINT.contains("ollama pull llama3.1:8b no terminal"));
        assert_eq!(PREVIEW_SEND, "Enviar ao OpenRouter");
    }

    #[test]
    fn texto_da_previa_nao_tem_espaco_duplo() {
        assert!(!INTRO.contains("  "), "{INTRO:?}");
        assert!(INTRO.contains("sem lançamentos, descrições"));
    }

    #[test]
    fn linha_resumo_conta_por_gravidade_no_singular_e_no_plural() {
        use crate::advisor::FindingKind;
        let f = |severity| Finding {
            kind: FindingKind::Pace,
            severity,
            title: String::new(),
            detail: String::new(),
            action: None,
            figures: Vec::new(),
            impact_minor: 0,
        };
        let many = vec![
            f(Severity::Alert),
            f(Severity::Alert),
            f(Severity::Attention),
            f(Severity::Attention),
            f(Severity::Attention),
            f(Severity::GoodSign),
        ];
        assert_eq!(
            summary_line(&many),
            "2 alertas, 3 atenções, 1 bom sinal. Calculado no seu PC."
        );
        let few = vec![
            f(Severity::Attention),
            f(Severity::GoodSign),
            f(Severity::GoodSign),
        ];
        assert_eq!(
            summary_line(&few),
            "1 atenção, 2 bons sinais. Calculado no seu PC."
        );
        assert_eq!(summary_line(&[]), "Nada a apontar. Calculado no seu PC.");
    }

    #[test]
    fn dois_paineis_contra_o_segmentado_em_728() {
        use crate::ui::app::tests::app_with_fixtures;
        use crate::ui::app::{AnalysisTab, Screen, update};
        // 1024 de janela = 728 de largura util: segmentado. 1440 = 1144: dois paineis.
        assert!(!two_panels(728.0));
        assert!(two_panels(1144.0));
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with_fixtures(dir.path());
        let _ = update(&mut app, Message::SelectScreen(Screen::Analysis));
        app.window_size.width = 1024.0;
        assert_eq!(shell::content_width(&app), 728.0);
        assert!(!two_panels(shell::content_width(&app)));
        assert_eq!(app.analysis_tab, AnalysisTab::Findings);
        for tab in [AnalysisTab::Chat, AnalysisTab::Findings] {
            let _ = update(&mut app, Message::AnalysisTab(tab));
            assert_eq!(app.analysis_tab, tab);
            let _ = shell::root(&app);
        }
        // Abrir uma conversa leva ao painel da conversa.
        let _ = update(&mut app, Message::OpenConversation("C1".into()));
        assert_eq!(app.analysis_tab, AnalysisTab::Chat);
        app.window_size.width = 1440.0;
        assert!(two_panels(shell::content_width(&app)));
        let _ = shell::root(&app);
    }

    /// Respostas com dinheiro em Markdown: o que a tela desenha (texto dos itens, ou o texto
    /// puro de reserva) nunca deixa valor, nem quando a marcacao corta o numero.
    #[test]
    fn markdown_mascarado_nao_vaza_valor() {
        use crate::dashboard::money::leaks;
        let cases = [
            "**Mercado:** R$ 1.234,56 em novembro.",
            "1. Delivery: *R$ 612,00*\n2. Mercado: `R$ 980,10`\n",
            "# Resumo\n\n- saldo -R$ 3,00\n- meta de 5.500,00\n\n> sobram 1.500 reais",
            "Valor 1.**234**,56 com enfase no meio.",
            "Escapado 1\\.234,56 no texto.",
            "Entidade &#49;.234,56 no texto.",
            "```\ntotal R$ 9.876,54\n```",
            "[R$ 10,00](https://exemplo.invalid) de link e ![R$ 20,00](img.png)",
            "| Categoria | Valor |\n|---|---|\n| Mercado | R$ 1.**234**,56 |",
            "| x | y |\n|---|---|\n| x | 1.~~234~~,56 |",
            "| x | y |\n|---|---|\n| x | 1.<b>234</b>,56 |",
            "| x | y |\n|---|---|\n| a | b |\n\nTexto 1.<b>234</b>,56 fora",
            "| x | y |\n|---|---|\n| x | 1\\.234,56 |",
        ];
        for case in cases {
            let r = render_reply(case, true);
            let shown = match &r.shown {
                ReplyShown::Markdown(items) => rendered_text(items).expect("sem tabela"),
                ReplyShown::Plain(s) => s.clone(),
            };
            assert_eq!(leaks(&shown), None, "{case:?} -> {shown:?}");
            assert_eq!(mask_str(&shown), shown, "{case:?} -> {shown:?}");
            assert_eq!(leaks(&r.copy), None, "copia: {case:?} -> {:?}", r.copy);
            assert_eq!(mask_str(&r.copy), r.copy, "copia: {case:?}");
            // Quem le junta "1.~~234~~,56" de cabeca: sem as marcas, continua sem valor.
            let unmarked: String = shown
                .replace("<b>", "")
                .replace("</b>", "")
                .chars()
                .filter(|c| !matches!(c, '*' | '_' | '`' | '~' | '\\'))
                .collect();
            assert_eq!(leaks(&unmarked), None, "{case:?} -> {shown:?}");
            assert_eq!(mask_str(&unmarked), unmarked, "{case:?} -> {shown:?}");
        }
        // A reserva da tabela le as celulas: o valor da celula vira mascara, o rotulo fica.
        let table = render_reply("| x | y |\n|---|---|\n| Mercado | 1.~~234~~,56 |", true);
        assert!(table.copy.contains("Mercado"), "{}", table.copy);
        assert!(table.copy.contains(MASK_DIGITS), "{}", table.copy);
        // Os casos comuns continuam em Markdown; os tortos caem no texto puro.
        assert!(matches!(
            render_reply("**Mercado:** R$ 1.234,56", true).shown,
            ReplyShown::Markdown(_)
        ));
        assert!(matches!(
            render_reply("Valor 1.**234**,56", true).shown,
            ReplyShown::Plain(_)
        ));
        // Com os valores a mostra e o Markdown da resposta, e a copia e o texto inteiro.
        let shown = render_reply("**R$ 1.234,56**", false);
        assert!(matches!(shown.shown, ReplyShown::Markdown(_)));
        assert_eq!(shown.copy, "**R$ 1.234,56**");
    }

    #[test]
    fn texto_chegando_esconde_o_numero_pela_metade_no_fim() {
        use crate::dashboard::money::leaks;
        for partial in ["gastou 1500,0", "gastou 1.23", "saldo 1.234,5"] {
            let shown = mask_streaming(partial);
            assert!(
                !shown.bytes().any(|b| b.is_ascii_digit()),
                "{partial:?} -> {shown:?}"
            );
        }
        assert_eq!(mask_streaming("gastou 1500,0"), "gastou ");
        // Numero terminado (com texto depois) segue a mascara de sempre; o resto fica.
        let done = mask_streaming("gastou R$ 1.500,00 em mercado e 3");
        assert_eq!(leaks(&done), None);
        assert!(
            done.starts_with(&format!("gastou {MASK} em mercado e ")),
            "{done}"
        );
        assert_eq!(mask_streaming("tudo certo."), "tudo certo.");
    }

    /// O streaming mascara o texto desenhado (`plain_text` do parcial): a cada pedaco que chega,
    /// nenhum digito de valor aparece, nem quando a unidade, o milhar ou a enfase ainda nao
    /// chegaram.
    #[test]
    fn texto_chegando_nao_mostra_digito_em_prefixo_nenhum() {
        use crate::dashboard::money::leaks;
        let full = [
            "gastou 5 mil no mês",
            "sobram 1,5 mil hoje",
            "total 1 234,56 hoje",
            "foi R$ **1500** no mês",
            "foi 1.**234**,56 hoje",
            "foi R\\$ 1.500 hoje",
            "saldo -1.500 e +R$ 3,00 ok",
        ];
        for text in full {
            for (i, _) in text.char_indices().skip(1).chain([(text.len(), ' ')]) {
                let partial = &text[..i];
                let shown = mask_streaming(&plain_text(partial));
                assert!(
                    !shown.chars().any(|c| c.is_ascii_digit()),
                    "{partial:?} -> {shown:?}"
                );
                assert_eq!(leaks(&shown), None, "{partial:?} -> {shown:?}");
            }
        }
        // Terminados, os tres casos de Markdown viram a mascara exata.
        assert_eq!(
            mask_streaming(&plain_text("foi R$ **1500** no mês")),
            format!("foi {MASK} no mês")
        );
        assert_eq!(
            mask_streaming(&plain_text("foi 1.**234**,56 hoje")),
            format!("foi {MASK_DIGITS} hoje")
        );
        assert_eq!(
            mask_streaming(&plain_text("foi R\\$ 1.500 hoje")),
            format!("foi {MASK} hoje")
        );
    }

    /// A previa com o olho ligado mascara o texto que as respostas anteriores desenham; com ele
    /// desligado continua o payload exato, Markdown incluido.
    #[test]
    fn previa_mascara_o_texto_desenhado_das_respostas_anteriores() {
        use crate::assistant::view::MessageRole;
        use crate::dashboard::money::leaks;
        use crate::ui::app::tests::app_with_fixtures;
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with_fixtures(dir.path());
        let replies = ["foi R$ **1500**", "foi 1.**234**,56", "foi R\\$ 1.500"];
        let mut messages = Vec::new();
        for (i, reply) in replies.iter().enumerate() {
            let mut q = msg(MessageRole::User, None);
            q.id = format!("q{i}");
            q.content = "e agora?".into();
            let mut a = msg(MessageRole::Assistant, None);
            a.id = format!("a{i}");
            a.content = (*reply).to_string();
            messages.extend([q, a]);
        }
        let mut pending = msg(MessageRole::User, None);
        pending.id = "q".into();
        pending.content = "e o resto?".into();
        messages.push(pending);
        let open = conversation_with(messages);

        app.hide_values = false;
        let exact = preview_shown(&app, &open);
        assert_eq!(exact, preview_body(&open));
        for reply in replies {
            assert!(exact.contains(reply), "{exact}");
        }

        app.hide_values = true;
        let shown = preview_shown(&app, &open);
        assert_eq!(leaks(&shown), None, "{shown}");
        assert!(!shown.chars().any(|c| c.is_ascii_digit()), "{shown}");
        assert!(
            shown.contains(&format!("Assistente: foi {MASK}\n")),
            "{shown}"
        );
        assert!(
            shown.contains(&format!("Assistente: foi {MASK_DIGITS}\n")),
            "{shown}"
        );
        assert!(shown.ends_with("Você: e o resto?"), "{shown}");
    }

    #[test]
    fn copiar_a_resposta_leva_o_texto_mascarado_com_o_olho_ligado() {
        use crate::assistant::view::MessageRole;
        use crate::ui::app::tests::app_with_fixtures;
        use crate::ui::app::{Screen, update};
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with_fixtures(dir.path());
        let _ = update(&mut app, Message::SelectScreen(Screen::Analysis));
        let mut reply = msg(MessageRole::Assistant, None);
        reply.id = "R1".into();
        reply.content = "Gastou **R$ 1.234,56** no mercado.".into();
        let mut open = conversation_with(vec![msg(MessageRole::User, None), reply]);
        open.book = app.book;
        let mut snapshot = app.snapshot.clone();
        snapshot.assistant.open = Some(open);
        let _ = update(&mut app, Message::Snapshot(Box::new(snapshot)));

        app.hide_values = false;
        app.refresh_replies();
        assert_eq!(app.replies["R1"].copy, "Gastou **R$ 1.234,56** no mercado.");
        let _ = update(&mut app, Message::ToggleHideValues);
        assert!(app.hide_values);
        // A troca do olho refaz o cache: nada montado com os valores a mostra fica valendo.
        let cached = &app.replies["R1"];
        assert!(cached.hidden);
        // O texto desenhado, nao o fonte: sem as marcas do Markdown.
        assert_eq!(cached.copy, format!("Gastou {MASK} no mercado."));
        let task = update(&mut app, Message::CopyReply("R1".into()));
        assert_eq!(task.units(), 2);
        assert!(app.is_copied(CopyTarget::Reply));
        assert_eq!(app.copied_reply.as_deref(), Some("R1"));
        let _ = view(&app);
        // Mensagem que nao esta na conversa aberta: nada a copiar.
        app.copied = None;
        let task = update(&mut app, Message::CopyReply("X".into()));
        assert_eq!(task.units(), 0);
        assert!(!app.is_copied(CopyTarget::Reply));
    }

    #[test]
    fn mascara_nao_mexe_em_texto_sem_dinheiro() {
        let s = "Gasto 35% acima do normal em 12 dias.";
        assert_eq!(mask_str(s), s);
        assert_eq!(mask_str(""), "");
    }
}
