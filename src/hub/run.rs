//! Loop do nucleo: sobe store, TLS e servidores, atende comandos da UI e eventos do servidor,
//! e publica o snapshot a cada mudanca.

use std::collections::BTreeSet;
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError, RwLock};
use std::time::{Duration, SystemTime};

use time::OffsetDateTime;
use tokio::sync::{mpsc, watch};

use super::Command;
use super::dashboard::{DashboardState, REFRESH_DEBOUNCE};
use super::snapshot::{ActivityKind, DeviceView, PairingView, Snapshot, Status};
use crate::config::Config;
use crate::pairing::token::TokenBook;
use crate::server::ca_http::{CaState, ca_router};
use crate::server::router::full_api;
use crate::server::serve::{Servers, bind_servers};
use crate::server::state::{AppState, ServerEvent, rfc3339};
use crate::store::{Store, StoreError};
use crate::tls::server_cert::{ServerCert, needs_reissue};
use crate::tls::{self, lan};

/// Vigia de IP e de expiracao: 30 s e o atraso maximo para a janela mostrar o IP novo.
const TICK: Duration = Duration::from_secs(30);

struct Hub {
    store: Arc<Store>,
    tokens: Arc<Mutex<TokenBook>>,
    primary_ip: Arc<RwLock<Option<Ipv4Addr>>>,
    events: mpsc::UnboundedReceiver<ServerEvent>,
    servers: Servers,
    tls_dir: PathBuf,
    ca: tls::ca::Ca,
    server_cert: ServerCert,
    snap: Snapshot,
    dashboard: DashboardState,
    /// Quando o dashboard deve recarregar; `None` = nada pendente. Cada push com linha aceita
    /// empurra o prazo (debounce).
    refresh_due: Option<tokio::time::Instant>,
}

pub(super) async fn run(
    config: Config,
    mut commands: mpsc::UnboundedReceiver<Command>,
    snapshot: watch::Sender<Snapshot>,
) {
    let mut snap = Snapshot {
        status: Status::Starting,
        data_dir: config.data_dir.clone(),
        https_port: config.https_port,
        http_port: config.http_port,
        ..Snapshot::default()
    };
    snapshot.send_replace(snap.clone());

    match boot(&config, &mut snap).await {
        Ok(hub) => hub.serve(&mut commands, &snapshot).await,
        Err(msg) => {
            tracing::error!("hub nao subiu: {msg}");
            snap.status = Status::Failed(msg);
            snapshot.send_replace(snap.clone());
            // Sem servidores nao ha o que fazer alem de mostrar o erro ate a janela fechar.
            while let Some(cmd) = commands.recv().await {
                if matches!(cmd, Command::Shutdown) {
                    break;
                }
            }
            snap.status = Status::Stopped;
            snapshot.send_replace(snap);
        }
    }
}

async fn boot(config: &Config, snap: &mut Snapshot) -> Result<Hub, String> {
    let store = Store::open(&config.db_path()).map_err(|e| {
        format!(
            "nao foi possivel abrir o banco em {}: {e}",
            config.db_path().display()
        )
    })?;
    let store = Arc::new(store);

    let ips = lan::discover();
    let tls_dir = config.tls_dir();
    let material = tls::load_or_create(&tls_dir, &ips, OffsetDateTime::now_utc())
        .map_err(|e| format!("falha ao preparar o certificado: {e}"))?;

    let tokens = Arc::new(Mutex::new(TokenBook::default()));
    let primary_ip = Arc::new(RwLock::new(ips.first().copied()));
    let (events_tx, events) = mpsc::unbounded_channel();
    let now: Arc<dyn Fn() -> SystemTime + Send + Sync> = Arc::new(SystemTime::now);
    let api = full_api(AppState {
        store: Arc::clone(&store),
        tokens: Arc::clone(&tokens),
        events: events_tx,
        now: Arc::clone(&now),
    });
    let ca = ca_router(CaState {
        ca_pem: Arc::new(material.ca.cert_pem.clone()),
        tokens: Arc::clone(&tokens),
        primary_ip: Arc::clone(&primary_ip),
        // Porta configurada: nas portas fixas de producao e a real. So os testes usam 0.
        https_port: config.https_port,
        now,
    });
    let servers = bind_servers(config.https_port, config.http_port, api, ca, &material)
        .await
        .map_err(|e| match e {
            crate::server::serve::ServeError::Bind(port, _) => format!("Porta {port} em uso"),
            other => other.to_string(),
        })?;

    snap.epoch = store.epoch().to_string();
    snap.https_port = servers.https_addr.port();
    snap.http_port = servers.http_addr.port();
    snap.addresses = ips;
    snap.status = network_status(&snap.addresses);
    let mut hub = Hub {
        store,
        tokens,
        primary_ip,
        events,
        servers,
        tls_dir,
        ca: material.ca,
        server_cert: material.server,
        snap: snap.clone(),
        dashboard: DashboardState::default(),
        refresh_due: None,
    };
    hub.reload_devices().await;
    // Falha aqui vira linha de erro na atividade; o hub sobe mesmo assim, com dataset vazio.
    hub.refresh_dashboard().await;
    hub.snap.log(ActivityKind::Info, "Hub ligado");
    if hub.snap.addresses.is_empty() {
        hub.snap.log(ActivityKind::Warning, "Sem rede local");
    }
    Ok(hub)
}

