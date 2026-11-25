//! Uma funcao por achado. Cada uma le so `dashboard/` e devolve zero ou mais achados; os
//! numeros saem das mesmas funcoes que desenham o dashboard.

use super::thresholds::{
    INVOICE_DAYS, TOP_CATEGORIES, classify_committed, classify_emergency_tenths,
    classify_fixed_weight, classify_savings, percent_of,
};
use super::{Figure, FigureValue, Finding, FindingKind, Severity};
use crate::dashboard::aggregate::{MonthTotals, NO_CATEGORY_KEY, NO_CATEGORY_NAME, monthly_totals};
use crate::dashboard::budgets::{BudgetLevel, budget_report};
use crate::dashboard::cards::upcoming_invoices;
use crate::dashboard::dataset::Dataset;
use crate::dashboard::insights::{self, SURPLUS_MONTHS};
use crate::dashboard::money::format_brl;
use crate::dashboard::periods::{last_months, month_of};
use crate::dashboard::recurrence::next_month_free;
use crate::dashboard::reserves::{emergency_view, ordered};

pub(super) fn pace(ds: &Dataset, today: &str) -> Option<Finding> {
    let month = month_of(today);
    let sp = insights::spending_pace(ds, &month, today);
    let p = sp.change_percent.filter(|p| *p > 0)?;
    let normal = sp.normal_at_cutoff_minor?;
    Some(Finding {
        kind: FindingKind::Pace,
        severity: Severity::Attention,
        title: format!("Gastos do mês {p}% acima do seu normal"),
        detail: format!(
            "Até hoje saíram {}; no mesmo dia, o seu normal é {}.",
            format_brl(sp.spent_minor),
            format_brl(normal)
        ),
        action: None,
        figures: vec![
            Figure::money("Gasto até hoje", sp.spent_minor),
            Figure::money("Normal até hoje", normal),
            Figure::percent("Variação", p),
        ],
        impact_minor: sp.spent_minor.saturating_sub(normal),
    })
}

pub(super) fn forecast(ds: &Dataset, today: &str) -> Option<Finding> {
    let f = insights::forecast(ds, today)?;
    let surplus = insights::round_to_tens(f.expected_surplus_minor);
    let (severity, title, action) = match surplus.cmp(&0) {
        std::cmp::Ordering::Less => (
            Severity::Alert,
            format!("Previsão: faltar ~{} no fim do mês", format_brl(-surplus)),
            insights::zero_allowance(&f).map(|per_day| {
                format!(
                    "Para fechar no zero, gaste até {} por dia até o fim do mês",
                    format_brl(per_day.max(0))
                )
            }),
        ),
        std::cmp::Ordering::Greater => (
            Severity::GoodSign,
            format!("Previsão: sobrar ~{} no fim do mês", format_brl(surplus)),
            None,
        ),
        std::cmp::Ordering::Equal => return None,
    };
    Some(Finding {
        kind: FindingKind::Forecast,
        severity,
        title,
        detail: format!(
            "Receita prevista {}, despesa prevista {}, no ritmo atual.",
            format_brl(f.expected_income_minor),
            format_brl(f.expected_expense_minor)
        ),
        action,
        figures: vec![
            Figure::money("Receita prevista", f.expected_income_minor),
            Figure::money("Despesa prevista", f.expected_expense_minor),
            Figure::money("Sobra prevista", surplus),
        ],
        impact_minor: surplus.saturating_abs(),
    })
}

fn category_label(ds: &Dataset, key: &str) -> String {
    if key == NO_CATEGORY_KEY {
        NO_CATEGORY_NAME.to_string()
    } else {
        ds.category_name(Some(key))
    }
}

