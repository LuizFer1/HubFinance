//! Tema Grafite, escuro e claro.
//!
//! Toda cor de widget vem de `Tokens`, nao da paleta derivada do iced: o `Palette` do iced
//! gera rampas proprias que nao batem com as do design, e usa-las seria aproximar. O `Theme`
//! do iced so existe para o fundo da janela e para o que nao tem estilo explicito.
//!
//! Os hex sao copiados da paleta Grafite §4.1 (`docs/mobile/specs/2026-11-19-paleta-grafite-design.md`),
//! a unica tabela de cores do produto: app, landing e hub usam os mesmos valores. Nenhum hex
//! novo nasce aqui; o que nao esta na tabela sai de `mix` entre tokens (spec do dashboard §9).

use iced::gradient::{self, Gradient};
use iced::overlay::menu;
// `button::{self}` traz so o modulo: a funcao `button` daqui e o estilo unico de botao.
use iced::widget::button::{self};
use iced::widget::{container, pick_list, scrollable, text_input};
use iced::{Background, Border, Color, Degrees, Shadow, Theme, Vector};

use crate::config::ThemeMode;
use crate::dashboard::colors::{self, Mode, Rgb};

/// Cores de um tema. Os nomes seguem a paleta §4.1; `text` e o `fg` da paleta (o nome antigo
/// ficou porque e lido em quase cem lugares da UI).
///
/// A tabela §4.1 entra inteira: `accent-100/500/600/800`, `on-accent` e `neutral-200`, que
/// sairam na H4b por falta de uso, voltaram na Fase 7 (toggle, link pressionado, variantes de
/// botao). Degrau novo na paleta pede a linha no teste da tabela, senao o hex diverge calado.
#[derive(Debug)]
pub struct Tokens {
    pub mode: Mode,
    pub bg: Color,
    pub surface: Color,
    /// Caixa dentro do card (inset). So o hub usa.
    pub surface_2: Color,
    pub text: Color,
    /// Texto secundario (rotulos, legendas, contagens): paleta §4.4. Nao e degrau da rampa de
    /// neutros porque no claro o `neutral_500` fica em 3,2:1 sobre `surface`.
    pub text_muted: Color,
    divider: Color,
    pub accent: Color,
    /// Texto/icone sobre preenchimento `accent`.
    #[allow(dead_code, reason = "degrau da paleta para a Fase 7")]
    pub on_accent: Color,
    pub accent_100: Color,
    pub accent_200: Color,
    pub accent_300: Color,
    pub accent_400: Color,
    /// No escuro e o proprio `accent`; no claro e um degrau mais claro que ele.
    #[allow(dead_code, reason = "degrau da paleta para a Fase 7")]
    pub accent_500: Color,
    #[allow(dead_code, reason = "degrau da paleta para a Fase 7")]
    pub accent_600: Color,
    pub accent_700: Color,
    /// Trilho do toggle ligado (paleta §4.1).
    #[allow(dead_code, reason = "degrau da paleta para a Fase 7")]
    pub accent_800: Color,
    pub accent_900: Color,
    /// Atencao (meta estourando, "sem dados", codigo expirado). Nao e o `tag-amber` das
    /// categorias: o dourado do acento ficou quase igual a ele, entao atencao virou laranja.
    pub warning: Color,
    pub neutral_100: Color,
    #[allow(dead_code, reason = "degrau da paleta para a Fase 7")]
    pub neutral_200: Color,
    pub neutral_300: Color,
    pub neutral_400: Color,
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

// ---- escalas (relatorio Grafite · Mesa 7.4) ----
//
// Uma regua para todas as telas: hoje convivem tres geracoes de UI (14 contra 16 entre cards,
// padding de 18, 20 e 24, oito alturas de botao). As telas migram para estas constantes uma a
// uma nas proximas tarefas; ate la, valor fora da escala numa tela e divida, nao regra nova.

/// Espacamento. Os valores 3, 5, 7, 10, 14 e 28 que as telas ainda usam arredondam para o
/// vizinho daqui.
pub mod space {
    pub const XS: f32 = 4.0;
    pub const S: f32 = 8.0;
    pub const M: f32 = 12.0;
    pub const L: f32 = 16.0;
    pub const XL: f32 = 20.0;
    pub const XXL: f32 = 24.0;
    #[cfg_attr(not(test), expect(dead_code, reason = "margem da pagina, issue 05"))]
    pub const XXXL: f32 = 32.0;
    #[cfg_attr(not(test), expect(dead_code, reason = "sem uso ainda"))]
    pub const HUGE: f32 = 40.0;
    #[cfg_attr(not(test), expect(dead_code, reason = "sem uso ainda"))]
    pub const GIANT: f32 = 56.0;
    /// Entre cards de uma tela.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "espaco entre blocos, issue de cada tela")
    )]
    pub const BETWEEN_CARDS: f32 = L;
    /// Padding de card.
    pub const CARD: f32 = XL;
    /// Padding de dialogo.
    pub const DIALOG: f32 = XXL;
    /// Margem da pagina.
    #[cfg_attr(not(test), expect(dead_code, reason = "margem da pagina, issue 05"))]
    pub const PAGE: f32 = XXXL;
}

/// Raio. Saem 5, 10 e 14.
pub mod radius {
    /// Barra de progresso.
    pub const BAR: f32 = 4.0;
    /// Segmento, item de menu, caixa interna, tile, botao compacto.
    pub const INSET: f32 = 6.0;
    /// Botao, input, linha.
    pub const CONTROL: f32 = 8.0;
    pub const CARD: f32 = 8.0;
    /// Dialogo e quadro do QR.
    pub const DIALOG: f32 = 12.0;
    pub const PILL: f32 = 999.0;
}

/// Alturas de controle e linha, e espessuras de borda.
pub mod size {
    /// Controle compacto, dentro de linha.
    pub const CTRL_S: f32 = 28.0;
    /// Controle padrao.
    pub const CTRL_M: f32 = 36.0;
    /// Cabecalho e navegacao.
    pub const CTRL_L: f32 = 44.0;
    /// Linha simples.
    #[cfg_attr(not(test), expect(dead_code, reason = "linha simples sem tela ainda"))]
    pub const ROW: f32 = 44.0;
    /// Linha de duas linhas.
    pub const ROW_RICH: f32 = 52.0;
    /// Toda borda tem 1 px.
    pub const BORDER: f32 = 1.0;
    /// Menos o foco do input, que tem 2 px no acento.
    pub const FOCUS_BORDER: f32 = 2.0;
}

/// Escala de tipo, derivada do Dashboard do dono sem mudar nada nele: as outras telas e que se
/// conformam. Light so em numero de 24 ou mais; SemiBold so em inicial de avatar e numero de
/// passo. Saem 10, 20 e 32.
pub mod type_scale {
    /// Tag, eixo, escala (Regular ou Medium).
    pub const MICRO: f32 = 11.0;
    /// Meta, legenda.
    pub const META: f32 = 12.0;
    /// Rotulo de bloco (muted), botao, celula secundaria.
    pub const LABEL: f32 = 13.0;
    /// Corpo, linha de lista.
    pub const BODY: f32 = 14.0;
    /// Frase de apoio dentro do card.
    pub const SUPPORT: f32 = 15.0;
    /// Titulo de secao, fora do card (Medium).
    pub const SECTION: f32 = 16.0;
    /// Valor secundario ("Receitas", "Guardado").
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "valor secundario, issue de cada tela")
    )]
    pub const VALUE_S: f32 = 17.0;
    /// Frase de previsao; titulo de dialogo e de estado vazio.
    pub const LEAD: f32 = 18.0;
    /// Valor medio (Light ou Regular).
    pub const VALUE_M: f32 = 24.0;
    /// Titulo da pagina.
    #[cfg_attr(not(test), expect(dead_code, reason = "titulo da pagina, issue 05"))]
    pub const PAGE: f32 = 30.0;
    /// Valor grande do bloco (Light).
    pub const VALUE_L: f32 = 36.0;
    /// Valor heroi, um por tela (Light).
    pub const VALUE_XL: f32 = 44.0;
}

