//! Tela Dashboard. Preenchida no plano 2b (resumo, rosca, barras, ultimos lancamentos).

use iced::Element;
use iced::widget::text;

use super::app::{App, Message};

pub fn view(app: &App) -> Element<'_, Message> {
    text("Dashboard em construção")
        .size(13)
        .color(app.tokens.text_alpha(0.55))
        .into()
}
