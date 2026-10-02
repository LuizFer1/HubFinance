// Build release no Windows e app de janela: sem isso abre um console preto junto. Os logs do
// `tracing` vao para o stdout e somem nesse modo, o que e aceitavel porque nada essencial passa
// por eles: estado e erros aparecem na propria janela. Em debug o console continua, para o log.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod dashboard;
mod hub;
mod pairing;
mod protocol;
mod server;
mod store;
mod tls;
mod ui;

fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                // `info` do hub; das dependencias (iced, wgpu) so avisos, senao o log do hub
                // some no meio do log do renderizador.
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn,hub_finance=info")),
        )
        .init();
    // O provider precisa existir antes de qualquer `ServerConfig::builder()`; sem ele o
    // rustls entra em panico na primeira conexao, nao no boot.
    if rustls::crypto::ring::default_provider()
        .install_default()
        .is_err()
    {
        tracing::warn!("provider rustls ja instalado");
    }
    // Lido antes de criar a thread do nucleo: em Unix o `time` so le o fuso com um unico
    // thread vivo. Sem ele, a janela mostra horas em UTC.
    let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    let config = match config::Config::from_env() {
        Ok(config) => config,
        Err(err) => {
            eprintln!("HubFinance: {err}");
            std::process::exit(2);
        }
    };
    let prefs = config::UiPrefs::load(&config.data_dir);
    let prefs_dir = config.data_dir.clone();
    let (handle, _nucleo) = hub::start(config);
    ui::run(handle, offset, prefs, prefs_dir)
}
