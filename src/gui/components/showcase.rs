//! Vitrine de debug: `HUBFINANCE_DEMO=componentes` abre a janela direto aqui, com um exemplo de
//! cada estado, para o dono comparar com o artboard Componentes. Nunca existe em release.
//!
//! Os estados hover e pressionado dependem do mouse e nao da para forca-los: a coluna mostra o
//! componente vivo (passe o mouse) e o foco e simulado com o mesmo anel de `focus::ring`.

use gpui::{AnyElement, Div, IntoElement, SharedString, div, prelude::*, px};

use super::badge::Badge;
use super::button::{Button, IconButton, Variant};
use super::chip::{Chip, Pill};
use super::focus;
use super::radio::Radio;
use super::segmented::{Segmented, Surface};
use super::switch::Switch;
use crate::gui::fonts;
use crate::gui::icons::Icon;
use crate::gui::theme::Tokens;

/// `HUBFINANCE_DEMO=componentes`.
pub fn requested() -> bool {
    std::env::var_os("HUBFINANCE_DEMO").is_some_and(|v| v == "componentes")
}

fn section(title: &'static str, t: &'static Tokens) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(16.0))
        .p(px(24.0))
        .rounded(px(24.0))
        .bg(t.surface)
        .when_some(t.card_ring.clone(), |d, r| d.shadow(r.shadows.to_vec()))
        .child(
            div()
                .text_size(px(13.0))
                .font_weight(fonts::weight(600))
                .text_color(t.muted)
                .child(title.to_uppercase()),
        )
}

fn caption(text: &'static str, t: &'static Tokens) -> Div {
    div()
        .text_size(px(12.0))
        .text_color(t.muted)
        .child(SharedString::from(text))
}

/// O botao com o anel de foco desenhado a forca (no `surface` do card).
fn focused(el: impl IntoElement, t: &'static Tokens) -> AnyElement {
    div()
        .rounded_full()
        .shadow(focus::ring(t, t.surface))
        .child(el)
        .into_any_element()
}

pub fn render(t: &'static Tokens) -> impl IntoElement {
    let gap = t.surface;
    let rows: [(&str, Variant); 5] = [
        ("Primario", Variant::Primary),
        ("Contorno", Variant::Outline),
        ("Fantasma", Variant::Ghost),
        ("Perigo", Variant::Danger),
        ("Destrutivo", Variant::DangerFilled),
    ];
    let mut buttons = section("Botoes", t);
    buttons = buttons.child(
        div().flex().gap(px(12.0)).children(
            ["Padrao (passe o mouse)", "Foco", "Desabilitado"]
                .map(|h| div().w(px(220.0)).child(caption(h, t))),
        ),
    );
    for (n, (name, v)) in rows.into_iter().enumerate() {
        let mk = |id: String, disabled: bool| {
            Button::new(SharedString::from(id), "Continuar", v, t)
                .ring_gap(gap)
                .disabled(disabled)
        };
        buttons = buttons.child(
            div()
                .flex()
                .items_center()
                .gap(px(12.0))
                .child(
                    div()
                        .w(px(100.0))
                        .text_size(px(13.0))
                        .text_color(t.muted)
                        .child(name),
                )
                .child(div().w(px(120.0)).child(mk(format!("b{n}a"), false)))
                .child(
                    div()
                        .w(px(120.0))
                        .child(focused(mk(format!("b{n}b"), false), t)),
                )
                .child(div().w(px(120.0)).child(mk(format!("b{n}c"), true))),
        );
    }
    buttons = buttons.child(
        div()
            .flex()
            .items_center()
            .gap(px(12.0))
            .child(IconButton::new("ib1", Icon::ChevronLeft, t).ring_gap(gap))
            .child(IconButton::new("ib2", Icon::ChevronRight, t).disabled(true))
            .child(Button::new("bi", "Com icone", Variant::Outline, t).icon(Icon::Copy)),
    );

    let controls = section("Controles", t)
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(24.0))
                .child(
                    Segmented::new(
                        "seg1",
                        vec!["Escuro".into(), "Claro".into()],
                        usize::from(!t.dark),
                        t,
                    )
                    .width(210.0),
                )
                .child(
                    Segmented::new("seg2", vec!["Mes".into(), "Ano".into()], 0, t)
                        .surface(Surface::Raised)
                        .width(210.0),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(14.0))
                .child(Switch::new("sw1", true, t).ring_gap(gap))
                .child(Switch::new("sw2", false, t))
                .child(Switch::new("sw3", true, t).disabled(true))
                .child(div().w(px(24.0)))
                .child(Radio::new("r1", false, t))
                .child(Radio::new("r2", true, t))
                .child(focused(Radio::new("r3", false, t), t)),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.0))
                .child(Chip::new("Ultimos 12 meses", t))
                .child(Pill::new("pill", "12 meses", t).ring_gap(gap))
                .child(Badge::new(2, t))
                .child(Badge::new(2, t).dot()),
        );

    div()
        .id("showcase")
        .size_full()
        .overflow_y_scroll()
        .p(px(24.0))
        .flex()
        .flex_col()
        .gap(px(20.0))
        .child(buttons)
        .child(controls)
}
