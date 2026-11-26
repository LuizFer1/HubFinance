//! Superficies: card, caixa interna. A cor e a borda vem de `theme`; aqui so o respiro e a
//! largura padrao, para cada tela parar de montar o proprio `container` com o estilo.

use iced::widget::{Container, Space, container};
use iced::{Element, Length};

use crate::ui::theme::{self, Tokens, size, space};

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

/// Divisoria horizontal de 1 px no divisor, na largura toda (linhas de lista, topo da barra de
/// titulo). E container e nao borda porque o iced nao tem borda de um lado so.
pub fn divider<'a, M: 'a>(t: &'static Tokens) -> Element<'a, M> {
    container(Space::new())
        .width(Length::Fill)
        .height(size::BORDER)
        .style(theme::fill(t.divider(), 0.0))
        .into()
}

/// Separador vertical de 1 x 20 no divisor, com 4 px de margem dos lados (barra de filtros).
pub fn vdivider<'a, M: 'a>(t: &'static Tokens) -> Element<'a, M> {
    container(
        container(Space::new())
            .width(size::BORDER)
            .height(20)
            .style(theme::fill(t.divider(), 0.0)),
    )
    .padding([0.0, space::XS])
    .into()
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
            let _: Element<'_, ()> = divider(t);
            let _: Element<'_, ()> = vdivider(t);
        }
    }
}
