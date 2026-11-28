//! Card de status do hub (Estados · "Status do hub"): forma + cor + texto, nunca so cor.
//!
//! O conteudo vem de `ui_model::shell::status_card` (a tabela da spec §5 e testada la); aqui so
//! se desenha. No trilho vira so o icone, com o texto no tooltip e no nome acessivel.

use gpui::{
    AnyElement, Entity, FocusHandle, Hsla, IntoElement, ParentElement as _, Role, Styled as _, div,
    prelude::*, px,
};

use super::send;
use crate::gui::components::button::{Button, Size, Variant};
use crate::gui::components::focus;
use crate::gui::components::tooltip::TooltipView;
use crate::gui::fonts;
use crate::gui::icons::{Icon, icon};
use crate::gui::root::{HubState, dispatch_later};
use crate::gui::theme::Tokens;
use crate::ui_model::shell::{StatusCard, StatusIcon, status_card_press};

/// Texto unico do card (nome acessivel e tooltip do trilho).
pub fn summary(card: &StatusCard) -> String {
    let mut parts = vec![card.title.clone()];
    parts.extend(card.lines.iter().cloned());
    parts.join(" · ")
}

fn mark(kind: StatusIcon, t: &Tokens) -> AnyElement {
    let disc = |color: Hsla| {
        div()
            .size(px(16.0))
            .flex_none()
            .rounded_full()
            .bg(color)
            .flex()
            .items_center()
            .justify_center()
    };
    match kind {
        StatusIcon::Ok => disc(t.ok)
            .child(icon(Icon::Check, px(10.0), t.bg))
            .into_any_element(),
        StatusIcon::Danger => disc(t.danger)
            .text_size(px(11.0))
            .font_weight(fonts::weight(800))
            .text_color(t.on_danger)
            .child("!")
            .into_any_element(),
        StatusIcon::Warning => icon(Icon::Warning, px(16.0), t.warning).into_any_element(),
        // Anel vazio: a forma ja diz "em andamento" sem depender da rotacao, que
        // fica para o relogio de animacao (e cai com "Reduzir movimento").
        StatusIcon::Spinner => div()
            .size(px(16.0))
            .flex_none()
            .rounded_full()
            .border_2()
            .border_color(t.accent)
            .into_any_element(),
    }
}

pub fn render(
    t: &'static Tokens,
    card: &StatusCard,
    rail: bool,
    state: &Entity<HubState>,
    focus: &FocusHandle,
    details: &FocusHandle,
) -> AnyElement {
    let label = summary(card);
    let ring = focus::style(t, t.bg);
    // Com "Ver detalhes" o card nao e alvo de clique: dois destinos (Conexao e Ajustes) no mesmo
    // retangulo, e o clique do botao subiria ate o card.
    let clickable = card.action.is_none();
    let base = div()
        .id("status-card")
        .role(Role::Status)
        .aria_label(label.clone())
        .bg(t.surface)
        .when(clickable, |d| {
            d.track_focus(focus)
                .focus_visible(ring)
                .cursor_pointer()
                .on_click(send(state, status_card_press))
        })
        .when_some(t.card_ring.as_ref(), |d, r| d.shadow(r.shadows.to_vec()));
    if rail {
        let tip = label.clone();
        return base
            .size(px(44.0))
            .rounded_full()
            .flex()
            .items_center()
            .justify_center()
            .child(mark(card.icon, t))
            .tooltip(move |_, cx| TooltipView::text(tip.clone(), t).build(cx))
            .into_any_element();
    }
    let mut head = div().flex().flex_col().min_w_0();
    head = head.child(
        div()
            .text_size(px(13.0))
            .font_weight(fonts::weight(600))
            .child(card.title.clone()),
    );
    let mut lines = card.lines.iter();
    if let Some(first) = lines.next() {
        head = head.child(
            div()
                .mt(px(2.0))
                .text_size(px(12.0))
                .text_color(t.muted)
                .child(first.clone()),
        );
    }
    let rest: Vec<&String> = lines.collect();
    let mut body = base
        .rounded(px(18.0))
        .p(px(16.0))
        .flex()
        .flex_col()
        .gap(px(12.0))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.0))
                .child(mark(card.icon, t))
                .child(head),
        );
    if !rest.is_empty() {
        body = body.child(div().h(px(1.0)).bg(t.divider)).child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .text_size(px(12.0))
                .line_height(px(17.0))
                .text_color(t.muted)
                .children(rest.into_iter().map(|l| div().child(l.clone()))),
        );
    }
    if let Some(action) = card.action {
        let state = state.clone();
        body = body.child(
            Button::new("status-action", action.label(), Variant::Outline, t)
                .size(Size::Small)
                .focus(details)
                .on_click(move |_, cx| dispatch_later(&state, action.msg(), cx)),
        );
    }
    body.into_any_element()
}
