//! Etapas do dialogo (artboards Remover): circulo de 24 px por etapa, fio `divider` entre elas.
//! A atual e `highlight` com o numero, as feitas `accent_900` com o check, as futuras so o
//! contorno. Nunca so cor: o numero ou o check distingue quem nao ve a diferenca de tom.

use gpui::{
    AnyElement, App, IntoElement, ParentElement as _, RenderOnce, SharedString, Styled as _,
    Window, div, px,
};

use crate::gui::fonts;
use crate::gui::icons::{Icon, icon};
use crate::gui::theme::Tokens;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepState {
    Done,
    Current,
    Upcoming,
}

/// Estado de cada etapa para a `current` (indice a partir de 0); fora do alcance = tudo feito.
pub fn states(total: usize, current: usize) -> Vec<StepState> {
    (0..total)
        .map(|i| match i.cmp(&current) {
            std::cmp::Ordering::Less => StepState::Done,
            std::cmp::Ordering::Equal => StepState::Current,
            std::cmp::Ordering::Greater => StepState::Upcoming,
        })
        .collect()
}

#[derive(IntoElement)]
pub struct Stepper {
    steps: Vec<SharedString>,
    current: usize,
    tokens: &'static Tokens,
}

impl Stepper {
    pub fn new(
        steps: impl IntoIterator<Item = impl Into<SharedString>>,
        current: usize,
        tokens: &'static Tokens,
    ) -> Stepper {
        Stepper {
            steps: steps.into_iter().map(Into::into).collect(),
            current,
            tokens,
        }
    }
}

impl RenderOnce for Stepper {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let t = self.tokens;
        let states = states(self.steps.len(), self.current);
        let last = self.steps.len().saturating_sub(1);
        let mut row = div().flex().flex_1().items_center().gap(px(10.0));
        for (i, (label, state)) in self.steps.into_iter().zip(states).enumerate() {
            let circle = div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(24.0))
                .rounded_full()
                .text_size(px(12.0));
            let circle: AnyElement = match state {
                StepState::Done => circle
                    .bg(t.accent_900)
                    .child(icon(Icon::CheckW32, 12.0, t.accent_200))
                    .into_any_element(),
                StepState::Current => circle
                    .bg(t.highlight)
                    .text_color(t.on_highlight)
                    .font_weight(fonts::weight(700))
                    .child(SharedString::from((i + 1).to_string()))
                    .into_any_element(),
                StepState::Upcoming => circle
                    .border_1()
                    .border_color(t.divider)
                    .text_color(t.muted)
                    .font_weight(fonts::weight(600))
                    .child(SharedString::from((i + 1).to_string()))
                    .into_any_element(),
            };
            let color = if state == StepState::Current {
                t.text
            } else {
                t.muted
            };
            row = row.child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(8.0))
                    .text_size(px(13.0))
                    .font_weight(fonts::weight(600))
                    .text_color(color)
                    .child(circle)
                    .child(label),
            );
            if i != last {
                row = row.child(div().flex_1().min_w(px(16.0)).h(px(1.0)).bg(t.divider));
            }
        }
        row
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn etapas_antes_da_atual_estao_feitas_e_depois_pendentes() {
        use StepState::*;
        assert_eq!(states(3, 0), vec![Current, Upcoming, Upcoming]);
        assert_eq!(states(3, 1), vec![Done, Current, Upcoming]);
        assert_eq!(states(3, 2), vec![Done, Done, Current]);
        assert_eq!(states(3, 9), vec![Done, Done, Done]);
        assert!(states(0, 0).is_empty());
    }
}
