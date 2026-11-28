use std::ffi::OsStr;
use std::net::{IpAddr, Ipv4Addr};
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
/// Fixa no binario: o hub nunca recebe URL de fora (celular, arquivo) para abrir. No Windows o
/// `open` passa a URL por `cmd /c start`, que interpreta `&` e `^`: uma URL de fora seria uma
/// injecao. Quando a 0.1.0 montar `.../releases/tag/v<versao>`, a versao tera passado antes
/// pelo parser estrito (tres inteiros) — mantenha isso para quem mexer aqui.
pub const RELEASES_URL: &str = "https://github.com/LuizFer1/HubFinance/releases";
/// Unico destino de nuvem do assistente (emenda da regra da LAN). Fixo e nao editavel: um campo
/// livre transformaria "local" em nuvem disfarcada, sem aviso nem previa.
pub const OPENROUTER_BASE_URL: &str = "https://openrouter.ai/api/v1";
/// Porta padrao do Ollama.
pub const OLLAMA_DEFAULT_PORT: u16 = 11434;
/// Subpasta de `data_dir` com os `hub.AAAA-MM-DD.log` (ver `logging.rs`).
pub const LOGS_DIR: &str = "logs";

/// Funcao livre, e nao so metodo: a UI so tem o `data_dir` do `Snapshot` e precisa mostrar a
/// pasta de logs sem conhecer `Config`.
pub fn logs_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(LOGS_DIR)
}

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
        let port = port_override(
            std::env::var_os("HUBFINANCE_PORT").as_deref(),
            cfg!(debug_assertions),
        );
        Ok(Config {
            data_dir,
            https_port: port.unwrap_or(HTTPS_PORT),
            http_port: port.map_or(HTTP_PORT, |p| p + 1),
        })
    }

    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("hub.sqlite")
    }

    pub fn tls_dir(&self) -> PathBuf {
        self.data_dir.join("tls")
    }
}

/// `HUBFINANCE_PORT=<n>` troca a porta HTTPS para `n` e a HTTP para `n+1`, so em build debug.
/// O hub instalado ocupa 7777/7778 nesta maquina: sem isso o hub de desenvolvimento ao lado
/// cairia em `Failed`. Em release e ignorada, para uma variavel esquecida no ambiente nunca
/// mudar a porta que os celulares pareados conhecem.
pub fn port_override(value: Option<&OsStr>, debug: bool) -> Option<u16> {
    if !debug {
        return None;
    }
    let n: u16 = value?.to_str()?.trim().parse().ok()?;
    // Sem espaco para o `n+1` da porta HTTP.
    n.checked_add(1)?;
    Some(n)
}

/// `HUBFINANCE_BIND=<ip>`: endereco de escuta, so em build debug. O padrao de producao e
/// `0.0.0.0` (LAN); o smoke usa `127.0.0.1` porque escutar em 0.0.0.0 dispara o alerta do
/// Firewall do Windows, que ninguem clica numa execucao noturna. Valor invalido cai no padrao.
pub fn bind_ip(value: Option<&OsStr>, debug: bool) -> IpAddr {
    let default = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
    if !debug {
        return default;
    }
    value
        .and_then(OsStr::to_str)
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}

/// Endereco de escuta efetivo (le `HUBFINANCE_BIND`).
pub fn bind_ip_from_env() -> IpAddr {
    bind_ip(
        std::env::var_os("HUBFINANCE_BIND").as_deref(),
        cfg!(debug_assertions),
    )
}

/// Tema da janela, em memoria. Escuro e o padrao do design.
///
/// `System` e so a preferencia: quem decide escuro ou claro e o sistema, lido pela UI. No
/// `ui.json` ele nao vira um terceiro valor de `theme` (ver `PrefsFile`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
    System,
}

/// O `theme` como fica no arquivo: so os dois valores que todo hub lancado conhece.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum StoredTheme {
    #[default]
    Dark,
    Light,
}

