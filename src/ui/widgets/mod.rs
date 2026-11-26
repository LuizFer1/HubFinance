//! Blocos visuais do design, reutilizados pelas telas: rotulos, tile de categoria, avatar,
//! tag de cor, passo numerado.
//!
//! Tudo recebe `&'static Tokens`: o estilo e decidido aqui, a tela so monta.

use iced::widget::{Space, button, container, image, row, text};
use iced::{Alignment, Border, Color, Element, Length, Padding, Theme};

use super::fonts;
use super::icons;
use super::theme::{self, Tokens};

mod surface;

pub use surface::{card, divider, inset, vdivider};

/// Controle segmentado de duas opcoes (o "toggle" do hub): `(glifo opcional, rotulo, ativo,
/// mensagem)` para cada lado.
pub fn seg_switch<'a, M: Clone + 'a>(
    t: &'static Tokens,
    options: [(Option<&'static str>, &'static str, bool, M); 2],
) -> Element<'a, M> {
    let option =
        |(glyph, label, active, message): (Option<&'static str>, &'static str, bool, M)| {
            let mut content = row![].spacing(6).align_y(Alignment::Center);
            if let Some(glyph) = glyph {
                content = content.push(text(glyph).font(fonts::PHOSPHOR).size(12).line_height(1.0));
            }
            content = content.push(text(label).size(12).font(fonts::INTER_MEDIUM));
            button(
                container(content)
                    .center_x(Length::Fill)
                    .height(26)
                    .align_y(Alignment::Center),
            )
            .padding(0)
            .width(Length::Fill)
            .style(theme::seg_option(t, active))
            .on_press(message)
        };
    let [a, b] = options;
    container(row![option(a), option(b)].width(Length::Fill))
        .padding(3)
        .width(Length::Fill)
        .style(theme::outlined(t.divider(), 8.0))
        .into()
}

/// Segmentado Grafite (tema, Pessoal | Comercial): trilho `surface` raio 8 com 4 px de
/// respiro, opcoes de 36 px em 13 Regular. `fill` divide a largura (seletor de tema da lateral);
/// sem ele cada opcao tem o tamanho do rotulo + 14 px de cada lado (cabecalho).
pub fn segmented<'a, M: Clone + 'a>(
    t: &'static Tokens,
    fill: bool,
    options: [(&'static str, bool, M); 2],
) -> Element<'a, M> {
    let option = |(label, active, message): (&'static str, bool, M)| {
        let content = container(text(label).size(13).line_height(1.0))
            .height(36)
            .align_y(Alignment::Center);
        let content = if fill {
            content.center_x(Length::Fill)
        } else {
            content.padding([0, 14])
        };
        let b = button(content)
            .padding(0)
            .style(theme::segment(t, active))
            .on_press(message);
        if fill { b.width(Length::Fill) } else { b }
    };
    let [a, b] = options;
    let line = row![option(a), option(b)].spacing(4);
    let rail = container(if fill { line.width(Length::Fill) } else { line })
        .padding(4)
        .style(theme::fill(t.surface, 8.0));
    if fill {
        rail.width(Length::Fill).into()
    } else {
        rail.into()
    }
}

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

/// Tile de categoria: 32 x 32, raio 8, fundo a cor a 18 % sobre a superficie, icone 16 na cor
/// cheia.
pub fn tile<'a, M: 'a>(t: &'static Tokens, glyph: &'static str, token: &str) -> Element<'a, M> {
    container(icons::icon(glyph, 16.0, t.token(token)))
        .width(32)
        .height(32)
        .center(32)
        .style(theme::fill(t.token_tint(token, 0.18), 8.0))
        .into()
}

/// Tamanho da inicial por diametro do avatar (40 / 24 / 22 no design).
fn initial_size(size: f32) -> f32 {
    if size >= 40.0 {
        16.0
    } else if size >= 24.0 {
        11.0
    } else {
        10.0
    }
}

