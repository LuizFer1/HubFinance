//! Tela Lancamentos: filtros (categoria, autor, tipo), linha de totais e a tabela do mes
//! agrupada por dia, com os tres vazios do design.
//!
//! Tudo vem de `app.transactions` (calculado em `App::rebuild`); aqui so se monta a arvore.

use iced::widget::text::Wrapping;
use iced::widget::{
    Space, button, column, container, mouse_area, opaque, rich_text, row, scrollable, span, stack,
    text,
};
use iced::{Alignment, Color, Element, Length, Padding};

use super::app::{App, Message};
use super::rows::{author_cell, category_tile, hover_row, value_cell};
use super::theme::{self, ButtonKind, ButtonSize, Tokens, alpha, mix, size};
use super::widgets::{ChipShape, SegmentedVariant, TagTone};
use super::{fonts, icons, shell, widgets};
use crate::dashboard::list::{
    ALL_CATEGORIES_COLOR, CategoryOption, Filters, ListRow, NONE_KEY, TypeFilter,
};
use crate::dashboard::money::{format_brl, group_thousands, signed_brl, signed_brl_plus};
use crate::dashboard::periods::month_long;
use crate::dashboard::view::TransactionsView;

const GAP: f32 = 14.0;
/// O menu abre 42 px abaixo do topo da barra de filtros (botao de 36 + 6), como o `top: 42px`
/// do prototipo: sem coordenada absoluta, um `Space` empurra o menu para baixo do botao.
const MENU_OFFSET: f32 = 42.0;
const MENU_WIDTH: f32 = 260.0;
const MENU_MAX_HEIGHT: f32 = 360.0;
const MENU_ITEM: f32 = 34.0;
/// Colunas `2.2fr 1.3fr 1fr 1fr 128px` do prototipo.
const COLUMNS: [u16; 4] = [22, 13, 10, 10];
const VALUE_WIDTH: f32 = 128.0;

pub fn view(app: &App) -> Element<'_, Message> {
    let mut base = column![filter_bar(app), totals_line(app), table(app)]
        .spacing(GAP)
        .width(Length::Fill);
    if !app.cat_menu_open {
        return base.into();
    }
    // Camada que fecha o menu ao clicar fora (dentro da area da tela) e, por cima, o menu.
    let catcher = opaque(
        mouse_area(
            container(Space::new())
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .on_press(Message::CloseCategoryMenu),
    );
    // O `stack` tem o tamanho da primeira camada: com poucas linhas ela e mais baixa que o
    // menu, que sairia cortado. Um espaco no fim da tela completa a altura que falta (o iced
    // nao tem altura minima, e um espacador ao lado numa `row` nao estica a linha).
    let needed = MENU_OFFSET + menu_height(app.transactions.categories.len()) + 12.0 + 8.0;
    let missing = needed - estimated_height(&app.transactions);
    if missing > 0.0 {
        base = base.push(Space::new().height(missing));
    }
    let menu = column![
        Space::new().height(MENU_OFFSET),
        row![opaque(category_menu(app)), Space::new().width(Length::Fill)],
    ];
    stack![base, catcher, menu].width(Length::Fill).into()
}

/// Cor de um token de categoria, com o marcador "neutral" de "Todas as categorias".
fn option_color(t: &'static Tokens, token: &str) -> Color {
    if token == ALL_CATEGORIES_COLOR {
        t.idle()
    } else {
        t.token(token)
    }
}

// ---- barra de filtros ----

/// Linha com quebra (`wrap`), gap 8: categoria | autores | tipo | limpar.
fn filter_bar(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let mut bar = row![category_button(app), widgets::vdivider(t)]
        .spacing(8)
        .align_y(Alignment::Center);
    for chip in author_chips(app) {
        bar = bar.push(chip);
    }
    bar = bar.push(widgets::vdivider(t)).push(type_segment(app));
    if app.filters.is_active() {
        let clear = widgets::action_button_icon(
            t,
            ButtonKind::Link,
            ButtonSize::Regular,
            icons::X,
            "Limpar filtros",
        )
        .on_press(Message::ClearFilters);
        bar = bar.push(clear);
    }
    bar.wrap().vertical_spacing(8).into()
}

