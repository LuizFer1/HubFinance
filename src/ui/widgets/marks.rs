//! Marcas pequenas sobre a superficie do card: tags, tiles de icone e barras.

use iced::widget::text::Wrapping;
use iced::widget::{Row, Space, container, row, text};
use iced::{Alignment, Color, Element, Length};

use super::dot;
use crate::ui::theme::{self, Tokens, Tone, radius, space, type_scale};
use crate::ui::{fonts, icons};

/// O que a barra desenha.
#[derive(Clone, Copy, Debug)]
pub enum Bar<'b> {
    /// Trilho opcional com o preenchimento de `value` (0..=1) a esquerda. `outlined` desenha so
    /// o contorno do preenchimento (mes parcial do historico).
    Fraction {
        value: f32,
        color: Color,
        track: Option<Color>,
        outlined: bool,
    },
    /// Barra dividida em partes proporcionais aos pesos, 3 px entre elas. Peso zero nao entra;
    /// sem nenhum peso, so o trilho.
    Parts {
        parts: &'b [(f64, Color)],
        track: Color,
    },
    /// Medidor de N segmentos iguais, 4 px entre eles, cada um com a sua fracao (0..=1).
    Segments {
        values: &'b [f32],
        color: Color,
        track: Color,
    },
}

/// Barra de `height` px (4, 6 ou 10 na regua; o Resumo usa 5 e 8). Raio 4, ou meia altura se
/// a barra for mais baixa que 8, para a ponta continuar redonda. `FillPortion` e `u16`, entao
/// as fracoes viram milesimos.
pub fn bar<'a, M: 'a>(spec: Bar<'_>, height: f32) -> Element<'a, M> {
    let r = radius::BAR.min(height / 2.0);
    match spec {
        Bar::Fraction {
            value,
            color,
            track,
            outlined,
        } => fraction_bar(value, color, track, outlined, height, r),
        Bar::Parts { parts, track } => {
            let Some(pieces) = parts_plan(parts) else {
                return container(Space::new())
                    .width(Length::Fill)
                    .height(height)
                    .style(theme::fill(track, r))
                    .into();
            };
            let segments = pieces.into_iter().map(|(p, color)| {
                container(Space::new())
                    .width(Length::FillPortion(p))
                    .height(height)
                    .style(theme::fill(color, r))
                    .into()
            });
            Row::with_children(segments)
                .spacing(3)
                .width(Length::Fill)
                .into()
        }
        Bar::Segments {
            values,
            color,
            track,
        } => {
            if values.is_empty() {
                return Space::new().height(height).into();
            }
            Row::with_children(
                values
                    .iter()
                    .map(|v| fraction_bar(*v, color, Some(track), false, height, r)),
            )
            .spacing(space::XS)
            .width(Length::Fill)
            .into()
        }
    }
}

fn fraction_bar<'a, M: 'a>(
    value: f32,
    color: Color,
    track: Option<Color>,
    outlined: bool,
    height: f32,
    r: f32,
) -> Element<'a, M> {
    let filled = (value.clamp(0.0, 1.0) * 1000.0).round() as u16;
    let inner: Element<'a, M> = if filled == 0 {
        Space::new().width(Length::Fill).height(height).into()
    } else {
        let piece = container(Space::new())
            .width(Length::FillPortion(filled))
            .height(height);
        let piece = if outlined {
            piece.style(theme::outlined(color, r))
        } else {
            piece.style(theme::fill(color, r))
        };
        if filled >= 1000 {
            piece.into()
        } else {
            row![
                piece,
                Space::new()
                    .width(Length::FillPortion(1000 - filled))
                    .height(height)
            ]
            .into()
        }
    };
    let bar = container(inner).width(Length::Fill).height(height);
    match track {
        Some(track) => bar.style(theme::fill(track, r)).into(),
        None => bar.into(),
    }
}

