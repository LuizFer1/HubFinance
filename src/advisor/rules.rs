//! Uma funcao por achado. Cada uma le so `dashboard/` e devolve zero ou mais achados; os
//! numeros saem das mesmas funcoes que desenham o dashboard.

use super::thresholds::{
    COMMITTED_ATTENTION_ABOVE, FIXED_ATTENTION_ABOVE, INVOICE_DAYS, SAVINGS_GOOD_MIN,
    TOP_CATEGORIES, classify_committed, classify_emergency_tenths, classify_fixed_weight,
    classify_savings, percent_of,
};
use super::{Figure, FigureValue, Finding, FindingKind, Severity, months_label};
use crate::dashboard::aggregate::{MonthTotals, NO_CATEGORY_KEY, NO_CATEGORY_NAME, monthly_totals};
use crate::dashboard::budgets::{BudgetLevel, budget_report};
use crate::dashboard::cards::upcoming_invoices;
use crate::dashboard::dataset::Dataset;
use crate::dashboard::insights::{self, SURPLUS_MONTHS};
use crate::dashboard::money::{Money, format_brl, signed_brl};
use crate::dashboard::periods::{last_months, month_of, shift_month};
use crate::dashboard::recurrence::next_month_free;
use crate::dashboard::reserves::{div_round, emergency_view, ordered};

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
            // Sem folga (`<= 0`) "gaste até R$ 0,00 por dia" soaria como conselho viável; o que
            // vale dizer é que as contas que ainda vêm já passam do saldo.
            insights::zero_allowance(&f).map(|per_day| {
                if per_day > 0 {
                    format!(
                        "Para fechar no zero, gaste até {} por dia até o fim do mês",
                        format_brl(per_day)
                    )
                } else {
                    "Não há folga para fechar no zero este mês".to_string()
                }
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
    let oldest = shift_month(&current, -SURPLUS_MONTHS);
    let window = last_months(&current, usize::try_from(SURPLUS_MONTHS).unwrap_or(12) + 1);
    let totals: Vec<MonthTotals> = monthly_totals(ds, &window);
    // Um unico recorte (mesmo criterio de `average_surplus_closed`) alimenta contagem e renda:
    // com dois filtros a renda poderia sair de uma janela e a sobra de outra.
    let closed: Vec<&MonthTotals> = totals
        .iter()
        .filter(|t| t.month >= first && t.month >= oldest && t.month < current)
        .collect();
    let n = i64::try_from(closed.len()).ok().filter(|n| *n > 0)?;
    let income: i64 = closed
        .iter()
        .fold(0i64, |a, t| a.saturating_add(t.income_minor));
    let surplus = insights::average_surplus_closed(&totals, Some(&first), &current)?;
    Some(ClosedAverages {
        months: closed.len(),
        income_minor: div_round(income, n),
        surplus_minor: surplus.average_minor,
    })
}

pub(super) fn savings(ds: &Dataset, today: &str) -> Option<Finding> {
    let avg = closed_averages(ds, today)?;
    // A mesma formula da taxa de poupanca do dashboard: renda e despesa media dos fechados.
    let rate = insights::savings_rate(
        avg.income_minor,
        avg.income_minor.saturating_sub(avg.surplus_minor),
    )?;
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
            // Com sinal: sobra negativa e justamente o caso do alerta "saiu mais do que
            // entrou"; em valor absoluto o texto diria que sobrou dinheiro.
            signed_brl(avg.surplus_minor)
        ),
        action: (severity != Severity::GoodSign)
            .then(|| "Mirar em guardar 20% da renda por mês".to_string()),
        figures: vec![
            Figure::money("Renda média", avg.income_minor),
            Figure::money("Sobra média", avg.surplus_minor),
            Figure::percent("Taxa de poupança", rate),
        ],
        impact_minor: (avg.income_minor.saturating_mul(SAVINGS_GOOD_MIN) / 100)
            .saturating_sub(avg.surplus_minor)
            .saturating_abs(),
    })
}

