//! Animacoes do GPUI (spec 11): scrim entrando e toast subindo/descendo. As duracoes e as curvas
//! sao as de `ui_model::motion`, a mesma fonte da iced; aqui so se liga cada uma a um
//! `with_animation`. Com "Reduzir movimento" nenhuma anima: o estado final aparece na hora,
//! porque uma animacao ligada pela metade e pior que nenhuma para quem pediu isso.

use std::time::Duration;

use gpui::{Animation, AnimationExt, AnyElement, ElementId, IntoElement, Styled, px};

use crate::ui_model::motion::{Easing, SCRIM_IN, TOAST};

/// Quanto o toast percorre na vertical entrando e saindo.
pub const TOAST_TRAVEL: f32 = 24.0;

/// Duracao efetiva: `None` = sem animacao (movimento reduzido).
pub fn duration(reduce: bool, dur: Duration) -> Option<Duration> {
    if reduce { None } else { Some(dur) }
}

/// Deslocamento do toast para um progresso de 0 (fora) a 1 (no lugar).
pub fn toast_offset(progress: f32) -> f32 {
    (1.0 - progress.clamp(0.0, 1.0)) * TOAST_TRAVEL
}

/// So a opacidade, de 0 a 1: o `Styled` entregue deve ser o scrim sozinho, nunca o dialogo (a
/// opacidade valeria tambem para os filhos).
pub fn scrim_in<E>(el: E, id: impl Into<ElementId>, reduce: bool) -> AnyElement
where
    E: IntoElement + Styled + 'static,
{
    match duration(reduce, SCRIM_IN) {
        None => el.into_any_element(),
        Some(dur) => el
            .with_animation(
                id,
                Animation::new(dur).with_easing(|x| Easing::EaseOut.apply(x)),
                |el, delta| el.opacity(delta),
            )
            .into_any_element(),
    }
}

/// Toast subindo da borda de baixo (`rising`) ou descendo ao sair. Quem o monta decide quando
/// remover o elemento: depois dos `TOAST` ms da descida.
pub fn toast_slide<E>(el: E, id: impl Into<ElementId>, reduce: bool, rising: bool) -> AnyElement
where
    E: IntoElement + Styled + 'static,
{
    match duration(reduce, TOAST) {
        None => el.into_any_element(),
        Some(dur) => el
            .with_animation(
                id,
                Animation::new(dur).with_easing(|x| Easing::EaseOutCubic.apply(x)),
                move |el, delta| {
                    let progress = if rising { delta } else { 1.0 - delta };
                    el.relative()
                        .top(px(toast_offset(progress)))
                        .opacity(progress)
                },
            )
            .into_any_element(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reduzir_movimento_desliga_a_animacao() {
        assert_eq!(duration(true, SCRIM_IN), None);
        assert_eq!(duration(false, SCRIM_IN), Some(SCRIM_IN));
        assert_eq!(duration(false, TOAST), Some(TOAST));
    }

    #[test]
    fn toast_parte_de_baixo_e_termina_no_lugar() {
        assert_eq!(toast_offset(0.0), TOAST_TRAVEL);
        assert_eq!(toast_offset(1.0), 0.0);
        // Fora de [0, 1] nao extrapola.
        assert_eq!(toast_offset(-3.0), TOAST_TRAVEL);
        assert_eq!(toast_offset(7.0), 0.0);
    }
}