/// Pedacos da barra de partes, em milesimos, sem os de peso zero. `None` quando nenhum peso e
/// positivo: a barra mostra so o trilho.
fn parts_plan(parts: &[(f64, Color)]) -> Option<Vec<(u16, Color)>> {
    let weights: Vec<f64> = parts.iter().map(|(w, _)| *w).collect();
    let pieces: Vec<(u16, Color)> = portions(&weights)
        .into_iter()
        .zip(parts)
        .filter(|(p, _)| *p > 0)
        .map(|(p, (_, color))| (p, *color))
        .collect();
    (!pieces.is_empty()).then_some(pieces)
}

/// Milesimos de cada peso no total (negativo conta zero). Peso positivo minimo vale 1, para a
/// parte pequena nao sumir; peso zero fica zero e sai da barra.
fn portions(weights: &[f64]) -> Vec<u16> {
    let total: f64 = weights.iter().map(|w| w.max(0.0)).sum();
    weights
        .iter()
        .map(|w| {
            let w = w.max(0.0);
            if total <= 0.0 || w <= 0.0 {
                0
            } else {
                (w * 1000.0 / total).clamp(1.0, 1000.0) as u16
            }
        })
        .collect()
}

/// Cor do passo `index` (0..) com o passo atual `current`: feito em `accent_700`, atual em
/// `accent`, futuro em `neutral_800`.
fn step_color(t: &Tokens, index: usize, current: usize) -> Color {
    match index.cmp(&current) {
        std::cmp::Ordering::Less => t.accent_700,
        std::cmp::Ordering::Equal => t.accent,
        std::cmp::Ordering::Greater => t.neutral_800,
    }
}

/// Progresso de um fluxo em passos (dialogo com mais de uma etapa): `total` segmentos de 3 px,
/// 4 px entre eles, raio de meia altura. `current` comeca em 0.
pub fn progress_steps<'a, M: 'a>(
    t: &'static Tokens,
    current: usize,
    total: usize,
) -> Element<'a, M> {
    Row::with_children((0..total).map(|i| {
        container(Space::new())
            .width(Length::Fill)
            .height(3)
            .style(theme::fill(step_color(t, i, current), 1.5))
            .into()
    }))
    .spacing(space::XS)
    .width(Length::Fill)
    .into()
}

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
pub(super) const TAG_HEIGHT: f32 = 20.0;

/// Tom da tag. Neutra (`text_soft` sobre `neutral_800`) para estado ou fato da linha ("Oculto",
/// "Transferência", "Mensal"); acento (`accent_300` sobre `accent_900`) so para o que marca o
/// agora ("hoje", a % da meta da emergencia): o dourado e o foco do bloco, nunca enfeite;
/// receita, despesa e atencao na cor do tom sobre a tinta de 10 % da superficie.
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
        .wrapping(Wrapping::None)
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
        .spacing(space::XS)
        .align_y(Alignment::Center),
        t.token_tint(token, 0.16),
        TAG_HEIGHT,
        space::S,
    )
}

/// Tom da tag compacta do Resumo. Os tres pares sao os que o design do dono desenha: acento
/// ("hoje", recorrencia), neutro no divisor (tag do lancamento) e forte, `fg` no divisor
/// ("↑ 38% do normal", que precisa pesar mais que a neutra).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompactTone {
    Accent,
    Neutral,
    Strong,
}

/// (texto, fundo) da tag compacta.
fn compact_colors(t: &Tokens, tone: CompactTone) -> (Color, Color) {
    match tone {
        CompactTone::Accent => (t.text_accent(), t.accent_900),
        // `text_soft`: o `text_muted` sobre o divisor fica abaixo de 4,5:1 no claro.
        CompactTone::Neutral => (t.text_soft(), t.divider()),
        CompactTone::Strong => (t.text, t.divider()),
    }
}

