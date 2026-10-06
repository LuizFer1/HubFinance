//! Interpretacao financeira das linhas: puro, sem rusqlite, iced, axum ou tokio.

pub mod aggregate;
pub mod colors;
pub mod contract;
pub mod dataset;
pub mod list;
pub mod money;
pub mod people;
pub mod periods;
pub mod reserves;
pub mod view;

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

/// `fixtures()` mais reservas e movimentacoes (plano 2026-10-02-hub-reservas, "Fixture de
/// referencia"). Com `today = "2026-09-24"`: total separado 603.000; custo essencial da
/// emergencia 65.000/mes (Moradia 60.000 + Alimentacao 5.000, janela 2026-03..2026-08, 6 meses
/// de historico); meta 390.000; saldo da emergencia 230.000. Ficam fora de tudo: `M06` (mes
/// futuro), `M07` (apagada), `M17` (reserva apagada) e `M18` (reserva ausente).
#[cfg(test)]
pub(crate) fn fixtures_with_reserves() -> dataset::Dataset {
    use serde_json::{Value, json};

    use dataset::{RESERVE_MOVEMENTS, RESERVES, RawRow};

    let mut ds = fixtures();
    let mut seq = 99;
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
        RESERVES,
        "RE1",
        false,
        json!({
            "kind": "emergency",
            "name": "Reserva de emergência",
            "icon": "piggy-bank",
            "color": "slate",
            "goalMinor": null,
            "multiple": 6,
            "essentialCategoryIds": ["C1", "C2", "C3", "C9"],
            "dueMonth": null,
            "recurringAmountMinor": 50000
        }),
    );
    let pot = |name: &str, icon: &str, color: &str, goal: Value, due: Value, recurring: Value| {
        json!({
            "kind": "pot",
            "name": name,
            "icon": icon,
            "color": color,
            "goalMinor": goal,
            "multiple": null,
            "essentialCategoryIds": null,
            "dueMonth": due,
            "recurringAmountMinor": recurring
        })
    };
    push(
        RESERVES,
        "RP1",
        false,
        pot(
            "Viagem de julho",
            "plane",
            "sky",
            json!(500000),
            json!("2027-07"),
            json!(30000),
        ),
    );
    push(
        RESERVES,
        "RP2",
        false,
        pot(
            "IPVA 2027",
            "car",
            "amber",
            json!(240000),
            json!("2027-01"),
            json!(20000),
        ),
    );
    push(
        RESERVES,
        "RP3",
        false,
        pot(
            "Presentes de Natal",
            "gift",
            "rose",
            json!(80000),
            json!("2026-12"),
            Value::Null,
        ),
    );
    push(
        RESERVES,
        "RP4",
        false,
        pot(
            "Sem meta",
            "wallet",
            "teal",
            Value::Null,
            Value::Null,
            Value::Null,
        ),
    );
    push(
        RESERVES,
        "RP5",
        true,
        pot(
            "Apagada",
            "tag",
            "lime",
            json!(100000),
            Value::Null,
            Value::Null,
        ),
    );
    push(
        RESERVES,
        "RP6",
        false,
        pot(
            "Notebook",
            "tag",
            "violet",
            json!(50000),
            json!("2026-11"),
            json!(10000),
        ),
    );

    let mv = |reserve: &str,
              kind: &str,
              amount: i64,
              on: &str,
              user: &str,
              description: Value,
              recurring: Value| {
        json!({
            "reserveId": reserve,
            "kind": kind,
            "amountMinor": amount,
            "occurredOn": on,
            "userId": user,
            "description": description,
            "recurring": recurring
        })
    };
    let monthly = || json!("Guardado todo mês");
    let t = || json!(true);
    let f = || json!(false);
    let null = || Value::Null;
    let movements = [
        (
            "M01",
            false,
            mv("RE1", "deposit", 50_000, "2026-07-06", "U2", monthly(), t()),
        ),
        (
            "M02",
            false,
            mv("RE1", "deposit", 50_000, "2026-08-06", "U2", monthly(), t()),
        ),
        (
            "M03",
            false,
            mv(
                "RE1",
                "withdrawal",
                20_000,
                "2026-08-18",
                "U1",
                json!("Conserto da geladeira"),
                f(),
            ),
        ),
        (
            "M04",
            false,
            mv("RE1", "deposit", 50_000, "2026-09-06", "U2", monthly(), t()),
        ),
        (
            "M05",
            false,
            mv(
                "RE1",
                "deposit",
                100_000,
                "2026-09-01",
                "U2",
                json!("Sobra de agosto"),
                null(),
            ),
        ),
        (
            "M06",
            false,
            mv("RE1", "deposit", 10_000, "2026-10-05", "U2", null(), null()),
        ),
        (
            "M07",
            true,
            mv("RE1", "deposit", 999, "2026-09-03", "U2", null(), null()),
        ),
        (
            "M08",
            false,
            mv("RP1", "deposit", 30_000, "2026-07-10", "U1", monthly(), t()),
        ),
        (
            "M09",
            false,
            mv(
                "RP1",
                "deposit",
                150_000,
                "2026-07-15",
                "U1",
                json!("Parte do freela"),
                f(),
            ),
        ),
        (
            "M10",
            false,
            mv("RP1", "deposit", 30_000, "2026-08-10", "U1", null(), t()),
        ),
        (
            "M11",
            false,
            mv("RP1", "deposit", 30_000, "2026-09-10", "U1", null(), t()),
        ),
        (
            "M12",
            false,
            mv(
                "RP1",
                "withdrawal",
                5_000,
                "2026-09-20",
                "U1",
                json!("Passagem"),
                f(),
            ),
        ),
        (
            "M13",
            false,
            mv("RP2", "deposit", 20_000, "2026-08-05", "U2", null(), t()),
        ),
        (
            "M14",
            false,
            mv("RP2", "deposit", 20_000, "2026-09-05", "U2", null(), t()),
        ),
        (
            "M15",
            false,
            mv(
                "RP3",
                "deposit",
                38_000,
                "2026-09-12",
                "U1",
                json!("Venda de roupas usadas"),
                f(),
            ),
        ),
        // Autor com perfil apagado e sem descricao.
        (
            "M16",
            false,
            mv("RP4", "deposit", 10_000, "2026-06-01", "U3", null(), null()),
        ),
        (
            "M17",
            false,
            mv("RP5", "deposit", 70_000, "2026-09-02", "U1", null(), null()),
        ),
        (
            "M18",
            false,
            mv("RX", "deposit", 1_000, "2026-09-02", "U1", null(), null()),
        ),
        (
            "M19",
            false,
            mv(
                "RP6",
                "deposit",
                50_000,
                "2026-05-03",
                "U2",
                json!("Bônus"),
                f(),
            ),
        ),
    ];
    for (id, deleted, data) in movements {
        push(RESERVE_MOVEMENTS, id, deleted, data);
    }
    ds.apply(rows);
    ds
}

