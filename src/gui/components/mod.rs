//! Componentes de acao do design (Componentes · "Botoes", "Controles").
//!
//! Cada controle e `RenderOnce` sem estado proprio: quem desenha (a tela) guarda o valor e os
//! `FocusHandle`s, e entrega um callback que manda a `Msg` ao `ui_model` (normalmente
//! `root::dispatch_later`). Assim o estado continua num lugar so e o teste de cor/medida roda
//! sem janela, sobre funcoes puras.
//!
//! O modulo inteiro e anotado com `dead_code`: so a vitrine de debug os usa ate as telas
//! (Tarefas 15 em diante) chegarem.
#![allow(dead_code)] // usado na Tarefa 17

pub mod avatar;
pub mod badge;
pub mod button;
pub mod card;
pub mod category_tile;
pub mod chip;
pub mod empty;
pub mod focus;
pub mod modal;
pub mod money;
pub mod month_picker;
pub mod progress;
pub mod radio;
pub mod segmented;
#[cfg(debug_assertions)]
pub mod showcase;
pub mod stat;
pub mod status_chip;
pub mod stepper;
pub mod switch;
pub mod text_field;
pub mod toast;
pub mod tooltip;

use gpui::{Hsla, Rgba};

/// Mistura em sRGB como o `color-mix(in srgb, base (1-t), over t)` do CSS. A cor do GPUI nao
/// tem mistura propria; o hover do primario (14 % de `text`) e o pressionado (22 % de preto)
/// da spec §4.1 dependem dela. Alfa tambem e interpolado.
pub fn mix(base: Hsla, over: Hsla, t: f32) -> Hsla {
    let (a, b) = (Rgba::from(base), Rgba::from(over));
    let t = t.clamp(0.0, 1.0);
    let lerp = |x: f32, y: f32| x + (y - x) * t;
    Rgba {
        r: lerp(a.r, b.r),
        g: lerp(a.g, b.g),
        b: lerp(a.b, b.b),
        a: lerp(a.a, b.a),
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::theme::rgb;

    fn hex(c: Hsla) -> u32 {
        let c = Rgba::from(c);
        let b = |v: f32| (v * 255.0).round() as u32;
        (b(c.r) << 16) | (b(c.g) << 8) | b(c.b)
    }

    #[test]
    fn mistura_nas_pontas_e_no_meio() {
        let (a, b) = (rgb(0x000000), rgb(0xFFFFFF));
        assert_eq!(hex(mix(a, b, 0.0)), 0x000000);
        assert_eq!(hex(mix(a, b, 1.0)), 0xFFFFFF);
        assert_eq!(hex(mix(a, b, 0.5)), 0x808080);
        // Fora de [0, 1] nao extrapola.
        assert_eq!(hex(mix(a, b, 3.0)), 0xFFFFFF);
    }
}
