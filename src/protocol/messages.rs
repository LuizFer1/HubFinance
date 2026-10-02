//! Tipos do contrato HTTP. Os nomes no fio sao camelCase, como no IndexedDB do app.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Mais que isso num push e recusado inteiro: limita o tempo de uma transacao do lote.
pub const MAX_PUSH_ROWS: usize = 1000;
pub const MAX_PULL_LIMIT: usize = 1000;
pub const DEFAULT_PULL_LIMIT: usize = 500;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InfoResponse {
    pub name: String,
    pub version: String,
    pub protocol: u32,
    pub epoch: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PairRequest {
    pub token: String,
    pub device_id: String,
    pub name: String,
    /// `meta.localUserId` do app: liga o aparelho a pessoa para a tela Conexao. Opcional
    /// porque um app sem perfil (ou mais antigo) ainda pode parear.
    #[serde(default)]
    pub user_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PairResponse {
    pub device_id: String,
    pub key: String,
    pub epoch: String,
    pub hub_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MeResponse {
    pub device_id: String,
    pub name: String,
    pub paired_at: String,
    pub epoch: String,
    /// `null` quando o pareamento veio sem `userId`.
    pub user_id: Option<String>,
}

/// `row` fica como `Value` de proposito: a validacao e por linha (`parse_row`), e uma linha
/// ruim nao pode derrubar a desserializacao do lote inteiro.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PushEntry {
    pub table: String,
    pub row: Value,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PushRequest {
    pub epoch: String,
    pub rows: Vec<PushEntry>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AcceptedRow {
    pub table: String,
    pub id: String,
    pub seq: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IgnoredRow {
    pub table: String,
    pub id: String,
    pub reason: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RejectedRow {
    pub index: usize,
    pub error: String,
    pub message: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PushResponse {
    pub epoch: String,
    pub accepted: Vec<AcceptedRow>,
    pub ignored: Vec<IgnoredRow>,
    pub rejected: Vec<RejectedRow>,
    pub seq: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PullQuery {
    pub epoch: String,
    #[serde(default)]
    pub cursor: i64,
    pub limit: Option<usize>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PulledRow {
    pub table: String,
    pub seq: i64,
    pub row: Value,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PullResponse {
    pub epoch: String,
    pub rows: Vec<PulledRow>,
    pub cursor: i64,
    pub has_more: bool,
}

/// `error` e o codigo estavel; `message` e texto para a tela do celular. `epoch` so aparece
/// no `epoch_mismatch`, para o celular saber para qual epoch recomecar.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ErrorBody {
    pub error: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epoch: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn push_request_desserializa_o_json_da_spec() {
        let body = json!({
            "epoch": "01J9ZZZZZZZZZZZZZZZZZZZZZZ",
            "rows": [
                { "table": "transactions", "row": {
                    "id": "01HZZZZZZZZZZZZZZZZZZZZZC1",
                    "updatedAt": "1759344000000-0000-01HZZZZZZZZZZZZZZZZZZZZZZA",
                    "deletedAt": null, "dirty": 1, "amountMinor": 1200
                } }
            ]
        });
        let request: PushRequest = serde_json::from_value(body).unwrap();
        assert_eq!(request.epoch, "01J9ZZZZZZZZZZZZZZZZZZZZZZ");
        assert_eq!(request.rows.len(), 1);
        assert_eq!(request.rows[0].table, "transactions");
        assert_eq!(request.rows[0].row["amountMinor"], json!(1200));
        assert_eq!(request.rows[0].row["dirty"], json!(1));
    }

    #[test]
    fn push_request_sem_rows_array_falha() {
        let body = json!({ "epoch": "x", "rows": {} });
        assert!(serde_json::from_value::<PushRequest>(body).is_err());
        assert!(serde_json::from_value::<PushRequest>(json!({ "epoch": "x" })).is_err());
    }

    #[test]
    fn push_response_serializa_em_camel_case() {
        let response = PushResponse {
            epoch: "E".into(),
            accepted: vec![AcceptedRow {
                table: "t".into(),
                id: "A".into(),
                seq: 42,
            }],
            ignored: vec![IgnoredRow {
                table: "t".into(),
                id: "B".into(),
                reason: "older".into(),
            }],
            rejected: vec![RejectedRow {
                index: 3,
                error: "invalid_updated_at".into(),
                message: "m".into(),
            }],
            seq: 42,
        };
        assert_eq!(
            serde_json::to_value(&response).unwrap(),
            json!({
                "epoch": "E",
                "accepted": [{ "table": "t", "id": "A", "seq": 42 }],
                "ignored": [{ "table": "t", "id": "B", "reason": "older" }],
                "rejected": [{ "index": 3, "error": "invalid_updated_at", "message": "m" }],
                "seq": 42
            })
        );
    }

    #[test]
    fn pull_response_usa_has_more_e_row_sem_envelope() {
        let response = PullResponse {
            epoch: "E".into(),
            rows: vec![PulledRow {
                table: "t".into(),
                seq: 41,
                row: json!({ "id": "A", "deletedAt": null }),
            }],
            cursor: 41,
            has_more: false,
        };
        assert_eq!(
            serde_json::to_value(&response).unwrap(),
            json!({
                "epoch": "E",
                "rows": [{ "table": "t", "seq": 41, "row": { "id": "A", "deletedAt": null } }],
                "cursor": 41,
                "hasMore": false
            })
        );
    }

    #[test]
    fn pull_query_tem_cursor_padrao_zero() {
        let query: PullQuery = serde_json::from_value(json!({ "epoch": "E" })).unwrap();
        assert_eq!(query.cursor, 0);
        assert_eq!(query.limit, None);
    }

    #[test]
    fn pair_request_exige_name() {
        let ok = json!({ "token": "ABCDEF", "deviceId": "D", "name": "Pixel" });
        let parsed: PairRequest = serde_json::from_value(ok).unwrap();
        assert_eq!(parsed.device_id, "D");
        assert_eq!(parsed.user_id, None);
        let with_user = json!({ "token": "ABCDEF", "deviceId": "D", "name": "N", "userId": "U" });
        let parsed: PairRequest = serde_json::from_value(with_user).unwrap();
        assert_eq!(parsed.user_id.as_deref(), Some("U"));
        let missing = json!({ "token": "ABCDEF", "deviceId": "D" });
        assert!(serde_json::from_value::<PairRequest>(missing).is_err());
    }

    #[test]
    fn respostas_de_pareamento_e_identidade_em_camel_case() {
        let pair = PairResponse {
            device_id: "D".into(),
            key: "K".into(),
            epoch: "E".into(),
            hub_name: "HubFinance".into(),
        };
        assert_eq!(
            serde_json::to_value(&pair).unwrap(),
            json!({ "deviceId": "D", "key": "K", "epoch": "E", "hubName": "HubFinance" })
        );
        let me = MeResponse {
            device_id: "D".into(),
            name: "N".into(),
            paired_at: "P".into(),
            epoch: "E".into(),
            user_id: None,
        };
        assert_eq!(
            serde_json::to_value(&me).unwrap(),
            json!({ "deviceId": "D", "name": "N", "pairedAt": "P", "epoch": "E", "userId": null })
        );
        let me = MeResponse {
            user_id: Some("U".into()),
            ..me
        };
        assert_eq!(serde_json::to_value(&me).unwrap()["userId"], "U");
    }

    #[test]
    fn error_body_omite_epoch_quando_none() {
        let without = ErrorBody {
            error: "invalid_token".into(),
            message: "m".into(),
            epoch: None,
        };
        assert_eq!(
            serde_json::to_value(&without).unwrap(),
            json!({ "error": "invalid_token", "message": "m" })
        );
        let with = ErrorBody {
            epoch: Some("E".into()),
            ..without
        };
        assert_eq!(serde_json::to_value(&with).unwrap()["epoch"], json!("E"));
    }
}
