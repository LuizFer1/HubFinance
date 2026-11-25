//! Assistente de analise (spec 2026-11-25). `envelope`, `prompt` e `sse` sao puros; `client`
//! e o unico lugar com `ureq`; `history` o unico com rusqlite fora de `store/`, e so abre o
//! `assistant.db`.

pub mod destination;
pub mod envelope;
#[cfg(test)]
pub mod fake;
pub mod prompt;
pub mod provider;
pub mod sse;

/// Cada falha vira uma mensagem em pt-BR na conversa, com "Tentar de novo" (spec 6). Nenhuma
/// derruba a janela nem o nucleo.
#[allow(dead_code)] // usado pelo cliente (Task 4.4)
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AssistantError {
    #[error("Não encontrei o Ollama em 127.0.0.1:{0}. Ele está aberto?")]
    LocalUnreachable(u16),
    #[error("O modelo \"{0}\" não está instalado no Ollama.")]
    ModelMissing(String),
    #[error("A chave do OpenRouter foi recusada. Confira em Configurar assistente.")]
    BadKey,
    #[error("Sem saldo ou limite atingido no OpenRouter.")]
    NoCredit,
    #[error("Sem conexão com a internet.")]
    Offline,
    #[error("Nenhum provedor deste modelo aceita a política de não reter dados.")]
    NoProviderForPolicy,
    #[error("O modelo demorou demais para responder.")]
    Timeout,
    #[error("Resposta fora do formato esperado.")]
    Malformed,
    #[error("Falta configurar a chave do OpenRouter.")]
    MissingKey,
    #[error("{0}")]
    Other(String),
}
