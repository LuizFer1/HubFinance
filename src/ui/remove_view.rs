//! Modal "Remover Ana?" (e a mesma decisao para um aparelho ja removido) e o aviso de aparelhos
//! removidos que ainda tem dados. Os textos saem de funcoes puras (testaveis sem janela); os
//! widgets so os dispõem.
//!
//! Dois passos, como os formularios do app: (1) escolher o que apagar por area, com a contagem
//! do plano que a escrita tambem usa; (2) confirmacao em vermelho com as contagens e um botao
//! que nomeia a acao. Nada marcado e "Manter tudo", o padrao, e nao exige o passo 2.

use iced::widget::{Space, button, column, row, text};
use iced::{Alignment, Element, Length};

use super::app::{App, Message, RemoveStep};
use super::theme::{self, ButtonKind, ButtonSize, Tokens, Tone, space};
use super::{fonts, icons, widgets};
use crate::hub::snapshot::DeviceView;
use crate::store::erase::{Area, AreaSet, DeviceData};
use crate::ui_model::remove::{
    area_count_text, area_notes, area_title, confirm_label, confirm_text,
};

/// Quem aparece no modal: a pessoa da lista (aparelho ativo) ou, para um aparelho ja removido
/// (que `people` filtra), o proprio aparelho.
struct Subject<'a> {
    name: String,
    device_name: String,
    color: Option<&'a str>,
    photo: Option<&'a iced::widget::image::Handle>,
    device: &'a DeviceView,
}

fn subject<'a>(app: &'a App, device_id: &str) -> Option<Subject<'a>> {
    let device = app
        .snapshot
        .devices
        .iter()
        .find(|d| d.device_id == device_id)?;
    if let Some(person) = app.people.iter().find(|p| p.device_id == device_id) {
        return Some(Subject {
            name: person.display_name.clone(),
            device_name: person.device_name.clone(),
            color: person.color.as_deref(),
            photo: app.iced.avatars.get(device_id),
            device,
        });
    }
    Some(Subject {
        name: device.name.clone(),
        device_name: device.name.clone(),
        color: None,
        photo: None,
        device,
    })
}

/// Fundo preto a 55 % que cancela ao clicar e a caixa por cima.
pub(super) fn dialog(app: &App) -> Option<Element<'_, Message>> {
    let t = app.tokens;
    let state = app.confirm_remove.as_ref()?;
    let who = subject(app, &state.device_id)?;
    let orphan = who.device.revoked;
    let data = &who.device.data;
    let name = who.name.as_str();

    let title = if orphan {
        format!("Dados de {name}")
    } else {
        format!("Remover {name}?")
    };
    let header = row![
        widgets::avatar(t, name, who.color, who.photo, 40.0),
        text(title).size(18).font(fonts::INTER_MEDIUM).color(t.text),
    ]
    .spacing(12)
    .align_y(Alignment::Center);

    let body: Element<'_, Message> = match state.step {
        RemoveStep::Choose => choose_body(app, &who, state.selected),
        RemoveStep::Confirm => text(confirm_text(data, state.selected))
            .size(14)
            .line_height(1.55)
            .color(t.expense_fg)
            .into(),
    };

    let size = ButtonSize::Regular;
    let buttons = match state.step {
        RemoveStep::Choose => {
            let first = if orphan { "Depois" } else { "Cancelar" };
            let cancel = widgets::action_button(t, ButtonKind::Secondary, size, first)
                .on_press(Message::CancelRemove);
            let nothing = state.selected.is_empty();
            let (primary_label, kind) = match (nothing, orphan) {
                (true, true) => ("Manter tudo", ButtonKind::Primary),
                (true, false) => ("Remover", ButtonKind::DangerFilled),
                (false, _) => ("Continuar", ButtonKind::Danger),
            };
            let primary = if nothing && !orphan {
                widgets::action_button_icon(t, kind, size, icons::USER_MINUS, primary_label)
            } else {
                widgets::action_button(t, kind, size, primary_label)
            }
            .on_press(Message::RemoveContinue);
            row![Space::new().width(Length::Fill), cancel, primary].spacing(space::M)
        }
        RemoveStep::Confirm => {
            let back = widgets::action_button(t, ButtonKind::Secondary, size, "Voltar")
                .on_press(Message::RemoveBack);
            let confirm = widgets::action_button(
                t,
                ButtonKind::DangerFilled,
                size,
                confirm_label(data, state.selected, orphan),
            )
            .on_press(Message::ConfirmRemove);
            row![Space::new().width(Length::Fill), back, confirm].spacing(space::M)
        }
    };

    Some(widgets::dialog(
        t,
        column![
            header,
            Space::new().height(space::L),
            body,
            Space::new().height(space::XXL),
            buttons
        ]
        .width(Length::Fill),
        app.window_size.w,
        460.0,
        None,
        Message::CancelRemove,
        app.motion.scrim(),
    ))
}

