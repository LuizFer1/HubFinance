//! Valor em dinheiro do design: "R$" a 0,6em/500/opacidade 0,6 e o numero ao lado.
//!
//! **O componente so desenha.** Quem decide a mascara e o view-model (`dashboard::money`: `MASK`
//! "R$ •••••", `AXIS_MASK` "•••"): o texto que chega ja e a mascara e nunca contem o valor. A
//! unica decisao daqui e **separar o "R$"** para dar a ele o tamanho menor, e reconhecer a
//! mascara (so pontos) para o leitor de tela ouvir "valor oculto" em vez de "ponto ponto...".
//! Ter uma segunda mascara aqui duplicaria a do view-model e as duas poderiam divergir.

use gpui::{
    App, ElementId, Hsla, InteractiveElement, IntoElement, ParentElement, RenderOnce, Role,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px, relative,
};

use crate::gui::fonts;
use crate::gui::theme::Tokens;

/// Rotulo de acessibilidade do valor oculto (spec 6.9).
pub const HIDDEN_LABEL: &str = "valor oculto";

/// Tamanhos do design (px): total do dashboard, cards, linhas, secundario e tabela.
pub const SIZES: [f32; 5] = [48.0, 30.0, 20.0, 15.0, 14.0];

/// `line-height: 1.05` dos valores no design: sem ele cada valor usa a altura de linha padrao e
/// os cards ficam mais altos que o artboard.
pub const LINE_HEIGHT: f32 = 1.05;

/// O corpo e so pontos de mascara (e tem algum): o valor esta oculto.
pub fn is_masked(body: &str) -> bool {
    !body.is_empty() && body.chars().all(|c| c == '•')
}

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

#[derive(IntoElement)]
pub struct Money {
    text: SharedString,
    size: f32,
    id: Option<ElementId>,
    weight: u16,
    color: Option<Hsla>,
    tokens: &'static Tokens,
}

impl Money {
    pub fn new(text: impl Into<SharedString>, size: f32, tokens: &'static Tokens) -> Money {
        Money {
            text: text.into(),
            size,
            id: None,
            weight: 600,
            color: None,
            tokens,
        }
    }

    /// Id estavel do elemento (so o valor oculto o usa, para expor o rotulo). Sem ele, um id
    /// derivado do tamanho.
    pub fn id(mut self, id: impl Into<ElementId>) -> Money {
        self.id = Some(id.into());
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
        let parts = split(&self.text);
        let masked = is_masked(&parts.body);
        let color = self.color.unwrap_or(self.tokens.text);
        let size = self.size;
        let mut row = div()
            .flex()
            .flex_none()
            .items_baseline()
            .line_height(relative(LINE_HEIGHT))
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
        row = row.child(div().child(SharedString::from(parts.body)));
        if masked {
            let id = self.id.unwrap_or(ElementId::Integer(self.size as u64));
            row.id(id)
                .role(Role::Label)
                .aria_label(HIDDEN_LABEL)
                .into_any_element()
        } else {
            row.into_any_element()
        }
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
    fn mascara_do_view_model_e_reconhecida_e_nao_tem_digito() {
        use crate::dashboard::money::{AXIS_MASK, MASK};
        let p = split(MASK);
        assert!(p.currency && is_masked(&p.body), "{p:?}");
        assert!(!p.body.chars().any(|c| c.is_ascii_digit()));
        let n = split(AXIS_MASK);
        assert!(!n.currency && is_masked(&n.body), "{n:?}");
    }

    #[test]
    fn valor_visivel_nao_e_mascara() {
        assert!(!is_masked(&split("R$ 1,00").body));
        assert!(!is_masked(&split("75,6%").body));
        assert!(!is_masked(""));
    }
}
