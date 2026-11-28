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
use super::{fonts, icons, rows, shell, widgets};
use crate::advisor::{Finding, Severity};
use crate::assistant::prompt::OPENING_REQUEST;
use crate::assistant::view::{ConversationSummary, MessageRole, MessageView, OpenConversation};
use crate::dashboard::money::MASK;
use crate::ui_model::analysis::{
    Comparison, HIDDEN_VALUES_NOTE, INTRO, PREVIEW_SEND, PREVIEW_TITLE, assistant_status,
    comparison, delete_all_label, group_title, mask_str, mask_streaming, plain_text, preview_body,
    preview_intro, preview_text_with, reply_notes, retention_value, summary_line,
    tokens_total_label, two_panels,
};
use crate::ui_model::time;

/// Entre os cards da tela.
const GAP: f32 = space::BETWEEN_CARDS;

/// Largura do painel do assistente (8.5).
const ASSISTANT_WIDTH: f32 = 400.0;

/// Teto do texto dos achados: linha longa demais cansa a leitura na janela maximizada.
const FINDINGS_MAX: f32 = 720.0;

/// Largura maxima da resposta: perto de 56 caracteres de 14 px, a medida de leitura (8.5).
/// No painel de 400 ela nunca chega a valer; vale no segmentado da janela minima.
const REPLY_MAX: f32 = 440.0;

/// Rolagem da conversa: id proprio, porque `CONTENT_SCROLL` (o que a troca de tela volta ao
/// topo) e o dos achados, e so uma rolagem pode ter cada id.
pub(super) const CHAT_SCROLL: &str = "conversa";

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
    let height = (app.window_size.h - 96.0).clamp(240.0, 600.0);
    Some(widgets::dialog(
        t,
        col,
        app.window_size.w,
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
    let height = (app.window_size.h - 96.0).clamp(240.0, 640.0);
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
        app.window_size.w,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::advisor::FindingKind;
    use crate::assistant::settings::{ProviderChoice, Settings};
    use crate::dashboard::money::MASK_DIGITS;
    use crate::ui_model::analysis::tests::{conversation_with, msg};
    use crate::ui_model::effect::Effect;

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
    fn dois_paineis_contra_o_segmentado_em_728() {
        use crate::ui::app::tests::app_with_fixtures;
        use crate::ui::app::{AnalysisTab, Screen, update};
        // 1024 de janela = 728 de largura util: segmentado. 1440 = 1144: dois paineis.
        assert!(!two_panels(728.0));
        assert!(two_panels(1144.0));
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with_fixtures(dir.path());
        let _ = update(&mut app, Message::SelectScreen(Screen::Analysis));
        app.window_size.w = 1024.0;
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
        app.window_size.w = 1440.0;
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
        let effects = crate::ui::app::step(&mut app, Message::CopyReply("R1".into()));
        assert!(matches!(
            effects.as_slice(),
            [
                Effect::CopyToClipboard(_),
                Effect::After(_, Message::CopyExpired)
            ]
        ));
        assert!(app.is_copied(CopyTarget::Reply));
        assert_eq!(app.copied_reply.as_deref(), Some("R1"));
        let _ = view(&app);
        // Mensagem que nao esta na conversa aberta: nada a copiar.
        app.copied = None;
        let effects = crate::ui::app::step(&mut app, Message::CopyReply("X".into()));
        assert!(effects.is_empty());
        assert!(!app.is_copied(CopyTarget::Reply));
    }
}
