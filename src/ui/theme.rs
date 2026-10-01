//! Tema Nocturne, escuro e claro.
//!
//! Toda cor de widget vem de `Tokens`, nao da paleta derivada do iced: o `Palette` do iced
//! gera rampas proprias que nao batem com as do handoff, e usa-las seria aproximar. O `Theme`
//! do iced so existe para o fundo da janela e para o que nao tem estilo explicito.

// Parte dos estilos so ganha uso nas telas do plano 2b (Dashboard e Lancamentos).
#![allow(dead_code)]

use iced::widget::{button, container};
use iced::{Background, Border, Color, Shadow, Theme, Vector};

use crate::config::ThemeMode;
use crate::dashboard::colors::{self, Mode, Rgb};

/// Cores de um tema. Os nomes seguem a tabela "Tokens de cor" da spec.
#[derive(Debug)]
pub struct Tokens {
    pub mode: Mode,
    pub bg: Color,
    pub surface: Color,
    pub side: Color,
    pub text: Color,
    pub accent: Color,
    pub accent_200: Color,
    pub accent_300: Color,
    pub accent_700: Color,
    pub accent_900: Color,
    pub neutral_100: Color,
    pub neutral_600: Color,
    pub neutral_700: Color,
    pub neutral_800: Color,
    pub neutral_900: Color,
    pub income: Color,
    pub income_fg: Color,
    pub expense: Color,
    pub expense_fg: Color,
    pub amber: Color,
    pub close_hover: Color,
    /// Texto sobre `close_hover`.
    pub close_hover_text: Color,
    divider_alpha: f32,
    shadow_md_ring: Color,
    shadow_md_color: Color,
    shadow_lg_ring: Color,
    shadow_lg_color: Color,
}

