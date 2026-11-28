//! Tema do sistema (escuro ou claro), para a preferencia "Sistema" da janela.
//!
//! Lido do registro, e nao do `system::theme_changes` do iced. No Windows o winit 0.30 decide o
//! tema por `ShouldAppsUseDarkMode` (uxtheme, ordinal 132, nao documentado) sem chamar antes
//! `RefreshImmersiveColorPolicyState`, entao o `ThemeChanged` que vira `theme_changes` nao chega
//! ou chega com o valor anterior. O `AppsUseLightTheme` do registro e o que o proprio Windows
//! grava ao trocar o tema: e a fonte confiavel, e a UI o consulta a cada tique. Onde o registro
//! nao responde (fora do Windows), vale a assinatura do iced.

/// `true` = escuro; `None` = o sistema nao diz (fora do Windows, ou valor ausente).
#[cfg(all(windows, not(test)))]
pub fn system_dark() -> Option<bool> {
    win::apps_use_light_theme().map(|light| !light)
}

/// Fora do Windows, e em todo teste: teste nenhum le o registro da maquina (o resultado
/// dependeria do tema de quem roda). Quem precisa de um valor usa o `Fake`.
#[cfg(any(not(windows), test))]
pub fn system_dark() -> Option<bool> {
    None
}

/// Tema da **barra de tarefas** (`SystemUsesLightTheme`), que no Windows e independente do dos
/// aplicativos: o icone da bandeja precisa do desenho que contrasta com ela. `true` = escura;
/// `None` = o sistema nao diz (fora do Windows, ou valor ausente), tratado como escura.
#[cfg(all(windows, not(test)))]
pub fn taskbar_dark() -> Option<bool> {
    win::read_light_flag("SystemUsesLightTheme").map(|light| !light)
}

/// Fora do Windows e em teste: nunca le o registro (ver `system_dark`).
#[cfg(any(not(windows), test))]
#[cfg_attr(not(windows), allow(dead_code, reason = "so o codigo Windows usa"))]
pub fn taskbar_dark() -> Option<bool> {
    None
}

#[cfg(all(windows, not(test)))]
mod win {
    use std::ptr;

    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};

    use crate::desktop::wide;

    const PERSONALIZE_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";
    /// 0 = aplicativos no escuro; 1 = no claro. Ausente em Windows sem a opcao (anterior ao 10).
    const VALUE_NAME: &str = "AppsUseLightTheme";

    pub fn apps_use_light_theme() -> Option<bool> {
        read_light_flag(VALUE_NAME)
    }

    pub fn read_light_flag(value_name: &str) -> Option<bool> {
        let sub = wide(PERSONALIZE_KEY);
        let name = wide(value_name);
        let mut value = 0u32;
        let mut size = u32::try_from(std::mem::size_of::<u32>()).ok()?;
        // SAFETY: ponteiros validos pelo tempo da chamada; `value` tem os 4 bytes que `size`
        // declara e `RRF_RT_REG_DWORD` recusa qualquer outro tipo.
        let rc = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                sub.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_DWORD,
                ptr::null_mut(),
                (&raw mut value).cast(),
                &mut size,
            )
        };
        (rc == ERROR_SUCCESS).then_some(value != 0)
    }
}