impl ThemeMode {
    /// Escuro de fato, com o que o sistema diz agora (`system_dark`). So `System` consulta.
    pub fn is_dark(self, system_dark: bool) -> bool {
        match self {
            ThemeMode::Dark => true,
            ThemeMode::Light => false,
            ThemeMode::System => system_dark,
        }
    }
}

/// Preferencias da janela, em `<data_dir>/ui.json`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "PrefsFile", into = "PrefsFile")]
pub struct UiPrefs {
    pub theme: ThemeMode,
    /// Com `ThemeMode::System`, o que o sistema dizia na ultima gravacao (`true` = escuro). E o
    /// `theme` escrito no arquivo e o palpite do boot quando o sistema nao responde. Fora do
    /// `System` fica `false` e nao pesa.
    pub system_dark: bool,
    pub autostart: Option<bool>,
    pub hide_values: bool,
    /// "Reduzir movimento" de Ajustes: desliga as tres animacoes da janela (spec 11).
    pub reduce_motion: bool,
}

/// O `ui.json` como esta no disco.
///
/// "Sistema" e `follow_system: true` ao lado de um `theme` resolvido ("dark"/"light"), e nao
/// `"theme": "system"`: um hub 1.4.0 so conhece os dois valores e, com um terceiro, o `load`
/// dele trataria o arquivo inteiro como ilegivel e perderia `autostart` e `hide_values`. Assim
/// ele le o tema da ultima vez e ignora o campo que nao conhece.
#[derive(Serialize, Deserialize)]
struct PrefsFile {
    #[serde(default)]
    theme: StoredTheme,
    /// Ausente (falso) nao vai para o arquivo, como os campos abaixo.
    #[serde(default, skip_serializing_if = "is_false")]
    follow_system: bool,
    /// "Iniciar com o Windows": `None` = nunca decidiu (e o que liga por padrao, uma vez);
    /// `Some(false)` = a pessoa desligou de proposito e o boot nao religa. O estado real da
    /// opcao e o registro; isto e so a memoria da decisao. Ausente nao vai para o arquivo,
    /// para o `ui.json` da 1.0.0 continuar identico ate alguem mexer na opcao.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    autostart: Option<bool>,
    /// "Ocultar valores" de todas as telas de dados (spec 7.4): local a este PC, nunca
    /// sincronizado, como o tema. Desligado nao vai para o arquivo, pelo mesmo motivo do
    /// `autostart`: o `ui.json` de quem nunca usou o olho fica igual ao de antes.
    #[serde(default, skip_serializing_if = "is_false")]
    hide_values: bool,
    /// "Reduzir movimento": desligado nao vai para o arquivo, e um hub antigo ignora o campo.
    #[serde(default, skip_serializing_if = "is_false")]
    reduce_motion: bool,
}

impl From<PrefsFile> for UiPrefs {
    fn from(file: PrefsFile) -> UiPrefs {
        let (theme, system_dark) = match (file.follow_system, file.theme) {
            (true, stored) => (ThemeMode::System, stored == StoredTheme::Dark),
            (false, StoredTheme::Dark) => (ThemeMode::Dark, false),
            (false, StoredTheme::Light) => (ThemeMode::Light, false),
        };
        UiPrefs {
            theme,
            system_dark,
            autostart: file.autostart,
            hide_values: file.hide_values,
            reduce_motion: file.reduce_motion,
        }
    }
}

impl From<UiPrefs> for PrefsFile {
    fn from(prefs: UiPrefs) -> PrefsFile {
        let stored = if prefs.theme.is_dark(prefs.system_dark) {
            StoredTheme::Dark
        } else {
            StoredTheme::Light
        };
        PrefsFile {
            theme: stored,
            follow_system: prefs.theme == ThemeMode::System,
            autostart: prefs.autostart,
            hide_values: prefs.hide_values,
            reduce_motion: prefs.reduce_motion,
        }
    }
}