pub(super) fn categories(ds: &Dataset, today: &str) -> Vec<Finding> {
    let month = month_of(today);
    let mut marked: Vec<_> = insights::categories_above_normal(ds, &month, today)
        .into_iter()
        .map(|(key, c)| (key, c.spent_minor.saturating_sub(c.normal_minor), c))
        .collect();
    marked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    marked
        .into_iter()
        .take(TOP_CATEGORIES)
        .map(|(key, diff, c)| {
            let name = category_label(ds, &key);
            Finding {
                kind: FindingKind::CategoryAboveNormal,
                severity: Severity::Attention,
                title: format!("{name} {}% acima do normal", c.change_percent),
                detail: format!(
                    "{} este mês contra {} de costume no mesmo período.",
                    format_brl(c.spent_minor),
                    format_brl(c.normal_minor)
                ),
                action: Some(format!(
                    "Se voltar ao normal, sobram {} este mês",
                    format_brl(diff)
                )),
                figures: vec![
                    Figure::money(&format!("{name} no mês"), c.spent_minor),
                    Figure::money(&format!("{name} normal"), c.normal_minor),
                ],
                impact_minor: diff,
            }
        })
        .collect()
}

pub(super) fn budgets(ds: &Dataset, today: &str) -> Vec<Finding> {
    budget_report(ds, &month_of(today))
        .into_iter()
        .filter_map(|b| {
            let (severity, title, detail, impact) = match b.level {
                BudgetLevel::Over => (
                    Severity::Alert,
                    format!("Meta de {} estourada", b.name),
                    format!(
                        "Gasto {} de {} (passou {}).",
                        format_brl(b.spent_minor),
                        format_brl(b.limit_minor),
                        format_brl(b.over_minor)
                    ),
                    b.over_minor,
                ),
                BudgetLevel::Warning => (
                    Severity::Attention,
                    format!("Meta de {} perto do limite", b.name),
                    format!(
                        "Restam {} de {}.",
                        format_brl(b.remaining_minor.max(0)),
                        format_brl(b.limit_minor)
                    ),
                    b.spent_minor,
                ),
                BudgetLevel::Ok => return None,
            };
            Some(Finding {
                kind: FindingKind::Budget,
                severity,
                title,
                detail,
                action: None,
                figures: vec![
                    Figure::money(&format!("Meta {}", b.name), b.limit_minor),
                    Figure::money(&format!("Gasto {}", b.name), b.spent_minor),
                ],
                impact_minor: impact,
            })
        })
        .collect()
}

/// Medias dos meses **fechados** (os ate 12 anteriores ao de hoje, a partir do primeiro
/// lancamento), a mesma janela de `average_surplus_closed`: renda media e base de fixo,
/// comprometimento e poupanca, para os tres falarem da mesma renda.
pub(super) struct ClosedAverages {
    pub months: usize,
    pub income_minor: i64,
    pub surplus_minor: i64,
}

pub(super) fn closed_averages(ds: &Dataset, today: &str) -> Option<ClosedAverages> {
    let current = month_of(today);
    let first = insights::first_month(ds)?;
    let months = last_months(&current, usize::try_from(SURPLUS_MONTHS).unwrap_or(12) + 1);
    let totals: Vec<MonthTotals> = monthly_totals(ds, &months);
    let surplus = insights::average_surplus_closed(&totals, Some(&first), &current)?;
    let closed: Vec<&MonthTotals> = totals
        .iter()
        .filter(|t| t.month < current && t.month >= first)
        .collect();
    let n = i64::try_from(closed.len()).ok().filter(|n| *n > 0)?;
    let income: i64 = closed
        .iter()
        .fold(0i64, |a, t| a.saturating_add(t.income_minor));
    Some(ClosedAverages {
        months: closed.len(),
        income_minor: crate::dashboard::reserves::div_round(income, n),
        surplus_minor: surplus.average_minor,
    })
}

