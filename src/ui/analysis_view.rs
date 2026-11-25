//! Tela Analise: os achados de `advisor` como cartoes (sempre, sem rede); o chat entra depois.
//! Tudo vem de `app.analysis` (calculado em `App::rebuild`); aqui so se monta a arvore.

use std::borrow::Cow;

use iced::widget::{
    Space, button, column, container, mouse_area, opaque, row, scrollable, stack, text, text_input,
};
use iced::{Alignment, Color, Element, Length, Padding};

use super::app::{App, Message};
use super::assistant_settings_view;
use super::theme::{self, Tokens};
use super::{fonts, widgets};
use crate::advisor::{Finding, Severity};
use crate::assistant::view::{ConversationSummary, MessageRole, MessageView, OpenConversation};
use crate::dashboard::money::{MASK, MINUS};
use crate::hub::snapshot::Status;

const GAP: f32 = 14.0;

/// Texto da sobreposicao; constante para um teste conferir que a quebra de linha no fonte nao
/// deixou espacos soltos no meio da frase.
const INTRO: &str = "Este é o texto exato que o assistente recebe: resumos e totais, sem \
lançamentos, descrições, nomes de pessoas, aparelhos ou formas de pagamento, nem datas \
exatas.";

pub fn view(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let toolbar = row![
        button(
            text("Conversar sobre isso")
                .size(13)
                .font(fonts::INTER_MEDIUM)
        )
        .padding([6, 12])
        .style(theme::outline_accent(t))
        .on_press(Message::StartConversation),
        Space::new().width(Length::Fill),
        button(
            text("Configurar assistente")
                .size(13)
                .font(fonts::INTER_MEDIUM)
        )
        .padding([6, 12])
        .style(theme::outline_divider(t))
        .on_press(Message::ToggleAssistantSettings),
        button(
            text("Ver o que o assistente vê")
                .size(13)
                .font(fonts::INTER_MEDIUM)
        )
        .padding([6, 12])
        .style(theme::outline_divider(t))
        .on_press(Message::ShowEnvelope),
    ]
    .spacing(8);
    let body: Element<'_, Message> = if app.analysis.findings.is_empty() {
        card(
            t,
            column![
                widgets::kicker(t, "Nada a apontar"),
                text(
                    "Ainda não há dados suficientes nesta conta, ou está tudo dentro do seu normal."
                )
                .size(14)
                .color(t.text_muted),
            ]
            .spacing(6),
        )
    } else {
        column(app.analysis.findings.iter().map(|f| finding_card(app, f)))
            .spacing(GAP)
            .width(Length::Fill)
            .into()
    };
    let mut page = column![toolbar].spacing(GAP).width(Length::Fill);
    if app.assistant_settings_open {
        page = page.push(assistant_settings_view::view(app));
    }
    page = page.push(body);
    if matches!(app.snapshot.status, Status::Failed(_)) {
        // Sem o nucleo nao ha assistente; os achados (do ultimo snapshot) seguem valendo.
        page = page.push(card(t, text(HUB_DOWN).size(14).color(t.text_muted)));
    } else {
        if let Some(chat) = chat_area(app) {
            page = page.push(chat);
        }
        if let Some(list) = conversation_list(app) {
            page = page.push(list);
        }
    }
    page.into()
}

/// No lugar do chat quando o nucleo nao subiu.
const HUB_DOWN: &str = "O hub não iniciou; a análise acima continua valendo.";

/// A conversa aberta, so se for da conta que a tela mostra: com a outra conta aberta ela falaria
/// de numeros que nao estao na tela.
pub(super) fn open_conversation(app: &App) -> Option<&OpenConversation> {
    app.snapshot
        .assistant
        .open
        .as_ref()
        .filter(|c| c.book == app.book)
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
        .filter(|c| c.book == app.book)
        .collect()
}

fn tokens_label((tokens_in, tokens_out): (u32, u32)) -> String {
    format!("{tokens_in} → {tokens_out} tokens")
}