/// Tag compacta do Resumo: 11 Regular, sem altura fixa (o respiro faz a altura: 18 px com
/// `[2, 8]`, 20 com `[3, 9]` no "hoje"). Fica fora da regua da `tag` de proposito: o Resumo do
/// dono nao muda de tamanho nem de peso na Fase 7.
pub fn tag_compact<'a, M: 'a>(
    t: &'static Tokens,
    tone: CompactTone,
    label: &str,
    padding: [u16; 2],
) -> Element<'a, M> {
    let (fg, bg) = compact_colors(t, tone);
    container(
        text(label.to_string())
            .size(type_scale::MICRO)
            .wrapping(Wrapping::None)
            .color(fg),
    )
    .padding(padding)
    .style(theme::fill(bg, radius::PILL))
    .into()
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
    fn partes_proporcionais_em_milesimos() {
        assert_eq!(portions(&[]), Vec::<u16>::new());
        assert_eq!(portions(&[0.0, 0.0]), vec![0, 0]);
        assert_eq!(portions(&[300.0, 100.0]), vec![750, 250]);
        assert_eq!(portions(&[1.0, 1_000_000.0]), vec![1, 999]);
        assert_eq!(portions(&[-5.0, 10.0]), vec![0, 1000]);
    }

    #[test]
    fn parte_de_peso_zero_sai_e_tudo_zerado_fica_so_o_trilho() {
        let (a, b, c) = (Color::WHITE, Color::BLACK, Color::from_rgb(1.0, 0.0, 0.0));
        assert_eq!(
            parts_plan(&[(300.0, a), (0.0, b), (100.0, c)]),
            Some(vec![(750, a), (250, c)])
        );
        assert_eq!(parts_plan(&[(0.0, a), (0.0, b)]), None);
        assert_eq!(parts_plan(&[(-5.0, a)]), None);
        assert_eq!(parts_plan(&[]), None);
    }

    #[test]
    fn barra_monta_fracao_partes_e_segmentos() {
        let c = Color::WHITE;
        for value in [-1.0, 0.0, 0.4, 1.0, 2.0] {
            for outlined in [true, false] {
                let _: Element<'_, ()> = bar(
                    Bar::Fraction {
                        value,
                        color: c,
                        track: Some(c),
                        outlined,
                    },
                    6.0,
                );
            }
        }
        for parts in [
            &[][..],
            &[(0.0, c), (0.0, c)][..],
            &[(3.0, c), (1.0, c)][..],
        ] {
            let _: Element<'_, ()> = bar(Bar::Parts { parts, track: c }, 8.0);
        }
        for values in [&[][..], &[1.0, 0.5, 0.0][..]] {
            let _: Element<'_, ()> = bar(
                Bar::Segments {
                    values,
                    color: c,
                    track: c,
                },
                10.0,
            );
        }
    }

    #[test]
    fn passos_feitos_atual_e_futuros() {
        for t in [&DARK, &LIGHT] {
            assert_eq!(step_color(t, 0, 1), t.accent_700);
            assert_eq!(step_color(t, 1, 1), t.accent);
            assert_eq!(step_color(t, 2, 1), t.neutral_800);
            for total in [0, 1, 3] {
                let _: Element<'_, ()> = progress_steps(t, 1, total);
            }
        }
    }

    /// O Resumo do dono continua com as tags de antes da Fase 7: forte e neutra no divisor,
    /// acento em `accent_900`, todas passando 4,5.
    #[test]
    fn tag_compacta_mantem_as_cores_do_resumo() {
        for t in [&DARK, &LIGHT] {
            assert_eq!(
                compact_colors(t, CompactTone::Strong),
                (t.text, t.divider())
            );
            assert_eq!(
                compact_colors(t, CompactTone::Neutral),
                (t.text_soft(), t.divider())
            );
            assert_eq!(
                compact_colors(t, CompactTone::Accent),
                (t.accent_300, t.accent_900)
            );
            for tone in [
                CompactTone::Accent,
                CompactTone::Neutral,
                CompactTone::Strong,
            ] {
                let (fg, bg) = compact_colors(t, tone);
                assert!(contrast(fg, bg) >= 4.5, "{tone:?} {:?}", t.mode);
                let _: Element<'_, ()> = tag_compact(t, tone, "hoje", [3, 9]);
            }
        }
    }

    #[test]
    fn tags_montam_em_todo_tom() {
        for t in [&DARK, &LIGHT] {
            for tone in TONES {
                let _: Element<'_, ()> = tag(t, tone, "Oculto");
                let _: Element<'_, ()> = tag_icon(t, tone, icons::REPEAT, "Mensal");
            }
            let _: Element<'_, ()> = color_tag(t, "magenta");
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
