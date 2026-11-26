//! Marcas pequenas que acompanham um texto: tags e tiles de icone, sobre a superficie do card.

use iced::widget::{container, row, text};
use iced::{Alignment, Color, Element};

use super::dot;
use crate::ui::theme::{self, Tokens, Tone, radius, space, type_scale};
use crate::ui::{fonts, icons};

/// Tinta do tile: a cor a 16 % sobre a superficie (antes 15, 16 ou 18 % conforme a tela).
const TILE_TINT: f32 = 0.16;

/// Tile de icone: quadrado de `size` (28, 32 ou 36 na regua; o Resumo usa 30), raio 6, fundo
/// `tint` a 16 % sobre a superficie e o icone na cor `icon` (16 px, 18 no de 36). Icone e tinta
/// sao separados porque o deposito pinta o icone em `accent_300` sobre a tinta do `accent`.
pub fn tile<'a, M: 'a>(
    t: &'static Tokens,
    glyph: &'static str,
    icon: Color,
    tint: Color,
    size: f32,
) -> Element<'a, M> {
    tile_box(glyph, icon, theme::mix(t.surface, tint, TILE_TINT), size)
}

/// Tile no token da categoria ou da caixinha: icone e tinta na mesma cor.
pub fn token_tile<'a, M: 'a>(
    t: &'static Tokens,
    glyph: &'static str,
    token: &str,
    size: f32,
) -> Element<'a, M> {
    let color = t.token(token);
    tile(t, glyph, color, color, size)
}

/// Tile sem cor (lancamento futuro sem categoria): icone no apoio sobre o divisor.
pub fn neutral_tile<'a, M: 'a>(
    t: &'static Tokens,
    glyph: &'static str,
    size: f32,
) -> Element<'a, M> {
    tile_box(glyph, t.text_soft(), t.divider(), size)
}

fn tile_box<'a, M: 'a>(
    glyph: &'static str,
    icon: Color,
    background: Color,
    size: f32,
) -> Element<'a, M> {
    let glyph_size = if size >= 36.0 { 18.0 } else { 16.0 };
    container(icons::icon(glyph, glyph_size, icon))
        .center(size)
        .style(theme::fill(background, radius::INSET))
        .into()
}

/// Altura da tag (relatorio 9).
const TAG_HEIGHT: f32 = 20.0;

/// Tom da tag. Neutra (`text_soft` sobre `neutral_800`) para estado que nao conta ("Oculto",
/// "Transferência"); acento (`accent_300` sobre `accent_900`) para o que e do mes ("hoje",
/// "Mensal"); receita, despesa e atencao na cor do tom sobre a tinta de 10 % da superficie.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TagTone {
    Neutral,
    Accent,
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "sem tag de receita nas telas ainda")
    )]
    Income,
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "sem tag de despesa nas telas ainda")
    )]
    Expense,
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "sem tag de atencao nas telas ainda")
    )]
    Warning,
}

/// (texto, fundo) de cada tom. Os pares passam 4,5 nos dois temas (teste abaixo).
fn tag_colors(t: &Tokens, tone: TagTone) -> (Color, Color) {
    match tone {
        TagTone::Neutral => (t.text_soft(), t.neutral_800),
        TagTone::Accent => (t.text_accent(), t.accent_900),
        TagTone::Income => (t.income_fg, t.tint(Tone::Income, t.surface)),
        TagTone::Expense => (t.expense_fg, t.tint(Tone::Expense, t.surface)),
        TagTone::Warning => (t.warning, t.tint(Tone::Warning, t.surface)),
    }
}

/// Pilula de `height` px com o conteudo centrado e `pad_x` dos lados. Base de toda tag.
fn pill<'a, M: 'a>(
    content: impl Into<Element<'a, M>>,
    background: Color,
    height: f32,
    pad_x: f32,
) -> Element<'a, M> {
    container(content)
        .height(height)
        .padding([0.0, pad_x])
        .align_y(Alignment::Center)
        .style(theme::fill(background, radius::PILL))
        .into()
}

fn tag_text<'a>(label: &str, color: Color) -> text::Text<'a> {
    text(label.to_string())
        .size(type_scale::MICRO)
        .font(fonts::INTER_MEDIUM)
        .line_height(1.0)
        .color(color)
}