fn chat_area(app: &App) -> Option<Element<'_, Message>> {
    let t = app.tokens;
    let view = &app.snapshot.assistant;
    let open = open_conversation(app)?;
    let header = row![
        text("Conversa")
            .size(15)
            .font(fonts::INTER_MEDIUM)
            .color(t.text),
        Space::new().width(Length::Fill),
        text(tokens_label(open.tokens_total()))
            .size(12)
            .color(t.text_muted),
        button(text("Fechar").size(13).font(fonts::INTER_MEDIUM))
            .padding([4, 10])
            .style(theme::outline_divider(t))
            .on_press(Message::CloseConversation),
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    let mut col = column![header].spacing(12).width(Length::Fill);
    if data_changed(app) {
        col = col.push(
            container(
                row![
                    text("Os dados mudaram desde o início desta conversa")
                        .size(13)
                        .color(t.warning),
                    Space::new().width(Length::Fill),
                    button(text("Nova conversa").size(13).font(fonts::INTER_MEDIUM))
                        .padding([4, 10])
                        .style(theme::outline_accent(t))
                        .on_press(Message::StartConversation),
                ]
                .spacing(12)
                .align_y(Alignment::Center),
            )
            .padding([8, 12])
            .width(Length::Fill)
            .style(theme::outlined(t.warning, 8.0)),
        );
    }
    let last = open.messages.len().saturating_sub(1);
    let mut messages = column![].spacing(10).width(Length::Fill);
    for (i, m) in open.messages.iter().enumerate() {
        messages = messages.push(message_bubble(app, m, i == last && !view.busy));
    }
    if let (true, Some(partial)) = (view.busy, view.streaming.as_deref()) {
        messages = messages.push(streaming_bubble(app, partial));
    }
    // Altura fixa: a pagina ja rola, e uma rolagem dentro de outra so se resolve com tamanho
    // definido. Ancorada embaixo, a conversa acompanha o texto que chega.
    let height = (app.window_size.height - 340.0).clamp(220.0, 520.0);
    col = col.push(
        scrollable(container(messages).padding([0, 12]))
            .direction(scrollable::Direction::Vertical(
                scrollable::Scrollbar::new()
                    .width(6)
                    .scroller_width(6)
                    .margin(3)
                    .anchor(scrollable::Anchor::End),
            ))
            .style(theme::scrollbar(t))
            .width(Length::Fill)
            .height(height),
    );
    // Sem `on_input` o campo fica desabilitado enquanto ha resposta chegando.
    let mut input = text_input("Pergunte algo sobre estes números", &app.chat_input)
        .size(14)
        .padding([10, 12])
        .style(theme::input(t))
        .width(Length::Fill);
    if !view.busy {
        input = input
            .on_input(Message::ChatInput)
            .on_submit(Message::ChatSubmit);
    }
    col = col.push(input);
    Some(card(t, col))
}

fn message_bubble<'a>(app: &'a App, m: &'a MessageView, is_last: bool) -> Element<'a, Message> {
    let t = app.tokens;
    let content = mask(app, &m.content);
    match m.role {
        MessageRole::User => {
            let bubble = container(text(content).size(14).color(t.text))
                .padding([8, 12])
                .max_width(520)
                .style(theme::fill(t.surface_2, 10.0));
            // Pergunta sem resposta e sem nada em andamento (a abertura nao saiu, ou o envio
            // morreu antes do erro virar mensagem): sem este botao a conversa ficaria parada.
            let mut side = column![bubble].spacing(6).align_x(Alignment::End);
            if is_last {
                side = side.push(retry_button(t));
            }
            row![Space::new().width(Length::Fill), side].into()
        }
        MessageRole::Assistant => {
            let mut notes = vec![match (m.tokens_in, m.tokens_out) {
                (Some(i), Some(o)) => tokens_label((i, o)),
                _ => "—".to_string(),
            }];
            if m.interrupted {
                notes.push("interrompida".to_string());
            }
            if m.sent_to_cloud {
                notes.push("enviado à nuvem".to_string());
            }
            row![
                column![
                    container(text(content).size(14).color(t.text))
                        .padding([8, 12])
                        .style(theme::outlined(t.divider(), 10.0)),
                    text(notes.join(" · ")).size(11).color(t.text_muted),
                ]
                .spacing(4)
                .max_width(560),
                Space::new().width(Length::Fill),
            ]
            .into()
        }
        MessageRole::Error => {
            let mut body = column![text(content).size(14).color(t.expense_fg)].spacing(8);
            if is_last {
                body = body.push(retry_button(t));
            }
            row![
                container(body)
                    .padding([8, 12])
                    .max_width(560)
                    .style(theme::outlined(theme::alpha(t.expense, 0.55), 10.0)),
                Space::new().width(Length::Fill),
            ]
            .into()
        }
    }
}

