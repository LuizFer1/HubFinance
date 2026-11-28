//! Fontes embutidas na interface GPUI: Inter Hub e Phosphor.
//!
//! Os bytes sao os mesmos da iced (`ui::fonts`); aqui so se registram no sistema de texto do
//! GPUI. Sem registrar, o GPUI cairia na fonte do sistema e os digitos deixariam de ser
//! tabulares (a Inter Hub tem `tnum` congelado; uma Inter instalada, nao).

use std::borrow::Cow;

use gpui::{App, FontWeight};

/// "Inter Hub", nao "Inter": a familia foi renomeada ao congelar `tnum`, para nunca disputar
/// com uma Inter instalada no sistema.
pub const FAMILY: &str = "Inter Hub";
// Usado pelos icones de categoria (Tarefa 9).
#[allow(dead_code)] // usado na Tarefa 9
pub const ICON_FAMILY: &str = "Phosphor";

const LIGHT: &[u8] = include_bytes!("../../assets/fonts/InterHub-Light.ttf");
const REGULAR: &[u8] = include_bytes!("../../assets/fonts/InterHub-Regular.ttf");
const MEDIUM: &[u8] = include_bytes!("../../assets/fonts/InterHub-Medium.ttf");
const SEMIBOLD: &[u8] = include_bytes!("../../assets/fonts/InterHub-SemiBold.ttf");
const PHOSPHOR: &[u8] = include_bytes!("../../assets/fonts/Phosphor.ttf");

/// Registra as fontes. Falha so se um arquivo embutido for invalido; em runtime fica o log e o
/// texto cai na fonte do sistema em vez de derrubar o hub.
pub fn register(cx: &mut App) {
    let faces: Vec<Cow<'static, [u8]>> = [LIGHT, REGULAR, MEDIUM, SEMIBOLD, PHOSPHOR]
        .into_iter()
        .map(Cow::Borrowed)
        .collect();
    if let Err(error) = cx.text_system().add_fonts(faces) {
        tracing::error!(%error, "fontes embutidas nao registradas");
    }
}

/// Peso do design para o mais proximo vendorizado (Light/Regular/Medium/SemiBold). O design usa
/// 700 e 800 (badges, passos, "Desfazer"); sem `InterHub-Bold.ttf` o GPUI sintetizaria negrito
/// por cima da SemiBold, borrado. Presuncao 12 da spec: 700 e 800 viram SemiBold.
#[allow(dead_code)] // usado na Tarefa 11
pub fn weight(w: u16) -> FontWeight {
    match w {
        0..=349 => FontWeight::LIGHT,
        350..=449 => FontWeight::NORMAL,
        450..=549 => FontWeight::MEDIUM,
        _ => FontWeight::SEMIBOLD,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pesos_do_design_caem_nos_vendorizados() {
        assert_eq!(weight(300), FontWeight::LIGHT);
        assert_eq!(weight(400), FontWeight::NORMAL);
        assert_eq!(weight(500), FontWeight::MEDIUM);
        assert_eq!(weight(600), FontWeight::SEMIBOLD);
        assert_eq!(weight(700), FontWeight::SEMIBOLD);
        assert_eq!(weight(800), FontWeight::SEMIBOLD);
    }
}