/// Tag: pilula de 20 px, 11 Medium, no tom.
pub fn tag<'a, M: 'a>(t: &'static Tokens, tone: TagTone, label: &str) -> Element<'a, M> {
    let (fg, bg) = tag_colors(t, tone);
    pill(tag_text(label, fg), bg, TAG_HEIGHT, space::S)
}

/// Tag com um icone de 11 na cor do texto antes do rotulo ("Mensal" com `repeat`).
pub fn tag_icon<'a, M: 'a>(
    t: &'static Tokens,
    tone: TagTone,
    glyph: &'static str,
    label: &str,
) -> Element<'a, M> {
    let (fg, bg) = tag_colors(t, tone);
    pill(
        row![
            icons::icon(glyph, type_scale::MICRO, fg),
            tag_text(label, fg)
        ]
        .spacing(space::XS)
        .align_y(Alignment::Center),
        bg,
        TAG_HEIGHT,
        space::S,
    )
}

/// Tag da cor da pessoa: fundo a cor a 16 % sobre a superficie, ponto de 6 na cor e o nome da
/// cor ("Magenta") no texto de apoio. O nome na propria cor ficava abaixo de 4,5:1 em varios
/// tokens do claro; o texto sai da cor, que fica no ponto e na tinta.
pub fn color_tag<'a, M: 'a>(t: &'static Tokens, token: &str) -> Element<'a, M> {
    pill(
        row![
            dot(t.token(token), 6.0),
            tag_text(crate::dashboard::colors::color_name(token), t.text_soft()),
        ]
        .spacing(5)
        .align_y(Alignment::Center),
        t.token_tint(token, 0.16),
        TAG_HEIGHT,
        space::S,
    )
}

/// Chip "Parcial · até 19 nov" do cabecalho (texto pronto do modelo): a pilula de acento maior,
/// 26 px com 12 Regular, ao lado dos controles de 36 do cabecalho. Fica fora da regua da tag de
/// proposito: e o Resumo do dono, que a Fase 7 nao muda de tamanho.
pub fn partial_tag<'a, M: 'a>(t: &'static Tokens, label: &str) -> Element<'a, M> {
    pill(
        text(label.to_string())
            .size(type_scale::META)
            .line_height(1.0)
            .color(t.text_accent()),
        t.accent_900,
        26.0,
        space::M,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::{DARK, LIGHT};

    const TONES: [TagTone; 5] = [
        TagTone::Neutral,
        TagTone::Accent,
        TagTone::Income,
        TagTone::Expense,
        TagTone::Warning,
    ];

    /// Luminancia relativa WCAG, para conferir o par de cada tom.
    fn contrast(a: Color, b: Color) -> f32 {
        let lum = |c: Color| {
            let ch = |v: f32| {
                if v <= 0.03928 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * ch(c.r) + 0.7152 * ch(c.g) + 0.0722 * ch(c.b)
        };
        let (x, y) = (lum(a), lum(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn tags_montam_em_todo_tom() {
        for t in [&DARK, &LIGHT] {
            for tone in TONES {
                let _: Element<'_, ()> = tag(t, tone, "Oculto");
                let _: Element<'_, ()> = tag_icon(t, tone, icons::REPEAT, "Mensal");
            }
            let _: Element<'_, ()> = color_tag(t, "magenta");
            let _: Element<'_, ()> = partial_tag(t, "Parcial · até 19 nov");
            for size in [28.0, 30.0, 32.0, 36.0] {
                let _: Element<'_, ()> = tile(t, icons::ARROW_DOWN, t.accent_300, t.accent, size);
                let _: Element<'_, ()> = token_tile(t, icons::REPEAT, "slate", size);
                let _: Element<'_, ()> = neutral_tile(t, icons::REPEAT, size);
            }
        }
    }

    #[test]
    fn texto_da_tag_passa_4_5_em_todo_tom() {
        for t in [&DARK, &LIGHT] {
            for tone in TONES {
                let (fg, bg) = tag_colors(t, tone);
                let c = contrast(fg, bg);
                assert!(c >= 4.5, "{tone:?} {:?}: {c:.2}", t.mode);
            }
        }
    }
}
