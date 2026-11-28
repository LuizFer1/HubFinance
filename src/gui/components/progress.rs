//! Barra de progresso de 6 px em pilula.

use gpui::{App, Hsla, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px, relative};

use crate::gui::theme::Tokens;

/// Fracao 0..=1 do preenchimento. Total zero ou negativo (reserva sem meta) e barra vazia, nunca
/// NaN: `relative(NaN)` desenharia largura indefinida.
pub fn fraction(value: f64, total: f64) -> f32 {
    if total <= 0.0 || !value.is_finite() || !total.is_finite() {
        return 0.0;
    }
    (value / total).clamp(0.0, 1.0) as f32
}

#[derive(IntoElement)]
pub struct Progress {
    fraction: f32,
    fill: Option<Hsla>,
    track: Option<Hsla>,
    tokens: &'static Tokens,
}

impl Progress {
    pub fn new(value: f64, total: f64, tokens: &'static Tokens) -> Progress {
        Progress {
            fraction: fraction(value, total),
            fill: None,
            track: None,
            tokens,
        }
    }

    pub fn fill(mut self, color: Hsla) -> Progress {
        self.fill = Some(color);
        self
    }

    /// Trilho sobre um card que nao e `surface` (o do Negocio).
    pub fn track(mut self, color: Hsla) -> Progress {
        self.track = Some(color);
        self
    }
}

impl RenderOnce for Progress {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let t = self.tokens;
        div()
            .h(px(6.0))
            .w_full()
            .rounded_full()
            .bg(self.track.unwrap_or(t.divider))
            .overflow_hidden()
            .child(
                div()
                    .h_full()
                    .w(relative(self.fraction))
                    .rounded_full()
                    .bg(self.fill.unwrap_or(t.accent)),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fracao_limita_e_nunca_e_nan() {
        assert_eq!(fraction(50.0, 100.0), 0.5);
        assert_eq!(fraction(150.0, 100.0), 1.0);
        assert_eq!(fraction(-5.0, 100.0), 0.0);
        assert_eq!(fraction(5.0, 0.0), 0.0);
        assert_eq!(fraction(f64::NAN, 10.0), 0.0);
        assert_eq!(fraction(1.0, f64::INFINITY), 0.0);
    }
}
