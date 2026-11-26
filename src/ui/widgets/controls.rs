//! Controles: botoes de acao e de icone, segmentado, chip, linha de lista, campo com "Copiar".
//! O estilo por `Status` mora em `theme` (testado la); aqui a altura, o respiro e o texto.

use iced::widget::text::Wrapping;
use iced::widget::{Button, Row, button, container, mouse_area, row, text, tooltip};
use iced::{Alignment, Color, Element, Length, Padding};

use crate::ui::theme::{self, ButtonKind, ButtonSize, Tokens, radius, size, space, type_scale};
use crate::ui::{fonts, icons};

/// Botao de acao: o `theme::button(kind, size)` com a altura, o padding lateral e o tamanho de
/// texto do `ButtonSize`, rotulo em Medium. Devolve o `Button` para a tela por `on_press` ou
/// `on_press_maybe` (sem mensagem ele fica desabilitado, com a cara de `Disabled`) e, quando
/// precisa, `width`.
pub fn action_button<'a, M: 'a>(
    t: &'static Tokens,
    kind: ButtonKind,
    size: ButtonSize,
    label: impl text::IntoFragment<'a>,
) -> Button<'a, M> {
    action_button_with(t, kind, size, label_text(size, label))
}

/// `action_button` com um icone antes do rotulo ("Remover", "Copiar", "Limpar filtros"). O
/// icone herda a cor do texto do botao, entao apaga junto quando desabilitado.
pub fn action_button_icon<'a, M: 'a>(
    t: &'static Tokens,
    kind: ButtonKind,
    size: ButtonSize,
    glyph: &'static str,
    label: impl text::IntoFragment<'a>,
) -> Button<'a, M> {
    action_button_with(
        t,
        kind,
        size,
        row![
            icons::icon_inherit(glyph, size.text_size()),
            label_text(size, label)
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
}

/// `action_button` com conteudo livre (icone depois do rotulo, linha inteira clicavel).
pub fn action_button_with<'a, M: 'a>(
    t: &'static Tokens,
    kind: ButtonKind,
    size: ButtonSize,
    content: impl Into<Element<'a, M>>,
) -> Button<'a, M> {
    button(
        container(content)
            .height(Length::Fill)
            .align_y(Alignment::Center),
    )
    .height(size.height())
    .padding([0.0, size.padding_x()])
    .style(theme::button(t, kind, size))
}

/// Botao so de icone, quadrado no `ButtonSize` (28, 36 ou 44), raio 8, com a dica embaixo
/// depois de `theme::TOOLTIP_DELAY` (sem o atraso ela pisca sob o mouse que so atravessa o
/// icone). O icone herda a cor do estilo, entao apaga junto quando `on_press` e `None`.
pub fn icon_button<'a, M: Clone + 'a>(
    t: &'static Tokens,
    glyph: &'static str,
    size: ButtonSize,
    tip: impl text::IntoFragment<'a>,
    on_press: Option<M>,
) -> Element<'a, M> {
    let side = size.height();
    let glyph_size = match size {
        ButtonSize::Compact => 16.0,
        ButtonSize::Regular => 18.0,
        ButtonSize::Large => 20.0,
    };
    let b = button(container(icons::icon_inherit(glyph, glyph_size)).center(Length::Fill))
        .width(side)
        .height(side)
        .padding(0)
        .style(theme::icon_button(t))
        .on_press_maybe(on_press);
    tooltip(
        b,
        container(text(tip).size(12).color(t.text))
            .padding([6, 10])
            .style(theme::menu(t)),
        tooltip::Position::Bottom,
    )
    .gap(4)
    .delay(theme::TOOLTIP_DELAY)
    .into()
}

/// Variante do segmentado. `Pill` (ativo neutro, no divisor) e o seletor de tema e de conta,
/// num trilho `surface` sobre o fundo da pagina; `PillInset` e a mesma peca dentro de card,
/// com trilho `surface_2` (o `surface` sumiria no card). `Bordered` (ativo no acento) e o
/// Sim | Nao e o filtro de tipo: trilho so com contorno.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SegmentedVariant {
    Pill,
    PillInset,
    Bordered,
}