pub static DARK: Tokens = Tokens {
    mode: Mode::Dark,
    bg: hex(0x131313),
    surface: hex(0x1e1e1e),
    surface_2: hex(0x262626),
    text: hex(0xf2f2f2),
    text_muted: hex(0xa6a6a6),
    // `color-mix(fg 10%)` sobre `surface`: o iced nao mistura, entao vai o hex resultante.
    divider: hex(0x333333),
    accent: hex(0xe2c15a),
    on_accent: hex(0x131313),
    accent_100: hex(0xf9f3dd),
    accent_200: hex(0xf8e9b8),
    accent_300: hex(0xf0d98c),
    accent_400: hex(0xe8cd78),
    accent_500: hex(0xe2c15a),
    accent_600: hex(0xb29947),
    accent_700: hex(0x847335),
    accent_800: hex(0x5a4f24),
    accent_900: hex(0x332d14),
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
    text_muted: hex(0x6b6b6b),
    // `color-mix(fg 12%)` sobre `surface`.
    divider: hex(0xe4e4e4),
    accent: hex(0x876c00),
    on_accent: hex(0xffffff),
    accent_100: hex(0x392c01),
    accent_200: hex(0x4d4014),
    accent_300: hex(0x675412),
    accent_400: hex(0x866c10),
    accent_500: hex(0xa48402),
    accent_600: hex(0xc0a44f),
    accent_700: hex(0xdbc276),
    accent_800: hex(0xf0e0af),
    accent_900: hex(0xf8efd5),
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
    /// Tokens da preferencia `mode`; `system_dark` so pesa com `ThemeMode::System`.
    pub fn for_mode(mode: ThemeMode, system_dark: bool) -> &'static Tokens {
        if mode.is_dark(system_dark) {
            &DARK
        } else {
            &LIGHT
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

    /// Texto de apoio forte (nav inativa, setas, olho): o #BDBDBD do design, `neutral_400` no
    /// escuro. No claro a rampa e invertida e o `neutral_400` (#747474) da 4,2:1 sobre `bg`;
    /// vai o degrau acima (`neutral_300`), que fica mais escuro que `text_muted` como no escuro
    /// fica mais claro: a hierarquia (fg > apoio > muted) e a mesma nos dois.
    pub fn text_soft(&self) -> Color {
        match self.mode {
            Mode::Dark => self.neutral_400,
            Mode::Light => self.neutral_300,
        }
    }

    /// Ponto de estado e icone apagados (hub desligado, aparelho fora, "Sem autor"): sem texto,
    /// pedem 3:1 sobre o fundo. O `neutral_600` serve no escuro; no claro a rampa invertida o
    /// deixa em ~2:1 sobre `bg`, e o degrau que passa em todo fundo e o `neutral_400`.
    pub fn idle(&self) -> Color {
        match self.mode {
            Mode::Dark => self.neutral_600,
            Mode::Light => self.neutral_400,
        }
    }

    /// Cursor da rolagem em repouso: o degrau abaixo de `idle()` (2,9:1 no escuro, 2,0:1 no
    /// claro sobre `bg`). Fica abaixo de 3 de proposito: parado ele nao e controle a achar, e
    /// mais forte disputaria com o conteudo; sob o mouse vira `idle()`, que passa 3.
    fn scroller_rest(&self) -> Color {
        match self.mode {
            Mode::Dark => self.neutral_700,
            Mode::Light => self.neutral_600,
        }
    }

    /// Avatar de quem nao tem cor de perfil: (preenchimento, inicial). O #C4C4C4 do design e o
    /// `neutral_300` com a inicial em `bg` no escuro; no claro o cinza claro equivalente e o
    /// `neutral_700`, com a inicial em `fg` (o `neutral_300` com `bg` daria 2,2:1). Escolha da
    /// H4b para "Quem lançou", valendo para toda tela.
    pub fn avatar_neutral(&self) -> (Color, Color) {
        match self.mode {
            Mode::Dark => (self.neutral_300, self.bg),
            Mode::Light => (self.neutral_700, self.text),
        }
    }

    /// Borda de 1 px de todo card (B2): `surface` sobre `bg` fica em 1,1:1 e o card some perto do
    /// brilho do canto. Nao chega a 3:1 (1,2:1 de cada lado), e ganho de percepcao, nao de
    /// contraste; no claro e o mesmo cinza da divisoria.
    pub fn card_border(&self) -> Color {
        match self.mode {
            Mode::Dark => self.neutral_900,
            Mode::Light => self.neutral_800,
        }
    }

    /// Contorno de controle (input, toggle desligado, checkbox): `idle()`, que passa 3:1 sobre
    /// `bg` e `surface`. A divisoria antiga dava 1,3:1 e o campo nao se via (B1).
    pub fn border_control(&self) -> Color {
        self.idle()
    }

    /// Camada neutra de hover (4 %) e pressionado (8 %), em `fg` com alfa sobre o pai. Linha
    /// interativa so mora em `surface`: sobre `surface_2` o hover a 4 % e a propria caixa e o
    /// `text_muted` do claro cai abaixo de 4,5 pressionado (relatorio 7.2).
    pub fn state(&self, pressed: bool) -> Color {
        self.text_alpha(if pressed { 0.08 } else { 0.04 })
    }

    /// Tinta de 10 % de um tom, opaca sobre o pai (banner, tile de severidade).
    pub fn tint(&self, tone: Tone, parent: Color) -> Color {
        let color = match tone {
            Tone::Accent => self.accent,
            Tone::Income => self.income,
            Tone::Expense => self.expense,
            Tone::Warning => self.warning,
        };
        mix(parent, color, 0.10)
    }

    /// Todo texto no acento. O `accent` cru do claro cai a 4,3:1 sobre `surface_2` e sobre linha
    /// pressionada; o `accent_300` passa em todo fundo dos dois temas.
    pub fn text_accent(&self) -> Color {
        self.accent_300
    }

    /// Texto e icone desabilitados: `idle()`. Fica abaixo de 4,5 de proposito (WCAG isenta
    /// controle inativo), e e o que separa "desabilitado" de "secundario".
    pub fn text_disabled(&self) -> Color {
        self.idle()
    }

    /// Brilho dourado do primario do Nocturne (relatorio 7.2). No claro o alfa e menor porque a
    /// sombra dourada sobre branco suja o fundo.
    ///
    /// **Nao e aplicado ao botao.** No renderizador de software (tiny-skia, o que o iced usa sem
    /// GPU, em VM ou RDP) a sombra e pintada fora de `quad.bounds` sem respeitar o recorte
    /// (`iced_tiny_skia-0.14.1/src/engine.rs`, desenho do quad), e o dano de `layer.rs::damage`
    /// so cobre o quad: o halo fica acumulado na tela a cada quadro. No wgpu a sombra e cortada
    /// a 24 px na borda do `scrollable`. Fica aqui para quem desenhar o brilho por outro caminho.
    #[cfg_attr(not(test), expect(dead_code, reason = "sem uso no botao, ver acima"))]
    pub fn accent_glow(&self) -> Shadow {
        Shadow {
            color: alpha(
                self.accent,
                match self.mode {
                    Mode::Dark => 0.28,
                    Mode::Light => 0.16,
                },
            ),
            offset: Vector::ZERO,
            blur_radius: 24.0,
        }
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
    pub fn chart_panel(&self) -> Color {
        self.bg_toward_surface(0.45)
    }

    /// Brilho do gradiente de fundo (#1C1C1C no escuro).
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
                radius: radius::DIALOG.into(),
            },
            Shadow {
                color: self.shadow_lg_color,
                offset: Vector::new(0.0, 16.0),
                blur_radius: 40.0,
            },
        )
    }
}

/// Tom de uma tinta (`Tokens::tint`). Nao e o `dashboard::grafite::Tone`: aquele e o tom
/// semantico de um texto do modelo, sem acento; este so escolhe a cor da tinta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "sem banner de acento nas telas ainda")
    )]
    Accent,
    Income,
    Expense,
    Warning,
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

/// Camada neutra por estado: nada em repouso, `state(false)` sob o mouse e `state(true)`
/// pressionado. E a regra dos estilos de item (lateral, segmento, icone, menu) e o que separa o
/// clique do hover que ja estava ali.
fn neutral_layer(t: &Tokens, status: button::Status) -> Option<Background> {
    match status {
        button::Status::Hovered => bg(t.state(false)),
        button::Status::Pressed => bg(t.state(true)),
        button::Status::Active | button::Status::Disabled => None,
    }
}

/// Cara de botao desabilitado, igual para todos: texto `idle()`, sem tinta, e o contorno (se o
/// estilo tem um) no divisor. Sem isto o iced entrega `Disabled` com o estilo de repouso, e
/// "Gerar codigo" com o hub parado ou "Salvar" com o campo vazio pareciam clicaveis. Estilos
/// sem contorno (link, fantasma, item da lateral) continuam sem: so o texto apaga.
fn disabled(t: &Tokens, style: button::Style) -> button::Style {
    let border = if style.border.width > 0.0 {
        Border {
            color: t.divider(),
            ..style.border
        }
    } else {
        style.border
    };
    button::Style {
        background: None,
        text_color: t.text_disabled(),
        border,
        shadow: Shadow::default(),
        ..style
    }
}

/// Envolve um estilo de botao para tratar `Disabled` com `disabled`.
fn disabled_aware(
    t: &'static Tokens,
    style: impl Fn(&Theme, button::Status) -> button::Style,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let base = style(theme, status);
        if status == button::Status::Disabled {
            disabled(t, base)
        } else {
            base
        }
    }
}

// ---- containers ----

/// Card: superficie solida, raio 8 e borda de 1 px `card_border` (nivel 0 de elevacao: borda,
/// sem sombra).
pub fn card(t: &'static Tokens) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: bg(t.surface),
        border: outline(t.card_border(), radius::CARD),
        text_color: Some(t.text),
        ..container::Style::default()
    }
}

/// Caixa interna (dentro de card ou dialogo): `surface_2`, raio 6, sem borda. Card dentro de
/// card dava borda dupla e nao separava os planos; a caixa e o terceiro plano da regua.
pub fn inset(t: &'static Tokens) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: bg(t.surface_2),
        border: rounded(radius::INSET),
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

/// Fundo da pagina (lateral + conteudo): o brilho `radial-gradient(1100px 620px at 0 0)` do
/// design aproximado por um gradiente linear a 135 graus, de `glow` no canto de cima a esquerda
/// para `bg` a 55 % (o iced so tem `Linear`; spec 8 e 7.9). A diferenca e de poucos niveis de
/// cinza, e o mesmo calculo vale no claro.
pub fn page(t: &'static Tokens) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(Background::Gradient(Gradient::Linear(
            gradient::Linear::new(Degrees(135.0))
                .add_stop(0.0, t.glow())
                .add_stop(0.55, t.bg),
        ))),
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

/// Linha de lista sob o mouse: a camada `state` de 4 % (so mora em `surface`, ver `state`).
/// Raio 8, ou 0 na tabela que encosta na borda do card.
pub fn list_row(
    t: &'static Tokens,
    hovered: bool,
    radius: f32,
) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: hovered.then(|| Background::Color(t.state(false))),
        border: rounded(radius),
        ..container::Style::default()
    }
}

