//! Erros da API no formato do contrato: `{ error, message, epoch? }`.

use axum::Json;
use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::protocol::messages::ErrorBody;
use crate::store::StoreError;

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
    pub epoch: Option<String>,
}

impl ApiError {
    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        ApiError {
            status,
            code,
            message: message.into(),
            epoch: None,
        }
    }

    pub fn invalid_request(msg: impl Into<String>) -> Self {
        ApiError::new(StatusCode::BAD_REQUEST, "invalid_request", msg)
    }

    pub fn too_many_rows() -> Self {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "too_many_rows",
            "Mais de 1000 linhas num envio.",
        )
    }

    pub fn invalid_token() -> Self {
        ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid_token",
            "Código inválido ou expirado.",
        )
    }

    pub fn unauthorized() -> Self {
        ApiError::new(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Aparelho não pareado ou revogado. Pareie de novo.",
        )
    }

    /// O corpo leva o `epoch` atual: o celular zera o cursor e refaz o sync sem outra chamada.
    pub fn epoch_mismatch(current: &str) -> Self {
        let mut err = ApiError::new(
            StatusCode::CONFLICT,
            "epoch_mismatch",
            "O hub foi recriado; sincronizando tudo de novo.",
        );
        err.epoch = Some(current.to_string());
        err
    }

    pub fn payload_too_large() -> Self {
        ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "payload_too_large",
            "Envio maior que 8 MiB.",
        )
    }

    /// O detalhe fica so no log: caminho de arquivo e erro de SQLite nao sao para a tela do
    /// celular.
    pub fn internal(err: impl std::fmt::Display) -> Self {
        tracing::error!("erro interno: {err}");
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            "Erro interno do hub.",
        )
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ErrorBody {
            error: self.code.to_string(),
            message: self.message,
            epoch: self.epoch,
        };
        (self.status, Json(body)).into_response()
    }
}

impl From<StoreError> for ApiError {
    fn from(err: StoreError) -> Self {
        ApiError::internal(err)
    }
}

/// Todo JSON ruim vira o erro da spec, e nao o 415/422 em texto puro do axum, que o celular
/// nao saberia ler.
impl From<JsonRejection> for ApiError {
    fn from(rejection: JsonRejection) -> Self {
        if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE {
            ApiError::payload_too_large()
        } else {
            ApiError::invalid_request(format!("JSON inválido: {}", rejection.body_text()))
        }
    }
}

impl From<QueryRejection> for ApiError {
    fn from(rejection: QueryRejection) -> Self {
        ApiError::invalid_request(format!("Parâmetros inválidos: {}", rejection.body_text()))
    }
}
