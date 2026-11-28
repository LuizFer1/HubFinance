//! Vitrine de debug: `HUBFINANCE_DEMO=componentes` abre a janela direto aqui, com um exemplo de
//! cada estado, para o dono comparar com o artboard Componentes. Nunca existe em release.
//!
//! Hover e pressionado dependem do mouse e foco do Tab (so ligado na Tarefa 13), entao a vitrine
//! os desenha parados: `Button::forced_state` usa `button_colors(Hover/Pressed)` e o foco e o
//! mesmo anel de `focus::ring`. As colunas seguem o artboard: Padrão, Hover, Pressionado, Foco,
//! Desabilitado.

use gpui::{AnyElement, Div, IntoElement, SharedString, div, prelude::*, px};

use super::badge::Badge;
use super::button::{Button, IconButton, State, Variant};
use super::chip::{Chip, Pill};
use super::focus;
use super::radio::Radio;
use super::segmented::{Preview, Segmented, Surface};
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

/// O controle com o anel de foco desenhado a forca (`gap` = fundo atras dele).
fn focused(el: impl IntoElement, t: &'static Tokens, gap: gpui::Hsla) -> AnyElement {
    div()
        .rounded_full()
        .shadow(focus::ring(t, gap))
        .child(el)
        .into_any_element()
}

const COL: f32 = 120.0;

pub fn render(t: &'static Tokens) -> impl IntoElement {
    let gap = t.surface;
    let rows: [(&str, Variant); 5] = [
        ("Primário", Variant::Primary),
        ("Contorno", Variant::Outline),
        ("Fantasma", Variant::Ghost),
        ("Perigo", Variant::Danger),
        ("Destrutivo", Variant::DangerFilled),
    ];
    let mut buttons = section("Botões", t);
    buttons = buttons.child(
        div()
            .flex()
            .gap(px(12.0))
            .child(div().w(px(100.0)))
            .children(
                ["Padrão", "Hover", "Pressionado", "Foco", "Desabilitado"]
                    .map(|h| div().w(px(COL)).child(caption(h, t))),
            ),
    );
    for (n, (name, v)) in rows.into_iter().enumerate() {
        let mk = |col: usize, state: Option<State>, disabled: bool| {
            let b = Button::new(SharedString::from(format!("b{n}{col}")), "Continuar", v, t)
                .ring_gap(gap)
                .disabled(disabled);
            match state {
                Some(s) => b.forced_state(s),
                None => b,
            }
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
                .child(div().w(px(COL)).child(mk(0, None, false)))
                .child(div().w(px(COL)).child(mk(1, Some(State::Hover), false)))
                .child(div().w(px(COL)).child(mk(2, Some(State::Pressed), false)))
                .child(div().w(px(COL)).child(focused(mk(3, None, false), t, gap)))
                .child(div().w(px(COL)).child(mk(4, None, true))),
        );
    }
    buttons = buttons.child(
        div()
            .flex()
            .items_center()
            .gap(px(12.0))
            .child(IconButton::new("ib1", Icon::ChevronLeft, t).ring_gap(gap))
            .child(focused(
                IconButton::new("ib3", Icon::ChevronLeft, t).ring_gap(gap),
                t,
                gap,
            ))
            .child(IconButton::new("ib2", Icon::ChevronRight, t).disabled(true))
            .child(Button::new("bi", "Com ícone", Variant::Outline, t).icon(Icon::Copy)),
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
                    Segmented::new("seg2", vec!["Mês".into(), "Ano".into()], 0, t)
                        .surface(Surface::Raised)
                        .width(210.0),
                ),
        )
        .child(
            // Hover num item inativo e foco num ativo (o ativo nao muda com o mouse).
            div()
                .flex()
                .items_center()
                .gap(px(24.0))
                .child(
                    Segmented::new("seg3", vec!["Escuro".into(), "Claro".into()], 0, t)
                        .preview(1, Preview::Hover)
                        .width(210.0),
                )
                .child(
                    Segmented::new("seg4", vec!["Escuro".into(), "Claro".into()], 0, t)
                        .preview(0, Preview::Focus)
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
                .child(focused(Switch::new("sw4", true, t).ring_gap(gap), t, gap))
                .child(div().w(px(24.0)))
                .child(Radio::new("r1", false, t))
                .child(Radio::new("r2", true, t))
                .child(focused(Radio::new("r3", false, t).ring_gap(gap), t, gap)),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.0))
                .child(Chip::new("Últimos 12 meses", t))
                .child(Pill::new("pill", "12 meses", t).ring_gap(gap))
                .child(focused(
                    Pill::new("pill2", "12 meses", t).ring_gap(gap),
                    t,
                    gap,
                ))
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
