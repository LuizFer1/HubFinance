//! Uma instancia por pasta de dados: mutex nomeado conferido antes de abrir o banco ou a
//! porta. A segunda execucao, em vez de so sair (como o Tex), pede a primeira para mostrar a
//! janela por um evento nomeado — senao o atalho do menu Iniciar nao faria nada enquanto o
//! hub roda.

use std::path::Path;

use sha2::{Digest, Sha256};

/// `HubFinance-` + 16 hex do SHA-256 do caminho normalizado. Hash, e nao o caminho: nomes de
/// objeto do kernel nao aceitam `\` e tem limite de tamanho; normalizado para a mesma pasta
/// escrita de dois jeitos dar a mesma instancia.
pub fn instance_key(data_dir: &Path) -> String {
    let mut path = data_dir.to_string_lossy().replace('\\', "/").to_lowercase();
    while path.len() > 1 && path.ends_with('/') {
        path.pop();
    }
    let digest = Sha256::digest(path.as_bytes());
    format!("HubFinance-{}", &hex::encode(digest)[..16])
}

/// `Local\`: por sessao de usuario. Dois usuarios logados tem pastas de dados diferentes e
/// hubs diferentes.
pub fn mutex_name(key: &str) -> String {
    format!("Local\\{key}")
}

pub fn event_name(key: &str) -> String {
    format!("Local\\{key}-abrir")
}

#[derive(Debug, thiserror::Error)]
pub enum InstanceError {
    #[error("ja ha um HubFinance aberto para esta pasta de dados")]
    AlreadyRunning,
    #[error("{0}")]
    Os(String),
}

// TEMPORARIO: `main.rs` passa a usar na Tarefa 10.
#[cfg(windows)]
#[allow(unused_imports)]
pub use win::{Instance, acquire, notify_running};

#[cfg(not(windows))]
pub struct Instance;

#[cfg(not(windows))]
impl Instance {
    pub fn watch(&self, _tx: crate::desktop::EventSender) {}
}

#[cfg(not(windows))]
pub fn acquire(_data_dir: &Path) -> Result<Instance, InstanceError> {
    Ok(Instance)
}

#[cfg(not(windows))]
pub fn notify_running(_data_dir: &Path) {}

