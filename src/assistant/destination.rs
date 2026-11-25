//! Para onde o assistente pode falar. Nao existe construtor com URL livre: local e sempre
//! `127.0.0.1:<porta>` e nuvem e sempre `OPENROUTER_BASE_URL`. Uma URL digitada mandaria o
//! envelope para qualquer lugar sem o aviso de nuvem.

use crate::config::{OLLAMA_DEFAULT_PORT, OPENROUTER_BASE_URL};

#[allow(dead_code)] // usado pelo cliente (Task 4.4)
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Destination {
    Local { port: u16 },
    Cloud,
}

#[allow(dead_code)] // usado pelas configuracoes (Task 4.5)
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("porta inválida")]
pub struct InvalidPort;

#[allow(dead_code)] // usado pelo cliente (Task 4.4)
impl Destination {
    pub fn try_local(port: u16) -> Result<Self, InvalidPort> {
        if port == 0 {
            Err(InvalidPort)
        } else {
            Ok(Destination::Local { port })
        }
    }

    /// Porta invalida cai na padrao do Ollama em vez de abrir um destino que nao existe.
    pub fn local(port: u16) -> Self {
        Destination::try_local(port).unwrap_or(Destination::Local {
            port: OLLAMA_DEFAULT_PORT,
        })
    }

    pub fn cloud() -> Self {
        Destination::Cloud
    }

    pub fn is_cloud(&self) -> bool {
        matches!(self, Destination::Cloud)
    }

    pub fn base_url(&self) -> String {
        match self {
            Destination::Local { port } => format!("http://127.0.0.1:{port}/v1"),
            Destination::Cloud => OPENROUTER_BASE_URL.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_e_sempre_loopback() {
        let d = Destination::local(11434);
        assert_eq!(d.base_url(), "http://127.0.0.1:11434/v1");
        assert!(!d.is_cloud());
    }

    #[test]
    fn nuvem_usa_a_constante() {
        let d = Destination::cloud();
        assert_eq!(d.base_url(), crate::config::OPENROUTER_BASE_URL);
        assert!(d.is_cloud());
    }

    #[test]
    fn porta_zero_e_recusada() {
        assert!(Destination::try_local(0).is_err());
    }
}
