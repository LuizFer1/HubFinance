//! Anel de foco do design: 2 px `accent_400` afastado 2 px, igual em todo controle e nos dois
//! temas (spec §4.3). Mora num lugar so: se cada controle o desenhasse, bastaria um esquecer
//! para o Tab "sumir" naquele botao.
//!
//! O GPUI nao tem `outline-offset`. O anel e feito com duas sombras sem desfoque: a de fora em
//! `accent_400` (spread 4, o traco de 2 px) e, POR CIMA dela, a na cor do fundo (spread 2, o
//! afastamento). A ordem importa: o GPUI pinta cada sombra como um retangulo cheio, na ordem do
//! vec, sem recortar o miolo (`paint_drop_shadows`). Com o accent por ultimo ele cobriria o
//! controle inteiro, e todo controle transparente (contorno, fantasma, radio, segmento inativo)
//! viraria um disco dourado; com o gap por cima o miolo fica na cor do fundo e o fundo proprio
//! do controle, pintado depois das sombras, aparece normal. Por isso o chamador informa `gap`,
//! a cor do fundo sobre o qual o controle esta (`bg` na tela, `surface` dentro de um card): com
//! outra cor o afastamento apareceria como um halo.

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
    vec![shadow(tokens.accent_400, 4.0), shadow(gap, 2.0)]
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
            // Ordem de pintura: primeiro o traco (ate 4 px, 2 px visiveis), depois, por cima,
            // o afastamento de 2 px na cor do fundo. Invertido, o accent taparia o miolo.
            assert_eq!(s[0].spread_radius, px(4.0));
            assert_eq!(s[0].color, t.accent_400);
            assert_eq!(s[1].spread_radius, px(2.0));
            assert_eq!(s[1].color, t.bg);
            assert!(s.iter().all(|x| x.blur_radius == px(0.0) && !x.inset));
        }
    }
}
