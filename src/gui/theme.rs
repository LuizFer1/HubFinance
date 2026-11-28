//! Tokens de cor do design (spec §4.1) e o tema do `gpui-component`.
//!
//! Dois conjuntos fixos, escuro e claro; "Sistema" escolhe um deles pelo `system_dark` do
//! `ui_model`. O tema do Windows continua vindo so do registro, lido a cada tique por
//! `Desktop::system_dark` (motivo em `desktop/appearance.rs`). Se `observe_window_appearance`
//! e confiavel nao da para conferir sem trocar o tema do Windows a mao: fica para o dono, e ate
//! la o registro manda.

use std::sync::LazyLock;

use gpui::{App, BoxShadow, Hsla, Rgba, point, px};
use gpui_component::{Theme, ThemeMode};

use super::fonts;

/// `#RRGGBB` como cor do GPUI.
pub fn rgb(hex: u32) -> Hsla {
    gpui::rgb(hex).into()
}

/// `rgba(r, g, b, a)` do CSS (`a` de 0 a 1).
pub fn rgba(r: u8, g: u8, b: u8, a: f32) -> Hsla {
    Rgba {
        r: f32::from(r) / 255.0,
        g: f32::from(g) / 255.0,
        b: f32::from(b) / 255.0,
        a,
    }
    .into()
}

/// Borda de 1 px e sombra suave do card no tema claro (`--card-ring`). No escuro nao ha.
#[allow(dead_code)] // usado na Tarefa 12
#[derive(Debug, Clone, PartialEq)]
pub struct CardRing {
    pub border: Hsla,
    pub shadow: BoxShadow,
}

#[allow(dead_code)] // campos usados nas Tarefas 11 a 15
#[derive(Debug, Clone, PartialEq)]
pub struct Tokens {
    pub dark: bool,
    pub bg: Hsla,
    pub surface: Hsla,
    pub surface_2: Hsla,
    pub text: Hsla,
    pub muted: Hsla,
    pub divider: Hsla,
    pub warning: Hsla,
    pub accent: Hsla,
    pub accent_200: Hsla,
    pub accent_300: Hsla,
    pub accent_400: Hsla,
    pub accent_700: Hsla,
    pub accent_900: Hsla,
    pub highlight: Hsla,
    pub on_highlight: Hsla,
    pub on_highlight_muted: Hsla,
    pub income: Hsla,
    pub expense: Hsla,
    pub ok: Hsla,
    pub danger: Hsla,
    pub on_danger: Hsla,
    pub chart: [Hsla; 6],
    pub card_ring: Option<CardRing>,
    pub scrim: Hsla,
    /// Sombra do dialogo: `0 24 64 rgba(0,0,0,.35)`, igual nos dois temas.
    pub dialog_shadow: BoxShadow,
    pub toast_bg: Hsla,
    pub toast_fg: Hsla,
    pub toast_muted: Hsla,
    pub toast_accent: Hsla,
    /// Hover do X da barra de titulo (padrao do Windows), fixo.
    pub close_hover: Hsla,
}

fn dialog_shadow() -> BoxShadow {
    BoxShadow {
        color: rgba(0, 0, 0, 0.35),
        offset: point(px(0.0), px(24.0)),
        blur_radius: px(64.0),
        spread_radius: px(0.0),
        inset: false,
    }
}

fn dark() -> Tokens {
    Tokens {
        dark: true,
        bg: rgb(0x131313),
        surface: rgb(0x1E1E1E),
        surface_2: rgb(0x262626),
        text: rgb(0xF2F2F2),
        muted: rgb(0xA6A6A6),
        divider: rgb(0x333333),
        warning: rgb(0xF2994A),
        accent: rgb(0xE2C15A),
        accent_200: rgb(0xF8E9B8),
        accent_300: rgb(0xF0D98C),
        accent_400: rgb(0xE8CD78),
        accent_700: rgb(0x847335),
        accent_900: rgb(0x332D14),
        highlight: rgb(0xE2C15A),
        on_highlight: rgb(0x131313),
        on_highlight_muted: rgb(0x4A3F1A),
        income: rgb(0x8FD3A8),
        expense: rgb(0xE8806F),
        ok: rgb(0x8FD3A8),
        danger: rgb(0xF28B82),
        on_danger: rgb(0x131313),
        chart: [
            rgb(0x7FA7E8),
            rgb(0x5CC3E0),
            rgb(0xA48CF0),
            rgb(0xE08AD6),
            rgb(0xF7CADF),
            rgb(0x8C8C8C),
        ],
        card_ring: None,
        scrim: rgba(0, 0, 0, 0.6),
        dialog_shadow: dialog_shadow(),
        toast_bg: rgb(0xF2F2F2),
        toast_fg: rgb(0x131313),
        toast_muted: rgb(0x4F4F4F),
        toast_accent: rgb(0x675412),
        close_hover: rgb(0xC42B1C),
    }
}

