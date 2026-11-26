//! Pecas de linha compartilhadas pelas listas de lancamentos e de movimentacoes de reserva
//! (tile da categoria, autor, valor, hover). Sairam do Dashboard quando ele virou o Grafite.

use iced::widget::text::Wrapping;
use iced::widget::{container, row, text};
use iced::{Alignment, Color, Element, Length};

use super::app::{App, Message};
use super::theme::Tokens;
use super::widgets::RowHover;
use super::{fonts, icons, widgets};
use crate::dashboard::contract::Kind;
use crate::dashboard::list::{ListRow, RowAuthor};
use crate::dashboard::money::{format_brl, signed_brl_plus};

/// Coluna do valor nas linhas de lancamento.
const VALUE_WIDTH: f32 = 128.0;

pub(super) fn kicker_with_icon<'a>(
    t: &'static Tokens,
    glyph: &'static str,
    color: Color,
    label: &str,
) -> Element<'a, Message> {
    row![icons::icon(glyph, 13.0, color), widgets::kicker(t, label)]
        .spacing(6)
        .align_y(Alignment::Center)
        .into()
}

/// Tile da categoria; sem categoria viva, o tile neutro com `circle-dashed`.
pub(super) fn category_tile<'a>(t: &'static Tokens, r: &ListRow) -> Element<'a, Message> {
    match (&r.category_icon, &r.category_color) {
        (Some(icon), Some(color)) => widgets::token_tile(t, icons::glyph(icon), color, 32.0),
        _ => widgets::token_tile(t, icons::CIRCLE_DASHED, "slate", 32.0),
    }
}

/// Celula de autor de uma linha de lancamento.
pub(super) fn author_cell<'a>(app: &'a App, r: &'a ListRow) -> Element<'a, Message> {
    author_cell_of(app, r.author.as_ref())
}

/// Avatar de 22 + nome; sem autor visivel, `circle-dashed` em `idle` e "—". Serve as
/// linhas de lancamento e as de movimentacao de reserva.
pub(super) fn author_cell_of<'a>(
    app: &'a App,
    author: Option<&'a RowAuthor>,
) -> Element<'a, Message> {
    let t = app.tokens;
    let (avatar, name): (Element<'a, Message>, String) = match author {
        Some(author) => (
            widgets::avatar(
                t,
                &author.name,
                Some(&author.color),
                app.user_avatars.get(&author.id).map(|a| &a.handle),
                22.0,
            ),
            author.name.clone(),
        ),
        None => (
            container(icons::icon(icons::CIRCLE_DASHED, 22.0, t.idle()))
                .center(22)
                .into(),
            "—".to_string(),
        ),
    };
    row![
        avatar,
        container(text(name).size(13).wrapping(Wrapping::None).color(t.text))
            .width(Length::Fill)
            .clip(true),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}

/// Valor da linha: receita com `+` na cor de receita; despesa com `−` na cor do texto (so a
/// receita ganha cor, como pede o README).
pub(super) fn value_cell<'a>(t: &'static Tokens, r: &ListRow) -> Element<'a, Message> {
    let (label, color) = match r.kind {
        Kind::Income => (signed_brl_plus(r.amount_minor.max(0)), t.income_fg),
        Kind::Expense => (format!("\u{2212}{}", format_brl(r.amount_minor)), t.text),
    };
    container(
        text(label)
            .size(14)
            .font(fonts::INTER_MEDIUM)
            .wrapping(Wrapping::None)
            .color(color),
    )
    .width(VALUE_WIDTH)
    .align_x(Alignment::End)
    .into()
}

/// Linha de lista com o hover da tela (`app.hover_row`), sobre `widgets::list_row`. Altura por
/// parametro: 52 nos lancamentos e nas conversas, 56 nas movimentacoes de reserva.
pub(super) fn hover_row<'a>(
    app: &'a App,
    id: &str,
    content: impl Into<Element<'a, Message>>,
    padding: [u16; 2],
    radius: f32,
    height: f32,
) -> Element<'a, Message> {
    widgets::list_row(
        app.tokens,
        content,
        RowHover {
            hovered: app.hover_row.as_deref() == Some(id),
            enter: Message::HoverRow(id.to_string()),
            exit: Message::UnhoverRow(id.to_string()),
        },
        Some(height),
        padding,
        radius,
    )
}