/// `fixtures()` mais o que a fusao dos padroes do app deixa: `C6` tombstone fundida em `C1`,
/// `P6` tombstone fundida em `P2` e `T20` (despesa de setembro, 100,00) apontando para as
/// copias. Separado da fixture para nao mexer nos totais que os outros testes afirmam.
#[cfg(test)]
pub(crate) fn fixtures_with_merged() -> dataset::Dataset {
    use serde_json::json;

    use dataset::{CATEGORIES, PAYMENT_METHODS, RawRow, TRANSACTIONS};

    let mut ds = fixtures();
    let row = |table: &str, id: &str, deleted: bool, seq: i64, data: serde_json::Value| RawRow {
        table: table.into(),
        id: id.into(),
        deleted_at: deleted.then(|| "2026-10-05T00:00:00Z".to_string()),
        seq,
        data: data.to_string(),
    };
    ds.apply([
        row(
            CATEGORIES,
            "C6",
            true,
            1001,
            json!({ "name": "Alimentação", "color": "red", "icon": "tag", "mergedInto": "C1" }),
        ),
        row(
            PAYMENT_METHODS,
            "P6",
            true,
            1002,
            json!({ "name": "Crédito", "icon": "tag", "mergedInto": "P2" }),
        ),
        row(
            TRANSACTIONS,
            "T20",
            false,
            1003,
            json!({
                "kind": "expense",
                "description": "Feira",
                "amountMinor": 10_000,
                "occurredOn": "2026-09-20",
                "categoryId": "C6",
                "paymentMethodId": "P6",
                "userId": "U1",
            }),
        ),
    ]);
    ds
}
