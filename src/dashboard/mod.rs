//! Interpretacao financeira das linhas: puro, sem rusqlite, iced, axum ou tokio.

pub mod aggregate;
pub mod budgets;
pub mod business;
pub mod cards;
pub mod colors;
pub mod contract;
pub mod dataset;
pub mod grafite;
pub mod ids;
pub mod insights;
pub mod list;
pub mod money;
pub mod people;
pub mod periods;
pub mod recurrence;
pub mod reserves;
pub mod schedule;
pub mod view;

/// Dataset montado de linhas JSON pelo `apply` de verdade: os vetores portados do app passam
/// pelo contrato (etapa 1 e 2), nao por struct montada a mao. `(tabela, id, apagada, data)`.
#[cfg(test)]
pub(crate) fn dataset_of(rows: Vec<(&str, &str, bool, serde_json::Value)>) -> dataset::Dataset {
    let mut ds = dataset::Dataset::default();
    ds.apply(
        rows.into_iter()
            .zip(1..)
            .map(|((table, id, deleted, data), seq)| dataset::RawRow {
                table: table.into(),
                id: id.into(),
                deleted_at: deleted.then(|| "2026-09-30T00:00:00Z".to_string()),
                seq,
                data: data.to_string(),
            }),
    );
    ds
}

/// "Hoje" da fixture Grafite: quinta, 19 de novembro de 2026.
#[cfg(test)]
pub(crate) const GRAFITE_TODAY: &str = "2026-11-19";