fn choose_body<'a>(app: &'a App, who: &Subject<'a>, selected: AreaSet) -> Element<'a, Message> {
    let t = app.tokens;
    let data = &who.device.data;
    let intro = if who.device.revoked {
        format!(
            "{} foi removido do hub e ainda tem dados aqui. O que apagar dos dados que ele enviou?",
            who.device_name
        )
    } else {
        format!(
            "O {} para de sincronizar com este computador. O que apagar dos dados que ele \
             enviou? Para voltar, basta conectar de novo pelo QR code.",
            who.device_name
        )
    };
    let mut col = column![
        text(intro).size(14).line_height(1.55).color(t.text_soft()),
        Space::new().height(4),
    ]
    .spacing(8);
    for area in Area::ALL {
        col = col.push(area_row(t, area, data, selected.contains(area)));
    }
    let shortcut = |label: &'static str, msg: Message, danger: bool| {
        let kind = if danger {
            ButtonKind::Danger
        } else {
            ButtonKind::Secondary
        };
        widgets::action_button(t, kind, ButtonSize::Compact, label).on_press(msg)
    };
    col.push(
        row![
            shortcut("Manter tudo", Message::SelectNoAreas, false),
            shortcut("Apagar tudo", Message::SelectAllAreas, true),
        ]
        .spacing(8),
    )
    .push(
        text("Categorias, formas de pagamento e o perfil não são apagados")
            .size(12)
            .color(t.text_muted),
    )
    .into()
}

/// Uma area: caixa de selecao, titulo, contagem e as linhas de apoio. Sem dados, fica
/// desabilitada.
fn area_row<'a>(
    t: &'static Tokens,
    area: Area,
    data: &DeviceData,
    selected: bool,
) -> Element<'a, Message> {
    let enabled = data.items(area) > 0;
    let mark: Element<'a, Message> = if selected && enabled {
        icons::icon(icons::CHECK_CIRCLE, 18.0, t.expense_fg)
    } else {
        icons::icon(icons::CIRCLE_DASHED, 18.0, t.text_muted)
    };
    let mut info = column![
        // Desabilitada, o titulo apaga como o texto dos botoes desabilitados (`theme::disabled`):
        // a cor fixa aqui passaria por cima do estilo do botao.
        text(area_title(area))
            .size(14)
            .font(fonts::INTER_MEDIUM)
            .color(if enabled { t.text } else { t.idle() }),
    ]
    .spacing(2);
    if enabled {
        info = info.push(
            text(area_count_text(area, data))
                .size(12)
                .color(t.text_soft()),
        );
        for note in area_notes(area, data) {
            info = info.push(text(note).size(12).color(t.text_muted));
        }
    } else {
        let mut empty = column![
            text("sem dados deste aparelho")
                .size(12)
                .color(t.text_muted)
        ];
        for note in area_notes(area, data) {
            empty = empty.push(text(note).size(12).color(t.text_muted));
        }
        info = info.push(empty);
    }
    let line = row![mark, info.width(Length::Fill)]
        .spacing(12)
        .align_y(Alignment::Center);
    // Caixa de selecao inteira, nao botao de acao: o papel do botao so empresta o contorno e os
    // estados (perigo marcada, secundario desmarcada).
    let kind = if selected && enabled {
        ButtonKind::Danger
    } else {
        ButtonKind::Secondary
    };
    let b = button(line)
        .width(Length::Fill)
        .padding([10, 12])
        .style(theme::button(t, kind, ButtonSize::Regular));
    if enabled {
        b.on_press(Message::ToggleArea(area)).into()
    } else {
        b.into()
    }
}

/// Aviso ambar no topo do conteudo: aparelhos removidos que ainda tem dados. Derivado do
/// estado, sem card permanente; "Depois" so esconde nesta sessao.
pub(super) fn orphan_banner(app: &App) -> Option<Element<'_, Message>> {
    let t = app.tokens;
    if app.orphans_postponed || app.confirm_remove.is_some() {
        return None;
    }
    let n = app.orphans().len();
    if n == 0 {
        return None;
    }
    let message = if n == 1 {
        "1 aparelho removido ainda tem dados no hub".to_string()
    } else {
        format!("{n} aparelhos removidos ainda têm dados no hub")
    };
    let later = widgets::action_button(t, ButtonKind::Ghost, ButtonSize::Compact, "Depois")
        .on_press(Message::PostponeOrphans);
    let decide = widgets::action_button(t, ButtonKind::Primary, ButtonSize::Compact, "Decidir")
        .on_press(Message::DecideOrphan);
    Some(widgets::banner(
        t,
        Tone::Warning,
        t.bg,
        icons::WARNING_CIRCLE,
        message,
        vec![later.into(), decide.into()],
    ))
}
