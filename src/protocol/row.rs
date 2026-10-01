//! Parse e validacao de uma linha recebida num push.

use serde_json::{Map, Value};

use super::hlc::is_valid_hlc;
use super::ids::{is_valid_id, is_valid_table};

pub const MAX_ROW_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidRow {
    pub table: String,
    pub id: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
    /// Objeto inteiro, ja sem `dirty`. Serializado e o que vai para `rows.data`.
    pub data: Map<String, Value>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RowError {
    #[error("nome de tabela invalido")]
    InvalidTable,
    #[error("row nao e um objeto JSON")]
    RowNotObject,
    #[error("linha maior que 64 KiB")]
    RowTooLarge,
    #[error("id invalido: precisa ter 26 caracteres Crockford")]
    InvalidId,
    #[error("updatedAt nao e um HLC valido")]
    InvalidUpdatedAt,
    #[error("deletedAt precisa ser null ou um HLC valido")]
    InvalidDeletedAt,
}

impl RowError {
    /// Codigo estavel do contrato; o celular decide por ele, nunca pela mensagem.
    pub fn code(self) -> &'static str {
        match self {
            RowError::InvalidTable => "invalid_table",
            RowError::RowNotObject => "row_not_object",
            RowError::RowTooLarge => "row_too_large",
            RowError::InvalidId => "invalid_id",
            RowError::InvalidUpdatedAt => "invalid_updated_at",
            RowError::InvalidDeletedAt => "invalid_deleted_at",
        }
    }

    pub fn message(self) -> String {
        self.to_string()
    }
}

