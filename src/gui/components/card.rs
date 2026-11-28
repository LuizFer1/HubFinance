//! Card do design: raio 24, padding 24, `surface` com o anel do tema claro (spec §4.3).
//! `CardHighlight` e o do Negocio: fundo `highlight`, texto `on_highlight`.

use gpui::{AnyElement, App, Div, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px};

use crate::gui::theme::Tokens;

#[derive(IntoElement)]
pub struct Card {
    tokens: &'static Tokens,
    highlight: bool,
    padding: f32,
    width: Option<f32>,
    children: Vec<AnyElement>,
}

impl Card {
    pub fn new(tokens: &'static Tokens) -> Card {
        Card {
            tokens,
            highlight: false,
            padding: 24.0,
            width: None,
            children: Vec::new(),
        }
    }

    /// Card do Negocio (PJ): destaque dourado, sem anel (o fundo ja separa do `bg`).
    pub fn highlight(tokens: &'static Tokens) -> Card {
        Card {
            highlight: true,
            ..Card::new(tokens)
        }
    }

    /// Largura fixa em px (sem ela o card ocupa o que o pai der). `Refineable` nao e exportado
    /// pelo gpui, entao o `Styled` generico nao cabe aqui.
    pub fn w(mut self, px: f32) -> Card {
        self.width = Some(px);
        self
    }

    pub fn padding(mut self, px: f32) -> Card {
        self.padding = px;
        self
    }
}

impl ParentElement for Card {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

/// Cor de fundo e de texto do card. Funcao pura para o teste ler sem janela.
pub fn colors(t: &Tokens, highlight: bool) -> (gpui::Hsla, gpui::Hsla) {
    if highlight {
        (t.highlight, t.on_highlight)
    } else {
        (t.surface, t.text)
    }
}

impl RenderOnce for Card {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let t = self.tokens;
        let (bg, fg) = colors(t, self.highlight);
        let base: Div = div()
            .flex()
            .flex_col()
            .rounded(px(24.0))
            .p(px(self.padding))
            .bg(bg)
            .text_color(fg);
        // O anel e uma sombra de 1 px fora da caixa: com borda o miolo ficaria menor que no escuro.
        let base = match (&t.card_ring, self.highlight) {
            (Some(r), false) => base.shadow(r.shadows.to_vec()),
            _ => base,
        };
        let base = match self.width {
            Some(w) => base.w(px(w)),
            None => base,
        };
        base.children(self.children)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::theme::{DARK, LIGHT};

    #[test]
    fn card_comum_usa_surface_e_o_do_negocio_usa_highlight() {
        for t in [&*DARK, &*LIGHT] {
            assert_eq!(colors(t, false), (t.surface, t.text));
            assert_eq!(colors(t, true), (t.highlight, t.on_highlight));
        }
    }
}