#[cfg(windows)]
mod win {
    use std::path::Path;
    use std::ptr;

    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, WAIT_OBJECT_0,
    };
    use windows_sys::Win32::System::Threading::{
        CreateEventW, CreateMutexW, EVENT_MODIFY_STATE, INFINITE, OpenEventW, SetEvent,
        WaitForSingleObject,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{ASFW_ANY, AllowSetForegroundWindow};

    use super::{InstanceError, event_name, instance_key, mutex_name};
    use crate::desktop::{DesktopEvent, EventSender, wide};

    /// `HANDLE` e um ponteiro cru e por isso nao e `Send`; o kernel nao se importa com a
    /// thread que espera no evento.
    struct SendHandle(HANDLE);
    // SAFETY: handle de kernel e valido em qualquer thread do processo.
    unsafe impl Send for SendHandle {}

    /// Vive ate o processo acabar. Sem `Drop`: a thread de espera usa o evento, e o kernel
    /// fecha os dois handles na saida.
    pub struct Instance {
        _mutex: HANDLE,
        event: HANDLE,
    }

    pub fn acquire(data_dir: &Path) -> Result<Instance, InstanceError> {
        let key = instance_key(data_dir);
        let mutex_name = wide(&mutex_name(&key));
        // SAFETY: nome valido; o handle (ou NULL) e conferido abaixo.
        let mutex = unsafe { CreateMutexW(ptr::null(), 0, mutex_name.as_ptr()) };
        // `GetLastError` logo depois: qualquer chamada no meio o sobrescreveria.
        // SAFETY: so le o erro da thread atual.
        let last = unsafe { GetLastError() };
        if mutex.is_null() {
            return Err(InstanceError::Os(format!("CreateMutexW: erro {last}")));
        }
        if last == ERROR_ALREADY_EXISTS {
            // SAFETY: handle valido, nosso, nao usado depois.
            unsafe { CloseHandle(mutex) };
            return Err(InstanceError::AlreadyRunning);
        }
        let event_name = wide(&event_name(&key));
        // Auto-reset (`bManualReset = 0`): cada `SetEvent` acorda a espera uma vez.
        // SAFETY: nome valido; NULL e conferido.
        let event = unsafe { CreateEventW(ptr::null(), 0, 0, event_name.as_ptr()) };
        if event.is_null() {
            // SAFETY: so le o erro da thread atual; o mutex e nosso e nao e usado depois.
            let err = unsafe { GetLastError() };
            unsafe { CloseHandle(mutex) };
            return Err(InstanceError::Os(format!("CreateEventW: erro {err}")));
        }
        Ok(Instance {
            _mutex: mutex,
            event,
        })
    }

    impl Instance {
        /// Thread que dorme no evento e, a cada sinal, pede a janela. Termina quando a UI solta
        /// o receptor (o processo esta saindo).
        pub fn watch(&self, tx: EventSender) {
            let event = SendHandle(self.event);
            let spawned = std::thread::Builder::new()
                .name("instancia".into())
                .spawn(move || {
                    let event = event;
                    loop {
                        // SAFETY: handle valido pelo tempo de vida do processo.
                        if unsafe { WaitForSingleObject(event.0, INFINITE) } != WAIT_OBJECT_0 {
                            tracing::warn!("espera do evento de instancia falhou");
                            break;
                        }
                        if tx.unbounded_send(DesktopEvent::OpenWindow).is_err() {
                            break;
                        }
                    }
                });
            if let Err(e) = spawned {
                tracing::warn!("thread de instancia: {e}");
            }
        }
    }

    /// Segunda execucao: avisa a primeira e volta. Se o evento ainda nao existe (a primeira
    /// esta entre criar o mutex e o evento), so volta — janela de milissegundos.
    pub fn notify_running(data_dir: &Path) {
        let name = wide(&event_name(&instance_key(data_dir)));
        // SAFETY: nome valido; NULL e conferido.
        let event = unsafe { OpenEventW(EVENT_MODIFY_STATE, 0, name.as_ptr()) };
        if event.is_null() {
            return;
        }
        // Este processo e o que a pessoa acabou de abrir, entao tem o direito de foco; cede-o
        // para a primeira instancia conseguir trazer a janela com `SetForegroundWindow`.
        // SAFETY: chamadas sem ponteiros; o handle e fechado em seguida.
        unsafe {
            AllowSetForegroundWindow(ASFW_ANY);
            SetEvent(event);
            CloseHandle(event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn chave_estavel_e_insensivel_a_caixa_separador_e_barra_final() {
        let a = instance_key(Path::new(r"C:\Users\Ana\AppData\Local\HubFinance\data"));
        assert_eq!(
            a,
            instance_key(Path::new(r"c:\users\ana\appdata\local\hubfinance\data"))
        );
        assert_eq!(
            a,
            instance_key(Path::new("C:/Users/Ana/AppData/Local/HubFinance/data/"))
        );
        assert_ne!(
            a,
            instance_key(Path::new(r"C:\Users\Ana\AppData\Local\HubFinance\outra"))
        );
    }

    #[test]
    fn chave_serve_de_nome_de_objeto_do_kernel() {
        let key = instance_key(Path::new("/x/y"));
        assert!(key.starts_with("HubFinance-"), "{key}");
        assert_eq!(key.len(), "HubFinance-".len() + 16);
        assert!(key.len() <= 40, "{key}");
        assert!(
            key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'),
            "{key}"
        );
        assert_eq!(mutex_name(&key), format!("Local\\{key}"));
        assert_eq!(event_name(&key), format!("Local\\{key}-abrir"));
    }
}
