//! Geometria dos graficos, pura (sem `gpui`): escalas, barras, fatias, anel e areas empilhadas.
//!
//! Fica separada da pintura para ser testada sem janela. Quem desenha so converte `Rect`/`Pt`
//! em tipos do toolkit; nenhuma conta de layout mora na pintura.
//!
//! O teto "redondo" do eixo vem de `dashboard/` (`aggregate::axis_top`,
//! `reserves::reserve_axis_top`), que e quem sabe o que e um teto; aqui so se aplica a escala.
//! Nada aqui foi copiado de `ui/charts.rs` (iced, congelada ate a Tarefa 39a): aquele tem layout
//! proprio, com margens de eixo; esta geometria foi escrita do zero para o design novo.
#![allow(dead_code)] // usado na Tarefa 19 em diante (a 18 ja usa parte)

use std::ops::Range;

/// Retangulo em pixels logicos, origem no canto superior esquerdo.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pt {
    pub x: f32,
    pub y: f32,
}

/// Raio dos cantos de cima das barras (design: `6 6 2 2`).
pub const BAR_TOP_RADIUS: f32 = 6.0;
pub const BAR_BOTTOM_RADIUS: f32 = 2.0;
/// Fatia menor que isto some a olho nu; a rosca a eleva a este minimo.
pub const MIN_SLICE_DEG: f32 = 4.0;
/// Espaco entre as duas barras de um mes (Main: gap 4 dentro do par).
pub const BAR_GAP: f32 = 4.0;
/// Largura maxima de cada barra do Historico (Main: 18 px).
pub const BAR_MAX_W: f32 = 18.0;
/// Raios das mini-barras do Negocio (Main: `4 4 1 1`).
pub const MINI_RADII: [f32; 4] = [4.0, 4.0, 1.0, 1.0];
/// Espessura do anel de destaque da barra sob o mouse: 2 px `surface` por dentro e 2 px
/// `accent_400` por fora (artboard Componentes, "Tooltips e menus").
pub const HOVER_RING: f32 = 2.0;
/// Fresta entre fatias da rosca: ~0,35 % da volta (Main), em graus.
pub const DONUT_GAP_DEG: f32 = 1.26;

/// Altura de uma barra proporcional ao teto, sem passar dele nem ficar negativa. Teto `<= 0`
/// vira 0: dividir por zero desenharia uma barra infinita.
pub fn scale_height(value: i64, top: i64, plot_h: f32) -> f32 {
    if top <= 0 || plot_h <= 0.0 {
        return 0.0;
    }
    (value as f32 / top as f32).clamp(0.0, 1.0) * plot_h
}

/// Largura de cada coluna quando a area util e dividida em `n` partes iguais.
pub fn slot_width(plot: Rect, n: usize) -> f32 {
    if n == 0 { 0.0 } else { plot.w / n as f32 }
}

/// Uma barra com os raios dos quatro cantos (`tl, tr, br, bl`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bar {
    pub rect: Rect,
    pub radii: [f32; 4],
}

/// Limita os raios a metade do MENOR lado do quad. `paint_quad` nao limita: raios maiores que
/// isso deformam o quad (o doc de `Window::paint_quad` manda usar
/// `Corners::clamp_radii_for_quad_size`, que e esta mesma conta). Com `h/2` por canto, topo +
/// base nunca passam de `h`.
pub fn clamp_radii(radii: [f32; 4], w: f32, h: f32) -> [f32; 4] {
    let cap = (w.min(h) / 2.0).max(0.0);
    radii.map(|r| r.min(cap))
}

/// Raios `6 6 2 2` das barras do Historico, limitados ao tamanho da barra.
pub fn bar_radii(w: f32, h: f32) -> [f32; 4] {
    clamp_radii(
        [
            BAR_TOP_RADIUS,
            BAR_TOP_RADIUS,
            BAR_BOTTOM_RADIUS,
            BAR_BOTTOM_RADIUS,
        ],
        w,
        h,
    )
}

