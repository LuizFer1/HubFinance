//! Controles: botoes de acao e de icone, segmentado, chip, linha de lista, campo com "Copiar".
//! O estilo por `Status` mora em `theme` (testado la); aqui a altura, o respiro e o texto.

use iced::widget::{Button, button, container, row, text, tooltip};
use iced::{Alignment, Element, Length};

use crate::ui::theme::{self, ButtonKind, ButtonSize, Tokens};
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
