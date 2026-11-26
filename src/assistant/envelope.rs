//! O que o LLM ve (spec 5). `render` e a UNICA fonte do texto: a previa mostra a mesma string
//! que sai, entao o que o usuario aprova e exatamente o que e enviado. Entra so agregado por
//! mes, por nome de categoria e por reserva; nunca descricao, pessoa, aparelho, forma de
//! pagamento, data exata ou id (o teste de sentinela abaixo confere, e se pegar algo a
//! correcao e no envelope ou no achado, nunca no teste).

use std::fmt::Write as _;

use crate::advisor::{Analysis, FigureValue, months_label};
use crate::dashboard::aggregate::{
    NO_CATEGORY_KEY, NO_CATEGORY_NAME, expense_by_category, monthly_totals,
};
use crate::dashboard::budgets::budget_report;
use crate::dashboard::contract::Book;
use crate::dashboard::dataset::Dataset;
use crate::dashboard::money::{Money, format_brl, signed_brl};
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
                    // Com sinal: "Sobra prevista" negativa e a falta que o titulo anuncia; em valor
                    // absoluto o modelo leria uma sobra.
                    FigureValue::Money(m) => signed_brl(*m),
                    FigureValue::Percent(p) => format!("{p}%"),
                    FigureValue::MonthsTenths(t) => months_label(*t),
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
        // So o numero da meta: o envelope nunca leva mascara.
        .map(|r| emergency_view(ds, r, &current, Money::default()).goal_minor)
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
        // Mes sem nenhum valor e mes sem lancamento (o hub so conhece o que os celulares
        // enviaram, e o historico pode comecar depois): "R$ 0,00" diria ao modelo que a casa
        // nao gastou nada, e ele trataria isso como fato.
        if *inc == 0 && *exp == 0 {
            let _ = writeln!(s, "- {m}: sem lançamentos");
            continue;
        }
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
                "sem despesas".to_string()
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
        let _ = writeln!(s, "- {n}: saldo {}", signed_brl(*bal));
    }
    if let Some(g) = e.emergency_goal_minor {
        let _ = writeln!(s, "- Meta da reserva de emergência: {}", format_brl(g));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::advisor::{FindingKind, analyze};
    use crate::dashboard::dataset::{
        BOOKS, BUDGETS, CATEGORIES, PAYMENT_METHODS, RECURRENCES, RESERVE_MOVEMENTS, RESERVES,
        TRANSACTIONS, USERS,
    };
    use crate::dashboard::money::MINUS;
    use crate::dashboard::{GRAFITE_TODAY, dataset_of, fixtures, fixtures_grafite};
    use serde_json::{Value, json};

    type Row = (&'static str, &'static str, bool, Value);

    const ALL_KINDS: [FindingKind; 9] = [
        FindingKind::Pace,
        FindingKind::Forecast,
        FindingKind::CategoryAboveNormal,
        FindingKind::Budget,
        FindingKind::SavingsRate,
        FindingKind::FixedWeight,
        FindingKind::NextMonthCommitted,
        FindingKind::Emergency,
        FindingKind::Invoices,
    ];

    fn envelope_of(ds: &Dataset, today: &str) -> (String, Vec<FindingKind>) {
        let a = analyze(ds, today);
        let kinds = a.findings.iter().map(|f| f.kind).collect();
        (render(&build(&a, ds, today, Book::Personal)), kinds)
    }

    fn grafite() -> Dataset {
        fixtures_grafite().for_book(Book::Personal)
    }

    /// Dataset com nome-sentinela em todo campo que o envelope nao pode levar: pessoa viva e
    /// apagada, forma de pagamento viva e apagada, categoria apagada (cujo gasto vira "Sem
    /// categoria"), descricao de lancamento, de serie e de movimento de reserva. Renda de 1.000
    /// em setembro e outubro e uma serie fixa de `series_amount` (com ou sem as ocorrencias
    /// gravadas: com elas o peso do fixo dispara, sem elas dispara o comprometido do mes que
    /// vem).
    fn sentinel_rows(series_amount: i64, occurrences: bool) -> Vec<Row> {
        let tx = |description: &str, kind: &str, amount: i64, on: &str, cat: Option<&str>| {
            json!({
                "kind": kind, "description": description, "amountMinor": amount,
                "occurredOn": on, "categoryId": cat,
                "userId": "U-VIVO", "paymentMethodId": "PM-CARTAO"
            })
        };
        let mut rows: Vec<Row> = vec![
            (
                USERS,
                "U-VIVO",
                false,
                json!({ "name": "Zelda Sentinela", "color": "teal" }),
            ),
            (
                USERS,
                "U-APAGADO",
                true,
                json!({ "name": "Ex Sentinela", "color": "teal" }),
            ),
            (
                CATEGORIES,
                "CAT-MORADIA",
                false,
                json!({ "name": "Moradia", "icon": "tag", "color": "sky", "kind": "expense", "mergedInto": null }),
            ),
            (
                CATEGORIES,
                "CAT-APAGADA",
                true,
                json!({ "name": "Antiga Sentinela", "icon": "tag", "color": "sky", "kind": "expense", "mergedInto": null }),
            ),
            (
                PAYMENT_METHODS,
                "PM-CARTAO",
                false,
                json!({ "name": "Cartão Sentinela", "icon": "credit-card" }),
            ),
            (
                PAYMENT_METHODS,
                "PM-APAGADA",
                true,
                json!({ "name": "Forma Apagada Sentinela", "icon": "zap" }),
            ),
            (
                RECURRENCES,
                "REC-ALUGUEL",
                false,
                json!({
                    "frequency": "monthly", "kind": "expense", "description": "Aluguel Sentinela",
                    "amountMinor": series_amount, "currency": "BRL", "categoryId": null,
                    "paymentMethodId": "PM-CARTAO", "cashbackMinor": null,
                    "scheduleType": "dayOfMonth", "scheduleN": 5,
                    "startOn": "2026-09-05", "endOn": null, "active": true, "userId": "U-VIVO"
                }),
            ),
            (
                TRANSACTIONS,
                "TX-RENDA-9",
                false,
                tx("Salário Sentinela", "income", 100_000, "2026-09-05", None),
            ),
            (
                TRANSACTIONS,
                "TX-RENDA-10",
                false,
                tx("Salário Sentinela", "income", 100_000, "2026-10-05", None),
            ),
            (
                TRANSACTIONS,
                "TX-GASTO-9",
                false,
                json!({
                    "kind": "expense", "description": "Gasto Sentinela apagado", "amountMinor": 5_000,
                    "occurredOn": "2026-09-12", "categoryId": "CAT-APAGADA",
                    "userId": "U-APAGADO", "paymentMethodId": "PM-APAGADA"
                }),
            ),
            (
                TRANSACTIONS,
                "TX-GASTO-10",
                false,
                tx(
                    "Gasto Sentinela solto",
                    "expense",
                    5_000,
                    "2026-10-12",
                    None,
                ),
            ),
            (
                RESERVES,
                "RES-EMERG",
                false,
                json!({
                    "kind": "emergency", "name": "Reserva de emergência", "icon": "lifebuoy",
                    "color": "violet", "targetMinor": null, "multiple": 6,
                    "essentialCategoryIds": null, "essentialOverrideMinor": 100_000,
                    "deadline": null, "recurring": null
                }),
            ),
            (
                RESERVE_MOVEMENTS,
                "MOV-DEPOSITO",
                false,
                json!({
                    "reserveId": "RES-EMERG", "amountMinor": 50_000, "occurredOn": "2026-10-06",
                    "userId": "U-VIVO", "description": "Depósito Sentinela", "reason": null,
                    "recurring": false
                }),
            ),
        ];
        if occurrences {
            for (id, on) in [
                ("TX-ALUGUEL-9", "2026-09-05"),
                ("TX-ALUGUEL-10", "2026-10-05"),
            ] {
                rows.push((
                    TRANSACTIONS,
                    id,
                    false,
                    json!({
                        "kind": "expense", "description": "Aluguel Sentinela",
                        "amountMinor": series_amount, "occurredOn": on,
                        "categoryId": "CAT-MORADIA", "recurrenceId": "REC-ALUGUEL",
                        "occurrenceKey": format!("REC-ALUGUEL:{}", &on[..7]),
                        "userId": "U-VIVO", "paymentMethodId": "PM-CARTAO"
                    }),
                ));
            }
        }
        rows
    }

    fn sentinel(series_amount: i64, occurrences: bool) -> Dataset {
        let ds = dataset_of(sentinel_rows(series_amount, occurrences));
        assert_eq!(
            ds.ignored_total(),
            0,
            "linha fora do contrato: {:?}",
            ds.ignored
        );
        ds.for_book(Book::Personal)
    }

    /// As linhas da conta COMERCIAL, com nomes-sentinela proprios: conta, categoria, forma de
    /// pagamento, vendas e compras, reserva, movimento e meta.
    fn business_rows() -> Vec<Row> {
        vec![
            (
                BOOKS,
                "BOOK-LOJA",
                false,
                json!({ "kind": "business", "name": "Loja Sentinela Comercial", "color": "amber", "icon": "storefront" }),
            ),
            (
                CATEGORIES,
                "CAT-LOJA",
                false,
                json!({ "name": "Insumos Sentinela Comercial", "icon": "tag", "color": "sky", "kind": "expense", "mergedInto": null, "book": "business" }),
            ),
            (
                PAYMENT_METHODS,
                "PM-LOJA",
                false,
                json!({ "name": "Maquininha Sentinela Comercial", "icon": "credit-card", "book": "business" }),
            ),
            (
                TRANSACTIONS,
                "TX-LOJA-VENDA-9",
                false,
                json!({ "kind": "income", "description": "Venda Sentinela Comercial", "amountMinor": 777_700, "occurredOn": "2026-09-08", "categoryId": null, "paymentMethodId": "PM-LOJA", "book": "business" }),
            ),
            (
                TRANSACTIONS,
                "TX-LOJA-VENDA-10",
                false,
                json!({ "kind": "income", "description": "Venda Sentinela Comercial", "amountMinor": 777_700, "occurredOn": "2026-10-08", "categoryId": null, "paymentMethodId": "PM-LOJA", "book": "business" }),
            ),
            (
                TRANSACTIONS,
                "TX-LOJA-COMPRA-9",
                false,
                json!({ "kind": "expense", "description": "Fornecedor Sentinela Comercial", "amountMinor": 333_300, "occurredOn": "2026-09-09", "categoryId": "CAT-LOJA", "paymentMethodId": "PM-LOJA", "book": "business" }),
            ),
            (
                TRANSACTIONS,
                "TX-LOJA-COMPRA-10",
                false,
                json!({ "kind": "expense", "description": "Fornecedor Sentinela Comercial", "amountMinor": 333_300, "occurredOn": "2026-10-09", "categoryId": "CAT-LOJA", "paymentMethodId": "PM-LOJA", "book": "business" }),
            ),
            (
                RESERVES,
                "RES-LOJA",
                false,
                json!({
                    "kind": "goal", "name": "Caixa Sentinela Comercial", "icon": "piggy-bank",
                    "color": "teal", "targetMinor": 500_000, "multiple": null,
                    "essentialCategoryIds": null, "essentialOverrideMinor": null,
                    "deadline": null, "recurring": null, "book": "business"
                }),
            ),
            (
                RESERVE_MOVEMENTS,
                "MOV-LOJA",
                false,
                json!({
                    "reserveId": "RES-LOJA", "amountMinor": 123_400, "occurredOn": "2026-10-07",
                    "userId": null, "description": "Aporte Sentinela Comercial", "reason": null,
                    "recurring": false, "book": "business"
                }),
            ),
            (
                BUDGETS,
                "BUD-LOJA",
                false,
                json!({ "categoryId": "CAT-LOJA", "limitMinor": 654_300, "book": "business" }),
            ),
        ]
    }

    /// As duas contas no mesmo dataset, como o hub as recebe.
    fn both_books() -> Dataset {
        let mut rows = sentinel_rows(70_000, true);
        rows.extend(business_rows());
        let ds = dataset_of(rows);
        assert_eq!(
            ds.ignored_total(),
            0,
            "linha fora do contrato: {:?}",
            ds.ignored
        );
        ds
    }

    /// Gasta mais do que ganha e deixa a reserva de emergencia no negativo: os numeros que
    /// o modelo precisa ler COM sinal.
    fn overspent() -> Dataset {
        let tx = |kind: &str, amount: i64, on: &str| json!({ "kind": kind, "description": "Lançamento Sentinela", "amountMinor": amount, "occurredOn": on });
        let ds = dataset_of(vec![
            (
                TRANSACTIONS,
                "TX-R9",
                false,
                tx("income", 100_000, "2026-09-05"),
            ),
            (
                TRANSACTIONS,
                "TX-R10",
                false,
                tx("income", 100_000, "2026-10-05"),
            ),
            (
                TRANSACTIONS,
                "TX-D9",
                false,
                tx("expense", 150_000, "2026-09-10"),
            ),
            (
                TRANSACTIONS,
                "TX-D10",
                false,
                tx("expense", 150_000, "2026-10-10"),
            ),
            (
                RESERVES,
                "RES-EMERG",
                false,
                json!({
                    "kind": "emergency", "name": "Reserva de emergência", "icon": "lifebuoy",
                    "color": "violet", "targetMinor": null, "multiple": 6,
                    "essentialCategoryIds": null, "essentialOverrideMinor": 100_000,
                    "deadline": null, "recurring": null
                }),
            ),
            (
                RESERVE_MOVEMENTS,
                "MOV-SAQUE",
                false,
                json!({
                    "reserveId": "RES-EMERG", "amountMinor": -20_000, "occurredOn": "2026-10-06",
                    "userId": null, "description": null, "reason": null, "recurring": false
                }),
            ),
        ]);
        assert_eq!(
            ds.ignored_total(),
            0,
            "linha fora do contrato: {:?}",
            ds.ignored
        );
        ds.for_book(Book::Personal)
    }

    /// Cada execucao do sentinela: nome, dataset e "hoje". Juntas tem de acender os nove
    /// tipos de achado e levar "Sem categoria" a um envelope.
    fn runs() -> Vec<(&'static str, Dataset, &'static str)> {
        vec![
            ("grafite", grafite(), GRAFITE_TODAY),
            ("grafite com fatura a vencer", grafite(), "2026-11-28"),
            (
                "exemplo no meio de setembro",
                fixtures().for_book(Book::Personal),
                "2026-09-24",
            ),
            (
                "exemplo com setembro fechado",
                fixtures().for_book(Book::Personal),
                "2026-10-03",
            ),
            ("fixo acima de 60%", sentinel(70_000, true), "2026-11-19"),
            (
                "comprometido acima de 70%",
                sentinel(80_000, false),
                "2026-11-19",
            ),
            ("saiu mais do que entrou", overspent(), "2026-11-19"),
        ]
    }

    #[derive(Default, Debug)]
    struct Checked {
        descriptions: usize,
        methods: usize,
        people: usize,
        ids: usize,
        deleted_names: usize,
    }

    /// Tudo que e dado pessoal no dataset e nao pode estar em `s`, em TODAS as linhas (vivas
    /// e apagadas), nao so nas que o envelope usa. Devolve os vazamentos e o que foi conferido.
    fn check(s: &str, ds: &Dataset) -> (Vec<String>, Checked) {
        check_in(s, ds, ds)
    }

    /// `scope` e a conta do envelope (de onde vem o que PODE aparecer); `ds` e o dataset
    /// inteiro, com as duas contas (de onde vem o que NAO pode).
    fn check_in(s: &str, scope: &Dataset, ds: &Dataset) -> (Vec<String>, Checked) {
        // Nomes que PODEM aparecer. Uma ocorrencia que cai inteira dentro de um deles nao e
        // vazamento ("Mercado" dentro de "Mercado e feira"); qualquer outra e. O nome nao e
        // apagado do texto antes de procurar: "Mercado Pago" (forma de pagamento) perderia o
        // "Mercado" e vazaria sem ser visto. Categoria apagada nao esta aqui: o gasto dela
        // sai como "Sem categoria".
        let allowed: Vec<&str> = scope
            .categories
            .values()
            .filter(|c| !c.deleted)
            .map(|c| c.name.as_str())
            .chain(
                scope
                    .reserves
                    .values()
                    .filter(|r| !r.deleted)
                    .map(|r| r.name.as_str()),
            )
            .chain([NO_CATEGORY_NAME, "Categoria removida"])
            .collect();
        let covered: Vec<(usize, usize)> = allowed
            .iter()
            .filter(|n| !n.is_empty())
            .flat_map(|n| s.match_indices(*n).map(|(i, m)| (i, i + m.len())))
            .collect();
        // Um nome que e ele mesmo permitido (descricao igual a categoria) nao se distingue.
        let leaks = |needle: &str| {
            !allowed.contains(&needle)
                && s.match_indices(needle)
                    .any(|(i, m)| !covered.iter().any(|&(a, b)| a <= i && i + m.len() <= b))
        };

        let mut out = Vec::new();
        let mut seen = Checked::default();
        let mut other_book = 0;
        let mut look = |what: &str, needle: &str, counter: &mut usize| {
            let needle = needle.trim();
            if needle.len() < 3 {
                return;
            }
            *counter += 1;
            if leaks(needle) {
                out.push(format!("{what} vazou: {needle}"));
            }
        };
        for t in ds.transactions.values() {
            look("descricao", &t.description, &mut seen.descriptions);
        }
        for r in ds.recurrences.values() {
            look(
                "serie",
                r.description.as_deref().unwrap_or(""),
                &mut seen.descriptions,
            );
        }
        for m in ds.reserve_movements.values() {
            look(
                "descricao de movimento",
                m.description.as_deref().unwrap_or(""),
                &mut seen.descriptions,
            );
        }
        for p in ds.payment_methods.values() {
            look("forma de pagamento", &p.name, &mut seen.methods);
            if let Some(card) = &p.card {
                look("banco", &card.bank, &mut seen.methods);
            }
            if p.deleted {
                seen.deleted_names += 1;
            }
        }
        for u in ds.users.values() {
            look("pessoa", &u.name, &mut seen.people);
            if u.deleted {
                seen.deleted_names += 1;
            }
        }
        for c in ds.categories.values().filter(|c| c.deleted) {
            look("categoria apagada", &c.name, &mut seen.deleted_names);
        }
        // Categoria e reserva da OUTRA conta: dentro da conta do envelope seus nomes sao
        // permitidos, fora dela nao.
        for c in ds
            .categories
            .values()
            .filter(|c| !scope.categories.contains_key(&c.id))
        {
            look("categoria de outra conta", &c.name, &mut other_book);
        }
        for r in ds
            .reserves
            .values()
            .filter(|r| !scope.reserves.contains_key(&r.id))
        {
            look("reserva de outra conta", &r.name, &mut other_book);
        }
        for b in ds.books.values() {
            look("conta", &b.name, &mut seen.people);
        }
        // Rotulos internos de campo ausente nao identificam ninguem, mas nao sao para o modelo.
        for label in [ds.payment_method_name(None), "Forma removida".to_string()] {
            let mut ignored = 0;
            look("rotulo interno", &label, &mut ignored);
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
            seen.ids += 1;
            if s.contains(id.as_str()) {
                out.push(format!("id vazou: {id}"));
            }
        }
        // Nenhuma data exata AAAA-MM-DD (AAAA-MM, de mes, pode).
        for (i, w) in s.as_bytes().windows(10).enumerate() {
            if is_exact_date(w) {
                out.push(format!("data exata vazou perto do byte {i}"));
            }
        }
        (out, seen)
    }

    fn is_exact_date(w: &[u8]) -> bool {
        w.len() == 10
            && w[..4].iter().all(u8::is_ascii_digit)
            && w[4] == b'-'
            && w[5..7].iter().all(u8::is_ascii_digit)
            && w[7] == b'-'
            && w[8..10].iter().all(u8::is_ascii_digit)
    }

    #[test]
    fn tem_os_seis_meses_e_os_achados() {
        let (s, _) = envelope_of(&grafite(), GRAFITE_TODAY);
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
        let mut kinds = std::collections::HashSet::new();
        let mut total = Checked::default();
        let mut sem_categoria = false;
        for (name, ds, today) in runs() {
            let (s, k) = envelope_of(&ds, today);
            kinds.extend(k);
            let (leaks, seen) = check(&s, &ds);
            assert!(leaks.is_empty(), "{name}: {leaks:?}\n{s}");
            // Nao-vacuo por execucao: ha o que procurar.
            assert!(seen.descriptions > 0, "{name}: {seen:?}");
            total.descriptions += seen.descriptions;
            total.methods += seen.methods;
            total.people += seen.people;
            total.ids += seen.ids;
            total.deleted_names += seen.deleted_names;
            sem_categoria |= s.contains(NO_CATEGORY_NAME);
            assert!(
                !s.contains(&ds.payment_method_name(None)),
                "{name}: rotulo de forma ausente no envelope"
            );
        }
        // ... e no conjunto: tudo que o teste diz procurar existe nos exemplos.
        assert!(total.methods >= 3 && total.people >= 4, "{total:?}");
        assert!(total.ids >= 20, "{total:?}");
        assert!(total.deleted_names >= 3, "{total:?}");
        assert!(sem_categoria, "'Sem categoria' nunca chegou a um envelope");
        for kind in ALL_KINDS {
            assert!(kinds.contains(&kind), "nenhuma execucao gera {kind:?}");
        }
    }

    /// O detector tem de gritar quando o texto leva um dos sentinelas: sem isto, um `check`
    /// que nunca acha nada passaria em tudo. "Mercado Pago" e o caso dificil (contem o nome da
    /// categoria "Mercado").
    #[test]
    fn o_detector_pega_cada_tipo_de_vazamento() {
        let ds = grafite();
        let (s, _) = envelope_of(&ds, GRAFITE_TODAY);
        assert!(check(&s, &ds).0.is_empty());
        for extra in [
            "Mercado Pago",
            "Gigi",
            "Meli+",
            "Amazon prime",
            "Sem forma de pagamento",
            "CAT-COMPRAS",
            "vence em 2026-11-18",
        ] {
            let leaked = format!("{s}\n{extra}");
            assert!(!check(&leaked, &ds).0.is_empty(), "nao pegou: {extra}");
        }
        let ds = sentinel(70_000, true);
        let (s, _) = envelope_of(&ds, "2026-11-19");
        assert!(check(&s, &ds).0.is_empty());
        for extra in [
            "Cartão Sentinela",
            "Forma Apagada Sentinela",
            "Zelda Sentinela",
            "Ex Sentinela",
            "Aluguel Sentinela",
            "Depósito Sentinela",
            "Salário Sentinela",
            "Antiga Sentinela",
            "TX-ALUGUEL-9",
        ] {
            let leaked = format!("{s}\n{extra}");
            assert!(!check(&leaked, &ds).0.is_empty(), "nao pegou: {extra}");
        }
        assert!(is_exact_date(b"2026-11-19"));
        assert!(!is_exact_date(b"2026-11 R$"));
    }

    /// Duas contas no mesmo hub: nada de uma pode aparecer no envelope da outra (nomes de
    /// categoria, reserva e conta, descricoes, formas de pagamento, ids). Cada lado e checado
    /// contra o dataset INTEIRO, que e quem tem os sentinelas do outro.
    #[test]
    fn contas_nao_se_misturam_no_envelope() {
        let full = both_books();
        assert!(full.has_business_book());
        let personal = full.for_book(Book::Personal);
        let business = full.for_book(Book::Business);
        assert!(business.transactions.len() >= 4 && personal.transactions.len() >= 4);

        let envelope = |scope: &Dataset, book| {
            let a = analyze(scope, "2026-11-19");
            render(&build(&a, scope, "2026-11-19", book))
        };
        let sp = envelope(&personal, Book::Personal);
        let sb = envelope(&business, Book::Business);

        let (leaks, seen) = check_in(&sp, &personal, &full);
        assert!(leaks.is_empty(), "pessoal: {leaks:?}\n{sp}");
        assert!(seen.descriptions >= 6 && seen.ids >= 15, "{seen:?}");
        let (leaks, _) = check_in(&sb, &business, &full);
        assert!(leaks.is_empty(), "comercial: {leaks:?}\n{sb}");

        // Nao-vacuo: cada envelope mostra o que e DELE, e so isso.
        assert!(
            sp.contains("Conta: pessoal") && sp.contains("Moradia"),
            "{sp}"
        );
        assert!(sp.contains("Reserva de emergência"), "{sp}");
        assert!(!sp.contains("Comercial"), "{sp}");
        assert!(sb.contains("Conta: comercial"), "{sb}");
        assert!(sb.contains("Insumos Sentinela Comercial"), "{sb}");
        assert!(sb.contains("Caixa Sentinela Comercial"), "{sb}");
        assert!(sb.contains("R$ 7.777,00"), "{sb}");
        assert!(!sb.contains("Moradia"), "{sb}");
        assert!(!sb.contains("Reserva de emergência"), "{sb}");
        assert!(!sb.contains("Zelda"), "{sb}");
        // Detector: um sentinela da outra conta no texto e pego.
        for extra in [
            "Caixa Sentinela Comercial",
            "Venda Sentinela Comercial",
            "Maquininha Sentinela Comercial",
            "Loja Sentinela Comercial",
        ] {
            let leaked = format!("{sp}\n{extra}");
            assert!(
                !check_in(&leaked, &personal, &full).0.is_empty(),
                "nao pegou: {extra}"
            );
        }
        for extra in ["Zelda Sentinela", "Aluguel Sentinela", "Cartão Sentinela"] {
            let leaked = format!("{sb}\n{extra}");
            assert!(
                !check_in(&leaked, &business, &full).0.is_empty(),
                "nao pegou: {extra}"
            );
        }
    }

    #[test]
    fn categorias_do_envelope_sao_nomes_nunca_ids() {
        let ds = grafite();
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
        // Gasto de categoria apagada ou sem categoria: "Sem categoria", nunca o id nem o nome.
        let ds = sentinel(70_000, true);
        let (s, _) = envelope_of(&ds, "2026-11-19");
        assert!(s.contains("Sem categoria R$ 50,00"), "{s}");
    }

    #[test]
    fn dinheiro_negativo_leva_sinal() {
        // A falta prevista do grafite: o titulo diz "faltar" e a figura tem de ser negativa.
        let (s, _) = envelope_of(&grafite(), GRAFITE_TODAY);
        assert!(
            s.contains(&format!("Sobra prevista: {MINUS}R$ 960,00")),
            "{s}"
        );

        let (s, kinds) = envelope_of(&overspent(), "2026-11-19");
        assert!(kinds.contains(&FindingKind::SavingsRate));
        assert!(s.contains(&format!("sobra {MINUS}R$ 500,00")), "{s}");
        assert!(s.contains(&format!("Sobra média: {MINUS}R$ 500,00")), "{s}");
        assert!(kinds.contains(&FindingKind::Emergency));
        // Meses cobertos negativos: sinal antes, sem "0,-2", e singular abaixo de 2.
        assert!(
            s.contains(&format!("Meses cobertos: {MINUS}0,2 mês")),
            "{s}"
        );
        assert!(!s.contains(",-"), "{s}");
        assert!(s.contains(&format!("Saldo {MINUS}R$ 200,00")), "{s}");
        assert!(
            s.contains(&format!("Saldo da reserva: {MINUS}R$ 200,00")),
            "{s}"
        );
        assert!(
            s.contains(&format!("Reserva de emergência: saldo {MINUS}R$ 200,00")),
            "{s}"
        );
    }

    #[test]
    fn mes_sem_lancamento_nao_vira_zero_de_verdade() {
        let (s, _) = envelope_of(&grafite(), GRAFITE_TODAY);
        assert!(s.contains("- 2026-05: sem lançamentos"), "{s}");
        assert!(!s.contains("2026-05: receita"), "{s}");
        assert!(s.contains("- 2026-10: receita R$ 5.072,29"), "{s}");
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
        let a = envelope_of(&grafite(), GRAFITE_TODAY).0;
        assert_eq!(a, envelope_of(&grafite(), GRAFITE_TODAY).0);
    }
}
