//! Seletor de mês de 44 px: "‹ Novembro 2026 ›". O "›" fica desabilitado no mês corrente (não
//! há mês futuro com dados) e o chip "Parcial · até 27 nov" avisa que o mês ainda corre.

use gpui::{
    App, ElementId, FocusHandle, IntoElement, ParentElement, RenderOnce, SharedString, Styled,
    Window, div, px,
};

use super::button::IconButton;
use crate::gui::fonts;
use crate::gui::icons::{Icon, icon};
use crate::gui::theme::Tokens;

type Handler = Box<dyn Fn(&mut Window, &mut App) + 'static>;

/// Opacidade da seta desabilitada (design: 0,35).
pub const DISABLED_OPACITY: f32 = 0.35;
/// Largura minima do rotulo (design: `min-width: 118px`): sem ela o seletor muda de largura de
/// "Maio 2026" para "Setembro 2026" e empurra o que estiver ao lado.
pub const LABEL_MIN_WIDTH: f32 = 118.0;
/// Largura minima do rotulo no cabecalho (design: `min-width: 124px`).
pub const HEADER_LABEL_MIN_WIDTH: f32 = 124.0;

/// Superficie do seletor: o Componentes usa `surface_2` (118 px, hover `divider`); os artboards
/// com cabecalho usam `surface` (124 px, hover `surface_2`), igual ao botao do olho ao lado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerSurface {
    Inset,
    Header,
}

/// O mês seguinte só existe se o mostrado for anterior ao corrente (`YYYY-MM` compara por
/// string porque tem largura fixa).
pub fn can_go_next(shown: &str, current: &str) -> bool {
    shown < current
}

#[derive(IntoElement)]
pub struct MonthPicker {
    id: SharedString,
    label: SharedString,
    can_prev: bool,
    can_next: bool,
    on_prev: Option<Handler>,
    on_next: Option<Handler>,
    focus_prev: Option<FocusHandle>,
    focus_next: Option<FocusHandle>,
    surface: PickerSurface,
    tokens: &'static Tokens,
}

impl MonthPicker {
    /// `id` e o prefixo dos ids das setas (`<id>-prev`, `<id>-next`): dois seletores na mesma
    /// tela (ou na vitrine) nao podem repetir id.
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        can_next: bool,
        tokens: &'static Tokens,
    ) -> Self {
        MonthPicker {
            id: id.into(),
            label: label.into(),
            can_prev: true,
            can_next,
            on_prev: None,
            on_next: None,
            focus_prev: None,
            focus_next: None,
            surface: PickerSurface::Inset,
            tokens,
        }
    }

    pub fn surface(mut self, surface: PickerSurface) -> Self {
        self.surface = surface;
        self
    }

    /// Sem mes anterior (o mais antigo com dados): a seta fica apagada e fora do Tab.
    pub fn can_prev(mut self, can_prev: bool) -> Self {
        self.can_prev = can_prev;
        self
    }

    pub fn on_prev(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_prev = Some(Box::new(f));
        self
    }

    pub fn on_next(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_next = Some(Box::new(f));
        self
    }

    /// Handles de foco das setas, guardados por quem desenha a tela (o Tab as percorre).
    pub fn focus(mut self, prev: &FocusHandle, next: &FocusHandle) -> Self {
        self.focus_prev = Some(prev.clone());
        self.focus_next = Some(next.clone());
        self
    }
}

impl RenderOnce for MonthPicker {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let t = self.tokens;
        let (bg, hover, min_w) = match self.surface {
            PickerSurface::Inset => (t.surface_2, t.divider, LABEL_MIN_WIDTH),
            PickerSurface::Header => (t.surface, t.surface_2, HEADER_LABEL_MIN_WIDTH),
        };
        // As setas sao o `IconButton` do design (foco, anel, cursor), com o hover `divider` e o
        // anel afastado na cor do fundo do proprio seletor (`surface_2`).
        let arrow = |suffix: &str,
                     glyph: Icon,
                     label: &'static str,
                     enabled: bool,
                     on: Option<Handler>,
                     focus: Option<FocusHandle>| {
            let id = ElementId::Name(SharedString::from(format!("{}-{suffix}", self.id)));
            let mut b = IconButton::new(id, glyph, t)
                .ring_gap(bg)
                .hover_bg(hover)
                .glyph_size(16.0)
                .aria_label(label)
                .disabled_opacity(DISABLED_OPACITY)
                .disabled(!enabled);
            if let Some(f) = focus {
                b = b.focus(&f);
            }
            if let Some(f) = on {
                b = b.on_click(move |w, cx| f(w, cx));
            }
            b
        };
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(4.0))
            .h(px(44.0))
            .px(px(4.0))
            .rounded_full()
            .bg(bg)
            .text_size(px(14.0))
            .text_color(t.text)
            .font_weight(fonts::weight(600))
            .child(arrow(
                "prev",
                Icon::ChevronLeft,
                "Mês anterior",
                self.can_prev,
                self.on_prev,
                self.focus_prev,
            ))
            .child(div().min_w(px(min_w)).text_center().child(self.label))
            .child(arrow(
                "next",
                Icon::ChevronRight,
                "Próximo mês",
                self.can_next,
                self.on_next,
                self.focus_next,
            ))
    }
}

/// Chip "Parcial · até 27 nov": meio círculo cheio em `warning` + texto `muted`.
#[derive(IntoElement)]
pub struct PartialChip {
    label: SharedString,
    tokens: &'static Tokens,
}

impl PartialChip {
    pub fn new(label: impl Into<SharedString>, tokens: &'static Tokens) -> PartialChip {
        PartialChip {
            label: label.into(),
            tokens,
        }
    }
}

impl RenderOnce for PartialChip {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let t = self.tokens;
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(8.0))
            .h(px(32.0))
            .px(px(12.0))
            .rounded_full()
            .border_1()
            .border_color(t.divider)
            .text_size(px(12.0))
            .font_weight(fonts::weight(500))
            .text_color(t.muted)
            .child(icon(Icon::HalfCircleWarning, px(14.0), t.warning))
            .child(self.label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proximo_so_antes_do_mes_corrente() {
        assert!(can_go_next("2026-10", "2026-11"));
        assert!(!can_go_next("2026-11", "2026-11"));
        assert!(can_go_next("2025-12", "2026-01"));
    }
}
