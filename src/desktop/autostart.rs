//! "Iniciar com o Windows": valor `HubFinance` em `HKCU\...\Run`, gravado e removido pelo
//! proprio programa (por usuario, sem administrador). O registro e a fonte da verdade do
//! estado; `UiPrefs.autostart` so lembra que a pessoa desligou de proposito.

use std::path::Path;

use super::args::AUTOSTART_FLAG;

// Fora do Windows nada grava o registro: o valor e so do caminho Windows (e dos testes).
#[cfg_attr(not(windows), allow(dead_code, reason = "so o codigo Windows usa"))]
pub const VALUE_NAME: &str = "HubFinance";
#[cfg_attr(not(windows), allow(dead_code, reason = "so o codigo Windows usa"))]
pub const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

/// Entre aspas: a pasta de instalacao tem espacos, e sem elas o Windows tentaria
/// `C:\Program` com o resto como argumentos.
#[cfg_attr(not(windows), allow(dead_code, reason = "so o codigo Windows usa"))]
pub fn run_value(exe: &Path) -> String {
    format!("\"{}\" {AUTOSTART_FLAG}", exe.display())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plan {
    /// Grava (ou regrava) o valor com o executavel atual.
    Register,
    Leave,
}

/// Decisao do boot. `available` = o sistema tem a opcao; `dev` = build de desenvolvimento ou
/// `HUBFINANCE_DATA_DIR` definido (um `cargo run` nunca pode por `target\` no `Run`).
/// Nunca remove: desligar e sempre acao da pessoa.
pub fn plan(pref: Option<bool>, available: bool, dev: bool) -> Plan {
    if !available || dev || pref == Some(false) {
        Plan::Leave
    } else {
        Plan::Register
    }
}

/// Estado da opcao no sistema; `None` = sistema sem a opcao.
#[cfg(windows)]
pub fn is_enabled() -> Option<bool> {
    Some(win::is_enabled())
}

#[cfg(not(windows))]
pub fn is_enabled() -> Option<bool> {
    None
}

#[cfg(windows)]
pub fn set_enabled(enabled: bool) -> Result<(), String> {
    win::set_enabled(enabled)
}

#[cfg(not(windows))]
pub fn set_enabled(_enabled: bool) -> Result<(), String> {
    Err("sem suporte neste sistema".into())
}

#[cfg(windows)]
mod win {
    use std::ptr;

    use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
        RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegQueryValueExW, RegSetValueExW,
    };

    use super::{RUN_KEY, VALUE_NAME, run_value};
    use crate::desktop::wide;

    struct Key(HKEY);

    impl Drop for Key {
        fn drop(&mut self) {
            // SAFETY: handle aberto por `open` e nunca fechado antes.
            unsafe { RegCloseKey(self.0) };
        }
    }

    /// `Create`, nao `Open`: a chave `Run` existe em todo perfil, mas criar e idempotente e
    /// dispensa um segundo caminho de erro.
    fn open(sam: u32) -> Result<Key, String> {
        let sub = wide(RUN_KEY);
        let mut hkey: HKEY = ptr::null_mut();
        // SAFETY: ponteiros validos pelo tempo da chamada; `hkey` recebe o handle.
        let rc = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                sub.as_ptr(),
                0,
                ptr::null(),
                REG_OPTION_NON_VOLATILE,
                sam,
                ptr::null(),
                &mut hkey,
                ptr::null_mut(),
            )
        };
        if rc != ERROR_SUCCESS {
            return Err(format!("RegCreateKeyExW: erro {rc}"));
        }
        Ok(Key(hkey))
    }

    pub fn is_enabled() -> bool {
        let Ok(key) = open(KEY_QUERY_VALUE) else {
            return false;
        };
        let name = wide(VALUE_NAME);
        let mut kind = 0u32;
        let mut size = 0u32;
        // SAFETY: sem buffer de dados (NULL) o Windows so devolve tipo e tamanho.
        let rc = unsafe {
            RegQueryValueExW(
                key.0,
                name.as_ptr(),
                ptr::null(),
                &mut kind,
                ptr::null_mut(),
                &mut size,
            )
        };
        rc == ERROR_SUCCESS && kind == REG_SZ
    }

    pub fn set_enabled(enabled: bool) -> Result<(), String> {
        let key = open(KEY_SET_VALUE)?;
        let name = wide(VALUE_NAME);
        let rc = if enabled {
            let exe = std::env::current_exe().map_err(|e| format!("caminho do executavel: {e}"))?;
            let value = wide(&run_value(&exe));
            let bytes =
                u32::try_from(value.len() * 2).map_err(|_| "valor longo demais".to_string())?;
            // SAFETY: `value` inclui o NUL e `bytes` e o tamanho dele em bytes.
            unsafe {
                RegSetValueExW(
                    key.0,
                    name.as_ptr(),
                    0,
                    REG_SZ,
                    value.as_ptr().cast(),
                    bytes,
                )
            }
        } else {
            // SAFETY: nome valido; valor inexistente e sucesso (idempotente).
            match unsafe { RegDeleteValueW(key.0, name.as_ptr()) } {
                ERROR_FILE_NOT_FOUND => ERROR_SUCCESS,
                rc => rc,
            }
        };
        if rc == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(format!("registro: erro {rc}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn valor_run_cita_o_exe_com_aspas_e_o_argumento() {
        let v = run_value(Path::new(
            r"C:\Users\Ana\AppData\Local\HubFinance\hub_finance.exe",
        ));
        assert_eq!(
            v,
            r#""C:\Users\Ana\AppData\Local\HubFinance\hub_finance.exe" --autostart"#
        );
        let v = run_value(Path::new(r"C:\Program Files\Hub Finance\hub_finance.exe"));
        assert!(v.starts_with("\"C:\\Program Files\\Hub Finance\\"), "{v}");
        assert!(v.ends_with("\" --autostart"), "{v}");
    }

    #[test]
    fn plano_do_boot() {
        // Release, opcao disponivel: ligado por padrao (pedido do dono) e regravado a cada boot.
        assert_eq!(plan(None, true, false), Plan::Register);
        assert_eq!(plan(Some(true), true, false), Plan::Register);
        // Quem desligou de proposito fica desligado.
        assert_eq!(plan(Some(false), true, false), Plan::Leave);
        // Desenvolvimento (debug ou HUBFINANCE_DATA_DIR): nunca registra sozinho.
        assert_eq!(plan(None, true, true), Plan::Leave);
        assert_eq!(plan(Some(true), true, true), Plan::Leave);
        // Sistema sem a opcao.
        assert_eq!(plan(None, false, false), Plan::Leave);
    }
}