/// Uma linha da fixture Grafite antes de virar `RawRow`: `(tabela, id, data)`, todas vivas.
#[cfg(test)]
pub(crate) type GrafiteRow = (&'static str, String, serde_json::Value);

/// Linhas da fixture Grafite (spec do dashboard 3.1 e 3.1.1), as mesmas de `grafite.fake.ts` do
/// app com os mesmos ids: a tabela 3.2 e conferida dos dois lados contra elas. Lista crua para
/// um teste poder tirar uma linha antes de montar (o `delete` dos testes do app tira a linha do
/// estado, o que nao e o mesmo que um tombstone para o dedupe de ocorrencia).
#[cfg(test)]
pub(crate) fn grafite_rows() -> Vec<GrafiteRow> {
    use serde_json::{Value, json};

    use dataset::{
        BUDGETS, CATEGORIES, PAYMENT_METHODS, RECURRENCES, RESERVE_MOVEMENTS, RESERVES,
        TRANSACTIONS, USERS,
    };

    let mut rows: Vec<GrafiteRow> = Vec::new();
    let mut push = |table: &'static str, id: &str, data: Value| {
        rows.push((table, id.to_string(), data));
    };

    push(USERS, "UL", json!({ "name": "Luiz", "color": "teal" }));
    push(USERS, "UG", json!({ "name": "Gigi", "color": "rose" }));

    for (id, name, icon, color) in [
        ("CAT-COMPRAS", "Compras", "shopping-bag", "violet"),
        ("CAT-CONFORTO", "Conforto", "coffee", "amber"),
        ("CAT-ASSINATURAS", "Assinaturas", "film", "indigo"),
        ("CAT-CONTAS", "Contas", "receipt", "sky"),
        ("CAT-MERCADO", "Mercado", "shopping-cart", "emerald"),
        ("CAT-SOGRINHA", "Sogrinha", "gift", "fuchsia"),
        ("CAT-MANUTENCAO", "Manutenção", "house", "orange"),
        ("CAT-PETS", "Pets", "paw-print", "lime"),
        ("CAT-TRANSPORTE", "Transporte", "bus", "teal"),
    ] {
        push(
            CATEGORIES,
            id,
            json!({ "name": name, "icon": icon, "color": color, "kind": "expense", "mergedInto": null }),
        );
    }

    push(
        PAYMENT_METHODS,
        "PMP",
        json!({
            "name": "Mercado Pago",
            "icon": "credit-card",
            "color": "violet",
            "kind": "credit",
            "bank": "Mercado Pago",
            "closingDay": 28,
            "dueDay": 8,
        }),
    );

    // Mensais, dia fixo (= dia de `startOn`), ativas, sem `variable`, autor UL.
    let series = [
        (
            "RSAL",
            "income",
            "Salário",
            None,
            350_000,
            "2026-11-05",
            None,
        ),
        (
            "RCON",
            "expense",
            "Contas da casa",
            Some("CAT-CONTAS"),
            53_901,
            "2026-11-10",
            None,
        ),
        // Sem categoria na serie (tile neutro); a ocorrencia entra em Assinaturas.
        (
            "RPRI",
            "expense",
            "Amazon prime",
            None,
            1_999,
            "2026-11-01",
            None,
        ),
        ("RMEL", "expense", "Meli+", None, 999, "2026-11-01", None),
        (
            "RCLA",
            "expense",
            "Claude code",
            Some("CAT-ASSINATURAS"),
            11_840,
            "2026-11-18",
            None,
        ),
        (
            "RSPO",
            "expense",
            "Spotify",
            Some("CAT-ASSINATURAS"),
            1_999,
            "2026-11-15",
            None,
        ),
        (
            "RSOG",
            "expense",
            "Mesada da sogrinha",
            Some("CAT-SOGRINHA"),
            16_862,
            "2026-11-02",
            None,
        ),
        (
            "RCAD",
            "expense",
            "Cadeira (2x)",
            Some("CAT-COMPRAS"),
            5_052,
            "2026-10-21",
            Some("2026-11-21"),
        ),
    ];
    for (id, kind, description, category, amount, start_on, end_on) in series {
        let day: i64 = start_on[8..10].parse().unwrap_or(1);
        push(
            RECURRENCES,
            id,
            json!({
                "frequency": "monthly",
                "kind": kind,
                "description": description,
                "amountMinor": amount,
                "currency": "BRL",
                "categoryId": category,
                "paymentMethodId": null,
                "cashbackMinor": null,
                "scheduleType": "dayOfMonth",
                "scheduleN": day,
                "startOn": start_on,
                "endOn": end_on,
                "active": true,
                "userId": "UL",
            }),
        );
    }

    // Ocorrencias gravadas: id = stableEntityId("<serie>:<competencia>") (3.1.1, colado).
    // RCAD de 2026-11 (QZM0FK4WFZC0Q3CWQFMGF34CFF) vence dia 21 e ainda nao foi gravada.
    for (id, rid, kind, description, amount, on, category) in [
        (
            "7748FB44FQCR7BWM7Q4RFV4MF7",
            "RSAL",
            "income",
            "Salário",
            350_000,
            "2026-11-05",
            None,
        ),
        (
            "FBCWZFMRQBMWQZCRFVCCZZM8QV",
            "RCON",
            "expense",
            "Contas da casa",
            53_901,
            "2026-11-10",
            Some("CAT-CONTAS"),
        ),
        (
            "32AZFJ67VJJFQ2Y7K2TZZJP7BJ",
            "RPRI",
            "expense",
            "Amazon prime",
            1_999,
            "2026-11-01",
            Some("CAT-ASSINATURAS"),
        ),
        (
            "VSRJZHCTV1RTF9CJV9R2Z1CAVH",
            "RMEL",
            "expense",
            "Meli+",
            999,
            "2026-11-01",
            Some("CAT-ASSINATURAS"),
        ),
        (
            "FDMP3D8PZN4YK58EFXM63X86Z5",
            "RCLA",
            "expense",
            "Claude code",
            11_840,
            "2026-11-18",
            Some("CAT-ASSINATURAS"),
        ),
        (
            "K7803KGMV7GGBKRMKQ8G33G4VQ",
            "RSPO",
            "expense",
            "Spotify",
            1_999,
            "2026-11-15",
            Some("CAT-ASSINATURAS"),
        ),
        (
            "Q461Z06XFME170YX74P1F0PXZM",
            "RSOG",
            "expense",
            "Mesada da sogrinha",
            16_862,
            "2026-11-02",
            Some("CAT-SOGRINHA"),
        ),
        (
            "ZWY97REN7CP9ZRPNFWE9QRYNQC",
            "RCAD",
            "expense",
            "Cadeira (2x)",
            5_052,
            "2026-10-21",
            Some("CAT-COMPRAS"),
        ),
    ] {
        push(
            TRANSACTIONS,
            id,
            json!({
                "kind": kind,
                "description": description,
                "amountMinor": amount,
                "currency": "BRL",
                "occurredOn": on,
                "categoryId": category,
                "paymentMethodId": null,
                "cashbackMinor": null,
                "userId": "UL",
                "recurrenceId": rid,
                "occurrenceKey": format!("{rid}:{}", &on[..7]),
            }),
        );
    }

    // Avulsos (3.1.1): `recurrenceId`/`occurrenceKey` nulos, sem `estimated`/`reportState`.
    for (id, on, kind, description, amount, category, user, method) in [
        (
            "1005-1-RECEITA",
            "2026-10-05",
            "income",
            "Salário de outubro",
            507_229,
            None,
            "UL",
            None,
        ),
        (
            "1010-1-COMPRAS",
            "2026-10-10",
            "expense",
            "Compras do mês",
            37_330,
            Some("CAT-COMPRAS"),
            "UL",
            None,
        ),
        (
            "1010-2-MERCADO",
            "2026-10-10",
            "expense",
            "Mercado do mês",
            233_170,
            Some("CAT-MERCADO"),
            "UL",
            None,
        ),
        (
            "1025-1-MERCADO",
            "2026-10-25",
            "expense",
            "Mercado",
            90_648,
            Some("CAT-MERCADO"),
            "UL",
            None,
        ),
        (
            "1103-1-COMPRAS",
            "2026-11-03",
            "expense",
            "Fone de ouvido",
            29_990,
            Some("CAT-COMPRAS"),
            "UL",
            Some("PMP"),
        ),
        (
            "1104-1-MERCADO",
            "2026-11-04",
            "expense",
            "Mercado",
            45_000,
            Some("CAT-MERCADO"),
            "UL",
            None,
        ),
        (
            "1106-1-PETS",
            "2026-11-06",
            "expense",
            "Ração",
            7_500,
            Some("CAT-PETS"),
            "UG",
            None,
        ),
        (
            "1107-1-CONFORTO",
            "2026-11-07",
            "expense",
            "Restaurante",
            18_000,
            Some("CAT-CONFORTO"),
            "UL",
            None,
        ),
        (
            "1109-1-MERCADO",
            "2026-11-09",
            "expense",
            "Feira",
            23_444,
            Some("CAT-MERCADO"),
            "UG",
            None,
        ),
        (
            "1111-1-SOGRINHA",
            "2026-11-11",
            "expense",
            "Remédio da sogrinha",
            628,
            Some("CAT-SOGRINHA"),
            "UG",
            None,
        ),
        (
            "1112-1-CONFORTO",
            "2026-11-12",
            "expense",
            "Cinema e pipoca",
            12_750,
            Some("CAT-CONFORTO"),
            "UL",
            None,
        ),
        (
            "1113-1-MANUTENCAO",
            "2026-11-13",
            "expense",
            "Encanador",
            10_728,
            Some("CAT-MANUTENCAO"),
            "UG",
            None,
        ),
        (
            "1114-1-COMPRAS",
            "2026-11-14",
            "expense",
            "Tênis",
            21_525,
            Some("CAT-COMPRAS"),
            "UL",
            Some("PMP"),
        ),
        (
            "1116-1-MERCADO",
            "2026-11-16",
            "expense",
            "Mercado",
            40_490,
            Some("CAT-MERCADO"),
            "UL",
            None,
        ),
        (
            "1117-1-TRANSPORTE",
            "2026-11-17",
            "expense",
            "Uber",
            5_000,
            Some("CAT-TRANSPORTE"),
            "UG",
            None,
        ),
        (
            "1118-2-ALMOCO",
            "2026-11-18",
            "expense",
            "Almoço",
            2_097,
            Some("CAT-CONFORTO"),
            "UL",
            None,
        ),
        (
            "1118-1-JANTINHA",
            "2026-11-18",
            "expense",
            "Jantinha",
            6_589,
            Some("CAT-CONFORTO"),
            "UL",
            None,
        ),
    ] {
        push(
            TRANSACTIONS,
            id,
            json!({
                "kind": kind,
                "description": description,
                "amountMinor": amount,
                "currency": "BRL",
                "occurredOn": on,
                "categoryId": category,
                "paymentMethodId": method,
                "cashbackMinor": null,
                "userId": user,
                "recurrenceId": null,
                "occurrenceKey": null,
            }),
        );
    }

    // Metas: id = budgetIdFor(categoria) = stableEntityId("budget:<categoria>").
    for (id, category, limit) in [
        ("P6H9YASDE69SPT1DPPHSYTSXEP", "CAT-COMPRAS", 40_000),
        ("5C811CM9N48SH4M1NCR1HC4954", "CAT-CONFORTO", 40_000),
        ("MKD247XYC35JC75E4KX2M7DYW3", "CAT-ASSINATURAS", 18_000),
        ("EFQTTQV2YQQ2AZBTYF7TAQB2EQ", "CAT-CONTAS", 65_000),
        ("PM5V2MS3PC532W9VP45B24SKPW", "CAT-MERCADO", 141_500),
    ] {
        push(
            BUDGETS,
            id,
            json!({ "categoryId": category, "limitMinor": limit }),
        );
    }

    push(
        RESERVES,
        "RE",
        json!({
            "kind": "emergency",
            "name": "Reserva de emergência",
            "icon": "lifebuoy",
            "color": "violet",
            "targetMinor": null,
            "multiple": 6,
            "essentialCategoryIds": null,
            "essentialOverrideMinor": 250_000,
            "deadline": null,
            "recurring": { "amountMinor": 55_000, "day": 5, "since": "2026-11" },
        }),
    );
    push(
        RESERVES,
        "RC",
        json!({
            "kind": "goal",
            "name": "Casamento",
            "icon": "gift",
            "color": "rose",
            "targetMinor": 150_000,
            "multiple": null,
            "essentialCategoryIds": null,
            "essentialOverrideMinor": null,
            "deadline": "2027-11",
            "recurring": null,
        }),
    );
    // O deposito recorrente de novembro de RE nao esta gravado.
    for (id, reserve, amount, on, reason) in [
        ("MOV-RE-1006", "RE", 55_000, "2026-10-06", Value::Null),
        ("MOV-RE-1106", "RE", 10_000, "2026-11-06", Value::Null),
        ("MOV-RE-1112", "RE", -30_000, "2026-11-12", json!("home")),
        ("MOV-RC-1015", "RC", 15_000, "2026-10-15", Value::Null),
    ] {
        push(
            RESERVE_MOVEMENTS,
            id,
            json!({
                "reserveId": reserve,
                "amountMinor": amount,
                "occurredOn": on,
                "userId": "UL",
                "description": null,
                "reason": reason,
                "recurring": false,
            }),
        );
    }
    rows
}

