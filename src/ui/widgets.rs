//! Blocos visuais do design, reutilizados pelas telas: rotulos, tile de categoria, avatar,
//! tag de cor, passo numerado.
//!
//! Tudo recebe `&'static Tokens`: o estilo e decidido aqui, a tela so monta.

// Parte dos blocos (tile, rotulos de card) so ganha uso nas telas do plano 2b.
#![allow(dead_code)]

use iced::widget::{Space, container, image, row, text};
use iced::{Alignment, Border, Color, Element, Length, Theme};

use super::fonts;
use super::icons;
use super::theme::{self, Tokens};

/// Rotulo em caixa alta (11 Medium a 55 %). O texto ja vem em maiusculas: o iced nao tem
/// `text-transform` nem `letter-spacing`, entao o espacamento de 0,08 em fica de fora.
pub fn kicker<'a, M: 'a>(t: &'static Tokens, content: &str) -> Element<'a, M> {
    text(content.to_uppercase())
        .size(11)
        .font(fonts::INTER_MEDIUM)
        .color(t.text_alpha(0.55))
        .into()
}

/// Rotulo em caixa alta no acento ("ENDEREÇO DO HUB", "CERTIFICADO DO HUB").
pub fn kicker_accent<'a, M: 'a>(t: &'static Tokens, content: &str) -> Element<'a, M> {
    text(content.to_uppercase())
        .size(11)
        .font(fonts::INTER_MEDIUM)
        .color(t.accent_300)
        .into()
}

/// Titulo da pagina (28 Medium) + subtitulo (13 a 55 %), separados por 4 px.
pub fn page_title<'a, M: 'a>(t: &'static Tokens, title: &str, subtitle: &str) -> Element<'a, M> {
    iced::widget::column![
        text(title.to_string())
            .size(28)
            .font(fonts::INTER_MEDIUM)
            .line_height(1.15)
            .color(t.text),
        text(subtitle.to_string())
            .size(13)
            .color(t.text_alpha(0.55)),
    ]
    .spacing(4)
    .into()
}

/// Titulo de card (15 Medium) com um aparte a direita (12 a 55 %).
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
        line = line.push(text(aside).size(12).color(t.text_alpha(0.55)));
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
    let color = token.map_or(t.neutral_600, |tk| t.token(tk));
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
            .color(t.bg),
    )
    .center(size)
    .style(theme::fill(color, size / 2.0))
    .into()
}

/// Tag da cor da pessoa: pilula de 20 px, fundo a cor a 16 % sobre a superficie, ponto de 6 e o
/// nome da cor ("Magenta") em 11 Medium na cor.
pub fn color_tag<'a, M: 'a>(t: &'static Tokens, token: &str) -> Element<'a, M> {
    let color = t.token(token);
    container(
        row![
            dot(color, 6.0),
            text(crate::dashboard::colors::color_name(token))
                .size(11)
                .font(fonts::INTER_MEDIUM)
                .line_height(1.0)
                .color(color),
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

/// Passo numerado: circulo de 20 px (`accent_900` / `accent_300`, 11 SemiBold) + texto 13 a
/// 75 % (o texto vem pronto, pode ter link no meio).
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

/// Texto de passo: 13 a 75 %, altura de linha 1,45 do prototipo.
pub fn step_text<'a>(t: &'static Tokens, content: impl text::IntoFragment<'a>) -> text::Text<'a> {
    text(content)
        .size(13)
        .line_height(1.45)
        .color(t.text_alpha(0.75))
}

/// Separador vertical de 1 x 20 no divisor, com 4 px de margem dos lados.
pub fn vdivider<'a, M: 'a>(t: &'static Tokens) -> Element<'a, M> {
    container(
        container(Space::new())
            .width(1)
            .height(20)
            .style(theme::fill(t.divider(), 0.0)),
    )
    .padding([0, 4])
    .into()
}

/// Separador horizontal de 1 px no divisor (borda superior das linhas de lista).
pub fn hdivider<'a, M: 'a>(t: &'static Tokens) -> Element<'a, M> {
    container(Space::new())
        .width(Length::Fill)
        .height(1)
        .style(theme::fill(t.divider(), 0.0))
        .into()
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