fn is_false(value: &bool) -> bool {
    !*value
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

    /// Atomico: o tema e gravado a cada troca, e uma queda no meio de `fs::write` deixaria um
    /// `ui.json` truncado (que o `load` trata como padrao, perdendo a escolha).
    pub fn save(&self, data_dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(data_dir)?;
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        crate::tls::files::write_atomic(&data_dir.join(UI_PREFS_FILE), &json)
    }
}

/// Grava o `ui.json` fora da thread da UI (disco lento nao trava a janela). Falha so vira
/// aviso: a preferencia e conforto, o tema ja trocou na tela. Compartilhada pelos dois
/// toolkits para a ordem das gravacoes valer em qualquer um.
///
/// Cada gravacao leva um numero tirado na thread da UI, na ordem dos cliques, e so escreve se
/// for mais nova que a ultima escrita naquela pasta: duas threads nao tem ordem garantida, e
/// sem isso trocar o tema logo depois de "Iniciar com o Windows" (ou o tema duas vezes) podia
/// terminar com o arquivo da escolha anterior.
pub fn save_prefs_ordered(prefs: UiPrefs, dir: PathBuf) -> futures::channel::oneshot::Receiver<()> {
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Mutex, OnceLock};

    static NEXT: AtomicU64 = AtomicU64::new(1);
    static WRITTEN: OnceLock<Mutex<HashMap<PathBuf, u64>>> = OnceLock::new();

    let generation = NEXT.fetch_add(1, Ordering::Relaxed);
    let (tx, rx) = futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        let written = WRITTEN.get_or_init(Default::default);
        // Envenenado = outra gravacao entrou em panico; a tabela continua coerente.
        let mut written = written.lock().unwrap_or_else(|e| e.into_inner());
        let last = written.entry(dir.clone()).or_insert(0);
        if generation > *last {
            if let Err(e) = prefs.save(&dir) {
                tracing::warn!("nao foi possivel gravar ui.json em {}: {e}", dir.display());
            }
            *last = generation;
        }
        drop(written);
        let _ = tx.send(());
    });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gravacao_antiga_que_chega_depois_nao_sobrescreve_a_nova() {
        let dir = tempfile::tempdir().unwrap();
        let first = save_prefs_ordered(
            UiPrefs {
                theme: ThemeMode::Dark,
                ..UiPrefs::default()
            },
            dir.path().to_path_buf(),
        );
        let second = save_prefs_ordered(
            UiPrefs {
                theme: ThemeMode::Light,
                ..UiPrefs::default()
            },
            dir.path().to_path_buf(),
        );
        futures::executor::block_on(async {
            let _ = second.await;
            let _ = first.await;
        });
        assert_eq!(UiPrefs::load(dir.path()).theme, ThemeMode::Light);
    }

    #[test]
    fn port_override_so_vale_em_debug_e_com_espaco_para_o_mais_um() {
        let os = |s: &'static str| Some(OsStr::new(s));
        assert_eq!(port_override(None, true), None);
        assert_eq!(port_override(os("47777"), true), Some(47777));
        assert_eq!(port_override(os("lixo"), true), None);
        assert_eq!(port_override(os("65535"), true), None);
        assert_eq!(port_override(os("65534"), true), Some(65534));
        assert_eq!(port_override(os("47777"), false), None);
    }

    #[test]
    fn bind_ip_padrao_e_lan_e_o_override_so_vale_em_debug() {
        let os = |s: &'static str| Some(OsStr::new(s));
        let lan = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
        let lo = IpAddr::V4(Ipv4Addr::LOCALHOST);
        assert_eq!(bind_ip(None, true), lan);
        assert_eq!(bind_ip(os("127.0.0.1"), true), lo);
        assert_eq!(bind_ip(os("lixo"), true), lan);
        assert_eq!(bind_ip(os("127.0.0.1"), false), lan);
    }

    #[test]
    fn url_das_releases_e_fixa_e_segura_para_o_start() {
        assert_eq!(
            RELEASES_URL,
            "https://github.com/LuizFer1/HubFinance/releases"
        );
        assert!(RELEASES_URL.starts_with("https://github.com/LuizFer1/HubFinance/"));
        for c in [' ', '&', '^', '"', '%'] {
            assert!(!RELEASES_URL.contains(c), "{c:?}");
        }
    }

    #[test]
    fn logs_ficam_numa_subpasta_dos_dados() {
        assert_eq!(
            logs_dir(Path::new("/x/data")),
            PathBuf::from("/x/data").join("logs")
        );
    }

    #[test]
    fn gravar_e_atomico_e_nao_deixa_temporario() {
        let dir = tempfile::tempdir().unwrap();
        let light = UiPrefs {
            theme: ThemeMode::Light,
            ..UiPrefs::default()
        };
        light.save(dir.path()).unwrap();
        UiPrefs::default().save(dir.path()).unwrap();
        light.save(dir.path()).unwrap();
        assert_eq!(UiPrefs::load(dir.path()), light);
        assert!(!dir.path().join("ui.json.tmp").exists());
    }

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
            ..UiPrefs::default()
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
    fn autostart_ausente_e_none_e_none_nao_vai_para_o_arquivo() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("ui.json"), r#"{"theme":"light"}"#).unwrap();
        let prefs = UiPrefs::load(dir.path());
        assert_eq!(prefs.theme, ThemeMode::Light);
        assert_eq!(prefs.autostart, None);
        prefs.save(dir.path()).unwrap();
        let raw = std::fs::read_to_string(dir.path().join("ui.json")).unwrap();
        assert!(!raw.contains("autostart"), "{raw}");
    }

    #[test]
    fn autostart_desligado_de_proposito_sobrevive() {
        let dir = tempfile::tempdir().unwrap();
        let prefs = UiPrefs {
            autostart: Some(false),
            ..UiPrefs::default()
        };
        prefs.save(dir.path()).unwrap();
        assert_eq!(UiPrefs::load(dir.path()), prefs);
        let raw = std::fs::read_to_string(dir.path().join("ui.json")).unwrap();
        assert!(raw.contains("\"autostart\": false"), "{raw}");
    }

    #[test]
    fn hide_values_ausente_no_arquivo_antigo_vale_false() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("ui.json"),
            r#"{"theme":"light","autostart":false}"#,
        )
        .unwrap();
        let prefs = UiPrefs::load(dir.path());
        assert_eq!(prefs.theme, ThemeMode::Light);
        assert!(!prefs.hide_values);
        prefs.save(dir.path()).unwrap();
        let raw = std::fs::read_to_string(dir.path().join("ui.json")).unwrap();
        assert!(!raw.contains("hide_values"), "{raw}");
    }

    #[test]
    fn hide_values_ligado_grava_e_le_de_volta() {
        let dir = tempfile::tempdir().unwrap();
        let prefs = UiPrefs {
            hide_values: true,
            ..UiPrefs::default()
        };
        prefs.save(dir.path()).unwrap();
        assert_eq!(UiPrefs::load(dir.path()), prefs);
        let raw = std::fs::read_to_string(dir.path().join("ui.json")).unwrap();
        assert!(raw.contains("\"hide_values\": true"), "{raw}");
    }

    #[test]
    fn reduzir_movimento_grava_le_de_volta_e_desligado_nao_aparece() {
        let dir = tempfile::tempdir().unwrap();
        let prefs = UiPrefs {
            reduce_motion: true,
            ..UiPrefs::default()
        };
        prefs.save(dir.path()).unwrap();
        assert_eq!(UiPrefs::load(dir.path()), prefs);
        UiPrefs::default().save(dir.path()).unwrap();
        let raw = std::fs::read_to_string(dir.path().join("ui.json")).unwrap();
        assert!(!raw.contains("reduce_motion"), "{raw}");
        // Arquivo de um hub anterior, sem o campo: desligado.
        std::fs::write(dir.path().join("ui.json"), r#"{"theme":"light"}"#).unwrap();
        assert!(!UiPrefs::load(dir.path()).reduce_motion);
    }

    #[test]
    fn tema_sistema_grava_o_modo_resolvido_e_le_de_volta() {
        let dir = tempfile::tempdir().unwrap();
        for (system_dark, stored) in [(false, "\"light\""), (true, "\"dark\"")] {
            let prefs = UiPrefs {
                theme: ThemeMode::System,
                system_dark,
                ..UiPrefs::default()
            };
            prefs.save(dir.path()).unwrap();
            assert_eq!(UiPrefs::load(dir.path()), prefs);
            let raw = std::fs::read_to_string(dir.path().join("ui.json")).unwrap();
            assert!(raw.contains(stored), "{raw}");
            assert!(raw.contains("\"follow_system\": true"), "{raw}");
            assert!(!raw.contains("\"system\""), "{raw}");
        }
        // Tema fixo nao escreve `follow_system`.
        UiPrefs::default().save(dir.path()).unwrap();
        let raw = std::fs::read_to_string(dir.path().join("ui.json")).unwrap();
        assert!(!raw.contains("follow_system"), "{raw}");
    }

    /// O `UiPrefs` da 1.4.0, com o enum de dois valores: le o arquivo novo sem perder nada.
    #[test]
    fn hub_antigo_le_o_arquivo_com_tema_sistema() {
        #[derive(Deserialize, Default, Debug, PartialEq)]
        #[serde(rename_all = "lowercase")]
        enum OldTheme {
            #[default]
            Dark,
            Light,
        }
        #[derive(Deserialize)]
        struct OldPrefs {
            #[serde(default)]
            theme: OldTheme,
            #[serde(default)]
            autostart: Option<bool>,
            #[serde(default)]
            hide_values: bool,
        }
        let dir = tempfile::tempdir().unwrap();
        UiPrefs {
            theme: ThemeMode::System,
            system_dark: false,
            autostart: Some(false),
            hide_values: true,
            reduce_motion: true,
        }
        .save(dir.path())
        .unwrap();
        let raw = std::fs::read(dir.path().join("ui.json")).unwrap();
        assert!(String::from_utf8_lossy(&raw).contains("\"reduce_motion\": true"));
        let old: OldPrefs = serde_json::from_slice(&raw).unwrap();
        assert_eq!(old.theme, OldTheme::Light);
        assert_eq!(old.autostart, Some(false));
        assert!(old.hide_values);
    }

    #[test]
    fn ui_json_antigo_continua_lendo_igual() {
        let dir = tempfile::tempdir().unwrap();
        for (raw, theme) in [
            (r#"{"theme":"dark"}"#, ThemeMode::Dark),
            (r#"{"theme":"light","autostart":true}"#, ThemeMode::Light),
            (r#"{"autostart":false}"#, ThemeMode::Dark),
        ] {
            std::fs::write(dir.path().join("ui.json"), raw).unwrap();
            assert_eq!(UiPrefs::load(dir.path()).theme, theme, "{raw}");
        }
        // Gravar de novo sem mexer no tema nao troca o valor por "system".
        let prefs = UiPrefs::load(dir.path());
        prefs.save(dir.path()).unwrap();
        let raw = std::fs::read_to_string(dir.path().join("ui.json")).unwrap();
        assert!(raw.contains("\"dark\""), "{raw}");
    }

    #[test]
    fn so_o_tema_sistema_segue_o_sistema() {
        assert!(ThemeMode::Dark.is_dark(false));
        assert!(!ThemeMode::Light.is_dark(true));
        assert!(ThemeMode::System.is_dark(true));
        assert!(!ThemeMode::System.is_dark(false));
    }

    #[test]
    fn campo_ausente_vale_o_padrao() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("ui.json"), "{}").unwrap();
        assert_eq!(UiPrefs::load(dir.path()), UiPrefs::default());
    }
}
