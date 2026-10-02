// Build release no Windows e app de janela: sem isso abre um console preto junto. Sem console,
// o log do `tracing` vai para `<data_dir>/logs/` (ver `logging.rs`); em debug o console
// continua recebendo a mesma coisa.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod dashboard;
mod hub;
mod logging;
mod pairing;
mod protocol;
mod server;
mod store;
mod tls;
mod ui;

fn main() -> iced::Result {
    // Lido antes de criar qualquer thread: em Unix o `time` so le o fuso com um unico thread
    // vivo. Sem ele, a janela e o arquivo de log mostram horas em UTC.
    let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    let config = match config::Config::from_env() {
        Ok(config) => config,
        Err(err) => {
            eprintln!("HubFinance: {err}");
            std::process::exit(2);
        }
    };
    // Depois do config porque o arquivo mora no `data_dir`; antes de tudo o mais para que
    // qualquer aviso do boot ja caia no arquivo.
    logging::init(&config.data_dir, offset);
    // O provider precisa existir antes de qualquer `ServerConfig::builder()`; sem ele o
    // rustls entra em panico na primeira conexao, nao no boot.
    if rustls::crypto::ring::default_provider()
        .install_default()
        .is_err()
    {
        tracing::warn!("provider rustls ja instalado");
    }
    let prefs = config::UiPrefs::load(&config.data_dir);
    let prefs_dir = config.data_dir.clone();
    let (handle, _nucleo) = hub::start(config);
    ui::run(handle, offset, prefs, prefs_dir)
}