/// Segmentado de N opcoes `(rotulo, ativa, mensagem)`. `size` da a altura da opcao (compacto
/// 28, padrao 36; o grande nao existe aqui e vale como padrao). `fill` divide a largura
/// igualmente (lateral, Sim | Nao); sem ele cada opcao tem a largura do rotulo mais o respiro.
pub fn segmented<'a, M: Clone + 'a>(
    t: &'static Tokens,
    variant: SegmentedVariant,
    size: ButtonSize,
    fill: bool,
    options: impl IntoIterator<Item = (&'a str, bool, M)>,
) -> Element<'a, M> {
    let height = match size {
        ButtonSize::Compact => size::CTRL_S,
        ButtonSize::Regular | ButtonSize::Large => size::CTRL_M,
    };
    let bordered = variant == SegmentedVariant::Bordered;
    // O contornado e mais denso (rotulo 12 Medium, respiro 12, trilho de 3); a pilula usa 13
    // Regular, respiro 14 e trilho de 4 com 4 entre as opcoes.
    let (pad_x, rail_pad, gap) = if bordered {
        (12.0, 3.0, 0.0)
    } else {
        (14.0, space::XS, space::XS)
    };
    let option = |(label, active, message): (&'a str, bool, M)| -> Element<'a, M> {
        let label = if bordered {
            text(label)
                .size(type_scale::META)
                .font(fonts::INTER_MEDIUM)
                .line_height(1.0)
        } else {
            text(label).size(type_scale::LABEL).line_height(1.0)
        };
        let content = container(label).height(height).align_y(Alignment::Center);
        let content = if fill {
            content.center_x(Length::Fill)
        } else {
            content.padding([0.0, pad_x])
        };
        let b = button(content).padding(0).on_press(message);
        let b = if bordered {
            b.style(theme::seg_option(t, active))
        } else {
            b.style(theme::segment(t, active))
        };
        if fill { b.width(Length::Fill) } else { b }.into()
    };
    let line = Row::with_children(options.into_iter().map(option)).spacing(gap);
    let rail = container(if fill { line.width(Length::Fill) } else { line }).padding(rail_pad);
    let rail = match variant {
        SegmentedVariant::Pill => rail.style(theme::fill(t.surface, radius::CONTROL)),
        SegmentedVariant::PillInset => rail.style(theme::fill(t.surface_2, radius::CONTROL)),
        SegmentedVariant::Bordered => rail.style(theme::outlined(t.divider(), radius::CONTROL)),
    };
    if fill {
        rail.width(Length::Fill).into()
    } else {
        rail.into()
    }
}

/// Forma do chip: pilula (autor) ou raio 8 (categoria da barra de filtros, que abre menu).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChipShape {
    Pill,
    Rounded,
}

/// Chip de filtro de 36 px: ativo em `accent_900` com borda `accent` e texto `accent_200`,
/// inativo em `surface` com borda no divisor. Respiro 12 dos lados; o chip com avatar troca o
/// da esquerda pelo `padding` do `Button` devolvido.
pub fn chip<'a, M: 'a>(
    t: &'static Tokens,
    content: impl Into<Element<'a, M>>,
    active: bool,
    shape: ChipShape,
) -> Button<'a, M> {
    let b = button(
        container(content)
            .height(Length::Fill)
            .align_y(Alignment::Center),
    )
    .height(size::CTRL_M)
    .padding([0.0, space::M]);
    match shape {
        ChipShape::Pill => b.style(theme::chip(t, active)),
        ChipShape::Rounded => b.style(theme::filter_button(t, active)),
    }
}

/// Estado de hover de uma linha e as mensagens que o mantem. O hover vem do estado da tela (uma
/// linha por vez), e nao do `Status` de um botao: a linha tem botoes dentro ("Remover",
/// "Apagar"), e botao dentro de botao nao recebe o clique.
pub struct RowHover<M> {
    pub hovered: bool,
    pub enter: M,
    pub exit: M,
}

/// Linha de lista com hover a 4 % (`theme::list_row`). `height` fixa a linha (44 simples, 52
/// de duas linhas; as Reservas ainda usam 56) ou, com `None`, ela cresce com o conteudo.
/// `radius` e 8, ou 0 na tabela que vai de borda a borda do card.
pub fn list_row<'a, M: Clone + 'a>(
    t: &'static Tokens,
    content: impl Into<Element<'a, M>>,
    hover: RowHover<M>,
    height: Option<f32>,
    padding: impl Into<Padding>,
    radius: f32,
) -> Element<'a, M> {
    let body = container(content)
        .padding(padding)
        .width(Length::Fill)
        .align_y(Alignment::Center)
        .style(theme::list_row(t, hover.hovered, radius));
    let body = match height {
        Some(h) => body.height(h),
        None => body,
    };
    mouse_area(body)
        .on_enter(hover.enter)
        .on_exit(hover.exit)
        .into()
}

/// "Copiar" fantasma compacto que vira "Copiado" com o check enquanto `copied` (1,6 s, quem
/// conta e a tela). Sem `on_copy` fica desabilitado (nada a copiar).
pub fn copy_button<'a, M: Clone + 'a>(
    t: &'static Tokens,
    copied: bool,
    on_copy: Option<M>,
) -> Element<'a, M> {
    action_button_icon(
        t,
        ButtonKind::Ghost,
        ButtonSize::Compact,
        if copied { icons::CHECK } else { icons::COPY },
        if copied { "Copiado" } else { "Copiar" },
    )
    .on_press_maybe(on_copy)
    .into()
}

