//! Cabecalho de tela (Main · topo): titulo 30, subtitulo, chip "Parcial", seletor de mes, olho
//! "Ocultar valores" e, na Analise com conta comercial, o segmentado Pessoal | Comercial.

use gpui::{
    AnyElement, Entity, IntoElement, ParentElement as _, Role, Styled as _, div, prelude::*, px,
};

use super::ShellFocus;
use crate::dashboard::contract::Book;
use crate::dashboard::periods::month_label_capitalized;
use crate::gui::components::button::IconButton;
use crate::gui::components::month_picker::{MonthPicker, PartialChip};
use crate::gui::components::segmented::{Segmented, Surface};
use crate::gui::fonts;
use crate::gui::icons::Icon;
use crate::gui::root::{HubState, dispatch_later};
use crate::gui::theme::Tokens;
use crate::ui_model::app::{App as Model, Msg, Screen};
use crate::ui_model::shell;

/// O segmentado Pessoal | Comercial fica so na Analise (spec §5 e §6.1.1; o cabecalho do Main
/// nao o tem) e so com dados de uma conta comercial.
fn has_book_switch(app: &Model) -> bool {
    app.screen == Screen::Analysis && app.dashboard.header.book_switch
}

pub fn render(
    t: &'static Tokens,
    app: &Model,
    state: &Entity<HubState>,
    focus: &ShellFocus,
) -> AnyElement {
    let (title, subtitle) = shell::header_texts(app);
    let titles = div()
        .min_w_0()
        .child(
            div()
                .id("screen-title")
                .role(Role::Heading)
                .text_size(px(30.0))
                .line_height(px(34.5))
                .font_weight(fonts::weight(600))
                .child(title),
        )
        .child(
            div()
                .mt(px(6.0))
                .text_size(px(14.0))
                .text_color(t.muted)
                .child(subtitle),
        );

    let mut controls = div()
        .flex()
        .flex_wrap()
        .items_center()
        .justify_end()
        .gap(px(10.0));
    if shell::has_month_picker(app.screen) {
        if let Some(partial) = shell::header_chip(app) {
            controls = controls.child(PartialChip::new(partial.to_string(), t));
        }
        let shown = month_label_capitalized(app.month());
        let (prev_state, next_state) = (state.clone(), state.clone());
        let mut picker = MonthPicker::new("month", shown, shell::can_go_next_month(app), t)
            .focus(&focus.prev, &focus.next)
            .on_next(move |_, cx| dispatch_later(&next_state, Msg::NextMonth, cx));
        if app.month_idx > 0 {
            picker = picker.on_prev(move |_, cx| dispatch_later(&prev_state, Msg::PrevMonth, cx));
        }
        controls = controls.child(picker);
    }
    if app.screen.shows_values() {
        let glyph = if app.hide_values {
            Icon::EyeOff
        } else {
            Icon::Eye
        };
        let state = state.clone();
        controls = controls.child(
            IconButton::new("hide-values", glyph, t)
                .box_size(44.0)
                .glyph_size(18.0)
                .bg(t.surface)
                .aria_label(app.dashboard.header.hide_toggle.clone())
                .focus(&focus.eye)
                .on_click(move |_, cx| dispatch_later(&state, Msg::ToggleHideValues, cx)),
        );
    }
    if has_book_switch(app) {
        let state = state.clone();
        controls = controls.child(
            Segmented::new(
                "book-seg",
                vec!["Pessoal".into(), "Comercial".into()],
                usize::from(app.book == Book::Business),
                t,
            )
            .surface(Surface::Raised)
            .focus_handles(focus.book.clone())
            .on_select(move |i, _, cx| {
                let book = if i == 0 {
                    Book::Personal
                } else {
                    Book::Business
                };
                dispatch_later(&state, Msg::SelectBook(book), cx);
            }),
        );
    }
    // Alinhado embaixo e com quebra: com a janela estreita os controles descem para baixo do
    // titulo em vez de comprimi-lo.
    div()
        .flex()
        .flex_wrap()
        .items_end()
        .justify_between()
        .gap(px(16.0))
        .child(titles)
        .child(controls)
        .into_any_element()
}
