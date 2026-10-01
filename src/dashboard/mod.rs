//! Interpretacao financeira das linhas: puro, sem rusqlite, iced, axum ou tokio.

// O tema (ui/theme.rs) usa parte da paleta; o resto ganha uso nas telas do plano 2b.
#![allow(dead_code)]

pub mod aggregate;
pub mod colors;
pub mod contract;
pub mod dataset;
pub mod list;
pub mod money;
pub mod people;
pub mod periods;

/// Dataset de exemplo das tarefas de agregacao, lista e telas: duas pessoas vivas e uma
/// apagada, cinco categorias (uma apagada), formas de pagamento (uma apagada), uma
/// recorrencia mensal e lancamentos em marco, julho, agosto e setembro de 2026 com os casos
/// de borda (sem categoria, categoria apagada, sem autor, autor apagado, lancamento apagado).
///
/// Setembro: receita 6.500,00 (1); despesas 2.310,00 (6) = Moradia 1.800 + Alimentacao 400 +
/// Transporte 60 (50 + 10) + "Sem categoria" 50 (20 sem categoria + 30 da apagada).
/// Agosto: receita 6.500,00; despesas 2.100,00. Julho: 6.500,00 / 1.800,00.
#[cfg(test)]
pub(crate) fn fixtures() -> dataset::Dataset {
    use serde_json::{Value, json};

    use dataset::{CATEGORIES, PAYMENT_METHODS, RECURRENCES, RawRow, TRANSACTIONS, USERS};

    let mut seq = 0;
    let mut rows = Vec::new();
    let mut push = |table: &str, id: &str, deleted: bool, data: Value| {
        seq += 1;
        rows.push(RawRow {
            table: table.into(),
            id: id.into(),
            deleted_at: deleted.then(|| "2026-09-30T00:00:00Z".to_string()),
            seq,
            data: data.to_string(),
        });
    };
    push(
        USERS,
        "U1",
        false,
        json!({ "name": "Ana", "color": "fuchsia" }),
    );
    push(
        USERS,
        "U2",
        false,
        json!({ "name": "Luiz", "color": "sky" }),
    );
    push(USERS, "U3", true, json!({ "name": "Ex", "color": "lime" }));
    let category =
        |name: &str, color: &str, icon: &str| json!({ "name": name, "color": color, "icon": icon });
    push(
        CATEGORIES,
        "C1",
        false,
        category("Alimentação", "orange", "utensils"),
    );
    push(
        CATEGORIES,
        "C2",
        false,
        category("Moradia", "amber", "house"),
    );
    push(
        CATEGORIES,
        "C3",
        false,
        category("Transporte", "sky", "car"),
    );
    push(
        CATEGORIES,
        "C4",
        false,
        category("Salário", "emerald", "briefcase"),
    );
    push(CATEGORIES, "C5", true, category("Antiga", "red", "tag"));
    let method = |name: &str, icon: &str| json!({ "name": name, "icon": icon });
    push(PAYMENT_METHODS, "P1", false, method("Pix", "zap"));
    push(
        PAYMENT_METHODS,
        "P2",
        false,
        method("Crédito", "credit-card"),
    );
    push(PAYMENT_METHODS, "P3", false, method("Dinheiro", "banknote"));
    push(PAYMENT_METHODS, "P4", true, method("Cheque", "receipt"));
    push(RECURRENCES, "R1", false, json!({ "frequency": "monthly" }));

    #[allow(clippy::too_many_arguments)]
    fn tx(
        kind: &str,
        description: &str,
        amount: i64,
        on: &str,
        category: Option<&str>,
        user: Option<&str>,
        method: Option<&str>,
        recurrence: Option<&str>,
    ) -> Value {
        json!({
            "kind": kind,
            "description": description,
            "amountMinor": amount,
            "occurredOn": on,
            "categoryId": category,
            "userId": user,
            "paymentMethodId": method,
            "recurrenceId": recurrence,
        })
    }
    let salary = |on: &str| {
        tx(
            "income",
            "Salário",
            650_000,
            on,
            Some("C4"),
            Some("U1"),
            Some("P1"),
            Some("R1"),
        )
    };
    let rent = |on: &str| {
        tx(
            "expense",
            "Aluguel",
            180_000,
            on,
            Some("C2"),
            Some("U2"),
            Some("P1"),
            None,
        )
    };
    // Fora dos seis meses que terminam em setembro.
    push(TRANSACTIONS, "T14", false, salary("2026-03-01"));
    push(TRANSACTIONS, "T01", false, salary("2026-07-05"));
    push(TRANSACTIONS, "T02", false, rent("2026-07-10"));
    push(TRANSACTIONS, "T03", false, salary("2026-08-05"));
    push(TRANSACTIONS, "T04", false, rent("2026-08-10"));
    push(
        TRANSACTIONS,
        "T05",
        false,
        tx(
            "expense",
            "Mercado",
            30_000,
            "2026-08-12",
            Some("C1"),
            Some("U1"),
            Some("P2"),
            None,
        ),
    );
    push(TRANSACTIONS, "T06", false, salary("2026-09-05"));
    push(TRANSACTIONS, "T07", false, rent("2026-09-10"));
    push(
        TRANSACTIONS,
        "T08",
        false,
        tx(
            "expense",
            "Mercado",
            40_000,
            "2026-09-24",
            Some("C1"),
            Some("U1"),
            Some("P2"),
            None,
        ),
    );
    push(
        TRANSACTIONS,
        "T09",
        false,
        tx(
            "expense",
            "Uber",
            5_000,
            "2026-09-24",
            Some("C3"),
            Some("U2"),
            Some("P3"),
            None,
        ),
    );
    push(
        TRANSACTIONS,
        "T10",
        false,
        tx(
            "expense",
            "Padaria",
            2_000,
            "2026-09-24",
            None,
            Some("U1"),
            Some("P2"),
            None,
        ),
    );
    push(
        TRANSACTIONS,
        "T11",
        false,
        tx(
            "expense",
            "Presente",
            3_000,
            "2026-09-12",
            Some("C5"),
            None,
            None,
            None,
        ),
    );
    push(
        TRANSACTIONS,
        "T12",
        true,
        tx(
            "expense",
            "Apagado",
            99_999,
            "2026-09-15",
            Some("C1"),
            Some("U1"),
            Some("P1"),
            None,
        ),
    );
    push(
        TRANSACTIONS,
        "T13",
        false,
        tx(
            "expense",
            "Estacionamento",
            1_000,
            "2026-09-14",
            Some("C3"),
            Some("U3"),
            Some("P4"),
            None,
        ),
    );
    let mut ds = dataset::Dataset::default();
    ds.apply(rows);
    ds
}