/// Fundo do dialogo: preto a 55 % sobre a janela inteira.
pub fn scrim() -> impl Fn(&Theme) -> container::Style {
    |_| container::Style {
        background: bg(Color::from_rgba8(0, 0, 0, 0.55)),
        ..container::Style::default()
    }
}

/// Atraso dos tooltips: sem ele a dica pisca sob o mouse que so atravessa o icone a caminho de
/// outro lugar.
pub const TOOLTIP_DELAY: std::time::Duration = std::time::Duration::from_millis(400);

// ---- botoes ----

/// Item de navegacao Grafite: ativo em `surface` com texto `fg`; inativo sem fundo, texto de
/// apoio. O design nao desenha hover; o inativo ganha a camada `state` (4 % sob o mouse, 8 %
/// pressionado) para o clique ter resposta.
pub fn nav_item(
    t: &'static Tokens,
    active: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    disabled_aware(t, move |_, status| {
        let background = if active {
            bg(t.surface)
        } else {
            neutral_layer(t, status)
        };
        button::Style {
            background,
            text_color: if active { t.text } else { t.text_soft() },
            border: rounded(8.0),
            ..button::Style::default()
        }
    })
}

/// Opcao do segmentado Grafite (tema, Pessoal | Comercial): raio 6, ativo no #333333 do
/// design (a divisoria opaca) com texto `fg`, inativo transparente com texto de apoio e a
/// camada `state` sob o mouse e pressionado.
pub fn segment(
    t: &'static Tokens,
    active: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    disabled_aware(t, move |_, status| button::Style {
        background: if active {
            bg(t.divider())
        } else {
            neutral_layer(t, status)
        },
        text_color: if active { t.text } else { t.text_soft() },
        border: rounded(6.0),
        ..button::Style::default()
    })
}

/// Botao so de icone (olho do cabecalho): raio 8, transparente; camada `state` sob o mouse e
/// pressionado.
pub fn icon_button(t: &'static Tokens) -> impl Fn(&Theme, button::Status) -> button::Style {
    disabled_aware(t, move |_, status| button::Style {
        background: neutral_layer(t, status),
        text_color: t.text_soft(),
        border: rounded(8.0),
        ..button::Style::default()
    })
}

/// Opcao de segmentado (tipo): raio 6, ativo `accent_900` + `accent_200`, inativo no texto de
/// apoio (o texto a 60 % fica abaixo de 4,5:1 no claro) com a camada `state`.
pub fn seg_option(
    t: &'static Tokens,
    active: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    disabled_aware(t, move |_, status| button::Style {
        background: if active {
            bg(t.accent_900)
        } else {
            neutral_layer(t, status)
        },
        text_color: if active { t.accent_200 } else { t.text_soft() },
        border: rounded(6.0),
        ..button::Style::default()
    })
}

/// Papel do botao (relatorio 7.5 e 9). Um so estilo por papel, em vez de uma funcao por tela.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonKind {
    /// Acao principal do bloco: contorno `accent`, tinta de 8/16/24 % (repouso, mouse,
    /// pressionado) e texto `text_accent`. Nunca preenchido (heranca do Nocturne), e sem o
    /// `accent_glow`: ver o motivo nele.
    Primary,
    /// Contorno no divisor, texto `fg`, camada `state` de 4/8 % ("Cancelar", "Fechar").
    Secondary,
    /// Sem contorno, texto `text_accent`, tinta do acento a 12/20 % ("Copiar", "Desfazer").
    Ghost,
    /// So texto: `accent_300`, `accent_200` sob o mouse, `accent_100` pressionado ("Ver todos").
    Link,
    /// Perigo em contorno: texto `expense_fg`, contorno da despesa a 45 %, tinta a 12 %.
    Danger,
    /// Perigo preenchido, a 14/20 %, so dentro de card ou dialogo (sobre `bg` o hover do claro
    /// cai a 4,25:1).
    DangerFilled,
}

/// Tamanho do botao: 28 (compacto, dentro de linha), 36 (padrao) e 44 (cabecalho). O
/// `button::Style` do iced nao tem altura, padding nem fonte; o estilo so usa o raio, e
/// `widgets::action_button` le `height`, `padding_x` e `text_size` daqui.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonSize {
    Compact,
    Regular,
    Large,
}

impl ButtonSize {
    pub const fn height(self) -> f32 {
        match self {
            ButtonSize::Compact => size::CTRL_S,
            ButtonSize::Regular => size::CTRL_M,
            ButtonSize::Large => size::CTRL_L,
        }
    }

    pub const fn padding_x(self) -> f32 {
        match self {
            ButtonSize::Compact => space::M,
            ButtonSize::Regular => space::L,
            ButtonSize::Large => space::XL,
        }
    }

    pub const fn text_size(self) -> f32 {
        match self {
            ButtonSize::Compact | ButtonSize::Regular => type_scale::LABEL,
            ButtonSize::Large => type_scale::BODY,
        }
    }

    /// O compacto usa o raio das pecas pequenas (6); os outros, o de controle (8).
    ///
    /// Desvio intencional da secao 7.4, que pede raio 8 para todo botao: o fantasma ("Copiar",
    /// "Desfazer") ja tinha raio 6, e o fantasma usa o compacto para nao mudar de cara.
    pub const fn radius(self) -> f32 {
        match self {
            ButtonSize::Compact => radius::INSET,
            ButtonSize::Regular | ButtonSize::Large => radius::CONTROL,
        }
    }
}

/// Repouso, mouse ou pressionado. `Pressed` tem cara propria: sem isso o clique nao da resposta
/// alem do hover que ja estava ali.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Press {
    Rest,
    Hover,
    Down,
}

impl Press {
    fn of(status: button::Status) -> Press {
        match status {
            button::Status::Hovered => Press::Hover,
            button::Status::Pressed => Press::Down,
            button::Status::Active | button::Status::Disabled => Press::Rest,
        }
    }

    fn pick(self, rest: f32, hover: f32, down: f32) -> f32 {
        match self {
            Press::Rest => rest,
            Press::Hover => hover,
            Press::Down => down,
        }
    }
}

/// Estilo unico de botao. `Disabled` apaga como todo botao (`disabled`); `Pressed` e `Hovered`
/// sao estados diferentes. O perigo pressionado firma o contorno em vez de subir a tinta: a
/// 18 % ou mais o `expense_fg` do claro cai abaixo de 4,5 sobre `bg`.
pub fn button(
    t: &'static Tokens,
    kind: ButtonKind,
    size: ButtonSize,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    let r = size.radius();
    disabled_aware(t, move |_, status| {
        let press = Press::of(status);
        let danger_border = |rest: f32| {
            if press == Press::Down {
                t.expense
            } else {
                alpha(t.expense, rest)
            }
        };
        match kind {
            ButtonKind::Primary => button::Style {
                background: bg(alpha(t.accent, press.pick(0.08, 0.16, 0.24))),
                text_color: t.text_accent(),
                border: outline(t.accent, r),
                ..button::Style::default()
            },
            ButtonKind::Secondary => button::Style {
                background: match press {
                    Press::Rest => None,
                    Press::Hover => bg(t.state(false)),
                    Press::Down => bg(t.state(true)),
                },
                text_color: t.text,
                border: outline(t.divider(), r),
                ..button::Style::default()
            },
            ButtonKind::Ghost => button::Style {
                background: (press != Press::Rest)
                    .then(|| Background::Color(alpha(t.accent, press.pick(0.0, 0.12, 0.20)))),
                text_color: t.text_accent(),
                border: rounded(r),
                ..button::Style::default()
            },
            ButtonKind::Link => button::Style {
                background: None,
                text_color: match press {
                    Press::Rest => t.text_accent(),
                    Press::Hover => t.accent_200,
                    Press::Down => t.accent_100,
                },
                border: rounded(r),
                ..button::Style::default()
            },
            ButtonKind::Danger => button::Style {
                background: (press != Press::Rest)
                    .then(|| Background::Color(alpha(t.expense, 0.12))),
                text_color: t.expense_fg,
                border: outline(danger_border(0.45), r),
                ..button::Style::default()
            },
            ButtonKind::DangerFilled => button::Style {
                background: bg(alpha(t.expense, press.pick(0.14, 0.20, 0.20))),
                text_color: t.expense_fg,
                border: outline(danger_border(0.55), r),
                ..button::Style::default()
            },
        }
    })
}

/// Botoes da barra de titulo: icone no texto de apoio, texto a 8 % sob o mouse e 12 %
/// pressionado; o fechar fica no vermelho do Windows e escurece 15 % pressionado (mais escuro, o
/// texto claro so ganha contraste).
pub fn titlebar_button(
    t: &'static Tokens,
    close: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    disabled_aware(t, move |_, status| {
        let press = Press::of(status);
        let hover = press != Press::Rest;
        button::Style {
            background: match (press, close) {
                (Press::Rest, _) => None,
                (Press::Hover, true) => bg(t.close_hover),
                (Press::Down, true) => bg(mix(t.close_hover, Color::BLACK, 0.15)),
                (Press::Hover, false) => bg(t.text_alpha(0.08)),
                (Press::Down, false) => bg(t.text_alpha(0.12)),
            },
            text_color: if hover && close {
                t.close_hover_text
            } else {
                t.text_soft()
            },
            border: rounded(0.0),
            ..button::Style::default()
        }
    })
}

