//! Desenho da janela: tudo passa pela casca, que roteia a tela atual.

use iced::Element;

use super::app::{App, Message};
use super::shell;

/// Uma janela so: o id do daemon nao escolhe nada aqui.
pub fn view(app: &App, _window: iced::window::Id) -> Element<'_, Message> {
    shell::root(app)
}
