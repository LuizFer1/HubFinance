//! Blocos visuais do design, reutilizados pelas telas: rotulos, tile de categoria, avatar,
//! tag de cor, passo numerado.
//!
//! Tudo recebe `&'static Tokens`: o estilo e decidido aqui, a tela so monta.

use iced::widget::{Space, button, container, image, row, text};
use iced::{Alignment, Border, Color, Element, Length, Theme};

use super::fonts;
use super::icons;
use super::theme::{self, Tokens};

mod controls;
mod marks;
mod surface;
mod values;

pub use controls::{
    ChipShape, RowEdge, RowHover, SegmentedSize, SegmentedVariant, action_button,
    action_button_icon, action_button_with, chip, copy_button, field_copy, icon_button, list_row,
    segmented, toggle,
};
pub use marks::{
    Bar, CompactTone, TagTone, bar, color_tag, neutral_tile, progress_steps, tag, tag_compact,
    tag_icon, tile, token_tile,
};
pub use surface::{
    banner, card, dialog, divider, empty_copy, empty_icon, empty_state, inset, section_heading,
    settings_row, vdivider,
};
pub use values::{StatSize, baseline_pad, figure, money, stat, stat_label, value_line};

/// Rotulo em frase normal ("Saldo do mês"), 12 Medium em `text_muted`. Desvio do handoff, que pede
/// caixa alta com `letter-spacing`: sem o espacamento (o iced nao tem) a caixa alta ficava
/// apertada e gritava mais que o valor embaixo. O texto vai como a tela passou.
pub fn kicker<'a, M: 'a>(t: &'static Tokens, content: &str) -> Element<'a, M> {
    text(content.to_string())
        .size(12)
        .font(fonts::INTER_MEDIUM)
        .color(t.text_muted)
        .into()
}

/// Rotulo em frase normal no acento ("Endereço do hub", "Certificado do hub").
pub fn kicker_accent<'a, M: 'a>(t: &'static Tokens, content: &str) -> Element<'a, M> {
    text(content.to_string())
        .size(12)
        .font(fonts::INTER_MEDIUM)
        .color(t.accent_300)
        .into()
}

/// Titulo da pagina (30 Regular, altura 1,1) + subtitulo (13 `text_muted`), separados por 6
/// px, como o `h1` do design Grafite. O `letter-spacing: -0.02em` do design fica de fora: o
/// iced nao tem espacamento de letra.
pub fn page_title<'a, M: 'a>(t: &'static Tokens, title: &str, subtitle: &str) -> Element<'a, M> {
    iced::widget::column![
        text(title.to_string())
            .size(30)
            .line_height(1.1)
            .color(t.text),
        text(subtitle.to_string()).size(13).color(t.text_muted),
    ]
    .spacing(6)
    .into()
}

/// Titulo de card (15 Medium) com um aparte a direita (12 em `text_muted`).
pub fn card_title<'a, M: 'a>(
    t: &'static Tokens,
    title: &str,
    aside: Option<String>,
) -> Element<'a, M> {
    let mut line = row![
        text(title.to_string())
            .size(15)
            .font(fonts::INTER_MEDIUM)
            .color(t.text)
            .width(Length::Fill)
    ]
    .align_y(Alignment::End)
    .spacing(12);
    if let Some(aside) = aside {
        line = line.push(text(aside).size(12).color(t.text_muted));
    }
    line.into()
}

/// Circulo solido de `size` px.
pub fn dot<'a, M: 'a>(color: Color, size: f32) -> Element<'a, M> {
    container(Space::new())
        .width(size)
        .height(size)
        .style(theme::fill(color, size / 2.0))
        .into()
}

/// Tamanho da inicial por diametro do avatar (40 / 28 / 24 / 22 no design; o de 28 e o de
/// "Quem lançou" no Resumo).
fn initial_size(size: f32) -> f32 {
    if size >= 40.0 {
        16.0
    } else if size >= 28.0 {
        12.0
    } else if size >= 24.0 {
        11.0
    } else {
        10.0
    }
}

/// Espessura do anel na cor da pessoa, com e sem foto (spec 9).
const AVATAR_RING: f32 = 1.5;

/// Avatar redondo. Com foto: a imagem recortada em circulo, anel de 2 px na superficie e anel
/// de 1,5 px na cor da pessoa (os aneis ficam por fora, como o `box-shadow` do prototipo).
/// Sem foto (spec 9): anel de 1,5 px na cor, fundo na tinta de 18 % dela e a inicial
/// (SemiBold) em `fg`; sem token, o anel e o cinza de controle (`Tokens::avatar_colors`).
pub fn avatar<'a, M: 'a>(
    t: &'static Tokens,
    name: &str,
    token: Option<&str>,
    photo: Option<&image::Handle>,
    size: f32,
) -> Element<'a, M> {
    let (color, fill) = t.avatar_colors(token);
    if let Some(handle) = photo {
        let ring = AVATAR_RING;
        let gap = 2.0;
        let outer = size + 2.0 * (ring + gap);
        let surface = t.surface;
        return container(
            image(handle.clone())
                .width(size)
                .height(size)
                .content_fit(iced::ContentFit::Cover)
                .border_radius(size / 2.0),
        )
        .width(outer)
        .height(outer)
        .padding(ring + gap)
        .style(move |_: &Theme| container::Style {
            background: Some(surface.into()),
            border: Border {
                color,
                width: ring,
                radius: (outer / 2.0).into(),
            },
            ..container::Style::default()
        })
        .into();
    }
    container(
        text(initial(name))
            .size(initial_size(size))
            .font(fonts::INTER_SEMIBOLD)
            .line_height(1.0)
            .color(t.text),
    )
    .center(size)
    .style(move |_: &Theme| container::Style {
        background: Some(fill.into()),
        border: Border {
            color,
            width: AVATAR_RING,
            radius: (size / 2.0).into(),
        },
        ..container::Style::default()
    })
    .into()
}

