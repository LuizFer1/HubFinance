//! Geometria dos graficos, pura (sem `gpui`): escalas, barras, fatias, anel e areas empilhadas.
//!
//! Fica separada da pintura para ser testada sem janela. Quem desenha so converte `Rect`/`Pt`
//! em tipos do toolkit; nenhuma conta de layout mora na pintura.
//!
//! O teto "redondo" do eixo vem de `dashboard/` (`aggregate::axis_top`,
//! `reserves::reserve_axis_top`), que e quem sabe o que e um teto; aqui so se aplica a escala.
//! Algumas contas nascem de `ui/charts.rs` (iced), congelada ate a Tarefa 39a: foram copiadas,
//! nao movidas.
#![allow(dead_code)] // usado na Tarefa 18

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
/// Espaco entre as duas barras de um mes.
pub const BAR_GAP: f32 = 2.0;

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

/// Coluna sob `x`; fora da area util (ou sem colunas) -> `None`. A faixa inteira da coluna
/// conta, nao so a barra: o hover do design e na coluna.
pub fn column_at(x: f32, plot: Rect, n: usize) -> Option<usize> {
    if n == 0 || plot.w <= 0.0 || x < plot.x || x >= plot.x + plot.w {
        return None;
    }
    Some((((x - plot.x) / slot_width(plot, n)).floor() as usize).min(n - 1))
}

/// Uma barra com os raios dos quatro cantos (`tl, tr, br, bl`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bar {
    pub rect: Rect,
    pub radii: [f32; 4],
}

/// Raios `6 6 2 2` limitados a metade da largura e a altura: uma barra de 3 px de altura com
/// raio 6 desenharia um quad invertido.
pub fn bar_radii(w: f32, h: f32) -> [f32; 4] {
    let cap = (w / 2.0).min(h).max(0.0);
    let top = BAR_TOP_RADIUS.min(cap);
    let bottom = BAR_BOTTOM_RADIUS.min(cap).min(h / 2.0);
    [top, top, bottom, bottom]
}

/// Receita e despesa de cada mes. `None` = mes que ainda nao existia (coluna vazia, nao zero).
/// Largura da barra: 70 % da coluna dividida entre as duas, entre 2 e 14 px.
pub fn grouped_bars(months: &[Option<(i64, i64)>], top: i64, plot: Rect) -> Vec<Option<[Bar; 2]>> {
    let slot = slot_width(plot, months.len());
    let bar_w = ((slot * 0.7 - BAR_GAP) / 2.0).clamp(2.0, 14.0);
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

/// Onde fica o tooltip (canto superior esquerdo): acima da barra quando cabe; senao ao lado da
/// coluna, no topo da area util, para nao cobrir o mes que se le.
pub fn tooltip_origin(
    tip: (f32, f32),
    col_x: f32,
    col_w: f32,
    bar_top: f32,
    plot_top: f32,
    width: f32,
) -> Pt {
    let above = bar_top - 6.0 - tip.1;
    if above >= 0.0 {
        let x = (col_x + col_w / 2.0 - tip.0 / 2.0).clamp(0.0, (width - tip.0).max(0.0));
        return Pt { x, y: above };
    }
    let right = col_x + col_w + 4.0;
    let x = if right + tip.0 <= width {
        right
    } else {
        (col_x - 4.0 - tip.0).max(0.0)
    };
    Pt { x, y: plot_top }
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
pub fn donut_slices(values: &[i64]) -> Vec<Slice> {
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
    sweeps
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let start = at;
            at += s;
            // A ultima fatia visivel fecha em 360 exato: a soma em f32 deixaria uma fresta.
            let end = if Some(i) == last { 360.0 } else { at };
            Slice { start, end }
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

    #[test]
    fn altura_proporcional_e_travada_no_teto() {
        assert_eq!(scale_height(100, 200, 150.0), 75.0);
        assert_eq!(scale_height(500, 200, 150.0), 150.0);
        assert_eq!(scale_height(-5, 200, 150.0), 0.0);
        assert_eq!(scale_height(5, 0, 150.0), 0.0);
    }

    #[test]
    fn coluna_sob_o_cursor() {
        assert_eq!(column_at(39.0, PLOT, 12), None);
        assert_eq!(column_at(40.0, PLOT, 12), Some(0));
        assert_eq!(column_at(639.9, PLOT, 12), Some(11));
        assert_eq!(column_at(640.0, PLOT, 12), None);
        assert_eq!(column_at(100.0, PLOT, 0), None);
    }

    #[test]
    fn raios_seis_seis_dois_dois_e_limitados() {
        assert_eq!(bar_radii(10.0, 100.0), [5.0, 5.0, 2.0, 2.0]);
        assert_eq!(bar_radii(14.0, 100.0), [6.0, 6.0, 2.0, 2.0]);
        let tiny = bar_radii(14.0, 3.0);
        assert!(tiny.iter().all(|r| *r <= 3.0));
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
    fn largura_da_barra_entre_2_e_14() {
        let narrow = grouped_bars(&[Some((1, 1)); 200], 1, PLOT);
        assert_eq!(narrow[0].unwrap()[0].rect.w, 2.0);
        let wide = grouped_bars(&[Some((1, 1))], 1, PLOT);
        assert_eq!(wide[0].unwrap()[0].rect.w, 14.0);
    }

    #[test]
    fn tooltip_acima_ou_ao_lado() {
        let above = tooltip_origin((100.0, 50.0), 200.0, 40.0, 120.0, 6.0, 600.0);
        assert_eq!(above, Pt { x: 170.0, y: 64.0 });
        let side = tooltip_origin((100.0, 50.0), 200.0, 40.0, 30.0, 6.0, 600.0);
        assert_eq!(side, Pt { x: 244.0, y: 6.0 });
        let edge = tooltip_origin((100.0, 50.0), 540.0, 40.0, 30.0, 6.0, 600.0);
        assert_eq!(edge, Pt { x: 436.0, y: 6.0 });
    }

    #[test]
    fn fatias_somam_360() {
        let s = donut_slices(&[500, 300, 150, 40, 9, 1]);
        assert_eq!(s[0].start, 0.0);
        assert_eq!(s.last().unwrap().end, 360.0);
        for w in s.windows(2) {
            assert_eq!(w[0].end, w[1].start);
        }
    }

    #[test]
    fn fatia_minuscula_ganha_minimo_visivel() {
        let s = donut_slices(&[100_000, 1]);
        assert!(s[1].sweep() >= MIN_SLICE_DEG - 1e-3);
        assert!((s[0].sweep() + s[1].sweep() - 360.0).abs() < 1e-3);
    }

    #[test]
    fn rosca_vazia_e_valor_zero() {
        assert!(donut_slices(&[0, 0]).iter().all(|s| s.sweep() == 0.0));
        let s = donut_slices(&[10, 0, 10]);
        assert_eq!(s[1].sweep(), 0.0);
        assert_eq!(s[2].end, 360.0);
        assert!(donut_slices(&[]).is_empty());
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
