//! Tela Reservas: total separado, emergencia coberta, ultimos 12 meses, o card da reserva de
//! emergencia, as caixinhas, a evolucao empilhada e as movimentacoes; ou o estado vazio.
//!
//! Tudo vem de `app.reserves` (calculado em `App::rebuild`); aqui so se monta a arvore. Os
//! textos seguem o bloco `isRes` do prototipo e o trecho de `renderVals()` a partir de `RD`.

use iced::widget::{Space, canvas, column, container, row, text};
use iced::{Alignment, Element, Length};

use super::app::{App, Message};
use super::charts::{EMPTY_RESERVES_HEIGHT, EMPTY_RESERVES_WIDTH, EmptyReservesProgram};
use super::theme;
use super::{fonts, shell};

/// Largura minima da coluna de texto do estado vazio, ao lado do esqueleto.
const EMPTY_TEXT_MIN: f32 = 260.0;
const EMPTY_GAP_X: f32 = 56.0;
const EMPTY_PADDING_X: f32 = 32.0;

pub fn view(app: &App) -> Element<'_, Message> {
    let v = &app.reserves;
    if v.is_empty {
        return empty_card(app);
    }
    // TODO tarefas 10-12: resumo, emergencia, caixinhas, evolucao e movimentacoes.
    column![].width(Length::Fill).into()
}

// ---- estado vazio ----

/// Um card so: o medidor tracejado e o texto. O `flex-wrap` do prototipo vira uma quebra manual
/// (o iced nao tem): lado a lado quando os dois cabem, senao empilhados.
fn empty_card(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let sketch = canvas(EmptyReservesProgram {
        tokens: t,
        cache: &app.reserves_empty_cache,
    })
    .width(EMPTY_RESERVES_WIDTH)
    .height(EMPTY_RESERVES_HEIGHT);
    let copy = column![
        text("Nenhuma reserva sincronizada")
            .size(20)
            .font(fonts::INTER_MEDIUM)
            .color(t.text),
        Space::new().height(8),
        text(
            "A reserva de emergência e as caixinhas são criadas no app. Assim que um celular \
             pareado sincronizar, saldos, metas e movimentações aparecem aqui."
        )
        .size(14)
        .line_height(1.55)
        .color(t.text_alpha(0.65)),
    ]
    .max_width(460);
    let inner = shell::content_width(app) - 2.0 * EMPTY_PADDING_X;
    let body: Element<'_, Message> = if inner >= EMPTY_RESERVES_WIDTH + EMPTY_GAP_X + EMPTY_TEXT_MIN
    {
        row![sketch, copy]
            .spacing(EMPTY_GAP_X)
            .align_y(Alignment::Center)
            .into()
    } else {
        column![sketch, copy].spacing(32).into()
    };
    container(body)
        .padding([56.0, EMPTY_PADDING_X])
        .width(Length::Fill)
        .style(theme::card(t))
        .into()
}

#[cfg(test)]
mod tests {
    use super::super::app::tests::{app_with, app_with_reserves};
    use super::*;
    use crate::hub::snapshot::Snapshot;

    #[test]
    fn constroi_vazia_e_com_dados_sem_panico() {
        let dir = tempfile::tempdir().unwrap();
        let (app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        assert!(app.reserves.is_empty);
        let _ = view(&app);
        let (mut app, _rx, _tx) = app_with_reserves(dir.path());
        assert!(!app.reserves.is_empty);
        let _ = view(&app);
        // Janela estreita: o estado vazio empilha.
        app.window_size = iced::Size::new(600.0, 720.0);
        let _ = view(&app);
    }
}
