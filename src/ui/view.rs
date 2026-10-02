//! Desenho da janela: tudo passa pela casca, que roteia a tela atual.

use iced::Element;

use super::app::{App, Message};
use super::shell;

pub fn view(app: &App) -> Element<'_, Message> {
    shell::root(app)
}
