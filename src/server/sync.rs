//! `POST /v1/push` e `GET /v1/pull`: o sync propriamente dito.

use axum::Json;
use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{Query, State};

use super::auth::AuthDevice;
use super::errors::ApiError;
use super::state::{AppState, ServerEvent};
use crate::protocol::messages::{
    DEFAULT_PULL_LIMIT, MAX_PULL_LIMIT, MAX_PUSH_ROWS, PullQuery, PullResponse, PulledRow,
    PushRequest, PushResponse,
};
use crate::store::devices::Touch;

/// O cursor do celular so faz sentido no banco em que nasceu: com outro `epoch`, um `seq` igual
/// aponta para outra linha, e o celular pularia dados sem saber.
fn check_epoch(state: &AppState, epoch: &str) -> Result<(), ApiError> {
    let current = state.store.epoch();
    if epoch == current {
        Ok(())
    } else {
        Err(ApiError::epoch_mismatch(current))
    }
}

pub async fn push(
    State(state): State<AppState>,
    AuthDevice(device): AuthDevice,
    body: Result<Json<PushRequest>, JsonRejection>,
) -> Result<Json<PushResponse>, ApiError> {
    let Json(req) = body?;
    check_epoch(&state, &req.epoch)?;
    if req.rows.len() > MAX_PUSH_ROWS {
        return Err(ApiError::too_many_rows());
    }

    let origin = device.device_id.clone();
    let now = state.now_rfc3339();
    let outcome = state
        .blocking(move |store| {
            let outcome = store.apply_batch(&origin, &req.rows)?;
            store.touch_device(&origin, Touch::Push, &now)?;
            Ok(outcome)
        })
        .await?;
    state.emit(ServerEvent::Pushed {
        device_id: device.device_id,
        name: device.name,
        accepted: outcome.accepted.len(),
        ignored: outcome.ignored.len(),
        rejected: outcome.rejected.len(),
    });
    Ok(Json(PushResponse {
        epoch: state.store.epoch().to_string(),
        accepted: outcome.accepted,
        ignored: outcome.ignored,
        rejected: outcome.rejected,
        seq: outcome.max_seq,
    }))
}

pub async fn pull(
    State(state): State<AppState>,
    AuthDevice(device): AuthDevice,
    query: Result<Query<PullQuery>, QueryRejection>,
) -> Result<Json<PullResponse>, ApiError> {
    let Query(query) = query?;
    check_epoch(&state, &query.epoch)?;
    let limit = query.limit.unwrap_or(DEFAULT_PULL_LIMIT);
    if !(1..=MAX_PULL_LIMIT).contains(&limit) {
        return Err(ApiError::invalid_request(
            "limit precisa estar entre 1 e 1000.",
        ));
    }
    if query.cursor < 0 {
        return Err(ApiError::invalid_request("cursor não pode ser negativo."));
    }

    let me = device.device_id.clone();
    let now = state.now_rfc3339();
    let cursor = query.cursor;
    let page = state
        .blocking(move |store| {
            let page = store.pull_after(cursor, &me, limit)?;
            store.touch_device(&me, Touch::Pull, &now)?;
            Ok(page)
        })
        .await?;
    state.emit(ServerEvent::Pulled {
        device_id: device.device_id,
        name: device.name,
        rows: page.rows.len(),
    });
    Ok(Json(PullResponse {
        epoch: state.store.epoch().to_string(),
        rows: page
            .rows
            .into_iter()
            .map(|r| PulledRow {
                table: r.table,
                seq: r.seq,
                row: r.row,
            })
            .collect(),
        cursor: page.cursor,
        has_more: page.has_more,
    }))
}
