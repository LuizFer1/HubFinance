//! As tres animacoes da janela (spec 11): o scrim do dialogo entrando, o toast entrando e
//! saindo, e o cursor da resposta que chega. O iced nao tem transicao implicita; cada uma e uma
//! `iced::Animation` que a `view` interpola contra `now`. O scrim e o toast ligam o
//! `window::frames()` so enquanto andam (`Motion::animating`); o cursor nao usa quadros: a
//! resposta pode levar minutos, e quadros na taxa da tela a toda hora seria redesenhar 60 vezes
//! por segundo para trocar um caractere duas. Ele tem um relogio proprio, um `Frame` a cada
//! `BLINK` (`Motion::blinking`, `app::blinks`). Fora disso a janela nao redesenha a toa.
//!
//! "Reduzir movimento" (Ajustes) desliga as tres: o estado final aparece na hora e o cursor
//! fica parado, aceso.

use std::time::{Duration, Instant};

use iced::Animation;
use iced::animation::Easing;

/// Entrada do scrim: so a opacidade, de 0 aos 55 % do `theme::scrim`.
pub const SCRIM_IN: Duration = Duration::from_millis(160);
/// Toast subindo da borda de baixo, e descendo ao sair.
pub const TOAST: Duration = Duration::from_millis(200);
/// Meio ciclo do cursor: aceso, depois apagado. O mesmo ritmo do cursor do Windows.
pub const BLINK: Duration = Duration::from_millis(530);

/// O toast que estava na tela: o texto sai do estado no instante em que a saida comeca, entao
/// a copia fica aqui pelos 200 ms da descida.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShownToast {
    pub text: String,
    pub undo: bool,
}

/// O que a tela mostra agora, lido do `App` depois de cada `update`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scene {
    pub dialog: bool,
    pub toast: Option<ShownToast>,
    /// Resposta chegando com a Analise na tela.
    pub streaming: bool,
}

pub struct Motion {
    reduce: bool,
    /// O instante da ultima interpolacao: o do quadro (`Message::Frame`) ou o do `update` que
    /// comecou uma animacao. A `view` le daqui, nunca do relogio, para dois desenhos do mesmo
    /// estado sairem iguais.
    now: Instant,
    scene: Scene,
    scrim: Animation<bool>,
    toast: Animation<bool>,
    /// O ultimo toast mostrado, para a descida.
    last_toast: Option<ShownToast>,
    cursor: Animation<bool>,
}

impl Motion {
    pub fn new(reduce: bool, now: Instant) -> Motion {
        Motion {
            reduce,
            now,
            scene: Scene::default(),
            scrim: settled(false),
            toast: settled(false),
            last_toast: None,
            cursor: settled(true),
        }
    }

    pub fn reduce(&self) -> bool {
        self.reduce
    }

    /// Ligar no meio de uma animacao a encerra: o estado final aparece na hora.
    pub fn set_reduce(&mut self, reduce: bool) {
        self.reduce = reduce;
        if reduce {
            self.scrim = settled(self.scene.dialog);
            self.toast = settled(self.scene.toast.is_some());
            self.cursor = settled(true);
        } else if self.scene.streaming {
            self.cursor = blinking(self.now);
        }
    }

    /// Encerra o que estiver andando, no estado final: sem janela nao chegam quadros, e o
    /// toast que comecou a descer com ela fechando apareceria por um quadro ao reabrir.
    pub fn settle(&mut self) {
        self.scrim = settled(self.scene.dialog);
        self.toast = settled(self.scene.toast.is_some());
    }

    /// Um quadro: so anda o relogio da interpolacao.
    pub fn frame(&mut self, at: Instant) {
        self.now = at;
    }

    /// Compara a cena nova com a anterior e comeca o que mudou. Segundo dialogo por cima do
    /// primeiro nao reinicia o scrim, e toast trocado por outro com o primeiro ainda na tela nao
    /// sobe de novo.
    pub fn sync(&mut self, scene: Scene, now: Instant) {
        let before = std::mem::replace(&mut self.scene, scene);
        let mut started = false;
        if self.scene.dialog != before.dialog {
            self.scrim = if self.scene.dialog && !self.reduce {
                started = true;
                settled(false)
                    .duration(SCRIM_IN)
                    .easing(Easing::EaseOut)
                    .go(true, now)
            } else {
                // A saida do scrim nao anima (spec 11): o dialogo some com ele.
                settled(self.scene.dialog)
            };
        }
        match (&before.toast, &self.scene.toast) {
            (None, Some(_)) | (Some(_), None) => {
                let visible = self.scene.toast.is_some();
                if self.reduce {
                    self.toast = settled(visible);
                } else {
                    started = true;
                    // Do ponto onde esta: toast que some no meio da subida desce dali.
                    self.toast = std::mem::replace(&mut self.toast, settled(false))
                        .duration(TOAST)
                        .easing(Easing::EaseOutCubic)
                        .go(visible, now);
                }
            }
            _ => {}
        }
        if let Some(toast) = &self.scene.toast {
            self.last_toast = Some(toast.clone());
        }
        if self.scene.streaming != before.streaming {
            self.cursor = if self.scene.streaming && !self.reduce {
                started = true;
                blinking(now)
            } else {
                settled(true)
            };
        }
        if started {
            self.now = now;
        }
    }

