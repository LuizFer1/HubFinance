//! Rosca de "Para onde foi": fatias em arco, total no centro (texto por cima, fora daqui).

use gpui::{
    App, ElementId, Hsla, IntoElement, ParentElement, PathBuilder, RenderOnce, Role, SharedString,
    Styled, Window, canvas, div, point, prelude::*, px,
};

use super::geometry::{self, Pt, Slice};
use super::{at, paint};

#[derive(IntoElement)]
pub struct Donut {
    id: ElementId,
    label: SharedString,
    values: Vec<i64>,
    colors: Vec<Hsla>,
    size: f32,
    thickness: f32,
    empty: Hsla,
}

impl Donut {
    /// `colors[i]` pinta `values[i]`; `empty` e o anel inteiro quando nao ha valor algum.
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        values: Vec<i64>,
        colors: Vec<Hsla>,
        size: f32,
        empty: Hsla,
    ) -> Donut {
        Donut {
            id: id.into(),
            label: label.into(),
            values,
            colors,
            size,
            thickness: 25.0,
            empty,
        }
    }

    pub fn thickness(mut self, t: f32) -> Donut {
        self.thickness = t;
        self
    }
}

/// Arco de `s.start` a `s.end` graus no raio `r`. Acima de 359,9 graus parte em duas metades:
/// um arco SVG cujo fim e igual ao comeco nao desenha nada.
pub(super) fn arc(
    builder: &mut PathBuilder,
    origin: gpui::Point<gpui::Pixels>,
    c: Pt,
    r: f32,
    s: Slice,
) {
    let mut seg = |from: f32, to: f32| {
        let (p0, p1) = (
            geometry::arc_point(c, r, from),
            geometry::arc_point(c, r, to),
        );
        builder.move_to(at(origin, p0));
        builder.arc_to(
            point(px(r), px(r)),
            px(0.0),
            to - from > 180.0,
            true,
            at(origin, p1),
        );
    };
    if s.sweep() >= 359.9 {
        let mid = s.start + 180.0;
        seg(s.start, mid);
        seg(mid, s.end);
    } else if s.sweep() > 0.0 {
        seg(s.start, s.end);
    }
}

impl RenderOnce for Donut {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let (size, thick, empty) = (self.size, self.thickness, self.empty);
        let (values, colors) = (self.values, self.colors);
        let layer = canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let c = Pt {
                    x: size / 2.0,
                    y: size / 2.0,
                };
                let r = (size - thick) / 2.0;
                let slices = geometry::donut_slices(&values, geometry::DONUT_GAP_DEG);
                if slices.iter().all(|s| s.sweep() == 0.0) {
                    let mut b = PathBuilder::stroke(px(thick));
                    arc(
                        &mut b,
                        bounds.origin,
                        c,
                        r,
                        Slice {
                            start: 0.0,
                            end: 360.0,
                        },
                    );
                    paint(window, b, empty);
                    return;
                }
                for (s, color) in slices.iter().zip(colors.iter()) {
                    let mut b = PathBuilder::stroke(px(thick));
                    arc(&mut b, bounds.origin, c, r, *s);
                    paint(window, b, *color);
                }
            },
        )
        .size_full();
        div()
            .id(self.id)
            .role(Role::Image)
            .aria_label(self.label)
            .w(px(self.size))
            .h(px(self.size))
            .child(layer)
    }
}
