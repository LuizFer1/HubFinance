//! Tecla pressionada, no vocabulario do `ui_model` (sem tipos de toolkit).
//!
//! Cada toolkit converte a sua tecla para `KeyPress`; a decisao do atalho (`shortcut`) so
//! olha para este tipo. A resolucao "posicao fisica x caractere" mora na conversao, porque
//! e ela que sabe o que o toolkit informa: aqui `Digit` ja e o digito da posicao fisica.

use crate::ui_model::analysis;
use crate::ui_model::app::{App, Msg, Screen};
use crate::ui_model::connect;
use crate::ui_model::shell::NAV_ORDER;

/// A tecla, ja resolvida para o que os atalhos precisam saber.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Escape,
    Tab,
    ArrowLeft,
    ArrowRight,
    /// Digito 1 a 9 pela posicao fisica: em teclado AZERTY o "1" sem Shift e "&", e o
    /// atalho Ctrl+1 continua sendo a tecla do 1. No teclado numerico so conta quando a
    /// tecla produz mesmo o digito (NumLock desligado: Numpad1 e End, e Ctrl+End nao troca
    /// de tela).
    Digit(u8),
    /// O caractere latino que a tecla produz (o DIGITADO: em Dvorak, Ctrl+H e a tecla cujo
    /// "h" logico sai ali, nao a posicao fisica do H). So quando o layout nao e latino
    /// (cirilico etc.) a posicao fisica entra como recurso, para Ctrl+H seguir na tecla do
    /// H. O valor pode nao ser letra ('0', 'a com crase', '-'): quem decide o atalho
    /// compara com os caracteres que usa e ignora o resto.
    Letter(char),
    /// Qualquer outra tecla: nenhum atalho a usa.
    Other,
}

/// Tecla pressionada que sobrou para a janela, com o que o atalho precisa saber.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyPress {
    pub key: Key,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub repeat: bool,
}

/// O que a tecla faz agora, ou `None`.
///
/// - Esc fecha o que estiver por cima: dialogo de remocao, "Conectar um celular", "O que o
///   assistente vê" (que abre por cima da configuracao, pelo link dela), previa de envio a
///   nuvem (pelo caminho do "Cancelar"), "Configurar assistente" e menu de categoria (nessa
///   ordem). O menu so na tela dele.
/// - Com dialogo aberto, so o Esc: atalho por baixo do scrim mexeria numa tela escondida.
///   Limite conhecido: isso inclui o Tab, entao no "Configurar assistente" ele nao anda entre
///   porta, chave e modelo (o painel antigo andava). O foco por Tab do toolkit percorre a janela
///   inteira e levaria o foco ao campo do chat por baixo do scrim; um Tab preso ao dialogo
///   precisa de foco por id, que fica para depois.
/// - Tab e Shift+Tab andam entre os campos.
/// - Ctrl+1..N troca de tela na ordem da navegacao.
/// - Alt+setas mudam o mes, onde ha seletor de mes.
/// - Ctrl+H alterna ocultar valores, onde o olho existe.
///
/// Repeticao (tecla segurada) so vale para Tab e para as setas: Ctrl+H segurado gravaria o
/// `ui.json` a cada repeticao e piscaria os valores.
pub fn shortcut(app: &App, press: &KeyPress) -> Option<Msg> {
    if press.key == Key::Escape {
        return escape(app);
    }
    if app.any_dialog_open() {
        return None;
    }
    if press.key == Key::Tab && !press.ctrl && !press.alt {
        return Some(if press.shift {
            Msg::FocusPrevious
        } else {
            Msg::FocusNext
        });
    }
    if press.alt && !press.ctrl && !press.shift {
        let month_screen = matches!(app.screen, Screen::Dashboard | Screen::Transactions);
        return match press.key {
            Key::ArrowLeft if month_screen => Some(Msg::PrevMonth),
            Key::ArrowRight if month_screen => Some(Msg::NextMonth),
            _ => None,
        };
    }
    if press.ctrl && !press.alt && !press.shift && !press.repeat {
        if let Key::Digit(n) = press.key {
            let screen = *NAV_ORDER.get(usize::from(n).checked_sub(1)?)?;
            // A mesma tela: nada a fazer (e nao manda a remocao pendente antes da hora).
            return (screen != app.screen).then_some(Msg::SelectScreen(screen));
        }
        if app.screen.shows_values() && press.key == Key::Letter('h') {
            return Some(Msg::ToggleHideValues);
        }
    }
    None
}