    /// Scrim ou toast em curso: e o que liga o `window::frames()`. O cursor fica de fora (ver
    /// `blinking`).
    pub fn animating(&self) -> bool {
        !self.reduce && (self.scrim.is_animating(self.now) || self.toast.is_animating(self.now))
    }

    /// Cursor piscando: liga o relogio de piscada, um `Frame` a cada `BLINK`. A piscada troca
    /// no meio de cada meio ciclo (`step`), entao a amostra a cada `BLINK` cai sempre a
    /// 265 ms da troca e acerta o estado mesmo com atraso do relogio.
    pub fn blinking(&self) -> bool {
        !self.reduce && self.scene.streaming
    }

    /// Fracao da opacidade final do scrim (0 a 1).
    pub fn scrim(&self) -> f32 {
        self.scrim.interpolate(0.0, 1.0, self.now)
    }

    /// Quanto do toast aparece (0 = abaixo da borda, 1 = no lugar), e qual: o atual, ou o que
    /// esta descendo. `None` = nenhum na tela.
    pub fn toast(&self) -> Option<(f32, &ShownToast, bool)> {
        let progress = self.toast.interpolate(0.0, 1.0, self.now);
        match &self.scene.toast {
            Some(current) => Some((progress, current, true)),
            None if progress > 0.0 => self.last_toast.as_ref().map(|t| (progress, t, false)),
            None => None,
        }
    }

    /// Cursor aceso agora?
    pub fn cursor_on(&self) -> bool {
        self.cursor.interpolate(0.0_f32, 1.0, self.now) >= 0.5
    }
}

/// Parada no valor: sem transicao, `interpolate` devolve o extremo.
fn settled(value: bool) -> Animation<bool> {
    Animation::new(value)
}

/// Pisca: aceso e apagado por `BLINK` cada, sem fim. Comeca aceso por meio `BLINK` (o degrau
/// fica no meio do meio ciclo: o progresso da `lilt` numa repeticao nunca chega a 1, entao um
/// degrau no fim nunca viraria).
fn blinking(now: Instant) -> Animation<bool> {
    Animation::new(true)
        .duration(BLINK)
        .easing(Easing::Custom(step))
        .repeat_forever()
        .auto_reverse()
        .go(false, now)
}

