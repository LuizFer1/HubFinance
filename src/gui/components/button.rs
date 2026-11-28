//! Botoes do design: primario, contorno, fantasma e destrutivo (+ botao de icone).
//!
//! As cores de cada estado saem de `button_colors`, pura e testada nos dois temas; o render so
//! as aplica com `hover`/`active`. Hover do primario = `highlight` 14 % com `text`, pressionado
//! = 22 % com preto (spec §4.1); no destrutivo cheio o design usa 14 % com `text` e 26 % com
//! preto. O "afundar 1 px" do pressionado no design (`translateY(1px)`) nao e reproduzido: o
//! GPUI nao tem `transform` em `div`, e a cor ja marca o estado.

use gpui::{
    App, ClickEvent, ElementId, FocusHandle, Hsla, IntoElement, RenderOnce, SharedString, Window,
    black, div, prelude::*, px,
};

use super::{focus, mix};
use crate::gui::fonts;
use crate::gui::icons::{Icon, icon};
use crate::gui::theme::Tokens;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    /// `highlight`, o botao da acao principal da tela.
    Primary,
    /// Borda `divider`, fundo transparente.
    Outline,
    /// Sem borda nem fundo; so o hover pinta.
    Ghost,
    /// Texto e borda `danger` (acao destrutiva comum).
    Danger,
    /// Cheio `danger`/`on_danger`: so o destrutivo do passo 3 do Remover.
    DangerFilled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Rest,
    Hover,
    Pressed,
    Disabled,
}

/// Altura do botao (alvo minimo do design: 44 nos principais, 40 com mouse, 36 nas linhas de
/// Ajustes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Size {
    Large,
    Medium,
    Small,
}

