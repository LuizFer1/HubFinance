//! Barras do design: `GroupedBars` (Historico: receita x despesa por mes, com hover) e
//! `MiniBars` (Negocio: uma barra por mes, com media). Cada uma so le a geometria de
//! `geometry` e pinta; nenhuma conta de layout mora aqui.

use gpui::{
    App, Background, BorderStyle, Bounds, Corners, Edges, ElementId, Hsla, IntoElement,
    ParentElement, RenderOnce, Role, SharedString, Size, Styled, Window, canvas, div,
    pattern_slash, prelude::*, px, quad, transparent_black,
};

use super::geometry::{self, Bar, Pt};
use super::{at, plot_of};
use crate::gui::components::tooltip::{Line, TooltipView};
use crate::gui::theme::{Tokens, rgba};

/// Texto do tooltip de um mes, pronto (com a mascara de "Ocultar valores" quando for o caso).
#[derive(Clone, Debug, PartialEq)]
pub struct BarTip {
    pub title: String,
    pub lines: Vec<Line>,
}

impl BarTip {
    /// O tooltip do Historico (Main): mes e tres pares, com "Sobrou" em negrito. Receber os tres
    /// valores prontos garante pelo tipo que nenhum fica de fora.
    pub fn month(
        title: impl Into<String>,
        income: impl Into<SharedString>,
        expense: impl Into<SharedString>,
        net: impl Into<SharedString>,
    ) -> BarTip {
        let pair = |label: &'static str, value: SharedString, bold: bool| Line::Pair {
            label: label.into(),
            value,
            bold,
        };
        BarTip {
            title: title.into(),
            lines: vec![
                pair("Receitas", income.into(), false),
                pair("Despesas", expense.into(), false),
                pair("Sobrou", net.into(), true),
            ],
        }
    }
}

/// Um mes: `None` em `values` = ainda nao existia (coluna vazia, nao zero).
#[derive(Clone, Debug, Default)]
pub struct BarMonth {
    pub values: Option<(i64, i64)>,
    pub tip: Option<BarTip>,
    /// Mes em andamento: as barras vao hachuradas (o valor ainda vai mudar).
    pub partial: bool,
}

fn corners(r: [f32; 4]) -> Corners<gpui::Pixels> {
    Corners {
        top_left: px(r[0]),
        top_right: px(r[1]),
        bottom_right: px(r[2]),
        bottom_left: px(r[3]),
    }
}

fn bar_bounds(bounds: Bounds<gpui::Pixels>, bar: &Bar) -> Bounds<gpui::Pixels> {
    Bounds {
        origin: at(
            bounds.origin,
            Pt {
                x: bar.rect.x,
                y: bar.rect.y,
            },
        ),
        size: Size {
            width: px(bar.rect.w),
            height: px(bar.rect.h),
        },
    }
}

/// Pinta o quad com os raios ja limitados pela geometria (`paint_quad` nao limita sozinho).
fn fill_bar(
    window: &mut Window,
    bounds: Bounds<gpui::Pixels>,
    bar: &Bar,
    background: impl Into<Background>,
    border: (f32, Hsla),
) {
    if bar.rect.h <= 0.0 {
        return;
    }
    window.paint_quad(quad(
        bar_bounds(bounds, bar),
        corners(bar.radii),
        background,
        Edges::all(px(border.0)),
        border.1,
        BorderStyle::default(),
    ));
}

/// Faixas do design (`repeating-linear-gradient(-45deg, ...)`) em pixels logicos: `stripe` de
/// cor e `gap` de vazio medidos na perpendicular. O shader do GPUI mede o periodo como
/// `(largura + intervalo) * sen 45` em pixels de dispositivo, entao sem esta conversao as faixas
/// saem com ~70 % do tamanho a 100 % e mais finas ainda em 150 % e 200 %. Devolve
/// `(largura, intervalo)` para `pattern_slash`.
pub(super) fn hatch_dims(stripe: f32, gap: f32, scale: f32) -> (f32, f32) {
    let period = stripe + gap;
    let total = period / std::f32::consts::FRAC_1_SQRT_2 * scale;
    let width = total * stripe / period;
    (width, total - width)
}

