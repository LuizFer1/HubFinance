//! Vitrine de debug: `HUBFINANCE_DEMO=componentes` abre a janela direto aqui, com um exemplo de
//! cada estado, para o dono comparar com o artboard Componentes. Nunca existe em release.
//!
//! Hover e pressionado dependem do mouse e foco do Tab (so ligado na Tarefa 13), entao a vitrine
//! os desenha parados: `Button::forced_state` usa `button_colors(Hover/Pressed)` e o foco e o
//! mesmo anel de `focus::ring`. As colunas seguem o artboard: Padrão, Hover, Pressionado, Foco,
//! Desabilitado.

use gpui::{AnyElement, Div, IntoElement, SharedString, div, prelude::*, px};

use super::avatar::Avatar;
use super::badge::Badge;
use super::button::{Button, IconButton, State, Variant};
use super::card::Card;
use super::category_tile::CategoryTile;
use super::chip::{Chip, Pill};
use super::empty::EmptyState;
use super::focus;
use super::money::Money;
use super::month_picker::{MonthPicker, PartialChip};
use super::progress::Progress;
use super::radio::Radio;
use super::segmented::{Preview, Segmented, Surface};
use super::stat::Stat;
use super::status_chip::{Kind, StatusChip};
use super::switch::Switch;
use super::tooltip::{Line, TooltipView};
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

    let data = data_section(t);
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
        .child(data)
}

/// Componentes de dado: cards, valores (visivel e oculto), avatares, tiles, chips, seletor de
/// mes, tooltips e estado vazio.
fn data_section(t: &'static Tokens) -> impl IntoElement {
    let row = || div().flex().items_center().gap(px(16.0));
    let tip = |v: TooltipView| {
        div().child(
            // O tooltip e uma view; na vitrine desenhamos o conteudo direto, sem hover.
            v.render_static(),
        )
    };
    section("Dados", t)
        .child(
            row()
                .child(
                    Card::new(t)
                        .child(Stat::new("Receitas", "R$ 14.180,00", t).note("novembro"))
                        .w(220.0),
                )
                .child(
                    Card::new(t)
                        .child(Stat::new("Receitas", "R$ 14.180,00", t).hidden(true))
                        .w(220.0),
                )
                .child(
                    Card::highlight(t)
                        .child(Money::new("R$ 6,3 mil", 20.0, t).color(t.on_highlight))
                        .child(Progress::new(3.0, 4.0, t).fill(t.on_highlight))
                        .w(220.0),
                ),
        )
        .child(
            row()
                .child(Money::new("R$ 12.345,67", 30.0, t))
                .child(Money::new("−R$ 96,40", 15.0, t))
                .child(Money::new("75,6%", 15.0, t))
                .child(Money::new("75,6%", 15.0, t).hidden(true)),
        )
        .child(
            row()
                .child(Avatar::new("Luiz", "sky", t))
                .child(Avatar::new("Ágata", "rose", t).size(48.0))
                .child(CategoryTile::new("cart", "orange", t))
                .child(CategoryTile::new("desconhecida", "violet", t))
                .child(StatusChip::new(Kind::Ok, "Emergência coberta", t))
                .child(StatusChip::new(Kind::Warning, "Ainda não coberta", t)),
        )
        .child(
            row()
                .child(MonthPicker::new("Novembro 2026", false, t))
                .child(MonthPicker::new("Outubro 2026", true, t))
                .child(PartialChip::new("Parcial · até 27 nov", t)),
        )
        .child(
            row()
                .items_start()
                .child(tip(TooltipView::rows(
                    "Outubro",
                    vec![
                        Line::Pair {
                            label: "Receitas".into(),
                            value: "R$ 14.180,00".into(),
                            bold: false,
                        },
                        Line::Pair {
                            label: "Sobrou".into(),
                            value: "R$ 2.870,40".into(),
                            bold: true,
                        },
                    ],
                    t,
                )))
                .child(tip(TooltipView::text(
                    "Média das despesas essenciais dos últimos 6 meses.",
                    t,
                ))),
        )
        .child(Card::new(t).child(EmptyState::new(
            Icon::Pulse,
            "Sem movimentações neste mês",
            "Nada foi guardado ou retirado em maio de 2025.",
            t,
        )))
}
