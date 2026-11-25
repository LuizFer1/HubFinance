//! O que o LLM ve (spec 5). `render` e a UNICA fonte do texto: a previa mostra a mesma string
//! que sai, entao o que o usuario aprova e exatamente o que e enviado. Entra so agregado por
//! mes, por nome de categoria e por reserva; nunca descricao, pessoa, aparelho, forma de
//! pagamento, data exata ou id (o teste de sentinela abaixo confere, e se pegar algo a
//! correcao e no envelope ou no achado, nunca no teste).

use std::fmt::Write as _;

use crate::advisor::{Analysis, FigureValue};
use crate::dashboard::aggregate::{
    NO_CATEGORY_KEY, NO_CATEGORY_NAME, expense_by_category, monthly_totals,
};
use crate::dashboard::budgets::budget_report;
use crate::dashboard::contract::Book;
use crate::dashboard::dataset::Dataset;
use crate::dashboard::money::format_brl;
use crate::dashboard::periods::{last_months, month_of, shift_month};
use crate::dashboard::reserves::{balance_until, emergency_view, ordered};

/// Meses fechados que vao ao envelope (decisao Q7).
pub const ENVELOPE_MONTHS: usize = 6;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Envelope {
    pub book: Book,
    /// `YYYY-MM` de hoje (em andamento; os meses abaixo sao os fechados antes dele).
    pub month: String,
    pub findings: Vec<String>,
    /// (mes, receita, despesa) em centavos.
    pub months: Vec<(String, i64, i64)>,
    /// Por mes, despesa por NOME de categoria (nunca id).
    pub categories: Vec<(String, Vec<(String, i64)>)>,
    /// (categoria, meta, gasto) do mes corrente.
    pub budgets: Vec<(String, i64, i64)>,
    /// (nome da reserva, saldo).
    pub reserves: Vec<(String, i64)>,
    pub emergency_goal_minor: Option<i64>,
}

pub fn build(analysis: &Analysis, ds: &Dataset, today: &str, book: Book) -> Envelope {
    let current = month_of(today);
    let last_closed = shift_month(&current, -1);
    let months = last_months(&last_closed, ENVELOPE_MONTHS);
    let findings = analysis
        .findings
        .iter()
        .map(|f| {
            let mut line = format!("[{}] {} — {}", f.severity.label(), f.title, f.detail);
            if let Some(a) = &f.action {
                let _ = write!(line, " Sugestão: {a}");
            }
            for fig in &f.figures {
                let v = match &fig.value {
                    FigureValue::Money(m) => format_brl(*m),
                    FigureValue::Percent(p) => format!("{p}%"),
                    FigureValue::MonthsTenths(t) => format!("{},{} meses", t / 10, t % 10),
                    FigureValue::Count(n) => n.to_string(),
                };
                let _ = write!(line, " | {}: {v}", fig.label);
            }
            line
        })
        .collect();
    let totals = monthly_totals(ds, &months)
        .into_iter()
        .map(|t| (t.month, t.income_minor, t.expense_minor))
        .collect();
    let categories = months
        .iter()
        .map(|m| {
            let slices = expense_by_category(ds, m)
                .into_iter()
                .map(|s| {
                    // A chave sintetica ja traz o nome "Sem categoria"; explicito aqui para
                    // que nenhum id jamais chegue ao texto.
                    let name = if s.key == NO_CATEGORY_KEY {
                        NO_CATEGORY_NAME.to_string()
                    } else {
                        s.name
                    };
                    (name, s.amount_minor)
                })
                .collect();
            (m.clone(), slices)
        })
        .collect();
    let budgets = budget_report(ds, &current)
        .into_iter()
        .map(|b| (b.name, b.limit_minor, b.spent_minor))
        .collect();
    let (emergency, pots) = ordered(ds);
    let reserves = emergency
        .into_iter()
        .chain(pots)
        .map(|r| (r.name.clone(), balance_until(ds, &r.id, &current)))
        .collect();
    let emergency_goal_minor = emergency
        .map(|r| emergency_view(ds, r, &current).goal_minor)
        .filter(|g| *g > 0);
    Envelope {
        book,
        month: current,
        findings,
        months: totals,
        categories,
        budgets,
        reserves,
        emergency_goal_minor,
    }
}

