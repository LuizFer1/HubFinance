//! Assistente de analise (spec 2026-11-25). `envelope`, `prompt` e `sse` sao puros; `client`
//! e o unico lugar com `ureq`; `history` o unico com rusqlite fora de `store/`, e so abre o
//! `assistant.db`.

pub mod envelope;
pub mod prompt;