fn light() -> Tokens {
    Tokens {
        dark: false,
        bg: rgb(0xF5F5F5),
        surface: rgb(0xFFFFFF),
        surface_2: rgb(0xEEEEEE),
        text: rgb(0x202020),
        muted: rgb(0x6B6B6B),
        divider: rgb(0xE4E4E4),
        warning: rgb(0x975201),
        accent: rgb(0x876C00),
        accent_200: rgb(0x4D4014),
        accent_300: rgb(0x675412),
        accent_400: rgb(0x866C10),
        accent_700: rgb(0xDBC276),
        accent_900: rgb(0xF8EFD5),
        highlight: rgb(0xDBC276),
        on_highlight: rgb(0x202020),
        on_highlight_muted: rgb(0x4D4014),
        income: rgb(0x17663A),
        expense: rgb(0xD0564A),
        ok: rgb(0x17663A),
        danger: rgb(0xB3261E),
        on_danger: rgb(0xFFFFFF),
        chart: [
            rgb(0x3D6CC0),
            rgb(0x1A7391),
            rgb(0x6146C4),
            rgb(0xA03A93),
            rgb(0xC9628C),
            rgb(0x8A8A8A),
        ],
        card_ring: Some(CardRing {
            border: rgb(0xE9E9E9),
            shadow: BoxShadow {
                color: rgba(0, 0, 0, 0.04),
                offset: point(px(0.0), px(1.0)),
                blur_radius: px(2.0),
                spread_radius: px(0.0),
                inset: false,
            },
        }),
        scrim: rgba(32, 32, 32, 0.35),
        dialog_shadow: dialog_shadow(),
        toast_bg: rgb(0x262626),
        toast_fg: rgb(0xF2F2F2),
        toast_muted: rgb(0xBDBDBD),
        toast_accent: rgb(0xE2C15A),
        close_hover: rgb(0xC42B1C),
    }
}

pub static DARK: LazyLock<Tokens> = LazyLock::new(dark);
pub static LIGHT: LazyLock<Tokens> = LazyLock::new(light);

impl Tokens {
    /// O conjunto do tema efetivo (`ThemeMode::is_dark(system_dark)` ja resolvido).
    pub fn of(is_dark: bool) -> &'static Tokens {
        if is_dark { &DARK } else { &LIGHT }
    }
}

