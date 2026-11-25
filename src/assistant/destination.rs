//! Para onde o assistente pode falar. Nao existe construtor com URL livre: local e sempre
//! `127.0.0.1:<porta>` e nuvem e sempre `OPENROUTER_BASE_URL`. Uma URL digitada mandaria o
//! envelope para qualquer lugar sem o aviso de nuvem.

use crate::config::{OLLAMA_DEFAULT_PORT, OPENROUTER_BASE_URL};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Destination {
    Local { port: u16 },
    Cloud,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("porta inválida")]
pub struct InvalidPort;

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

/// Modelos do Ollama cuja tag termina em `cloud` (`gpt-oss:120b-cloud`, `foo:cloud`) sao
/// executados em ollama.com: o Ollama local so repassa o pedido. Tratar um deles como "local"
/// mandaria o envelope para fora do PC sem a previa e o consentimento da nuvem, que e justamente
/// o que o destino local promete nao exigir.
pub fn is_ollama_cloud_model(name: &str) -> bool {
    let Some((_, tag)) = name.rsplit_once(':') else {
        return false;
    };
    let tag = tag.to_ascii_lowercase();
    tag == "cloud" || tag.ends_with("-cloud")
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
    fn reconhece_modelos_cloud_do_ollama() {
        for (name, cloud) in [
            ("llama3.1:8b", false),
            ("gpt-oss:120b-cloud", true),
            ("qwen3-coder:480b-cloud", true),
            ("deepseek-v3.1:671b-cloud", true),
            ("foo:cloud", true),
            ("cloudy:7b", false),
            ("mycloud", false),
            ("cloud:7b", false),
        ] {
            assert_eq!(is_ollama_cloud_model(name), cloud, "{name}");
        }
    }

    #[test]
    fn porta_zero_e_recusada() {
        assert!(Destination::try_local(0).is_err());
    }
}
