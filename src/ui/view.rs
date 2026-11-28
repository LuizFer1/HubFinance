//! Desenho da janela: tudo passa pela casca, que roteia a tela atual.

use iced::Element;

use super::app::{Msg, State};
use super::shell;

/// Uma janela so: o id do daemon nao escolhe nada aqui.
pub fn view(app: &State, _window: iced::window::Id) -> Element<'_, Msg> {
    shell::root(app)
}
