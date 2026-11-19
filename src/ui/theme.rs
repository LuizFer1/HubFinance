//! Tema Grafite, escuro e claro.
//!
//! Toda cor de widget vem de `Tokens`, nao da paleta derivada do iced: o `Palette` do iced
//! gera rampas proprias que nao batem com as do design, e usa-las seria aproximar. O `Theme`
//! do iced so existe para o fundo da janela e para o que nao tem estilo explicito.
//!
//! Os hex sao copiados da paleta Grafite §4.1 (`docs/mobile/specs/2026-11-19-paleta-grafite-design.md`),
//! a unica tabela de cores do produto: app, landing e hub usam os mesmos valores. Nenhum hex
//! novo nasce aqui; o que nao esta na tabela sai de `mix` entre tokens (spec do dashboard §9).

use iced::widget::{button, container, scrollable};
use iced::{Background, Border, Color, Shadow, Theme, Vector};

use crate::config::ThemeMode;
use crate::dashboard::colors::{self, Mode, Rgb};

/// Cores de um tema. Os nomes seguem a paleta §4.1; `text` e o `fg` da paleta (o nome antigo
/// ficou porque e lido em quase cem lugares da UI).
///
/// Os degraus marcados com `allow(dead_code)` ainda nao tem tela: entram com o Dashboard Grafite
/// (H4a/H4b removem o `allow`). Estao aqui desde ja para a tabela inteira ser testada de uma vez
/// contra a paleta.
#[derive(Debug)]
pub struct Tokens {
    pub mode: Mode,
    pub bg: Color,
    pub surface: Color,
    /// Caixa dentro do card (inset). So o hub usa.
    #[allow(dead_code)] // H4a/H4b removem
    pub surface_2: Color,
    pub text: Color,
    divider: Color,
    pub accent: Color,
    #[allow(dead_code)] // H4a/H4b removem
    pub accent_100: Color,
    pub accent_200: Color,
    pub accent_300: Color,
    #[allow(dead_code)] // H4a/H4b removem
    pub accent_400: Color,
    #[allow(dead_code)] // H4a/H4b removem
    pub accent_500: Color,
    #[allow(dead_code)] // H4a/H4b removem
    pub accent_600: Color,
    pub accent_700: Color,
    #[allow(dead_code)] // H4a/H4b removem
    pub accent_800: Color,
    pub accent_900: Color,
    /// Texto/icone sobre preenchimento `accent`.
    #[allow(dead_code)] // H4a/H4b removem
    pub on_accent: Color,
    /// Atencao (meta estourando, "sem dados", codigo expirado). Nao e o `tag-amber` das
    /// categorias: o dourado do acento ficou quase igual a ele, entao atencao virou laranja.
    pub warning: Color,
    pub neutral_100: Color,
    #[allow(dead_code)] // H4a/H4b removem
    pub neutral_200: Color,
    #[allow(dead_code)] // H4a/H4b removem
    pub neutral_300: Color,
    #[allow(dead_code)] // H4a/H4b removem
    pub neutral_400: Color,
    #[allow(dead_code)] // H4a/H4b removem
    pub neutral_500: Color,
    pub neutral_600: Color,
    pub neutral_700: Color,
    pub neutral_800: Color,
    pub neutral_900: Color,
    pub income: Color,
    pub income_fg: Color,
    pub expense: Color,
    pub expense_fg: Color,
    pub close_hover: Color,
    /// Texto sobre `close_hover`.
    pub close_hover_text: Color,
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
    bg: hex(0x131313),
    surface: hex(0x1e1e1e),
    surface_2: hex(0x262626),
    text: hex(0xf2f2f2),
    // `color-mix(fg 10%)` sobre `surface`: o iced nao mistura, entao vai o hex resultante.
    divider: hex(0x333333),
    accent: hex(0xe2c15a),
    accent_100: hex(0xf9f3dd),
    accent_200: hex(0xf8e9b8),
    accent_300: hex(0xf0d98c),
    accent_400: hex(0xe8cd78),
    accent_500: hex(0xe2c15a),
    accent_600: hex(0xb29947),
    accent_700: hex(0x847335),
    accent_800: hex(0x5a4f24),
    accent_900: hex(0x332d14),
    on_accent: hex(0x131313),
    warning: hex(0xf2994a),
    neutral_100: hex(0xf2f2f2),
    neutral_200: hex(0xe8e8e8),
    neutral_300: hex(0xd4d4d4),
    neutral_400: hex(0xbdbdbd),
    neutral_500: hex(0x989898),
    neutral_600: hex(0x7a7a7a),
    neutral_700: hex(0x5e5e5e),
    neutral_800: hex(0x3d3d3d),
    neutral_900: hex(0x2a2a2a),
    income: rgb(colors::INCOME.dark),
    income_fg: rgb(colors::INCOME_FG.dark),
    expense: rgb(colors::EXPENSE.dark),
    expense_fg: rgb(colors::EXPENSE_FG.dark),
    // Vermelho do botao fechar do Windows: nao e da paleta, imita o sistema.
    close_hover: hex(0xc42b1c),
    close_hover_text: hex(0xf2f2f2),
    // Contornos das sombras como os da landing: neutral-700 (md) e neutral-500 (lg).
    shadow_md_ring: hex(0x5e5e5e),
    shadow_md_color: Color::from_rgba8(0, 0, 0, 0.55),
    shadow_lg_ring: hex(0x989898),
    shadow_lg_color: Color::from_rgba8(0, 0, 0, 0.65),
};