fn hatch(color: Hsla, stripe: f32, gap: f32, window: &Window) -> Background {
    let (w, i) = hatch_dims(stripe, gap, window.scale_factor());
    pattern_slash(color, w, i)
}

#[derive(IntoElement)]
pub struct GroupedBars {
    id: ElementId,
    label: SharedString,
    months: Vec<BarMonth>,
    top: i64,
    height: f32,
    tokens: &'static Tokens,
}

impl GroupedBars {
    /// `top` e o teto do eixo (`aggregate::axis_top`), o mesmo dos rotulos do eixo. O `id` e
    /// obrigatorio: o estado de hover e os ids das colunas moram debaixo dele, e duas instancias
    /// sob o mesmo ancestral (Historico e mini-barras do Dashboard) dividiriam tudo sem ele.
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        months: Vec<BarMonth>,
        top: i64,
        height: f32,
        tokens: &'static Tokens,
    ) -> Self {
        GroupedBars {
            id: id.into(),
            label: label.into(),
            months,
            top,
            height,
            tokens,
        }
    }
}

impl RenderOnce for GroupedBars {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = self.tokens;
        // Coluna sob o mouse. Fica num estado do GPUI para o canvas (que nao ve o hover) ler na
        // hora de pintar o anel; `notify` redesenha so quando a coluna muda.
        let hover =
            window.use_keyed_state(
                (self.id.clone(), "hover"),
                cx,
                |_, _| None::<(usize, usize)>,
            );
        let values: Vec<Option<(i64, i64)>> = self.months.iter().map(|m| m.values).collect();
        let partial: Vec<bool> = self.months.iter().map(|m| m.partial).collect();
        let (top, income, expense) = (self.top, t.income, t.expense);
        let (surface, ring) = (t.surface, t.accent_400);
        let hover_read = hover.clone();
        let paint_layer = canvas(
            |_, _, _| (),
            move |bounds, _, window, cx| {
                let hovered = *hover_read.read(cx);
                let bars = geometry::grouped_bars(&values, top, plot_of(bounds));
                let none = (0.0, transparent_black());
                for (i, pair) in bars.iter().enumerate() {
                    let Some([inc, exp]) = pair else { continue };
                    if let Some((_, k)) = hovered.filter(|(m, _)| *m == i) {
                        // So a barra sob o mouse leva anel (2 px `surface` por dentro, 2 px
                        // `accent_400` por fora), como no artboard. Barra de altura 0 nao tem
                        // o que destacar: `grown` a viraria numa pilula solta na base.
                        let bar = [inc, exp][k];
                        if bar.rect.h > 0.0 {
                            let outer = bar.grown(2.0 * geometry::HOVER_RING);
                            fill_bar(window, bounds, &outer, ring, none);
                            let inner = bar.grown(geometry::HOVER_RING);
                            fill_bar(window, bounds, &inner, surface, none);
                        }
                    }
                    for (bar, color) in [(inc, income), (exp, expense)] {
                        if partial.get(i).copied().unwrap_or(false) {
                            // Design: faixas de 5 px na cor cheia e 5 px a 40 %. A base a 40 % e
                            // as faixas cheias por cima reproduzem isso.
                            fill_bar(window, bounds, bar, color.opacity(0.4), none);
                            fill_bar(window, bounds, bar, hatch(color, 5.0, 5.0, window), none);
                        } else {
                            fill_bar(window, bounds, bar, color, none);
                        }
                    }
                }
            },
        )
        .absolute()
        .size_full();
        // Uma area de hit por barra (metade esquerda = receita, direita = despesa da coluna): o
        // anel e o tooltip seguem a barra sob o mouse, e cada uma carrega o tooltip do mes.
        let hits = self.months.into_iter().enumerate().map(|(i, m)| {
            let halves = (0..2usize).map(|k| {
                let state = hover.clone();
                let half = div()
                    .id((self.id.clone(), format!("col-{i}-{k}")))
                    .flex_1()
                    .h_full()
                    .on_hover(move |entered, _, cx| {
                        state.update(cx, |cur, cx| {
                            let next = if *entered {
                                Some((i, k))
                            } else if *cur == Some((i, k)) {
                                None
                            } else {
                                *cur
                            };
                            if next != *cur {
                                *cur = next;
                                cx.notify();
                            }
                        });
                    });
                match (m.values, m.tip.clone()) {
                    (Some(_), Some(tip)) => half.tooltip(move |_, cx| {
                        TooltipView::rows(tip.title.clone(), tip.lines.clone(), t).build(cx)
                    }),
                    _ => half,
                }
            });
            div().flex_1().h_full().flex().children(halves)
        });
        div()
            .id(self.id.clone())
            .role(Role::Image)
            .aria_label(self.label)
            .relative()
            .w_full()
            .h(px(self.height))
            .child(paint_layer)
            .child(div().absolute().size_full().flex().children(hits))
    }
}

