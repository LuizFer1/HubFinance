//! O que a janela mostra. O nucleo publica um `Snapshot` inteiro a cada mudanca; a UI so
//! renderiza, sem estado proprio para desincronizar.

use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use crate::dashboard::dataset::Dataset;
use crate::dashboard::people::DeviceInfo;
use crate::store::devices::Device;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Status {
    #[default]
    Starting,
    Running,
    /// Servidores no ar, mas sem IP privado: nenhum celular alcanca o hub ainda.
    NoNetwork,
    Stopping,
    Stopped,
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PairingView {
    pub token: String,
    pub display: String,
    pub expires_at: SystemTime,
    pub qr_url: String,
}

/// Sem `key_hash` de proposito: o hash nao tem utilidade na tela e nao deve chegar a UI nem a
/// log nenhum.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceView {
    pub device_id: String,
    pub name: String,
    pub paired_at: String,
    pub last_seen_at: Option<String>,
    pub last_push_at: Option<String>,
    pub last_pull_at: Option<String>,
    pub revoked: bool,
    /// A pessoa do app que pareou (`users[user_id]`), quando o app mandou.
    pub user_id: Option<String>,
}

impl From<Device> for DeviceView {
    fn from(d: Device) -> Self {
        DeviceView {
            device_id: d.device_id,
            name: d.name,
            paired_at: d.paired_at,
            last_seen_at: d.last_seen_at,
            last_push_at: d.last_push_at,
            last_pull_at: d.last_pull_at,
            revoked: d.revoked_at.is_some(),
            user_id: d.user_id,
        }
    }
}

/// O modulo puro `dashboard::people` tem a sua copia do aparelho; a ponte fica aqui, no hub.
impl From<&DeviceView> for DeviceInfo {
    fn from(d: &DeviceView) -> Self {
        DeviceInfo {
            device_id: d.device_id.clone(),
            name: d.name.clone(),
            user_id: d.user_id.clone(),
            paired_at: d.paired_at.clone(),
            last_seen_at: d.last_seen_at.clone(),
            last_push_at: d.last_push_at.clone(),
            last_pull_at: d.last_pull_at.clone(),
            revoked: d.revoked,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivityKind {
    Info,
    Sync,
    Warning,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivityEntry {
    pub at: SystemTime,
    pub kind: ActivityKind,
    pub text: String,
}

pub const MAX_ACTIVITY: usize = 50;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub status: Status,
    pub epoch: String,
    pub data_dir: PathBuf,
    /// Principal primeiro: e o que entra no QR.
    pub addresses: Vec<Ipv4Addr>,
    pub https_port: u16,
    pub http_port: u16,
    pub pairing: Option<PairingView>,
    pub devices: Vec<DeviceView>,
    /// Mais recente primeiro.
    pub activity: Vec<ActivityEntry>,
    /// `Arc` porque o snapshot e republicado inteiro a cada mudanca: clonar o `Arc` e um
    /// incremento, e a UI compara por ponteiro para saber se os dados mudaram.
    pub dashboard: Arc<Dataset>,
}

impl Snapshot {
    /// A atividade guarda 50 linhas e some ao fechar; o arquivo de log e a memoria. A atividade
    /// ja nunca traz chave, hash, token nem linha sincronizada, e o espelho herda a garantia.
    /// O `target` proprio deixa `RUST_LOG=hub_finance::atividade=info` isolar so a atividade.
    pub fn log(&mut self, kind: ActivityKind, text: impl Into<String>) {
        let text = text.into();
        match kind {
            ActivityKind::Info | ActivityKind::Sync => {
                tracing::info!(target: "hub_finance::atividade", "{text}");
            }
            ActivityKind::Warning => tracing::warn!(target: "hub_finance::atividade", "{text}"),
            ActivityKind::Error => tracing::error!(target: "hub_finance::atividade", "{text}"),
        }
        self.activity.insert(
            0,
            ActivityEntry {
                at: SystemTime::now(),
                kind,
                text,
            },
        );
        self.activity.truncate(MAX_ACTIVITY);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atividade_guarda_as_50_mais_recentes() {
        let mut snap = Snapshot::default();
        for i in 0..60 {
            snap.log(ActivityKind::Info, format!("{i}"));
        }
        assert_eq!(snap.activity.len(), MAX_ACTIVITY);
        assert_eq!(snap.activity[0].text, "59");
    }

    #[test]
    fn view_nao_carrega_o_hash() {
        let view = DeviceView::from(Device {
            device_id: "A".into(),
            name: "Pixel".into(),
            key_hash: "segredo".into(),
            paired_at: "2026-10-01T18:00:00Z".into(),
            last_seen_at: None,
            last_push_at: None,
            last_pull_at: None,
            revoked_at: Some("2026-10-01T19:00:00Z".into()),
            user_id: None,
        });
        assert!(view.revoked);
        assert!(!format!("{view:?}").contains("segredo"));
    }
}
