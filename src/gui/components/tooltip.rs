//! Tooltip próprio no visual do design (fundo `toast_bg`, raio 12, 12 px). O do `gpui-component`
//! tem outro visual; copiar o do design dá menos código que reestilizar aquele.

use gpui::{
    AnyView, App, AppContext, Context, IntoElement, ParentElement, Render, SharedString, Styled,
    Window, div, px,
};

use crate::gui::fonts;
use crate::gui::theme::Tokens;

/// Uma linha do tooltip: texto simples ou par rótulo/valor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Line {
    Text(SharedString),
    Pair {
        label: SharedString,
        value: SharedString,
        bold: bool,
    },
}

pub struct TooltipView {
    title: Option<SharedString>,
    lines: Vec<Line>,
    max_width: Option<f32>,
    tokens: &'static Tokens,
}

impl TooltipView {
    /// Texto corrido (o ⓘ do "Custo essencial"): largura máxima 240.
    pub fn text(text: impl Into<SharedString>, tokens: &'static Tokens) -> TooltipView {
        TooltipView {
            title: None,
            lines: vec![Line::Text(text.into())],
            max_width: Some(240.0),
            tokens,
        }
    }

    /// Título e pares (hover numa barra do histórico): largura mínima 180.
    pub fn rows(
        title: impl Into<SharedString>,
        lines: Vec<Line>,
        tokens: &'static Tokens,
    ) -> TooltipView {
        TooltipView {
            title: Some(title.into()),
            lines,
            max_width: None,
            tokens,
        }
    }

    /// Para `.tooltip(|window, cx| ...)` de um elemento.
    pub fn build(self, cx: &mut App) -> AnyView {
        cx.new(|_| self).into()
    }
}

impl Render for TooltipView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.render_static()
    }
}

impl TooltipView {
    /// O desenho sem a janela: a vitrine de debug mostra o tooltip parado, sem hover.
    pub fn render_static(&self) -> gpui::Div {
        let t = self.tokens;
        let mut card = div()
            .flex()
            .flex_col()
            .rounded(px(12.0))
            .px(px(12.0))
            .py(px(10.0))
            .bg(t.toast_bg)
            .text_color(t.toast_fg)
            .text_size(px(12.0))
            .line_height(px(18.0))
            .shadow(vec![gpui::BoxShadow {
                color: crate::gui::theme::rgba(0, 0, 0, 0.25),
                offset: gpui::point(px(0.0), px(8.0)),
                blur_radius: px(24.0),
                spread_radius: px(0.0),
                inset: false,
            }]);
        card = match self.max_width {
            Some(w) => card.max_w(px(w)),
            None => card.min_w(px(180.0)),
        };
        if let Some(title) = &self.title {
            card = card.child(
                div()
                    .mb(px(4.0))
                    .font_weight(fonts::weight(700))
                    .child(title.clone()),
            );
        }
        for line in &self.lines {
            card = card.child(match line {
                Line::Text(s) => div().child(s.clone()),
                Line::Pair { label, value, bold } => div()
                    .flex()
                    .justify_between()
                    .gap(px(16.0))
                    .when_bold(*bold)
                    .child(label.clone())
                    .child(value.clone()),
            });
        }
        card
    }
}

trait WhenBold {
    fn when_bold(self, bold: bool) -> Self;
}

impl WhenBold for gpui::Div {
    fn when_bold(self, bold: bool) -> Self {
        if bold {
            self.font_weight(fonts::weight(700))
        } else {
            self
        }
    }
}
