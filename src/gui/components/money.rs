//! Valor em dinheiro do design: "R$" a 0,6em/500/opacidade 0,6 e o numero ao lado.
//!
//! O texto chega pronto do view-model (ja mascarado quando "Ocultar valores" esta ligado, ou
//! com os digitos). O componente so desenha; a unica decisao daqui e **separar o "R$"** para
//! dar a ele o tamanho menor. A mascara tem largura fixa: se o numero oculto mantivesse o
//! comprimento do real, a largura vazaria a ordem de grandeza do valor.

use gpui::{
    App, Hsla, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div, px,
};

use crate::gui::fonts;
use crate::gui::theme::Tokens;

/// Mascara de dinheiro (largura fixa) e de numero que revela valor (porcentagem, variacao).
pub const MASK_MONEY: &str = "••••";
pub const MASK_NUMBER: &str = "•••";
/// Rotulo de acessibilidade do valor oculto (o GPUI 0.3 ainda nao expoe arvore de
/// acessibilidade; fica para quando expuser e para o tooltip do olho).
pub const HIDDEN_LABEL: &str = "valor oculto";

/// Tamanhos do design (px): total do dashboard, cards, linhas, secundario e tabela.
pub const SIZES: [f32; 5] = [48.0, 30.0, 20.0, 15.0, 14.0];

/// Texto do valor separado em sinal, "R$" e corpo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parts {
    pub sign: String,
    pub currency: bool,
    pub body: String,
}

/// Divide "−R$ 96,40" em (`−`, R$, `96,40`). Sem "R$" o texto inteiro e o corpo (numero que
/// nao e dinheiro: "75,6%", "+12%").
pub fn split(text: &str) -> Parts {
    let trimmed = text.trim_start();
    let sign_len = trimmed
        .chars()
        .take_while(|c| matches!(c, '−' | '-' | '+'))
        .map(char::len_utf8)
        .sum::<usize>();
    let (sign, rest) = trimmed.split_at(sign_len);
    match rest.strip_prefix("R$") {
        Some(body) => Parts {
            sign: sign.to_string(),
            currency: true,
            body: body.trim_start_matches([' ', '\u{a0}']).to_string(),
        },
        None => Parts {
            sign: String::new(),
            currency: false,
            body: trimmed.to_string(),
        },
    }
}

/// O que a tela mostra: oculto vira mascara de largura fixa, sem digito nenhum.
pub fn display(text: &str, hidden: bool) -> Parts {
    let parts = split(text);
    if !hidden {
        return parts;
    }
    Parts {
        sign: String::new(),
        body: if parts.currency {
            MASK_MONEY
        } else {
            MASK_NUMBER
        }
        .to_string(),
        currency: parts.currency,
    }
}

#[derive(IntoElement)]
pub struct Money {
    text: SharedString,
    size: f32,
    hidden: bool,
    weight: u16,
    color: Option<Hsla>,
    tokens: &'static Tokens,
}

impl Money {
    pub fn new(text: impl Into<SharedString>, size: f32, tokens: &'static Tokens) -> Money {
        Money {
            text: text.into(),
            size,
            hidden: false,
            weight: 600,
            color: None,
            tokens,
        }
    }

    pub fn hidden(mut self, hidden: bool) -> Money {
        self.hidden = hidden;
        self
    }

    pub fn weight(mut self, weight: u16) -> Money {
        self.weight = weight;
        self
    }

    pub fn color(mut self, color: Hsla) -> Money {
        self.color = Some(color);
        self
    }
}

impl RenderOnce for Money {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let parts = display(&self.text, self.hidden);
        let color = self.color.unwrap_or(self.tokens.text);
        let size = self.size;
        let mut row = div()
            .flex()
            .flex_none()
            .items_baseline()
            .text_size(px(size))
            .font_weight(fonts::weight(self.weight))
            .text_color(color);
        if !parts.sign.is_empty() {
            row = row.child(SharedString::from(parts.sign));
        }
        if parts.currency {
            row = row.child(
                div()
                    .mr(px(size * 0.3))
                    .text_size(px(size * 0.6))
                    .font_weight(fonts::weight(500))
                    .text_color(color.opacity(0.6))
                    .child("R$"),
            );
        }
        let body = div().child(SharedString::from(parts.body));
        row.child(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separa_sinal_moeda_e_corpo() {
        let p = split("−R$\u{a0}96,40");
        assert_eq!(
            (p.sign.as_str(), p.currency, p.body.as_str()),
            ("−", true, "96,40")
        );
        let p = split("R$ 14.180,00");
        assert_eq!(
            (p.sign.as_str(), p.currency, p.body.as_str()),
            ("", true, "14.180,00")
        );
        let p = split("+12%");
        assert_eq!(
            (p.sign.as_str(), p.currency, p.body.as_str()),
            ("", false, "+12%")
        );
    }

    #[test]
    fn oculto_tem_largura_fixa_e_nao_revela_digitos() {
        let curto = display("R$ 9,00", true);
        let longo = display("R$ 1.234.567,89", true);
        assert_eq!(curto, longo);
        assert_eq!(curto.body, MASK_MONEY);
        assert!(curto.currency);
        assert!(!curto.body.chars().any(|c| c.is_ascii_digit()));
    }

    #[test]
    fn numero_que_revela_valor_vira_tres_pontos_sem_moeda() {
        let p = display("75,6%", true);
        assert_eq!(
            (p.currency, p.body.as_str(), p.sign.as_str()),
            (false, MASK_NUMBER, "")
        );
        assert_eq!(display("−R$ 5,00", true).sign, "");
    }

    #[test]
    fn visivel_nao_muda_o_texto() {
        assert_eq!(display("R$ 1,00", false), split("R$ 1,00"));
    }
}
