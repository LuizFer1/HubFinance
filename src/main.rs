use iced::widget::{button, column, text};
use iced::{Center, Element};

fn main() -> iced::Result {
    iced::application(Hub::default, Hub::update, Hub::view)
        .title("HubFinance")
        .run()
}

#[derive(Default)]
struct Hub {
    contador: i64,
}

#[derive(Debug, Clone, Copy)]
enum Message {
    Incrementar,
    Decrementar,
}

impl Hub {
    fn update(&mut self, message: Message) {
        match message {
            Message::Incrementar => self.contador += 1,
            Message::Decrementar => self.contador -= 1,
        }
    }

    fn view(&self) -> Element<'_, Message> {
        column![
            button("+").on_press(Message::Incrementar),
            text(self.contador).size(40),
            button("-").on_press(Message::Decrementar),
        ]
        .spacing(10)
        .padding(20)
        .align_x(Center)
        .into()
    }
}
