//! Onde a chave do OpenRouter mora. Windows: DPAPI do usuario (`CryptProtectData`), entao o
//! arquivo copiado para outra conta ou maquina nao abre. Outros sistemas: texto puro (so o dono
//! le, `0600` em Unix) e o painel avisa. A chave nunca vai para log, `assistant.json` nem
//! `assistant.db`.

use std::path::{Path, PathBuf};
#[cfg(test)]
use std::sync::Mutex;

pub const KEY_FILE: &str = "assistant.key";

pub trait SecretStore: Send {
    fn load(&self) -> std::io::Result<Option<String>>;
    fn save(&self, secret: &str) -> std::io::Result<()>;
    fn clear(&self) -> std::io::Result<()>;
    /// `false` fora do Windows: o painel mostra o aviso de texto puro.
    fn encrypted(&self) -> bool;
}

/// So para os testes: guarda em memoria, sem tocar em disco nem em DPAPI.
#[cfg(test)]
#[derive(Default)]
pub struct FakeSecretStore(Mutex<Option<String>>);

#[cfg(test)]
impl SecretStore for FakeSecretStore {
    fn load(&self) -> std::io::Result<Option<String>> {
        Ok(self
            .0
            .lock()
            .map_err(|_| std::io::Error::other("lock"))?
            .clone())
    }
    fn save(&self, s: &str) -> std::io::Result<()> {
        *self.0.lock().map_err(|_| std::io::Error::other("lock"))? = Some(s.to_string());
        Ok(())
    }
    fn clear(&self) -> std::io::Result<()> {
        *self.0.lock().map_err(|_| std::io::Error::other("lock"))? = None;
        Ok(())
    }
    fn encrypted(&self) -> bool {
        true
    }
}

/// Arquivo `assistant.key` na pasta de dados. O conteudo e o que `protect` devolve: um blob
/// DPAPI no Windows, o texto puro nos outros sistemas.
pub struct FileSecretStore {
    path: PathBuf,
}

/// O armazenamento de verdade do sistema, na pasta de dados do hub.
pub fn system(data_dir: &Path) -> Box<dyn SecretStore> {
    Box::new(FileSecretStore {
        path: data_dir.join(KEY_FILE),
    })
}

impl SecretStore for FileSecretStore {
    fn load(&self) -> std::io::Result<Option<String>> {
        let bytes = match std::fs::read(&self.path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        let plain = unprotect(&bytes)?;
        String::from_utf8(plain)
            .map(Some)
            .map_err(|_| std::io::Error::other("chave salva ilegivel"))
    }

    fn save(&self, s: &str) -> std::io::Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        write_private(&self.path, &protect(s.as_bytes())?)
    }

    fn clear(&self) -> std::io::Result<()> {
        match std::fs::remove_file(&self.path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }

    fn encrypted(&self) -> bool {
        cfg!(windows)
    }
}

/// `.tmp` + `rename`, como `write_atomic`: uma queda no meio nao deixa a chave pela metade (e
/// o usuario ficaria sem chave e sem saber). Em Unix o arquivo nasce `0600`, entao nunca
/// existe uma janela em que outro usuario da maquina possa le-lo; no Windows quem protege e o
/// DPAPI (e a ACL do perfil).
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;

    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&tmp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&tmp, path)
}

#[cfg(windows)]
fn protect(data: &[u8]) -> std::io::Result<Vec<u8>> {
    dpapi(data, true)
}

#[cfg(windows)]
fn unprotect(data: &[u8]) -> std::io::Result<Vec<u8>> {
    dpapi(data, false)
}

/// `CryptProtectData`/`CryptUnprotectData` com o escopo do usuario atual e sem nenhuma janela
/// (`CRYPTPROTECT_UI_FORBIDDEN`): numa sessao sem interface a chamada falha em vez de travar.
#[cfg(windows)]
fn dpapi(data: &[u8], encrypt: bool) -> std::io::Result<Vec<u8>> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
    };

    let input = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(data.len()).map_err(std::io::Error::other)?,
        pbData: data.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    // SAFETY: `input` aponta para `data`, vivo durante a chamada, e a API so le dele; `output`
    // e preenchido pela API e liberado com `LocalFree` logo depois de copiado. Os demais
    // ponteiros sao opcionais e ficam nulos.
    let ok = unsafe {
        if encrypt {
            CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        }
    };
    if ok == 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: com sucesso a API devolve `cbData` bytes validos em `pbData`.
    let out = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    // SAFETY: memoria alocada pela DPAPI com `LocalAlloc`; nao ha mais referencia a ela.
    unsafe { LocalFree(output.pbData.cast()) };
    Ok(out)
}

#[cfg(not(windows))]
fn protect(data: &[u8]) -> std::io::Result<Vec<u8>> {
    Ok(data.to_vec())
}

#[cfg(not(windows))]
fn unprotect(data: &[u8]) -> std::io::Result<Vec<u8>> {
    Ok(data.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_guarda_e_apaga() {
        let s = FakeSecretStore::default();
        assert_eq!(s.load().unwrap(), None);
        s.save("sk-or-123").unwrap();
        assert_eq!(s.load().unwrap().as_deref(), Some("sk-or-123"));
        s.clear().unwrap();
        assert_eq!(s.load().unwrap(), None);
    }

    #[test]
    fn arquivo_nunca_guarda_a_chave_em_claro_no_windows() {
        let dir = tempfile::tempdir().unwrap();
        let s = system(dir.path());
        assert_eq!(s.load().unwrap(), None, "sem arquivo = sem chave");
        s.save("sk-or-segredo").unwrap();
        let raw = std::fs::read(dir.path().join(KEY_FILE)).unwrap();
        if cfg!(windows) {
            assert!(!raw.windows(13).any(|w| w == b"sk-or-segredo"));
        }
        assert_eq!(s.encrypted(), cfg!(windows));
        assert_eq!(s.load().unwrap().as_deref(), Some("sk-or-segredo"));
        // Trocar a chave regrava o arquivo; nao sobra o `.tmp` nem a chave antiga.
        s.save("sk-or-nova").unwrap();
        assert_eq!(s.load().unwrap().as_deref(), Some("sk-or-nova"));
        assert!(!dir.path().join("assistant.key.tmp").exists());
        s.clear().unwrap();
        assert!(!dir.path().join(KEY_FILE).exists());
        assert_eq!(s.load().unwrap(), None);
        // Apagar o que ja nao existe nao e erro.
        s.clear().unwrap();
    }

    #[test]
    fn arquivo_corrompido_vira_erro_e_nao_panico() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(KEY_FILE), [0xff, 0xfe, 0x00, 0x80]).unwrap();
        // No Windows nao e um blob DPAPI valido; nos outros nao e UTF-8.
        assert!(system(dir.path()).load().is_err());
    }

    #[test]
    fn cria_a_pasta_de_dados_se_nao_existe() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("a").join("b");
        let s = system(&nested);
        s.save("sk-or-x").unwrap();
        assert_eq!(s.load().unwrap().as_deref(), Some("sk-or-x"));
    }

    #[cfg(unix)]
    #[test]
    fn em_unix_so_o_dono_le_o_arquivo() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let s = system(dir.path());
        s.save("sk-or-x").unwrap();
        let mode = std::fs::metadata(dir.path().join(KEY_FILE))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
