//! Segmentado: pilula com opcoes lado a lado (tema Escuro | Claro, filtros).
//!
//! Padding 4, gap 4, itens de 36 px; o ativo e `accent_900`/`accent_200` em 600, o inativo e
//! `muted` e no hover ganha `surface`. O fundo da pilula e `surface_2` (artboard Componentes)
//! ou `surface` com `card_ring` quando o controle fica solto na tela (spec §4.3).

use gpui::{
    App, ClickEvent, ElementId, FocusHandle, Hsla, IntoElement, RenderOnce, SharedString, Window,
    div, prelude::*, px,
};

use super::focus;
use crate::gui::fonts;
use crate::gui::theme::Tokens;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    /// `surface_2`: dentro de card ou barra lateral.
    Inset,
    /// `surface` + `card_ring`: solto sobre o fundo.
    Raised,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ItemColors {
    pub bg: Option<Hsla>,
    pub fg: Hsla,
    pub bold: bool,
}

pub fn item_colors(active: bool, hover: bool, t: &Tokens) -> ItemColors {
    if active {
        ItemColors {
            bg: Some(t.accent_900),
            fg: t.accent_200,
            bold: true,
        }
    } else if hover {
        ItemColors {
            bg: Some(t.surface),
            fg: t.text,
            bold: false,
        }
    } else {
        ItemColors {
            bg: None,
            fg: t.muted,
            bold: false,
        }
    }
}

type Select = std::rc::Rc<dyn Fn(usize, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct Segmented {
    id: ElementId,
    items: Vec<SharedString>,
    selected: usize,
    surface: Surface,
    tokens: &'static Tokens,
    width: Option<f32>,
    focus: Vec<FocusHandle>,
    on_select: Option<Select>,
}

impl Segmented {
    pub fn new(
        id: impl Into<ElementId>,
        items: Vec<SharedString>,
        selected: usize,
        tokens: &'static Tokens,
    ) -> Segmented {
        Segmented {
            id: id.into(),
            items,
            selected,
            surface: Surface::Inset,
            tokens,
            width: None,
            focus: Vec::new(),
            on_select: None,
        }
    }

    pub fn surface(mut self, surface: Surface) -> Segmented {
        self.surface = surface;
        self
    }

    /// Largura fixa; sem ela o controle ocupa o conteudo.
    pub fn width(mut self, w: f32) -> Segmented {
        self.width = Some(w);
        self
    }

    /// Um handle por item, na ordem; sem eles so o mouse seleciona.
    pub fn focus_handles(mut self, handles: Vec<FocusHandle>) -> Segmented {
        self.focus = handles;
        self
    }

    pub fn on_select(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Segmented {
        self.on_select = Some(std::rc::Rc::new(f));
        self
    }
}

impl RenderOnce for Segmented {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let t = self.tokens;
        let gap_color = match self.surface {
            Surface::Inset => t.surface_2,
            Surface::Raised => t.surface,
        };
        let container_bg = gap_color;
        let ring_shadows = match (self.surface, &t.card_ring) {
            (Surface::Raised, Some(r)) => Some(r.shadows.to_vec()),
            _ => None,
        };
        let mut row = div()
            .id(self.id.clone())
            .flex()
            .gap(px(4.0))
            .p(px(4.0))
            .rounded_full()
            .bg(container_bg)
            .when_some(ring_shadows, |d, s| d.shadow(s))
            .when_some(self.width, |d, w| d.w(px(w)));
        for (i, label) in self.items.into_iter().enumerate() {
            let active = i == self.selected;
            let rest = item_colors(active, false, t);
            let hover = item_colors(active, true, t);
            let select = self.on_select.clone();
            let handle = self.focus.get(i).cloned();
            let ring = focus::style(t, gap_color);
            row = row.child(
                div()
                    .id(ElementId::NamedInteger("seg".into(), i as u64))
                    .flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .h(px(36.0))
                    .px(px(14.0))
                    .rounded_full()
                    .text_size(px(13.0))
                    .text_color(rest.fg)
                    .font_weight(fonts::weight(if rest.bold { 600 } else { 400 }))
                    .when_some(rest.bg, |d, c| d.bg(c))
                    .when_some(handle, |d, h| d.track_focus(&h))
                    .cursor_pointer()
                    .hover(move |s| match hover.bg {
                        Some(c) => s.bg(c).text_color(hover.fg),
                        None => s,
                    })
                    .focus_visible(ring)
                    .when_some(select, |d, f| {
                        d.on_click(move |_: &ClickEvent, window, cx| f(i, window, cx))
                    })
                    .child(label),
            );
        }
        row
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::theme::{DARK, LIGHT};

    #[test]
    fn ativo_inativo_e_hover_nos_dois_temas() {
        for t in [&*DARK, &*LIGHT] {
            let a = item_colors(true, false, t);
            assert_eq!(
                (a.bg, a.fg, a.bold),
                (Some(t.accent_900), t.accent_200, true)
            );
            // Ativo nao muda com o mouse em cima.
            assert_eq!(item_colors(true, true, t), a);
            let r = item_colors(false, false, t);
            assert_eq!((r.bg, r.fg, r.bold), (None, t.muted, false));
            let h = item_colors(false, true, t);
            assert_eq!((h.bg, h.fg), (Some(t.surface), t.text));
        }
    }
}
