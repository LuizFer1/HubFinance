//! `GET /v1/info` (publico) e `GET /v1/me` (Bearer).

use axum::Json;
use axum::extract::State;

use super::auth::AuthDevice;
use super::state::AppState;
use crate::config::{HUB_NAME, PROTOCOL, VERSION};
use crate::protocol::messages::{InfoResponse, MeResponse};

/// Publico de proposito: o celular testa alcance e confianca no TLS antes de parear.
pub async fn info(State(state): State<AppState>) -> Json<InfoResponse> {
    Json(InfoResponse {
        name: HUB_NAME.to_string(),
        version: VERSION.to_string(),
        protocol: PROTOCOL,
        epoch: state.store.epoch().to_string(),
    })
}

/// O celular pergunta "ainda estou pareado?"; revogado ja caiu no `401` do extrator.
pub async fn me(State(state): State<AppState>, AuthDevice(device): AuthDevice) -> Json<MeResponse> {
    Json(MeResponse {
        device_id: device.device_id,
        name: device.name,
        paired_at: device.paired_at,
        epoch: state.store.epoch().to_string(),
    })
}
