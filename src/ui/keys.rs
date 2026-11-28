//! Atalhos de teclado. So chegam aqui as teclas que nenhum widget consumiu
//! (`keyboard::listen`): o que o campo de texto focado usa (digitar, setas, Esc para sair do
//! campo) nunca vira atalho. A decisao e uma funcao pura do estado, testavel sem janela.
//!
//! Excecao conhecida: Ctrl+H alterna "Ocultar valores" mesmo com um campo focado. O Windows
//! manda o `\x08` como texto, o `text_input` o descarta por ser caractere de controle e nao
//! consome a tecla.

use iced::Subscription;
use iced::keyboard::key::{Code, Named, Physical};
use iced::keyboard::{self, Key, Modifiers};

use super::app::Msg;
use crate::ui_model::keys::{Key as MKey, KeyPress};

/// Traduz a tecla do iced para o `KeyPress` do `ui_model`. A posicao fisica e o caractere
/// sao resolvidos aqui (ver `ui_model::keys::Key`), para a decisao do atalho nao conhecer
/// o iced.
///
/// `Key::Letter` vem de `Key::to_latin` do iced: devolve o caractere digitado quando ele e
/// latino (< U+0370) e so recorre a posicao fisica em layout nao latino. Por isso o valor
/// pode nao ser letra. O conversor do GPUI deve reproduzir essa regra.
pub fn convert(key: &Key, physical: Physical, modifiers: Modifiers, repeat: bool) -> KeyPress {
    let resolved = match key {
        Key::Named(Named::Escape) => MKey::Escape,
        Key::Named(Named::Tab) => MKey::Tab,
        Key::Named(Named::ArrowLeft) => MKey::ArrowLeft,
        Key::Named(Named::ArrowRight) => MKey::ArrowRight,
        _ => match digit(key, physical) {
            Some(n) => MKey::Digit(n),
            None => match key.to_latin(physical) {
                Some(c) => MKey::Letter(c),
                None => MKey::Other,
            },
        },
    };
    KeyPress {
        key: resolved,
        ctrl: modifiers.control(),
        shift: modifiers.shift(),
        alt: modifiers.alt(),
        repeat,
    }
}

pub fn subscription() -> Subscription<Msg> {
    keyboard::listen().filter_map(|event| match event {
        keyboard::Event::KeyPressed {
            key,
            physical_key,
            modifiers,
            repeat,
            ..
        } => Some(Msg::KeyPressed(convert(
            &key,
            physical_key,
            modifiers,
            repeat,
        ))),
        _ => None,
    })
}

/// O numero da tecla (1 a 9), pela posicao fisica: em teclado AZERTY o "1" sem Shift e "&".
///
/// O teclado numerico so conta quando a tecla e mesmo o digito: com o NumLock desligado o
/// Numpad1 e End e o Numpad7 e Home, e Ctrl+End nao pode trocar de tela.
fn digit(key: &Key, physical: Physical) -> Option<u8> {
    let code = match physical {
        Physical::Code(code) => code,
        Physical::Unidentified(_) => return None,
    };
    let (n, numpad) = match code {
        Code::Digit1 => (1, false),
        Code::Digit2 => (2, false),
        Code::Digit3 => (3, false),
        Code::Digit4 => (4, false),
        Code::Digit5 => (5, false),
        Code::Digit6 => (6, false),
        Code::Digit7 => (7, false),
        Code::Digit8 => (8, false),
        Code::Digit9 => (9, false),
        Code::Numpad1 => (1, true),
        Code::Numpad2 => (2, true),
        Code::Numpad3 => (3, true),
        Code::Numpad4 => (4, true),
        Code::Numpad5 => (5, true),
        Code::Numpad6 => (6, true),
        Code::Numpad7 => (7, true),
        Code::Numpad8 => (8, true),
        Code::Numpad9 => (9, true),
        _ => return None,
    };
    if numpad && *key != Key::Character(n.to_string().into()) {
        return None;
    }
    Some(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(key: Key, physical: Code, modifiers: Modifiers) -> KeyPress {
        convert(&key, Physical::Code(physical), modifiers, false)
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

    fn tab(shift: bool) -> KeyPress {
        let mods = if shift {
            Modifiers::SHIFT
        } else {
            Modifiers::empty()
        };
        press(Key::Named(Named::Tab), Code::Tab, mods)
    }

    #[test]
    fn conversao_resolve_posicao_fisica_e_modificadores() {
        let p = ctrl_digit('&', Code::Digit1);
        assert_eq!(p.key, MKey::Digit(1));
        assert!(p.ctrl && !p.shift && !p.alt && !p.repeat);
        assert_eq!(esc().key, MKey::Escape);
        assert_eq!(tab(true).key, MKey::Tab);
        assert!(tab(true).shift);
        assert_eq!(ctrl_h().key, MKey::Letter('h'));
        let cyrillic = press(Key::Character("р".into()), Code::KeyH, Modifiers::CTRL);
        assert_eq!(cyrillic.key, MKey::Letter('h'));
        let numpad = press(Key::Character("3".into()), Code::Numpad3, Modifiers::CTRL);
        assert_eq!(numpad.key, MKey::Digit(3));
        let end = press(Key::Named(Named::End), Code::Numpad1, Modifiers::CTRL);
        assert_eq!(end.key, MKey::Other);
        let held = convert(
            &Key::Named(Named::Tab),
            Physical::Code(Code::Tab),
            Modifiers::ALT,
            true,
        );
        assert!(held.alt && held.repeat);
    }
}