/// Degrau: o cursor nao esmaece, acende e apaga.
fn step(x: f32) -> f32 {
    if x < 0.5 { 0.0 } else { 1.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toast(text: &str) -> Option<ShownToast> {
        Some(ShownToast {
            text: text.into(),
            undo: false,
        })
    }

    #[test]
    fn parado_nao_anima() {
        let t0 = Instant::now();
        let m = Motion::new(false, t0);
        assert!(!m.animating());
        assert_eq!(m.scrim(), 0.0);
        assert!(m.toast().is_none());
        assert!(m.cursor_on());
    }

    #[test]
    fn scrim_entra_em_160_ms_e_para() {
        let t0 = Instant::now();
        let mut m = Motion::new(false, t0);
        m.sync(
            Scene {
                dialog: true,
                ..Scene::default()
            },
            t0,
        );
        assert!(m.animating());
        assert!(m.scrim() < 0.05);
        m.frame(t0 + SCRIM_IN / 2);
        let half = m.scrim();
        assert!(half > 0.05 && half < 1.0, "{half}");
        m.frame(t0 + SCRIM_IN);
        assert_eq!(m.scrim(), 1.0);
        assert!(!m.animating());
    }

    #[test]
    fn segundo_dialogo_nao_reinicia_o_scrim_e_fechar_e_na_hora() {
        let t0 = Instant::now();
        let mut m = Motion::new(false, t0);
        let open = Scene {
            dialog: true,
            ..Scene::default()
        };
        m.sync(open.clone(), t0);
        m.frame(t0 + SCRIM_IN);
        m.sync(open, t0 + SCRIM_IN);
        assert_eq!(m.scrim(), 1.0);
        assert!(!m.animating());
        m.sync(Scene::default(), t0 + SCRIM_IN * 2);
        assert_eq!(m.scrim(), 0.0);
        assert!(!m.animating());
    }

    #[test]
    fn toast_sobe_desce_e_guarda_o_texto_na_descida() {
        let t0 = Instant::now();
        let mut m = Motion::new(false, t0);
        m.sync(
            Scene {
                toast: toast("Conversa apagada"),
                ..Scene::default()
            },
            t0,
        );
        assert!(m.animating());
        m.frame(t0 + TOAST);
        let (p, shown, live) = m.toast().unwrap();
        assert_eq!(
            (p, shown.text.as_str(), live),
            (1.0, "Conversa apagada", true)
        );
        assert!(!m.animating());

        // Trocar o texto com o toast na tela nao sobe de novo.
        m.sync(
            Scene {
                toast: toast("Outro"),
                ..Scene::default()
            },
            t0 + TOAST,
        );
        assert!(!m.animating());

        let t1 = t0 + TOAST * 2;
        m.sync(Scene::default(), t1);
        assert!(m.animating());
        m.frame(t1 + TOAST / 2);
        let (p, shown, live) = m.toast().unwrap();
        assert!(p > 0.0 && p < 1.0, "{p}");
        assert_eq!(shown.text, "Outro");
        assert!(!live, "o que desce nao recebe clique");
        m.frame(t1 + TOAST);
        assert!(m.toast().is_none());
        assert!(!m.animating());
    }

    #[test]
    fn cursor_pisca_enquanto_a_resposta_chega() {
        let t0 = Instant::now();
        let mut m = Motion::new(false, t0);
        let streaming = Scene {
            streaming: true,
            ..Scene::default()
        };
        m.sync(streaming, t0);
        assert!(!m.animating(), "o cursor nao liga os quadros");
        assert!(m.blinking());
        let ms = |n| t0 + Duration::from_millis(n);
        let samples: Vec<bool> = [100, 400, 700, 1000, 1200, 1500]
            .into_iter()
            .map(|n| {
                m.frame(ms(n));
                m.cursor_on()
            })
            .collect();
        // Aceso ate 265 ms, apagado ate 795, aceso ate 1325, apagado de novo.
        assert_eq!(samples, [true, false, false, true, true, false]);
        // Amostras do relogio de piscada (a cada `BLINK`, com alguns ms de atraso) alternam.
        let ticks: Vec<bool> = (1..=6)
            .map(|k| {
                m.frame(t0 + BLINK * k + Duration::from_millis(7));
                m.cursor_on()
            })
            .collect();
        assert_eq!(ticks, [false, true, false, true, false, true]);
        m.sync(Scene::default(), ms(1600));
        assert!(m.cursor_on());
        assert!(!m.blinking());
    }

    #[test]
    fn reduzir_movimento_mostra_o_final_na_hora() {
        let t0 = Instant::now();
        let mut m = Motion::new(true, t0);
        m.sync(
            Scene {
                dialog: true,
                toast: toast("x"),
                streaming: true,
            },
            t0,
        );
        assert!(!m.animating());
        assert_eq!(m.scrim(), 1.0);
        assert_eq!(m.toast().unwrap().0, 1.0);
        m.frame(t0 + BLINK + BLINK / 2);
        assert!(m.cursor_on(), "parado e aceso");
        m.sync(Scene::default(), t0);
        assert!(m.toast().is_none(), "sem descida");
    }

    #[test]
    fn encerrar_tira_o_toast_que_descia() {
        let t0 = Instant::now();
        let mut m = Motion::new(false, t0);
        m.sync(
            Scene {
                toast: toast("x"),
                ..Scene::default()
            },
            t0,
        );
        m.frame(t0 + TOAST);
        m.sync(Scene::default(), t0 + TOAST);
        assert!(m.toast().is_some(), "descendo");
        m.settle();
        assert!(m.toast().is_none());
        assert!(!m.animating());
    }

    #[test]
    fn ligar_reduzir_no_meio_encerra_a_animacao() {
        let t0 = Instant::now();
        let mut m = Motion::new(false, t0);
        m.sync(
            Scene {
                dialog: true,
                streaming: true,
                ..Scene::default()
            },
            t0,
        );
        assert!(m.animating());
        m.set_reduce(true);
        assert!(!m.animating());
        assert_eq!(m.scrim(), 1.0);
        assert!(!m.blinking());
        m.set_reduce(false);
        assert!(
            m.blinking(),
            "a resposta ainda chega: o cursor volta a piscar"
        );
    }
}