/// `dirty` e removido aqui, e nao na store: e estado local do aparelho, e se o hub o
/// devolvesse o outro celular reenviaria uma linha que o hub ja tem.
pub fn parse_row(table: &str, row: &Value) -> Result<ValidRow, RowError> {
    if !is_valid_table(table) {
        return Err(RowError::InvalidTable);
    }
    let mut data = row.as_object().ok_or(RowError::RowNotObject)?.clone();
    data.remove("dirty");

    let id = match data.get("id") {
        Some(Value::String(id)) if is_valid_id(id) => id.clone(),
        _ => return Err(RowError::InvalidId),
    };
    let updated_at = match data.get("updatedAt") {
        Some(Value::String(hlc)) if is_valid_hlc(hlc) => hlc.clone(),
        _ => return Err(RowError::InvalidUpdatedAt),
    };
    // Ausente nao e o mesmo que `null`: o app sempre grava a coluna, entao uma linha sem ela
    // veio de outro formato e nao deve ser tratada como viva por omissao.
    let deleted_at = match data.get("deletedAt") {
        Some(Value::Null) => None,
        Some(Value::String(hlc)) if is_valid_hlc(hlc) => Some(hlc.clone()),
        _ => return Err(RowError::InvalidDeletedAt),
    };

    // Medido sobre o que sera guardado: e o tamanho no banco e no pull que o limite protege.
    let size = serde_json::to_string(&data).map_or(usize::MAX, |json| json.len());
    if size > MAX_ROW_BYTES {
        return Err(RowError::RowTooLarge);
    }

    Ok(ValidRow {
        table: table.to_string(),
        id,
        updated_at,
        deleted_at,
        data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const ID: &str = "01HZZZZZZZZZZZZZZZZZZZZZC1";
    const T1: &str = "1759344000000-0000-01HZZZZZZZZZZZZZZZZZZZZZZA";
    const T2: &str = "1759344000001-0000-01HZZZZZZZZZZZZZZZZZZZZZZA";

    fn valid() -> Value {
        json!({
            "id": ID,
            "createdAt": "2026-10-01T18:00:00.000Z",
            "updatedAt": T1,
            "deletedAt": null,
            "dirty": 1,
            "name": "Mercado",
        })
    }

    fn with(field: &str, value: Value) -> Value {
        let mut row = valid();
        row[field] = value;
        row
    }

    fn without(field: &str) -> Value {
        let mut row = valid();
        row.as_object_mut().map(|o| o.remove(field));
        row
    }

    #[test]
    fn linha_valida_perde_dirty_e_expoe_chaves() {
        let parsed = parse_row("categories", &valid()).unwrap();
        assert_eq!(parsed.table, "categories");
        assert_eq!(parsed.id, ID);
        assert_eq!(parsed.updated_at, T1);
        assert_eq!(parsed.deleted_at, None);
        assert!(!parsed.data.contains_key("dirty"));
        assert_eq!(parsed.data["name"], json!("Mercado"));
        assert_eq!(parsed.data["deletedAt"], Value::Null);
    }

    #[test]
    fn deleted_at_hlc_vira_some() {
        let parsed = parse_row("categories", &with("deletedAt", json!(T2))).unwrap();
        assert_eq!(parsed.deleted_at.as_deref(), Some(T2));
    }

    #[test]
    fn deleted_at_ausente_ou_invalido_e_rejeitado() {
        let err = parse_row("categories", &without("deletedAt"));
        assert_eq!(err, Err(RowError::InvalidDeletedAt));
        let err = parse_row("categories", &with("deletedAt", json!("x")));
        assert_eq!(err, Err(RowError::InvalidDeletedAt));
        let err = parse_row("categories", &with("deletedAt", json!(0)));
        assert_eq!(err, Err(RowError::InvalidDeletedAt));
    }

    #[test]
    fn row_que_nao_e_objeto_e_rejeitada() {
        for row in [json!([1, 2]), json!("linha"), Value::Null] {
            assert_eq!(parse_row("categories", &row), Err(RowError::RowNotObject));
        }
    }

    #[test]
    fn id_ausente_ou_invalido_e_rejeitado() {
        assert_eq!(
            parse_row("categories", &without("id")),
            Err(RowError::InvalidId)
        );
        let err = parse_row("categories", &with("id", json!("abc")));
        assert_eq!(err, Err(RowError::InvalidId));
        let err = parse_row("categories", &with("id", json!(42)));
        assert_eq!(err, Err(RowError::InvalidId));
    }

    #[test]
    fn updated_at_ausente_ou_invalido_e_rejeitado() {
        let err = parse_row("categories", &without("updatedAt"));
        assert_eq!(err, Err(RowError::InvalidUpdatedAt));
        let err = parse_row("categories", &with("updatedAt", json!("x")));
        assert_eq!(err, Err(RowError::InvalidUpdatedAt));
    }

    #[test]
    fn tabela_invalida_e_checada_antes_do_resto() {
        assert_eq!(parse_row("1abc", &Value::Null), Err(RowError::InvalidTable));
        assert_eq!(parse_row("", &valid()), Err(RowError::InvalidTable));
    }

    #[test]
    fn linha_grande_demais_e_rejeitada() {
        let err = parse_row("categories", &with("note", json!("x".repeat(70_000))));
        assert_eq!(err, Err(RowError::RowTooLarge));
    }

    #[test]
    fn limite_de_tamanho_e_medido_sem_dirty() {
        // Linha que so passa do limite por causa do `dirty`: o que importa e o que se guarda.
        let base = serde_json::to_string(&without("dirty")).unwrap().len();
        let filler = MAX_ROW_BYTES - base - r#","note":"""#.len();
        let mut row = with("note", json!("x".repeat(filler)));
        row["dirty"] = json!("y".repeat(100));
        let parsed = parse_row("categories", &row).unwrap();
        assert_eq!(
            serde_json::to_string(&parsed.data).unwrap().len(),
            MAX_ROW_BYTES
        );
    }

    #[test]
    fn codigos_e_mensagens_seguem_a_spec() {
        let cases = [
            (RowError::InvalidTable, "invalid_table"),
            (RowError::RowNotObject, "row_not_object"),
            (RowError::RowTooLarge, "row_too_large"),
            (RowError::InvalidId, "invalid_id"),
            (RowError::InvalidUpdatedAt, "invalid_updated_at"),
            (RowError::InvalidDeletedAt, "invalid_deleted_at"),
        ];
        for (err, code) in cases {
            assert_eq!(err.code(), code);
            assert!(!err.message().is_empty());
        }
    }
}
