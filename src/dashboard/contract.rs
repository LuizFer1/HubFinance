//! Contrato de campos: o que o dashboard le de cada tabela que o app sincroniza.
//!
//! O hub guarda JSON opaco; so aqui uma linha vira transacao, categoria etc. Campo
//! desconhecido e ignorado (sem `deny_unknown_fields`): o app ganha campos novos sem o hub
//! mudar. Campo do contrato ausente ou com tipo errado faz a *linha* ser ignorada e contada
//! pelo `Dataset`, sem derrubar sync nem janela.
//!
//! `#[serde(default)]` so nos campos anulaveis: para campo obrigatorio, ausente nao e nulo.
//! Para anulavel, ausente vale nulo porque o `ROADMAP.md` do mobile fixa que campo novo nasce
//! anulavel justamente para linha antiga continuar valida.

use serde::Deserialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Income,
    Expense,
}

/// `src/domain/model/transaction.ts`. Nao lidos: `currency`, `cashbackMinor`, `occurrenceKey`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionFields {
    pub kind: Kind,
    pub description: String,
    /// Inteiro JSON: `12.5` e `"12"` sao tipo errado (o `serde_json` recusa os dois para `i64`).
    pub amount_minor: i64,
    pub occurred_on: String,
    #[serde(default)]
    pub category_id: Option<String>,
    #[serde(default)]
    pub payment_method_id: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub recurrence_id: Option<String>,
}

/// `src/domain/model/category.ts`. Token de cor e chave de icone desconhecidos sao validos:
/// a tela cai no neutro e no `circle-dashed`, a linha guarda o que veio.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryFields {
    pub name: String,
    pub color: String,
    pub icon: String,
    /// So em tombstone: a fusao dos padroes do app (spec mobile `padroes-estaveis`) apaga cada
    /// copia apontando para a linha estavel, sem regravar os lancamentos antigos. O dashboard
    /// segue o campo ao resolver referencias; o sync nao o conhece.
    #[serde(default)]
    pub merged_into: Option<String>,
}

/// `src/domain/model/user.ts`. `avatar` e um data URI; decodificar e problema da tela.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserFields {
    pub name: String,
    pub color: String,
    #[serde(default)]
    pub avatar: Option<String>,
}

/// `src/domain/model/payment-method.ts`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentMethodFields {
    pub name: String,
    pub icon: String,
    /// Como em `CategoryFields`; so em tombstone: a fusao dos padroes do app (spec mobile `padroes-estaveis`) apaga cada
    /// copia apontando para a linha estavel, sem regravar os lancamentos antigos. O dashboard
    /// segue o campo ao resolver referencias; o sync nao o conhece.
    #[serde(default)]
    pub merged_into: Option<String>,
}

