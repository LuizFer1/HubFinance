//! Configuracao do assistente, fora do SQLite como o `ui.json` (o banco de sync so guarda sync).
//! Ausente ou ilegivel = padrao: nuvem desligada, retencao negada.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::destination::Destination;
use crate::config::OLLAMA_DEFAULT_PORT;

pub const SETTINGS_FILE: &str = "assistant.json";
/// Sugestao do campo de modelo da nuvem; o usuario troca a vontade (id do OpenRouter).
pub const SUGGESTED_CLOUD_MODEL: &str = "anthropic/claude-sonnet-4.5";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderChoice {
    #[default]
    None,
    Local,
    Cloud,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LocalSettings {
    pub port: u16,
    pub model: Option<String>,
}

impl Default for LocalSettings {
    fn default() -> Self {
        LocalSettings {
            port: OLLAMA_DEFAULT_PORT,
            model: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CloudSettings {
    /// O interruptor da emenda: so com ele ligado algo sai da maquina.
    pub enabled: bool,
    pub model: String,
    pub deny_data_collection: bool,
}

impl Default for CloudSettings {
    fn default() -> Self {
        CloudSettings {
            enabled: false,
            model: SUGGESTED_CLOUD_MODEL.into(),
            deny_data_collection: true,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub provider: ProviderChoice,
    #[serde(default)]
    pub local: LocalSettings,
    #[serde(default)]
    pub cloud: CloudSettings,
}

#[allow(dead_code)] // usado pelo servico (Task 4.6)
impl Settings {
    pub fn load(dir: &Path) -> Settings {
        std::fs::read(dir.join(SETTINGS_FILE))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        crate::tls::files::write_atomic(&dir.join(SETTINGS_FILE), &json)
    }

    /// `None` = sem assistente utilizavel (nenhum, ou nuvem escolhida com o interruptor
    /// desligado).
    pub fn destination(&self) -> Option<Destination> {
        match self.provider {
            ProviderChoice::None => None,
            ProviderChoice::Local => Some(Destination::local(self.local.port)),
            ProviderChoice::Cloud => self.cloud.enabled.then(Destination::cloud),
        }
    }

    pub fn model(&self) -> Option<String> {
        match self.provider {
            ProviderChoice::None => None,
            ProviderChoice::Local => self.local.model.clone(),
            ProviderChoice::Cloud => {
                Some(self.cloud.model.trim().to_string()).filter(|m| !m.is_empty())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ausente_vale_o_padrao_com_nuvem_desligada_e_deny_ligado() {
        let dir = tempfile::tempdir().unwrap();
        let s = Settings::load(dir.path());
        assert_eq!(s.provider, ProviderChoice::None);
        assert!(!s.cloud.enabled);
        assert!(s.cloud.deny_data_collection);
        assert_eq!(s.local.port, crate::config::OLLAMA_DEFAULT_PORT);
    }

    #[test]
    fn salva_e_le_de_volta() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Settings {
            provider: ProviderChoice::Local,
            ..Settings::default()
        };
        s.local.model = Some("llama3.1:8b".into());
        s.save(dir.path()).unwrap();
        assert_eq!(Settings::load(dir.path()), s);
    }

    #[test]
    fn nuvem_escolhida_mas_desligada_nao_tem_destino() {
        let mut s = Settings {
            provider: ProviderChoice::Cloud,
            ..Settings::default()
        };
        assert_eq!(s.destination(), None);
        s.cloud.enabled = true;
        assert_eq!(s.destination(), Some(Destination::cloud()));
    }

    #[test]
    fn arquivo_ilegivel_vale_padrao() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(SETTINGS_FILE), "{lixo").unwrap();
        assert_eq!(Settings::load(dir.path()), Settings::default());
    }

    #[test]
    fn campo_ausente_no_arquivo_vale_o_padrao_do_campo() {
        // Arquivo de versao antiga (ou editado a mao) sem `cloud`: a nuvem continua desligada.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(SETTINGS_FILE),
            r#"{"provider":"local","local":{"port":11500,"model":null}}"#,
        )
        .unwrap();
        let s = Settings::load(dir.path());
        assert_eq!(s.provider, ProviderChoice::Local);
        assert_eq!(s.local.port, 11500);
        assert!(!s.cloud.enabled);
        assert!(s.cloud.deny_data_collection);
    }

    #[test]
    fn modelo_em_branco_da_nuvem_nao_conta() {
        let mut s = Settings {
            provider: ProviderChoice::Cloud,
            ..Settings::default()
        };
        s.cloud.model = "   ".into();
        assert_eq!(s.model(), None);
        s.cloud.model = " openai/gpt-4o ".into();
        assert_eq!(s.model().as_deref(), Some("openai/gpt-4o"));
    }
}
