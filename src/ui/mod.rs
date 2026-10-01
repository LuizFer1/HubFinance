//! Janela iced: so renderiza o snapshot do nucleo e envia comandos; unico modulo com iced.

mod app;
mod view;

use time::UtcOffset;

use crate::hub::HubHandle;

pub fn run(handle: HubHandle, offset: UtcOffset) -> iced::Result {
    iced::application(app::boot(handle, offset), app::update, app::view)
        .title("HubFinance")
        .subscription(app::subscription)
        // Fechar desliga o hub com ordem: o `CloseRequested` manda `Shutdown` e a janela so
        // some quando o nucleo responde `Stopped` (ou em 5 s).
        .exit_on_close_request(false)
        .window_size((520.0, 820.0))
        .theme(iced::Theme::Dark)
        .run()
}