/// Passo numerado: circulo de 20 px (`accent_900` / `accent_300`, 11 SemiBold) + texto 13 no
/// apoio (o texto vem pronto, pode ter link no meio).
pub fn step<'a, M: 'a>(t: &'static Tokens, n: u8, content: Element<'a, M>) -> Element<'a, M> {
    let number = container(
        text(n.to_string())
            .size(11)
            .font(fonts::INTER_SEMIBOLD)
            .line_height(1.0)
            .color(t.accent_300),
    )
    .center(20)
    .style(theme::fill(t.accent_900, 10.0));
    row![number, container(content).width(Length::Fill)]
        .spacing(10)
        .align_y(Alignment::Start)
        .into()
}

/// Texto de passo: 13 no texto de apoio (`text_soft`), altura de linha 1,45 do prototipo.
pub fn step_text<'a>(t: &'static Tokens, content: impl text::IntoFragment<'a>) -> text::Text<'a> {
    text(content)
        .size(13)
        .line_height(1.45)
        .color(t.text_soft())
}

/// Largura do rotulo do seletor de mes ("Nov 2026" em 14 Medium mais os 4 px de cada lado).
/// Fixa para as setas nao andarem quando o mes troca ("Mai" e mais estreito que "Nov").
const MONTH_LABEL_WIDTH: f32 = 76.0;

/// Seletor de mes Grafite: contorno de 1 px em `neutral_800`, raio 8, sem fundo; setas de 44 x
/// 42 no texto de apoio e o mes ("Nov 2026") em 14 Medium no meio. Seta sem mensagem (limite
/// da janela) vai a `neutral_600`, como a seta apagada do design, e nao reage.
pub fn month_picker<'a, M: Clone + 'a>(
    t: &'static Tokens,
    label: &str,
    prev: Option<M>,
    next: Option<M>,
) -> Element<'a, M> {
    let arrow = |glyph: &'static str, message: Option<M>| {
        let color = if message.is_some() {
            t.text_soft()
        } else {
            // O mesmo apagado dos botoes desabilitados (`theme::disabled`).
            t.idle()
        };
        button(container(icons::icon(glyph, 16.0, color)).center(Length::Fill))
            .width(44)
            .height(42)
            .padding(0)
            .style(theme::plain(color))
            .on_press_maybe(message)
    };
    container(
        row![
            arrow(icons::CARET_LEFT, prev),
            container(
                text(label.to_string())
                    .size(14)
                    .font(fonts::INTER_MEDIUM)
                    .color(t.text)
            )
            .center_x(MONTH_LABEL_WIDTH),
            arrow(icons::CARET_RIGHT, next),
        ]
        .align_y(Alignment::Center),
    )
    .padding(1)
    .style(theme::outlined(t.neutral_800, 8.0))
    .into()
}

/// Tres cards de largura igual; abaixo de `3 x min + 2 x gap`, dois por linha e o terceiro
/// embaixo (o `auto-fit` do prototipo, que o iced nao tem).
pub fn three_up<'a, M: 'a>(
    width: f32,
    min: f32,
    gap: f32,
    cards: [Element<'a, M>; 3],
) -> Element<'a, M> {
    if width >= 3.0 * min + 2.0 * gap {
        iced::widget::Row::with_children(cards).spacing(gap).into()
    } else {
        let [a, b, c] = cards;
        iced::widget::column![
            row![a, b].spacing(gap),
            row![c, Space::new().width(Length::Fill)].spacing(gap),
        ]
        .spacing(gap)
        .into()
    }
}

/// Primeira letra em maiuscula, para o avatar sem foto. `chars().next()` e nao grafema: nome
/// com acento combinante e raro e o pior caso e a inicial sem o acento.
pub fn initial(name: &str) -> String {
    name.trim()
        .chars()
        .next()
        .map_or_else(|| "?".to_string(), |c| c.to_uppercase().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn avatar_monta_com_e_sem_cor_em_todo_tamanho() {
        for t in [&theme::DARK, &theme::LIGHT] {
            for size in [22.0, 24.0, 28.0, 40.0] {
                let _: Element<'_, ()> = avatar(t, "Ana", Some("magenta"), None, size);
                let _: Element<'_, ()> = avatar(t, "Pixel", None, None, size);
            }
        }
        assert_eq!(initial_size(28.0), 12.0);
        assert_eq!(initial_size(24.0), 11.0);
    }

    #[test]
    fn inicial_maiuscula_e_vazio_vira_interrogacao() {
        assert_eq!(initial("ana"), "A");
        assert_eq!(initial("  érico"), "É");
        assert_eq!(initial("Pixel da Ana"), "P");
        assert_eq!(initial(""), "?");
    }
}
