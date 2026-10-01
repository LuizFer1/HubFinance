//! Nucleo do hub: thread com runtime tokio, loop de comandos e snapshot para a UI.

use tokio::sync::{mpsc, watch};

use crate::config::Config;

mod run;
pub mod snapshot;

use snapshot::{Snapshot, Status};

#[derive(Debug, Clone)]
pub enum Command {
    IssuePairingToken,
    RevokeDevice(String),
    Shutdown,
}

#[derive(Clone)]
pub struct HubHandle {
    pub commands: mpsc::UnboundedSender<Command>,
    pub snapshot: watch::Receiver<Snapshot>,
}

/// Cria a thread "nucleo" com um runtime tokio proprio e devolve os dois canais. A thread
/// termina quando `Shutdown` e processado; `join` e opcional.
///
/// Runtime proprio, e nao o executor do iced: o servidor nao deve depender do ciclo de vida
/// da UI, e o desligamento ordenado fica explicito. `tokio::sync` funciona de qualquer
/// executor, entao a ponte e so um canal.
pub fn start(config: Config) -> (HubHandle, std::thread::JoinHandle<()>) {
    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel();
    let (snap_tx, snap_rx) = watch::channel(Snapshot {
        data_dir: config.data_dir.clone(),
        ..Snapshot::default()
    });
    let join = std::thread::spawn(move || {
        match tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("nucleo")
            .build()
        {
            Ok(runtime) => runtime.block_on(run::run(config, cmd_rx, snap_tx)),
            Err(e) => {
                snap_tx.send_modify(|s| {
                    s.status = Status::Failed(format!("nao foi possivel iniciar o runtime: {e}"));
                });
                while let Some(cmd) = cmd_rx.blocking_recv() {
                    if matches!(cmd, Command::Shutdown) {
                        break;
                    }
                }
                snap_tx.send_modify(|s| s.status = Status::Stopped);
            }
        }
    });
    (
        HubHandle {
            commands: cmd_tx,
            snapshot: snap_rx,
        },
        join,
    )
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    fn config(dir: &std::path::Path) -> Config {
        Config {
            data_dir: dir.to_path_buf(),
            https_port: 0,
            http_port: 0,
        }
    }

    /// Espera o snapshot satisfazer `pred`, com prazo; devolve o snapshot que satisfez.
    fn wait_for(
        rx: &mut watch::Receiver<Snapshot>,
        limit: Duration,
        pred: impl FnMut(&Snapshot) -> bool,
    ) -> Snapshot {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        rt.block_on(async {
            tokio::time::timeout(limit, rx.wait_for(pred))
                .await
                .expect("prazo esgotado esperando o snapshot")
                .expect("nucleo terminou antes")
                .clone()
        })
    }

    fn up(snap: &Snapshot) -> bool {
        matches!(snap.status, Status::Running | Status::NoNetwork)
    }

    #[test]
    fn sobe_pareia_revoga_e_desliga() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let dir = tempfile::tempdir().unwrap();
        let (mut handle, join) = start(config(dir.path()));

        let snap = wait_for(&mut handle.snapshot, Duration::from_secs(10), |s| {
            up(s) || matches!(s.status, Status::Failed(_))
        });
        assert!(up(&snap), "{:?}", snap.status);
        assert_eq!(snap.epoch.len(), 26);
        assert_ne!(snap.https_port, 0);
        assert_ne!(snap.http_port, 0);
        assert!(snap.devices.is_empty());
        assert!(dir.path().join("hub.sqlite").exists());
        assert!(dir.path().join("tls").join("ca.crt").exists());

        handle.commands.send(Command::IssuePairingToken).unwrap();
        let snap = wait_for(&mut handle.snapshot, Duration::from_secs(5), |s| {
            s.pairing.is_some()
        });
        let pairing = snap.pairing.unwrap();
        assert_eq!(pairing.token.len(), 6);
        assert!(pairing.qr_url.starts_with("http://"), "{}", pairing.qr_url);
        assert!(
            pairing.qr_url.ends_with(&format!("/p/{}", pairing.token)),
            "{}",
            pairing.qr_url
        );

        handle
            .commands
            .send(Command::RevokeDevice("inexistente".into()))
            .unwrap();
        let snap = wait_for(&mut handle.snapshot, Duration::from_secs(5), |s| {
            s.activity.iter().any(|a| a.text.contains("inexistente"))
        });
        assert!(up(&snap));

        let started = Instant::now();
        handle.commands.send(Command::Shutdown).unwrap();
        wait_for(&mut handle.snapshot, Duration::from_secs(5), |s| {
            s.status == Status::Stopped
        });
        join.join().unwrap();
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn diretorio_invalido_falha_e_ainda_desliga() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("arquivo");
        std::fs::write(&file, "nao sou diretorio").unwrap();
        let (mut handle, join) = start(config(&file));

        let snap = wait_for(&mut handle.snapshot, Duration::from_secs(10), |s| {
            matches!(s.status, Status::Failed(_))
        });
        let Status::Failed(msg) = snap.status else {
            unreachable!()
        };
        assert!(!msg.is_empty());

        handle.commands.send(Command::Shutdown).unwrap();
        wait_for(&mut handle.snapshot, Duration::from_secs(5), |s| {
            s.status == Status::Stopped
        });
        join.join().unwrap();
    }
}