pub(super) fn savings(ds: &Dataset, today: &str) -> Option<Finding> {
    let avg = closed_averages(ds, today)?;
    let rate = percent_of(avg.surplus_minor, avg.income_minor)?;
    let severity = classify_savings(rate)?;
    let title = match severity {
        Severity::Alert => format!("Nos últimos meses saiu mais do que entrou ({rate}%)"),
        Severity::Attention => format!("Sobra média de só {rate}% da renda"),
        Severity::GoodSign => format!("Você guarda em média {rate}% da renda"),
    };
    Some(Finding {
        kind: FindingKind::SavingsRate,
        severity,
        title,
        detail: format!(
            "Média {}: renda {}, sobra {}.",
            if avg.months == 1 {
                "do último mês fechado".to_string()
            } else {
                format!("dos {} meses fechados", avg.months)
            },
            format_brl(avg.income_minor),
            format_brl(avg.surplus_minor)
        ),
        action: (severity != Severity::GoodSign)
            .then(|| "Mirar em guardar 20% da renda por mês".to_string()),
        figures: vec![
            Figure::money("Renda média", avg.income_minor),
            Figure::money("Sobra média", avg.surplus_minor),
            Figure::percent("Taxa de poupança", rate),
        ],
        impact_minor: avg.surplus_minor.saturating_abs(),
    })
}

pub(super) fn fixed_weight(ds: &Dataset, today: &str) -> Option<Finding> {
    let avg = closed_averages(ds, today)?;
    let fv = insights::fixed_variable(ds, &month_of(today));
    let p = percent_of(fv.fixed_minor, avg.income_minor)?;
    let severity = classify_fixed_weight(p)?;
    Some(Finding {
        kind: FindingKind::FixedWeight,
        severity,
        title: format!("Gastos fixos levam {p}% da renda"),
        detail: format!(
            "{} fixos este mês contra renda média de {}.",
            format_brl(fv.fixed_minor),
            format_brl(avg.income_minor)
        ),
        action: Some("Revisar contratos e assinaturas abre margem todo mês".into()),
        figures: vec![
            Figure::money("Fixo no mês", fv.fixed_minor),
            Figure::percent("Fixo sobre a renda", p),
        ],
        impact_minor: fv.fixed_minor,
    })
}

pub(super) fn next_month(ds: &Dataset, today: &str) -> Option<Finding> {
    let avg = closed_averages(ds, today)?;
    let rm = next_month_free(ds, today);
    let p = percent_of(rm.expense_minor, avg.income_minor)?;
    let severity = classify_committed(p)?;
    Some(Finding {
        kind: FindingKind::NextMonthCommitted,
        severity,
        title: format!("Mês que vem já tem {p}% da renda comprometida"),
        detail: format!(
            "{} em recorrências e parcelas contra renda média de {}.",
            format_brl(rm.expense_minor),
            format_brl(avg.income_minor)
        ),
        action: Some("Evitar novas parcelas até o comprometimento baixar".into()),
        figures: vec![
            Figure::money("Comprometido no mês que vem", rm.expense_minor),
            Figure::percent("Sobre a renda", p),
        ],
        impact_minor: rm.expense_minor,
    })
}

pub(super) fn emergency(ds: &Dataset, today: &str) -> Option<Finding> {
    let current = month_of(today);
    let (Some(r), _) = ordered(ds) else {
        return None;
    };
    let view = emergency_view(ds, r, &current);
    let cost = view.cost.as_ref().filter(|c| c.total_minor > 0)?;
    // Decimos de mes por inteiros: o `coverage` f64 da view e so exibicao.
    let tenths = view.balance_minor.saturating_mul(10) / cost.total_minor;
    let severity = classify_emergency_tenths(tenths)?;
    let title = match severity {
        Severity::GoodSign => format!("Reserva de emergência cobre {} meses", view.coverage_label),
        _ => format!(
            "Reserva de emergência cobre só {} meses",
            view.coverage_label
        ),
    };
    Some(Finding {
        kind: FindingKind::Emergency,
        severity,
        title,
        detail: format!(
            "Saldo {} para um custo essencial de {} por mês.",
            format_brl(view.balance_minor),
            format_brl(cost.total_minor)
        ),
        action: (severity != Severity::GoodSign).then(|| view.remaining_label.clone()),
        figures: vec![
            Figure::money("Saldo da reserva", view.balance_minor),
            Figure::money("Custo essencial mensal", cost.total_minor),
            Figure {
                label: "Meses cobertos".into(),
                value: FigureValue::MonthsTenths(tenths),
            },
        ],
        impact_minor: view.goal_minor.saturating_sub(view.balance_minor).max(0),
    })
}