fn category_button(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let v = &app.transactions;
    let (label, dot) = match &v.selected_category {
        Some(selected) => (selected.name.clone(), option_color(t, &selected.color)),
        None => ("Todas as categorias".to_string(), t.idle()),
    };
    let active = v.selected_category.is_some();
    let text_color = if active { t.accent_200 } else { t.text };
    widgets::chip(
        t,
        row![
            widgets::dot(dot, 8.0),
            text(label).size(13),
            // Seta a 70 % da cor do texto do botao (`opacity: .7` do prototipo).
            icons::icon(icons::CARET_DOWN, 13.0, alpha(text_color, 0.7)),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        active,
        ChipShape::Rounded,
    )
    .on_press(Message::ToggleCategoryMenu)
    .into()
}

/// Altura aproximada da tela sem o menu: barra de filtros (36), totais (~25), cabecalho da
/// tabela (41), grupos (35) e linhas (53), ou o vazio (~250). So decide se falta espaco para o
/// menu; errar para cima so deixa um pouco de rolagem a mais.
fn estimated_height(v: &TransactionsView) -> f32 {
    let table = 41.0
        + if v.list.count == 0 {
            250.0
        } else {
            v.list.groups.len() as f32 * 35.0 + v.list.count as f32 * 53.0
        };
    36.0 + GAP + 25.0 + GAP + table
}

/// Altura da lista do menu: um item de 34 por opcao, ate 360 com o padding de 6.
fn menu_height(options: usize) -> f32 {
    (options as f32 * MENU_ITEM).min(MENU_MAX_HEIGHT - 12.0)
}

fn category_menu(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let v = &app.transactions;
    let selected = app.filters.category.as_deref();
    let mut items = column![].spacing(0);
    for option in &v.categories {
        items = items.push(menu_item(t, option, option.key.as_deref() == selected));
    }
    let height = menu_height(v.categories.len());
    container(
        scrollable(items)
            .direction(scrollable::Direction::Vertical(
                scrollable::Scrollbar::new()
                    .width(4)
                    .scroller_width(4)
                    .margin(1),
            ))
            .style(theme::scrollbar(t))
            .height(height),
    )
    .width(MENU_WIDTH)
    .padding(6)
    .style(theme::menu(t))
    .into()
}

fn menu_item<'a>(
    t: &'static Tokens,
    option: &'a CategoryOption,
    selected: bool,
) -> Element<'a, Message> {
    let check = icons::icon(
        icons::CHECK,
        14.0,
        if selected {
            t.accent_300
        } else {
            Color::TRANSPARENT
        },
    );
    button(
        container(
            row![
                widgets::dot(option_color(t, &option.color), 8.0),
                container(text(option.name.clone()).size(13).wrapping(Wrapping::None))
                    .width(Length::Fill)
                    .clip(true),
                text(group_thousands(option.count as u64))
                    .size(12)
                    .color(t.text_muted),
                container(check).width(14),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        )
        .height(Length::Fill)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .height(MENU_ITEM)
    .padding([0, 10])
    .style(theme::menu_item(t, selected))
    .on_press(Message::PickCategory(option.key.clone()))
    .into()
}

/// "Todos" + um chip por perfil vivo (+ "Sem autor" quando o mes tem lancamento sem autor).
fn author_chips(app: &App) -> Vec<Element<'_, Message>> {
    let t = app.tokens;
    let v = &app.transactions;
    let authors = &app.filters.authors;
    let chip = |content: Element<'static, Message>, padding_left: f32, active: bool, msg| {
        widgets::chip(t, content, active, ChipShape::Pill)
            .padding(Padding {
                top: 0.0,
                right: 12.0,
                bottom: 0.0,
                left: padding_left,
            })
            .on_press(msg)
            .into()
    };
    let mut chips: Vec<Element<'_, Message>> = vec![chip(
        text("Todos").size(13).into(),
        14.0,
        authors.is_empty(),
        Message::AllAuthors,
    )];
    for author in &v.authors {
        let photo = app.user_avatars.get(&author.id).map(|a| &a.handle);
        let content: Element<'static, Message> = row![
            widgets::avatar(t, &author.name, Some(&author.color), photo, 24.0),
            text(author.name.clone()).size(13),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into();
        chips.push(chip(
            content,
            5.0,
            authors.contains(&author.id),
            Message::ToggleAuthor(author.id.clone()),
        ));
    }
    if v.unauthored_chip {
        let content: Element<'static, Message> = row![
            container(icons::icon(icons::CIRCLE_DASHED, 22.0, t.idle())).center(24),
            text("Sem autor").size(13),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into();
        chips.push(chip(
            content,
            5.0,
            authors.contains(NONE_KEY),
            Message::ToggleAuthor(NONE_KEY.to_string()),
        ));
    }
    chips
}

