//! Barra de titulo propria (Componentes · "Barra de titulo"): 40 px, logo, nome e tres botoes.
//!
//! Arrastar, duplo clique, Snap Layouts e os botoes sao do sistema: cada area declara seu
//! `WindowControlArea` e o hit-test nativo (HTCAPTION/HTMINBUTTON/...) faz o resto. Por isso a
//! interface GPUI nao manda `TitlePressed/TitleMoved/Resize`; sem a declaracao a barra seria
//! so um desenho e a janela deixaria de arrastar. Fechar chega pelo `on_window_should_close`
//! como `Msg::CloseRequested` (bandeja x desligar).

use gpui::{
    AnyElement, Hsla, IntoElement, ParentElement as _, Styled as _, WindowControlArea, div,
    prelude::*, px, svg,
};

use crate::gui::theme::Tokens;

pub const HEIGHT: f32 = 40.0;
pub const BUTTON_WIDTH: f32 = 46.0;
const GLYPH: f32 = 10.0;

/// Camadas da marca de 16 px (telhado, ponto): arquivos monocromaticos tingidos por `svg()`.
pub const LOGO_ROOF: &str = "brand/hubfinance-pequeno-telhado.svg";
pub const LOGO_DOT: &str = "brand/hubfinance-pequeno-ponto.svg";

/// Cores da marca pelo fundo (as mesmas das versoes "fundo escuro"/"fundo claro" do brand):
/// (telhado, ponto).
pub fn logo_colors(dark: bool) -> (u32, u32) {
    if dark {
        (0xF2F2F2, 0xE2C15A)
    } else {
        (0x131313, 0x876C00)
    }
}

/// Marca em duas camadas sobrepostas. `img()` rasteriza SVG a escala 1.0 (gpui elements/img.rs),
/// entao o pixel-art de 16x16 seria esticado e ficaria borrado em 125% ou 150%; `svg()` e
/// vetorial e redesenha na escala da tela, mas tinge uma cor so, dai uma camada por cor.
fn logo(dark: bool) -> impl IntoElement {
    let (roof, dot) = logo_colors(dark);
    let layer = |path: &'static str, color: u32| {
        svg()
            .path(path)
            .size(px(16.0))
            .text_color(gpui::rgb(color))
            .absolute()
            .top_0()
            .left_0()
    };
    div()
        .relative()
        .size(px(16.0))
        .flex_none()
        .child(layer(LOGO_ROOF, roof))
        .child(layer(LOGO_DOT, dot))
}

/// Glifo do botao do meio: Restaurar quando a janela ja esta maximizada.
pub fn maximize_glyph(maximized: bool) -> &'static str {
    if maximized {
        "window/restore.svg"
    } else {
        "window/maximize.svg"
    }
}

fn control(
    id: &'static str,
    area: WindowControlArea,
    glyph: &'static str,
    t: &'static Tokens,
    hover_bg: Hsla,
    hover_fg: Hsla,
) -> impl IntoElement {
    div()
        .id(id)
        .window_control_area(area)
        .w(px(BUTTON_WIDTH))
        .h(px(HEIGHT))
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
        .group(id)
        .hover(move |s| s.bg(hover_bg))
        .child(
            svg()
                .path(glyph)
                .size(px(GLYPH))
                .text_color(t.muted)
                .group_hover(id, move |s| s.text_color(hover_fg))
                .flex_none(),
        )
}

pub fn render(t: &'static Tokens, maximized: bool) -> AnyElement {
    div()
        .h(px(HEIGHT))
        .w_full()
        .flex_none()
        .flex()
        .items_center()
        .border_b_1()
        .border_color(t.divider)
        .child(
            div()
                .window_control_area(WindowControlArea::Drag)
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .gap_2()
                .pl_4()
                .text_size(px(12.0))
                .text_color(t.muted)
                .child(logo(t.dark))
                .child("HubFinance"),
        )
        .child(control(
            "tb-min",
            WindowControlArea::Min,
            "window/minimize.svg",
            t,
            t.surface_2,
            t.text,
        ))
        .child(control(
            "tb-max",
            WindowControlArea::Max,
            maximize_glyph(maximized),
            t,
            t.surface_2,
            t.text,
        ))
        .child(control(
            "tb-close",
            WindowControlArea::Close,
            "window/close.svg",
            t,
            t.close_hover,
            gpui::white(),
        ))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::assets::HubAssets;
    use gpui::AssetSource as _;

    #[test]
    fn logo_segue_a_cor_do_fundo() {
        assert_eq!(logo_colors(true), (0xF2F2F2, 0xE2C15A));
        assert_eq!(logo_colors(false), (0x131313, 0x876C00));
    }

    #[test]
    fn botao_do_meio_restaura_quando_maximizada() {
        assert_eq!(maximize_glyph(false), "window/maximize.svg");
        assert_eq!(maximize_glyph(true), "window/restore.svg");
    }

    #[test]
    fn todo_asset_da_barra_esta_embutido() {
        for path in [
            LOGO_ROOF,
            LOGO_DOT,
            maximize_glyph(false),
            maximize_glyph(true),
            "window/minimize.svg",
            "window/close.svg",
        ] {
            let bytes = HubAssets.load(path).expect("load").expect("arquivo");
            assert!(bytes.starts_with(b"<svg"), "{path}");
        }
    }
}
