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
}

/// `src/domain/model/recurrence.ts`: so a frequencia, para o rotulo da tag ("Mensal").
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurrenceFields {
    pub frequency: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ContractError {
    /// Traz o motivo do serde ("missing field `kind`", "invalid type: ... amountMinor").
    #[error("{0}")]
    Json(#[from] serde_json::Error),
    #[error("occurredOn fora do formato YYYY-MM-DD")]
    OccurredOnShape,
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
        assert!(err_text(parse_category(&without(ok, "icon"))).contains("icon"));
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
        assert!(err_text(parse_payment_method(&without(ok, "icon"))).contains("icon"));
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
}
