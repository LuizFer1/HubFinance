//! Achados de "onde melhorar" (spec do assistente, seção 4). Puro como `dashboard/`: recebe a
//! visao de UMA conta (`Dataset::for_book`) e "hoje" por parametro, sem relogio, rusqlite, axum,
//! tokio nem iced. Cada numero vem de uma funcao do `dashboard/`, para o achado bater com o
//! que o dashboard mostra; aqui so se decide o que vira achado e com que severidade.

// Nada fora de `advisor/` chama `analyze` ate a tela Analise (Task 2.4); sem isto o clippy do
// binario reclama de codigo morto. Sai junto com a primeira chamada de verdade.
#![allow(dead_code)]

mod rules;
pub mod thresholds;

use crate::dashboard::dataset::Dataset;
use crate::dashboard::periods::month_of;

/// Ordem das variantes = ordem na tela (derive `Ord`): alerta primeiro.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Alert,
    Attention,
    GoodSign,
}

impl Severity {
    pub fn label(self) -> &'static str {
        match self {
            Severity::Alert => "Alerta",
            Severity::Attention => "Atenção",
            Severity::GoodSign => "Bom sinal",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FindingKind {
    Pace,
    Forecast,
    CategoryAboveNormal,
    Budget,
    SavingsRate,
    FixedWeight,
    NextMonthCommitted,
    Emergency,
    Invoices,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FigureValue {
    Money(i64),
    Percent(i64),
    /// Meses com uma casa, em decimos (35 = 3,5 meses).
    MonthsTenths(i64),
    Count(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Figure {
    pub label: String,
    pub value: FigureValue,
}

impl Figure {
    pub fn money(label: &str, minor: i64) -> Self {
        Figure {
            label: label.into(),
            value: FigureValue::Money(minor),
        }
    }
    pub fn percent(label: &str, p: i64) -> Self {
        Figure {
            label: label.into(),
            value: FigureValue::Percent(p),
        }
    }
}

/// Um achado. `title`, `detail` e `action` vao para a tela **e** para o envelope: por isso
/// nunca levam descricao de lancamento, nome de pessoa, de aparelho ou de forma de pagamento
/// (o teste de sentinela de `assistant::envelope` confere).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    pub kind: FindingKind,
    pub severity: Severity,
    pub title: String,
    pub detail: String,
    pub action: Option<String>,
    pub figures: Vec<Figure>,
    /// Peso em centavos para ordenar dentro da mesma severidade (maior primeiro). Tem de ser
    /// comparavel entre regras: e o **excesso em R$ sobre a linha saudavel** de cada uma (fixo
    /// acima de 60% da renda, comprometido acima de 70%, gasto acima do normal, meta estourada,
    /// folga que falta na previsao, distancia a 20% de poupanca, falta para a meta da reserva).
    /// Faturas e meta "perto do limite" pesam o proprio valor, por ser dinheiro a sair.
    pub impact_minor: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Analysis {
    /// `YYYY-MM` de hoje.
    pub month: String,
    pub findings: Vec<Finding>,
}

/// Todos os achados da conta, ordenados por severidade e depois impacto. Achado sem dado
/// suficiente nao e emitido: zerado pareceria "esta tudo bem".
pub fn analyze(ds: &Dataset, today: &str) -> Analysis {
    let mut findings = Vec::new();
    findings.extend(rules::pace(ds, today));
    findings.extend(rules::forecast(ds, today));
    findings.extend(rules::categories(ds, today));
    findings.extend(rules::budgets(ds, today));
    findings.extend(rules::savings(ds, today));
    findings.extend(rules::fixed_weight(ds, today));
    findings.extend(rules::next_month(ds, today));
    findings.extend(rules::emergency(ds, today));
    findings.extend(rules::invoices(ds, today));
    findings.sort_by(|a, b| {
        a.severity
            .cmp(&b.severity)
            .then(b.impact_minor.cmp(&a.impact_minor))
            .then(a.title.cmp(&b.title))
    });
    Analysis {
        month: month_of(today),
        findings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dashboard::{GRAFITE_TODAY, fixtures_grafite};

    #[test]
    fn dataset_vazio_nao_gera_achado() {
        let a = analyze(&Dataset::default(), "2026-11-19");
        assert_eq!(a.month, "2026-11");
        assert!(a.findings.is_empty(), "{:?}", a.findings);
    }

    #[test]
    fn achados_vem_ordenados_por_severidade_e_impacto() {
        let a = analyze(&fixtures_grafite(), GRAFITE_TODAY);
        assert!(!a.findings.is_empty());
        for w in a.findings.windows(2) {
            let (x, y) = (&w[0], &w[1]);
            assert!(
                x.severity < y.severity
                    || (x.severity == y.severity && x.impact_minor >= y.impact_minor),
                "{x:?} antes de {y:?}"
            );
        }
    }
}