pub(super) fn fixed_weight(ds: &Dataset, today: &str) -> Option<Finding> {
    let avg = closed_averages(ds, today)?;
    // Ultimo mes FECHADO, nao o corrente: no meio do mes as recorrencias ainda nao foram gravadas
    // e o fixo apareceria menor do que sera, escondendo o achado justo quando ele ajudaria.
    let last_closed = shift_month(&month_of(today), -1);
    let fv = insights::fixed_variable(ds, &last_closed);
    let p = percent_of(fv.fixed_minor, avg.income_minor)?;
    let severity = classify_fixed_weight(p)?;
    Some(Finding {
        kind: FindingKind::FixedWeight,
        severity,
        title: format!("Gastos fixos levaram {p}% da renda no mês passado"),
        detail: format!(
            "{} fixos no mês passado contra renda média de {}.",
            format_brl(fv.fixed_minor),
            format_brl(avg.income_minor)
        ),
        action: Some("Revisar contratos e assinaturas abre margem todo mês".into()),
        figures: vec![
            Figure::money("Fixo no mês passado", fv.fixed_minor),
            Figure::percent("Fixo sobre a renda", p),
        ],
        impact_minor: fv
            .fixed_minor
            .saturating_sub(avg.income_minor.saturating_mul(FIXED_ATTENTION_ABOVE) / 100),
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
        impact_minor: rm
            .expense_minor
            .saturating_sub(avg.income_minor.saturating_mul(COMMITTED_ATTENTION_ABOVE) / 100),
    })
}