pub fn render(e: &Envelope) -> String {
    let mut s = String::new();
    let conta = match e.book {
        Book::Personal => "pessoal",
        Book::Business => "comercial",
    };
    let _ = writeln!(s, "Conta: {conta}");
    let _ = writeln!(s, "Mês de referência: {}", e.month);
    let _ = writeln!(s, "\n## Achados");
    if e.findings.is_empty() {
        let _ = writeln!(s, "- nenhum");
    }
    for f in &e.findings {
        let _ = writeln!(s, "- {f}");
    }
    let _ = writeln!(s, "\n## Receita e despesa por mês (meses fechados)");
    for (m, inc, exp) in &e.months {
        let _ = writeln!(
            s,
            "- {m}: receita {}, despesa {}",
            format_brl(*inc),
            format_brl(*exp)
        );
    }
    let _ = writeln!(s, "\n## Despesa por categoria");
    for (m, cats) in &e.categories {
        let list: Vec<String> = cats
            .iter()
            .map(|(n, v)| format!("{n} {}", format_brl(*v)))
            .collect();
        let _ = writeln!(
            s,
            "- {m}: {}",
            if list.is_empty() {
                "—".to_string()
            } else {
                list.join("; ")
            }
        );
    }
    let _ = writeln!(s, "\n## Metas do mês");
    if e.budgets.is_empty() {
        let _ = writeln!(s, "- nenhuma");
    }
    for (n, limit, spent) in &e.budgets {
        let _ = writeln!(
            s,
            "- {n}: meta {}, gasto {}",
            format_brl(*limit),
            format_brl(*spent)
        );
    }
    let _ = writeln!(s, "\n## Reservas");
    if e.reserves.is_empty() {
        let _ = writeln!(s, "- nenhuma");
    }
    for (n, bal) in &e.reserves {
        let _ = writeln!(s, "- {n}: saldo {}", format_brl(*bal));
    }
    if let Some(g) = e.emergency_goal_minor {
        let _ = writeln!(s, "- Meta da reserva de emergência: {}", format_brl(g));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::advisor::analyze;
    use crate::dashboard::{GRAFITE_TODAY, fixtures_grafite};

    fn rendered() -> (String, Dataset) {
        let ds = fixtures_grafite().for_book(Book::Personal);
        let a = analyze(&ds, GRAFITE_TODAY);
        (render(&build(&a, &ds, GRAFITE_TODAY, Book::Personal)), ds)
    }

    #[test]
    fn tem_os_seis_meses_e_os_achados() {
        let (s, _) = rendered();
        assert!(s.contains("Conta: pessoal"));
        assert!(s.contains("Mês de referência: 2026-11"));
        for m in [
            "2026-05", "2026-06", "2026-07", "2026-08", "2026-09", "2026-10",
        ] {
            assert!(s.contains(m), "falta {m}");
        }
        assert!(s.contains("## Achados"));
    }

    #[test]
    fn nunca_leva_descricao_pessoa_forma_de_pagamento_id_nem_data_exata() {
        let (s, ds) = rendered();

        // Nomes que PODEM aparecer (categorias, reservas e os rotulos de "sem ..."). Uma
        // ocorrencia que cai inteira dentro de um desses nomes nao e vazamento ("Mercado" dentro
        // da categoria "Mercado e feira"); qualquer outra e. Nao se apaga o nome do texto antes
        // de procurar: "Mercado Pago" (forma de pagamento) perderia o "Mercado" e vazaria sem
        // ser visto.
        let allowed: Vec<String> = ds
            .categories
            .values()
            .map(|c| c.name.clone())
            .chain(ds.reserves.values().map(|r| r.name.clone()))
            .chain([
                NO_CATEGORY_NAME.to_string(),
                "Categoria removida".to_string(),
            ])
            .filter(|n| !n.is_empty())
            .collect();
        let covered: Vec<(usize, usize)> = allowed
            .iter()
            .flat_map(|n| s.match_indices(n.as_str()).map(|(i, m)| (i, i + m.len())))
            .collect();
        // Um nome que e ele mesmo permitido (descricao igual a categoria) nao se distingue.
        let leaks = |needle: &str| {
            !allowed.iter().any(|n| n == needle)
                && s.match_indices(needle)
                    .any(|(i, m)| !covered.iter().any(|&(a, b)| a <= i && i + m.len() <= b))
        };
        // Rotulos neutros do dataset para forma de pagamento ausente: nao identificam ninguem.
        let neutral = [ds.payment_method_name(None), "Forma removida".to_string()];

        // Nao-vacuo: o exemplo tem de ter o que o teste procura, senao ele passa por vazio.
        let descriptions: Vec<&str> = ds
            .alive_transactions()
            .map(|t| t.description.trim())
            .filter(|d| d.len() >= 3)
            .collect();
        let methods: Vec<String> = ds
            .alive_transactions()
            .map(|t| ds.payment_method_name(t.payment_method_id.as_deref()))
            .filter(|p| p.len() >= 3 && !neutral.contains(p))
            .collect();
        let people: Vec<&str> = ds.alive_users().iter().map(|u| u.name.as_str()).collect();
        assert!(descriptions.len() >= 5, "poucas descricoes no exemplo");
        assert!(
            !methods.is_empty(),
            "o exemplo nao tem forma de pagamento nomeada"
        );
        assert!(!people.is_empty(), "o exemplo nao tem pessoa");
        assert!(
            ds.alive_reserves().count() > 0 && !ds.categories.is_empty(),
            "o exemplo precisa de reservas e categorias (permitidas) no envelope"
        );
        assert!(
            !covered.is_empty(),
            "nomes de categoria/reserva deveriam estar no envelope"
        );
        // O teste enxerga o caso "Mercado Pago": sensivel que contem nome permitido.
        assert!(!covered.is_empty() && leaks("Mercado Pago") == s.contains("Mercado Pago"));

        for d in descriptions {
            assert!(!leaks(d), "descricao vazou: {d}");
        }
        for pm in &methods {
            assert!(!leaks(pm), "forma de pagamento vazou: {pm}");
        }
        for u in &people {
            assert!(!leaks(u), "pessoa vazou: {u}");
        }
        for n in &neutral {
            assert!(!leaks(n), "rotulo interno vazou: {n}");
        }

        // Ids de qualquer tabela (>= 4 chars: "U1" e "C1" casariam com texto comum).
        let ids = ds
            .transactions
            .keys()
            .chain(ds.categories.keys())
            .chain(ds.users.keys())
            .chain(ds.payment_methods.keys())
            .chain(ds.reserves.keys())
            .chain(ds.reserve_movements.keys())
            .chain(ds.recurrences.keys())
            .chain(ds.budgets.keys())
            .filter(|id| id.len() >= 4);
        for id in ids {
            assert!(!s.contains(id.as_str()), "id vazou: {id}");
        }

        // Nenhuma data exata AAAA-MM-DD (AAAA-MM, de mes, pode).
        let is_date = |w: &[u8]| {
            w.len() == 10
                && w[..4].iter().all(u8::is_ascii_digit)
                && w[4] == b'-'
                && w[5..7].iter().all(u8::is_ascii_digit)
                && w[7] == b'-'
                && w[8..10].iter().all(u8::is_ascii_digit)
        };
        for (i, w) in s.as_bytes().windows(10).enumerate() {
            assert!(!is_date(w), "data exata vazou perto do byte {i}");
        }
        // O detector enxerga uma data de verdade (senao o laco acima nao prova nada).
        assert!(is_date(b"2026-11-19"));
        assert!(!is_date(b"2026-11 R$"));
    }

    #[test]
    fn categorias_do_envelope_sao_nomes_nunca_ids() {
        let ds = fixtures_grafite().for_book(Book::Personal);
        let a = analyze(&ds, GRAFITE_TODAY);
        let e = build(&a, &ds, GRAFITE_TODAY, Book::Personal);
        let names: Vec<&str> = e
            .categories
            .iter()
            .flat_map(|(_, c)| c.iter().map(|(n, _)| n.as_str()))
            .collect();
        assert!(!names.is_empty());
        let known: Vec<&str> = ds
            .categories
            .values()
            .map(|c| c.name.as_str())
            .chain([NO_CATEGORY_NAME])
            .collect();
        for n in names {
            assert!(known.contains(&n), "{n} nao e nome de categoria");
        }
    }

    #[test]
    fn dataset_vazio_e_conta_comercial_rendem_sem_quebrar() {
        let ds = Dataset::default();
        let a = analyze(&ds, GRAFITE_TODAY);
        let s = render(&build(&a, &ds, GRAFITE_TODAY, Book::Business));
        assert!(s.contains("Conta: comercial"));
        assert!(s.contains("- nenhum"));
        assert!(s.contains("- nenhuma"));
    }

    #[test]
    fn render_e_deterministico() {
        assert_eq!(rendered().0, rendered().0);
    }
}