const fn hex(v: u32) -> Color {
    Color::from_rgb8((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

const fn rgb(c: Rgb) -> Color {
    Color::from_rgb8(c.0, c.1, c.2)
}

pub static DARK: Tokens = Tokens {
    mode: Mode::Dark,
    bg: hex(0x161826),
    surface: hex(0x232532),
    // `surface` a 45 % sobre `bg`, como o `--side` do prototipo.
    side: hex(0x1c1e2b),
    text: hex(0xe9e9ed),
    accent: hex(0x9184d9),
    accent_200: hex(0xe7e5fe),
    accent_300: hex(0xd2cefd),
    accent_700: hex(0x5d5294),
    accent_900: hex(0x2b2741),
    neutral_100: hex(0xf3f5fe),
    neutral_600: hex(0x75798c),
    neutral_700: hex(0x595d6c),
    neutral_800: hex(0x3f424d),
    neutral_900: hex(0x292b31),
    income: rgb(colors::INCOME.dark),
    income_fg: rgb(colors::INCOME_FG.dark),
    expense: rgb(colors::EXPENSE.dark),
    expense_fg: rgb(colors::EXPENSE_FG.dark),
    amber: hex(0xdeb866),
    close_hover: hex(0xc42b1c),
    close_hover_text: hex(0xf3f5fe),
    divider_alpha: 0.16,
    shadow_md_ring: hex(0x595d6c),
    shadow_md_color: Color::from_rgba8(0, 0, 0, 0.55),
    shadow_lg_ring: hex(0x9397ab),
    shadow_lg_color: Color::from_rgba8(0, 0, 0, 0.65),
};

pub static LIGHT: Tokens = Tokens {
    mode: Mode::Light,
    bg: hex(0xe4e7f5),
    surface: hex(0xf3f5fe),
    side: hex(0xdde0ef),
    text: hex(0x161826),
    accent: hex(0x796cbf),
    accent_200: hex(0x423a6a),
    accent_300: hex(0x5d5294),
    accent_700: hex(0xb5abfc),
    accent_900: hex(0xe7e5fe),
    neutral_100: hex(0xf3f5fe),
    neutral_600: hex(0x75798c),
    neutral_700: hex(0xb2b6ca),
    neutral_800: hex(0xcfd3e5),
    neutral_900: hex(0xe4e7f5),
    income: rgb(colors::INCOME.light),
    income_fg: rgb(colors::INCOME_FG.light),
    expense: rgb(colors::EXPENSE.light),
    expense_fg: rgb(colors::EXPENSE_FG.light),
    amber: hex(0xb08505),
    close_hover: hex(0xc42b1c),
    close_hover_text: hex(0xf3f5fe),
    divider_alpha: 0.13,
    shadow_md_ring: hex(0xcfd3e5),
    shadow_md_color: Color::from_rgba8(0x16, 0x18, 0x26, 0.12),
    shadow_lg_ring: hex(0xb2b6ca),
    shadow_lg_color: Color::from_rgba8(0x16, 0x18, 0x26, 0.20),
};

impl Tokens {
    pub fn for_mode(mode: ThemeMode) -> &'static Tokens {
        match mode {
            ThemeMode::Dark => &DARK,
            ThemeMode::Light => &LIGHT,
        }
    }

    pub fn iced_theme(&self) -> Theme {
        let name = match self.mode {
            Mode::Dark => "Nocturne",
            Mode::Light => "Nocturne Claro",
        };
        Theme::custom(
            name,
            iced::theme::Palette {
                background: self.bg,
                text: self.text,
                primary: self.accent,
                success: self.income,
                warning: self.amber,
                danger: self.expense,
            },
        )
    }

    /// Texto secundario: o design usa o texto a 75/70/65/60/55/50/45 % de opacidade.
    pub fn text_alpha(&self, a: f32) -> Color {
        alpha(self.text, a)
    }

    pub fn divider(&self) -> Color {
        alpha(self.text, self.divider_alpha)
    }

    pub fn rgb(&self, rgb: Rgb) -> Color {
        Color::from_rgb8(rgb.0, rgb.1, rgb.2)
    }

    /// Cor de um token da paleta (categoria ou pessoa) neste tema.
    pub fn token(&self, token: &str) -> Color {
        self.rgb(colors::token_color(token, self.mode))
    }

    /// Cor do token pre-misturada sobre a superficie (`color-mix` do prototipo).
    pub fn token_tint(&self, token: &str, amount: f32) -> Color {
        mix(self.surface, self.token(token), amount)
    }

    /// `0 0 0 1px` + `0 6px 18px`: o anel vira a borda de 1 px (o iced desenha borda por
    /// dentro, o que da a mesma caixa visual para quem olha).
    pub fn shadow_md(&self) -> (Border, Shadow) {
        (
            Border {
                color: self.shadow_md_ring,
                width: 1.0,
                radius: 8.0.into(),
            },
            Shadow {
                color: self.shadow_md_color,
                offset: Vector::new(0.0, 6.0),
                blur_radius: 18.0,
            },
        )
    }

    pub fn shadow_lg(&self) -> (Border, Shadow) {
        (
            Border {
                color: self.shadow_lg_ring,
                width: 1.0,
                radius: 14.0.into(),
            },
            Shadow {
                color: self.shadow_lg_color,
                offset: Vector::new(0.0, 16.0),
                blur_radius: 40.0,
            },
        )
    }
}

pub fn alpha(color: Color, a: f32) -> Color {
    Color { a, ..color }
}

/// Mistura opaca por canal, para os `color-mix` sobre fundo solido.
pub fn mix(base: Color, over: Color, amount: f32) -> Color {
    let t = amount.clamp(0.0, 1.0);
    Color::from_rgb(
        base.r + (over.r - base.r) * t,
        base.g + (over.g - base.g) * t,
        base.b + (over.b - base.b) * t,
    )
}

fn bg(color: Color) -> Option<Background> {
    Some(Background::Color(color))
}

fn rounded(radius: f32) -> Border {
    Border {
        radius: radius.into(),
        ..Border::default()
    }
}

fn outline(color: Color, radius: f32) -> Border {
    Border {
        color,
        width: 1.0,
        radius: radius.into(),
    }
}

fn hovered(status: button::Status) -> bool {
    matches!(status, button::Status::Hovered | button::Status::Pressed)
}

// ---- containers ----

/// Card: superficie solida, raio 8.
pub fn card(t: &'static Tokens) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: bg(t.surface),
        border: rounded(8.0),
        text_color: Some(t.text),
        ..container::Style::default()
    }
}

/// Card de status da barra lateral: superficie com a borda interna no divisor.
pub fn card_inset(t: &'static Tokens) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: bg(t.surface),
        border: outline(t.divider(), 8.0),
        text_color: Some(t.text),
        ..container::Style::default()
    }
}

