//! Limiares dos achados (spec do assistente 4, decididos com o dono em 2026-11-25). Os que o
//! dashboard ja tem (ritmo, categoria acima do normal, `BudgetLevel`) nao se repetem aqui: o
//! achado usa o mesmo criterio da tela para nao discordar dela.

use super::Severity;
use crate::dashboard::aggregate::js_round;

/// Poupanca media negativa: os meses fechados gastaram mais do que entrou.
pub const SAVINGS_ALERT_BELOW: i64 = 0;
/// Ate 10% de sobra qualquer imprevisto vira divida; e a faixa em que vale agir.
pub const SAVINGS_ATTENTION_MAX: i64 = 10;
/// 20% e a referencia classica de "guardar um quinto"; daqui para cima e bom sinal.
pub const SAVINGS_GOOD_MIN: i64 = 20;
/// Acima de 60% da renda em fixo sobra pouca margem para cortar quando apertar.
pub const FIXED_ATTENTION_ABOVE: i64 = 60;
/// Mes que vem com mais de 70% ja comprometido deixa menos de 30% para o variavel.
pub const COMMITTED_ATTENTION_ABOVE: i64 = 70;
/// Menos de 3 meses de custo essencial: a reserva nao cobre uma troca de emprego tipica.
pub const EMERGENCY_ATTENTION_BELOW_TENTHS: i64 = 30;
/// 6 meses e a meta padrao do app (`DEFAULT_MULTIPLE`).
pub const EMERGENCY_GOOD_MIN_TENTHS: i64 = 60;
/// Janela das faturas a vencer, em dias.
pub const INVOICE_DAYS: i64 = 10;
/// Quantas categorias acima do normal viram achado.
pub const TOP_CATEGORIES: usize = 3;

pub fn classify_savings(rate: i64) -> Option<Severity> {
    if rate < SAVINGS_ALERT_BELOW {
        Some(Severity::Alert)
    } else if rate <= SAVINGS_ATTENTION_MAX {
        Some(Severity::Attention)
    } else if rate >= SAVINGS_GOOD_MIN {
        Some(Severity::GoodSign)
    } else {
        None
    }
}

pub fn classify_fixed_weight(percent: i64) -> Option<Severity> {
    (percent > FIXED_ATTENTION_ABOVE).then_some(Severity::Attention)
}

pub fn classify_committed(percent: i64) -> Option<Severity> {
    (percent > COMMITTED_ATTENTION_ABOVE).then_some(Severity::Attention)
}

pub fn classify_emergency_tenths(tenths: i64) -> Option<Severity> {
    if tenths < EMERGENCY_ATTENTION_BELOW_TENTHS {
        Some(Severity::Attention)
    } else if tenths >= EMERGENCY_GOOD_MIN_TENTHS {
        Some(Severity::GoodSign)
    } else {
        None
    }
}

/// `part / whole` em % inteira com `js_round`, como o resto do dashboard; `None` sem base.
pub fn percent_of(part: i64, whole: i64) -> Option<i64> {
    (whole > 0).then(|| js_round(part as f64 / whole as f64 * 100.0) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::advisor::Severity::*;

    #[test]
    fn poupanca_nas_bordas() {
        assert_eq!(classify_savings(-1), Some(Alert));
        assert_eq!(classify_savings(0), Some(Attention));
        assert_eq!(classify_savings(10), Some(Attention));
        assert_eq!(classify_savings(11), None);
        assert_eq!(classify_savings(19), None);
        assert_eq!(classify_savings(20), Some(GoodSign));
    }

    #[test]
    fn fixo_so_acima_de_60() {
        assert_eq!(classify_fixed_weight(60), None);
        assert_eq!(classify_fixed_weight(61), Some(Attention));
    }

    #[test]
    fn comprometimento_so_acima_de_70() {
        assert_eq!(classify_committed(70), None);
        assert_eq!(classify_committed(71), Some(Attention));
    }

    #[test]
    fn emergencia_em_decimos_de_mes() {
        assert_eq!(classify_emergency_tenths(29), Some(Attention));
        assert_eq!(classify_emergency_tenths(30), None);
        assert_eq!(classify_emergency_tenths(59), None);
        assert_eq!(classify_emergency_tenths(60), Some(GoodSign));
    }

    #[test]
    fn percentual_inteiro_sem_divisao_por_zero() {
        assert_eq!(percent_of(50, 200), Some(25));
        assert_eq!(percent_of(1, 3), Some(33));
        assert_eq!(percent_of(2, 3), Some(67));
        assert_eq!(percent_of(10, 0), None);
        assert_eq!(percent_of(10, -5), None);
    }
}