/// Mini-barras do Negocio: uma barra por mes sobre o fundo `highlight`, em `on_highlight`.
#[derive(IntoElement)]
pub struct MiniBars {
    label: SharedString,
    /// `None` = mes que ainda nao existia (sem barra).
    months: Vec<Option<i64>>,
    top: i64,
    height: f32,
    /// Indice do mes em andamento (hachurado), se houver.
    partial: Option<usize>,
    tokens: &'static Tokens,
}

impl MiniBars {
    pub fn new(
        label: impl Into<SharedString>,
        months: Vec<Option<i64>>,
        top: i64,
        height: f32,
        tokens: &'static Tokens,
    ) -> Self {
        MiniBars {
            label: label.into(),
            months,
            top,
            height,
            partial: None,
            tokens,
        }
    }

    pub fn partial(mut self, index: Option<usize>) -> Self {
        self.partial = index;
        self
    }
}

impl RenderOnce for MiniBars {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let MiniBars {
            label,
            months,
            top,
            height,
            partial,
            tokens: t,
        } = self;
        let ink = t.on_highlight;
        let layer = canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let plot = plot_of(bounds);
                let bars = geometry::single_bars(&months, top, plot, 28.0, geometry::MINI_RADII);
                for (i, bar) in bars.iter().enumerate() {
                    let Some(bar) = bar else { continue };
                    if partial == Some(i) {
                        // Main: faixas 4/3 (periodo 7 px) na cor cheia, sem fundo, com a mesma
                        // borda interna de 1 px das barras fechadas: o mes ainda vai mudar.
                        fill_bar(
                            window,
                            bounds,
                            bar,
                            hatch(ink, 4.0, 3.0, window),
                            (1.0, rgba(0, 0, 0, 0.25)),
                        );
                    } else {
                        fill_bar(
                            window,
                            bounds,
                            bar,
                            ink.opacity(0.62),
                            (1.0, rgba(0, 0, 0, 0.25)),
                        );
                    }
                }
            },
        )
        .size_full();
        div()
            .id(ElementId::Name(format!("mini-bars-{}", label).into()))
            .role(Role::Image)
            .aria_label(label)
            .w_full()
            .h(px(height))
            .child(layer)
    }
}

#[cfg(test)]
mod tests {
    use super::hatch_dims;

    /// Periodo perpendicular que o shader pinta, em pixels de dispositivo.
    fn period(dims: (f32, f32)) -> f32 {
        (dims.0 + dims.1) * std::f32::consts::FRAC_1_SQRT_2
    }

    #[test]
    fn faixas_do_design_em_qualquer_escala() {
        for scale in [1.0_f32, 1.5, 2.0] {
            // Historico 5/5 -> periodo 10 px logicos; Negocio 4/3 -> 7 px.
            assert!((period(hatch_dims(5.0, 5.0, scale)) - 10.0 * scale).abs() < 1e-3);
            assert!((period(hatch_dims(4.0, 3.0, scale)) - 7.0 * scale).abs() < 1e-3);
        }
    }

    #[test]
    fn proporcao_faixa_vazio_se_mantem() {
        let (w, i) = hatch_dims(4.0, 3.0, 1.0);
        assert!((w / i - 4.0 / 3.0).abs() < 1e-4);
        let (w, i) = hatch_dims(5.0, 5.0, 1.0);
        assert!((w - i).abs() < 1e-4);
    }
}