impl Bar {
    /// A barra engrossada em `by` px de cada lado (anel de hover); os raios crescem junto para o
    /// anel acompanhar o contorno.
    pub fn grown(&self, by: f32) -> Bar {
        let rect = Rect {
            x: self.rect.x - by,
            y: self.rect.y - by,
            w: self.rect.w + 2.0 * by,
            h: self.rect.h + 2.0 * by,
        };
        Bar {
            rect,
            radii: self.radii.map(|r| r + by),
        }
    }
}

/// Receita e despesa de cada mes. `None` = mes que ainda nao existia (coluna vazia, nao zero).
/// Largura da barra: 70 % da coluna dividida entre as duas, entre 2 e `BAR_MAX_W` px.
pub fn grouped_bars(months: &[Option<(i64, i64)>], top: i64, plot: Rect) -> Vec<Option<[Bar; 2]>> {
    let slot = slot_width(plot, months.len());
    let bar_w = ((slot * 0.7 - BAR_GAP) / 2.0).clamp(2.0, BAR_MAX_W);
    let base = plot.y + plot.h;
    months
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let (income, expense) = (*m)?;
            let x0 = plot.x + i as f32 * slot + (slot - 2.0 * bar_w - BAR_GAP) / 2.0;
            let make = |k: f32, v: i64| {
                let h = scale_height(v, top, plot.h);
                Bar {
                    rect: Rect {
                        x: x0 + k * (bar_w + BAR_GAP),
                        y: base - h,
                        w: bar_w,
                        h,
                    },
                    radii: bar_radii(bar_w, h),
                }
            };
            Some([make(0.0, income), make(1.0, expense)])
        })
        .collect()
}

/// Uma barra por mes (mini-barras do Negocio): largura = a coluna menos `BAR_GAP`, entre 2 e
/// `max_w`. `None` = mes que ainda nao existia.
pub fn single_bars(
    months: &[Option<i64>],
    top: i64,
    plot: Rect,
    max_w: f32,
    radii: [f32; 4],
) -> Vec<Option<Bar>> {
    let slot = slot_width(plot, months.len());
    let w = (slot - BAR_GAP).clamp(2.0, max_w.max(2.0));
    let base = plot.y + plot.h;
    months
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let h = scale_height((*m)?, top, plot.h);
            Some(Bar {
                rect: Rect {
                    x: plot.x + i as f32 * slot + (slot - w) / 2.0,
                    y: base - h,
                    w,
                    h,
                },
                radii: clamp_radii(radii, w, h),
            })
        })
        .collect()
}

/// Fatia da rosca em graus, 0 = 12 horas, sentido horario.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slice {
    pub start: f32,
    pub end: f32,
}

impl Slice {
    pub fn sweep(&self) -> f32 {
        self.end - self.start
    }
}

/// Fatias proporcionais a `values` somando exatamente 360. Valor `<= 0` fica com fatia vazia
/// (sem pintar); fatia pequena sobe ao minimo visivel e o excesso sai das maiores, para a
/// soma continuar 360. Tudo zero -> todas vazias (a tela mostra o estado vazio).
///
/// `gap` (graus) abre uma fresta entre fatias vizinhas: cada uma perde `gap/2` em cada ponta.
/// Uma fatia so (a volta inteira) nao tem vizinha e fica sem fresta.
pub fn donut_slices(values: &[i64], gap: f32) -> Vec<Slice> {
    let total: i64 = values.iter().map(|v| (*v).max(0)).sum();
    if total <= 0 {
        return vec![
            Slice {
                start: 0.0,
                end: 0.0
            };
            values.len()
        ];
    }
    let mut sweeps: Vec<f32> = values
        .iter()
        .map(|v| (*v).max(0) as f32 / total as f32 * 360.0)
        .collect();
    let mut deficit = 0.0;
    for s in sweeps.iter_mut() {
        if *s > 0.0 && *s < MIN_SLICE_DEG {
            deficit += MIN_SLICE_DEG - *s;
            *s = MIN_SLICE_DEG;
        }
    }
    if deficit > 0.0 {
        let big: f32 = sweeps.iter().filter(|s| **s > MIN_SLICE_DEG).sum();
        if big > 0.0 {
            for s in sweeps.iter_mut().filter(|s| **s > MIN_SLICE_DEG) {
                *s -= deficit * (*s / big);
            }
        }
    }
    let last = sweeps.iter().rposition(|s| *s > 0.0);
    let mut at = 0.0;
    let visible = sweeps.iter().filter(|s| **s > 0.0).count();
    sweeps
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let start = at;
            at += s;
            // A ultima fatia visivel fecha em 360 exato: a soma em f32 deixaria uma fresta.
            let end = if Some(i) == last { 360.0 } else { at };
            if visible > 1 && *s > 0.0 {
                // Nunca come mais que um quarto da fatia de cada lado: fatia no minimo visivel
                // (4 graus) continua aparecendo.
                let cut = (gap / 2.0).min(s / 4.0).max(0.0);
                Slice {
                    start: start + cut,
                    end: end - cut,
                }
            } else {
                Slice { start, end }
            }
        })
        .collect()
}

