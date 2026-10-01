//! Arquivos de `tls/`: PEMs da CA e do servidor e o `server.json` com o que foi emitido.

use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use super::TlsError;
use super::ca::Ca;
use super::server_cert::ServerCert;

pub struct TlsPaths {
    pub ca_cert: PathBuf,
    pub ca_key: PathBuf,
    pub server_cert: PathBuf,
    pub server_key: PathBuf,
    pub server_meta: PathBuf,
}

/// O hub escreveu o certificado, entao sabe o que pos nele: comparar com isto evita reler o
/// SAN com `x509-parser`.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerMeta {
    ips: Vec<Ipv4Addr>,
    not_after: String,
}

impl TlsPaths {
    pub fn in_dir(dir: &Path) -> TlsPaths {
        TlsPaths {
            ca_cert: dir.join("ca.crt"),
            ca_key: dir.join("ca.key"),
            server_cert: dir.join("server.crt"),
            server_key: dir.join("server.key"),
            server_meta: dir.join("server.json"),
        }
    }

    #[cfg(test)]
    pub fn all_present(&self) -> bool {
        self.all().iter().all(|p| p.is_file())
    }

    fn all(&self) -> [&Path; 5] {
        [
            &self.ca_cert,
            &self.ca_key,
            &self.server_cert,
            &self.server_key,
            &self.server_meta,
        ]
    }
}

pub fn read_ca(paths: &TlsPaths) -> Result<Ca, TlsError> {
    Ok(Ca {
        cert_pem: std::fs::read_to_string(&paths.ca_cert)?,
        key_pem: std::fs::read_to_string(&paths.ca_key)?,
    })
}

pub fn read_server(paths: &TlsPaths) -> Result<ServerCert, TlsError> {
    let meta: ServerMeta = serde_json::from_str(&std::fs::read_to_string(&paths.server_meta)?)?;
    let not_after = OffsetDateTime::parse(&meta.not_after, &Rfc3339)
        .map_err(|e| TlsError::Time(e.to_string()))?;
    Ok(ServerCert {
        cert_pem: std::fs::read_to_string(&paths.server_cert)?,
        key_pem: std::fs::read_to_string(&paths.server_key)?,
        ips: meta.ips,
        not_after,
    })
}

pub fn write_ca(paths: &TlsPaths, ca: &Ca) -> Result<(), TlsError> {
    write_atomic(&paths.ca_cert, &ca.cert_pem)?;
    write_secret(&paths.ca_key, &ca.key_pem)
}

pub fn write_server(paths: &TlsPaths, cert: &ServerCert) -> Result<(), TlsError> {
    let meta = ServerMeta {
        ips: cert.ips.clone(),
        not_after: cert
            .not_after
            .format(&Rfc3339)
            .map_err(|e| TlsError::Time(e.to_string()))?,
    };
    write_atomic(&paths.server_cert, &cert.cert_pem)?;
    write_secret(&paths.server_key, &cert.key_pem)?;
    // Por ultimo: se a escrita parar antes, o `server.json` antigo (ou ausente) faz a proxima
    // execucao reemitir, em vez de acreditar num certificado que nao foi gravado.
    write_atomic(&paths.server_meta, &serde_json::to_string_pretty(&meta)?)?;
    Ok(())
}

/// Apaga o que existir; arquivo ausente nao e erro.
pub fn remove_all(paths: &TlsPaths) -> Result<(), TlsError> {
    for path in paths.all() {
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

/// `.tmp` + `rename`: uma queda de energia no meio nao deixa arquivo pela metade (PEM aqui,
/// `ui.json` em `config`). O `rename` troca o arquivo inteiro de uma vez, inclusive no Windows.
pub(crate) fn write_atomic(path: &Path, contents: &str) -> std::io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, contents)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Chaves privadas: no Windows quem protege e a ACL do perfil do usuario (nao ha `chmod`); em
/// Unix, so o dono le.
fn write_secret(path: &Path, contents: &str) -> Result<(), TlsError> {
    write_atomic(path, contents)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}
