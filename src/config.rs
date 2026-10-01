use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const HTTPS_PORT: u16 = 7777;
pub const HTTP_PORT: u16 = 7778;
pub const HUB_NAME: &str = "HubFinance";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const PROTOCOL: u32 = 1;
/// Origens que podem chamar a API. A de dev entra sempre: o Bearer é o que autoriza.
pub const ALLOWED_ORIGINS: [&str; 2] = ["https://luizfer1.github.io", "http://localhost:5173"];
pub const PWA_URL: &str = "https://luizfer1.github.io/homefinance/";

#[derive(Clone, Debug)]
pub struct Config {
    pub data_dir: PathBuf,
    pub https_port: u16,
    pub http_port: u16,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("nao foi possivel determinar o diretorio de dados do usuario")]
    NoDataDir,
}

impl Config {
    /// `HUBFINANCE_DATA_DIR` sobrescreve o diretorio (dev e teste ponta a ponta com dois hubs).
    pub fn from_env() -> Result<Config, ConfigError> {
        let data_dir = match std::env::var_os("HUBFINANCE_DATA_DIR") {
            Some(dir) => PathBuf::from(dir),
            None => directories::ProjectDirs::from("", "", "HubFinance")
                .ok_or(ConfigError::NoDataDir)?
                .data_local_dir()
                .to_path_buf(),
        };
        Ok(Config {
            data_dir,
            https_port: HTTPS_PORT,
            http_port: HTTP_PORT,
        })
    }

    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("hub.sqlite")
    }

    pub fn tls_dir(&self) -> PathBuf {
        self.data_dir.join("tls")
    }
}

/// Tema da janela. Escuro e o padrao do design.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
}

/// Preferencias da janela, em `<data_dir>/ui.json`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiPrefs {
    #[serde(default)]
    pub theme: ThemeMode,
}

const UI_PREFS_FILE: &str = "ui.json";

impl UiPrefs {
    /// Fora do SQLite de proposito: "o dashboard nunca escreve no banco" tem que ser literal.
    /// Arquivo ausente ou ilegivel vale o padrao: preferencia de tela nunca impede o hub de
    /// abrir.
    pub fn load(data_dir: &Path) -> UiPrefs {
        std::fs::read(data_dir.join(UI_PREFS_FILE))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    // A janela passa a gravar ao trocar o tema na casca (tarefa 6 do plano 2a).
    #[allow(dead_code)]
    pub fn save(&self, data_dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(data_dir)?;
        let json = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(data_dir.join(UI_PREFS_FILE), json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sem_arquivo_vale_o_padrao_escuro() {
        let dir = tempfile::tempdir().unwrap();
        let prefs = UiPrefs::load(dir.path());
        assert_eq!(prefs, UiPrefs::default());
        assert_eq!(prefs.theme, ThemeMode::Dark);
    }

    #[test]
    fn grava_e_le_de_volta() {
        let dir = tempfile::tempdir().unwrap();
        let prefs = UiPrefs {
            theme: ThemeMode::Light,
        };
        prefs.save(dir.path()).unwrap();
        assert_eq!(UiPrefs::load(dir.path()), prefs);
        let raw = std::fs::read_to_string(dir.path().join("ui.json")).unwrap();
        assert!(raw.contains("\"light\""), "{raw}");
    }

    #[test]
    fn arquivo_com_lixo_vale_o_padrao() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("ui.json"), "{nao e json").unwrap();
        assert_eq!(UiPrefs::load(dir.path()), UiPrefs::default());
        std::fs::write(dir.path().join("ui.json"), r#"{"theme":"sepia"}"#).unwrap();
        assert_eq!(UiPrefs::load(dir.path()), UiPrefs::default());
    }

    #[test]
    fn campo_ausente_vale_o_padrao() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("ui.json"), "{}").unwrap();
        assert_eq!(UiPrefs::load(dir.path()), UiPrefs::default());
    }
}
