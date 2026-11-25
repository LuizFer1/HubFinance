//! Atalhos de teclado. So chegam aqui as teclas que nenhum widget consumiu
//! (`keyboard::listen`): o que o campo de texto focado usa (digitar, setas, Esc para sair do
//! campo) nunca vira atalho. A decisao e uma funcao pura do estado, testavel sem janela.

use iced::keyboard::key::{Code, Named, Physical};
use iced::keyboard::{self, Key, Modifiers};
use iced::{Subscription, Task};

use super::app::{App, Message, Screen};
use super::shell;

/// Tecla pressionada que sobrou para a janela, com o que o atalho precisa saber.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyPress {
    pub key: Key,
    pub physical: Physical,
    pub modifiers: Modifiers,
    pub repeat: bool,
}

pub fn subscription() -> Subscription<Message> {
    keyboard::listen().filter_map(|event| match event {
        keyboard::Event::KeyPressed {
            key,
            physical_key,
            modifiers,
            repeat,
            ..
        } => Some(Message::KeyPressed(KeyPress {
            key,
            physical: physical_key,
            modifiers,
            repeat,
        })),
        _ => None,
    })
}

/// O que a tecla faz agora, ou `None`.
///
/// - Esc fecha o que estiver por cima: dialogo de remocao, previa do assistente, menu de
///   categoria, painel do assistente (nessa ordem). Menu e painel so na tela deles: o painel
///   continua "aberto" ao trocar de tela, e Esc em outra tela o fecharia sem ninguem ver.
/// - Com dialogo aberto, so o Esc: atalho por baixo do scrim mexeria numa tela escondida.
/// - Tab e Shift+Tab andam entre os campos.
/// - Ctrl+1..N troca de tela na ordem da navegacao.
/// - Alt+setas mudam o mes, onde ha seletor de mes.
/// - Ctrl+H alterna ocultar valores, onde o olho existe.
///
/// Repeticao (tecla segurada) so vale para Tab e para as setas: Ctrl+H segurado gravaria o
/// `ui.json` a cada repeticao e piscaria os valores.
pub fn shortcut(app: &App, press: &KeyPress) -> Option<Message> {
    let mods = press.modifiers;
    if press.key == Key::Named(Named::Escape) {
        return escape(app);
    }
    if app.confirm_remove.is_some() || app.envelope_preview.is_some() {
        return None;
    }
    if press.key == Key::Named(Named::Tab) && !mods.control() && !mods.alt() {
        return Some(if mods.shift() {
            Message::FocusPrevious
        } else {
            Message::FocusNext
        });
    }
    if mods.alt() && !mods.control() && !mods.shift() {
        let month_screen = matches!(app.screen, Screen::Dashboard | Screen::Transactions);
        return match &press.key {
            Key::Named(Named::ArrowLeft) if month_screen => Some(Message::PrevMonth),
            Key::Named(Named::ArrowRight) if month_screen => Some(Message::NextMonth),
            _ => None,
        };
    }
    if mods.control() && !mods.alt() && !mods.shift() && !press.repeat {
        if let Some(n) = digit(press) {
            let entries = shell::nav_entries(app);
            let screen = entries.get(n.checked_sub(1)?)?.screen;
            // A mesma tela: nada a fazer (e nao manda a remocao pendente antes da hora).
            return (screen != app.screen).then_some(Message::SelectScreen(screen));
        }
        let eye = matches!(app.screen, Screen::Dashboard | Screen::Analysis);
        if eye && press.key.to_latin(press.physical) == Some('h') {
            return Some(Message::ToggleHideValues);
        }
    }
    None
}

fn escape(app: &App) -> Option<Message> {
    if app.confirm_remove.is_some() {
        Some(Message::CancelRemove)
    } else if app.envelope_preview.is_some() {
        Some(Message::CloseEnvelope)
    } else if app.screen == Screen::Transactions && app.cat_menu_open {
        Some(Message::CloseCategoryMenu)
    } else if app.screen == Screen::Analysis && app.assistant_settings_open {
        // Pelo mesmo caminho do botao: grava os rascunhos validos e esquece a chave digitada.
        Some(Message::ToggleAssistantSettings)
    } else {
        None
    }
}

/// O numero da tecla (1 a 9), pela posicao fisica: em teclado AZERTY o "1" sem Shift e "&".
fn digit(press: &KeyPress) -> Option<usize> {
    let code = match press.physical {
        Physical::Code(code) => code,
        Physical::Unidentified(_) => return None,
    };
    let n = match code {
        Code::Digit1 | Code::Numpad1 => 1,
        Code::Digit2 | Code::Numpad2 => 2,
        Code::Digit3 | Code::Numpad3 => 3,
        Code::Digit4 | Code::Numpad4 => 4,
        Code::Digit5 | Code::Numpad5 => 5,
        Code::Digit6 | Code::Numpad6 => 6,
        Code::Digit7 | Code::Numpad7 => 7,
        Code::Digit8 | Code::Numpad8 => 8,
        Code::Digit9 | Code::Numpad9 => 9,
        _ => return None,
    };
    Some(n)
}

/// Tab entre os campos: so `text_input` e focavel no iced 0.14 (botao nao tem foco).
pub fn focus(next: bool) -> Task<Message> {
    if next {
        iced::widget::operation::focus_next()
    } else {
        iced::widget::operation::focus_previous()
    }
}

#[cfg(test)]
mod tests {
    use super::super::app::tests::{app_with, app_with_fixtures};
    use super::super::app::{RemoveDialog, RemoveStep};
    use super::*;
    use crate::hub::snapshot::Snapshot;
    use crate::store::erase::AreaSet;

