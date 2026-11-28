//! Barra lateral (Main · lateral, Componentes · "Navegacao lateral"): marca, navegacao, secao
//! "HUB", card de status e tema. Abaixo de 1024 px vira o trilho de 76 px so com icones.

use ::time::OffsetDateTime;
use gpui::{
    AnyElement, AnyView, App, Entity, FocusHandle, IntoElement, ParentElement as _, Role,
    SharedString, Styled as _, Window, div, img, prelude::*, px,
};

use super::{ShellFocus, send, status_card};
use crate::config::ThemeMode;
use crate::gui::components::badge::Badge;
use crate::gui::components::button::IconButton;
use crate::gui::components::focus;
use crate::gui::components::segmented::{Segmented, Surface};
use crate::gui::components::tooltip::TooltipView;
use crate::gui::fonts;
use crate::gui::icons::{Icon, icon};
use crate::gui::root::{HubState, dispatch_later};
use crate::gui::theme::Tokens;
use crate::ui_model::app::{App as Model, Msg, Screen};
use crate::ui_model::shell::{self, NAV_ORDER};

pub const WIDTH: f32 = 232.0;
pub const RAIL_WIDTH: f32 = 76.0;
pub const BRAND: &str = "brand/hubfinance-simbolo-app-160.svg";

/// Icone e rotulo de cada tela da navegacao (os do artboard Main).
pub fn nav_item(screen: Screen) -> (Icon, &'static str) {
    match screen {
        Screen::Dashboard => (Icon::Dashboard, "Dashboard"),
        Screen::Transactions => (Icon::List, "Lançamentos"),
        Screen::Reserves => (Icon::Shield, "Reservas"),
        Screen::Analysis => (Icon::Bulb, "Análise"),
        Screen::Connection => (Icon::Wifi, "Conexão"),
        Screen::Settings => (Icon::Settings, "Ajustes"),
    }
}

fn tip(text: String, t: &'static Tokens) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    move |_, cx| TooltipView::text(text.clone(), t).build(cx)
}

fn brand(t: &'static Tokens, rail: bool) -> AnyElement {
    let mark = img(BRAND).size(px(40.0)).flex_none();
    let row = div()
        .id("brand")
        .flex()
        .items_center()
        .gap(px(12.0))
        .child(mark);
    if rail {
        let text = format!("HubFinance · {}", shell::brand_subtitle());
        return row
            .justify_center()
            .tooltip(tip(text, t))
            .into_any_element();
    }
    row.px(px(6.0))
        .child(
            div()
                .min_w_0()
                .child(
                    div()
                        .text_size(px(16.0))
                        .font_weight(fonts::weight(600))
                        .child("HubFinance"),
                )
                .child(
                    div()
                        .mt(px(2.0))
                        .text_size(px(12.0))
                        .text_color(t.muted)
                        .child(shell::brand_subtitle()),
                ),
        )
        .into_any_element()
}

fn nav_link(
    t: &'static Tokens,
    app: &Model,
    state: &Entity<HubState>,
    index: usize,
    rail: bool,
    handle: &FocusHandle,
) -> AnyElement {
    let screen = NAV_ORDER[index];
    let (glyph, label) = nav_item(screen);
    let active = app.screen == screen;
    let (bg, fg, weight) = if active {
        (Some(t.accent_900), t.accent_200, 600)
    } else {
        (None, t.muted, 500)
    };
    let badge = (screen == Screen::Analysis)
        .then(|| shell::alert_count(app))
        .flatten();
    let shortcut = shell::nav_shortcut(index + 1);
    // O badge e so um numero na tela; quem usa leitor ouve "Analise, 2 alertas".
    let name = match badge {
        Some(count) => format!("{label}, {}", shell::plural(count, "alerta", "alertas")),
        None => label.to_string(),
    };
    // `.nav:not([aria-current]):hover`: fundo surface-2 e texto/icone em `text`.
    let hover_bg = t.surface_2;
    let hover_fg = t.text;
    let group = SharedString::from(format!("nav-{index}"));
    let el = div()
        .id(("nav", index as u64))
        .group(group.clone())
        .role(Role::Button)
        .aria_label(SharedString::from(name))
        .aria_selected(active)
        .track_focus(handle)
        .focus_visible(focus::style(t, t.bg))
        .cursor_pointer()
        .relative()
        .flex()
        .items_center()
        .h(px(44.0))
        .rounded_full()
        .text_size(px(14.0))
        .font_weight(fonts::weight(weight))
        .text_color(fg)
        .when_some(bg, |d, c| d.bg(c))
        .when(!active, |d| {
            d.hover(move |s| s.bg(hover_bg).text_color(hover_fg))
        })
        .on_click(send(state, move || Msg::SelectScreen(screen)))
        .child(icon(glyph, px(18.0), fg).when(!active, |i| {
            i.group_hover(group, move |s| s.text_color(hover_fg))
        }));
    let tooltip = if rail {
        format!("{label} · {shortcut}")
    } else {
        shortcut
    };
    let el = if rail {
        // No trilho o numero nao cabe: vira o ponto de 9 px no canto do icone.
        el.w(px(44.0)).justify_center().when(badge.is_some(), |d| {
            d.child(
                div()
                    .absolute()
                    .top(px(7.0))
                    .right(px(7.0))
                    .child(Badge::new(0, t).dot()),
            )
        })
    } else {
        el.gap(px(12.0))
            .px(px(14.0))
            .child(div().child(label))
            .when_some(badge, |d, count| {
                d.child(
                    div()
                        .ml_auto()
                        .child(Badge::new(u32::try_from(count).unwrap_or(u32::MAX), t)),
                )
            })
    };
    el.tooltip(tip(tooltip, t)).into_any_element()
}

