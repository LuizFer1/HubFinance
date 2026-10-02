//! Autenticacao por `Authorization: Bearer <chave>`.

use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;

use super::errors::ApiError;
use super::state::{AppState, ServerEvent};
use crate::pairing::key::hash_key;
use crate::store::devices::{Device, Touch};

/// Aparelho ativo dono da chave da requisicao.
pub struct AuthDevice(pub Device);

impl FromRequestParts<AppState> for AuthDevice {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let key = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(bearer)
            .ok_or_else(ApiError::unauthorized)?;
        let hash = hash_key(key);
        // Busca sem o filtro de ativo: so assim da para distinguir "revogado" de
        // "desconhecido" e avisar na janela que um aparelho revogado tentou entrar.
        let device = state
            .blocking(move |store| store.device_by_key_hash_any(&hash))
            .await?
            .ok_or_else(ApiError::unauthorized)?;
        if device.revoked_at.is_some() {
            state.emit(ServerEvent::RevokedAttempt { name: device.name });
            return Err(ApiError::unauthorized());
        }
        let id = device.device_id.clone();
        let now = state.now_rfc3339();
        state
            .blocking(move |store| store.touch_device(&id, Touch::Seen, &now))
            .await?;
        Ok(AuthDevice(device))
    }
}

/// O esquema e case-insensitive (RFC 7235); a chave nao.
fn bearer(value: &str) -> Option<&str> {
    let (scheme, key) = value.split_once(' ')?;
    let key = key.trim();
    (scheme.eq_ignore_ascii_case("bearer") && !key.is_empty()).then_some(key)
}

#[cfg(test)]
mod tests {
    use super::bearer;

    #[test]
    fn extrai_a_chave_do_bearer() {
        assert_eq!(bearer("Bearer abc"), Some("abc"));
        assert_eq!(bearer("bearer  abc "), Some("abc"));
        assert_eq!(bearer("Basic abc"), None);
        assert_eq!(bearer("Bearer "), None);
        assert_eq!(bearer("abc"), None);
    }
}
