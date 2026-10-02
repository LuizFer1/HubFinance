//! Log em arquivo: no build release do Windows nao ha console, e o arquivo e a unica coisa que
//! explica um bug achado no teste de campo. Unico modulo (com `main.rs`) que conhece
//! `tracing-subscriber` e `tracing-appender`.
//!
//! Decisoes (plano 0.0.2, Tarefa 4):
//! - `hub.AAAA-MM-DD.log` em `<data_dir>/logs/`, rotacao diaria. Com a data no nome nao existe
//!   um `hub.log` fixo; por isso o card "Sobre o hub" mostra a pasta, nao um arquivo.
//! - Retencao de `KEEP_FILES` arquivos por `prune`, chamada no boot (o contrato testado), e
//!   tambem por `max_log_files` do appender, que poda a cada rotacao: um hub pode ficar ligado
//!   semanas sem reiniciar.
//! - Escrita sincrona, sem `non_blocking`: `iced::exit()` e o `ForceExit` terminam o processo
//!   sem soltar um `WorkerGuard`, e o `warn!` escrito segundos antes de uma queda e justamente
//!   o que o teste de campo precisa ler. Sao dezenas de linhas por dia; o custo e nenhum.
//! - Falha ao criar a pasta ou o arquivo (disco so leitura, quota) cai para o console com um
//!   `warn!`: log nunca impede o hub de abrir.

use std::path::{Path, PathBuf};

use time::format_description::well_known::Rfc3339;
use tracing_appender::rolling::{InitError, RollingFileAppender, Rotation};
use tracing_subscriber::fmt::time::OffsetTime;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt};

use crate::config;

pub const FILE_PREFIX: &str = "hub";
pub const FILE_SUFFIX: &str = "log";
pub const KEEP_FILES: usize = 7;
/// `info` do hub; das dependencias (iced, wgpu) so avisos, senao o log do hub some no meio do
/// log do renderizador. `RUST_LOG` sobrescreve, para o arquivo e para o console.
pub const DEFAULT_FILTER: &str = "warn,hub_finance=info";

/// `hub.AAAA-MM-DD.log`, como o appender diario nomeia. So esses entram na retencao: um
/// arquivo que a pessoa deixou na pasta nunca e apagado por nos.
pub fn is_log_file(name: &str) -> bool {
    let Some(rest) = name
        .strip_prefix(FILE_PREFIX)
        .and_then(|r| r.strip_prefix('.'))
    else {
        return false;
    };
    let Some(date) = rest
        .strip_suffix(FILE_SUFFIX)
        .and_then(|r| r.strip_suffix('.'))
    else {
        return false;
    };
    let bytes = date.as_bytes();
    bytes.len() == 10
        && bytes.iter().enumerate().all(|(i, b)| match i {
            4 | 7 => *b == b'-',
            _ => b.is_ascii_digit(),
        })
}

/// Apaga os mais antigos alem de `keep`. Ordem pelo nome: a data no nome e ISO, entao a ordem
/// lexicografica e a cronologica. Devolve o que apagou (para o log e para o teste).
pub fn prune(dir: &Path, keep: usize) -> std::io::Result<Vec<PathBuf>> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        // O boot chama antes de o appender criar a pasta: pasta ausente e "nada a podar".
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err),
    };
    let mut logs = Vec::new();
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        if let Some(name) = entry.file_name().to_str()
            && is_log_file(name)
        {
            logs.push(name.to_owned());
        }
    }
    logs.sort();
    let excess = logs.len().saturating_sub(keep);
    let mut removed = Vec::with_capacity(excess);
    for name in logs.into_iter().take(excess) {
        let path = dir.join(name);
        std::fs::remove_file(&path)?;
        removed.push(path);
    }
    Ok(removed)
}

pub fn file_appender(dir: &Path) -> Result<RollingFileAppender, InitError> {
    RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix(FILE_PREFIX)
        .filename_suffix(FILE_SUFFIX)
        .max_log_files(KEEP_FILES)
        .build(dir)
}

