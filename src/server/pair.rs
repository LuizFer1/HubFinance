//! `POST /v1/pair`: troca o token de uso unico por uma chave de longa duracao.

use std::sync::PoisonError;

use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;

use super::errors::ApiError;
use super::state::{AppState, ServerEvent};
use crate::config::HUB_NAME;
use crate::pairing::key::{generate_key, hash_key};
use crate::pairing::token::TokenError;
use crate::protocol::ids::is_valid_id;
use crate::protocol::messages::{PairRequest, PairResponse};

pub const MAX_NAME_CHARS: usize = 64;

pub async fn pair(
    State(state): State<AppState>,
    body: Result<Json<PairRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<PairResponse>), ApiError> {
    let Json(req) = body?;
    // Pedido malformado e validado antes do token: nao deve gastar uma das cinco tentativas.
    if !is_valid_id(&req.device_id) {
        return Err(ApiError::invalid_request(
            "deviceId precisa ter 26 caracteres Crockford.",
        ));
    }
    let name = req.name.trim().to_string();
    let name_len = name.chars().count();
    if name_len == 0 || name_len > MAX_NAME_CHARS {
        return Err(ApiError::invalid_request(
            "O nome do aparelho precisa ter de 1 a 64 caracteres.",
        ));
    }

    let consumed = state
        .tokens
        .lock()
        // Envenenado so se alguem entrou em panico com o lock; o `TokenBook` continua
        // coerente, e recusar aqui travaria o pareamento ate reiniciar o hub.
        .unwrap_or_else(PoisonError::into_inner)
        .consume(&req.token, (state.now)());
    match consumed {
        Ok(()) => {}
        Err(TokenError::Exhausted) => {
            state.emit(ServerEvent::TokenExhausted);
            return Err(ApiError::invalid_token());
        }
        Err(_) => return Err(ApiError::invalid_token()),
    }

    // A chave sai daqui uma vez e nunca e gravada; o banco so conhece o hash.
    let key = generate_key();
    let hash = hash_key(&key);
    let now = state.now_rfc3339();
    let device_id = req.device_id.clone();
    let device = state
        .blocking(move |store| store.upsert_device(&device_id, &name, &hash, &now))
        .await?;
    state.emit(ServerEvent::Paired {
        device: device.clone(),
    });
    Ok((
        StatusCode::CREATED,
        Json(PairResponse {
            device_id: device.device_id,
            key,
            epoch: state.store.epoch().to_string(),
            hub_name: HUB_NAME.to_string(),
        }),
    ))
}
