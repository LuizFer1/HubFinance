//! Superficies: card, caixa interna. A cor e a borda vem de `theme`; aqui so o respiro e a
//! largura padrao, para cada tela parar de montar o proprio `container` com o estilo.

use iced::Element;
use iced::Length;
use iced::widget::{Container, container};

use crate::ui::theme::{self, Tokens, space};

/// Card (nivel 0): `surface`, raio 8, borda `card_border`, padding 20 e largura cheia. Devolve o
/// `Container` para a tela trocar o padding (os cards do Dashboard tem respiros proprios) ou
/// acrescentar `clip`.
pub fn card<'a, M: 'a>(t: &'static Tokens, content: impl Into<Element<'a, M>>) -> Container<'a, M> {
    container(content)
        .padding(space::CARD)
        .width(Length::Fill)
        .style(theme::card(t))
}

/// Caixa interna (dentro de card ou dialogo): `surface_2`, raio 6, sem borda nem linha
/// interativa (o hover a 4 % some sobre ela). Padding 12; a previa do envelope usa 4 para o
/// cursor da rolagem nao encostar no canto.
pub fn inset<'a, M: 'a>(
    t: &'static Tokens,
    content: impl Into<Element<'a, M>>,
) -> Container<'a, M> {
    container(content)
        .padding(space::M)
        .width(Length::Fill)
        .style(theme::inset(t))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::{DARK, LIGHT};

    #[test]
    fn card_e_caixa_montam_nos_dois_temas() {
        for t in [&DARK, &LIGHT] {
            let _: Element<'_, ()> = card(t, "conteudo").into();
            let _: Element<'_, ()> = inset(t, "conteudo").padding(4).into();
        }
    }
}