/// Campo de endereco: fundo da janela com a borda interna no divisor.
pub fn field(t: &'static Tokens) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: bg(t.bg),
        border: outline(t.divider(), 8.0),
        text_color: Some(t.text),
        ..container::Style::default()
    }
}

/// Fundo liso de uma cor, com raio (barra de titulo, lateral, quadro do QR...).
pub fn fill(color: Color, radius: f32) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: bg(color),
        border: rounded(radius),
        ..container::Style::default()
    }
}

/// Borda de 1 px, fundo transparente (segmentado, separadores com contorno).
pub fn outlined(color: Color, radius: f32) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        border: outline(color, radius),
        ..container::Style::default()
    }
}

pub fn menu(t: &'static Tokens) -> impl Fn(&Theme) -> container::Style {
    move |_| {
        let (border, shadow) = t.shadow_md();
        container::Style {
            background: bg(t.surface),
            border,
            shadow,
            text_color: Some(t.text),
            ..container::Style::default()
        }
    }
}

pub fn dialog(t: &'static Tokens) -> impl Fn(&Theme) -> container::Style {
    move |_| {
        let (border, shadow) = t.shadow_lg();
        container::Style {
            background: bg(t.surface),
            border,
            shadow,
            text_color: Some(t.text),
            ..container::Style::default()
        }
    }
}

pub fn toast(t: &'static Tokens) -> impl Fn(&Theme) -> container::Style {
    menu(t)
}

/// Fundo do dialogo: preto a 55 % sobre a janela inteira.
pub fn scrim() -> impl Fn(&Theme) -> container::Style {
    |_| container::Style {
        background: bg(Color::from_rgba8(0, 0, 0, 0.55)),
        ..container::Style::default()
    }
}

// ---- botoes ----

/// Item de navegacao: ativo `accent_900` + `accent_200`; hover `accent` a 10 % (no prototipo
/// o hover vale tambem para o item ativo).
pub fn nav_item(
    t: &'static Tokens,
    active: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let background = if hovered(status) {
            bg(alpha(t.accent, 0.10))
        } else if active {
            bg(t.accent_900)
        } else {
            None
        };
        button::Style {
            background,
            text_color: if active {
                t.accent_200
            } else {
                t.text_alpha(0.75)
            },
            border: rounded(8.0),
            ..button::Style::default()
        }
    }
}

/// Opcao de segmentado (tema, tipo): raio 6, ativo `accent_900` + `accent_200`, inativo a 60 %.
pub fn seg_option(
    t: &'static Tokens,
    active: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| button::Style {
        background: if active {
            bg(t.accent_900)
        } else if hovered(status) {
            bg(t.text_alpha(0.05))
        } else {
            None
        },
        text_color: if active {
            t.accent_200
        } else {
            t.text_alpha(0.60)
        },
        border: rounded(6.0),
        ..button::Style::default()
    }
}

/// Botao fantasma no acento ("Copiar", "Desfazer", "Gerar outro codigo").
pub fn ghost(t: &'static Tokens) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| button::Style {
        background: hovered(status).then(|| Background::Color(alpha(t.accent, 0.12))),
        text_color: t.accent_300,
        border: rounded(6.0),
        ..button::Style::default()
    }
}

/// Link de texto: `accent_300`, hover `accent_200` ("Ver todos", "Limpar filtros", "Ver como").
pub fn link(t: &'static Tokens) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| button::Style {
        background: None,
        text_color: if hovered(status) {
            t.accent_200
        } else {
            t.accent_300
        },
        ..button::Style::default()
    }
}

/// Contorno no acento, fundo a 8 % e hover a 16 % ("Gerar codigo", "Limpar filtros" do vazio).
pub fn outline_accent(t: &'static Tokens) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| button::Style {
        background: bg(alpha(t.accent, if hovered(status) { 0.16 } else { 0.08 })),
        text_color: t.accent_300,
        border: outline(t.accent, 8.0),
        ..button::Style::default()
    }
}