/// Chip pilula (autor): ativo borda `accent` + `accent_900` + `accent_200`; inativo superficie
/// com borda no divisor e o texto pre-misturado a 5 % sob o mouse e 10 % pressionado.
pub fn chip(t: &'static Tokens, active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    disabled_aware(t, move |_, status| {
        let (background, border, text_color) = if active {
            (t.accent_900, t.accent, t.accent_200)
        } else {
            let amount = Press::of(status).pick(0.0, 0.05, 0.10);
            (mix(t.surface, t.text, amount), t.divider(), t.text)
        };
        button::Style {
            background: bg(background),
            text_color,
            border: outline(border, 999.0),
            ..button::Style::default()
        }
    })
}

/// Botao de categoria da barra de filtros: como o chip, mas com raio 8.
pub fn filter_button(
    t: &'static Tokens,
    active: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    disabled_aware(t, move |theme, status| button::Style {
        border: outline(if active { t.accent } else { t.divider() }, 8.0),
        ..chip(t, active)(theme, status)
    })
}

/// Item do menu de categoria: raio 6, escolhido com o acento a 10 %, camada `state` sob o
/// mouse e pressionado (por cima da escolha: o clique responde ate no item ja escolhido).
pub fn menu_item(
    t: &'static Tokens,
    selected: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    disabled_aware(t, move |_, status| button::Style {
        background: match neutral_layer(t, status) {
            Some(layer) => Some(layer),
            None if selected => bg(alpha(t.accent, 0.10)),
            None => None,
        },
        text_color: t.text,
        border: rounded(6.0),
        ..button::Style::default()
    })
}

/// Botao sem fundo nem hover (setas do seletor de mes, area clicavel generica).
pub fn plain(color: Color) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, _| button::Style {
        background: None,
        text_color: color,
        ..button::Style::default()
    }
}

/// Lista de modelos (`pick_list`): a mesma cara do campo de texto (fundo da janela, contorno
/// `border_control`, `text_soft` sob o mouse e acento de 2 px aberta), para nao ser a unica peca
/// com as cores derivadas do iced.
pub fn pick(t: &'static Tokens) -> impl Fn(&Theme, pick_list::Status) -> pick_list::Style {
    move |_, status| {
        let (color, width) = match status {
            pick_list::Status::Opened { .. } => (t.accent, size::FOCUS_BORDER),
            pick_list::Status::Hovered => (t.text_soft(), size::BORDER),
            pick_list::Status::Active => (t.border_control(), size::BORDER),
        };
        pick_list::Style {
            text_color: t.text,
            placeholder_color: t.text_muted,
            handle_color: t.text_soft(),
            background: Background::Color(t.bg),
            border: Border {
                color,
                width,
                radius: radius::CONTROL.into(),
            },
        }
    }
}

/// Menu aberto do `pick_list`: o `menu` do hub (superficie, sombra media) e o escolhido com o
/// acento a 10 %, como o item do menu de categoria.
pub fn pick_menu(t: &'static Tokens) -> impl Fn(&Theme) -> menu::Style {
    move |_| {
        let (border, shadow) = t.shadow_md();
        menu::Style {
            background: Background::Color(t.surface),
            border,
            text_color: t.text,
            selected_text_color: t.text,
            selected_background: Background::Color(mix(t.surface, t.accent, 0.10)),
            shadow,
        }
    }
}

/// Campo de texto: fundo da janela com contorno `border_control` (3,9:1 ou mais; o divisor
/// antigo dava 1,3:1 e o campo sumia, B1), `text_soft` sob o mouse e acento de 2 px com foco.
/// Desabilitado, o contorno volta ao divisor e o texto esmaece, como o botao desabilitado.
pub fn input(t: &'static Tokens) -> impl Fn(&Theme, text_input::Status) -> text_input::Style {
    move |_, status| {
        let (color, width) = match status {
            text_input::Status::Focused { .. } => (t.accent, size::FOCUS_BORDER),
            text_input::Status::Hovered => (t.text_soft(), size::BORDER),
            text_input::Status::Active => (t.border_control(), size::BORDER),
            text_input::Status::Disabled => (t.divider(), size::BORDER),
        };
        text_input::Style {
            background: Background::Color(t.bg),
            border: Border {
                color,
                width,
                radius: radius::CONTROL.into(),
            },
            icon: t.text_muted,
            placeholder: t.text_muted,
            value: if matches!(status, text_input::Status::Disabled) {
                t.text_muted
            } else {
                t.text
            },
            selection: alpha(t.accent, 0.35),
        }
    }
}