fn type_segment(app: &App) -> Element<'_, Message> {
    let option = |kind: TypeFilter, label: &'static str| {
        (label, app.filters.kind == kind, Message::SetType(kind))
    };
    widgets::segmented(
        app.tokens,
        SegmentedVariant::Bordered,
        ButtonSize::Compact,
        false,
        [
            option(TypeFilter::All, "Todos"),
            option(TypeFilter::Income, "Receitas"),
            option(TypeFilter::Expense, "Despesas"),
        ],
    )
}

// ---- totais ----

/// "**N** lançamentos · Receitas +R$ … · Despesas −R$ … · Saldo R$ …" (13, rotulos no texto de apoio).
fn totals_line(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let list = &app.transactions.list;
    let muted = t.text_soft();
    let count = list.count;
    let pair = |label: &'static str, value: String, color: Color| {
        rich_text::<(), _, _, _>([
            span(label).color(muted),
            span(" "),
            span(value).color(color),
        ])
        .size(13)
    };
    let line = row![
        rich_text::<(), _, _, _>([
            span(group_thousands(count as u64))
                .font(fonts::INTER_MEDIUM)
                .color(t.text),
            span(if count == 1 {
                " lançamento"
            } else {
                " lançamentos"
            })
            .color(muted),
        ])
        .size(13),
        pair(
            "Receitas",
            format!("+{}", format_brl(list.totals.income_minor)),
            t.income_fg
        ),
        pair(
            "Despesas",
            format!("\u{2212}{}", format_brl(list.totals.expense_minor)),
            t.expense_fg
        ),
        pair("Saldo", signed_brl(list.totals.balance_minor), t.text),
    ]
    .spacing(24)
    .wrap()
    .vertical_spacing(6);
    container(line).padding([4, 2]).into()
}

// ---- tabela ----

/// Celulas nas colunas da tabela (gap 16, padding horizontal 20).
fn columns<'a>(cells: [Element<'a, Message>; 5]) -> iced::widget::Row<'a, Message> {
    let [desc, cat, author, pay, value] = cells;
    let [w0, w1, w2, w3] = COLUMNS;
    row![
        container(desc).width(Length::FillPortion(w0)).clip(true),
        container(cat).width(Length::FillPortion(w1)).clip(true),
        container(author).width(Length::FillPortion(w2)).clip(true),
        container(pay).width(Length::FillPortion(w3)).clip(true),
        container(value).width(VALUE_WIDTH).align_x(Alignment::End),
    ]
    .spacing(16)
    .align_y(Alignment::Center)
}

fn table(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let v = &app.transactions;
    let head = |label: &'static str| -> Element<'_, Message> {
        text(label)
            .size(12)
            .font(fonts::INTER_MEDIUM)
            .color(t.text_muted)
            .into()
    };
    let header = container(columns([
        head("Descrição"),
        head("Categoria"),
        head("Autor"),
        head("Pagamento"),
        head("Valor"),
    ]))
    .height(40)
    .padding([0, 20])
    .align_y(Alignment::Center);
    let mut col = column![header, widgets::divider(t)];
    if v.list.count == 0 {
        col = col.push(empty(app));
    }
    let day_bg = mix(t.surface, t.bg, 0.45);
    for (i, group) in v.list.groups.iter().enumerate() {
        if i > 0 {
            col = col.push(widgets::divider(t));
        }
        col = col.push(
            container(
                row![
                    text(group.heading.clone())
                        .size(12)
                        .font(fonts::INTER_MEDIUM)
                        .color(t.text_soft())
                        .width(Length::Fill),
                    text(signed_brl_plus(group.net_minor))
                        .size(12)
                        .color(t.text_muted),
                ]
                .spacing(12)
                .align_y(Alignment::Center),
            )
            .height(34)
            .padding([0, 20])
            .align_y(Alignment::Center)
            .style(theme::fill(day_bg, 0.0)),
        );
        for r in &group.rows {
            col = col.push(widgets::divider(t)).push(table_row(app, r));
        }
    }
    // `padding(1)`: as faixas de dia sao opacas e de largura cheia, e o iced desenha a borda por
    // dentro; sem o respiro elas, os divisores e o hover da ultima linha cobrem a borda do card.
    widgets::card(t, col).padding(1).clip(true).into()
}