/// Um achado so para todas as faturas: sem o nome do cartao (nome de forma de pagamento nao
/// vai ao envelope), varias faturas viram uma soma, e o vencimento vira janela em dias (data
/// exata tambem nao vai ao envelope).
pub(super) fn invoices(ds: &Dataset, today: &str) -> Option<Finding> {
    let list = upcoming_invoices(ds, today, INVOICE_DAYS);
    if list.is_empty() {
        return None;
    }
    let total = list
        .iter()
        .fold(0i64, |a, i| a.saturating_add(i.total_minor));
    let n = list.len();
    Some(Finding {
        kind: FindingKind::Invoices,
        severity: Severity::Attention,
        title: if n == 1 {
            format!("Fatura de cartão de {} vence em breve", format_brl(total))
        } else {
            format!(
                "{n} faturas de cartão somam {} nos próximos dias",
                format_brl(total)
            )
        },
        // Sem datas: o texto vai ao envelope e a regra do produto proibe data exata.
        detail: format!("Vencem nos próximos {INVOICE_DAYS} dias."),
        action: Some("Separar o valor antes do vencimento evita juros".into()),
        figures: vec![
            Figure::money("Faturas a vencer", total),
            Figure {
                label: "Quantidade".into(),
                value: FigureValue::Count(n),
            },
        ],
        impact_minor: total,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::advisor::{FindingKind, Severity};
    use crate::dashboard::{GRAFITE_TODAY, fixtures_grafite};

    fn empty() -> Dataset {
        Dataset::default()
    }

    #[test]
    fn nenhuma_regra_dispara_sem_dado() {
        let ds = empty();
        let t = "2026-11-19";
        assert!(pace(&ds, t).is_none());
        assert!(forecast(&ds, t).is_none());
        assert!(categories(&ds, t).is_empty());
        assert!(budgets(&ds, t).is_empty());
        assert!(savings(&ds, t).is_none());
        assert!(fixed_weight(&ds, t).is_none());
        assert!(next_month(&ds, t).is_none());
        assert!(emergency(&ds, t).is_none());
        assert!(invoices(&ds, t).is_none());
    }

    #[test]
    fn ritmo_segue_o_change_percent_do_dashboard() {
        let ds = fixtures_grafite();
        let sp = insights::spending_pace(&ds, "2026-11", GRAFITE_TODAY);
        let f = pace(&ds, GRAFITE_TODAY);
        match sp.change_percent {
            Some(p) if p > 0 => {
                let f = f.expect("ritmo acima vira achado");
                assert_eq!(f.kind, FindingKind::Pace);
                assert_eq!(f.severity, Severity::Attention);
                assert!(f.title.contains(&format!("{p}%")), "{}", f.title);
            }
            _ => assert!(f.is_none()),
        }
    }

    #[test]
    fn previsao_segue_o_forecast() {
        let ds = fixtures_grafite();
        let fc = insights::forecast(&ds, GRAFITE_TODAY);
        let f = forecast(&ds, GRAFITE_TODAY);
        match fc.map(|f| insights::round_to_tens(f.expected_surplus_minor)) {
            Some(s) if s < 0 => assert_eq!(f.map(|f| f.severity), Some(Severity::Alert)),
            Some(s) if s > 0 => assert_eq!(f.map(|f| f.severity), Some(Severity::GoodSign)),
            _ => assert!(f.is_none()),
        }
    }

    #[test]
    fn categorias_no_maximo_tres_e_maior_diferenca_primeiro() {
        let ds = fixtures_grafite();
        let marked = insights::categories_above_normal(&ds, "2026-11", GRAFITE_TODAY);
        let got = categories(&ds, GRAFITE_TODAY);
        assert_eq!(got.len(), marked.len().min(3));
        for w in got.windows(2) {
            assert!(w[0].impact_minor >= w[1].impact_minor);
        }
    }

    #[test]
    fn metas_seguem_o_budget_level() {
        let ds = fixtures_grafite();
        let report = crate::dashboard::budgets::budget_report(&ds, "2026-11");
        let over = report
            .iter()
            .filter(|b| b.level == BudgetLevel::Over)
            .count();
        let warn = report
            .iter()
            .filter(|b| b.level == BudgetLevel::Warning)
            .count();
        let got = budgets(&ds, GRAFITE_TODAY);
        assert_eq!(
            got.iter().filter(|f| f.severity == Severity::Alert).count(),
            over
        );
        assert_eq!(
            got.iter()
                .filter(|f| f.severity == Severity::Attention)
                .count(),
            warn
        );
    }

    #[test]
    fn media_de_renda_so_de_meses_fechados() {
        let ds = fixtures_grafite();
        let avg = closed_averages(&ds, GRAFITE_TODAY);
        if let Some(a) = &avg {
            assert!(a.months >= 1);
            assert!(a.income_minor >= 0);
        }
    }

    #[test]
    fn faturas_somam_sem_nome_de_cartao() {
        let ds = fixtures_grafite();
        let list = crate::dashboard::cards::upcoming_invoices(&ds, GRAFITE_TODAY, 10);
        let f = invoices(&ds, GRAFITE_TODAY);
        if list.is_empty() {
            assert!(f.is_none());
        } else {
            let f = f.expect("faturas viram achado");
            let total: i64 = list.iter().map(|i| i.total_minor).sum();
            assert_eq!(f.impact_minor, total);
            for i in &list {
                assert!(!f.title.contains(&i.card_name) && !f.detail.contains(&i.card_name));
            }
        }
    }

    #[test]
    fn fatura_a_vencer_vira_um_achado_sem_nome_nem_data() {
        let ds = fixtures_grafite();
        // 2026-11-28: a fatura da fixture vence dentro da janela de INVOICE_DAYS.
        let today = "2026-11-28";
        let list = crate::dashboard::cards::upcoming_invoices(&ds, today, INVOICE_DAYS);
        assert!(
            !list.is_empty(),
            "a fixture deveria ter fatura a vencer em {today}"
        );
        let f = invoices(&ds, today).expect("faturas viram achado");
        assert_eq!(f.kind, FindingKind::Invoices);
        assert_eq!(f.severity, Severity::Attention);
        let text = format!("{} {} {:?}", f.title, f.detail, f.action);
        for i in &list {
            assert!(!text.contains(&i.card_name), "{text}");
            assert!(!text.contains(&i.due_on), "{text}");
        }
    }

    #[test]
    fn fixo_e_comprometimento_seguem_a_renda_media_dos_fechados() {
        let ds = fixtures_grafite();
        let avg = closed_averages(&ds, GRAFITE_TODAY).expect("a fixture tem mes fechado");
        let fixed = insights::fixed_variable(&ds, "2026-11").fixed_minor;
        let committed = next_month_free(&ds, GRAFITE_TODAY).expense_minor;
        let want_fixed = percent_of(fixed, avg.income_minor).and_then(classify_fixed_weight);
        let want_committed = percent_of(committed, avg.income_minor).and_then(classify_committed);
        assert_eq!(
            fixed_weight(&ds, GRAFITE_TODAY).map(|f| f.severity),
            want_fixed
        );
        assert_eq!(
            next_month(&ds, GRAFITE_TODAY).map(|f| f.severity),
            want_committed
        );
    }

    #[test]
    fn nenhum_achado_da_fixture_cita_cartao_ou_pessoa() {
        let ds = fixtures_grafite();
        let mut sentinels: Vec<String> = ds
            .payment_methods
            .values()
            .map(|m| m.name.clone())
            .collect();
        sentinels.extend(ds.users.values().map(|u| u.name.clone()));
        assert!(!sentinels.is_empty());
        for today in [GRAFITE_TODAY, "2026-11-28", "2026-12-01"] {
            let a = crate::advisor::analyze(&ds, today);
            for f in &a.findings {
                let text = format!("{} {} {:?}", f.title, f.detail, f.action);
                for s in &sentinels {
                    assert!(!text.contains(s.as_str()), "{s} em {text}");
                }
            }
        }
    }
}