pub(super) fn emergency(ds: &Dataset, today: &str) -> Option<Finding> {
    let current = month_of(today);
    let (Some(r), _) = ordered(ds) else {
        return None;
    };
    // Sempre visivel: o achado vira texto do envelope do assistente, que leva os valores reais;
    // "Ocultar valores" e so da tela, que mascara o achado na hora de desenhar.
    let view = emergency_view(ds, r, &current, Money::default());
    let cost = view.cost.as_ref().filter(|c| c.total_minor > 0)?;
    // Decimos de mes por inteiros: o `coverage` f64 da view e so exibicao.
    let tenths = view.balance_minor.saturating_mul(10) / cost.total_minor;
    let severity = classify_emergency_tenths(tenths)?;
    // O mesmo `months_label` da cifra do envelope: com `coverage_label` (que trava em 0,0) o
    // card diria "0,0 mês" e o envelope "−1,2 mês" para o mesmo saldo negativo.
    let months = months_label(tenths);
    let title = match severity {
        Severity::GoodSign => format!("Reserva de emergência cobre {months}"),
        _ => format!("Reserva de emergência cobre só {months}"),
    };
    Some(Finding {
        kind: FindingKind::Emergency,
        severity,
        title,
        detail: format!(
            "Saldo {} para um custo essencial de {} por mês.",
            signed_brl(view.balance_minor),
            format_brl(cost.total_minor)
        ),
        // `remaining_label` vira "Meta alcancada" quando o saldo ja passa da meta; isso nao e acao
        // e contradiria o "so" do titulo.
        // Com multiplo acima de 6 o GoodSign (>= 6 meses) ainda pode estar abaixo da meta: ai o
        // "Faltam R$ X" e util e fica.
        action: (view.balance_minor < view.goal_minor).then(|| view.remaining_label.clone()),
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
    use crate::advisor::{FindingKind, Severity, analyze};
    use crate::dashboard::dataset::{
        BUDGETS, CATEGORIES, RECURRENCES, RESERVE_MOVEMENTS, RESERVES, TRANSACTIONS,
    };
    use crate::dashboard::money::MINUS;
    use crate::dashboard::{GRAFITE_TODAY, dataset_of, fixtures_grafite};
    use serde_json::{Value, json};

    const TODAY: &str = "2026-11-19";

    /// Monta um `Dataset` pelo `apply` de verdade, linha a linha, para cada teste forcar UM
    /// caminho com numeros que se conferem de cabeca.
    #[derive(Default)]
    struct Build {
        rows: Vec<(&'static str, String, Value)>,
    }

    impl Build {
        fn push(mut self, table: &'static str, id: &str, data: Value) -> Self {
            self.rows.push((table, id.to_string(), data));
            self
        }

        fn category(self, id: &str, name: &str) -> Self {
            self.push(
                CATEGORIES,
                id,
                json!({ "name": name, "icon": "tag", "color": "sky", "kind": "expense", "mergedInto": null }),
            )
        }

        fn tx(self, id: &str, kind: &str, amount: i64, on: &str, category: Option<&str>) -> Self {
            self.push(
                TRANSACTIONS,
                id,
                json!({
                    "kind": kind, "description": id, "amountMinor": amount,
                    "occurredOn": on, "categoryId": category
                }),
            )
        }

        /// Ocorrencia gravada de uma serie (`recurrenceId` + `occurrenceKey`).
        fn occurrence(self, id: &str, rid: &str, amount: i64, on: &str) -> Self {
            self.push(
                TRANSACTIONS,
                id,
                json!({
                    "kind": "expense", "description": id, "amountMinor": amount,
                    "occurredOn": on, "categoryId": null,
                    "recurrenceId": rid, "occurrenceKey": format!("{rid}:{}", &on[..7])
                }),
            )
        }

        fn series(self, id: &str, amount: i64, start_on: &str) -> Self {
            let day: i64 = start_on[8..10].parse().unwrap_or(1);
            self.push(
                RECURRENCES,
                id,
                json!({
                    "frequency": "monthly", "kind": "expense", "description": id,
                    "amountMinor": amount, "currency": "BRL", "categoryId": null,
                    "paymentMethodId": null, "cashbackMinor": null,
                    "scheduleType": "dayOfMonth", "scheduleN": day,
                    "startOn": start_on, "endOn": null, "active": true, "userId": null
                }),
            )
        }

        fn budget(self, id: &str, category: &str, limit: i64) -> Self {
            self.push(
                BUDGETS,
                id,
                json!({ "categoryId": category, "limitMinor": limit }),
            )
        }

        /// Reserva de emergencia com custo essencial informado (meta = 6 x custo) e um
        /// deposito em outubro.
        fn emergency(self, cost: i64, deposit: i64) -> Self {
            self.emergency_multiple(cost, deposit, 6)
        }

        fn emergency_multiple(self, cost: i64, deposit: i64, multiple: u8) -> Self {
            self.push(
                RESERVES,
                "RE",
                json!({
                    "kind": "emergency", "name": "Reserva de emergência", "icon": "lifebuoy",
                    "color": "violet", "targetMinor": null, "multiple": multiple,
                    "essentialCategoryIds": null, "essentialOverrideMinor": cost,
                    "deadline": null, "recurring": null
                }),
            )
            .push(
                RESERVE_MOVEMENTS,
                "MOV",
                json!({
                    "reserveId": "RE", "amountMinor": deposit, "occurredOn": "2026-10-06",
                    "userId": null, "description": null, "reason": null, "recurring": false
                }),
            )
        }

        /// Renda de R$ 1.000,00 em setembro e outubro: dois meses fechados para as medias.
        fn income_history(self) -> Self {
            self.tx("I9", "income", 100_000, "2026-09-05", None).tx(
                "I10",
                "income",
                100_000,
                "2026-10-05",
                None,
            )
        }

        fn build(self) -> Dataset {
            let ds = dataset_of(
                self.rows
                    .iter()
                    .map(|(t, id, data)| (*t, id.as_str(), false, data.clone()))
                    .collect(),
            );
            assert_eq!(
                ds.ignored_total(),
                0,
                "linha fora do contrato: {:?}",
                ds.ignored
            );
            ds
        }
    }

    fn surplus_months(ds: &Dataset, today: &str) -> Option<usize> {
        let current = month_of(today);
        let first = insights::first_month(ds);
        let window = last_months(&current, 13);
        let totals = monthly_totals(ds, &window);
        insights::average_surplus_closed(&totals, first.as_deref(), &current).map(|s| s.months)
    }

    fn text(f: &Finding) -> String {
        format!("{} | {} | {:?}", f.title, f.detail, f.action)
    }

    #[test]
    fn nenhuma_regra_dispara_sem_dado() {
        let ds = Dataset::default();
        let t = TODAY;
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
    fn fixture_grafite_gera_exatamente_estes_achados() {
        use FindingKind::*;
        use Severity::*;
        let got: Vec<_> = analyze(&fixtures_grafite(), GRAFITE_TODAY)
            .findings
            .iter()
            .map(|f| (f.kind, f.severity))
            .collect();
        assert_eq!(
            got,
            vec![
                (Forecast, Alert),
                (Budget, Alert),
                (Emergency, Attention),
                (Budget, Attention),
                (Pace, Attention),
                (Budget, Attention),
                (Budget, Attention),
                (CategoryAboveNormal, Attention),
                (SavingsRate, GoodSign),
            ]
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
            let a = analyze(&ds, today);
            assert!(!a.findings.is_empty());
            for f in &a.findings {
                let t = text(f);
                for s in &sentinels {
                    assert!(!t.contains(s.as_str()), "{s} em {t}");
                }
            }
        }
    }

    // ---- ritmo -----------------------------------------------------------------------

    #[test]
    fn ritmo_da_fixture_segue_o_dashboard() {
        let ds = fixtures_grafite();
        let sp = insights::spending_pace(&ds, "2026-11", GRAFITE_TODAY);
        let p = sp.change_percent.expect("a fixture tem normal");
        assert!(p > 0);
        let f = pace(&ds, GRAFITE_TODAY).expect("ritmo acima vira achado");
        assert_eq!(
            (f.kind, f.severity),
            (FindingKind::Pace, Severity::Attention)
        );
        assert!(f.title.contains(&format!("{p}%")), "{}", f.title);
        assert_eq!(
            f.impact_minor,
            sp.spent_minor - sp.normal_at_cutoff_minor.unwrap_or(0)
        );
    }

    // ---- previsao --------------------------------------------------------------------

    #[test]
    fn previsao_da_fixture_e_alerta_com_teto_por_dia() {
        let ds = fixtures_grafite();
        let fc = insights::forecast(&ds, GRAFITE_TODAY).expect("depois do dia 5");
        let surplus = insights::round_to_tens(fc.expected_surplus_minor);
        assert!(surplus < 0);
        let f = forecast(&ds, GRAFITE_TODAY).expect("sobra negativa vira achado");
        assert_eq!(f.severity, Severity::Alert);
        assert_eq!(f.impact_minor, -surplus);
        assert!(
            f.action.as_deref().is_some_and(|a| a.contains("por dia")),
            "{f:?}"
        );
    }

    #[test]
    fn previsao_negativa_sem_folga_nao_manda_gastar_zero_por_dia() {
        // Receita 1.000 e despesa 1.500 ja lancadas: as contas passam do saldo.
        let ds = Build::default()
            .tx("R", "income", 100_000, "2026-11-05", None)
            .tx("D", "expense", 150_000, "2026-11-06", None)
            .build();
        let f = forecast(&ds, TODAY).expect("sobra negativa vira achado");
        assert_eq!(
            (f.kind, f.severity),
            (FindingKind::Forecast, Severity::Alert)
        );
        // 150.000 + 150.000 x 11 / 19 (86.842) = 236.842; sobra -136.842 ~ -137.000 (dezena de reais).
        assert_eq!(f.impact_minor, 137_000);
        assert_eq!(
            f.action.as_deref(),
            Some("Não há folga para fechar no zero este mês")
        );
    }

    #[test]
    fn previsao_positiva_e_bom_sinal_sem_acao() {
        let ds = Build::default()
            .tx("R", "income", 300_000, "2026-11-05", None)
            .tx("D", "expense", 50_000, "2026-11-06", None)
            .build();
        let f = forecast(&ds, TODAY).expect("sobra positiva vira achado");
        assert_eq!(f.severity, Severity::GoodSign);
        assert_eq!(f.action, None);
    }

    // ---- categorias e ritmo com historico --------------------------------------------

    /// Agosto a outubro: R$ 100 em cada uma das quatro categorias por mes. Em novembro (ate o
    /// dia 19) Alfa gasta 200, Beta 300, Gama 250 e Delta 180.
    fn four_above_normal() -> Dataset {
        let mut b = Build::default()
            .category("CA", "Alfa")
            .category("CB", "Beta")
            .category("CC", "Gama")
            .category("CD", "Delta");
        for month in ["2026-08", "2026-09", "2026-10"] {
            for c in ["CA", "CB", "CC", "CD"] {
                let on = format!("{month}-05");
                b = b.tx(&format!("{c}-{month}"), "expense", 10_000, &on, Some(c));
            }
        }
        for (c, amount) in [
            ("CA", 20_000),
            ("CB", 30_000),
            ("CC", 25_000),
            ("CD", 18_000),
        ] {
            b = b.tx(&format!("{c}-11"), "expense", amount, "2026-11-10", Some(c));
        }
        b.build()
    }

    #[test]
    fn quatro_categorias_acima_do_normal_viram_tres_com_a_maior_diferenca_primeiro() {
        let ds = four_above_normal();
        assert_eq!(
            insights::categories_above_normal(&ds, "2026-11", TODAY).len(),
            4
        );
        let got = categories(&ds, TODAY);
        assert_eq!(got.len(), 3);
        assert!(got.iter().all(|f| f.severity == Severity::Attention));
        assert_eq!(
            got.iter().map(|f| f.title.as_str()).collect::<Vec<_>>(),
            vec![
                "Beta 200% acima do normal",
                "Gama 150% acima do normal",
                "Alfa 100% acima do normal"
            ]
        );
        assert_eq!(
            got.iter().map(|f| f.impact_minor).collect::<Vec<_>>(),
            vec![20_000, 15_000, 10_000]
        );
    }

    #[test]
    fn ritmo_forcado_pesa_o_gasto_acima_do_normal() {
        let ds = four_above_normal();
        // Gasto ate o dia 19: 930; normal no mesmo dia: 400.
        let f = pace(&ds, TODAY).expect("ritmo acima vira achado");
        assert_eq!(f.severity, Severity::Attention);
        assert_eq!(f.impact_minor, 93_000 - 40_000);
    }

    #[test]
    fn categoria_da_fixture_segue_o_dashboard() {
        let ds = fixtures_grafite();
        let marked = insights::categories_above_normal(&ds, "2026-11", GRAFITE_TODAY);
        assert!(!marked.is_empty());
        let got = categories(&ds, GRAFITE_TODAY);
        assert_eq!(got.len(), marked.len().min(3));
        for w in got.windows(2) {
            assert!(w[0].impact_minor >= w[1].impact_minor);
        }
    }

    // ---- metas -----------------------------------------------------------------------

    #[test]
    fn meta_estourada_e_alerta_e_perto_do_limite_e_atencao() {
        let ds = Build::default()
            .category("C1", "Mercado")
            .category("C2", "Lazer")
            .category("C3", "Casa")
            .budget("B1", "C1", 10_000)
            .budget("B2", "C2", 10_000)
            .budget("B3", "C3", 10_000)
            .tx("T1", "expense", 15_000, "2026-11-03", Some("C1"))
            .tx("T2", "expense", 9_000, "2026-11-04", Some("C2"))
            .tx("T3", "expense", 2_000, "2026-11-05", Some("C3"))
            .build();
        let got = budgets(&ds, TODAY);
        assert_eq!(got.len(), 2, "a meta folgada nao vira achado");
        let over = got
            .iter()
            .find(|f| f.severity == Severity::Alert)
            .expect("estourada");
        assert_eq!(over.title, "Meta de Mercado estourada");
        assert_eq!(over.impact_minor, 5_000);
        let warn = got
            .iter()
            .find(|f| f.severity == Severity::Attention)
            .expect("perto");
        assert_eq!(warn.title, "Meta de Lazer perto do limite");
        assert_eq!(warn.impact_minor, 9_000);
    }

    #[test]
    fn metas_da_fixture_seguem_o_budget_level() {
        let ds = fixtures_grafite();
        let report = budget_report(&ds, "2026-11");
        let over = report
            .iter()
            .filter(|b| b.level == BudgetLevel::Over)
            .count();
        let warn = report
            .iter()
            .filter(|b| b.level == BudgetLevel::Warning)
            .count();
        assert!(over >= 1 && warn >= 1, "a fixture deveria ter as duas");
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

    // ---- medias dos meses fechados ---------------------------------------------------

    #[test]
    fn medias_fechadas_usam_os_mesmos_meses_da_sobra_do_dashboard() {
        let grafite = fixtures_grafite();
        let avg = closed_averages(&grafite, GRAFITE_TODAY).expect("a fixture tem mes fechado");
        assert_eq!(Some(avg.months), surplus_months(&grafite, GRAFITE_TODAY));
        // So outubro fechou; novembro (parcial) nao entra na renda.
        assert_eq!((avg.months, avg.income_minor), (1, 507_229));

        let two = Build::default()
            .income_history()
            .tx("D", "expense", 30_000, "2026-10-10", None)
            .build();
        let avg = closed_averages(&two, TODAY).expect("setembro e outubro");
        assert_eq!(Some(avg.months), surplus_months(&two, TODAY));
        assert_eq!(
            (avg.months, avg.income_minor, avg.surplus_minor),
            (2, 100_000, 85_000)
        );
    }

    // ---- poupanca --------------------------------------------------------------------

    #[test]
    fn poupanca_negativa_e_alerta() {
        let ds = Build::default()
            .income_history()
            .tx("D9", "expense", 150_000, "2026-09-10", None)
            .tx("D10", "expense", 150_000, "2026-10-10", None)
            .build();
        let f = savings(&ds, TODAY).expect("sobra media negativa vira achado");
        assert_eq!(
            (f.kind, f.severity),
            (FindingKind::SavingsRate, Severity::Alert)
        );
        assert!(f.title.contains("-50%"), "{}", f.title);
        // Distancia a 20% da renda (200) mais o rombo (500).
        assert_eq!(f.impact_minor, 70_000);
        assert!(f.action.is_some());
    }

    #[test]
    fn poupanca_da_fixture_e_bom_sinal_com_a_formula_do_dashboard() {
        let ds = fixtures_grafite();
        let avg = closed_averages(&ds, GRAFITE_TODAY).expect("mes fechado");
        let rate = insights::savings_rate(avg.income_minor, avg.income_minor - avg.surplus_minor);
        assert_eq!(rate, Some(28));
        let f = savings(&ds, GRAFITE_TODAY).expect("28% e bom sinal");
        assert_eq!(f.severity, Severity::GoodSign);
        assert_eq!(f.action, None);
        assert!(f.title.contains("28%"), "{}", f.title);
    }

    // ---- fixo e comprometimento ------------------------------------------------------

    /// Renda 1.000 por mes e uma serie fixa de 700 gravada em setembro e outubro; novembro
    /// ainda nao tem a ocorrencia (o caso do meio do mes).
    fn fixed_seventy() -> Dataset {
        Build::default()
            .income_history()
            .series("R1", 70_000, "2026-09-05")
            .occurrence("O9", "R1", 70_000, "2026-09-05")
            .occurrence("O10", "R1", 70_000, "2026-10-05")
            .build()
    }

    #[test]
    fn fixo_acima_de_60_pct_da_renda_e_atencao_pelo_ultimo_mes_fechado() {
        let ds = fixed_seventy();
        // Novembro ainda nao gravou a serie: o fixo do mes corrente seria 0.
        assert_eq!(insights::fixed_variable(&ds, "2026-11").fixed_minor, 0);
        let f = fixed_weight(&ds, TODAY).expect("70% da renda e acima de 60%");
        assert_eq!(
            (f.kind, f.severity),
            (FindingKind::FixedWeight, Severity::Attention)
        );
        assert!(f.title.contains("70%"), "{}", f.title);
        assert!(f.title.contains("mês passado"), "{}", f.title);
        assert_eq!(f.impact_minor, 70_000 - 60_000);
        // 700 de 1.000 no mes que vem e exatamente 70%: nao passa do limite de comprometimento.
        assert!(next_month(&ds, TODAY).is_none());
    }

    #[test]
    fn comprometimento_acima_de_70_pct_e_atencao() {
        let ds = Build::default()
            .income_history()
            .series("R2", 80_000, "2026-09-05")
            .build();
        let f = next_month(&ds, TODAY).expect("80% da renda e acima de 70%");
        assert_eq!(
            (f.kind, f.severity),
            (FindingKind::NextMonthCommitted, Severity::Attention)
        );
        assert!(f.title.contains("80%"), "{}", f.title);
        assert_eq!(f.impact_minor, 80_000 - 70_000);
        // Nada gravado em outubro: o fixo do mes passado e zero.
        assert!(fixed_weight(&ds, TODAY).is_none());
    }

    #[test]
    fn fixo_e_comprometimento_da_fixture_ficam_abaixo_dos_limites() {
        let ds = fixtures_grafite();
        let avg = closed_averages(&ds, GRAFITE_TODAY).expect("mes fechado");
        let fixed = insights::fixed_variable(&ds, "2026-10").fixed_minor;
        let committed = next_month_free(&ds, GRAFITE_TODAY).expense_minor;
        assert!(percent_of(fixed, avg.income_minor).is_some_and(|p| p <= 60));
        assert!(percent_of(committed, avg.income_minor).is_some_and(|p| p <= 70));
        assert!(fixed_weight(&ds, GRAFITE_TODAY).is_none());
        assert!(next_month(&ds, GRAFITE_TODAY).is_none());
    }

    // ---- emergencia ------------------------------------------------------------------

    #[test]
    fn emergencia_abaixo_de_tres_meses_e_atencao_com_acao() {
        // Custo 1.000; saldo 500 = 0,5 mes; meta 6.000.
        let ds = Build::default().emergency(100_000, 50_000).build();
        let f = emergency(&ds, TODAY).expect("0,5 mes e atencao");
        assert_eq!(
            (f.kind, f.severity),
            (FindingKind::Emergency, Severity::Attention)
        );
        assert_eq!(f.title, "Reserva de emergência cobre só 0,5 mês");
        assert!(
            f.action.as_deref().is_some_and(|a| a.starts_with("Faltam")),
            "{f:?}"
        );
        assert_eq!(f.impact_minor, 600_000 - 50_000);
    }

    #[test]
    fn emergencia_pluraliza_a_partir_de_dois_meses() {
        let um_e_meio = emergency(&Build::default().emergency(100_000, 150_000).build(), TODAY)
            .expect("1,5 mes");
        assert_eq!(um_e_meio.title, "Reserva de emergência cobre só 1,5 mês");
        let dois = emergency(&Build::default().emergency(100_000, 200_000).build(), TODAY)
            .expect("2,0 meses");
        assert_eq!(dois.title, "Reserva de emergência cobre só 2,0 meses");
    }

    #[test]
    fn emergencia_com_saldo_negativo_mostra_o_sinal_igual_ao_da_cifra() {
        let ds = Build::default().emergency(100_000, -150_000).build();
        let f = emergency(&ds, TODAY).expect("saldo negativo e atencao");
        assert_eq!(f.severity, Severity::Attention);
        assert_eq!(
            f.title,
            format!("Reserva de emergência cobre só {MINUS}1,5 mês")
        );
        // A cifra que vai ao envelope usa a mesma funcao, entao os dois textos concordam.
        let tenths = f
            .figures
            .iter()
            .find_map(|g| match g.value {
                FigureValue::MonthsTenths(t) => Some(t),
                _ => None,
            })
            .expect("cifra de meses");
        assert!(f.title.ends_with(&months_label(tenths)), "{}", f.title);
    }

    #[test]
    fn emergencia_de_tres_a_seis_meses_nao_vira_achado() {
        let ds = Build::default().emergency(100_000, 350_000).build();
        assert!(emergency(&ds, TODAY).is_none());
    }

    #[test]
    fn emergencia_bom_sinal_abaixo_de_meta_maior_mantem_a_acao() {
        // Multiplo 12: meta 12.000; saldo 7.000 = 7,0 meses (bom sinal), mas faltam 5.000.
        let ds = Build::default()
            .emergency_multiple(100_000, 700_000, 12)
            .build();
        let f = emergency(&ds, TODAY).expect("7 meses e bom sinal");
        assert_eq!(f.severity, Severity::GoodSign);
        assert!(
            f.action.as_deref().is_some_and(|a| a.starts_with("Faltam")),
            "{f:?}"
        );
        assert_eq!(f.impact_minor, 1_200_000 - 700_000);
    }

    #[test]
    fn emergencia_de_seis_meses_ou_mais_e_bom_sinal_sem_acao() {
        let ds = Build::default().emergency(100_000, 700_000).build();
        let f = emergency(&ds, TODAY).expect("7 meses e bom sinal");
        assert_eq!(f.severity, Severity::GoodSign);
        assert_eq!(f.title, "Reserva de emergência cobre 7,0 meses");
        assert_eq!(f.action, None, "meta alcancada nao e acao");
        assert_eq!(f.impact_minor, 0);
    }

    // ---- faturas ---------------------------------------------------------------------

    #[test]
    fn sem_fatura_a_vencer_nao_ha_achado() {
        let ds = fixtures_grafite();
        assert!(upcoming_invoices(&ds, GRAFITE_TODAY, INVOICE_DAYS).is_empty());
        assert!(invoices(&ds, GRAFITE_TODAY).is_none());
    }

    #[test]
    fn fatura_a_vencer_vira_um_achado_somado_sem_nome_nem_data() {
        let ds = fixtures_grafite();
        // 2026-11-28: a fatura da fixture vence dentro da janela de INVOICE_DAYS.
        let today = "2026-11-28";
        let list = upcoming_invoices(&ds, today, INVOICE_DAYS);
        assert!(
            !list.is_empty(),
            "a fixture deveria ter fatura a vencer em {today}"
        );
        let f = invoices(&ds, today).expect("faturas viram achado");
        assert_eq!(
            (f.kind, f.severity),
            (FindingKind::Invoices, Severity::Attention)
        );
        assert_eq!(
            f.impact_minor,
            list.iter().map(|i| i.total_minor).sum::<i64>()
        );
        let t = text(&f);
        for i in &list {
            assert!(!t.contains(&i.card_name), "{t}");
            assert!(!t.contains(&i.due_on), "{t}");
        }
    }
}