    fn press(key: Key, physical: Code, modifiers: Modifiers) -> KeyPress {
        KeyPress {
            key,
            physical: Physical::Code(physical),
            modifiers,
            repeat: false,
        }
    }

    fn esc() -> KeyPress {
        press(Key::Named(Named::Escape), Code::Escape, Modifiers::empty())
    }

    fn ctrl_digit(n: char, code: Code) -> KeyPress {
        press(Key::Character(n.to_string().into()), code, Modifiers::CTRL)
    }

    fn ctrl_h() -> KeyPress {
        press(Key::Character("h".into()), Code::KeyH, Modifiers::CTRL)
    }

    fn alt(named: Named, code: Code) -> KeyPress {
        press(Key::Named(named), code, Modifiers::ALT)
    }

    fn tab(shift: bool) -> KeyPress {
        let mods = if shift {
            Modifiers::SHIFT
        } else {
            Modifiers::empty()
        };
        press(Key::Named(Named::Tab), Code::Tab, mods)
    }

    fn app() -> (App, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let (app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        (app, dir)
    }

    fn is(m: Option<Message>, want: &str) -> bool {
        format!("{m:?}") == want
    }

    #[test]
    fn esc_fecha_o_que_esta_por_cima_na_ordem() {
        let (mut app, _dir) = app();
        assert!(shortcut(&app, &esc()).is_none());
        app.screen = Screen::Analysis;
        app.assistant_settings_open = true;
        assert!(is(shortcut(&app, &esc()), "Some(ToggleAssistantSettings)"));
        // Painel "aberto" em outra tela: Esc nao o fecha as cegas.
        app.screen = Screen::Dashboard;
        assert!(shortcut(&app, &esc()).is_none());
        app.screen = Screen::Transactions;
        app.cat_menu_open = true;
        assert!(is(shortcut(&app, &esc()), "Some(CloseCategoryMenu)"));
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
        assert!(shortcut(&app, &ctrl_digit('2', Code::Digit2)).is_none());
        assert!(shortcut(&app, &ctrl_h()).is_none());
        assert!(shortcut(&app, &tab(false)).is_none());
        assert!(shortcut(&app, &alt(Named::ArrowLeft, Code::ArrowLeft)).is_none());
    }

    #[test]
    fn ctrl_numero_segue_a_ordem_da_navegacao() {
        let (mut app, _dir) = app();
        let entries = shell::nav_entries(&app);
        for (i, entry) in entries.iter().enumerate() {
            let n = char::from(b'1' + u8::try_from(i).unwrap());
            let code = [
                Code::Digit1,
                Code::Digit2,
                Code::Digit3,
                Code::Digit4,
                Code::Digit5,
            ][i];
            app.screen = if entry.screen == Screen::Dashboard {
                Screen::Connection
            } else {
                Screen::Dashboard
            };
            assert!(is(
                shortcut(&app, &ctrl_digit(n, code)),
                &format!("Some(SelectScreen({:?}))", entry.screen)
            ));
        }
        // Teclado numerico, alem da fileira de cima.
        app.screen = Screen::Dashboard;
        assert!(is(
            shortcut(&app, &ctrl_digit('3', Code::Numpad3)),
            "Some(SelectScreen(Reserves))"
        ));
        // Alem da ultima tela, a tela atual, sem Ctrl ou segurado: nada.
        assert!(shortcut(&app, &ctrl_digit('6', Code::Digit6)).is_none());
        assert!(shortcut(&app, &ctrl_digit('1', Code::Digit1)).is_none());
        assert!(
            shortcut(
                &app,
                &press(Key::Character("2".into()), Code::Digit2, Modifiers::empty())
            )
            .is_none()
        );
        let held = KeyPress {
            repeat: true,
            ..ctrl_digit('2', Code::Digit2)
        };
        assert!(shortcut(&app, &held).is_none());
    }

    #[test]
    fn alt_setas_so_onde_ha_seletor_de_mes() {
        let (mut app, _dir) = app();
        let left = alt(Named::ArrowLeft, Code::ArrowLeft);
        let right = alt(Named::ArrowRight, Code::ArrowRight);
        for screen in [Screen::Dashboard, Screen::Transactions] {
            app.screen = screen;
            assert!(is(shortcut(&app, &left), "Some(PrevMonth)"));
            assert!(is(shortcut(&app, &right), "Some(NextMonth)"));
        }
        for screen in [Screen::Reserves, Screen::Analysis, Screen::Connection] {
            app.screen = screen;
            assert!(shortcut(&app, &left).is_none());
        }
    }

    #[test]
    fn ctrl_h_so_onde_o_olho_existe_e_sem_repeticao() {
        let (mut app, _dir) = app();
        for screen in [Screen::Dashboard, Screen::Analysis] {
            app.screen = screen;
            assert!(is(shortcut(&app, &ctrl_h()), "Some(ToggleHideValues)"));
        }
        for screen in [Screen::Transactions, Screen::Reserves, Screen::Connection] {
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
        let cyrillic = press(Key::Character("р".into()), Code::KeyH, Modifiers::CTRL);
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
        let _ =
            super::super::app::update(&mut app, Message::KeyPressed(ctrl_digit('2', Code::Digit2)));
        assert_eq!(app.screen, Screen::Transactions);
        let _ = super::super::app::update(&mut app, Message::KeyPressed(tab(false)));
        let before = app.month_idx;
        let _ = super::super::app::update(
            &mut app,
            Message::KeyPressed(alt(Named::ArrowLeft, Code::ArrowLeft)),
        );
        assert_eq!(app.month_idx, before - 1);
    }
}
