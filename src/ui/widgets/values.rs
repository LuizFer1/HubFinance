//! Numeros grandes: o valor partido em linha de base (`value_line`), o dinheiro com "R$" e
//! centavos menores (`money`) e o bloco rotulo + valor + nota (`stat`).

use iced::widget::text::Wrapping;
use iced::widget::{Column, column, container, row, text};
use iced::{Alignment, Color, Element, Font, Padding};

use crate::dashboard::money::BigMoney;
use crate::ui::fonts;
use crate::ui::theme::{Tokens, type_scale};

/// Respiro que o pedaco menor precisa embaixo para a linha de base casar com a do maior, com
/// altura de linha 1,0 (o iced nao alinha por linha de base). Na Inter a base fica a 0,1367 em
/// do fundo da caixa (ascendente 0,96875 e descendente 0,2421875 centrados em 1 em).
pub fn baseline_pad(big: f32, small: f32) -> f32 {
    0.1367 * (big - small).max(0.0)
}

/// Valor partido em linha de base: `(texto, tamanho, fonte, cor)` de cada pedaco, o maior com
/// `big` px. Pedaco vazio nao entra (sinal ou sufixo ausente).
pub fn value_line<'a, M: 'a>(
    pieces: Vec<(String, f32, Font, Color)>,
    big: f32,
    spacing: f32,
) -> Element<'a, M> {
    let mut line = row![].spacing(spacing).align_y(Alignment::End);
    for (content, size, font, color) in pieces {
        if content.is_empty() {
            continue;
        }
        line = line.push(
            container(
                text(content)
                    .size(size)
                    .font(font)
                    .line_height(1.0)
                    .wrapping(Wrapping::None)
                    .color(color),
            )
            .padding(Padding {
                bottom: baseline_pad(big, size),
                ..Padding::ZERO
            }),
        );
    }
    line.into()
}

/// Degrau do valor: medio (24), grande (36, o do bloco) e heroi (44, um por tela). Sempre em
/// Light: o peso fino so cabe em numero desse tamanho para cima.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatSize {
    #[cfg_attr(not(test), expect(dead_code, reason = "valor medio sem tela ainda"))]
    M,
    L,
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "o heroi do Resumo monta as pecas")
    )]
    XL,
}

impl StatSize {
    pub const fn size(self) -> f32 {
        match self {
            StatSize::M => type_scale::VALUE_M,
            StatSize::L => type_scale::VALUE_L,
            StatSize::XL => type_scale::VALUE_XL,
        }
    }

    /// Prefixo e centavos: 13 no medio, 16 nos outros.
    const fn small(self) -> f32 {
        match self {
            StatSize::M => type_scale::LABEL,
            StatSize::L | StatSize::XL => type_scale::SECTION,
        }
    }

    const fn spacing(self) -> f32 {
        match self {
            StatSize::M => 4.0,
            StatSize::L => 6.0,
            StatSize::XL => 8.0,
        }
    }
}

/// Dinheiro grande: sinal com `R$` ("R$", "−R$") e centavos em `text_muted`, o inteiro em Light
/// na `color`. O valor ja vem formatado (e mascarado) pelo modelo; aqui so a tipografia. Os
/// centavos ja trazem a virgula e somem ocultos: juntar uma virgula aqui deixaria "•••••,".
pub fn money<'a, M: 'a>(
    t: &'static Tokens,
    value: &BigMoney,
    size: StatSize,
    color: Color,
) -> Element<'a, M> {
    value_line(
        vec![
            (
                format!("{}{}", value.sign, value.currency),
                size.small(),
                fonts::INTER,
                t.text_muted,
            ),
            (value.whole.clone(), size.size(), fonts::INTER_LIGHT, color),
            (
                value.cents.clone(),
                size.small(),
                fonts::INTER,
                t.text_muted,
            ),
        ],
        size.size(),
        size.spacing(),
    )
}

/// Bloco de numero: rotulo 13 `text_muted` em cima, o valor e uma nota 12 opcional. Devolve a
/// `Column` (espaco 8) para a tela trocar o espaco ou acrescentar linhas embaixo.
pub fn stat<'a, M: 'a>(
    t: &'static Tokens,
    label: &str,
    value: Element<'a, M>,
    note: Option<&str>,
) -> Column<'a, M> {
    let mut col = column![
        text(label.to_string())
            .size(type_scale::LABEL)
            .color(t.text_muted),
        value
    ]
    .spacing(8);
    if let Some(note) = note {
        col = col.push(
            text(note.to_string())
                .size(type_scale::META)
                .color(t.text_muted),
        );
    }
    col
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dashboard::money::Money;
    use crate::ui::theme::{DARK, LIGHT};

    #[test]
    fn linha_de_base_do_valor_partido() {
        assert!((baseline_pad(44.0, 16.0) - 3.8276).abs() < 1e-3);
        assert_eq!(baseline_pad(16.0, 44.0), 0.0);
    }

    #[test]
    fn numero_monta_em_todo_degrau_com_e_sem_nota() {
        for t in [&DARK, &LIGHT] {
            for size in [StatSize::M, StatSize::L, StatSize::XL] {
                let value: Element<'_, ()> =
                    money(t, &Money::new(false).big(123_456), size, t.text);
                let _: Element<'_, ()> = stat(t, "Total separado", value, Some("nota")).into();
                let value: Element<'_, ()> =
                    money(t, &Money::new(true).big(-1), size, t.expense_fg);
                let _: Element<'_, ()> = stat(t, "Saldo", value, None).into();
            }
            let _: Element<'_, ()> = value_line(
                vec![
                    ("3,5".into(), 36.0, fonts::INTER_LIGHT, t.text),
                    (String::new(), 14.0, fonts::INTER, t.text_muted),
                ],
                36.0,
                8.0,
            );
        }
    }
}
