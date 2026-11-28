//! Linha de comando e a decisao do boot: sair, so bandeja ou janela. Puro.

/// O valor `Run` lanca o hub com este argumento; e o que distingue "o Windows me abriu" de
/// "a pessoa me abriu".
pub const AUTOSTART_FLAG: &str = "--autostart";

/// Abre so na bandeja, sem janela, mesmo com "Iniciar com o Windows" desligado: e o pedido
/// explicito de quem lanca (atalho, script). `--autostart` e o Windows pedindo e depende da
/// opcao; este e a pessoa pedindo e nao depende.
pub const BACKGROUND_FLAG: &str = "--background";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Launch {
    pub autostart: bool,
    pub background: bool,
}

impl Launch {
    pub fn from_args<I: IntoIterator<Item = String>>(args: I) -> Launch {
        let mut launch = Launch::default();
        for arg in args {
            launch.autostart |= arg == AUTOSTART_FLAG;
            launch.background |= arg == BACKGROUND_FLAG;
        }
        launch
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootMode {
    /// `--autostart` com a opcao desligada: sai antes de log, mutex, banco ou janela. E o que
    /// faz "desligar a opcao" valer mesmo com um valor `Run` esquecido pela desinstalacao.
    Exit,
    /// Aberto pelo Windows: hub no ar, so o icone.
    TrayOnly,
    WithWindow,
}

/// `autostart` e o estado da opcao no sistema (`None` = sistema sem a opcao).
pub fn boot_mode(launch: Launch, autostart: Option<bool>) -> BootMode {
    match (launch.autostart, autostart) {
        (true, Some(false)) => BootMode::Exit,
        (true, Some(true)) => BootMode::TrayOnly,
        // `--background` sozinho nao olha a opcao: so o `--autostart` e que se cala quando a
        // opcao esta desligada (o valor `Run` esquecido), e ele ja foi decidido acima.
        _ if launch.background => BootMode::TrayOnly,
        _ => BootMode::WithWindow,
    }
}

/// Sem bandeja (nao subiu, ou sistema sem ela) a janela abre sempre: ela e a unica porta.
pub fn opens_window(mode: BootMode, has_tray: bool) -> bool {
    mode == BootMode::WithWindow || !has_tray
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Launch {
        Launch::from_args(list.iter().map(|s| s.to_string()))
    }

    #[test]
    fn autostart_em_qualquer_posicao() {
        assert!(!args(&[]).autostart);
        assert!(args(&["--autostart"]).autostart);
        assert!(args(&["x", "--autostart", "y"]).autostart);
        assert!(!args(&["--autostart=1", "-autostart"]).autostart);
    }

    #[test]
    fn modo_de_abertura() {
        let auto = Launch {
            autostart: true,
            ..Launch::default()
        };
        let normal = Launch::default();
        assert_eq!(boot_mode(auto, Some(false)), BootMode::Exit);
        assert_eq!(boot_mode(auto, Some(true)), BootMode::TrayOnly);
        // Sistema sem a opcao nunca sai: --autostart la seria um atalho feito a mao.
        assert_eq!(boot_mode(auto, None), BootMode::WithWindow);
        assert_eq!(boot_mode(normal, Some(false)), BootMode::WithWindow);
        assert_eq!(boot_mode(normal, Some(true)), BootMode::WithWindow);
        assert_eq!(boot_mode(normal, None), BootMode::WithWindow);
    }

    #[test]
    fn background_abre_so_na_bandeja_mesmo_com_a_opcao_desligada() {
        let bg = args(&["x", "--background"]);
        assert!(bg.background && !bg.autostart);
        assert!(!args(&["--background=1"]).background);
        for option in [None, Some(false), Some(true)] {
            assert_eq!(boot_mode(bg, option), BootMode::TrayOnly);
        }
        // O `--autostart` com a opcao desligada continua saindo, mesmo junto do outro.
        let both = args(&["--autostart", "--background"]);
        assert_eq!(boot_mode(both, Some(false)), BootMode::Exit);
        // Sem bandeja nao ha como voltar: a janela abre.
        assert!(opens_window(boot_mode(bg, None), false));
        assert!(!opens_window(boot_mode(bg, None), true));
    }

    #[test]
    fn so_bandeja_exige_bandeja() {
        assert!(!opens_window(BootMode::TrayOnly, true));
        assert!(
            opens_window(BootMode::TrayOnly, false),
            "sem bandeja nao haveria como voltar"
        );
        assert!(opens_window(BootMode::WithWindow, true));
        assert!(opens_window(BootMode::WithWindow, false));
    }
}