/// Aplica os tokens ao tema global do `gpui-component`, que pinta campos, rolagem e menus.
///
/// Dois passos de proposito: `Theme::change` carrega o tema registrado do modo e substitui as
/// cores; so depois se editam as cores (a doc de `Theme::update` avisa que mudar modo e cores
/// no mesmo passo perde as cores). Trocar so por `global_mut` deixaria `tokens` com as cores
/// velhas e metade dos controles na cor errada.
pub fn apply(tokens: &Tokens, cx: &mut App) {
    Theme::change(
        if tokens.dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        },
        None,
        cx,
    );
    let t = tokens.clone();
    Theme::update(cx, |theme| {
        theme.font_family = fonts::FAMILY.into();
        let c = &mut theme.colors;
        c.background = t.bg;
        c.foreground = t.text;
        c.border = t.divider;
        c.input = t.divider;
        c.muted = t.surface_2;
        c.muted_foreground = t.muted;
        // Anel de foco do design: `accent_400`.
        c.ring = t.accent_400;
        c.selection = t.accent_900;
        c.caret = t.accent;
        c.accent = t.surface_2;
        c.accent_foreground = t.text;
        c.primary = t.highlight;
        c.primary_foreground = t.on_highlight;
        c.danger = t.danger;
        c.danger_foreground = t.on_danger;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hex `#RRGGBB` da cor, sem alfa.
    fn hex(c: Hsla) -> u32 {
        let c = Rgba::from(c);
        let b = |v: f32| (v * 255.0).round() as u32;
        (b(c.r) << 16) | (b(c.g) << 8) | b(c.b)
    }

    type Field = fn(&Tokens) -> Hsla;

    /// A tabela 4.1 da spec, linha a linha: nome, escuro, claro.
    fn tabela() -> Vec<(&'static str, Field, u32, u32)> {
        vec![
            ("bg", |t| t.bg, 0x131313, 0xF5F5F5),
            ("surface", |t| t.surface, 0x1E1E1E, 0xFFFFFF),
            ("surface_2", |t| t.surface_2, 0x262626, 0xEEEEEE),
            ("text", |t| t.text, 0xF2F2F2, 0x202020),
            ("muted", |t| t.muted, 0xA6A6A6, 0x6B6B6B),
            ("divider", |t| t.divider, 0x333333, 0xE4E4E4),
            ("warning", |t| t.warning, 0xF2994A, 0x975201),
            ("accent", |t| t.accent, 0xE2C15A, 0x876C00),
            ("accent_200", |t| t.accent_200, 0xF8E9B8, 0x4D4014),
            ("accent_300", |t| t.accent_300, 0xF0D98C, 0x675412),
            ("accent_400", |t| t.accent_400, 0xE8CD78, 0x866C10),
            ("accent_700", |t| t.accent_700, 0x847335, 0xDBC276),
            ("accent_900", |t| t.accent_900, 0x332D14, 0xF8EFD5),
            ("highlight", |t| t.highlight, 0xE2C15A, 0xDBC276),
            ("on_highlight", |t| t.on_highlight, 0x131313, 0x202020),
            (
                "on_highlight_muted",
                |t| t.on_highlight_muted,
                0x4A3F1A,
                0x4D4014,
            ),
            ("income", |t| t.income, 0x8FD3A8, 0x17663A),
            ("expense", |t| t.expense, 0xE8806F, 0xD0564A),
            ("ok", |t| t.ok, 0x8FD3A8, 0x17663A),
            ("danger", |t| t.danger, 0xF28B82, 0xB3261E),
            ("on_danger", |t| t.on_danger, 0x131313, 0xFFFFFF),
            ("chart_1", |t| t.chart[0], 0x7FA7E8, 0x3D6CC0),
            ("chart_2", |t| t.chart[1], 0x5CC3E0, 0x1A7391),
            ("chart_3", |t| t.chart[2], 0xA48CF0, 0x6146C4),
            ("chart_4", |t| t.chart[3], 0xE08AD6, 0xA03A93),
            ("chart_5", |t| t.chart[4], 0xF7CADF, 0xC9628C),
            ("chart_6", |t| t.chart[5], 0x8C8C8C, 0x8A8A8A),
            ("toast_bg", |t| t.toast_bg, 0xF2F2F2, 0x262626),
            ("toast_fg", |t| t.toast_fg, 0x131313, 0xF2F2F2),
            ("toast_muted", |t| t.toast_muted, 0x4F4F4F, 0xBDBDBD),
            ("toast_accent", |t| t.toast_accent, 0x675412, 0xE2C15A),
            ("close_hover", |t| t.close_hover, 0xC42B1C, 0xC42B1C),
        ]
    }

    #[test]
    fn tokens_batem_com_o_design() {
        for (name, field, dark_hex, light_hex) in tabela() {
            assert_eq!(hex(field(&DARK)), dark_hex, "{name} escuro");
            assert_eq!(hex(field(&LIGHT)), light_hex, "{name} claro");
            assert_eq!(field(&DARK).a, 1.0, "{name} escuro opaco");
            assert_eq!(field(&LIGHT).a, 1.0, "{name} claro opaco");
        }
    }

    #[test]
    fn scrim_sombras_e_anel_do_card() {
        let (d, l) = (&*DARK, &*LIGHT);
        assert_eq!(hex(d.scrim), 0x000000);
        assert!((d.scrim.a - 0.6).abs() < 1e-6);
        assert_eq!(hex(l.scrim), 0x202020);
        assert!((l.scrim.a - 0.35).abs() < 1e-6);
        assert!(d.card_ring.is_none());
        let ring = l.card_ring.as_ref().expect("o claro tem anel");
        assert_eq!(hex(ring.border), 0xE9E9E9);
        assert!((ring.shadow.color.a - 0.04).abs() < 1e-6);
        assert_eq!(ring.shadow.offset.y, px(1.0));
        assert_eq!(ring.shadow.blur_radius, px(2.0));
        for t in [d, l] {
            assert_eq!(t.dialog_shadow.offset.y, px(24.0));
            assert_eq!(t.dialog_shadow.blur_radius, px(64.0));
            assert!((t.dialog_shadow.color.a - 0.35).abs() < 1e-6);
        }
        assert!(d.dark && !l.dark);
        assert!(std::ptr::eq(Tokens::of(true), &*DARK));
        assert!(std::ptr::eq(Tokens::of(false), &*LIGHT));
    }

    /// Luminancia relativa WCAG 2 (sRGB).
    fn luminance(c: Hsla) -> f64 {
        let c = Rgba::from(c);
        let lin = |v: f32| {
            let s = f64::from(v);
            if s <= 0.04045 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b)
    }

    fn contrast(a: Hsla, b: Hsla) -> f64 {
        let (la, lb) = (luminance(a), luminance(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    #[test]
    fn contraste_minimo_nos_dois_temas() {
        for t in [&*DARK, &*LIGHT] {
            let n = if t.dark { "escuro" } else { "claro" };
            assert!(contrast(t.text, t.bg) >= 4.5, "text x bg {n}");
            assert!(contrast(t.text, t.surface) >= 4.5, "text x surface {n}");
            assert!(contrast(t.muted, t.surface) >= 3.0, "muted x surface {n}");
            assert!(
                contrast(t.on_highlight, t.highlight) >= 4.5,
                "on_highlight x highlight {n}"
            );
        }
    }
}
