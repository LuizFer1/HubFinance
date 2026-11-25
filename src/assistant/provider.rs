//! Contrato entre o servico e quem fala com o modelo. O real e `client::OpenAiCompatible`; os
//! testes usam `fake::FakeProvider`, sem rede.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::AssistantError;
use super::prompt::Turn;

#[allow(dead_code)] // usado pelo cliente (Task 4.4)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Turn>,
    pub max_tokens: u32,
}

#[allow(dead_code)] // usado pelo cliente (Task 4.4)
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChatOutcome {
    pub text: String,
    pub tokens_in: Option<u32>,
    pub tokens_out: Option<u32>,
    pub interrupted: bool,
}

/// "Parar": checado entre pedacos do stream.
#[allow(dead_code)] // usado pelo cliente (Task 4.4)
#[derive(Clone, Default)]
pub struct CancelFlag(Arc<AtomicBool>);

#[allow(dead_code)] // usado pelo cliente (Task 4.4)
impl CancelFlag {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

#[allow(dead_code)] // usado pelo cliente (Task 4.4)
pub trait Provider: Send + Sync {
    fn list_models(&self) -> Result<Vec<String>, AssistantError>;
    /// Chama `on_delta` a cada pedaco. Cancelado: devolve `Ok` com `interrupted = true` e o
    /// texto ate ali.
    fn chat(
        &self,
        req: &ChatRequest,
        on_delta: &mut dyn FnMut(&str),
        cancel: &CancelFlag,
    ) -> Result<ChatOutcome, AssistantError>;
}