/// Monta um `Dataset` das linhas pelo `apply` de verdade (contrato etapa 1 e 2).
#[cfg(test)]
pub(crate) fn dataset_from(rows: Vec<GrafiteRow>) -> dataset::Dataset {
    let mut ds = dataset::Dataset::default();
    ds.apply(
        rows.into_iter()
            .zip(1..)
            .map(|((table, id, data), seq)| dataset::RawRow {
                table: table.into(),
                id,
                deleted_at: None,
                seq,
                data: data.to_string(),
            }),
    );
    ds
}

/// A fixture Grafite inteira (conta pessoal; `books` vazio).
#[cfg(test)]
pub(crate) fn fixtures_grafite() -> dataset::Dataset {
    dataset_from(grafite_rows())
}

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
            "targetMinor": null,
            "multiple": 6,
            "essentialCategoryIds": ["C1", "C2", "C3", "C9"],
            "essentialOverrideMinor": null,
            "deadline": null,
            "recurring": { "amountMinor": 50000, "day": 5, "since": "2026-01" }
        }),
    );
    let pot = |name: &str, icon: &str, color: &str, goal: Value, due: Value, recurring: Value| {
        json!({
            "kind": "goal",
            "name": name,
            "icon": icon,
            "color": color,
            "targetMinor": goal,
            "multiple": null,
            "essentialCategoryIds": null,
            "essentialOverrideMinor": null,
            "deadline": due,
            // since antigo o bastante para nao mudar nenhum resultado.
            "recurring": if recurring.is_null() {
                serde_json::Value::Null
            } else {
                json!({ "amountMinor": recurring, "day": 10, "since": "2026-01" })
            }
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

    // Formato do app: sem `kind`, o sinal de `amountMinor` decide guardar/retirar. O app exige
    // `reason` em retirada; o hub nao le o campo, mas a fixture segue o contrato real.
    let mv =
        |reserve: &str, amount: i64, on: &str, user: &str, description: Value, recurring: Value| {
            json!({
                "reserveId": reserve,
                "amountMinor": amount,
                "occurredOn": on,
                "userId": user,
                "description": description,
                "reason": if amount < 0 { json!("other") } else { Value::Null },
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
            mv("RE1", 50_000, "2026-07-06", "U2", monthly(), t()),
        ),
        (
            "M02",
            false,
            mv("RE1", 50_000, "2026-08-06", "U2", monthly(), t()),
        ),
        (
            "M03",
            false,
            mv(
                "RE1",
                -20_000,
                "2026-08-18",
                "U1",
                json!("Conserto da geladeira"),
                f(),
            ),
        ),
        (
            "M04",
            false,
            mv("RE1", 50_000, "2026-09-06", "U2", monthly(), t()),
        ),
        (
            "M05",
            false,
            mv(
                "RE1",
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
            mv("RE1", 10_000, "2026-10-05", "U2", null(), null()),
        ),
        (
            "M07",
            true,
            mv("RE1", 999, "2026-09-03", "U2", null(), null()),
        ),
        (
            "M08",
            false,
            mv("RP1", 30_000, "2026-07-10", "U1", monthly(), t()),
        ),
        (
            "M09",
            false,
            mv(
                "RP1",
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
            mv("RP1", 30_000, "2026-08-10", "U1", null(), t()),
        ),
        (
            "M11",
            false,
            mv("RP1", 30_000, "2026-09-10", "U1", null(), t()),
        ),
        (
            "M12",
            false,
            mv("RP1", -5_000, "2026-09-20", "U1", json!("Passagem"), f()),
        ),
        (
            "M13",
            false,
            mv("RP2", 20_000, "2026-08-05", "U2", null(), t()),
        ),
        (
            "M14",
            false,
            mv("RP2", 20_000, "2026-09-05", "U2", null(), t()),
        ),
        (
            "M15",
            false,
            mv(
                "RP3",
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
            mv("RP4", 10_000, "2026-06-01", "U3", null(), null()),
        ),
        (
            "M17",
            false,
            mv("RP5", 70_000, "2026-09-02", "U1", null(), null()),
        ),
        (
            "M18",
            false,
            mv("RX", 1_000, "2026-09-02", "U1", null(), null()),
        ),
        (
            "M19",
            false,
            mv("RP6", 50_000, "2026-05-03", "U2", json!("Bônus"), f()),
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

#[cfg(test)]
mod grafite_tests {
    use super::*;
    use crate::dashboard::aggregate::{in_month, recent, totals};
    use crate::dashboard::contract::Kind;
    use crate::dashboard::ids::stable_entity_id_v1;
    use crate::dashboard::recurrence::project_month;

    #[test]
    fn fixture_grafite_passa_inteira_pelo_contrato() {
        let ds = fixtures_grafite();
        assert_eq!(ds.ignored_total(), 0, "{:?}", ds.ignored);
        assert_eq!(ds.recurrences.len(), 8);
        for r in ds.recurrences.values() {
            assert!(r.rule.is_some(), "{} sem regra", r.id);
        }
        assert!(ds.payment_methods["PMP"].card.is_some());
        assert_eq!(ds.transactions.len(), 25);
        assert_eq!(ds.budgets.len(), 5);
        assert_eq!(ds.reserves.len(), 2);
        assert_eq!(ds.reserve_movements.len(), 4);
    }

    #[test]
    fn fixture_grafite_ids_das_ocorrencias_sao_os_estaveis() {
        let ds = fixtures_grafite();
        for (seed, id) in [
            ("RSAL:2026-11", "7748FB44FQCR7BWM7Q4RFV4MF7"),
            ("RCON:2026-11", "FBCWZFMRQBMWQZCRFVCCZZM8QV"),
            ("RPRI:2026-11", "32AZFJ67VJJFQ2Y7K2TZZJP7BJ"),
            ("RMEL:2026-11", "VSRJZHCTV1RTF9CJV9R2Z1CAVH"),
            ("RCLA:2026-11", "FDMP3D8PZN4YK58EFXM63X86Z5"),
            ("RSPO:2026-11", "K7803KGMV7GGBKRMKQ8G33G4VQ"),
            ("RSOG:2026-11", "Q461Z06XFME170YX74P1F0PXZM"),
            ("RCAD:2026-10", "ZWY97REN7CP9ZRPNFWE9QRYNQC"),
            ("RCAD:2026-11", "QZM0FK4WFZC0Q3CWQFMGF34CFF"),
        ] {
            assert_eq!(stable_entity_id_v1(seed), id, "{seed}");
        }
        assert!(
            !ds.occurrence_index()
                .knows_occurrence("RCAD", "RCAD:2026-11")
        );
        for b in [
            ("budget:CAT-COMPRAS", "P6H9YASDE69SPT1DPPHSYTSXEP"),
            ("budget:CAT-MERCADO", "PM5V2MS3PC532W9VP45B24SKPW"),
        ] {
            assert_eq!(stable_entity_id_v1(b.0), b.1);
        }
    }

    #[test]
    fn fixture_grafite_totais_de_outubro_e_novembro() {
        let ds = fixtures_grafite();
        let nov = totals(in_month(&ds, "2026-11"));
        assert_eq!((nov.income_minor, nov.expense_minor), (350_000, 311_341));
        let oct = totals(in_month(&ds, "2026-10"));
        assert_eq!((oct.income_minor, oct.expense_minor), (507_229, 366_200));
        let oct_19 =
            totals(in_month(&ds, "2026-10").filter(|t| t.occurred_on.as_str() <= "2026-10-19"));
        assert_eq!(oct_19.expense_minor, 270_500);
        let sum = |f: &dyn Fn(&dataset::Transaction) -> bool| -> i64 {
            in_month(&ds, "2026-11")
                .filter(|t| t.kind == Kind::Expense && f(t))
                .map(|t| t.amount_minor)
                .sum()
        };
        assert_eq!(sum(&|t| t.recurrence_id.is_some()), 87_600);
        assert_eq!(sum(&|t| t.recurrence_id.is_none()), 223_741);
        assert_eq!(sum(&|t| t.user_id.as_deref() == Some("UL")), 264_041);
        assert_eq!(sum(&|t| t.user_id.as_deref() == Some("UG")), 47_300);
        assert_eq!(
            sum(&|t| t.payment_method_id.as_deref() == Some("PMP")),
            51_515
        );
    }

    #[test]
    fn fixture_grafite_por_vir_em_novembro_e_so_a_cadeira() {
        let ds = fixtures_grafite();
        let due = project_month(&ds, "2026-11");
        assert_eq!(due.len(), 1);
        assert_eq!(
            (
                due[0].recurrence_id.as_str(),
                due[0].date.as_str(),
                due[0].amount_minor
            ),
            ("RCAD", "2026-11-21", 5_052)
        );
    }

    #[test]
    fn fixture_grafite_ultimos_lancamentos() {
        let ds = fixtures_grafite();
        let names: Vec<&str> = recent(&ds, "2026-11", 3)
            .iter()
            .map(|t| t.description.as_str())
            .collect();
        assert_eq!(names, ["Claude code", "Almoço", "Jantinha"]);
        assert_eq!(GRAFITE_TODAY, "2026-11-19");
    }
}