/// Campo de 44 px com "Copiar": icone `(glifo, cor)`, o valor em Medium no tamanho `size` (cortado na borda se
/// ainda nao couber: o iced 0.14 nao tem reticencias) e o botao a direita. Sem valor mostra
/// `placeholder` em `text_muted` e o botao fica desabilitado.
pub fn field_copy<'a, M: Clone + 'a>(
    t: &'static Tokens,
    (glyph, glyph_color): (&'static str, Color),
    value: Option<String>,
    size: f32,
    placeholder: &str,
    copied: bool,
    on_copy: M,
) -> Element<'a, M> {
    let enabled = value.is_some();
    let shown = match value {
        Some(v) => text(v).size(size).font(fonts::INTER_MEDIUM).color(t.text),
        None => text(placeholder.to_string())
            .size(type_scale::SUPPORT)
            .color(t.text_muted),
    };
    let value = container(shown.wrapping(Wrapping::None))
        .width(Length::Fill)
        .clip(true);
    let copy = copy_button(t, copied, enabled.then_some(on_copy));
    container(
        row![icons::icon(glyph, 15.0, glyph_color), value, copy]
            .spacing(10)
            .align_y(Alignment::Center),
    )
    .height(size::CTRL_L)
    .width(Length::Fill)
    .padding(Padding {
        top: 0.0,
        right: 6.0,
        bottom: 0.0,
        left: 14.0,
    })
    .align_y(Alignment::Center)
    .style(theme::field(t))
    .into()
}

fn label_text<'a>(size: ButtonSize, label: impl text::IntoFragment<'a>) -> text::Text<'a> {
    text(label)
        .size(size.text_size())
        .font(fonts::INTER_MEDIUM)
        .line_height(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::{DARK, LIGHT};

    const KINDS: [ButtonKind; 6] = [
        ButtonKind::Primary,
        ButtonKind::Secondary,
        ButtonKind::Ghost,
        ButtonKind::Link,
        ButtonKind::Danger,
        ButtonKind::DangerFilled,
    ];
    const SIZES: [ButtonSize; 3] = [ButtonSize::Compact, ButtonSize::Regular, ButtonSize::Large];

    #[test]
    fn botao_de_icone_monta_nos_tres_tamanhos_com_e_sem_acao() {
        for t in [&DARK, &LIGHT] {
            for size in SIZES {
                let _ = icon_button(t, icons::EYE, size, "Ocultar valores", Some(()));
                let _ = icon_button::<()>(t, icons::EYE, size, "Ocultar valores", None);
            }
        }
    }

    #[test]
    fn segmentado_monta_em_toda_variante_com_duas_e_tres_opcoes() {
        let variants = [
            SegmentedVariant::Pill,
            SegmentedVariant::PillInset,
            SegmentedVariant::Bordered,
        ];
        for t in [&DARK, &LIGHT] {
            for variant in variants {
                for size in SIZES {
                    for fill in [true, false] {
                        let _ =
                            segmented(t, variant, size, fill, [("A", true, ()), ("B", false, ())]);
                        let _ = segmented(
                            t,
                            variant,
                            size,
                            fill,
                            [("A", false, 1), ("B", true, 2), ("C", false, 3)],
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn chip_monta_ativo_e_inativo_nas_duas_formas() {
        for t in [&DARK, &LIGHT] {
            for shape in [ChipShape::Pill, ChipShape::Rounded] {
                for active in [true, false] {
                    let _: Element<'_, ()> =
                        chip(t, text("Todos"), active, shape).on_press(()).into();
                }
            }
        }
    }

    #[test]
    fn linha_de_lista_monta_com_e_sem_altura() {
        for t in [&DARK, &LIGHT] {
            for hovered in [true, false] {
                let hover = || RowHover {
                    hovered,
                    enter: 1,
                    exit: 2,
                };
                let _ = list_row(t, text("Linha"), hover(), Some(52.0), [0, 20], 0.0);
                let _ = list_row(t, text("Linha"), hover(), None, [10, 8], 8.0);
            }
        }
    }

    #[test]
    fn campo_com_copiar_monta_com_e_sem_valor() {
        for t in [&DARK, &LIGHT] {
            for copied in [true, false] {
                let _ = field_copy(
                    t,
                    (icons::LOCK_SIMPLE, t.income_fg),
                    Some("https://192.168.0.2:7777".into()),
                    15.0,
                    "sem endereço de rede",
                    copied,
                    (),
                );
                let _ = field_copy(
                    t,
                    (icons::LOCK_SIMPLE, t.income_fg),
                    None,
                    15.0,
                    "-",
                    copied,
                    (),
                );
                let _ = copy_button(t, copied, Some(()));
                let _ = copy_button::<()>(t, copied, None);
            }
        }
    }

    #[test]
    fn botao_de_acao_monta_em_todo_papel_e_tamanho() {
        for t in [&DARK, &LIGHT] {
            for kind in KINDS {
                for size in SIZES {
                    let _: Element<'_, ()> =
                        action_button(t, kind, size, "Salvar").on_press(()).into();
                    let _: Element<'_, ()> =
                        action_button_icon(t, kind, size, icons::COPY, "Copiar").into();
                    let _: Element<'_, ()> =
                        action_button_with(t, kind, size, text("Ver todos")).into();
                }
            }
        }
    }
}
