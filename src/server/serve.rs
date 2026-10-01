//! Sobe os dois servidores (HTTPS da API e HTTP da CA) e os desliga em ordem.

use std::net::SocketAddr;
use std::time::Duration;

use axum::Router;
use axum_server::Handle;
use axum_server::tls_rustls::RustlsConfig;
use tokio::task::JoinHandle;

use crate::tls::TlsMaterial;

/// Tempo que uma requisicao em curso tem para terminar ao fechar a janela; o push que for
/// cortado nao entra pela metade porque o lote e uma transacao so.
pub const GRACEFUL: Duration = Duration::from_secs(3);

pub struct Servers {
    pub https_addr: SocketAddr,
    pub http_addr: SocketAddr,
    /// `reload_from_pem` quando o certificado e reemitido: troca sem derrubar conexoes.
    pub tls_config: RustlsConfig,
    https_handle: Handle<SocketAddr>,
    http_handle: Handle<SocketAddr>,
    tasks: Vec<JoinHandle<()>>,
}

#[derive(Debug, thiserror::Error)]
pub enum ServeError {
    #[error("porta {0} em uso ou indisponivel: {1}")]
    Bind(u16, std::io::Error),
    #[error("tls: {0}")]
    Tls(std::io::Error),
}

/// Faz o bind com `std::net::TcpListener` ANTES de spawnar: porta ocupada vira erro aqui,
/// no boot, e nao uma tarefa morrendo em silencio depois.
pub async fn bind_servers(
    https_port: u16,
    http_port: u16,
    api: Router,
    ca: Router,
    material: &TlsMaterial,
) -> Result<Servers, ServeError> {
    let tls_config = RustlsConfig::from_pem(
        material.server.cert_pem.clone().into_bytes(),
        material.server.key_pem.clone().into_bytes(),
    )
    .await
    .map_err(ServeError::Tls)?;

    let https_listener = listen(https_port)?;
    let http_listener = listen(http_port)?;
    let https_addr = https_listener
        .local_addr()
        .map_err(|e| ServeError::Bind(https_port, e))?;
    let http_addr = http_listener
        .local_addr()
        .map_err(|e| ServeError::Bind(http_port, e))?;

    let https_handle = Handle::new();
    let http_handle = Handle::new();
    let https = axum_server::from_tcp_rustls(https_listener, tls_config.clone())
        .map_err(|e| ServeError::Bind(https_port, e))?
        .handle(https_handle.clone());
    let http = axum_server::from_tcp(http_listener)
        .map_err(|e| ServeError::Bind(http_port, e))?
        .handle(http_handle.clone());

    let tasks = vec![
        tokio::spawn(async move {
            if let Err(e) = https.serve(api.into_make_service()).await {
                tracing::error!("https: {e}");
            }
        }),
        tokio::spawn(async move {
            if let Err(e) = http.serve(ca.into_make_service()).await {
                tracing::error!("http: {e}");
            }
        }),
    ];

    Ok(Servers {
        https_addr,
        http_addr,
        tls_config,
        https_handle,
        http_handle,
        tasks,
    })
}

/// `0.0.0.0`: os celulares chegam por qualquer interface de LAN, e o IP pode mudar sem
/// reiniciar o hub.
fn listen(port: u16) -> Result<std::net::TcpListener, ServeError> {
    let listener =
        std::net::TcpListener::bind(("0.0.0.0", port)).map_err(|e| ServeError::Bind(port, e))?;
    listener
        .set_nonblocking(true)
        .map_err(|e| ServeError::Bind(port, e))?;
    Ok(listener)
}

impl Servers {
    pub async fn shutdown(self) {
        self.https_handle.graceful_shutdown(Some(GRACEFUL));
        self.http_handle.graceful_shutdown(Some(GRACEFUL));
        for task in self.tasks {
            if let Err(e) = task.await {
                tracing::error!("servidor terminou com erro: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;
    use std::time::Instant;

    use time::OffsetDateTime;

    use super::*;

    #[tokio::test]
    async fn sobe_em_portas_efemeras_e_desliga_rapido() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let dir = tempfile::tempdir().unwrap();
        let material = crate::tls::load_or_create(
            dir.path(),
            &[Ipv4Addr::new(192, 168, 0, 5)],
            OffsetDateTime::now_utc(),
        )
        .unwrap();
        let servers = bind_servers(0, 0, Router::new(), Router::new(), &material)
            .await
            .unwrap();
        assert_ne!(servers.https_addr.port(), 0);
        assert_ne!(servers.http_addr.port(), 0);
        assert_ne!(servers.https_addr.port(), servers.http_addr.port());

        let started = Instant::now();
        servers.shutdown().await;
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[tokio::test]
    async fn porta_ocupada_e_erro_no_bind() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let dir = tempfile::tempdir().unwrap();
        let material =
            crate::tls::load_or_create(dir.path(), &[], OffsetDateTime::now_utc()).unwrap();
        let taken = std::net::TcpListener::bind(("0.0.0.0", 0)).unwrap();
        let port = taken.local_addr().unwrap().port();
        let result = bind_servers(port, 0, Router::new(), Router::new(), &material).await;
        assert!(matches!(result, Err(ServeError::Bind(p, _)) if p == port));
    }
}
