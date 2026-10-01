use std::path::PathBuf;

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