/// Ponto do circulo para um angulo em graus (0 = topo, horario).
pub fn arc_point(center: Pt, radius: f32, deg: f32) -> Pt {
    let rad = deg.to_radians();
    Pt {
        x: center.x + radius * rad.sin(),
        y: center.y - radius * rad.cos(),
    }
}

/// Graus do anel de percentual: 0..=360. NaN e negativo viram 0; passou de 100 % trava em 360
/// (a reserva acima da meta nao da a volta de novo).
pub fn ring_sweep(fraction: f64) -> f32 {
    if !fraction.is_finite() {
        return 0.0;
    }
    (fraction.clamp(0.0, 1.0) * 360.0) as f32
}

/// Poligonos da area empilhada, de baixo para cima: o da serie `k` vai pela borda de cima
/// (acumulado ate `k`) e volta pela de baixo (acumulado ate `k-1`). `series[k][i]` = saldo do
/// mes `i`. Um unico mes vira ponto no centro, nao uma divisao por zero.
pub fn stacked_polygons(series: &[Vec<i64>], top: i64, plot: Rect) -> Vec<Vec<Pt>> {
    let n = series.iter().map(Vec::len).max().unwrap_or(0);
    if n == 0 || top <= 0 {
        return series.iter().map(|_| Vec::new()).collect();
    }
    let x_of = |i: usize| {
        if n == 1 {
            plot.x + plot.w / 2.0
        } else {
            plot.x + plot.w * i as f32 / (n - 1) as f32
        }
    };
    let base = plot.y + plot.h;
    let mut below = vec![0i64; n];
    series
        .iter()
        .map(|s| {
            let mut upper = Vec::with_capacity(n * 2);
            let mut lower = Vec::with_capacity(n);
            for (i, acc) in below.iter_mut().enumerate() {
                let v = s.get(i).copied().unwrap_or(0).max(0);
                let from = *acc;
                let to = from.saturating_add(v);
                let x = x_of(i);
                lower.push(Pt {
                    x,
                    y: base - scale_height(from, top, plot.h),
                });
                upper.push(Pt {
                    x,
                    y: base - scale_height(to, top, plot.h),
                });
                *acc = to;
            }
            lower.reverse();
            upper.extend(lower);
            upper
        })
        .collect()
}

