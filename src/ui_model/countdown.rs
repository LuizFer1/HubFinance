//! Prazo que pode ser pausado: o toast do "Desfazer" fica 10 s e para enquanto o mouse esta em
//! cima ou o foco esta nele (artboard Estados). Puro: o relogio entra por parametro (`now`), nunca
//! `Instant::now()` aqui, para o teste mandar o tempo andar sem dormir.
//!
//! Guarda o que ja correu antes da ultima pausa e, se esta correndo, desde quando. Sem isso,
//! pausar e retomar recomecaria os 10 s, e pausar duas vezes seguidas (hover e foco ao mesmo
//! tempo) contaria o intervalo em dobro.
#![allow(dead_code)] // usado na Tarefa 26

use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug)]
pub struct Countdown {
    total: Duration,
    elapsed_before_pause: Duration,
    running_since: Option<Instant>,
}

impl Countdown {
    /// Comeca correndo em `now`.
    pub fn new(total: Duration, now: Instant) -> Countdown {
        Countdown {
            total,
            elapsed_before_pause: Duration::ZERO,
            running_since: Some(now),
        }
    }

    pub fn paused(&self) -> bool {
        self.running_since.is_none()
    }

    /// Para o relogio guardando o que ja correu. Pausar de novo nao faz nada.
    pub fn pause(&mut self, now: Instant) {
        if let Some(since) = self.running_since.take() {
            self.elapsed_before_pause += now.saturating_duration_since(since);
        }
    }

    /// Volta a correr de onde parou. Retomar o que ja corre nao faz nada.
    pub fn resume(&mut self, now: Instant) {
        if self.running_since.is_none() {
            self.running_since = Some(now);
        }
    }

    fn elapsed(&self, now: Instant) -> Duration {
        let running = self
            .running_since
            .map_or(Duration::ZERO, |since| now.saturating_duration_since(since));
        (self.elapsed_before_pause + running).min(self.total)
    }

    pub fn remaining(&self, now: Instant) -> Duration {
        self.total - self.elapsed(now)
    }

    pub fn expired(&self, now: Instant) -> bool {
        self.remaining(now).is_zero()
    }

    /// Fracao que falta (1 = cheio, 0 = acabou): a largura da barra do toast.
    pub fn fraction_left(&self, now: Instant) -> f32 {
        if self.total.is_zero() {
            return 0.0;
        }
        self.remaining(now).as_secs_f32() / self.total.as_secs_f32()
    }

    /// Segundos inteiros que faltam, arredondando para cima: o "(6)" do botao.
    pub fn seconds_left(&self, now: Instant) -> u64 {
        let rest = self.remaining(now);
        rest.as_secs() + u64::from(rest.subsec_nanos() > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOTAL: Duration = Duration::from_secs(10);

    fn at(t0: Instant, secs: u64) -> Instant {
        t0 + Duration::from_secs(secs)
    }

    #[test]
    fn corre_e_expira_no_prazo() {
        let t0 = Instant::now();
        let c = Countdown::new(TOTAL, t0);
        assert_eq!(c.remaining(at(t0, 4)), Duration::from_secs(6));
        assert_eq!(c.seconds_left(at(t0, 4)), 6);
        assert!(!c.expired(at(t0, 9)));
        assert!(c.expired(at(t0, 10)));
        // Depois do prazo nao fica negativo.
        assert_eq!(c.remaining(at(t0, 99)), Duration::ZERO);
        assert_eq!(c.fraction_left(at(t0, 99)), 0.0);
    }

    #[test]
    fn pausa_congela_e_retoma_de_onde_parou() {
        let t0 = Instant::now();
        let mut c = Countdown::new(TOTAL, t0);
        c.pause(at(t0, 4));
        assert!(c.paused());
        // Parado: 60 s depois ainda faltam 6.
        assert_eq!(c.remaining(at(t0, 64)), Duration::from_secs(6));
        assert!(!c.expired(at(t0, 64)));
        c.resume(at(t0, 64));
        assert_eq!(c.remaining(at(t0, 66)), Duration::from_secs(4));
        assert!(c.expired(at(t0, 70)));
    }

    #[test]
    fn pausar_duas_vezes_nao_conta_em_dobro() {
        let t0 = Instant::now();
        let mut c = Countdown::new(TOTAL, t0);
        c.pause(at(t0, 3));
        // Hover e foco pausam juntos: o segundo pause nao soma o intervalo de novo.
        c.pause(at(t0, 8));
        assert_eq!(c.remaining(at(t0, 20)), Duration::from_secs(7));
        c.resume(at(t0, 20));
        c.resume(at(t0, 25));
        assert_eq!(c.remaining(at(t0, 21)), Duration::from_secs(6));
    }

    #[test]
    fn fracao_e_segundos_arredondam_para_cima() {
        let t0 = Instant::now();
        let c = Countdown::new(TOTAL, t0);
        assert_eq!(c.fraction_left(t0), 1.0);
        let t = t0 + Duration::from_millis(3_500);
        assert!((c.fraction_left(t) - 0.65).abs() < 1e-6);
        assert_eq!(c.seconds_left(t), 7);
    }
}
