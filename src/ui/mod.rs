//! Janela iced: so renderiza o snapshot do nucleo e envia comandos; unico modulo com iced.

use iced::Element;

/// Janela provisoria; a Tarefa 18 a substitui pela janela de verdade.
pub fn run() -> iced::Result {
    iced::application(|| (), |_: &mut (), _: ()| {}, view)
        .title("HubFinance")
        .run()
}

fn view(_: &()) -> Element<'_, ()> {
    iced::widget::text("HubFinance").into()
}