/// Instala o subscriber global. Devolve a pasta de logs quando o arquivo esta ativo; `None`
/// quando caiu para console (ja avisado por `warn!`). Chamado uma vez, em `main`.
pub fn init(data_dir: &Path, offset: time::UtcOffset) -> Option<PathBuf> {
    let logs_dir = config::logs_dir(data_dir);
    let pruned = prune(&logs_dir, KEEP_FILES);
    let appender = std::fs::create_dir_all(&logs_dir)
        .map_err(|err| err.to_string())
        .and_then(|()| file_appender(&logs_dir).map_err(|err| err.to_string()));

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER));
    // Fuso local, lido por `main` antes de qualquer thread: as horas do arquivo batem com as da
    // janela.
    let timer = OffsetTime::new(offset, Rfc3339);
    let (file_layer, file_err) = match appender {
        Ok(appender) => (
            Some(
                fmt::layer()
                    .with_ansi(false)
                    .with_target(false)
                    .with_timer(timer.clone())
                    .with_writer(appender),
            ),
            None,
        ),
        Err(err) => (None, Some(err)),
    };
    tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(fmt::layer().with_timer(timer).with_writer(std::io::stderr))
        .init();

    tracing::info!(
        "HubFinance v{} iniciando; dados em {}",
        config::VERSION,
        data_dir.display()
    );
    match pruned {
        Ok(removed) if !removed.is_empty() => {
            tracing::info!("{} arquivo(s) de log antigo(s) apagado(s)", removed.len());
        }
        Ok(_) => {}
        Err(err) => tracing::warn!("nao foi possivel podar os logs antigos: {err}"),
    }
    match file_err {
        None => Some(logs_dir),
        Some(err) => {
            tracing::warn!("log em arquivo desligado: {err}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tracing_subscriber::fmt::MakeWriter;

    use super::*;

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn reconhece_so_o_nome_do_appender_diario() {
        assert!(is_log_file("hub.2026-10-02.log"));
        for name in [
            "hub.log",
            "outro.2026-10-02.log",
            "hub.2026-10-02.log.tmp",
            "hub.2026-10-02",
            "hub.2026-1a-02.log",
            "hub.2026_10_02.log",
        ] {
            assert!(!is_log_file(name), "{name}");
        }
    }

    #[test]
    fn poda_os_mais_antigos_alem_de_sete() {
        let dir = tempfile::tempdir().unwrap();
        let days = [
            "2026-09-24",
            "2026-09-25",
            "2026-09-26",
            "2026-09-27",
            "2026-09-28",
            "2026-09-29",
            "2026-09-30",
            "2026-10-01",
            "2026-10-02",
        ];
        for day in days {
            std::fs::write(dir.path().join(format!("hub.{day}.log")), day).unwrap();
        }
        let removed = prune(dir.path(), 7).unwrap();
        assert_eq!(
            removed,
            vec![
                dir.path().join("hub.2026-09-24.log"),
                dir.path().join("hub.2026-09-25.log"),
            ]
        );
        let expected: Vec<String> = days[2..].iter().map(|d| format!("hub.{d}.log")).collect();
        assert_eq!(names(dir.path()), expected);
    }

    #[test]
    fn abaixo_do_limite_nao_apaga_nada() {
        let dir = tempfile::tempdir().unwrap();
        for day in ["2026-09-30", "2026-10-01", "2026-10-02"] {
            std::fs::write(dir.path().join(format!("hub.{day}.log")), "").unwrap();
        }
        assert!(prune(dir.path(), 7).unwrap().is_empty());
        assert_eq!(names(dir.path()).len(), 3);
    }

    #[test]
    fn nunca_apaga_arquivo_fora_do_padrao() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("notas.txt"), "").unwrap();
        std::fs::write(dir.path().join("hub.log"), "").unwrap();
        for i in 0..20 {
            std::fs::write(dir.path().join(format!("notas-{i}.txt")), "").unwrap();
        }
        assert!(prune(dir.path(), 7).unwrap().is_empty());
        assert_eq!(names(dir.path()).len(), 22);
    }

    #[test]
    fn pasta_inexistente_nao_e_erro() {
        let dir = tempfile::tempdir().unwrap();
        assert!(prune(&dir.path().join("logs"), 7).unwrap().is_empty());
    }

    #[test]
    fn appender_escreve_num_arquivo_com_o_nome_do_padrao() {
        let dir = tempfile::tempdir().unwrap();
        let appender = file_appender(dir.path()).unwrap();
        {
            let mut writer = appender.make_writer();
            writer.write_all(b"ola\n").unwrap();
            writer.flush().unwrap();
        }
        drop(appender);
        let files = names(dir.path());
        assert_eq!(files.len(), 1, "{files:?}");
        let name = &files[0];
        assert!(is_log_file(name), "{name}");
        let content = std::fs::read_to_string(dir.path().join(name)).unwrap();
        assert!(content.contains("ola"), "{content}");
    }
}
