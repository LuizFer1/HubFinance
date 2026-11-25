//! Onde a chave do OpenRouter mora. Por ora so o contrato e o fake dos testes; a Issue 05
//! acrescenta o armazenamento de verdade (DPAPI no Windows). A chave nunca vai para log,
//! `assistant.json` nem `assistant.db`.

use std::sync::Mutex;

pub trait SecretStore: Send {
    fn load(&self) -> std::io::Result<Option<String>>;
    fn save(&self, secret: &str) -> std::io::Result<()>;
    fn clear(&self) -> std::io::Result<()>;
    /// `false` fora do Windows: o painel mostra o aviso de texto puro.
    #[allow(dead_code)] // usado pela UI (Task 5.2)
    fn encrypted(&self) -> bool;
}

#[derive(Default)]
pub struct FakeSecretStore(Mutex<Option<String>>);

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
}
