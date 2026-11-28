//! Barras agrupadas (Historico, mini-barras do Negocio): receita x despesa por mes.

use gpui::{
    App, BorderStyle, Bounds, Corners, Edges, Hsla, IntoElement, ParentElement, RenderOnce, Size,
    Styled, Window, canvas, div, prelude::*, px, quad, transparent_black,
};

use super::geometry::{self, Bar};
use super::{at, plot_of};
use crate::gui::components::tooltip::{Line, TooltipView};
use crate::gui::theme::Tokens;

/// Texto do tooltip de um mes, pronto (com a mascara de "Ocultar valores" quando for o caso).
#[derive(Clone, Debug, PartialEq)]
pub struct BarTip {
    pub title: String,
    pub lines: Vec<Line>,
}

/// Um mes: `None` em `values` = ainda nao existia (coluna vazia, nao zero).
#[derive(Clone, Debug, Default)]
pub struct BarMonth {
    pub values: Option<(i64, i64)>,
    pub tip: Option<BarTip>,
}

#[derive(IntoElement)]
pub struct GroupedBars {
    months: Vec<BarMonth>,
    top: i64,
    height: f32,
    tokens: &'static Tokens,
}

impl GroupedBars {
    /// `top` e o teto do eixo (`aggregate::axis_top`), o mesmo dos rotulos do eixo.
    pub fn new(months: Vec<BarMonth>, top: i64, height: f32, tokens: &'static Tokens) -> Self {
        GroupedBars {
            months,
            top,
            height,
            tokens,
        }
    }
}

fn corners(r: [f32; 4]) -> Corners<gpui::Pixels> {
    Corners {
        top_left: px(r[0]),
        top_right: px(r[1]),
        bottom_right: px(r[2]),
        bottom_left: px(r[3]),
    }
}

fn paint_bar(window: &mut Window, bounds: Bounds<gpui::Pixels>, bar: &Bar, color: Hsla) {
    if bar.rect.h <= 0.0 {
        return;
    }
    let b = Bounds {
        origin: at(
            bounds.origin,
            geometry::Pt {
                x: bar.rect.x,
                y: bar.rect.y,
            },
        ),
        size: Size {
            width: px(bar.rect.w),
            height: px(bar.rect.h),
        },
    };
    window.paint_quad(quad(
        b,
        corners(bar.radii),
        color,
        Edges::default(),
        transparent_black(),
        BorderStyle::default(),
    ));
}

impl RenderOnce for GroupedBars {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let t = self.tokens;
        let values: Vec<Option<(i64, i64)>> = self.months.iter().map(|m| m.values).collect();
        let (top, income, expense) = (self.top, t.income, t.expense);
        let paint = canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let bars = geometry::grouped_bars(&values, top, plot_of(bounds));
                for [inc, exp] in bars.iter().flatten() {
                    paint_bar(window, bounds, inc, income);
                    paint_bar(window, bounds, exp, expense);
                }
            },
        )
        .absolute()
        .size_full();
        // Colunas transparentes por cima: o hover e na coluna inteira (geometry::column_at) e
        // cada uma carrega o tooltip pronto do modelo.
        let hits = self.months.into_iter().enumerate().map(|(i, m)| {
            let col = div().id(("bar-col", i)).flex_1().h_full();
            match (m.values, m.tip) {
                (Some(_), Some(tip)) => col.tooltip(move |_, cx| {
                    TooltipView::rows(tip.title.clone(), tip.lines.clone(), t).build(cx)
                }),
                _ => col,
            }
        });
        div()
            .relative()
            .w_full()
            .h(px(self.height))
            .child(paint)
            .child(div().absolute().size_full().flex().children(hits))
    }
}
