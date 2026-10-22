//! Modal "Remover Ana?" (e a mesma decisao para um aparelho ja removido) e o aviso de aparelhos
//! removidos que ainda tem dados. Os textos saem de funcoes puras (testaveis sem janela); os
//! widgets so os dispõem.
//!
//! Dois passos, como os formularios do app: (1) escolher o que apagar por area, com a contagem
//! do plano que a escrita tambem usa; (2) confirmacao em vermelho com as contagens e um botao
//! que nomeia a acao. Nada marcado e "Manter tudo", o padrao, e nao exige o passo 2.

use iced::widget::{Space, button, column, container, mouse_area, opaque, row, stack, text};
use iced::{Alignment, Element, Length};

use super::app::{App, Message, RemoveStep};
use super::theme::{self, Tokens};
use super::{fonts, icons, widgets};
use crate::hub::snapshot::DeviceView;
use crate::store::erase::{Area, AreaSet, DeviceData};

pub(super) fn area_title(area: Area) -> &'static str {
    match area {
        Area::Budgets => "Metas",
        Area::Transactions => "Transferências",
        Area::Reserves => "Reservas",
        Area::Recurrences => "Recorrências",
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// "128 lançamentos", "2 reservas e 14 movimentos": o que a area apaga.
pub(super) fn area_count_text(area: Area, d: &DeviceData) -> String {
    match area {
        Area::Budgets => plural(d.budgets, "meta", "metas"),
        Area::Transactions => plural(d.transactions, "lançamento", "lançamentos"),
        Area::Reserves => format!(
            "{} e {}",
            plural(d.reserves, "reserva", "reservas"),
            plural(d.reserve_movements, "movimento", "movimentos")
        ),
        Area::Recurrences => format!(
            "{} e {}",
            plural(d.recurrences, "série", "séries"),
            plural(d.adjustments, "reajuste", "reajustes")
        ),
    }
}

/// Linhas de apoio de cada area: o que a contagem nao diz sozinha (internas, cascata, o que
/// fica).
pub(super) fn area_notes(area: Area, d: &DeviceData) -> Vec<String> {
    let mut notes = Vec::new();
    match area {
        Area::Budgets => {}
        Area::Transactions => {
            if d.internal_transfers > 0 {
                notes.push(format!(
                    "das quais {} internas",
                    plural(d.internal_transfers, "transferência", "transferências")
                ));
            }
        }
        Area::Reserves => {
            if d.reserves_kept > 0 {
                let kept = if d.reserves_kept == 1 {
                    "1 reserva mantida: tem".to_string()
                } else {
                    format!("{} reservas mantidas: têm", d.reserves_kept)
                };
                notes.push(format!(
                    "{kept} movimentos de outros aparelhos. Só os movimentos deste aparelho \
                     caem, e isso reduz o saldo da reserva para todos"
                ));
            }
        }
        Area::Recurrences => {
            if d.adjustments_other > 0 {
                notes.push(format!(
                    "+{} de outros aparelhos",
                    plural(d.adjustments_other, "reajuste", "reajustes")
                ));
            }
            if d.generated > 0 {
                notes.push(format!(
                    "{} gerados por estas séries continuam",
                    plural(d.generated, "lançamento", "lançamentos")
                ));
            }
        }
    }
    notes
}

/// "128 lançamentos, 3 metas, 2 reservas com 14 movimentos e 1 recorrência com 2 reajustes".
pub(super) fn selected_list(d: &DeviceData, selected: AreaSet) -> String {
    let mut parts = Vec::new();
    for area in selected.iter() {
        let part = match area {
            Area::Transactions => plural(d.transactions, "lançamento", "lançamentos"),
            Area::Budgets => plural(d.budgets, "meta", "metas"),
            Area::Reserves => format!(
                "{} com {}",
                plural(d.reserves, "reserva", "reservas"),
                plural(d.reserve_movements, "movimento", "movimentos")
            ),
            Area::Recurrences => format!(
                "{} com {}",
                plural(d.recurrences, "recorrência", "recorrências"),
                plural(d.adjustments, "reajuste", "reajustes")
            ),
        };
        parts.push(part);
    }
    match parts.len() {
        0 => String::new(),
        1 => parts.remove(0),
        _ => {
            let last = parts.pop().unwrap_or_default();
            format!("{} e {last}", parts.join(", "))
        }
    }
}

/// Texto em vermelho do passo 2.
pub(super) fn confirm_text(d: &DeviceData, selected: AreaSet) -> String {
    let mut text = format!(
        "Isso apaga {}, também dos outros celulares. Se este aparelho for pareado de novo, ele \
         também perde esses dados. Só se desfaz lançando de novo.",
        selected_list(d, selected)
    );
    // Linha apagada nao e recriada pelo app, mas a serie mantida segue gerando os meses novos.
    if selected.contains(Area::Transactions) && !selected.contains(Area::Recurrences) {
        text.push_str(
            " Séries de recorrência mantidas continuam gerando os próximos meses, mesmo com \
             lançamentos apagados.",
        );
    }
    text
}

/// O botao do passo 2 nomeia a acao: "Remover e apagar 150 itens".
pub(super) fn confirm_label(d: &DeviceData, selected: AreaSet, orphan: bool) -> String {
    let items = plural(d.total_items(selected), "item", "itens");
    if orphan {
        format!("Apagar {items}")
    } else {
        format!("Remover e apagar {items}")
    }
}

/// Texto do toast "Desfazer": cobre a remocao **e** o apagar.
pub(super) fn toast_text(base: &str, d: &DeviceData, selected: AreaSet) -> String {
    if selected.is_empty() {
        return base.to_string();
    }
    let n = d.total_items(selected);
    let verb = if n == 1 {
        "item será apagado"
    } else {
        "itens serão apagados"
    };
    format!("{base} · {n} {verb}")
}

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
            photo: app.avatars.get(device_id).map(|a| &a.handle),
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

    let action = |content: Element<'static, Message>| {
        button(
            container(content)
                .height(Length::Fill)
                .align_y(Alignment::Center),
        )
        .height(38)
        .padding([0, 16])
    };
    let label = |s: String| -> Element<'static, Message> {
        text(s).size(13).font(fonts::INTER_MEDIUM).into()
    };
    let buttons = match state.step {
        RemoveStep::Choose => {
            let first = if orphan { "Depois" } else { "Cancelar" };
            let cancel = action(label(first.into()))
                .style(theme::outline_divider(t))
                .on_press(Message::CancelRemove);
            let nothing = state.selected.is_empty();
            let (primary_label, style_danger) = match (nothing, orphan) {
                (true, true) => ("Manter tudo", false),
                (true, false) => ("Remover", false),
                (false, _) => ("Continuar", true),
            };
            let content: Element<'static, Message> = if nothing && !orphan {
                row![
                    icons::icon_inherit(icons::USER_MINUS, 13.0),
                    text(primary_label).size(13).font(fonts::INTER_MEDIUM),
                ]
                .spacing(6)
                .align_y(Alignment::Center)
                .into()
            } else {
                label(primary_label.into())
            };
            let primary = action(content).on_press(Message::RemoveContinue);
            let primary = if style_danger {
                primary.style(theme::danger_outline(t))
            } else if orphan {
                primary.style(theme::outline_accent(t))
            } else {
                primary.style(theme::danger_filled(t))
            };
            row![Space::new().width(Length::Fill), cancel, primary].spacing(10)
        }
        RemoveStep::Confirm => {
            let back = action(label("Voltar".into()))
                .style(theme::outline_divider(t))
                .on_press(Message::RemoveBack);
            let confirm = action(label(confirm_label(data, state.selected, orphan)))
                .style(theme::danger_filled(t))
                .on_press(Message::ConfirmRemove);
            row![Space::new().width(Length::Fill), back, confirm].spacing(10)
        }
    };

    let width = (app.window_size.width - 48.0).clamp(240.0, 460.0);
    let dialog_box = container(
        column![
            header,
            Space::new().height(14),
            body,
            Space::new().height(22),
            buttons
        ]
        .width(Length::Fill),
    )
    .width(width)
    .padding(24)
    .style(theme::dialog(t));
    // A caixa engole o clique: sem isso, clicar dentro dela chegaria ao fundo e cancelaria.
    let dialog_box = opaque(mouse_area(dialog_box).on_press(Message::Noop));
    let scrim = opaque(
        mouse_area(
            container(Space::new())
                .width(Length::Fill)
                .height(Length::Fill)
                .style(theme::scrim()),
        )
        .on_press(Message::CancelRemove),
    );
    Some(
        stack![scrim, container(dialog_box).center(Length::Fill)]
            .width(Length::Fill)
            .height(Length::Fill)
            .into(),
    )
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
        text(intro)
            .size(14)
            .line_height(1.55)
            .color(t.text_alpha(0.70)),
        Space::new().height(4),
    ]
    .spacing(8);
    for area in Area::ALL {
        col = col.push(area_row(t, area, data, selected.contains(area)));
    }
    let shortcut = |label: &'static str, msg: Message, danger: bool| {
        let b = button(text(label).size(12).font(fonts::INTER_MEDIUM))
            .padding([6, 12])
            .on_press(msg);
        if danger {
            b.style(theme::danger_outline(t))
        } else {
            b.style(theme::outline_divider(t))
        }
    };
    col.push(
        row![
            shortcut("Manter tudo", Message::SelectNoAreas, false),
            shortcut("Apagar tudo", Message::SelectAllAreas, true),
        ]
        .spacing(8),
    )
    .push(
        text("Categorias, formas de pagamento e o perfil não são apagados.")
            .size(12)
            .color(t.text_alpha(0.55)),
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
        icons::icon(icons::CIRCLE_DASHED, 18.0, t.text_alpha(0.45))
    };
    let mut info = column![
        text(area_title(area))
            .size(14)
            .font(fonts::INTER_MEDIUM)
            .color(t.text),
    ]
    .spacing(2);
    if enabled {
        info = info.push(
            text(area_count_text(area, data))
                .size(12)
                .color(t.text_alpha(0.75)),
        );
        for note in area_notes(area, data) {
            info = info.push(text(note).size(12).color(t.text_alpha(0.55)));
        }
    } else {
        let mut empty = column![
            text("sem dados deste aparelho")
                .size(12)
                .color(t.text_alpha(0.45))
        ];
        for note in area_notes(area, data) {
            empty = empty.push(text(note).size(12).color(t.text_alpha(0.45)));
        }
        info = info.push(empty);
    }
    let line = row![mark, info.width(Length::Fill)]
        .spacing(12)
        .align_y(Alignment::Center);
    let b = button(line).width(Length::Fill).padding([10, 12]);
    let b = if selected && enabled {
        b.style(theme::danger_outline(t))
    } else {
        b.style(theme::outline_divider(t))
    };
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
        "1 aparelho removido ainda tem dados no hub.".to_string()
    } else {
        format!("{n} aparelhos removidos ainda têm dados no hub.")
    };
    let later = button(text("Depois").size(13).font(fonts::INTER_MEDIUM))
        .padding([6, 12])
        .style(theme::ghost(t))
        .on_press(Message::PostponeOrphans);
    let decide = button(text("Decidir").size(13).font(fonts::INTER_MEDIUM))
        .padding([6, 12])
        .style(theme::outline_accent(t))
        .on_press(Message::DecideOrphan);
    Some(
        container(
            row![
                icons::icon(icons::WARNING_CIRCLE, 18.0, t.amber),
                text(message).size(13).color(t.text).width(Length::Fill),
                later,
                decide,
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        )
        .padding([10, 14])
        .width(Length::Fill)
        .style(theme::outlined(t.amber, 10.0))
        .into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> DeviceData {
        DeviceData {
            budgets: 3,
            transactions: 128,
            internal_transfers: 4,
            reserves: 2,
            reserve_movements: 14,
            reserves_kept: 1,
            recurrences: 1,
            adjustments: 2,
            adjustments_other: 1,
            generated: 5,
        }
    }

    #[test]
    fn contagem_e_notas_por_area() {
        let d = data();
        assert_eq!(area_count_text(Area::Budgets, &d), "3 metas");
        assert_eq!(area_count_text(Area::Transactions, &d), "128 lançamentos");
        assert_eq!(
            area_count_text(Area::Reserves, &d),
            "2 reservas e 14 movimentos"
        );
        assert_eq!(
            area_count_text(Area::Recurrences, &d),
            "1 série e 2 reajustes"
        );
        assert_eq!(
            area_notes(Area::Transactions, &d),
            vec!["das quais 4 transferências internas"]
        );
        let reserves = area_notes(Area::Reserves, &d);
        assert!(reserves[0].starts_with("1 reserva mantida: tem movimentos de outros aparelhos"));
        assert!(reserves[0].contains("reduz o saldo da reserva para todos"));
        assert_eq!(
            area_notes(Area::Recurrences, &d),
            vec![
                "+1 reajuste de outros aparelhos",
                "5 lançamentos gerados por estas séries continuam"
            ]
        );
        assert!(area_notes(Area::Budgets, &d).is_empty());
    }

    #[test]
    fn confirmacao_traz_as_contagens_das_areas_marcadas() {
        let d = data();
        let all = confirm_text(&d, AreaSet::all());
        assert!(
            all.starts_with(
                "Isso apaga 3 metas, 128 lançamentos, 2 reservas com 14 movimentos e 1 \
                 recorrência com 2 reajustes, também dos outros celulares."
            ),
            "{all}"
        );
        assert!(all.contains("pareado de novo, ele também perde esses dados"));
        assert!(!all.contains("continuam gerando"));
        let only_tx: AreaSet = [Area::Transactions].into_iter().collect();
        let text = confirm_text(&d, only_tx);
        assert!(
            text.starts_with("Isso apaga 128 lançamentos, também"),
            "{text}"
        );
        assert!(text.contains("Séries de recorrência mantidas continuam gerando"));
    }

    #[test]
    fn botao_nomeia_a_acao() {
        let d = data();
        assert_eq!(
            confirm_label(&d, AreaSet::all(), false),
            "Remover e apagar 150 itens"
        );
        assert_eq!(confirm_label(&d, AreaSet::all(), true), "Apagar 150 itens");
        let one: AreaSet = [Area::Budgets].into_iter().collect();
        let d1 = DeviceData {
            budgets: 1,
            ..DeviceData::default()
        };
        assert_eq!(confirm_label(&d1, one, false), "Remover e apagar 1 item");
    }

    #[test]
    fn toast_so_fala_de_apagar_se_algo_foi_marcado() {
        let d = data();
        assert_eq!(
            toast_text("Ana foi removida do hub", &d, AreaSet::default()),
            "Ana foi removida do hub"
        );
        assert_eq!(
            toast_text("Ana foi removida do hub", &d, AreaSet::all()),
            "Ana foi removida do hub · 150 itens serão apagados"
        );
    }
}
