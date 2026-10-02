//! TLS: IPs de LAN, CA local e certificado do servidor.

use std::net::Ipv4Addr;
use std::path::Path;

use time::OffsetDateTime;

pub mod ca;
pub mod files;
pub mod lan;
pub mod server_cert;

use ca::{Ca, generate_ca};
use files::TlsPaths;
use server_cert::{ServerCert, issue_server, needs_reissue};

#[derive(Clone, Debug)]
pub struct TlsMaterial {
    pub ca: Ca,
    pub server: ServerCert,
}

#[derive(Debug, thiserror::Error)]
pub enum TlsError {
    #[error("rcgen: {0}")]
    Rcgen(#[from] rcgen::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("tempo: {0}")]
    Time(String),
}

/// Primeira execucao (ou CA perdida): gera CA e servidor do zero, apagando antes o que
/// sobrou — uma CA sem chave nao serve para nada. Depois: carrega e reemite so o do servidor
/// quando preciso, mantendo a CA que o celular ja instalou.
///
/// So `ca.crt`/`ca.key` ausentes ou ilegiveis justificam uma CA nova: ela obriga a reinstalar
/// o certificado em todo celular. Faltar `server.*` (apagado a mao, gravacao interrompida) so
/// reemite o do servidor.
pub fn load_or_create(
    dir: &Path,
    ips: &[Ipv4Addr],
    now: OffsetDateTime,
) -> Result<TlsMaterial, TlsError> {
    std::fs::create_dir_all(dir)?;
    let paths = TlsPaths::in_dir(dir);
    if let Ok(ca) = files::read_ca(&paths)
        && ca::issuer(&ca).is_ok()
    {
        let server = match files::read_server(&paths) {
            Ok(server) if !needs_reissue(&server, ips, now) => server,
            // Servidor ilegivel ou desatualizado: a CA vale, so ele e refeito.
            _ => reissue_server(dir, &ca, ips, now)?,
        };
        return Ok(TlsMaterial { ca, server });
    }

    files::remove_all(&paths)?;
    let ca = generate_ca(now)?;
    files::write_ca(&paths, &ca)?;
    let server = reissue_server(dir, &ca, ips, now)?;
    Ok(TlsMaterial { ca, server })
}

pub fn reissue_server(
    dir: &Path,
    ca: &Ca,
    ips: &[Ipv4Addr],
    now: OffsetDateTime,
) -> Result<ServerCert, TlsError> {
    let server = issue_server(ca, ips, now)?;
    files::write_server(&TlsPaths::in_dir(dir), &server)?;
    Ok(server)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ips() -> Vec<Ipv4Addr> {
        vec![Ipv4Addr::new(192, 168, 0, 5)]
    }

    #[test]
    fn cria_tudo_em_diretorio_vazio() {
        let dir = tempfile::tempdir().unwrap();
        let tls = dir.path().join("tls");
        let material = load_or_create(&tls, &ips(), OffsetDateTime::now_utc()).unwrap();
        let paths = TlsPaths::in_dir(&tls);
        assert!(paths.all_present());
        assert_eq!(files::read_ca(&paths).unwrap(), material.ca);
        assert_eq!(files::read_server(&paths).unwrap(), material.server);
    }

    #[test]
    fn mesmos_ips_nao_reemitem_e_ip_novo_reemite_so_o_servidor() {
        let dir = tempfile::tempdir().unwrap();
        let now = OffsetDateTime::now_utc();
        let first = load_or_create(dir.path(), &ips(), now).unwrap();
        let ca_bytes = std::fs::read(dir.path().join("ca.crt")).unwrap();

        let again = load_or_create(dir.path(), &ips(), now).unwrap();
        assert_eq!(again.server.cert_pem, first.server.cert_pem);

        let mut more = ips();
        more.push(Ipv4Addr::new(10, 0, 0, 2));
        let reissued = load_or_create(dir.path(), &more, now).unwrap();
        assert_ne!(reissued.server.cert_pem, first.server.cert_pem);
        assert_eq!(reissued.server.ips, more);
        assert_eq!(reissued.ca, first.ca);
        assert_eq!(std::fs::read(dir.path().join("ca.crt")).unwrap(), ca_bytes);
    }

    #[test]
    fn faltar_arquivo_do_servidor_reemite_so_o_servidor() {
        let now = OffsetDateTime::now_utc();
        for missing in ["server.crt", "server.key", "server.json"] {
            let dir = tempfile::tempdir().unwrap();
            let first = load_or_create(dir.path(), &ips(), now).unwrap();
            let ca_bytes = std::fs::read(dir.path().join("ca.crt")).unwrap();
            std::fs::remove_file(dir.path().join(missing)).unwrap();
            let second = load_or_create(dir.path(), &ips(), now).unwrap();
            assert_eq!(
                second.ca, first.ca,
                "{missing}: a CA instalada no celular fica"
            );
            assert_eq!(std::fs::read(dir.path().join("ca.crt")).unwrap(), ca_bytes);
            assert_ne!(second.server.cert_pem, first.server.cert_pem, "{missing}");
            assert!(TlsPaths::in_dir(dir.path()).all_present());
        }
    }

    #[test]
    fn ca_ilegivel_regenera_tudo() {
        let dir = tempfile::tempdir().unwrap();
        let now = OffsetDateTime::now_utc();
        let first = load_or_create(dir.path(), &ips(), now).unwrap();
        std::fs::write(dir.path().join("ca.key"), "lixo").unwrap();
        let second = load_or_create(dir.path(), &ips(), now).unwrap();
        assert_ne!(second.ca, first.ca);
    }

    #[test]
    fn ca_sem_chave_regenera_tudo() {
        let dir = tempfile::tempdir().unwrap();
        let now = OffsetDateTime::now_utc();
        let first = load_or_create(dir.path(), &ips(), now).unwrap();
        std::fs::remove_file(dir.path().join("ca.key")).unwrap();
        let second = load_or_create(dir.path(), &ips(), now).unwrap();
        assert_ne!(second.ca, first.ca);
        assert!(TlsPaths::in_dir(dir.path()).all_present());
        assert!(!dir.path().join("ca.crt.tmp").exists());
    }
}