pub static LIGHT: Tokens = Tokens {
    mode: Mode::Light,
    bg: hex(0xf5f5f5),
    surface: hex(0xffffff),
    surface_2: hex(0xeeeeee),
    text: hex(0x202020),
    // `color-mix(fg 12%)` sobre `surface`.
    divider: hex(0xe4e4e4),
    accent: hex(0x876c00),
    accent_100: hex(0x392c01),
    accent_200: hex(0x4d4014),
    accent_300: hex(0x675412),
    accent_400: hex(0x866c10),
    accent_500: hex(0xa48402),
    accent_600: hex(0xc0a44f),
    accent_700: hex(0xdbc276),
    accent_800: hex(0xf0e0af),
    accent_900: hex(0xf8efd5),
    on_accent: hex(0xffffff),
    warning: hex(0x975201),
    neutral_100: hex(0x2b2b2b),
    neutral_200: hex(0x404040),
    neutral_300: hex(0x5a5a5a),
    neutral_400: hex(0x747474),
    neutral_500: hex(0x8f8f8f),
    neutral_600: hex(0xaeaeae),
    neutral_700: hex(0xcdcdcd),
    neutral_800: hex(0xe4e4e4),
    neutral_900: hex(0xeeeeee),
    income: rgb(colors::INCOME.light),
    income_fg: rgb(colors::INCOME_FG.light),
    expense: rgb(colors::EXPENSE.light),
    expense_fg: rgb(colors::EXPENSE_FG.light),
    close_hover: hex(0xc42b1c),
    // Texto claro sobre o vermelho nos dois temas (neutral-100 do escuro), como o Windows.
    close_hover_text: hex(0xf2f2f2),
    // Contornos: neutral-800 (md) e neutral-700 (lg) do claro; sombra no `bg` do escuro.
    shadow_md_ring: hex(0xe4e4e4),
    shadow_md_color: Color::from_rgba8(0x13, 0x13, 0x13, 0.12),
    shadow_lg_ring: hex(0xcdcdcd),
    shadow_lg_color: Color::from_rgba8(0x13, 0x13, 0x13, 0.20),
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
            Mode::Dark => "Grafite",
            Mode::Light => "Grafite Claro",
        };
        Theme::custom(
            name,
            iced::theme::Palette {
                background: self.bg,
                text: self.text,
                primary: self.accent,
                success: self.income,
                warning: self.warning,
                danger: self.expense,
            },
        )
    }

    /// Texto secundario: o design usa o texto a 75/70/65/60/55/50/45 % de opacidade.
    pub fn text_alpha(&self, a: f32) -> Color {
        alpha(self.text, a)
    }

    /// Divisoria opaca: o hex da paleta sobre `surface` (sobre `bg` fica um tom mais clara que
    /// a do app, diferenca que a paleta aceita para o hub).
    pub fn divider(&self) -> Color {
        self.divider
    }

    /// `surface` a `amount` sobre `bg`, arredondado em 8 bits. Arredondar e o que faz o painel
    /// do grafico dar #181818 exato, como no design; a mistura em float de `mix` daria 23,95.
    fn bg_toward_surface(&self, amount: f32) -> Color {
        let rgb8 = |c: Color| {
            let [r, g, b, _] = c.into_rgba8();
            Rgb(r, g, b)
        };
        rgb(colors::mix(rgb8(self.bg), rgb8(self.surface), amount))
    }

    /// Fundo da barra lateral: `surface` a 45 % sobre `bg`, como o `--side` do prototipo.
    pub fn side(&self) -> Color {
        self.bg_toward_surface(0.45)
    }

    /// Painel do grafico de ritmo (#181818 no escuro): a mesma regra da lateral (spec 9).
    #[allow(dead_code)] // H4b remove
    pub fn chart_panel(&self) -> Color {
        self.bg_toward_surface(0.45)
    }

    /// Brilho do gradiente de fundo (#1C1C1C no escuro).
    #[allow(dead_code)] // H4b remove
    pub fn glow(&self) -> Color {
        self.bg_toward_surface(0.8)
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
        let [r, g, b, _] = self.surface.into_rgba8();
        self.rgb(colors::mix(
            Rgb(r, g, b),
            colors::token_color(token, self.mode),
            amount,
        ))
    }

    /// Anel de 1 px do `shadow-md`, para o que e desenhado em canvas (tooltip das barras).
    pub fn shadow_md_ring(&self) -> Color {
        self.shadow_md_ring
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

/// Botao de categoria da barra de filtros: como o chip, mas com raio 8.
pub fn filter_button(
    t: &'static Tokens,
    active: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| button::Style {
        border: outline(if active { t.accent } else { t.divider() }, 8.0),
        ..chip(t, active)(theme, status)
    }
}

/// Item do menu de categoria: raio 6, escolhido com o acento a 10 %, hover texto a 6 %.
pub fn menu_item(
    t: &'static Tokens,
    selected: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| button::Style {
        background: if hovered(status) {
            bg(t.text_alpha(0.06))
        } else if selected {
            bg(alpha(t.accent, 0.10))
        } else {
            None
        },
        text_color: t.text,
        border: rounded(6.0),
        ..button::Style::default()
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

/// Barra de rolagem discreta: sem trilho, so o cursor em texto a 18 % (32 % sob o mouse). A
/// padrao do iced pinta o trilho com a paleta derivada, que nao e a do design.
pub fn scrollbar(t: &'static Tokens) -> impl Fn(&Theme, scrollable::Status) -> scrollable::Style {
    move |theme, status| {
        let active = matches!(
            status,
            scrollable::Status::Hovered {
                is_vertical_scrollbar_hovered: true,
                ..
            } | scrollable::Status::Dragged {
                is_vertical_scrollbar_dragged: true,
                ..
            }
        );
        let rail = scrollable::Rail {
            background: None,
            border: Border::default(),
            scroller: scrollable::Scroller {
                background: Background::Color(t.text_alpha(if active { 0.32 } else { 0.18 })),
                border: rounded(3.0),
            },
        };
        scrollable::Style {
            vertical_rail: rail,
            horizontal_rail: rail,
            ..scrollable::default(theme, status)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Paleta Grafite §4.1 (`docs/mobile/specs/2026-11-19-paleta-grafite-design.md`): a unica
    /// fonte dos hex. `(nome, token, escuro, claro)`.
    fn grafite(t: &Tokens) -> [(&'static str, Color); 26] {
        [
            ("bg", t.bg),
            ("surface", t.surface),
            ("surface_2", t.surface_2),
            ("fg", t.text),
            ("divider", t.divider()),
            ("accent", t.accent),
            ("accent_100", t.accent_100),
            ("accent_200", t.accent_200),
            ("accent_300", t.accent_300),
            ("accent_400", t.accent_400),
            ("accent_500", t.accent_500),
            ("accent_600", t.accent_600),
            ("accent_700", t.accent_700),
            ("accent_800", t.accent_800),
            ("accent_900", t.accent_900),
            ("on_accent", t.on_accent),
            ("warning", t.warning),
            ("neutral_100", t.neutral_100),
            ("neutral_200", t.neutral_200),
            ("neutral_300", t.neutral_300),
            ("neutral_400", t.neutral_400),
            ("neutral_500", t.neutral_500),
            ("neutral_600", t.neutral_600),
            ("neutral_700", t.neutral_700),
            ("neutral_800", t.neutral_800),
            ("neutral_900", t.neutral_900),
        ]
    }

    #[test]
    fn tokens_por_modo() {
        let expected: [(u32, u32); 26] = [
            (0x131313, 0xf5f5f5),
            (0x1e1e1e, 0xffffff),
            (0x262626, 0xeeeeee),
            (0xf2f2f2, 0x202020),
            (0x333333, 0xe4e4e4),
            (0xe2c15a, 0x876c00),
            (0xf9f3dd, 0x392c01),
            (0xf8e9b8, 0x4d4014),
            (0xf0d98c, 0x675412),
            (0xe8cd78, 0x866c10),
            (0xe2c15a, 0xa48402),
            (0xb29947, 0xc0a44f),
            (0x847335, 0xdbc276),
            (0x5a4f24, 0xf0e0af),
            (0x332d14, 0xf8efd5),
            (0x131313, 0xffffff),
            (0xf2994a, 0x975201),
            (0xf2f2f2, 0x2b2b2b),
            (0xe8e8e8, 0x404040),
            (0xd4d4d4, 0x5a5a5a),
            (0xbdbdbd, 0x747474),
            (0x989898, 0x8f8f8f),
            (0x7a7a7a, 0xaeaeae),
            (0x5e5e5e, 0xcdcdcd),
            (0x3d3d3d, 0xe4e4e4),
            (0x2a2a2a, 0xeeeeee),
        ];
        assert_eq!(Tokens::for_mode(ThemeMode::Dark).bg, hex(0x131313));
        assert_eq!(Tokens::for_mode(ThemeMode::Light).bg, hex(0xf5f5f5));
        for (((name, dark), (_, light)), (want_dark, want_light)) in grafite(&DARK)
            .into_iter()
            .zip(grafite(&LIGHT))
            .zip(expected)
        {
            assert_eq!(dark, hex(want_dark), "{name} escuro");
            assert_eq!(light, hex(want_light), "{name} claro");
        }
    }

    #[test]
    fn receita_despesa_e_tags_nao_mudam_com_a_paleta() {
        assert_eq!(DARK.income, Color::from_rgb8(0x75, 0xd5, 0xa0));
        assert_eq!(LIGHT.expense_fg, Color::from_rgb8(0xaf, 0x3c, 0x40));
        assert_eq!(DARK.token("amber"), hex(0xdeb866));
        assert_eq!(LIGHT.token("amber"), hex(0xb08505));
    }

    #[test]
    fn warning_nao_e_tag_amber() {
        assert_ne!(DARK.warning, DARK.token("amber"));
        assert_ne!(LIGHT.warning, LIGHT.token("amber"));
        assert_eq!(DARK.iced_theme().palette().warning, DARK.warning);
        assert_eq!(LIGHT.iced_theme().palette().warning, LIGHT.warning);
    }

    #[test]
    fn painel_do_grafico_brilho_e_lateral_saem_de_mix() {
        // Spec 9: #181818 e #1C1C1C exatos no escuro.
        assert_eq!(DARK.chart_panel(), hex(0x181818));
        assert_eq!(DARK.glow(), hex(0x1c1c1c));
        assert_eq!(DARK.side(), DARK.chart_panel());
        // No claro a mesma formula; o valor nao e fixado (0,45 cai no meio de um degrau).
        let rgb = |c: Color| {
            let [r, g, b, _] = c.into_rgba8();
            Rgb(r, g, b)
        };
        assert_eq!(
            rgb(LIGHT.chart_panel()),
            colors::mix(rgb(LIGHT.bg), rgb(LIGHT.surface), 0.45)
        );
        assert_eq!(
            rgb(LIGHT.glow()),
            colors::mix(rgb(LIGHT.bg), rgb(LIGHT.surface), 0.8)
        );
        assert_eq!(LIGHT.side(), LIGHT.chart_panel());
    }

    #[test]
    fn divisor_e_opaco_e_texto_secundario_e_alpha_do_texto() {
        // Paleta §4.1: o hub, sem `color-mix`, usa o hex da divisoria sobre `surface`.
        assert_eq!(DARK.divider().a, 1.0);
        assert_eq!(LIGHT.divider().a, 1.0);
        assert!((DARK.text_alpha(0.55).a - 0.55).abs() < f32::EPSILON);
    }

    /// Luminancia relativa WCAG 2 (sRGB), como a tabela 4.3 da paleta.
    fn luminance(c: Color) -> f64 {
        let [r, g, b, _] = c.into_rgba8();
        let lin = |v: u8| {
            let s = f64::from(v) / 255.0;
            if s <= 0.04045 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
    }

    fn contrast(a: Color, b: Color) -> f64 {
        let (la, lb) = (luminance(a), luminance(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    #[test]
    fn contraste_reproduz_a_tabela_da_paleta() {
        let near = |got: f64, want: f64| (got - want).abs() < 0.01;
        assert!(near(contrast(DARK.text, DARK.bg), 16.60));
        assert!(near(contrast(LIGHT.text, LIGHT.bg), 14.94));
        assert!(near(contrast(DARK.warning, DARK.surface), 7.49));
        assert!(near(contrast(LIGHT.warning, LIGHT.surface), 5.96));
        assert!(near(contrast(LIGHT.accent, LIGHT.bg), 4.61));
        assert!(near(contrast(DARK.neutral_500, DARK.surface), 5.78));
    }

    #[test]
    fn warning_accent_e_texto_secundario_passam_4_5_nos_dois_temas() {
        for t in [&DARK, &LIGHT] {
            for (fundo, cor_fundo) in [("surface", t.surface), ("bg", t.bg)] {
                for (nome, cor) in [
                    ("warning", t.warning),
                    ("accent", t.accent),
                    ("accent_300", t.accent_300),
                ] {
                    let c = contrast(cor, cor_fundo);
                    assert!(c >= 4.5, "{nome}/{fundo} {:?}: {c:.2}", t.mode);
                }
            }
            assert!(contrast(t.on_accent, t.accent) >= 4.5, "{:?}", t.mode);
        }
        // Rotulos e legendas do design (#A6A6A6, #9C9C9C) -> `neutral_500` (spec 9).
        for fundo in [DARK.surface, DARK.surface_2, DARK.bg] {
            assert!(contrast(DARK.neutral_500, fundo) >= 4.5);
        }
    }

    /// Lacuna da paleta, registrada e nao corrigida aqui (a H1 nao inventa hex): no claro,
    /// `neutral_500` (rampa invertida) e o texto a 60 % ficam abaixo de 4.5:1. A paleta §4.3
    /// ja mede 3.23 e 4.35; quem desenhar texto secundario no claro precisa de outro degrau.
    #[test]
    fn texto_secundario_no_claro_fica_abaixo_do_piso() {
        let c500 = contrast(LIGHT.neutral_500, LIGHT.surface);
        assert!((c500 - 3.23).abs() < 0.01, "{c500:.2}");
        let c60 = contrast(mix(LIGHT.surface, LIGHT.text, 0.60), LIGHT.surface);
        assert!(c60 < 4.5, "{c60:.2}");
    }

    #[test]
    fn token_segue_o_modo() {
        assert_eq!(DARK.token("fuchsia"), Color::from_rgb8(0xe0, 0x93, 0xcf));
        assert_eq!(LIGHT.token("fuchsia"), Color::from_rgb8(0xae, 0x53, 0x9d));
        assert_eq!(DARK.token("neon"), DARK.token("slate"));
    }
}
