//! Tela Analise: os achados de `advisor` como cartoes (sempre, sem rede); o chat entra depois.
//! Tudo vem de `app.analysis` (calculado em `App::rebuild`); aqui so se monta a arvore.

use iced::widget::{Space, column, container, row, text};
use iced::{Alignment, Color, Element, Length, Padding};

use super::app::{App, Message};
use super::theme::{self, Tokens};
use super::{fonts, widgets};
use crate::advisor::{Finding, Severity};
use crate::dashboard::money::{MASK, MINUS};

const GAP: f32 = 14.0;

pub fn view(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    if app.analysis.findings.is_empty() {
        return card(
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
        );
    }
    column(app.analysis.findings.iter().map(|f| finding_card(app, f)))
        .spacing(GAP)
        .width(Length::Fill)
        .into()
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
fn mask(app: &App, s: &str) -> String {
    if app.hide_values {
        mask_str(s)
    } else {
        s.to_string()
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
    fn mascara_nao_mexe_em_texto_sem_dinheiro() {
        let s = "Gasto 35% acima do normal em 12 dias.";
        assert_eq!(mask_str(s), s);
        assert_eq!(mask_str(""), "");
    }
}