/// `src/domain/model/recurrence.ts`: so a frequencia, para o rotulo da tag ("Mensal").
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurrenceFields {
    pub frequency: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReserveKind {
    Emergency,
    /// O texto de UI continua "Caixinha"; "goal" e o nome que o app grava
    /// (`HomeFinance_Mobile/src/domain/model/reserve.ts`). Sem alias para "pot": nenhum app
    /// gravou "pot", so testes e seed usavam, e um alias aceitaria dado que o app nunca produz.
    Goal,
}

/// Linha de `reserves`. O contrato segue o formato que o app ja grava
/// (`HomeFinance_Mobile/src/domain/model/reserve.ts`; handoff em
/// `docs/desktop/plans/2026-10-07-hub-reservas-contrato-app.md`): o log e eterno e ja ha
/// linhas gravadas assim, por isso e o hub que se adapta, nao o contrario.
/// Nao lidos: `createdAt` e `recurring.day` (regra de materializacao do app).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReserveFields {
    pub kind: ReserveKind,
    pub name: String,
    pub icon: String,
    pub color: String,
    /// Meta da caixinha (`targetMinor` no app). Nao e lida para `emergency`: a meta dela e
    /// derivada do custo essencial.
    #[serde(default, rename = "targetMinor")]
    pub goal_minor: Option<i64>,
    /// `u8` de proposito: o serde recusa fracao, texto e fora de 0..=255, entao "tipo errado"
    /// sai de graca, sem validacao a mais. Nulo vale o padrao (6) na tela.
    #[serde(default)]
    pub multiple: Option<u8>,
    /// Anulavel porque e configuracao: nulo vale "nenhuma categoria essencial", e a tela diz
    /// isso em vez de ignorar a reserva inteira.
    #[serde(default)]
    pub essential_category_ids: Option<Vec<String>>,
    /// Custo essencial digitado no app quando nao havia historico; substitui o calculado
    /// (o uso na meta da emergencia e de outra etapa, aqui so se carrega).
    #[serde(default)]
    pub essential_override_minor: Option<i64>,
    /// `deadline` no app: `YYYY-MM`; fora do formato e tipo errado (compara como string com
    /// `month_of`).
    #[serde(default, rename = "deadline")]
    pub due_month: Option<String>,
    /// Deposito mensal. Objeto errado ("sim", `amountMinor` fracionado ou ausente) derruba a
    /// linha pelo proprio serde, como os outros campos: o hub nao adivinha o que o app quis dizer.
    #[serde(default)]
    pub recurring: Option<RecurringFields>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringFields {
    pub amount_minor: i64,
    /// Nao lido pelo hub (regra de materializacao do app); aceito para tipar o objeto.
    #[serde(default)]
    pub day: Option<u8>,
    /// Primeiro mes (`YYYY-MM`) em que o deposito vale.
    #[serde(default)]
    pub since: Option<String>,
}

impl ReserveFields {
    pub fn recurring_amount_minor(&self) -> Option<i64> {
        self.recurring.as_ref().map(|r| r.amount_minor)
    }

    pub fn recurring_since(&self) -> Option<&str> {
        self.recurring.as_ref().and_then(|r| r.since.as_deref())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MovementKind {
    Deposit,
    Withdrawal,
}

/// Linha de `reserveMovements` ja traduzida. Nao lido: `reason` (o motivo da retirada e regra
/// do app). Nao e desserializada direto: o JSON do app nao tem `kind`, o sinal de `amountMinor`
/// decide guardar/retirar e `parse_reserve_movement` o traduz para `kind`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReserveMovementFields {
    pub reserve_id: String,
    pub kind: MovementKind,
    /// Sempre positivo aqui: o sinal vindo do JSON ja virou `kind`.
    pub amount_minor: i64,
    pub occurred_on: String,
    pub user_id: Option<String>,
    pub description: Option<String>,
    pub recurring: Option<bool>,
}

/// Forma crua do app. Sem `deny_unknown_fields`: um `kind` legado vira campo desconhecido e e
/// ignorado, senao o sinal e o `kind` poderiam discordar.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMovementFields {
    reserve_id: String,
    amount_minor: i64,
    occurred_on: String,
    #[serde(default)]
    user_id: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    recurring: Option<bool>,
}

#[derive(Debug, thiserror::Error)]
pub enum ContractError {
    /// Traz o motivo do serde ("missing field `reserveId`", "invalid type: ... amountMinor").
    #[error("{0}")]
    Json(#[from] serde_json::Error),
    #[error("amountMinor nao pode ser zero")]
    AmountZero,
    #[error("amountMinor fora do intervalo")]
    AmountOutOfRange,
    #[error("occurredOn fora do formato YYYY-MM-DD")]
    OccurredOnShape,
    #[error("deadline fora do formato YYYY-MM")]
    DueMonthShape,
    #[error("recurring.since fora do formato YYYY-MM")]
    RecurringSinceShape,
}

pub fn parse_transaction(data: &str) -> Result<TransactionFields, ContractError> {
    let fields: TransactionFields = serde_json::from_str(data)?;
    if !is_iso_date(&fields.occurred_on) {
        return Err(ContractError::OccurredOnShape);
    }
    Ok(fields)
}

pub fn parse_category(data: &str) -> Result<CategoryFields, ContractError> {
    Ok(serde_json::from_str(data)?)
}

pub fn parse_user(data: &str) -> Result<UserFields, ContractError> {
    Ok(serde_json::from_str(data)?)
}

pub fn parse_payment_method(data: &str) -> Result<PaymentMethodFields, ContractError> {
    Ok(serde_json::from_str(data)?)
}

pub fn parse_recurrence(data: &str) -> Result<RecurrenceFields, ContractError> {
    Ok(serde_json::from_str(data)?)
}

pub fn parse_reserve(data: &str) -> Result<ReserveFields, ContractError> {
    let fields: ReserveFields = serde_json::from_str(data)?;
    if fields
        .due_month
        .as_deref()
        .is_some_and(|m| !is_iso_month(m))
    {
        return Err(ContractError::DueMonthShape);
    }
    // A projecao do deposito mensal compara `since` como string com o mes corrente; um formato
    // livre compararia errado sem aviso.
    if fields.recurring_since().is_some_and(|m| !is_iso_month(m)) {
        return Err(ContractError::RecurringSinceShape);
    }
    Ok(fields)
}

pub fn parse_reserve_movement(data: &str) -> Result<ReserveMovementFields, ContractError> {
    let raw: RawMovementFields = serde_json::from_str(data)?;
    if !is_iso_date(&raw.occurred_on) {
        return Err(ContractError::OccurredOnShape);
    }
    let (kind, amount_minor) = match raw.amount_minor.cmp(&0) {
        std::cmp::Ordering::Greater => (MovementKind::Deposit, raw.amount_minor),
        // `checked_abs`: `-i64::MIN` estoura (panic em debug); essa linha vira "fora do contrato".
        std::cmp::Ordering::Less => match raw.amount_minor.checked_abs() {
            Some(abs) => (MovementKind::Withdrawal, abs),
            None => return Err(ContractError::AmountOutOfRange),
        },
        // Linha com 0 e corrupta (o app nunca grava): contar em "fora do contrato", nao somar.
        std::cmp::Ordering::Equal => return Err(ContractError::AmountZero),
    };
    Ok(ReserveMovementFields {
        reserve_id: raw.reserve_id,
        kind,
        amount_minor,
        occurred_on: raw.occurred_on,
        user_id: raw.user_id,
        description: raw.description,
        recurring: raw.recurring,
    })
}

/// 7 bytes, `-` em 4, digitos no resto, mes 01..=12: a mesma forma do comeco de `is_iso_date`,
/// para `deadline`/`recurring.since` compararem como string com `month_of(occurredOn)` sem parsear.
pub fn is_iso_month(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 7 || b[4] != b'-' {
        return false;
    }
    if !b
        .iter()
        .enumerate()
        .all(|(i, c)| i == 4 || c.is_ascii_digit())
    {
        return false;
    }
    let month = (b[5] - b'0') * 10 + (b[6] - b'0');
    (1..=12).contains(&month)
}

/// 10 bytes, `-` em 4 e 7, digitos no resto, mes 01..=12. Dia nao e validado: o hub nao julga
/// regra de negocio, e `2026-02-31` aparece cru na lista.
pub fn is_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let digits = b
        .iter()
        .enumerate()
        .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit());
    if !digits {
        return false;
    }
    let month = (b[5] - b'0') * 10 + (b[6] - b'0');
    (1..=12).contains(&month)
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    fn tx() -> Value {
        json!({
            "kind": "expense",
            "description": "Mercado",
            "amountMinor": 12345,
            "currency": "BRL",
            "occurredOn": "2026-09-14",
            "categoryId": "01HZZZZZZZZZZZZZZZZZZZZZC1",
            "paymentMethodId": "01HZZZZZZZZZZZZZZZZZZZZZM1",
            "userId": "01HZZZZZZZZZZZZZZZZZZZZZP1",
            "recurrenceId": null,
            "cashbackMinor": 10,
            "foo": 1
        })
    }

    fn with(mut v: Value, key: &str, value: Value) -> String {
        v[key] = value;
        v.to_string()
    }

    fn without(mut v: Value, key: &str) -> String {
        v.as_object_mut().unwrap().remove(key);
        v.to_string()
    }

    fn err_text<T: std::fmt::Debug>(r: Result<T, ContractError>) -> String {
        r.expect_err("devia falhar").to_string()
    }

    #[test]
    fn transacao_valida_com_campos_extras() {
        let t = parse_transaction(&tx().to_string()).unwrap();
        assert_eq!(t.kind, Kind::Expense);
        assert_eq!(t.amount_minor, 12345);
        assert_eq!(t.occurred_on, "2026-09-14");
        assert_eq!(t.user_id.as_deref(), Some("01HZZZZZZZZZZZZZZZZZZZZZP1"));
        assert_eq!(t.recurrence_id, None);
    }

    #[test]
    fn kind_desconhecido_e_erro() {
        let e = err_text(parse_transaction(&with(tx(), "kind", json!("transfer"))));
        assert!(e.contains("transfer") || e.contains("variant"), "{e}");
        let e = err_text(parse_transaction(&without(tx(), "kind")));
        assert!(e.contains("kind"), "{e}");
    }

    #[test]
    fn amount_minor_precisa_ser_inteiro() {
        for bad in [json!(12.5), json!("12")] {
            let e = err_text(parse_transaction(&with(tx(), "amountMinor", bad)));
            assert!(
                e.contains("amountMinor") || e.contains("invalid type"),
                "{e}"
            );
        }
        let e = err_text(parse_transaction(&without(tx(), "amountMinor")));
        assert!(e.contains("amountMinor"), "{e}");
    }

    #[test]
    fn occurred_on_fora_do_formato() {
        for bad in [
            json!("2026-9-1"),
            json!("2026-13-01"),
            json!("2026-00-01"),
            json!("2026-09-01T00:00"),
        ] {
            let e = err_text(parse_transaction(&with(tx(), "occurredOn", bad)));
            assert!(e.contains("occurredOn"), "{e}");
        }
        assert!(parse_transaction(&with(tx(), "occurredOn", json!(20260901))).is_err());
        // Dia impossivel passa: o hub nao julga.
        assert!(parse_transaction(&with(tx(), "occurredOn", json!("2026-02-31"))).is_ok());
    }

    #[test]
    fn anulaveis_ausentes_ou_nulos_viram_none() {
        let mut v = tx();
        for key in ["categoryId", "paymentMethodId", "userId", "recurrenceId"] {
            v.as_object_mut().unwrap().remove(key);
        }
        let t = parse_transaction(&v.to_string()).unwrap();
        assert_eq!(t.category_id, None);
        assert_eq!(t.payment_method_id, None);
        assert_eq!(t.user_id, None);
        assert_eq!(t.recurrence_id, None);
        let t = parse_transaction(&with(tx(), "userId", Value::Null)).unwrap();
        assert_eq!(t.user_id, None);
    }

    #[test]
    fn descricao_com_tipo_errado() {
        assert!(parse_transaction(&with(tx(), "description", json!(5))).is_err());
    }

    #[test]
    fn categorias() {
        let ok =
            json!({ "name": "Mercado", "color": "orange", "icon": "utensils", "kind": "expense" });
        assert!(parse_category(&ok.to_string()).is_ok());
        let neon = with(ok.clone(), "color", json!("neon"));
        assert_eq!(parse_category(&neon).unwrap().color, "neon");
        assert!(err_text(parse_category(&without(ok.clone(), "color"))).contains("color"));
        assert!(err_text(parse_category(&without(ok.clone(), "icon"))).contains("icon"));
        assert_eq!(parse_category(&ok.to_string()).unwrap().merged_into, None);
        let nulo = with(ok.clone(), "mergedInto", Value::Null);
        assert_eq!(parse_category(&nulo).unwrap().merged_into, None);
        let fundida = with(ok.clone(), "mergedInto", json!("C1"));
        assert_eq!(
            parse_category(&fundida).unwrap().merged_into.as_deref(),
            Some("C1")
        );
        // O serde_json nao cita o campo em tipo errado de valor ("invalid type: integer `5`,
        // expected a string"); a linha cai em `ignored` do mesmo jeito.
        let e = err_text(parse_category(&with(ok, "mergedInto", json!(5))));
        assert!(e.contains("invalid type"), "{e}");
    }

    #[test]
    fn usuarios() {
        let ok = json!({ "name": "Ana", "color": "fuchsia" });
        assert_eq!(parse_user(&ok.to_string()).unwrap().avatar, None);
        let big = format!("data:image/png;base64,{}", "A".repeat(60 * 1024));
        let u = parse_user(&with(ok.clone(), "avatar", json!(big))).unwrap();
        assert_eq!(u.avatar.map(|a| a.len()), Some(big.len()));
        assert!(parse_user(&with(ok, "avatar", json!(7))).is_err());
    }

    #[test]
    fn formas_de_pagamento() {
        let ok = json!({ "name": "Pix", "icon": "zap", "color": "teal", "kind": "pix" });
        assert!(parse_payment_method(&ok.to_string()).is_ok());
        assert!(err_text(parse_payment_method(&without(ok.clone(), "name"))).contains("name"));
        assert!(err_text(parse_payment_method(&without(ok.clone(), "icon"))).contains("icon"));
        assert_eq!(
            parse_payment_method(&ok.to_string()).unwrap().merged_into,
            None
        );
        let nulo = with(ok.clone(), "mergedInto", Value::Null);
        assert_eq!(parse_payment_method(&nulo).unwrap().merged_into, None);
        let fundida = with(ok.clone(), "mergedInto", json!("M1"));
        assert_eq!(
            parse_payment_method(&fundida)
                .unwrap()
                .merged_into
                .as_deref(),
            Some("M1")
        );
        let e = err_text(parse_payment_method(&with(ok, "mergedInto", json!(5))));
        assert!(e.contains("invalid type"), "{e}");
    }

    #[test]
    fn recorrencias() {
        let ok = json!({ "frequency": "monthly", "description": "Aluguel" });
        assert_eq!(
            parse_recurrence(&ok.to_string()).unwrap().frequency,
            "monthly"
        );
        assert!(err_text(parse_recurrence(&without(ok, "frequency"))).contains("frequency"));
    }

    #[test]
    fn json_que_nao_e_objeto() {
        for bad in ["[]", "null", "{", "\"texto\"", "12"] {
            assert!(parse_transaction(bad).is_err(), "{bad}");
            assert!(parse_category(bad).is_err(), "{bad}");
            assert!(parse_user(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn data_iso() {
        assert!(is_iso_date("2026-09-24"));
        assert!(is_iso_date("2026-12-31"));
        assert!(!is_iso_date("2026-9-24"));
        assert!(!is_iso_date("2026/09/24"));
        assert!(!is_iso_date("2026-09-2a"));
        assert!(!is_iso_date("2026-13-01"));
        assert!(!is_iso_date(""));
    }

    fn reserve() -> Value {
        json!({
            "id": "01JZ0000000000000000000RE1",
            "createdAt": "2026-10-01T18:00:00.000Z",
            "updatedAt": "0001790000000-0000-01JZ0000000000000000000DV1",
            "deletedAt": null,
            "kind": "goal",
            "name": "Viagem",
            "icon": "gift",
            "color": "rose",
            "targetMinor": 500000,
            "multiple": null,
            "essentialCategoryIds": null,
            "essentialOverrideMinor": null,
            "deadline": "2027-07",
            "recurring": { "amountMinor": 30000, "day": 10, "since": "2026-11" },
            "foo": 1
        })
    }

    fn movement() -> Value {
        json!({
            "id": "01JZ0000000000000000000MV1",
            "createdAt": "2026-09-06T12:00:00.000Z",
            "updatedAt": "0001790000000-0000-01JZ0000000000000000000DV1",
            "deletedAt": null,
            "reserveId": "RE1",
            "amountMinor": 50000,
            "occurredOn": "2026-09-06",
            "userId": "U2",
            "description": "Guardado todo mês",
            "reason": null,
            "recurring": true
        })
    }

    #[test]
    fn reserva_valida_com_campos_extras() {
        let r = parse_reserve(&reserve().to_string()).unwrap();
        assert_eq!(r.kind, ReserveKind::Goal);
        assert_eq!(r.goal_minor, Some(500_000));
        assert_eq!(r.due_month.as_deref(), Some("2027-07"));
        assert_eq!(r.recurring_amount_minor(), Some(30_000));
        assert_eq!(r.recurring_since(), Some("2026-11"));
        assert_eq!(r.essential_override_minor, None);
        assert_eq!(r.multiple, None);
        assert_eq!(r.essential_category_ids, None);
    }

    #[test]
    fn reserva_kind_desconhecido_ou_ausente() {
        let e = err_text(parse_reserve(&with(reserve(), "kind", json!("box"))));
        assert!(e.contains("box") || e.contains("variant"), "{e}");
        // "pot" foi o nome antigo: sem alias, cai como qualquer valor desconhecido.
        assert!(parse_reserve(&with(reserve(), "kind", json!("pot"))).is_err());
        let e = err_text(parse_reserve(&without(reserve(), "kind")));
        assert!(e.contains("kind"), "{e}");
    }

    #[test]
    fn reserva_multiple_e_u8() {
        let r = parse_reserve(&with(reserve(), "multiple", json!(6))).unwrap();
        assert_eq!(r.multiple, Some(6));
        for bad in [json!(6.5), json!("6"), json!(300), json!(-1)] {
            assert!(
                parse_reserve(&with(reserve(), "multiple", bad.clone())).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn reserva_categorias_essenciais() {
        let r = parse_reserve(&with(
            reserve(),
            "essentialCategoryIds",
            json!(["C1", "C2"]),
        ))
        .unwrap();
        assert_eq!(
            r.essential_category_ids,
            Some(vec!["C1".to_string(), "C2".to_string()])
        );
        for bad in [json!("C1"), json!([1])] {
            assert!(
                parse_reserve(&with(reserve(), "essentialCategoryIds", bad.clone())).is_err(),
                "{bad}"
            );
        }
        let r = parse_reserve(&without(reserve(), "essentialCategoryIds")).unwrap();
        assert_eq!(r.essential_category_ids, None);
    }

    #[test]
    fn reserva_due_month_fora_do_formato() {
        for bad in [json!("2027-7"), json!("2027-13"), json!("2027-07-01")] {
            let e = err_text(parse_reserve(&with(reserve(), "deadline", bad)));
            assert!(e.contains("deadline"), "{e}");
        }
        // Numero e tipo errado do serde, que tambem cita o campo.
        let e = err_text(parse_reserve(&with(reserve(), "deadline", json!(202707))));
        assert!(e.contains("invalid type"), "{e}");
        let r = parse_reserve(&with(reserve(), "deadline", Value::Null)).unwrap();
        assert_eq!(r.due_month, None);
        let r = parse_reserve(&without(reserve(), "deadline")).unwrap();
        assert_eq!(r.due_month, None);
    }

    #[test]
    fn reserva_goal_fracionado_e_anulaveis_ausentes() {
        assert!(parse_reserve(&with(reserve(), "targetMinor", json!(12.5))).is_err());
        let r = parse_reserve(
            &json!({ "kind": "goal", "name": "X", "icon": "tag", "color": "slate" }).to_string(),
        )
        .unwrap();
        assert_eq!(r.goal_minor, None);
        assert_eq!(r.multiple, None);
        assert_eq!(r.essential_category_ids, None);
        assert_eq!(r.due_month, None);
        assert_eq!(r.recurring, None);
        assert_eq!(r.recurring_amount_minor(), None);
        assert_eq!(r.essential_override_minor, None);
    }

    #[test]
    fn reserva_recurring_tipos_e_formato() {
        let r = parse_reserve(&with(reserve(), "recurring", Value::Null)).unwrap();
        assert_eq!(r.recurring_amount_minor(), None);
        assert!(parse_reserve(&with(reserve(), "recurring", json!("sim"))).is_err());
        assert!(
            parse_reserve(&with(
                reserve(),
                "recurring",
                json!({ "amountMinor": 12.5 })
            ))
            .is_err()
        );
        assert!(parse_reserve(&with(reserve(), "recurring", json!({ "day": 10 }))).is_err());
        let e = err_text(parse_reserve(&with(
            reserve(),
            "recurring",
            json!({ "amountMinor": 100, "since": "2026-1" }),
        )));
        assert!(e.contains("recurring.since"), "{e}");
        let r =
            parse_reserve(&with(reserve(), "recurring", json!({ "amountMinor": 100 }))).unwrap();
        assert_eq!(r.recurring_since(), None);
    }

    #[test]
    fn reserva_essential_override_lido() {
        let r = parse_reserve(&with(reserve(), "essentialOverrideMinor", json!(250000))).unwrap();
        assert_eq!(r.essential_override_minor, Some(250_000));
        assert!(parse_reserve(&with(reserve(), "essentialOverrideMinor", json!("x"))).is_err());
    }

    #[test]
    fn reserva_de_emergencia() {
        let ok = json!({
            "kind": "emergency",
            "name": "Reserva",
            "icon": "x",
            "color": "slate",
            "multiple": 6,
            "essentialCategoryIds": ["C1"]
        });
        let r = parse_reserve(&ok.to_string()).unwrap();
        assert_eq!(r.kind, ReserveKind::Emergency);
        assert_eq!(r.multiple, Some(6));
    }

    #[test]
    fn reserva_campos_do_app_chegam_projetados() {
        let r = parse_reserve(&reserve().to_string()).unwrap();
        assert_eq!(r.goal_minor, Some(500_000));
        assert_eq!(r.due_month.as_deref(), Some("2027-07"));
        assert_eq!(r.recurring_amount_minor(), Some(30_000));
        assert_eq!(r.recurring.as_ref().and_then(|x| x.day), Some(10));
        assert_eq!(r.recurring_since(), Some("2026-11"));
    }

    #[test]
    fn reserva_recurring_nulo_ou_ausente_passa() {
        let r = parse_reserve(&with(reserve(), "recurring", Value::Null)).unwrap();
        assert_eq!(r.recurring, None);
        let r = parse_reserve(&without(reserve(), "recurring")).unwrap();
        assert_eq!(r.recurring, None);
    }

    #[test]
    fn reserva_essential_override_nulo_ausente_e_tipos_errados() {
        let r = parse_reserve(&with(reserve(), "essentialOverrideMinor", json!(65000))).unwrap();
        assert_eq!(r.essential_override_minor, Some(65_000));
        let r = parse_reserve(&with(reserve(), "essentialOverrideMinor", Value::Null)).unwrap();
        assert_eq!(r.essential_override_minor, None);
        let r = parse_reserve(&without(reserve(), "essentialOverrideMinor")).unwrap();
        assert_eq!(r.essential_override_minor, None);
        for bad in [json!(12.5), json!("65000")] {
            assert!(
                parse_reserve(&with(reserve(), "essentialOverrideMinor", bad.clone())).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn reserva_de_emergencia_como_o_app_grava() {
        let row = json!({
            "id": "01JZ0000000000000000000RE2",
            "createdAt": "2026-10-01T18:00:00.000Z",
            "updatedAt": "1759341600000-0000-01JZ0000000000000000000DV1",
            "deletedAt": null,
            "kind": "emergency",
            "name": "Reserva de emergência",
            "icon": "lifebuoy",
            "color": "violet",
            "targetMinor": null,
            "multiple": 6,
            "essentialCategoryIds": ["C1", "C2"],
            "essentialOverrideMinor": 350000,
            "deadline": null,
            "recurring": { "amountMinor": 50000, "day": 5, "since": "2026-11" }
        });
        let r = parse_reserve(&row.to_string()).unwrap();
        assert_eq!(r.kind, ReserveKind::Emergency);
        assert_eq!(r.multiple, Some(6));
        assert_eq!(r.essential_override_minor, Some(350_000));
        assert_eq!(r.goal_minor, None);
        assert_eq!(r.due_month, None);
        assert_eq!(r.recurring_amount_minor(), Some(50_000));
    }

    #[test]
    fn movimentacao_valida() {
        let m = parse_reserve_movement(&movement().to_string()).unwrap();
        assert_eq!(m.reserve_id, "RE1");
        assert_eq!(m.kind, MovementKind::Deposit);
        assert_eq!(m.amount_minor, 50_000);
        assert_eq!(m.recurring, Some(true));
    }

    #[test]
    fn movimentacao_positiva_e_deposito() {
        let m = parse_reserve_movement(&with(movement(), "amountMinor", json!(50_000))).unwrap();
        assert_eq!(m.kind, MovementKind::Deposit);
        assert_eq!(m.amount_minor, 50_000);
    }

    #[test]
    fn movimentacao_negativa_e_retirada() {
        let mut v = movement();
        v["amountMinor"] = json!(-15_000);
        v["reason"] = json!("health");
        let m = parse_reserve_movement(&v.to_string()).unwrap();
        assert_eq!(m.kind, MovementKind::Withdrawal);
        assert_eq!(m.amount_minor, 15_000);
    }

    #[test]
    fn movimentacao_zero_e_erro() {
        let e = err_text(parse_reserve_movement(&with(
            movement(),
            "amountMinor",
            json!(0),
        )));
        assert!(e.contains("zero"), "{e}");
    }

    #[test]
    fn movimentacao_i64_min_e_erro_sem_panic() {
        let e = err_text(parse_reserve_movement(&with(
            movement(),
            "amountMinor",
            json!(i64::MIN),
        )));
        assert!(e.contains("intervalo"), "{e}");
        let m =
            parse_reserve_movement(&with(movement(), "amountMinor", json!(i64::MIN + 1))).unwrap();
        assert_eq!(m.kind, MovementKind::Withdrawal);
        assert_eq!(m.amount_minor, i64::MAX);
    }

    #[test]
    fn movimentacao_kind_extra_e_ignorado() {
        for kind in ["withdrawal", "transfer"] {
            let m = parse_reserve_movement(&with(movement(), "kind", json!(kind))).unwrap();
            assert_eq!(m.kind, MovementKind::Deposit);
            assert_eq!(m.amount_minor, 50_000);
        }
    }

    #[test]
    fn movimentacao_valor_invalido() {
        for bad in [json!(12.5), json!("12")] {
            assert!(parse_reserve_movement(&with(movement(), "amountMinor", bad)).is_err());
        }
        assert!(parse_reserve_movement(&without(movement(), "amountMinor")).is_err());
    }

    #[test]
    fn movimentacao_data_e_campos() {
        for bad in [
            json!("2026-9-6"),
            json!("2026-13-01"),
            json!("2026-09-06T00:00"),
        ] {
            let e = err_text(parse_reserve_movement(&with(movement(), "occurredOn", bad)));
            assert!(e.contains("occurredOn"), "{e}");
        }
        assert!(
            parse_reserve_movement(&with(movement(), "occurredOn", json!("2026-02-31"))).is_ok()
        );
        assert!(parse_reserve_movement(&without(movement(), "reserveId")).is_err());
        assert!(parse_reserve_movement(&with(movement(), "recurring", json!("sim"))).is_err());
        let mut v = movement();
        for key in ["userId", "description", "recurring"] {
            v.as_object_mut().unwrap().remove(key);
        }
        let m = parse_reserve_movement(&v.to_string()).unwrap();
        assert_eq!(m.user_id, None);
        assert_eq!(m.description, None);
        assert_eq!(m.recurring, None);
    }

    #[test]
    fn mes_iso() {
        assert!(is_iso_month("2027-07"));
        assert!(is_iso_month("2027-12"));
        for bad in ["2027-7", "2027-13", "2027-00", "2027/07", "2027-07-01", ""] {
            assert!(!is_iso_month(bad), "{bad}");
        }
    }

    #[test]
    fn reservas_json_que_nao_e_objeto() {
        for bad in ["[]", "null", "{"] {
            assert!(parse_reserve(bad).is_err(), "{bad}");
            assert!(parse_reserve_movement(bad).is_err(), "{bad}");
        }
    }
}
