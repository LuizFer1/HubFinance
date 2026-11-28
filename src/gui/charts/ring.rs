//! Anel de percentual da reserva de emergencia (o numero fica por cima, em texto).

use gpui::{
    App, ElementId, Hsla, IntoElement, ParentElement, PathBuilder, RenderOnce, Role, SharedString,
    Styled, Window, canvas, div, prelude::*, px,
};

use super::donut::arc;
use super::geometry::{self, Pt, Slice};
use super::paint;

#[derive(IntoElement)]
pub struct Ring {
    label: SharedString,
    fraction: f64,
    size: f32,
    thickness: f32,
    fill: Hsla,
    track: Hsla,
}

impl Ring {
    /// Espessura 11: o anel do design (Reservas) tem 104 de lado e miolo de 82.
    pub fn new(
        label: impl Into<SharedString>,
        fraction: f64,
        size: f32,
        fill: Hsla,
        track: Hsla,
    ) -> Ring {
        Ring {
            label: label.into(),
            fraction,
            size,
            thickness: 11.0,
            fill,
            track,
        }
    }

    pub fn thickness(mut self, t: f32) -> Ring {
        self.thickness = t;
        self
    }
}

impl RenderOnce for Ring {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let (size, thick, fill, track) = (self.size, self.thickness, self.fill, self.track);
        let sweep = geometry::ring_sweep(self.fraction);
        let layer = canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let c = Pt {
                    x: size / 2.0,
                    y: size / 2.0,
                };
                let r = (size - thick) / 2.0;
                let mut t = PathBuilder::stroke(px(thick));
                arc(
                    &mut t,
                    bounds.origin,
                    c,
                    r,
                    Slice {
                        start: 0.0,
                        end: 360.0,
                    },
                );
                paint(window, t, track);
                if sweep > 0.0 {
                    let mut f = PathBuilder::stroke(px(thick));
                    arc(
                        &mut f,
                        bounds.origin,
                        c,
                        r,
                        Slice {
                            start: 0.0,
                            end: sweep,
                        },
                    );
                    paint(window, f, fill);
                }
            },
        )
        .size_full();
        div()
            .id(ElementId::Name(format!("ring-{}", self.label).into()))
            .role(Role::Image)
            .aria_label(self.label)
            .w(px(self.size))
            .h(px(self.size))
            .child(layer)
    }
}