/// Indices dos pontos da linha: `(solido, tracejado)`. Com o ultimo mes parcial, o trecho que
/// chega nele e tracejado (o valor ainda vai mudar); o solido termina no penultimo.
pub fn partial_segments(points: usize, partial: bool) -> (Range<usize>, Option<Range<usize>>) {
    if partial && points >= 2 {
        (0..points - 1, Some(points - 2..points))
    } else {
        (0..points, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLOT: Rect = Rect {
        x: 40.0,
        y: 6.0,
        w: 600.0,
        h: 150.0,
    };

    fn bar_at(x: f32, y: f32, w: f32, h: f32) -> Bar {
        Bar {
            rect: Rect { x, y, w, h },
            radii: bar_radii(w, h),
        }
    }

    #[test]
    fn altura_proporcional_e_travada_no_teto() {
        assert_eq!(scale_height(100, 200, 150.0), 75.0);
        assert_eq!(scale_height(500, 200, 150.0), 150.0);
        assert_eq!(scale_height(-5, 200, 150.0), 0.0);
        assert_eq!(scale_height(5, 0, 150.0), 0.0);
    }

    #[test]
    fn raios_seis_seis_dois_dois_e_limitados() {
        assert_eq!(bar_radii(10.0, 100.0), [5.0, 5.0, 2.0, 2.0]);
        assert_eq!(bar_radii(14.0, 100.0), [6.0, 6.0, 2.0, 2.0]);
        // Barra baixa: topo + base nunca passam da altura (paint_quad nao limita sozinho).
        for h in [0.5, 1.0, 3.0, 5.0, 7.0, 12.0] {
            let [tl, tr, br, bl] = bar_radii(14.0, h);
            assert!(tl + bl <= h + 1e-4 && tr + br <= h + 1e-4, "h={h}");
            assert!(tl <= 7.0 && tl <= h / 2.0 + 1e-4);
        }
        assert_eq!(bar_radii(14.0, 3.0), [1.5, 1.5, 1.5, 1.5]);
        assert_eq!(bar_radii(14.0, 0.0), [0.0; 4]);
    }

    #[test]
    fn barras_agrupadas_sentam_na_base_e_pulam_mes_inexistente() {
        let bars = grouped_bars(&[None, Some((200, 100))], 200, PLOT);
        assert!(bars[0].is_none());
        let [inc, exp] = bars[1].expect("mes existe");
        let base = PLOT.y + PLOT.h;
        assert_eq!(inc.rect.y + inc.rect.h, base);
        assert_eq!(inc.rect.h, PLOT.h);
        assert_eq!(exp.rect.h, PLOT.h / 2.0);
        // Dentro da propria coluna (a segunda de duas).
        assert!(inc.rect.x >= PLOT.x + PLOT.w / 2.0);
        assert!(exp.rect.x > inc.rect.x);
    }

    #[test]
    fn largura_da_barra_entre_2_e_18_com_gap_4() {
        let narrow = grouped_bars(&[Some((1, 1)); 200], 1, PLOT);
        assert_eq!(narrow[0].unwrap()[0].rect.w, 2.0);
        let wide = grouped_bars(&[Some((1, 1))], 1, PLOT);
        let [a, b] = wide[0].unwrap();
        assert_eq!(a.rect.w, 18.0);
        assert_eq!(b.rect.x - (a.rect.x + a.rect.w), 4.0);
    }

    #[test]
    fn fatias_somam_360() {
        let s = donut_slices(&[500, 300, 150, 40, 9, 1], 0.0);
        assert_eq!(s[0].start, 0.0);
        assert_eq!(s.last().unwrap().end, 360.0);
        for w in s.windows(2) {
            assert_eq!(w[0].end, w[1].start);
        }
    }

    #[test]
    fn fatia_minuscula_ganha_minimo_visivel() {
        let s = donut_slices(&[100_000, 1], 0.0);
        assert!(s[1].sweep() >= MIN_SLICE_DEG - 1e-3);
        assert!((s[0].sweep() + s[1].sweep() - 360.0).abs() < 1e-3);
    }

    #[test]
    fn fresta_entre_fatias_vizinhas() {
        let s = donut_slices(&[500, 300, 200], DONUT_GAP_DEG);
        for w in s.windows(2) {
            assert!((w[1].start - w[0].end - DONUT_GAP_DEG).abs() < 1e-3);
        }
        // A fresta tambem separa a ultima fatia da primeira, na volta.
        assert!((s[0].start + (360.0 - s[2].end) - DONUT_GAP_DEG).abs() < 1e-3);
        // Fatia unica = volta inteira, sem fresta; fatia minima segue visivel.
        let one = donut_slices(&[10, 0], DONUT_GAP_DEG);
        assert_eq!((one[0].start, one[0].end), (0.0, 360.0));
        let tiny = donut_slices(&[100_000, 1], DONUT_GAP_DEG);
        assert!(tiny[1].sweep() > 1.0);
    }

    #[test]
    fn barra_unica_com_largura_maxima_e_raios_do_negocio() {
        let months = [Some(100), None, Some(50)];
        let bars = single_bars(&months, 100, PLOT, 28.0, MINI_RADII);
        let a = bars[0].unwrap();
        assert_eq!(a.rect.w, 28.0);
        assert_eq!(a.rect.h, PLOT.h);
        assert!(bars[1].is_none());
        assert_eq!(bars[2].unwrap().rect.h, PLOT.h / 2.0);
        assert_eq!(a.radii, MINI_RADII);
        // Barra baixa limita o raio.
        let low = single_bars(&[Some(1)], 1000, PLOT, 28.0, MINI_RADII)[0].unwrap();
        assert!(low.radii[0] <= low.rect.h / 2.0 + 1e-4);
        // Coluna estreita: nunca abaixo de 2 px.
        let narrow = single_bars(&[Some(1); 400], 1, PLOT, 28.0, MINI_RADII);
        assert_eq!(narrow[0].unwrap().rect.w, 2.0);
    }

    #[test]
    fn anel_de_hover_cresce_barra_e_raios() {
        let bar = bar_at(10.0, 20.0, 18.0, 40.0);
        let ring = bar.grown(HOVER_RING);
        assert_eq!(
            ring.rect,
            Rect {
                x: 8.0,
                y: 18.0,
                w: 22.0,
                h: 44.0
            }
        );
        assert_eq!(ring.radii, [8.0, 8.0, 4.0, 4.0]);
        let outer = bar.grown(2.0 * HOVER_RING);
        assert_eq!(outer.rect.w, 26.0);
    }

    #[test]
    fn rosca_vazia_e_valor_zero() {
        assert!(
            donut_slices(&[0, 0], DONUT_GAP_DEG)
                .iter()
                .all(|s| s.sweep() == 0.0)
        );
        let s = donut_slices(&[10, 0, 10], 0.0);
        assert_eq!(s[1].sweep(), 0.0);
        assert_eq!(s[2].end, 360.0);
        assert!(donut_slices(&[], DONUT_GAP_DEG).is_empty());
    }

    #[test]
    fn ponto_do_arco_zero_no_topo_horario() {
        let c = Pt { x: 50.0, y: 50.0 };
        let top = arc_point(c, 10.0, 0.0);
        assert!((top.x - 50.0).abs() < 1e-4 && (top.y - 40.0).abs() < 1e-4);
        let right = arc_point(c, 10.0, 90.0);
        assert!((right.x - 60.0).abs() < 1e-4 && (right.y - 50.0).abs() < 1e-4);
    }

    #[test]
    fn anel_trava_e_nao_aceita_nan() {
        assert_eq!(ring_sweep(0.5), 180.0);
        assert_eq!(ring_sweep(3.0), 360.0);
        assert_eq!(ring_sweep(-1.0), 0.0);
        assert_eq!(ring_sweep(f64::NAN), 0.0);
    }

    #[test]
    fn area_empilhada_fecha_com_a_borda_de_baixo() {
        let plot = Rect {
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 100.0,
        };
        let polys = stacked_polygons(&[vec![10, 20], vec![10, 20]], 40, plot);
        assert_eq!(polys.len(), 2);
        // Serie 0: base 100 -> topo 75 (10/40) e 50 (20/40); volta pela base.
        assert_eq!(polys[0][0], Pt { x: 0.0, y: 75.0 });
        assert_eq!(polys[0][1], Pt { x: 100.0, y: 50.0 });
        assert_eq!(polys[0][2], Pt { x: 100.0, y: 100.0 });
        // Serie 1 comeca onde a 0 terminou.
        assert_eq!(polys[1][2], Pt { x: 100.0, y: 50.0 });
        assert_eq!(polys[1][1], Pt { x: 100.0, y: 0.0 });
    }

    #[test]
    fn area_com_um_mes_e_teto_zero() {
        let plot = Rect {
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 100.0,
        };
        let one = stacked_polygons(&[vec![10]], 40, plot);
        assert!(one[0].iter().all(|p| p.x == 50.0));
        assert!(stacked_polygons(&[vec![10]], 0, plot)[0].is_empty());
    }

    #[test]
    fn trecho_tracejado_so_com_mes_parcial() {
        assert_eq!(partial_segments(6, false), (0..6, None));
        assert_eq!(partial_segments(6, true), (0..5, Some(4..6)));
        assert_eq!(partial_segments(1, true), (0..1, None));
    }
}