/// "Cancelar": contorno no divisor, hover texto a 7 %.
pub fn outline_divider(t: &'static Tokens) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| button::Style {
        background: hovered(status).then(|| Background::Color(t.text_alpha(0.07))),
        text_color: t.text,
        border: outline(t.divider(), 8.0),
        ..button::Style::default()
    }
}

/// "Remover" da lista: texto de despesa, contorno da despesa a 45 %, hover a 12 %.
pub fn danger_outline(t: &'static Tokens) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| button::Style {
        background: hovered(status).then(|| Background::Color(alpha(t.expense, 0.12))),
        text_color: t.expense_fg,
        border: outline(alpha(t.expense, 0.45), 8.0),
        ..button::Style::default()
    }
}

/// "Remover" do dialogo: fundo da despesa a 14 % (hover 24 %), contorno a 55 %.
pub fn danger_filled(t: &'static Tokens) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| button::Style {
        background: bg(alpha(t.expense, if hovered(status) { 0.24 } else { 0.14 })),
        text_color: t.expense_fg,
        border: outline(alpha(t.expense, 0.55), 8.0),
        ..button::Style::default()
    }
}

/// Linha de lista: transparente, hover texto a 4 %, raio 8.
pub fn row_hover(t: &'static Tokens) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| button::Style {
        background: hovered(status).then(|| Background::Color(t.text_alpha(0.04))),
        text_color: t.text,
        border: rounded(8.0),
        ..button::Style::default()
    }
}

/// Botoes da barra de titulo: hover texto a 8 %; o fechar fica vermelho do Windows.
pub fn titlebar_button(
    t: &'static Tokens,
    close: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let hover = hovered(status);
        button::Style {
            background: match (hover, close) {
                (true, true) => bg(t.close_hover),
                (true, false) => bg(t.text_alpha(0.08)),
                (false, _) => None,
            },
            text_color: if hover && close {
                t.close_hover_text
            } else {
                t.text_alpha(0.70)
            },
            border: rounded(0.0),
            ..button::Style::default()
        }
    }
}

/// Chip pilula (autor): ativo borda `accent` + `accent_900` + `accent_200`; inativo superficie
/// com borda no divisor e hover texto a 5 % pre-misturado.
pub fn chip(t: &'static Tokens, active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let (background, border, text_color) = if active {
            (t.accent_900, t.accent, t.accent_200)
        } else if hovered(status) {
            (mix(t.surface, t.text, 0.05), t.divider(), t.text)
        } else {
            (t.surface, t.divider(), t.text)
        };
        button::Style {
            background: bg(background),
            text_color,
            border: outline(border, 999.0),
            ..button::Style::default()
        }
    }
}

/// Botao sem fundo nem hover (setas do seletor de mes, area clicavel generica).
pub fn plain(color: Color) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, _| button::Style {
        background: None,
        text_color: color,
        ..button::Style::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_por_modo() {
        assert_eq!(Tokens::for_mode(ThemeMode::Dark).bg, hex(0x161826));
        assert_eq!(Tokens::for_mode(ThemeMode::Light).bg, hex(0xe4e7f5));
        assert_eq!(DARK.income, Color::from_rgb8(0x75, 0xd5, 0xa0));
        assert_eq!(LIGHT.expense_fg, Color::from_rgb8(0xaf, 0x3c, 0x40));
    }

    #[test]
    fn divisor_e_texto_secundario_sao_alpha_do_texto() {
        assert_eq!(DARK.divider(), alpha(DARK.text, 0.16));
        assert_eq!(LIGHT.divider(), alpha(LIGHT.text, 0.13));
        assert!((DARK.text_alpha(0.55).a - 0.55).abs() < f32::EPSILON);
    }

    #[test]
    fn token_segue_o_modo() {
        assert_eq!(DARK.token("fuchsia"), Color::from_rgb8(0xe0, 0x93, 0xcf));
        assert_eq!(LIGHT.token("fuchsia"), Color::from_rgb8(0xae, 0x53, 0x9d));
        assert_eq!(DARK.token("neon"), DARK.token("slate"));
    }
}
