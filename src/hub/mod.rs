//! Nucleo do hub: thread com runtime tokio, loop de comandos e snapshot para a UI.

use tokio::sync::{mpsc, watch};

use crate::config::Config;

pub mod dashboard;
mod run;
pub mod snapshot;

use crate::store::erase::AreaSet;
use snapshot::{Snapshot, Status};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    IssuePairingToken,
    /// Remove (revoga) o aparelho e apaga as `erase` areas na mesma transacao. `erase` vazio e
    /// "Manter tudo": remove e grava a decisao de manter os dados.
    RemoveDevice {
        device_id: String,
        erase: AreaSet,
    },
    /// Aparelho ja removido que ainda tem dados: apaga as areas sem mexer na revogacao.
    EraseDeviceData {
        device_id: String,
        areas: AreaSet,
    },
    /// "Manter tudo" de um aparelho ja removido.
    KeepDeviceData(String),
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
        // Banco com dado antes de subir: o primeiro `Running` ja tem que trazer o dataset.
        {
            let store = crate::store::Store::open(&dir.path().join("hub.sqlite")).unwrap();
            store
                .apply_batch(
                    "01HZZZZZZZZZZZZZZZZZZZZZZA",
                    &[dashboard::tests::push_entry(
                        "transactions",
                        "01HZZZZZZZZZZZZZZZZZZZZZT1",
                        1000,
                        false,
                        dashboard::tests::tx(serde_json::json!(100)),
                    )],
                )
                .unwrap();
        }
        let (mut handle, join) = start(config(dir.path()));

        let snap = wait_for(&mut handle.snapshot, Duration::from_secs(10), |s| {
            up(s) || matches!(s.status, Status::Failed(_))
        });
        assert!(up(&snap), "{:?}", snap.status);
        assert_eq!(snap.dashboard.alive_transactions().count(), 1);
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
            .send(Command::RemoveDevice {
                device_id: "inexistente".into(),
                erase: AreaSet::default(),
            })
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

    // --- desconectar aparelho: apagar dados por area ---

    use crate::store::Store;
    use crate::store::erase::Area;

    const A: &str = "01HZZZZZZZZZZZZZZZZZZZZZZA";
    const B: &str = "01HZZZZZZZZZZZZZZZZZZZZZZB";
    const GONE: &str = "01HZZZZZZZZZZZZZZZZZZZZZZC";
    const T1: &str = "2026-10-01T18:00:00Z";

    fn id(prefix: &str, n: u32) -> String {
        format!("{:Z<21}{prefix}{n:03}", "01H")
    }

    fn push(store: &Store, origin: &str, table: &str, row_id: &str, extra: serde_json::Value) {
        // `push_entry` carimba com o aparelho A; o HLC de cada linha leva o sufixo do `origin`
        // pedido (o criterio provisorio exige os dois).
        let mut entry = dashboard::tests::push_entry(table, row_id, 1000, false, extra);
        if let Some(row) = entry.row.as_object_mut() {
            row.insert(
                "updatedAt".into(),
                serde_json::json!(format!("{:013}-0000-{origin}", 1000)),
            );
        }
        store.apply_batch(origin, &[entry]).unwrap();
    }

    /// Banco com A e B ativos e GONE ja removido (revogado), cada um com dados.
    fn seed(dir: &std::path::Path) {
        use serde_json::json;
        let store = Store::open(&dir.join("hub.sqlite")).unwrap();
        store
            .upsert_device(A, "Pixel da Ana", &"a".repeat(64), None, T1)
            .unwrap();
        store
            .upsert_device(B, "iPhone do Bia", &"b".repeat(64), None, T1)
            .unwrap();
        store
            .upsert_device(GONE, "Moto antigo", &"c".repeat(64), None, T1)
            .unwrap();
        for n in 1..=2 {
            push(
                &store,
                A,
                "transactions",
                &id("TX", n),
                dashboard::tests::tx(json!(100)),
            );
        }
        push(
            &store,
            B,
            "transactions",
            &id("TY", 1),
            dashboard::tests::tx(json!(50)),
        );
        push(
            &store,
            A,
            "budgets",
            &id("BG", 1),
            json!({ "categoryId": id("CA", 1), "limitMinor": 5000 }),
        );
        push(
            &store,
            A,
            "reserves",
            &id("RE", 1),
            json!({ "kind": "goal", "name": "Viagem", "icon": "plane", "color": "sky" }),
        );
        push(
            &store,
            A,
            "reserveMovements",
            &id("MV", 1),
            json!({ "reserveId": id("RE", 1), "amountMinor": 5000, "occurredOn": "2026-09-06" }),
        );
        push(
            &store,
            GONE,
            "transactions",
            &id("TZ", 1),
            dashboard::tests::tx(json!(70)),
        );
        store.revoke_device(GONE, T1).unwrap();
    }

    fn run_hub(dir: &std::path::Path) -> (HubHandle, std::thread::JoinHandle<()>, Snapshot) {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let (mut handle, join) = start(config(dir));
        let snap = wait_for(&mut handle.snapshot, Duration::from_secs(10), |s| {
            up(s) || matches!(s.status, Status::Failed(_))
        });
        assert!(up(&snap), "{:?}", snap.status);
        (handle, join, snap)
    }

    fn stop(mut handle: HubHandle, join: std::thread::JoinHandle<()>) {
        handle.commands.send(Command::Shutdown).unwrap();
        wait_for(&mut handle.snapshot, Duration::from_secs(5), |s| {
            s.status == Status::Stopped
        });
        join.join().unwrap();
    }

    fn view<'a>(snap: &'a Snapshot, id: &str) -> &'a snapshot::DeviceView {
        snap.devices.iter().find(|d| d.device_id == id).unwrap()
    }

    #[test]
    fn aparelho_removido_com_dados_e_orfao_na_primeira_abertura_e_em_toda_abertura() {
        let dir = tempfile::tempdir().unwrap();
        seed(dir.path());
        let (handle, join, snap) = run_hub(dir.path());
        let gone = view(&snap, GONE);
        assert!(gone.is_orphan(), "migracao deixa data_kept_at = NULL");
        assert_eq!(gone.data.transactions, 1);
        assert!(!view(&snap, A).is_orphan(), "ativo nunca e orfao");
        stop(handle, join);
        // Segunda abertura sem decidir: mesmo resultado, nada foi gravado.
        let (handle, join, snap) = run_hub(dir.path());
        assert!(view(&snap, GONE).is_orphan());
        stop(handle, join);
    }

    #[test]
    fn manter_tudo_tira_o_orfao_e_continua_assim_na_reabertura() {
        let dir = tempfile::tempdir().unwrap();
        seed(dir.path());
        let (mut handle, join, snap) = run_hub(dir.path());
        let before = snap.dashboard.alive_transactions().count();
        handle
            .commands
            .send(Command::KeepDeviceData(GONE.into()))
            .unwrap();
        let snap = wait_for(&mut handle.snapshot, Duration::from_secs(5), |s| {
            s.devices.iter().any(|d| d.device_id == GONE && d.data_kept)
        });
        assert!(!view(&snap, GONE).is_orphan());
        assert_eq!(
            snap.dashboard.alive_transactions().count(),
            before,
            "manter nao muda total"
        );
        stop(handle, join);
        let (handle, join, snap) = run_hub(dir.path());
        assert!(!view(&snap, GONE).is_orphan());
        stop(handle, join);
    }

    #[test]
    fn apagar_dados_de_removido_nao_mexe_na_revogacao_e_o_dashboard_perde_os_itens() {
        let dir = tempfile::tempdir().unwrap();
        seed(dir.path());
        let (mut handle, join, snap) = run_hub(dir.path());
        assert_eq!(snap.dashboard.alive_transactions().count(), 4);
        handle
            .commands
            .send(Command::EraseDeviceData {
                device_id: GONE.into(),
                areas: AreaSet::all(),
            })
            .unwrap();
        let snap = wait_for(&mut handle.snapshot, Duration::from_secs(5), |s| {
            s.dashboard.alive_transactions().count() == 3
        });
        let gone = view(&snap, GONE);
        assert!(gone.revoked, "apagar nao reativa nem muda a revogacao");
        assert!(!gone.is_orphan());
        assert_eq!(gone.data, Default::default());
        stop(handle, join);
    }

    #[test]
    fn remover_com_subconjunto_de_areas_apaga_so_elas_e_o_dashboard_nao_diverge_do_banco() {
        let dir = tempfile::tempdir().unwrap();
        seed(dir.path());
        let (mut handle, join, snap) = run_hub(dir.path());
        assert_eq!(view(&snap, A).data.transactions, 2);
        assert_eq!(snap.dashboard.alive_budgets().count(), 1);
        handle
            .commands
            .send(Command::RemoveDevice {
                device_id: A.into(),
                erase: [Area::Transactions, Area::Reserves].into_iter().collect(),
            })
            .unwrap();
        let snap = wait_for(&mut handle.snapshot, Duration::from_secs(5), |s| {
            s.dashboard.alive_transactions().count() == 2
        });
        let a = view(&snap, A);
        assert!(a.revoked);
        // Metas ficaram: o aparelho segue pendente (orfao) so por elas.
        assert!(a.is_orphan());
        assert_eq!((a.data.transactions, a.data.budgets), (0, 1));
        assert_eq!(snap.dashboard.alive_budgets().count(), 1);
        assert_eq!(snap.dashboard.alive_reserve_movements().count(), 0);
        assert_eq!(snap.dashboard.reserve_count(), 0);
        assert!(
            snap.activity
                .iter()
                .any(|e| e.text == "Removido: Pixel da Ana; apagado: 2 lançamentos, 1 reserva, 1 movimento de reserva"),
            "{:?}",
            snap.activity.iter().map(|e| &e.text).collect::<Vec<_>>()
        );
        let live = |s: &Snapshot| -> Vec<String> {
            let mut ids: Vec<String> = s
                .dashboard
                .alive_transactions()
                .map(|t| t.id.clone())
                .collect();
            ids.sort();
            ids
        };
        let shown = live(&snap);
        stop(handle, join);

        // O que a janela mostra e o que um dashboard novo le do banco: sem divergencia.
        let store = std::sync::Arc::new(Store::open(&dir.path().join("hub.sqlite")).unwrap());
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let mut fresh = dashboard::DashboardState::default();
        rt.block_on(fresh.refresh(&store, std::time::SystemTime::UNIX_EPOCH))
            .unwrap();
        assert_eq!(
            shown,
            live(&Snapshot {
                dashboard: fresh.dataset.clone(),
                ..Snapshot::default()
            })
        );
    }

    #[test]
    fn remover_sem_areas_deixa_o_dataset_igual_e_mantem_os_dados() {
        let dir = tempfile::tempdir().unwrap();
        seed(dir.path());
        let (mut handle, join, snap) = run_hub(dir.path());
        handle
            .commands
            .send(Command::RemoveDevice {
                device_id: A.into(),
                erase: AreaSet::default(),
            })
            .unwrap();
        let after = wait_for(&mut handle.snapshot, Duration::from_secs(5), |s| {
            s.devices.iter().any(|d| d.device_id == A && d.revoked)
        });
        assert!(std::sync::Arc::ptr_eq(&snap.dashboard, &after.dashboard));
        assert!(view(&after, A).data_kept);
        assert!(!view(&after, A).is_orphan());
        stop(handle, join);
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