fn retry_button(t: &'static Tokens) -> Element<'static, Message> {
    button(text("Tentar de novo").size(13).font(fonts::INTER_MEDIUM))
        .padding([4, 10])
        .style(theme::danger_outline(t))
        .on_press(Message::ChatRetry)
        .into()
}

fn streaming_bubble<'a>(app: &'a App, partial: &'a str) -> Element<'a, Message> {
    let t = app.tokens;
    let shown = if partial.is_empty() {
        Cow::Borrowed("…")
    } else {
        mask(app, partial)
    };
    row![
        container(
            column![
                text(shown).size(14).color(t.text),
                button(text("Parar").size(13).font(fonts::INTER_MEDIUM))
                    .padding([4, 10])
                    .style(theme::outline_divider(t))
                    .on_press(Message::ChatStop),
            ]
            .spacing(8)
        )
        .padding([8, 12])
        .max_width(560)
        .style(theme::outlined(t.divider(), 10.0)),
        Space::new().width(Length::Fill),
    ]
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

/// Conversas da conta que a tela mostra, com abrir e apagar em cada uma.
fn conversation_list(app: &App) -> Option<Element<'_, Message>> {
    let t = app.tokens;
    let items = conversations_of_book(app);
    if items.is_empty() {
        return None;
    }
    let armed = app.confirm_delete_all.is_some();
    let delete_all = button(
        text(delete_all_label(armed))
            .size(13)
            .font(fonts::INTER_MEDIUM),
    )
    .padding([4, 10])
    .style(move |theme: &iced::Theme, status| {
        if armed {
            theme::danger_filled(t)(theme, status)
        } else {
            theme::danger_outline(t)(theme, status)
        }
    })
    .on_press(Message::DeleteAllConversations);
    let open_id = open_conversation(app).map(|c| c.id.as_str());
    let mut col = column![
        row![
            widgets::kicker(t, "Conversas"),
            Space::new().width(Length::Fill),
            delete_all,
        ]
        .align_y(Alignment::Center)
    ]
    .spacing(8)
    .width(Length::Fill);
    for c in items {
        let is_open = open_id == Some(c.id.as_str());
        let title = button(
            text(c.title.as_str())
                .size(14)
                .font(if is_open {
                    fonts::INTER_SEMIBOLD
                } else {
                    fonts::INTER
                })
                .color(if is_open { t.accent_300 } else { t.text }),
        )
        .padding([6, 8])
        .width(Length::Fill)
        .style(theme::plain(t.text))
        .on_press(Message::OpenConversation(c.id.clone()));
        let delete = button(text("Apagar").size(12).font(fonts::INTER_MEDIUM))
            .padding([4, 8])
            .style(theme::danger_outline(t))
            .on_press(Message::DeleteConversation(c.id.clone()));
        col = col.push(row![title, delete].spacing(8).align_y(Alignment::Center));
    }
    Some(card(t, col))
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
    let content = container(scrollable(
        container(text(preview).size(12).font(fonts::INTER).color(t.text))
            .padding(12)
            .width(Length::Fill),
    ))
    .height(Length::Fill)
    .style(theme::card(t));
    let close = button(text("Fechar").size(13).font(fonts::INTER_MEDIUM))
        .height(38)
        .padding([0, 16])
        .style(theme::outline_divider(t))
        .on_press(Message::CloseEnvelope);
    let width = (app.window_size.width - 48.0).clamp(280.0, 720.0);
    let height = (app.window_size.height - 96.0).clamp(240.0, 640.0);
    let dialog_box = container(
        column![
            header,
            intro,
            content,
            row![Space::new().width(Length::Fill), close]
        ]
        .spacing(12)
        .height(Length::Fill),
    )
    .width(width)
    .height(height)
    .padding(24)
    .style(theme::dialog(t));
    let dialog_box = opaque(mouse_area(dialog_box).on_press(Message::Noop));
    let scrim = opaque(
        mouse_area(
            container(Space::new())
                .width(Length::Fill)
                .height(Length::Fill)
                .style(theme::scrim()),
        )
        .on_press(Message::CloseEnvelope),
    );
    Some(
        stack![scrim, container(dialog_box).center(Length::Fill)]
            .width(Length::Fill)
            .height(Length::Fill)
            .into(),
    )
}

fn severity_color(t: &'static Tokens, s: Severity) -> Color {
    match s {
        Severity::Alert => t.expense,
        Severity::Attention => t.warning,
        Severity::GoodSign => t.income,
    }
}

fn finding_card<'a>(app: &'a App, f: &'a Finding) -> Element<'a, Message> {
    let t = app.tokens;
    let tag = row![
        widgets::dot(severity_color(t, f.severity), 8.0),
        text(f.severity.label()).size(12).color(t.text_muted),
    ]
    .spacing(6)
    .align_y(Alignment::Center);
    let mut body = column![
        tag,
        text(mask(app, &f.title))
            .size(16)
            .font(fonts::INTER_SEMIBOLD)
            .color(t.text),
        text(mask(app, &f.detail)).size(14).color(t.text_muted),
    ]
    .spacing(6);
    if let Some(action) = &f.action {
        body = body
            .push(Space::new().height(4))
            .push(text(mask(app, action)).size(14).color(t.accent));
    }
    card(t, body)
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

/// Troca cada "R$ ..." por `MASK`. O "−" colado ao valor sai junto (oculto nao mostra o sinal,
/// como o `signed_brl` do dashboard); o "~" de estimativa e a pontuacao em volta ficam. O fim
/// do numero e decidido por digitos: um ponto ou virgula so pertence ao valor quando vem um
/// digito depois, senao "R$ 960,00." comeria o ponto final e "R$ 5 de R$ 9" comeria o " de".
/// "R$" sem digito depois (parte do nome de uma categoria, por ex.) nao e valor: fica literal.
fn mask_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find("R$") {
        let before = &rest[..i];
        let after_sign = &rest[i + 2..];
        let number = after_sign.trim_start_matches([' ', '\u{a0}']);
        let chars: Vec<(usize, char)> = number.char_indices().collect();
        let mut end = 0;
        for (n, &(pos, c)) in chars.iter().enumerate() {
            let next_is_digit = chars.get(n + 1).is_some_and(|&(_, d)| d.is_ascii_digit());
            if c.is_ascii_digit() || (matches!(c, '.' | ',') && next_is_digit) {
                end = pos + c.len_utf8();
            } else {
                break;
            }
        }
        if end == 0 {
            out.push_str(before);
            out.push_str("R$");
        } else {
            out.push_str(before.strip_suffix(MINUS).unwrap_or(before));
            out.push_str(MASK);
        }
        rest = if end == 0 { after_sign } else { &number[end..] };
    }
    out.push_str(rest);
    out
}

fn card<'a>(t: &'static Tokens, content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content)
        .padding(Padding::from([18, 20]))
        .width(Length::Fill)
        .style(theme::card(t))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn mascara_nao_deixa_valor_nenhum_nos_achados_do_exemplo() {
        use crate::dashboard::{GRAFITE_TODAY, fixtures_grafite};
        let analysis = crate::advisor::analyze(&fixtures_grafite(), GRAFITE_TODAY);
        let texts: Vec<&str> = analysis
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
            texts.iter().any(|t| t.contains("R$ ")),
            "o teste so vale se algum texto trouxer dinheiro"
        );
        for t in texts {
            let masked = mask_str(t);
            let leaked = masked
                .match_indices("R$ ")
                .any(|(i, _)| masked[i + 3..].starts_with(|c: char| c.is_ascii_digit()));
            assert!(!leaked, "{t:?} -> {masked:?}");
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
    fn texto_da_previa_nao_tem_espaco_duplo() {
        assert!(!INTRO.contains("  "), "{INTRO:?}");
        assert!(INTRO.contains("sem lançamentos, descrições"));
    }

    #[test]
    fn mascara_nao_mexe_em_texto_sem_dinheiro() {
        let s = "Gasto 35% acima do normal em 12 dias.";
        assert_eq!(mask_str(s), s);
        assert_eq!(mask_str(""), "");
    }
}