/// Barra de rolagem discreta: sem trilho, so o cursor opaco (`scroller_rest`, `idle()` sob o
/// mouse ou arrastado, B3). A padrao do iced pinta o trilho com a paleta derivada, que nao e a
/// do design; o texto a 18 % que estava aqui ficava em 1,7:1 e o cursor quase nao se via.
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
                background: Background::Color(if active { t.idle() } else { t.scroller_rest() }),
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
    fn grafite(t: &Tokens) -> [(&'static str, Color); 27] {
        [
            ("bg", t.bg),
            ("surface", t.surface),
            ("surface_2", t.surface_2),
            ("fg", t.text),
            ("text_muted", t.text_muted),
            ("divider", t.divider()),
            ("accent", t.accent),
            ("on_accent", t.on_accent),
            ("accent_100", t.accent_100),
            ("accent_200", t.accent_200),
            ("accent_300", t.accent_300),
            ("accent_400", t.accent_400),
            ("accent_500", t.accent_500),
            ("accent_600", t.accent_600),
            ("accent_700", t.accent_700),
            ("accent_800", t.accent_800),
            ("accent_900", t.accent_900),
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
        let expected: [(u32, u32); 27] = [
            (0x131313, 0xf5f5f5),
            (0x1e1e1e, 0xffffff),
            (0x262626, 0xeeeeee),
            (0xf2f2f2, 0x202020),
            (0xa6a6a6, 0x6b6b6b),
            (0x333333, 0xe4e4e4),
            (0xe2c15a, 0x876c00),
            (0x131313, 0xffffff),
            (0xf9f3dd, 0x392c01),
            (0xf8e9b8, 0x4d4014),
            (0xf0d98c, 0x675412),
            (0xe8cd78, 0x866c10),
            (0xe2c15a, 0xa48402),
            (0xb29947, 0xc0a44f),
            (0x847335, 0xdbc276),
            (0x5a4f24, 0xf0e0af),
            (0x332d14, 0xf8efd5),
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
        assert_eq!(Tokens::for_mode(ThemeMode::Dark, false).bg, hex(0x131313));
        assert_eq!(Tokens::for_mode(ThemeMode::Light, true).bg, hex(0xf5f5f5));
        assert_eq!(Tokens::for_mode(ThemeMode::System, true).bg, hex(0x131313));
        assert_eq!(Tokens::for_mode(ThemeMode::System, false).bg, hex(0xf5f5f5));
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

    /// Pares texto/fundo que o Dashboard Grafite introduziu (H4b), nos dois temas.
    #[test]
    fn textos_novos_do_dashboard_passam_4_5() {
        for t in [&DARK, &LIGHT] {
            let pairs = [
                ("renda/painel", t.income_fg, t.chart_panel()),
                ("normal/painel", t.text_muted, t.chart_panel()),
                ("previsao/painel", t.accent_300, t.chart_panel()),
                ("eixo/painel", t.text_muted, t.chart_panel()),
                ("tag/divisor", t.text_soft(), t.divider()),
                ("chip normal/divisor", t.text, t.divider()),
                ("apoio/caixa", t.neutral_300, t.surface_2),
                ("apoio/card", t.neutral_300, t.surface),
                ("rotulo/caixa", t.text_muted, t.surface_2),
                ("aviso/caixa", t.warning, t.surface_2),
                ("faltar/card", t.expense_fg, t.surface),
                ("sobrar/card", t.income_fg, t.surface),
                ("chip hoje", t.accent_300, t.accent_900),
                ("link/card", t.accent_300, t.surface),
            ];
            for (name, fg, bg) in pairs {
                let c = contrast(fg, bg);
                assert!(c >= 4.5, "{name} {:?}: {c:.2}", t.mode);
            }
        }
    }

    #[test]
    fn fundo_da_pagina_e_gradiente_do_brilho_ao_bg() {
        for t in [&DARK, &LIGHT] {
            let style = page(t)(&t.iced_theme());
            let Some(Background::Gradient(Gradient::Linear(g))) = style.background else {
                panic!("fundo sem gradiente");
            };
            let stops: Vec<(f32, Color)> = g
                .stops
                .iter()
                .flatten()
                .map(|s| (s.offset, s.color))
                .collect();
            assert_eq!(stops, vec![(0.0, t.glow()), (0.55, t.bg)], "{:?}", t.mode);
        }
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

    /// Paleta §4.4: o texto secundario do hub passa 4.5:1 em todo fundo onde aparece (lateral e
    /// cabecalho sobre `bg`, card de status sobre `surface`, caixas internas sobre `surface_2`).
    #[test]
    fn text_muted_passa_4_5_nos_dois_temas() {
        for t in [&DARK, &LIGHT] {
            for (fundo, cor) in [
                ("bg", t.bg),
                ("surface", t.surface),
                ("surface_2", t.surface_2),
            ] {
                let c = contrast(t.text_muted, cor);
                assert!(c >= 4.5, "text_muted/{fundo} {:?}: {c:.2}", t.mode);
            }
        }
        // A §4.4 diz 7,5:1 sobre #1e1e1e; a conta da 6,85 (7,63 e sobre `bg`). Os do claro batem.
        let near = |got: f64, want: f64| (got - want).abs() < 0.01;
        assert!(near(contrast(DARK.text_muted, DARK.surface), 6.85));
        assert!(near(contrast(DARK.text_muted, DARK.bg), 7.63));
        assert!(near(contrast(LIGHT.text_muted, LIGHT.surface), 5.33));
        assert!(near(contrast(LIGHT.text_muted, LIGHT.bg), 4.89));
        assert!(near(contrast(LIGHT.text_muted, LIGHT.surface_2), 4.59));
    }

    #[test]
    fn texto_de_apoio_passa_4_5_e_fica_entre_fg_e_muted() {
        for t in [&DARK, &LIGHT] {
            for (fundo, cor) in [("bg", t.bg), ("surface", t.surface)] {
                let c = contrast(t.text_soft(), cor);
                assert!(c >= 4.5, "text_soft/{fundo} {:?}: {c:.2}", t.mode);
            }
            let (fg, soft, muted) = (
                contrast(t.text, t.bg),
                contrast(t.text_soft(), t.bg),
                contrast(t.text_muted, t.bg),
            );
            assert!(fg > soft && soft > muted, "{:?}", t.mode);
        }
        assert_eq!(DARK.text_soft(), hex(0xbdbdbd));
    }

    /// Fundos onde um ponto de estado ou um icone apagado aparece: pagina, lateral, card, caixa
    /// interna e linha sob o mouse (texto a 4 % sobre o card).
    fn backgrounds(t: &Tokens) -> [(&'static str, Color); 5] {
        [
            ("bg", t.bg),
            ("side", t.side()),
            ("surface", t.surface),
            ("surface_2", t.surface_2),
            ("hover", mix(t.surface, t.text, 0.04)),
        ]
    }

    /// WCAG 1.4.11: ponto de estado e icone sem texto pedem 3:1. O `neutral_600` do claro dava
    /// ~2:1 (H1); `idle` e o degrau que passa nos dois temas.
    #[test]
    fn pontos_de_estado_passam_3_em_todo_fundo() {
        for t in [&DARK, &LIGHT] {
            for (fundo, cor) in backgrounds(t) {
                for (nome, dot) in [
                    ("idle", t.idle()),
                    ("income", t.income),
                    ("warning", t.warning),
                    ("expense", t.expense),
                ] {
                    let c = contrast(dot, cor);
                    assert!(c >= 3.0, "{nome}/{fundo} {:?}: {c:.2}", t.mode);
                }
            }
        }
        assert_eq!(DARK.idle(), DARK.neutral_600);
        assert_eq!(LIGHT.idle(), LIGHT.neutral_400);
    }

    /// Avatar sem cor de perfil: a escolha da H4b para "Quem lançou" vale em toda tela.
    #[test]
    fn avatar_neutro_passa_4_5_nos_dois_temas() {
        assert_eq!(DARK.avatar_neutral(), (DARK.neutral_300, DARK.bg));
        assert_eq!(LIGHT.avatar_neutral(), (LIGHT.neutral_700, LIGHT.text));
        for t in [&DARK, &LIGHT] {
            let (fill, initial) = t.avatar_neutral();
            let c = contrast(initial, fill);
            assert!(c >= 4.5, "{:?}: {c:.2}", t.mode);
        }
    }

    /// Pares texto/fundo de Conexao, Lancamentos, Reservas, dialogo de remocao, toasts e menus
    /// (H4c), nos dois temas.
    #[test]
    fn textos_das_outras_telas_passam_4_5() {
        for t in [&DARK, &LIGHT] {
            let pairs = [
                ("muted/card", t.text_muted, t.surface),
                ("muted/bg", t.text_muted, t.bg),
                ("apoio/card", t.text_soft(), t.surface),
                ("apoio/caixa", t.text_soft(), t.surface_2),
                ("apoio/pilula", t.text_soft(), t.neutral_800),
                ("apoio/divisor", t.text_soft(), t.divider()),
                (
                    "muted/linha sob o mouse",
                    t.text_muted,
                    mix(t.surface, t.text, 0.04),
                ),
                ("chip ativo", t.accent_200, t.accent_900),
                ("selo do acento", t.accent_300, t.accent_900),
                (
                    "tile do acento",
                    t.accent_300,
                    mix(t.surface, t.accent, 0.16),
                ),
                (
                    "contorno do acento",
                    t.accent_300,
                    mix(t.surface, t.accent, 0.16),
                ),
                ("item escolhido", t.text, mix(t.surface, t.accent, 0.10)),
                ("remover", t.expense_fg, t.surface),
                (
                    "remover cheio",
                    t.expense_fg,
                    mix(t.surface, t.expense, 0.14),
                ),
                (
                    "remover cheio sob o mouse",
                    t.expense_fg,
                    mix(t.surface, t.expense, 0.20),
                ),
                ("aviso/card", t.warning, t.surface),
                ("erro/card", t.expense_fg, t.surface),
                ("texto/card", t.text, t.surface),
            ];
            for (name, fg, bg) in pairs {
                let c = contrast(fg, bg);
                assert!(c >= 4.5, "{name} {:?}: {c:.2}", t.mode);
            }
        }
        // O quadro do QR e claro nos dois temas: aviso do claro e texto `bg` do escuro por cima.
        assert!(contrast(LIGHT.warning, DARK.neutral_100) >= 4.5);
        assert!(contrast(DARK.bg, DARK.neutral_100) >= 4.5);
    }

    /// Lacuna da paleta, registrada e nao corrigida (os `tag-*` nao mudam, paleta §1): no claro,
    /// a inicial em `bg` sobre a cor da pessoa fica entre 3,1 e 4,5:1. Nenhum cinza fixo passa
    /// nas doze (o `fg` da 3,3 no violeta); quem mexer precisa de outra regra, nao de outro hex.
    #[test]
    fn inicial_sobre_a_cor_da_pessoa_no_claro_fica_abaixo_do_piso() {
        let worst = colors::TOKENS
            .iter()
            .map(|tk| contrast(LIGHT.bg, LIGHT.token(tk)))
            .fold(f64::MAX, f64::min);
        assert!(worst > 3.0 && worst < 4.5, "{worst:.2}");
        for tk in colors::TOKENS {
            assert!(contrast(DARK.bg, DARK.token(tk)) >= 4.5, "{tk}");
        }
    }

    /// Spec 9: a serie da emergencia (Reservas) usa `accent`, que ficou quase igual ao
    /// `tag-amber`. Medido aqui para o roteiro do `[Validação]` conferir a olho: caixinha ambar ao
    /// lado da emergencia fica com a mesma cor no grafico.
    #[test]
    fn acento_e_tag_amber_quase_iguais() {
        assert!(contrast(DARK.accent, DARK.token("amber")) < 1.2);
        assert!(contrast(LIGHT.accent, LIGHT.token("amber")) < 1.6);
    }

    /// B3: o que saiu do piso de 4,5 e o que entrou no lugar. Rodape do Dashboard (era o texto
    /// a 50 %, 3,17 no claro) em `text_muted`; ritmo da caixinha (era `tag-orange`, 3,80 no claro)
    /// em `warning`; tag da cor da pessoa (era o nome na cor, 2,84 no pior token do claro) com o
    /// nome em `text_soft` sobre a mesma tinta de 16 %, e a cor so no ponto.
    #[test]
    fn rodape_ritmo_e_tag_de_cor_passam_4_5() {
        assert!(contrast(alpha_over(LIGHT.text, 0.50, LIGHT.bg), LIGHT.bg) < 4.5);
        assert!(contrast(LIGHT.token("orange"), LIGHT.surface) < 4.5);
        let worst_old = colors::TOKENS
            .iter()
            .map(|tk| contrast(LIGHT.token(tk), LIGHT.token_tint(tk, 0.16)))
            .fold(f64::MAX, f64::min);
        assert!(worst_old < 4.5, "{worst_old:.2}");
        for t in [&DARK, &LIGHT] {
            assert!(contrast(t.text_muted, t.bg) >= 4.5, "rodape {:?}", t.mode);
            assert!(contrast(t.warning, t.surface) >= 4.5, "ritmo {:?}", t.mode);
            for tk in colors::TOKENS {
                let c = contrast(t.text_soft(), t.token_tint(tk, 0.16));
                assert!(c >= 4.5, "tag {tk} {:?}: {c:.2}", t.mode);
            }
        }
    }

    /// Cor com alfa composta sobre um fundo solido, como a tela a desenha.
    fn alpha_over(c: Color, a: f32, bg: Color) -> Color {
        mix(bg, c, a)
    }

    /// A3: todo estilo de botao apaga quando desabilitado, sem tinta e com contorno no divisor.
    #[test]
    fn todo_botao_desabilitado_apaga_sem_tinta() {
        type Style = Box<dyn Fn(&Theme, button::Status) -> button::Style>;
        for t in [&DARK, &LIGHT] {
            let theme = t.iced_theme();
            let styles: Vec<(&str, Style)> = vec![
                ("nav ativo", Box::new(nav_item(t, true))),
                ("nav", Box::new(nav_item(t, false))),
                ("segment ativo", Box::new(segment(t, true))),
                ("icone", Box::new(icon_button(t))),
                ("seg_option ativo", Box::new(seg_option(t, true))),
                (
                    "ghost",
                    Box::new(button(t, ButtonKind::Ghost, ButtonSize::Compact)),
                ),
                (
                    "link",
                    Box::new(button(t, ButtonKind::Link, ButtonSize::Regular)),
                ),
                (
                    "outline_accent",
                    Box::new(button(t, ButtonKind::Primary, ButtonSize::Regular)),
                ),
                (
                    "outline_divider",
                    Box::new(button(t, ButtonKind::Secondary, ButtonSize::Regular)),
                ),
                (
                    "danger_outline",
                    Box::new(button(t, ButtonKind::Danger, ButtonSize::Regular)),
                ),
                (
                    "danger_filled",
                    Box::new(button(t, ButtonKind::DangerFilled, ButtonSize::Regular)),
                ),
                ("titlebar", Box::new(titlebar_button(t, true))),
                ("chip ativo", Box::new(chip(t, true))),
                ("filtro ativo", Box::new(filter_button(t, true))),
                ("menu_item", Box::new(menu_item(t, true))),
            ];
            for (name, style) in styles {
                let s = style(&theme, button::Status::Disabled);
                assert_eq!(s.text_color, t.text_disabled(), "{name} {:?}", t.mode);
                assert!(s.background.is_none(), "{name} {:?}", t.mode);
                if s.border.width > 0.0 {
                    assert_eq!(s.border.color, t.divider(), "{name} {:?}", t.mode);
                }
                // Ativo continua com a cara de sempre.
                let active = style(&theme, button::Status::Active);
                assert_ne!(active.text_color, t.idle(), "{name} {:?}", t.mode);
            }
        }
    }

    #[test]
    fn papeis_derivados_saem_da_paleta() {
        assert_eq!(DARK.card_border(), DARK.neutral_900);
        assert_eq!(LIGHT.card_border(), LIGHT.neutral_800);
        for t in [&DARK, &LIGHT] {
            assert_eq!(t.border_control(), t.idle(), "{:?}", t.mode);
            assert_eq!(t.text_disabled(), t.idle(), "{:?}", t.mode);
            assert_eq!(t.text_accent(), t.accent_300, "{:?}", t.mode);
            assert_eq!(t.state(false), t.text_alpha(0.04), "{:?}", t.mode);
            assert_eq!(t.state(true), t.text_alpha(0.08), "{:?}", t.mode);
            let glow = t.accent_glow();
            assert_eq!(glow.blur_radius, 24.0);
            assert_eq!(
                Color {
                    a: 1.0,
                    ..glow.color
                },
                t.accent
            );
        }
        assert!((DARK.accent_glow().color.a - 0.28).abs() < f32::EPSILON);
        assert!((LIGHT.accent_glow().color.a - 0.16).abs() < f32::EPSILON);
    }

    /// Canal a canal com folga de 1: a mistura e em float e dois canais do claro caem em x,5.
    fn assert_rgb8(got: Color, want: u32, name: &str) {
        let [r, g, b, a] = got.into_rgba8();
        let w = hex(want).into_rgba8();
        assert_eq!(a, 255, "{name} opaco");
        for (c, wc) in [r, g, b].into_iter().zip(w) {
            assert!(c.abs_diff(wc) <= 1, "{name}: {got:?} contra #{want:06x}");
        }
    }

    /// Tinta de 10 % de cada tom sobre `surface`, conferida contra o hex calculado a mao da
    /// paleta (tom a 10 % sobre #1e1e1e e #ffffff).
    #[test]
    fn tinta_de_cada_tom_sobre_surface() {
        let cases = [
            (Tone::Accent, 0x322e24, 0xf3f0e6),
            (Tone::Income, 0x27302b, 0xeaf4f0),
            (Tone::Expense, 0x332928, 0xfaeeee),
            (Tone::Warning, 0x332a22, 0xf5eee6),
        ];
        for (tone, dark, light) in cases {
            assert_rgb8(
                DARK.tint(tone, DARK.surface),
                dark,
                &format!("{tone:?} escuro"),
            );
            assert_rgb8(
                LIGHT.tint(tone, LIGHT.surface),
                light,
                &format!("{tone:?} claro"),
            );
        }
    }

    /// Relatorio 7.3: a borda do card nao e contraste, e percepcao (1,29 e 1,16 no escuro, 1,17
    /// e 1,27 no claro). Fixado para quem mexer saber que nao e erro de piso.
    #[test]
    fn borda_do_card_separa_sem_chegar_a_3() {
        let near = |got: f64, want: f64| (got - want).abs() < 0.01;
        assert!(near(contrast(DARK.card_border(), DARK.bg), 1.29));
        assert!(near(contrast(DARK.card_border(), DARK.surface), 1.16));
        assert!(near(contrast(LIGHT.card_border(), LIGHT.bg), 1.17));
        assert!(near(contrast(LIGHT.card_border(), LIGHT.surface), 1.27));
    }

    /// Relatorio 7.3, input (nao texto, piso 3): contorno sobre `surface` e `bg`, hover e foco
    /// sobre `bg`. Valor e placeholder continuam no piso de texto.
    #[test]
    fn campo_de_texto_tem_contorno_visivel() {
        for t in [&DARK, &LIGHT] {
            let theme = t.iced_theme();
            let style = input(t);
            let rest = style(&theme, text_input::Status::Active);
            let hover = style(&theme, text_input::Status::Hovered);
            let focus = style(&theme, text_input::Status::Focused { is_hovered: false });
            assert_eq!(rest.border.color, t.border_control(), "{:?}", t.mode);
            assert_eq!(hover.border.color, t.text_soft(), "{:?}", t.mode);
            assert_eq!(focus.border.color, t.accent, "{:?}", t.mode);
            assert_eq!(rest.border.width, size::BORDER);
            assert_eq!(focus.border.width, size::FOCUS_BORDER);
            for (name, fg, bg) in [
                ("contorno/card", rest.border.color, t.surface),
                ("contorno/bg", rest.border.color, t.bg),
                ("hover/bg", hover.border.color, t.bg),
                ("foco/bg", focus.border.color, t.bg),
                ("foco/card", focus.border.color, t.surface),
            ] {
                let c = contrast(fg, bg);
                assert!(c >= 3.0, "{name} {:?}: {c:.2}", t.mode);
            }
            assert!(contrast(t.text, t.bg) >= 4.5);
            assert!(contrast(t.text_muted, t.bg) >= 4.5);
            // A lista de modelos acompanha o campo.
            let pick = pick(t)(&theme, pick_list::Status::Active);
            assert_eq!(pick.border.color, t.border_control(), "{:?}", t.mode);
        }
        let near = |got: f64, want: f64| (got - want).abs() < 0.01;
        assert!(near(contrast(DARK.border_control(), DARK.surface), 3.88));
        assert!(near(contrast(LIGHT.border_control(), LIGHT.surface), 4.67));
    }

    /// B3: o cursor da rolagem em repouso fica abaixo de 3 de proposito (2,87 e 2,03 sobre `bg`,
    /// lacuna registrada); sob o mouse ou arrastado vira `idle()` e passa.
    #[test]
    fn cursor_da_rolagem_segue_b3() {
        let near = |got: f64, want: f64| (got - want).abs() < 0.01;
        assert!(near(contrast(DARK.scroller_rest(), DARK.bg), 2.87));
        assert!(near(contrast(LIGHT.scroller_rest(), LIGHT.bg), 2.03));
        for t in [&DARK, &LIGHT] {
            let theme = t.iced_theme();
            let style = scrollbar(t);
            let scroller = |status| match style(&theme, status).vertical_rail.scroller.background {
                Background::Color(c) => c,
                Background::Gradient(_) => panic!("cursor com gradiente"),
            };
            let rest = scroller(scrollable::Status::Active {
                is_horizontal_scrollbar_disabled: false,
                is_vertical_scrollbar_disabled: false,
            });
            assert_eq!(rest, t.scroller_rest(), "{:?}", t.mode);
            let active = scroller(scrollable::Status::Dragged {
                is_horizontal_scrollbar_dragged: false,
                is_vertical_scrollbar_dragged: true,
                is_horizontal_scrollbar_disabled: false,
                is_vertical_scrollbar_disabled: false,
            });
            assert_eq!(active, t.idle(), "{:?}", t.mode);
            assert!(contrast(active, t.bg) >= 3.0, "{:?}", t.mode);
        }
    }

    #[test]
    fn caixa_interna_e_surface_2_sem_borda() {
        for t in [&DARK, &LIGHT] {
            let style = inset(t)(&t.iced_theme());
            assert_eq!(style.background, bg(t.surface_2), "{:?}", t.mode);
            assert_eq!(style.border.width, 0.0);
            assert_eq!(style.border.radius, 6.0.into());
        }
    }

    #[test]
    fn card_tem_borda_de_1_px() {
        for t in [&DARK, &LIGHT] {
            let style = card(t)(&t.iced_theme());
            assert_eq!(style.border.color, t.card_border(), "{:?}", t.mode);
            assert_eq!(style.border.width, size::BORDER);
            assert_eq!(style.border.radius, radius::CARD.into());
        }
    }

    const KINDS: [ButtonKind; 6] = [
        ButtonKind::Primary,
        ButtonKind::Secondary,
        ButtonKind::Ghost,
        ButtonKind::Link,
        ButtonKind::Danger,
        ButtonKind::DangerFilled,
    ];
    const SIZES: [ButtonSize; 3] = [ButtonSize::Compact, ButtonSize::Regular, ButtonSize::Large];

    /// Cor com alfa composta sobre um pai opaco, como a tela a desenha.
    fn composite(c: Color, parent: Color) -> Color {
        mix(parent, Color { a: 1.0, ..c }, c.a)
    }

    /// Fundo opaco que o botao desenha sobre `parent` (tinta com alfa composta).
    fn painted(s: &button::Style, parent: Color) -> Color {
        match s.background {
            None => parent,
            Some(Background::Color(c)) => composite(c, parent),
            Some(Background::Gradient(_)) => panic!("botao com gradiente"),
        }
    }

    #[test]
    fn todo_papel_e_tamanho_apaga_desabilitado() {
        for t in [&DARK, &LIGHT] {
            let theme = t.iced_theme();
            for kind in KINDS {
                for size in SIZES {
                    let s = button(t, kind, size)(&theme, button::Status::Disabled);
                    assert_eq!(s.text_color, t.text_disabled(), "{kind:?} {:?}", t.mode);
                    assert!(s.background.is_none(), "{kind:?} {:?}", t.mode);
                    assert_eq!(s.shadow, Shadow::default(), "{kind:?} {:?}", t.mode);
                    if s.border.width > 0.0 {
                        assert_eq!(s.border.color, t.divider(), "{kind:?} {:?}", t.mode);
                    }
                    assert_eq!(s.border.radius, size.radius().into(), "{kind:?}");
                }
            }
        }
    }

    /// Os oito estilos de item tambem separam `Pressed` de `Hovered`, e o repouso do hover.
    /// Ativos sem camada (item da lateral ativo, segmento ativo) ficam de fora: neles o clique
    /// nao muda nada.
    #[test]
    fn pressionado_difere_do_hover_nos_estilos_de_item() {
        type Style = Box<dyn Fn(&Theme, button::Status) -> button::Style>;
        for t in [&DARK, &LIGHT] {
            let theme = t.iced_theme();
            let styles: Vec<(&str, Style)> = vec![
                ("nav", Box::new(nav_item(t, false))),
                ("segment", Box::new(segment(t, false))),
                ("icone", Box::new(icon_button(t))),
                ("seg_option", Box::new(seg_option(t, false))),
                ("titlebar", Box::new(titlebar_button(t, false))),
                ("fechar", Box::new(titlebar_button(t, true))),
                ("chip", Box::new(chip(t, false))),
                ("filtro", Box::new(filter_button(t, false))),
                ("menu_item", Box::new(menu_item(t, false))),
                ("menu_item escolhido", Box::new(menu_item(t, true))),
            ];
            for (name, style) in styles {
                let rest = style(&theme, button::Status::Active);
                let hover = style(&theme, button::Status::Hovered);
                let down = style(&theme, button::Status::Pressed);
                assert_ne!(hover, down, "{name} {:?}", t.mode);
                assert_ne!(rest, hover, "{name} {:?}", t.mode);
            }
            // Camada neutra: 4 % sob o mouse, 8 % pressionado.
            let nav = nav_item(t, false);
            assert_eq!(
                nav(&theme, button::Status::Hovered).background,
                bg(t.state(false))
            );
            assert_eq!(
                nav(&theme, button::Status::Pressed).background,
                bg(t.state(true))
            );
        }
    }

    /// O fechar pressionado escurece: o texto claro continua acima de 4,5 por cima dele.
    #[test]
    fn fechar_pressionado_mantem_o_contraste() {
        for t in [&DARK, &LIGHT] {
            let theme = t.iced_theme();
            let s = titlebar_button(t, true)(&theme, button::Status::Pressed);
            let Some(Background::Color(fill)) = s.background else {
                panic!("fechar pressionado sem fundo");
            };
            assert!(contrast(s.text_color, fill) >= 4.5, "{:?}", t.mode);
        }
    }

    #[test]
    fn linha_de_lista_so_tem_fundo_sob_o_mouse() {
        for t in [&DARK, &LIGHT] {
            let theme = t.iced_theme();
            assert_eq!(list_row(t, false, 8.0)(&theme).background, None);
            assert_eq!(
                list_row(t, true, 8.0)(&theme).background,
                bg(t.state(false))
            );
            assert_eq!(list_row(t, true, 0.0)(&theme).border.radius, 0.0.into());
        }
    }

    /// `Pressed` tem cara propria em todo papel; antes era o mesmo estilo do hover.
    #[test]
    fn pressionado_difere_do_hover_em_todo_papel() {
        for t in [&DARK, &LIGHT] {
            let theme = t.iced_theme();
            for kind in KINDS {
                let style = button(t, kind, ButtonSize::Regular);
                let rest = style(&theme, button::Status::Active);
                let hover = style(&theme, button::Status::Hovered);
                let down = style(&theme, button::Status::Pressed);
                assert_ne!(hover, down, "{kind:?} {:?}", t.mode);
                assert_ne!(rest, hover, "{kind:?} {:?}", t.mode);
            }
        }
    }

    /// O que o relatorio nao pediu para mudar continua igual: fantasma a 12 % no hover com raio
    /// 6, primario a 8/16 %, perigo a 12 % e preenchido a 14/20 %. O secundario passou de 7 % para
    /// a camada `state` de 4 % (relatorio 9); o primario continua sem sombra.
    #[test]
    fn estilos_antigos_delegam_e_mantem_a_cara() {
        let opaque = |c: Color| c.a == 1.0;
        for t in [&DARK, &LIGHT] {
            let theme = t.iced_theme();
            let (rest, hover, down) = (
                button::Status::Active,
                button::Status::Hovered,
                button::Status::Pressed,
            );
            let m = t.mode;

            let g = button(t, ButtonKind::Ghost, ButtonSize::Compact);
            assert_eq!(g(&theme, rest).background, None, "{m:?}");
            assert_eq!(
                g(&theme, hover).background,
                bg(alpha(t.accent, 0.12)),
                "{m:?}"
            );
            assert_eq!(
                g(&theme, down).background,
                bg(alpha(t.accent, 0.20)),
                "{m:?}"
            );
            assert_eq!(g(&theme, rest).border.radius, 6.0.into());
            assert_eq!(g(&theme, rest).text_color, t.accent_300, "{m:?}");

            let p = button(t, ButtonKind::Primary, ButtonSize::Regular);
            assert_eq!(
                p(&theme, rest).background,
                bg(alpha(t.accent, 0.08)),
                "{m:?}"
            );
            assert_eq!(
                p(&theme, hover).background,
                bg(alpha(t.accent, 0.16)),
                "{m:?}"
            );
            assert_eq!(
                p(&theme, down).background,
                bg(alpha(t.accent, 0.24)),
                "{m:?}"
            );
            assert_eq!(p(&theme, rest).border.color, t.accent, "{m:?}");
            assert_eq!(p(&theme, rest).border.radius, 8.0.into());
            for status in [rest, hover, down] {
                assert_eq!(
                    p(&theme, status).shadow,
                    Shadow::default(),
                    "primario sem brilho"
                );
            }

            let sec = button(t, ButtonKind::Secondary, ButtonSize::Regular);
            assert_eq!(sec(&theme, rest).background, None, "{m:?}");
            assert_eq!(
                sec(&theme, hover).background,
                bg(alpha(t.text, 0.04)),
                "{m:?}"
            );
            assert_eq!(
                sec(&theme, down).background,
                bg(alpha(t.text, 0.08)),
                "{m:?}"
            );
            assert_eq!(sec(&theme, rest).border.color, t.divider(), "{m:?}");

            let d = button(t, ButtonKind::Danger, ButtonSize::Regular);
            assert_eq!(
                d(&theme, hover).background,
                bg(alpha(t.expense, 0.12)),
                "{m:?}"
            );
            assert_eq!(
                d(&theme, down).background,
                bg(alpha(t.expense, 0.12)),
                "{m:?}"
            );
            assert_eq!(
                d(&theme, hover).border.color,
                alpha(t.expense, 0.45),
                "{m:?}"
            );
            assert_eq!(d(&theme, down).border.color, t.expense, "{m:?}");
            assert!(opaque(d(&theme, down).border.color), "{m:?}");

            let f = button(t, ButtonKind::DangerFilled, ButtonSize::Regular);
            assert_eq!(
                f(&theme, rest).background,
                bg(alpha(t.expense, 0.14)),
                "{m:?}"
            );
            assert_eq!(
                f(&theme, hover).background,
                bg(alpha(t.expense, 0.20)),
                "{m:?}"
            );
            assert_eq!(
                f(&theme, down).background,
                bg(alpha(t.expense, 0.20)),
                "{m:?}"
            );
            assert_eq!(
                f(&theme, hover).border.color,
                alpha(t.expense, 0.55),
                "{m:?}"
            );
            assert_eq!(f(&theme, down).border.color, t.expense, "{m:?}");
            assert!(opaque(f(&theme, down).border.color), "{m:?}");

            let l = button(t, ButtonKind::Link, ButtonSize::Regular);
            assert_eq!(l(&theme, rest).text_color, t.accent_300, "{m:?}");
            assert_eq!(l(&theme, hover).text_color, t.accent_200, "{m:?}");
            assert_eq!(l(&theme, down).text_color, t.accent_100, "{m:?}");
            assert_eq!(l(&theme, down).background, None, "{m:?}");
        }
    }

    /// Relatorio 7.3: texto de botao passa 4,5 em repouso, sob o mouse e pressionado, sobre os
    /// fundos onde o papel pode morar. Perigo preenchido so dentro de card ou dialogo (`surface`).
    #[test]
    fn texto_de_botao_passa_4_5_em_todo_estado() {
        let states = [
            button::Status::Active,
            button::Status::Hovered,
            button::Status::Pressed,
        ];
        for t in [&DARK, &LIGHT] {
            let theme = t.iced_theme();
            for kind in KINDS {
                let parents: &[(&str, Color)] = if kind == ButtonKind::DangerFilled {
                    &[("surface", t.surface)]
                } else {
                    &[("surface", t.surface), ("bg", t.bg)]
                };
                let style = button(t, kind, ButtonSize::Regular);
                for status in states {
                    let s = style(&theme, status);
                    for (name, parent) in parents {
                        let c = contrast(s.text_color, painted(&s, *parent));
                        assert!(c >= 4.5, "{kind:?} {status:?}/{name} {:?}: {c:.2}", t.mode);
                    }
                }
            }
        }
        // Os numeros da tabela 7.3 (primario 8/16/24 % sobre `surface`).
        let near = |got: f64, want: f64| (got - want).abs() < 0.02;
        let theme = DARK.iced_theme();
        let primary = button(&DARK, ButtonKind::Primary, ButtonSize::Regular);
        let on = |status| {
            let s = primary(&theme, status);
            contrast(s.text_color, painted(&s, DARK.surface))
        };
        assert!(near(on(button::Status::Active), 10.10));
        assert!(near(on(button::Status::Hovered), 8.36));
        assert!(near(on(button::Status::Pressed), 6.81));
    }

    /// Contorno de botao e controle (nao texto, piso 3): primario e perigo pressionado. O
    /// secundario fica no divisor de proposito (o texto `fg` e o que o identifica).
    #[test]
    fn contorno_do_primario_e_do_perigo_pressionado_passam_3() {
        for t in [&DARK, &LIGHT] {
            let theme = t.iced_theme();
            for (name, kind) in [
                ("primario", ButtonKind::Primary),
                ("perigo", ButtonKind::Danger),
                ("perigo cheio", ButtonKind::DangerFilled),
            ] {
                let s = button(t, kind, ButtonSize::Regular)(&theme, button::Status::Pressed);
                for parent in [t.surface, t.bg] {
                    let c = contrast(s.border.color, parent);
                    assert!(c >= 3.0, "{name} {:?}: {c:.2}", t.mode);
                }
            }
        }
    }

    /// `on_accent` sobre preenchimento `accent` (paleta §4.3: 10,63 e 5,03).
    #[test]
    fn on_accent_passa_4_5_sobre_o_acento() {
        let near = |got: f64, want: f64| (got - want).abs() < 0.01;
        assert!(near(contrast(DARK.on_accent, DARK.accent), 10.63));
        assert!(near(contrast(LIGHT.on_accent, LIGHT.accent), 5.03));
    }

    /// Relatorio 7.4: os valores de cada escala, em ordem.
    #[test]
    fn escalas_batem_com_o_relatorio() {
        use space::*;
        assert_eq!(
            [XS, S, M, L, XL, XXL, XXXL, HUGE, GIANT],
            [4.0, 8.0, 12.0, 16.0, 20.0, 24.0, 32.0, 40.0, 56.0]
        );
        assert_eq!(
            [BETWEEN_CARDS, space::CARD, DIALOG, PAGE],
            [16.0, 20.0, 24.0, 32.0]
        );
        assert_eq!(
            [
                radius::BAR,
                radius::INSET,
                radius::CONTROL,
                radius::CARD,
                radius::DIALOG,
                radius::PILL
            ],
            [4.0, 6.0, 8.0, 8.0, 12.0, 999.0]
        );
        assert_eq!(
            [
                size::CTRL_S,
                size::CTRL_M,
                size::CTRL_L,
                size::ROW,
                size::ROW_RICH,
                size::BORDER,
                size::FOCUS_BORDER
            ],
            [28.0, 36.0, 44.0, 44.0, 52.0, 1.0, 2.0]
        );
        use type_scale as ty;
        assert_eq!(
            [
                ty::MICRO,
                ty::META,
                ty::LABEL,
                ty::BODY,
                ty::SUPPORT,
                ty::SECTION,
                ty::VALUE_S,
                ty::LEAD,
                ty::VALUE_M,
                ty::PAGE,
                ty::VALUE_L,
                ty::VALUE_XL
            ],
            [
                11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0, 18.0, 24.0, 30.0, 36.0, 44.0
            ]
        );
    }

    #[test]
    fn tamanhos_de_botao_seguem_a_escala() {
        assert_eq!(
            SIZES.map(ButtonSize::height),
            [size::CTRL_S, size::CTRL_M, size::CTRL_L]
        );
        assert_eq!(SIZES.map(ButtonSize::height), [28.0, 36.0, 44.0]);
        assert_eq!(ButtonSize::Compact.radius(), radius::INSET);
        assert_eq!(ButtonSize::Regular.radius(), radius::CONTROL);
        assert_eq!(SIZES.map(ButtonSize::text_size), [13.0, 13.0, 14.0]);
        assert_eq!(SIZES.map(ButtonSize::padding_x), [12.0, 16.0, 20.0]);
    }

    /// Relatorio 7.3, pares de componente que as primitivas da Fase 7 vao desenhar: banner (tom
    /// sobre a propria tinta de 10 %, e o corpo em `fg`/`text_soft` sobre a pior tinta), toggle e
    /// icone de severidade. Texto pede 4,5; borda, botao do toggle e icone pedem 3.
    #[test]
    fn pares_novos_dos_componentes_passam_o_piso() {
        for t in [&DARK, &LIGHT] {
            let tones = [
                ("warning", Tone::Warning, t.warning),
                ("expense", Tone::Expense, t.expense_fg),
                ("income", Tone::Income, t.income_fg),
                ("accent", Tone::Accent, t.text_accent()),
            ];
            for (fundo, parent) in [("bg", t.bg), ("surface", t.surface)] {
                for (name, tone, fg) in tones {
                    let tint = t.tint(tone, parent);
                    let c = contrast(fg, tint);
                    assert!(c >= 4.5, "banner {name}/{fundo} {:?}: {c:.2}", t.mode);
                    for (body, color) in [("fg", t.text), ("text_soft", t.text_soft())] {
                        let c = contrast(color, tint);
                        assert!(c >= 4.5, "corpo {body} {name}/{fundo} {:?}: {c:.2}", t.mode);
                    }
                }
            }
            let text = [
                ("toggle ligado", t.accent_200, t.accent_800),
                ("toggle desligado", t.text_muted, t.neutral_900),
                ("tag neutra", t.text_soft(), t.neutral_800),
                ("segmento neutro", t.text, t.divider()),
                ("tag de acento", t.accent_300, t.accent_900),
            ];
            for (name, fg, bg) in text {
                let c = contrast(fg, bg);
                assert!(c >= 4.5, "{name} {:?}: {c:.2}", t.mode);
            }
            let controls = [
                ("borda do toggle ligado", t.accent, t.surface),
                ("borda do toggle desligado", t.border_control(), t.surface),
                ("icone expense", t.expense, t.surface),
                ("icone warning", t.warning, t.surface),
                ("icone income", t.income, t.surface),
            ];
            for (name, fg, bg) in controls {
                let c = contrast(fg, bg);
                assert!(c >= 3.0, "{name} {:?}: {c:.2}", t.mode);
            }
        }
        // A tabela 7.3 mede o toggle ligado em 6,72 e 7,76.
        let near = |got: f64, want: f64| (got - want).abs() < 0.01;
        assert!(near(contrast(DARK.accent_200, DARK.accent_800), 6.72));
        assert!(near(contrast(LIGHT.accent_200, LIGHT.accent_800), 7.76));
    }

    /// Relatorio 7.3, "texto por superficie": todo texto do hub sobre os seis fundos onde pode
    /// aparecer, incluindo a linha sob o mouse e pressionada (`state` de 4 e 8 % sobre `surface`).
    #[test]
    fn texto_por_superficie_passa_4_5() {
        for t in [&DARK, &LIGHT] {
            let surfaces = [
                ("bg", t.bg),
                ("lateral", t.side()),
                ("surface", t.surface),
                ("surface_2", t.surface_2),
                ("hover", composite(t.state(false), t.surface)),
                ("pressionado", composite(t.state(true), t.surface)),
            ];
            let texts = [
                ("fg", t.text),
                ("text_soft", t.text_soft()),
                ("text_muted", t.text_muted),
                ("text_accent", t.text_accent()),
                ("income_fg", t.income_fg),
                ("expense_fg", t.expense_fg),
                ("warning", t.warning),
            ];
            for (fundo, bg) in surfaces {
                for (name, fg) in texts {
                    let c = contrast(fg, bg);
                    assert!(c >= 4.5, "{name}/{fundo} {:?}: {c:.2}", t.mode);
                }
            }
        }
        // Lacunas registradas (asteriscos da tabela): `idle` so serve para desabilitado, e o
        // `accent` cru do claro cai abaixo do piso em caixa interna e linha pressionada, que e o
        // motivo de `text_accent` existir.
        assert!(contrast(DARK.text_disabled(), DARK.surface) < 4.5);
        assert!(contrast(LIGHT.text_disabled(), LIGHT.surface_2) < 4.5);
        assert!(contrast(LIGHT.accent, LIGHT.surface_2) < 4.5);
        assert!(contrast(LIGHT.accent, composite(LIGHT.state(true), LIGHT.surface)) < 4.5);
        // Fechar da barra de titulo: #f2f2f2 sobre #c42b1c.
        let near = |got: f64, want: f64| (got - want).abs() < 0.01;
        for t in [&DARK, &LIGHT] {
            assert!(near(contrast(t.close_hover_text, t.close_hover), 5.06));
        }
    }

    #[test]
    fn token_segue_o_modo() {
        assert_eq!(DARK.token("fuchsia"), Color::from_rgb8(0xe0, 0x93, 0xcf));
        assert_eq!(LIGHT.token("fuchsia"), Color::from_rgb8(0xae, 0x53, 0x9d));
        assert_eq!(DARK.token("neon"), DARK.token("slate"));
    }
}
