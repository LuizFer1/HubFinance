//! Estado compartilhado pelos handlers e os eventos que o servidor manda para o nucleo.

use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tokio::sync::mpsc;

use super::errors::ApiError;
use crate::pairing::token::TokenBook;
use crate::store::devices::Device;
use crate::store::{Store, StoreError};

/// Relogio injetado: os testes fixam o tempo e o token nao expira no meio de um teste lento.
pub type Clock = Arc<dyn Fn() -> SystemTime + Send + Sync>;

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<Store>,
    pub tokens: Arc<Mutex<TokenBook>>,
    pub events: mpsc::UnboundedSender<ServerEvent>,
    pub now: Clock,
}

/// O que a janela precisa saber do que aconteceu nas requisicoes. O servidor nao conhece o
/// snapshot: so avisa, e o nucleo decide o que mostrar.
#[derive(Debug, Clone)]
pub enum ServerEvent {
    Paired {
        device: Device,
    },
    Pushed {
        device_id: String,
        name: String,
        accepted: usize,
        ignored: usize,
        rejected: usize,
    },
    Pulled {
        device_id: String,
        name: String,
        rows: usize,
    },
    TokenExhausted,
    RevokedAttempt {
        name: String,
    },
    Failure(String),
}

impl AppState {
    /// Nucleo fechado (receiver caido) nao e erro de requisicao: o evento so se perde.
    pub fn emit(&self, event: ServerEvent) {
        let _ = self.events.send(event);
    }

    pub fn now_rfc3339(&self) -> String {
        rfc3339((self.now)())
    }

    /// A store bloqueia num Mutex e no disco; fora do executor, um lote grande nao trava as
    /// outras requisicoes. Falha vira `500` e entrada de erro na janela.
    pub async fn blocking<T, F>(&self, f: F) -> Result<T, ApiError>
    where
        T: Send + 'static,
        F: FnOnce(&Store) -> Result<T, StoreError> + Send + 'static,
    {
        let store = Arc::clone(&self.store);
        let result = match tokio::task::spawn_blocking(move || f(&store)).await {
            Ok(Ok(value)) => return Ok(value),
            Ok(Err(err)) => err.to_string(),
            Err(join) => format!("tarefa da store falhou: {join}"),
        };
        self.emit(ServerEvent::Failure(result.clone()));
        Err(ApiError::internal(result))
    }
}

/// RFC 3339 em UTC, sem fracao de segundo: e so para exibicao na janela e no `/v1/me`.
pub fn rfc3339(t: SystemTime) -> String {
    let dt = OffsetDateTime::from(t);
    let dt = dt.replace_nanosecond(0).unwrap_or(dt);
    dt.format(&Rfc3339).unwrap_or_default()
}