/// Avatar redondo. Com foto: a imagem recortada em circulo, anel de 2 px na superficie e anel
/// de 1,5 px na cor da pessoa (os aneis ficam por fora, como o `box-shadow` do prototipo).
/// Sem foto: a inicial (SemiBold, cor `bg`) sobre um circulo solido na cor; sem token,
/// `neutral_600` (aparelho sem perfil conhecido).
pub fn avatar<'a, M: 'a>(
    t: &'static Tokens,
    name: &str,
    token: Option<&str>,
    photo: Option<&image::Handle>,
    size: f32,
) -> Element<'a, M> {
    let (neutral, neutral_initial) = t.avatar_neutral();
    let (color, initial_color) = token.map_or((neutral, neutral_initial), |tk| (t.token(tk), t.bg));
    if let Some(handle) = photo {
        let ring = 1.5;
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
            .color(initial_color),
    )
    .center(size)
    .style(theme::fill(color, size / 2.0))
    .into()
}

/// Tag da cor da pessoa: pilula de 20 px, fundo a cor a 16 % sobre a superficie, ponto de 6 na
/// cor e o nome da cor ("Magenta") em 11 Medium no texto de apoio. O nome na propria cor ficava
/// abaixo de 4,5:1 em varios tokens do claro; o texto sai da cor, que fica no ponto e na tinta.
pub fn color_tag<'a, M: 'a>(t: &'static Tokens, token: &str) -> Element<'a, M> {
    let color = t.token(token);
    container(
        row![
            dot(color, 6.0),
            text(crate::dashboard::colors::color_name(token))
                .size(11)
                .font(fonts::INTER_MEDIUM)
                .line_height(1.0)
                .color(t.text_soft()),
        ]
        .spacing(5)
        .align_y(Alignment::Center),
    )
    .height(20)
    .padding([0, 7])
    .align_y(Alignment::Center)
    .style(theme::fill(t.token_tint(token, 0.16), 999.0))
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

/// Valor grande dos cards: prefixo ("R$", "+R$", "−R$") e centavos em 16 Regular, inteiro em
/// 32 Medium. O iced nao alinha por linha de base; com altura de linha 1,0 a base da Inter fica
/// a 0,136 em do fundo da caixa, entao os pedacos de 16 sobem 2 px para casar com o de 32.
pub fn money_big<'a, M: 'a>(
    prefix: &str,
    whole: &str,
    cents: &str,
    main: Color,
    small: Color,
) -> Element<'a, M> {
    let piece = |content: String| {
        container(text(content).size(16).line_height(1.0).color(small)).padding(Padding {
            bottom: 2.0,
            ..Padding::ZERO
        })
    };
    row![
        piece(prefix.to_string()),
        text(whole.to_string())
            .size(32)
            .font(fonts::INTER_MEDIUM)
            .line_height(1.0)
            .color(main),
        piece(format!(",{cents}")),
    ]
    .spacing(4)
    .align_y(Alignment::End)
    .into()
}

/// Chip "Parcial · até 19 nov" (texto pronto do modelo): pilula `accent_900` com 12 Regular em
/// `accent_300`, 7 x 12 de respiro. Avisa que o mes corrente ainda pode crescer.
pub fn partial_tag<'a, M: 'a>(t: &'static Tokens, label: &str) -> Element<'a, M> {
    container(
        text(label.to_string())
            .size(12)
            .line_height(1.0)
            .color(t.accent_300),
    )
    .padding([7, 12])
    .style(theme::fill(t.accent_900, 999.0))
    .into()
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
    fn inicial_maiuscula_e_vazio_vira_interrogacao() {
        assert_eq!(initial("ana"), "A");
        assert_eq!(initial("  érico"), "É");
        assert_eq!(initial("Pixel da Ana"), "P");
        assert_eq!(initial(""), "?");
    }
}