fn section_label(t: &'static Tokens, rail: bool) -> AnyElement {
    if rail {
        // Sem espaco para o rotulo: um fio separa os dois grupos.
        return div()
            .w(px(24.0))
            .h(px(1.0))
            .mx_auto()
            .my(px(12.0))
            .bg(t.divider)
            .into_any_element();
    }
    div()
        .px(px(14.0))
        .pt(px(18.0))
        .pb(px(6.0))
        .text_size(px(12.0))
        .font_weight(fonts::weight(600))
        .text_color(t.muted)
        .child("HUB")
        .into_any_element()
}

fn theme_switch(
    t: &'static Tokens,
    app: &Model,
    state: &Entity<HubState>,
    rail: bool,
    focus: &ShellFocus,
) -> AnyElement {
    let dark = app.theme.is_dark(app.system_dark);
    let state = state.clone();
    if rail {
        // `.tg` em coluna: os dois botoes de 40 px (raio 24) um sobre o outro; o ativo e o
        // `accent_900`/`accent_200` do segmentado.
        let choices = [
            (0, Icon::Moon, "Tema escuro", ThemeMode::Dark),
            (1, Icon::Sun, "Tema claro", ThemeMode::Light),
        ];
        let buttons = choices.map(|(i, glyph, name, mode)| {
            let active = usize::from(!dark) == i;
            let state = state.clone();
            let b = IconButton::new(("theme-rail", i as u64), glyph, t)
                .box_size(40.0)
                .glyph_size(15.0)
                .aria_label(name)
                .focus(&focus.theme[i])
                .on_click(move |_, cx| dispatch_later(&state, Msg::SetTheme(mode), cx));
            if active {
                b.bg(t.accent_900).glyph_color(t.accent_200)
            } else {
                b.glyph_color(t.muted)
            }
        });
        return div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .p(px(4.0))
            .rounded(px(24.0))
            .bg(t.surface)
            .when_some(t.card_ring.as_ref(), |d, r| d.shadow(r.shadows.to_vec()))
            .children(buttons)
            .into_any_element();
    }
    Segmented::new(
        "theme-seg",
        vec!["Escuro".into(), "Claro".into()],
        usize::from(!dark),
        t,
    )
    .label("Tema")
    .surface(Surface::Raised)
    .item_height(40.0)
    .icons(vec![Icon::Moon, Icon::Sun])
    .width(WIDTH - 32.0)
    .focus_handles(focus.theme.clone())
    .on_select(move |i, _, cx| {
        let mode = if i == 0 {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        };
        dispatch_later(&state, Msg::SetTheme(mode), cx);
    })
    .into_any_element()
}

pub fn render(
    t: &'static Tokens,
    app: &Model,
    state: &Entity<HubState>,
    focus: &ShellFocus,
) -> AnyElement {
    let rail = shell::is_rail(app.window_size.w);
    let card = shell::status_card(&app.snapshot, OffsetDateTime::now_utc());
    let link = |i: usize| nav_link(t, app, state, i, rail, &focus.nav[i]);
    let nav = div()
        .id("nav-main")
        .role(Role::Navigation)
        .aria_label("Principal")
        .flex()
        .flex_col()
        .gap(px(4.0))
        .children((0..4).map(link))
        .child(section_label(t, rail))
        .children((4..NAV_ORDER.len()).map(link));
    // A coluna rola inteira quando a altura nao basta (960x640, 1100x640): marca e navegacao no
    // topo, card e tema logo depois do espacador, sempre alcancaveis; sem isso o segmentado de
    // tema saia cortado embaixo.
    div()
        .id("sidebar")
        .w(px(if rail { RAIL_WIDTH } else { WIDTH }))
        .h_full()
        .flex_none()
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .gap(px(24.0))
        .py(px(if rail { 20.0 } else { 24.0 }))
        .px(px(if rail { 12.0 } else { 16.0 }))
        .when(rail, |d| d.items_center())
        .border_r_1()
        .border_color(t.divider)
        .child(div().flex_none().child(brand(t, rail)))
        .child(nav.flex_none())
        .child(div().flex_1().min_h(px(0.0)))
        .child(div().flex_none().child(status_card::render(
            t,
            &card,
            rail,
            app.motion.reduce(),
            state,
            &focus.card,
            &focus.details,
        )))
        .child(
            div()
                .flex_none()
                .child(theme_switch(t, app, state, rail, focus)),
        )
        .into_any_element()
}