impl Size {
    pub fn height(self) -> f32 {
        match self {
            Size::Large => 44.0,
            Size::Medium => 40.0,
            Size::Small => 36.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Colors {
    /// `None` = transparente.
    pub bg: Option<Hsla>,
    pub fg: Hsla,
    pub border: Option<Hsla>,
}

pub fn button_colors(variant: Variant, state: State, t: &Tokens) -> Colors {
    let hover = state == State::Hover;
    let pressed = state == State::Pressed;
    // Desabilitado se parece com o repouso (a opacidade 0,4 vem do render).
    match variant {
        Variant::Primary => Colors {
            bg: Some(if pressed {
                mix(t.highlight, black(), 0.22)
            } else if hover {
                mix(t.highlight, t.text, 0.14)
            } else {
                t.highlight
            }),
            fg: t.on_highlight,
            border: None,
        },
        Variant::DangerFilled => Colors {
            bg: Some(if pressed {
                mix(t.danger, black(), 0.26)
            } else if hover {
                mix(t.danger, t.text, 0.14)
            } else {
                t.danger
            }),
            fg: t.on_danger,
            border: None,
        },
        Variant::Outline | Variant::Ghost => Colors {
            bg: if pressed {
                Some(t.divider)
            } else if hover {
                Some(t.surface_2)
            } else {
                None
            },
            fg: t.text,
            border: (variant == Variant::Outline).then_some(t.divider),
        },
        Variant::Danger => Colors {
            bg: if pressed {
                Some(t.danger.opacity(0.22))
            } else if hover {
                Some(t.danger.opacity(0.12))
            } else {
                None
            },
            fg: t.danger,
            border: Some(t.danger),
        },
    }
}

fn filled(v: Variant) -> bool {
    matches!(v, Variant::Primary | Variant::DangerFilled)
}

type Handler = Box<dyn Fn(&mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: SharedString,
    variant: Variant,
    size: Size,
    icon: Option<Icon>,
    disabled: bool,
    tokens: &'static Tokens,
    ring_gap: Option<Hsla>,
    focus: Option<FocusHandle>,
    forced: Option<State>,
    on_click: Option<Handler>,
}

impl Button {
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        variant: Variant,
        tokens: &'static Tokens,
    ) -> Button {
        Button {
            id: id.into(),
            label: label.into(),
            variant,
            // Primario e destrutivo cheio sao a acao principal: 44.
            size: if filled(variant) {
                Size::Large
            } else {
                Size::Medium
            },
            icon: None,
            disabled: false,
            tokens,
            ring_gap: None,
            focus: None,
            forced: None,
            on_click: None,
        }
    }

    /// Vitrine: desenha o botao parado num estado (hover/pressionado dependem do mouse e nao
    /// da para forca-los ao vivo). Telas nao usam.
    pub fn forced_state(mut self, state: State) -> Button {
        self.forced = Some(state);
        self
    }

    pub fn size(mut self, size: Size) -> Button {
        self.size = size;
        self
    }

    pub fn icon(mut self, icon: Icon) -> Button {
        self.icon = Some(icon);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Button {
        self.disabled = disabled;
        self
    }

    /// Cor do fundo atras do botao, para o afastamento do anel de foco (padrao: `bg`).
    pub fn ring_gap(mut self, gap: Hsla) -> Button {
        self.ring_gap = Some(gap);
        self
    }

    /// Sem `FocusHandle` o botao so responde ao mouse: o Tab precisa de um handle com
    /// `tab_stop(true)` criado por quem desenha.
    pub fn focus(mut self, handle: &FocusHandle) -> Button {
        self.focus = Some(handle.clone());
        self
    }

    pub fn on_click(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Button {
        self.on_click = Some(Box::new(f));
        self
    }
}

impl RenderOnce for Button {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let t = self.tokens;
        let rest = button_colors(self.variant, self.forced.unwrap_or(State::Rest), t);
        let hover = button_colors(self.variant, State::Hover, t);
        let pressed = button_colors(self.variant, State::Pressed, t);
        let (pad, gap, font) = if filled(self.variant) {
            (20.0, 8.0, 14.0)
        } else if self.size == Size::Large {
            (16.0, 6.0, 14.0)
        } else {
            (16.0, 6.0, 13.0)
        };
        let ring = focus::style(t, self.ring_gap.unwrap_or(t.bg));
        let enabled = !self.disabled;
        let handler = self.on_click;
        div()
            .id(self.id)
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .gap(px(gap))
            .h(px(self.size.height()))
            .px(px(pad))
            .rounded_full()
            .text_size(px(font))
            .font_weight(fonts::weight(600))
            .text_color(rest.fg)
            .when_some(rest.bg, |d, c| d.bg(c))
            .when_some(rest.border, |d, c| d.border_1().border_color(c))
            // Desabilitado nao entra na ordem do Tab: pararia num alvo sem anel e sem acao.
            .when_some(self.focus.clone().filter(|_| enabled), |d, h| {
                d.track_focus(&h)
            })
            .when(enabled, |d| {
                d.cursor_pointer()
                    .hover(move |s| match hover.bg {
                        Some(c) => s.bg(c),
                        None => s,
                    })
                    .active(move |s| match pressed.bg {
                        Some(c) => s.bg(c),
                        None => s,
                    })
                    .focus_visible(ring)
            })
            .when(!enabled, |d| d.opacity(0.4))
            .when_some(handler.filter(|_| enabled), |d, f| {
                d.on_click(move |_: &ClickEvent, window, cx| f(window, cx))
            })
            .when_some(self.icon, |d, i| d.child(icon(i, px(18.0), rest.fg)))
            .child(self.label)
    }
}

/// Fundo do `IconButton` por estado: so hover e pressionado pintam o circulo.
pub fn icon_button_bg(state: State, t: &Tokens) -> Option<Hsla> {
    match state {
        State::Pressed => Some(t.divider),
        State::Hover => Some(t.surface_2),
        State::Rest | State::Disabled => None,
    }
}

/// Botao fantasma de 36 x 36, circulo.
#[derive(IntoElement)]
pub struct IconButton {
    id: ElementId,
    glyph: Icon,
    tokens: &'static Tokens,
    disabled: bool,
    ring_gap: Option<Hsla>,
    focus: Option<FocusHandle>,
    on_click: Option<Handler>,
}

impl IconButton {
    pub fn new(id: impl Into<ElementId>, glyph: Icon, tokens: &'static Tokens) -> IconButton {
        IconButton {
            id: id.into(),
            glyph,
            tokens,
            disabled: false,
            ring_gap: None,
            focus: None,
            on_click: None,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> IconButton {
        self.disabled = disabled;
        self
    }

    pub fn ring_gap(mut self, gap: Hsla) -> IconButton {
        self.ring_gap = Some(gap);
        self
    }

    pub fn focus(mut self, handle: &FocusHandle) -> IconButton {
        self.focus = Some(handle.clone());
        self
    }

    pub fn on_click(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> IconButton {
        self.on_click = Some(Box::new(f));
        self
    }
}

impl RenderOnce for IconButton {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let t = self.tokens;
        let hover = icon_button_bg(State::Hover, t);
        let pressed = icon_button_bg(State::Pressed, t);
        let ring = focus::style(t, self.ring_gap.unwrap_or(t.bg));
        let enabled = !self.disabled;
        let handler = self.on_click;
        div()
            .id(self.id)
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(36.0))
            .rounded_full()
            // Desabilitado nao entra na ordem do Tab: pararia num alvo sem anel e sem acao.
            .when_some(self.focus.filter(|_| enabled), |d, h| d.track_focus(&h))
            .when(enabled, |d| {
                d.cursor_pointer()
                    .hover(move |s| match hover {
                        Some(c) => s.bg(c),
                        None => s,
                    })
                    .active(move |s| match pressed {
                        Some(c) => s.bg(c),
                        None => s,
                    })
                    .focus_visible(ring)
            })
            .when(!enabled, |d| d.opacity(0.4))
            .when_some(handler.filter(|_| enabled), |d, f| {
                d.on_click(move |_: &ClickEvent, window, cx| f(window, cx))
            })
            .child(icon(self.glyph, px(18.0), t.text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::theme::{DARK, LIGHT};
    use gpui::Rgba;

    fn hex(c: Hsla) -> u32 {
        let c = Rgba::from(c);
        let b = |v: f32| (v * 255.0).round() as u32;
        (b(c.r) << 16) | (b(c.g) << 8) | b(c.b)
    }

    const ALL_STATES: [State; 4] = [State::Rest, State::Hover, State::Pressed, State::Disabled];

    #[test]
    fn primario_repouso_hover_e_pressionado_nos_dois_temas() {
        for t in [&*DARK, &*LIGHT] {
            let at = |s| button_colors(Variant::Primary, s, t);
            assert_eq!(at(State::Rest).bg, Some(t.highlight));
            assert_eq!(at(State::Rest).fg, t.on_highlight);
            assert_eq!(at(State::Rest).border, None);
            assert_eq!(at(State::Hover).bg, Some(mix(t.highlight, t.text, 0.14)));
            assert_eq!(at(State::Pressed).bg, Some(mix(t.highlight, black(), 0.22)));
            assert_eq!(at(State::Disabled), at(State::Rest));
        }
        // Tema escuro, conta feita a mao: 0xE2C15A com 14 % de 0xF2F2F2 e com 22 % de preto.
        let d = &*DARK;
        let bg = |s| hex(button_colors(Variant::Primary, s, d).bg.unwrap());
        assert_eq!(bg(State::Hover), 0xE4C86F);
        assert_eq!(bg(State::Pressed), 0xB09746);
    }

    #[test]
    fn contorno_e_fantasma_so_diferem_na_borda() {
        for t in [&*DARK, &*LIGHT] {
            for s in ALL_STATES {
                let o = button_colors(Variant::Outline, s, t);
                let g = button_colors(Variant::Ghost, s, t);
                assert_eq!((o.bg, o.fg), (g.bg, g.fg), "{s:?}");
                assert_eq!(o.border, Some(t.divider));
                assert_eq!(g.border, None);
                assert_eq!(o.fg, t.text);
            }
            let o = |s| button_colors(Variant::Outline, s, t).bg;
            assert_eq!(o(State::Rest), None);
            assert_eq!(o(State::Hover), Some(t.surface_2));
            assert_eq!(o(State::Pressed), Some(t.divider));
        }
    }

    #[test]
    fn destrutivo_usa_danger_nos_dois_formatos() {
        for t in [&*DARK, &*LIGHT] {
            let c = button_colors(Variant::DangerFilled, State::Rest, t);
            assert_eq!((c.bg, c.fg), (Some(t.danger), t.on_danger));
            assert_eq!(
                button_colors(Variant::DangerFilled, State::Hover, t).bg,
                Some(mix(t.danger, t.text, 0.14))
            );
            assert_eq!(
                button_colors(Variant::DangerFilled, State::Pressed, t).bg,
                Some(mix(t.danger, black(), 0.26))
            );
            let c = button_colors(Variant::Danger, State::Rest, t);
            assert_eq!((c.bg, c.fg, c.border), (None, t.danger, Some(t.danger)));
            // Hover 12 % e pressionado 22 % do Perigo (contorno) sao PRESUNCAO: o design so
            // define o repouso (texto e borda `danger`); os valores seguem a mesma ideia do
            // destrutivo cheio, com `danger` translucido.
            let d = |s| button_colors(Variant::Danger, s, t);
            assert_eq!(d(State::Hover).bg, Some(t.danger.opacity(0.12)));
            assert_eq!(d(State::Pressed).bg, Some(t.danger.opacity(0.22)));
            for s in [State::Hover, State::Pressed, State::Disabled] {
                assert_eq!((d(s).fg, d(s).border), (t.danger, Some(t.danger)), "{s:?}");
            }
            assert_eq!(d(State::Disabled), d(State::Rest));
            let f = |s| button_colors(Variant::DangerFilled, s, t);
            assert_eq!(f(State::Disabled), f(State::Rest));
        }
    }

    fn lum(c: Hsla) -> f32 {
        let c = Rgba::from(c);
        let f = |v: f32| {
            if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * f(c.r) + 0.7152 * f(c.g) + 0.0722 * f(c.b)
    }

    fn ratio(a: Hsla, b: Hsla) -> f32 {
        let (x, y) = (lum(a), lum(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn texto_do_botao_tem_contraste_legivel_em_todo_estado() {
        for t in [&*DARK, &*LIGHT] {
            for v in [Variant::Primary, Variant::DangerFilled] {
                for s in [State::Rest, State::Hover, State::Pressed] {
                    let c = button_colors(v, s, t);
                    let r = ratio(c.fg, c.bg.unwrap());
                    // Pressionado escurece o fundo e e transitorio: 3:1 (texto de 14/600).
                    let min = if s == State::Pressed { 3.0 } else { 4.5 };
                    assert!(r >= min, "{v:?} {s:?} escuro={}: {r}", t.dark);
                }
            }
            for v in [Variant::Outline, Variant::Ghost] {
                for s in ALL_STATES {
                    let c = button_colors(v, s, t);
                    assert!(ratio(c.fg, c.bg.unwrap_or(t.bg)) >= 4.5, "{v:?} {s:?}");
                }
            }
        }
    }

    #[test]
    fn alturas_do_design() {
        assert_eq!(Size::Large.height(), 44.0);
        assert_eq!(Size::Medium.height(), 40.0);
        assert_eq!(Size::Small.height(), 36.0);
    }

    #[test]
    fn botao_de_icone_pinta_so_hover_e_pressionado() {
        for t in [&*DARK, &*LIGHT] {
            assert_eq!(icon_button_bg(State::Rest, t), None);
            assert_eq!(icon_button_bg(State::Hover, t), Some(t.surface_2));
            assert_eq!(icon_button_bg(State::Pressed, t), Some(t.divider));
            assert_eq!(icon_button_bg(State::Disabled, t), None);
        }
    }
}
