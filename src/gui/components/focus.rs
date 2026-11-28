//! Anel de foco do design: 2 px `accent_400` afastado 2 px, igual em todo controle e nos dois
//! temas (spec §4.3). Mora num lugar so: se cada controle o desenhasse, bastaria um esquecer
//! para o Tab "sumir" naquele botao.
//!
//! O GPUI nao tem `outline-offset`. O anel e feito com duas sombras sem desfoque: a de dentro
//! na cor do fundo atras do controle (cria o afastamento de 2 px) e a de fora em `accent_400`
//! (o traco de 2 px). Por isso o chamador informa `gap`, a cor do fundo sobre o qual o
//! controle esta (`bg` na tela, `surface` dentro de um card): com outra cor o afastamento
//! apareceria como um halo.

use gpui::{BoxShadow, Hsla, StyleRefinement, point, px};

use crate::gui::theme::Tokens;

pub fn ring(tokens: &Tokens, gap: Hsla) -> Vec<BoxShadow> {
    let shadow = |color: Hsla, spread: f32| BoxShadow {
        color,
        offset: point(px(0.0), px(0.0)),
        blur_radius: px(0.0),
        spread_radius: px(spread),
        inset: false,
    };
    vec![shadow(gap, 2.0), shadow(tokens.accent_400, 4.0)]
}

/// Estilo do `focus_visible`: so aparece na navegacao por teclado, nunca no clique (o design
/// mostra o anel no estado "Foco" do Tab, nao no pressionado do mouse).
pub fn style(tokens: &Tokens, gap: Hsla) -> impl FnOnce(StyleRefinement) -> StyleRefinement {
    use gpui::Styled as _;
    let shadows = ring(tokens, gap);
    move |s| s.shadow(shadows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::theme::{DARK, LIGHT};

    #[test]
    fn anel_e_dois_pixels_afastado_dois_nos_dois_temas() {
        for t in [&*DARK, &*LIGHT] {
            let s = ring(t, t.bg);
            assert_eq!(s.len(), 2);
            // Afastamento: 2 px na cor do fundo; traco: ate 4 px, ou seja 2 px visiveis.
            assert_eq!(s[0].spread_radius, px(2.0));
            assert_eq!(s[0].color, t.bg);
            assert_eq!(s[1].spread_radius, px(4.0));
            assert_eq!(s[1].color, t.accent_400);
            assert!(s.iter().all(|x| x.blur_radius == px(0.0) && !x.inset));
        }
    }
}
