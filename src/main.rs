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
    let (handle, _nucleo) = hub::start(config);
    ui::run(handle, offset)
}