fn escape(app: &App) -> Option<Msg> {
    if app.confirm_remove.is_some() {
        Some(Msg::CancelRemove)
    } else if connect::dialog_open(app) {
        // Pelo mesmo caminho do "x": cancela o codigo pendente.
        Some(Msg::ConnectClose)
    } else if app.envelope_preview.is_some() {
        Some(Msg::CloseEnvelope)
    } else if analysis::shown_consent(app).is_some() {
        // So fecha (como o scrim): um Esc nunca apaga a conversa, so o "Cancelar".
        Some(Msg::ClosePreview)
    } else if app.assistant_setup.is_some() {
        // Como o "x": nada e gravado e a chave digitada e esquecida.
        Some(Msg::CloseAssistantSetup)
    } else if app.screen == Screen::Transactions && app.cat_menu_open {
        Some(Msg::CloseCategoryMenu)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::snapshot::Snapshot;
    use crate::store::erase::AreaSet;
    use crate::ui_model::app::tests::{app_with, app_with_fixtures};
    use crate::ui_model::app::{RemoveDialog, RemoveStep, step, update};
    use crate::ui_model::effect::Effect;

    fn kp(key: Key, ctrl: bool, shift: bool, alt: bool) -> KeyPress {
        KeyPress {
            key,
            ctrl,
            shift,
            alt,
            repeat: false,
        }
    }

    fn esc() -> KeyPress {
        kp(Key::Escape, false, false, false)
    }

    fn ctrl_digit(n: u8) -> KeyPress {
        kp(Key::Digit(n), true, false, false)
    }

    fn ctrl_h() -> KeyPress {
        kp(Key::Letter('h'), true, false, false)
    }

    fn alt(key: Key) -> KeyPress {
        kp(key, false, false, true)
    }

    fn tab(shift: bool) -> KeyPress {
        kp(Key::Tab, false, shift, false)
    }

    fn app() -> (App, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let (app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        (app, dir)
    }

    fn is(m: Option<Msg>, want: &str) -> bool {
        format!("{m:?}") == want
    }

    #[test]
    fn esc_fecha_o_que_esta_por_cima_na_ordem() {
        let (mut app, _dir) = app();
        assert!(shortcut(&app, &esc()).is_none());
        app.screen = Screen::Transactions;
        app.cat_menu_open = true;
        assert!(is(shortcut(&app, &esc()), "Some(CloseCategoryMenu)"));
        // O dialogo de configuracao fica por cima do menu, e vale em qualquer tela.
        app.assistant_setup = Some(crate::ui_model::assistant_setup::AssistantSetup::open(
            &Default::default(),
        ));
        assert!(is(shortcut(&app, &esc()), "Some(CloseAssistantSetup)"));
        // O envelope abre por cima da configuracao (pelo link dela).
        app.envelope_preview = Some("ENV".into());
        assert!(is(shortcut(&app, &esc()), "Some(CloseEnvelope)"));
        app.confirm_remove = Some(RemoveDialog {
            device_id: "D1".into(),
            step: RemoveStep::Choose,
            selected: AreaSet::default(),
        });
        assert!(is(shortcut(&app, &esc()), "Some(CancelRemove)"));
    }

    #[test]
    fn com_dialogo_aberto_so_o_esc_funciona() {
        let (mut app, _dir) = app();
        app.envelope_preview = Some("ENV".into());
        assert!(shortcut(&app, &ctrl_digit(2)).is_none());
        assert!(shortcut(&app, &ctrl_h()).is_none());
        assert!(shortcut(&app, &tab(false)).is_none());
        assert!(shortcut(&app, &alt(Key::ArrowLeft)).is_none());
        // O mesmo com "Configurar assistente" aberto.
        app.envelope_preview = None;
        app.assistant_setup = Some(crate::ui_model::assistant_setup::AssistantSetup::open(
            &Default::default(),
        ));
        assert!(shortcut(&app, &ctrl_digit(2)).is_none());
        assert!(shortcut(&app, &ctrl_h()).is_none());
        assert!(shortcut(&app, &tab(false)).is_none());
    }

    #[test]
    fn esc_na_previa_de_envio_so_fecha() {
        use crate::assistant::settings::{ProviderChoice, Settings};
        use crate::assistant::view::{MessageRole, MessageView, OpenConversation};
        let (mut app, _dir) = app();
        let mut settings = Settings {
            provider: ProviderChoice::Cloud,
            ..Settings::default()
        };
        settings.cloud.enabled = true;
        app.snapshot.assistant.settings = settings;
        app.snapshot.assistant.open = Some(OpenConversation {
            id: "C1".into(),
            book: app.book,
            envelope: "ENV".into(),
            messages: vec![MessageView {
                id: "m".into(),
                role: MessageRole::User,
                content: "Resuma".into(),
                sent_to_cloud: false,
                deny_data_collection: None,
                provider: None,
                model: None,
                tokens_in: None,
                tokens_out: None,
                interrupted: false,
            }],
            needs_cloud_consent: true,
        });
        // Sem a previa aberta, Esc nao faz nada com a conversa.
        assert!(shortcut(&app, &esc()).is_none());
        app.consent_preview = Some("C1".into());
        assert!(is(shortcut(&app, &esc()), "Some(ClosePreview)"));
        assert!(shortcut(&app, &ctrl_digit(2)).is_none());
    }

    #[test]
    fn ctrl_numero_segue_a_ordem_da_navegacao() {
        let (mut app, _dir) = app();
        for (i, screen) in NAV_ORDER.iter().enumerate() {
            let n = u8::try_from(i + 1).unwrap();
            app.screen = if *screen == Screen::Dashboard {
                Screen::Connection
            } else {
                Screen::Dashboard
            };
            assert!(is(
                shortcut(&app, &ctrl_digit(n)),
                &format!("Some(SelectScreen({screen:?}))")
            ));
        }
        // A ordem nova: Aparelhos no 5 e Ajustes no 6.
        app.screen = Screen::Dashboard;
        assert!(is(
            shortcut(&app, &ctrl_digit(5)),
            "Some(SelectScreen(Connection))"
        ));
        assert!(is(
            shortcut(&app, &ctrl_digit(6)),
            "Some(SelectScreen(Settings))"
        ));
        // Teclado numerico, alem da fileira de cima.
        app.screen = Screen::Dashboard;
        assert!(is(
            shortcut(&app, &ctrl_digit(3)),
            "Some(SelectScreen(Reserves))"
        ));
        // NumLock desligado: Numpad1 com Ctrl e Ctrl+End, nao troca de tela.
        let end = kp(Key::Other, true, false, false);
        assert!(shortcut(&app, &end).is_none());
        // Alem da ultima tela, a tela atual, sem Ctrl ou segurado: nada.
        assert!(shortcut(&app, &ctrl_digit(7)).is_none());
        assert!(shortcut(&app, &ctrl_digit(1)).is_none());
        assert!(shortcut(&app, &kp(Key::Digit(2), false, false, false)).is_none());
        let held = KeyPress {
            repeat: true,
            ..ctrl_digit(2)
        };
        assert!(shortcut(&app, &held).is_none());
    }

    #[test]
    fn alt_setas_so_onde_ha_seletor_de_mes() {
        let (mut app, _dir) = app();
        let left = alt(Key::ArrowLeft);
        let right = alt(Key::ArrowRight);
        for screen in [Screen::Dashboard, Screen::Transactions] {
            app.screen = screen;
            assert!(is(shortcut(&app, &left), "Some(PrevMonth)"));
            assert!(is(shortcut(&app, &right), "Some(NextMonth)"));
        }
        for screen in [
            Screen::Reserves,
            Screen::Analysis,
            Screen::Connection,
            Screen::Settings,
        ] {
            app.screen = screen;
            assert!(shortcut(&app, &left).is_none());
        }
    }

    #[test]
    fn ctrl_h_so_onde_o_olho_existe_e_sem_repeticao() {
        let (mut app, _dir) = app();
        for screen in [
            Screen::Dashboard,
            Screen::Transactions,
            Screen::Reserves,
            Screen::Analysis,
        ] {
            app.screen = screen;
            assert!(is(shortcut(&app, &ctrl_h()), "Some(ToggleHideValues)"));
        }
        for screen in [Screen::Connection, Screen::Settings] {
            app.screen = screen;
            assert!(shortcut(&app, &ctrl_h()).is_none());
        }
        app.screen = Screen::Dashboard;
        let held = KeyPress {
            repeat: true,
            ..ctrl_h()
        };
        assert!(shortcut(&app, &held).is_none());
        // Layout nao latino: a posicao fisica do H vale.
        let cyrillic = kp(Key::Letter('h'), true, false, false);
        assert!(is(shortcut(&app, &cyrillic), "Some(ToggleHideValues)"));
    }

    #[test]
    fn tab_anda_entre_os_campos() {
        let (app, _dir) = app();
        assert!(is(shortcut(&app, &tab(false)), "Some(FocusNext)"));
        assert!(is(shortcut(&app, &tab(true)), "Some(FocusPrevious)"));
    }

    #[test]
    fn atalho_passa_pelo_update() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with_fixtures(dir.path());
        let _ = update(&mut app, Msg::KeyPressed(ctrl_digit(2)));
        assert_eq!(app.screen, Screen::Transactions);
        // Tab vira a operacao de foco (um efeito de foco); tecla sem atalho, nenhuma.
        let effects = step(&mut app, Msg::KeyPressed(tab(false)));
        assert!(matches!(effects.as_slice(), [Effect::FocusNext]));
        let effects = step(&mut app, Msg::KeyPressed(tab(true)));
        assert!(matches!(effects.as_slice(), [Effect::FocusPrevious]));
        let effects = step(
            &mut app,
            Msg::KeyPressed(kp(Key::Letter('x'), false, false, false)),
        );
        assert!(effects.is_empty());
        assert_eq!(app.screen, Screen::Transactions);
        let before = app.month_idx;
        let _ = update(&mut app, Msg::KeyPressed(alt(Key::ArrowLeft)));
        assert_eq!(app.month_idx, before - 1);
    }

    #[test]
    fn esc_fecha_o_conectar_um_celular_e_os_atalhos_param_por_baixo() {
        use crate::ui_model::connect::ConnectFlow;
        let (mut app, _dir) = app();
        app.screen = Screen::Connection;
        app.connect = Some(ConnectFlow::new(true));
        assert!(matches!(shortcut(&app, &esc()), Some(Msg::ConnectClose)));
        assert!(shortcut(&app, &ctrl_digit(1)).is_none());
        assert!(shortcut(&app, &tab(false)).is_none());
        // O fluxo em tela (sem modal) nao e dialogo: Esc nao faz nada e os atalhos valem.
        app.connect = Some(ConnectFlow::new(false));
        assert!(shortcut(&app, &esc()).is_none());
        assert!(shortcut(&app, &ctrl_digit(1)).is_some());
    }
}
