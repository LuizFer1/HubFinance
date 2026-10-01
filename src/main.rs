// Temporario: os modulos ainda nao sao ligados ao `main`; a Tarefa 17 remove isto.
#![allow(dead_code)]

mod config;
mod hub;
mod pairing;
mod protocol;
mod server;
mod store;
mod tls;
mod ui;

fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    // O provider precisa existir antes de qualquer `ServerConfig::builder()`; sem ele o
    // rustls entra em panico na primeira conexao, nao no boot.
    if rustls::crypto::ring::default_provider()
        .install_default()
        .is_err()
    {
        tracing::warn!("provider rustls ja instalado");
    }
    ui::run()
}
