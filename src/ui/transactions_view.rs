//! Tela Lancamentos. Preenchida no plano 2b (filtros, totais, tabela agrupada por dia).

use iced::Element;
use iced::widget::text;

use super::app::{App, Message};

pub fn view(app: &App) -> Element<'_, Message> {
    text("Lançamentos em construção")
        .size(13)
        .color(app.tokens.text_alpha(0.55))
        .into()
}