fn network_status(ips: &[Ipv4Addr]) -> Status {
    if ips.is_empty() {
        Status::NoNetwork
    } else {
        Status::Running
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

impl Hub {
    async fn serve(
        mut self,
        commands: &mut mpsc::UnboundedReceiver<Command>,
        snapshot: &watch::Sender<Snapshot>,
    ) {
        snapshot.send_replace(self.snap.clone());
        let mut tick = tokio::time::interval_at(tokio::time::Instant::now() + TICK, TICK);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                cmd = commands.recv() => match cmd {
                    // UI caiu sem mandar Shutdown: desliga do mesmo jeito.
                    None | Some(Command::Shutdown) => break,
                    Some(Command::IssuePairingToken) => self.issue_token(),
                    Some(Command::RevokeDevice(id)) => self.revoke(id).await,
                },
                Some(event) = self.events.recv() => self.on_event(event).await,
                _ = tick.tick() => self.on_tick().await,
                _ = tokio::time::sleep_until(
                    self.refresh_due.unwrap_or_else(tokio::time::Instant::now)
                ), if self.refresh_due.is_some() => {
                    self.refresh_due = None;
                    self.refresh_dashboard().await;
                }
            }
            snapshot.send_replace(self.snap.clone());
        }

        self.snap.status = Status::Stopping;
        self.snap.pairing = None;
        snapshot.send_replace(self.snap.clone());
        let mut snap = self.snap;
        self.servers.shutdown().await;
        // Os routers (e com eles as copias da store) morreram com os servidores; esta e a
        // ultima referencia, e solta-la fecha o SQLite antes de anunciar `Stopped`.
        drop(self.store);
        snap.status = Status::Stopped;
        snapshot.send_replace(snap);
    }

    fn issue_token(&mut self) {
        let token = self
            .tokens
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .issue(SystemTime::now());
        let host = self
            .snap
            .addresses
            .first()
            .map_or_else(|| "127.0.0.1".to_string(), Ipv4Addr::to_string);
        self.snap.pairing = Some(PairingView {
            display: token.display(),
            qr_url: format!("http://{host}:{}/p/{}", self.snap.http_port, token.value),
            token: token.value,
            expires_at: token.expires_at,
        });
    }

    async fn revoke(&mut self, device_id: String) {
        let now = rfc3339(SystemTime::now());
        let id = device_id.clone();
        match self
            .blocking(move |store| store.revoke_device(&id, &now))
            .await
        {
            Ok(true) => {
                let name = self
                    .snap
                    .devices
                    .iter()
                    .find(|d| d.device_id == device_id)
                    .map_or(device_id.clone(), |d| d.name.clone());
                self.snap
                    .log(ActivityKind::Info, format!("Revogado: {name}"));
            }
            Ok(false) => self.snap.log(
                ActivityKind::Warning,
                format!("Aparelho nao encontrado: {device_id}"),
            ),
            Err(e) => self
                .snap
                .log(ActivityKind::Error, format!("Erro ao revogar: {e}")),
        }
        self.reload_devices().await;
    }

    async fn on_event(&mut self, event: ServerEvent) {
        match event {
            ServerEvent::Paired { device } => {
                self.snap.pairing = None;
                self.snap
                    .log(ActivityKind::Info, format!("Pareado: {}", device.name));
                self.reload_devices().await;
            }
            ServerEvent::Pushed {
                name,
                accepted,
                ignored,
                rejected,
                ..
            } => {
                let total = accepted + ignored + rejected;
                let mut details = Vec::new();
                if ignored > 0 {
                    details.push(plural(ignored, "ignorada", "ignoradas"));
                }
                if rejected > 0 {
                    details.push(plural(rejected, "rejeitada", "rejeitadas"));
                }
                let mut text = format!("{name} enviou {}", plural(total, "linha", "linhas"));
                if !details.is_empty() {
                    text.push_str(&format!(" ({})", details.join(", ")));
                }
                let kind = if rejected > 0 {
                    ActivityKind::Warning
                } else {
                    ActivityKind::Sync
                };
                self.snap.log(kind, text);
                self.reload_devices().await;
                // Pull nao muda o banco; so push com linha aceita pede refresh.
                if accepted > 0 {
                    self.refresh_due = Some(tokio::time::Instant::now() + REFRESH_DEBOUNCE);
                }
            }
            ServerEvent::Pulled { name, rows, .. } => {
                // Pull vazio e o celular so conferindo; registrar cada um afogaria o resto.
                if rows > 0 {
                    self.snap.log(
                        ActivityKind::Sync,
                        format!("{name} recebeu {}", plural(rows, "linha", "linhas")),
                    );
                }
                self.reload_devices().await;
            }
            ServerEvent::TokenExhausted => {
                self.snap.pairing = None;
                self.snap
                    .log(ActivityKind::Warning, "Código invalidado após 5 tentativas");
            }
            ServerEvent::RevokedAttempt { name } => self.snap.log(
                ActivityKind::Warning,
                format!("Tentativa com aparelho revogado: {name}"),
            ),
            ServerEvent::Failure(msg) => self.snap.log(ActivityKind::Error, format!("Erro: {msg}")),
        }
    }

    async fn on_tick(&mut self) {
        if self
            .snap
            .pairing
            .as_ref()
            .is_some_and(|p| SystemTime::now() >= p.expires_at)
        {
            self.snap.pairing = None;
        }

        let ips = match tokio::task::spawn_blocking(lan::discover).await {
            Ok(ips) => ips,
            Err(e) => {
                self.snap
                    .log(ActivityKind::Error, format!("Erro ao ler a rede: {e}"));
                return;
            }
        };
        let now = OffsetDateTime::now_utc();
        if needs_reissue(&self.server_cert, &ips, now) {
            self.reissue(&ips, now).await;
        }
        let changed = ips.iter().collect::<BTreeSet<_>>()
            != self.snap.addresses.iter().collect::<BTreeSet<_>>()
            || ips.first() != self.snap.addresses.first();
        if changed {
            *self
                .primary_ip
                .write()
                .unwrap_or_else(PoisonError::into_inner) = ips.first().copied();
            self.snap.addresses = ips;
            if matches!(self.snap.status, Status::Running | Status::NoNetwork) {
                self.snap.status = network_status(&self.snap.addresses);
            }
        }
    }

    /// Mesma CA, certificado novo: o celular nao reinstala nada e as conexoes abertas seguem.
    async fn reissue(&mut self, ips: &[Ipv4Addr], now: OffsetDateTime) {
        let issued = tls::reissue_server(&self.tls_dir, &self.ca, ips, now);
        let cert = match issued {
            Ok(cert) => cert,
            Err(e) => {
                self.snap.log(
                    ActivityKind::Error,
                    format!("Falha ao reemitir o certificado: {e}"),
                );
                return;
            }
        };
        let reload = self
            .servers
            .tls_config
            .reload_from_pem(
                cert.cert_pem.clone().into_bytes(),
                cert.key_pem.clone().into_bytes(),
            )
            .await;
        match reload {
            Ok(()) => {
                let list: Vec<String> = ips.iter().map(Ipv4Addr::to_string).collect();
                self.snap.log(
                    ActivityKind::Info,
                    format!("Certificado reemitido: {}", list.join(", ")),
                );
                self.server_cert = cert;
            }
            Err(e) => self.snap.log(
                ActivityKind::Error,
                format!("Falha ao recarregar o certificado: {e}"),
            ),
        }
    }

    async fn refresh_dashboard(&mut self) {
        let ignored_before = self.snap.dashboard.ignored_total();
        match self.dashboard.refresh(&self.store, SystemTime::now()).await {
            Ok(report) => {
                self.snap.dashboard = Arc::clone(&self.dashboard.dataset);
                if report.ignored_total > ignored_before {
                    let detail: Vec<String> = self
                        .dashboard
                        .dataset
                        .ignored
                        .iter()
                        .map(|(table, ids)| format!("{table}: {}", ids.len()))
                        .collect();
                    self.snap.log(
                        ActivityKind::Warning,
                        format!(
                            "Dashboard: {} fora do contrato de campos ({})",
                            plural(report.ignored_total, "linha", "linhas"),
                            detail.join(", ")
                        ),
                    );
                }
            }
            Err(e) => self.snap.log(
                ActivityKind::Error,
                format!("Erro ao atualizar o dashboard: {e}"),
            ),
        }
    }

    async fn reload_devices(&mut self) {
        match self.blocking(|store| store.list_devices()).await {
            Ok(devices) => {
                self.snap.devices = devices.into_iter().map(DeviceView::from).collect();
            }
            Err(e) => self.snap.log(
                ActivityKind::Error,
                format!("Erro ao ler os aparelhos: {e}"),
            ),
        }
    }

    /// Erro da store vira linha vermelha na atividade, nunca queda do loop: o hub fica de pe
    /// para o usuario ver o que houve.
    async fn blocking<T, F>(&self, f: F) -> Result<T, String>
    where
        T: Send + 'static,
        F: FnOnce(&Store) -> Result<T, StoreError> + Send + 'static,
    {
        let store = Arc::clone(&self.store);
        match tokio::task::spawn_blocking(move || f(&store)).await {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(e)) => Err(e.to_string()),
            Err(e) => Err(e.to_string()),
        }
    }
}
