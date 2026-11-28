//! Card de status do hub (Estados · "Status do hub"): forma + cor + texto, nunca so cor.
//!
//! O conteudo vem de `ui_model::shell::status_card` (a tabela da spec §5 e testada la); aqui so
//! se desenha. No trilho vira so o icone, com o texto no tooltip e no nome acessivel.

use gpui::{
    AnyElement, Entity, FocusHandle, Hsla, IntoElement, ParentElement as _, Role, Styled as _, div,
    prelude::*, px,
};

use super::send;
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

fn arc_ring(t: &Tokens, arc: Hsla) -> AnyElement {
    // Dois filhos absolutos na mesma caixa de 16: o anel inteiro e, por cima, o arco, que cobre a
    // caixa toda (viewBox 16) sem depender de como a borda desloca o `absolute`.
    div()
        .size(px(16.0))
        .flex_none()
        .relative()
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .rounded_full()
                .border_2()
                .border_color(t.divider),
        )
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .child(icon(Icon::StatusArc, px(16.0), arc)),
        )
        .into_any_element()
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
        // Anel de 2 px com o check de 8 px: "ligado, ainda sem dados".
        StatusIcon::OkEmpty => div()
            .size(px(16.0))
            .flex_none()
            .rounded_full()
            .border_2()
            .border_color(t.ok)
            .flex()
            .items_center()
            .justify_center()
            .child(icon(Icon::Check, px(8.0), t.ok))
            .into_any_element(),
        StatusIcon::Warning => icon(Icon::StatusDiamond, px(16.0), t.warning).into_any_element(),
        // Anel `divider` com so o arco de cima colorido: a forma ja diz "em andamento" sem
        // depender da rotacao (que fica para o relogio de animacao e cai com "Reduzir
        // movimento"), e o "Desligado" (anel inteiro) continua outra forma.
        StatusIcon::Spinner => arc_ring(t, t.accent),
        StatusIcon::Stopping => arc_ring(t, t.muted),
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
    // No trilho nao ha link: o card inteiro e o unico caminho e dispara a acao (com foco e nome).
    let clickable = rail || card.action.is_none();
    let press = match card.action {
        Some(action) if rail => action.msg(),
        _ => status_card_press(),
    };
    let base = div()
        .id("status-card")
        .role(Role::Status)
        .aria_label(label.clone())
        .bg(t.surface)
        .when(clickable, |d| {
            d.track_focus(focus)
                .focus_visible(ring)
                .cursor_pointer()
                .on_click(send(state, move || press.clone()))
        })
        .when_some(t.card_ring.as_ref(), |d, r| d.shadow(r.shadows.to_vec()));
    if rail {
        let tip = label.clone();
        return base
            .p(px(14.0))
            .rounded(px(18.0))
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
        // Link de texto 12/600 `accent`, como no design: o botao contornado de 40 px e o do
        // "Ligar hub" (Tarefa 32).
        body = body.child(
            div()
                .id("status-action")
                .role(Role::Link)
                .aria_label(action.label())
                .self_start()
                .rounded(px(4.0))
                .text_size(px(12.0))
                .font_weight(fonts::weight(600))
                .text_color(t.accent)
                .track_focus(details)
                .focus_visible(focus::style(t, t.surface))
                .cursor_pointer()
                .on_click(move |_, _, cx| dispatch_later(&state, action.msg(), cx))
                .child(action.label()),
        );
    }
    body.into_any_element()
}