fn table_row<'a>(app: &'a App, r: &'a ListRow) -> Element<'a, Message> {
    let t = app.tokens;
    let mut description = row![
        category_tile(t, r),
        text(r.description.clone())
            .size(14)
            .font(fonts::INTER_MEDIUM)
            .wrapping(Wrapping::None)
            .color(t.text),
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    if let Some(label) = &r.recurrence_label {
        description = description.push(widgets::tag_icon(t, TagTone::Accent, icons::REPEAT, label));
    }
    if let Some(state) = r.report_state {
        description = description.push(widgets::tag(t, TagTone::Neutral, state.label()));
    }
    let category_color = r
        .category_color
        .as_deref()
        .map_or_else(|| t.token("slate"), |c| t.token(c));
    let category = row![
        widgets::dot(category_color, 8.0),
        text(r.category_name.clone())
            .size(13)
            .wrapping(Wrapping::None)
            .color(t.text_soft()),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    let muted = t.text_soft();
    let payment = row![
        icons::icon(icons::glyph(&r.payment_icon), 15.0, muted),
        text(r.payment_name.clone())
            .size(13)
            .wrapping(Wrapping::None)
            .color(muted),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    let content = columns([
        description.into(),
        category.into(),
        author_cell(app, r),
        payment.into(),
        value_cell(t, r),
    ]);
    hover_row(app, &r.id, content, [0, 20], 0.0, size::ROW_RICH)
}

// ---- vazio ----

/// Titulo, texto e se mostra "Limpar filtros", nos tres vazios do design.
fn empty_copy(v: &TransactionsView, filters: &Filters) -> (String, String, bool) {
    if v.is_empty_hub {
        (
            "Nenhum lançamento sincronizado".to_string(),
            "Quando um celular pareado sincronizar, o histórico aparece aqui — só leitura, \
             direto da rede de casa"
                .to_string(),
            false,
        )
    } else if filters.is_active() {
        (
            "Nada com esses filtros".to_string(),
            format!(
                "Nenhum lançamento em {} combina com a categoria, o autor e o tipo escolhidos",
                month_long(&v.month)
            ),
            true,
        )
    } else {
        (
            format!("Nada em {}", month_long(&v.month)),
            "Nenhum lançamento neste mês".to_string(),
            false,
        )
    }
}

fn empty(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let v = &app.transactions;
    let (title, body, show_clear) = empty_copy(v, &app.filters);
    // So no vazio do hub (nada sincronizado) vem "Conectar um celular"; o vazio por filtro tem
    // "Limpar filtros" e o do mes nenhum. Os dois nunca aparecem juntos.
    let connect = if v.is_empty_hub {
        shell::connect_phone_button(app)
    } else {
        None
    };
    let action = connect.or_else(|| {
        show_clear.then(|| {
            widgets::action_button(
                t,
                ButtonKind::Primary,
                ButtonSize::Regular,
                "Limpar filtros",
            )
            .on_press(Message::ClearFilters)
            .into()
        })
    });
    let col = widgets::empty_state(
        t,
        icons::icon(icons::FUNNEL_X, 28.0, t.accent_300),
        &title,
        &body,
        action,
    );
    container(col)
        .padding([56, 24])
        .center_x(Length::Fill)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dashboard::dataset::Dataset;
    use crate::dashboard::fixtures;
    use crate::dashboard::view::build_transactions;

    #[test]
    fn tres_vazios() {
        let none = Filters::default();
        let hub = build_transactions(&Dataset::default(), "2026-09-24", "2026-09", &none);
        let (title, _, clear) = empty_copy(&hub, &none);
        assert_eq!(title, "Nenhum lançamento sincronizado");
        assert!(!clear);
        let ds = fixtures();
        let may = build_transactions(&ds, "2026-09-24", "2026-05", &none);
        assert_eq!(
            empty_copy(&may, &none),
            (
                "Nada em maio".to_string(),
                "Nenhum lançamento neste mês".to_string(),
                false
            )
        );
        let filters = Filters {
            kind: TypeFilter::Income,
            category: Some("C1".into()),
            ..Filters::default()
        };
        let sep = build_transactions(&ds, "2026-09-24", "2026-09", &filters);
        assert_eq!(sep.list.count, 0);
        let (title, body, clear) = empty_copy(&sep, &filters);
        assert_eq!(title, "Nada com esses filtros");
        assert!(body.contains("em setembro"));
        assert!(clear);
    }
}
