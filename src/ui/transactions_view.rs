//! Tela Lancamentos: filtros (categoria, autor, tipo), resumo do mes e a tabela do mes
//! agrupada por dia, com os tres vazios do design. E a unica tela com rolagem propria: filtros,
//! resumo e cabecalho da tabela ficam fixos e so as linhas rolam (`shell::owns_scroll`).
//!
//! Tudo vem de `app.transactions` (calculado em `App::rebuild`); aqui so se monta a arvore.

use iced::widget::text::Wrapping;
use iced::widget::{
    Space, button, column, container, mouse_area, opaque, row, scrollable, stack, text,
};
use iced::{Alignment, Color, Element, Length, Padding};

use super::app::{App, Message};
use super::rows::{author_cell, category_tile, hover_row, value_cell};
use super::theme::{self, ButtonKind, ButtonSize, Tokens, alpha, mix, size, space, type_scale};
use super::widgets::{ChipShape, RowEdge, SegmentedSize, SegmentedVariant, TagTone};
use super::{fonts, icons, shell, widgets};
use crate::dashboard::list::{
    ALL_CATEGORIES_COLOR, CategoryOption, Filters, ListRow, NONE_KEY, TypeFilter,
};
use crate::dashboard::money::group_thousands;
use crate::dashboard::periods::month_long;
use crate::dashboard::view::TransactionsView;
use crate::ui_model::shell::Arrival;

/// Entre filtros, resumo e tabela: o espaco entre blocos de toda tela (7.4).
const GAP: f32 = space::BETWEEN_CARDS;
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
    // No boot o "Nenhum lançamento sincronizado" seria um vazio falso: o esqueleto comum das
    // telas de dados entra antes de filtros e tabela (que nem tem o que filtrar ainda).
    if shell::screen_arrival(app, !app.transactions.is_empty_hub) == Arrival::Opening {
        return shell::opening(app);
    }
    // Altura cheia: a casca nao rola esta tela (`shell::owns_scroll`). Filtros, resumo e o
    // cabecalho da tabela ficam parados; so as linhas rolam, dentro do card (`table`). A roda do
    // mouse so rola sobre as linhas: sobre os filtros e as margens nao ha o que rolar.
    let mut base = column![filter_bar(app)];
    if shows_summary(&app.transactions) {
        base = base.push(summary_line(app));
    }
    let base = base
        .push(table(app))
        .spacing(GAP)
        .width(Length::Fill)
        .height(Length::Fill);
    // Sempre um `stack` com a tela no indice 0, aberto ou fechado o menu: o iced casa o estado
    // dos widgets pela posicao na arvore, e trocar `Column` por `Stack` ao abrir o menu jogava
    // fora o estado da rolagem das linhas (voltava ao topo a cada abertura e fechamento).
    // Sem teste: a arvore de estado (`Tree`) so e publica com a feature `advanced` do iced, que
    // o hub nao liga; ligar so para um teste nao compensa. Conferir na tela.
    let mut layers = stack![base].width(Length::Fill).height(Length::Fill);
    if app.cat_menu_open {
        // Camada que fecha o menu ao clicar fora (dentro da area da tela) e, por cima, o menu.
        // A tela ocupa a altura toda da regiao de conteudo (720 de janela minima deixa perto de
        // 500 px), e o menu tem no maximo 42 + 360: cabe sem empurrar nada. Se a janela ficar
        // mais baixa que isso, o menu passa da borda de baixo e e cortado.
        let catcher = opaque(
            mouse_area(
                container(Space::new())
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
            .on_press(Message::CloseCategoryMenu),
        );
        let menu = column![
            Space::new().height(MENU_OFFSET),
            row![opaque(category_menu(app)), Space::new().width(Length::Fill)],
        ];
        layers = layers.push(catcher).push(menu);
    }
    layers.into()
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
            .spacing(space::M)
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
        let photo = app.iced.user_avatars.get(&author.id);
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
        SegmentedSize::Compact,
        false,
        [
            option(TypeFilter::All, "Todos"),
            option(TypeFilter::Income, "Receitas"),
            option(TypeFilter::Expense, "Despesas"),
        ],
    )
}

// ---- resumo do mes ----

/// "128 lançamentos   Receitas +R$ …   Despesas −R$ …   Saldo R$ …": a contagem e os totais do
/// que a tabela mostra (com filtro, so o filtrado), em 17 Regular com o rotulo em 12
/// `text_muted` (8.3). Os valores vem prontos do modelo, ja mascarados com "Ocultar valores".
/// Quebra em duas linhas quando nao cabe (728 de largura util).
fn summary_line(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let list = &app.transactions.list;
    let count = widgets::value_line(
        vec![
            (
                group_thousands(list.count as u64),
                type_scale::VALUE_S,
                fonts::INTER,
                t.text,
            ),
            (
                count_word(list.count).to_string(),
                type_scale::META,
                fonts::INTER,
                t.text_muted,
            ),
        ],
        type_scale::VALUE_S,
        6.0,
    );
    row![
        count,
        widgets::figure(t, "Receitas", &list.income_label, t.income_fg),
        widgets::figure(t, "Despesas", &list.expense_label, t.expense_fg),
        widgets::figure(t, "Saldo", &list.balance_label, t.text),
    ]
    .spacing(space::XXL)
    .align_y(Alignment::End)
    .wrap()
    .vertical_spacing(space::S)
    .into()
}

/// O resumo do mes aparece em todo vazio menos o do hub sem nada sincronizado: la, "0
/// lançamentos" e tres R$ 0,00 so repetiriam o vazio da tabela. No vazio por filtro e no mes
/// sem lancamento ele fica, e diz que o zero e do filtro ou do mes.
fn shows_summary(v: &TransactionsView) -> bool {
    !v.is_empty_hub
}

/// "lançamento" / "lançamentos", depois do numero da contagem.
fn count_word(count: usize) -> &'static str {
    if count == 1 {
        "lançamento"
    } else {
        "lançamentos"
    }
}

// ---- tabela ----

/// Largura util a partir da qual cada coluna opcional aparece. Pagamento sai primeiro, depois
/// Autor (8.3); na janela minima (728 de largura util) as duas ja sairam e a descricao, a
/// categoria e o valor ficam com o espaco. Com as quatro colunas, a 930 a de Pagamento tem ~126
/// px (fracao de (930 - 234) / 55); abaixo disso "Cartão de crédito" com o icone saia cortado.
/// A largura do texto e estimada (o iced nao mede antes do layout): conferir na tela.
const PAYMENT_MIN_WIDTH: f32 = 930.0;
const AUTHOR_MIN_WIDTH: f32 = 780.0;

/// Quais colunas opcionais a tabela mostra. Cabecalho e linhas leem a mesma resposta, para os
/// rotulos nunca ficarem sobre a coluna errada.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Visible {
    author: bool,
    payment: bool,
}

fn visible_columns(width: f32) -> Visible {
    Visible {
        author: width >= AUTHOR_MIN_WIDTH,
        payment: width >= PAYMENT_MIN_WIDTH,
    }
}

/// Celulas nas colunas da tabela (gap 16, padding horizontal 20), sem as que a largura tirou.
/// O valor fica alinhado a direita numa coluna fixa, como toda coluna de dinheiro.
fn columns<'a>(
    visible: Visible,
    cells: [Element<'a, Message>; 5],
) -> iced::widget::Row<'a, Message> {
    let [desc, cat, author, pay, value] = cells;
    let [w0, w1, w2, w3] = COLUMNS;
    let mut line = row![
        container(desc).width(Length::FillPortion(w0)).clip(true),
        container(cat).width(Length::FillPortion(w1)).clip(true),
    ];
    if visible.author {
        line = line.push(container(author).width(Length::FillPortion(w2)).clip(true));
    }
    if visible.payment {
        line = line.push(container(pay).width(Length::FillPortion(w3)).clip(true));
    }
    line.push(container(value).width(VALUE_WIDTH).align_x(Alignment::End))
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
    let visible = visible_columns(shell::content_width(app));
    let header = container(columns(
        visible,
        [
            head("Descrição"),
            head("Categoria"),
            head("Autor"),
            head("Pagamento"),
            head("Valor"),
        ],
    ))
    .height(40)
    .padding([0, 20])
    .align_y(Alignment::Center);
    let mut rows = column![];
    if v.list.count == 0 {
        rows = rows.push(empty(app));
    }
    let day_bg = mix(t.surface, t.bg, 0.45);
    for (i, group) in v.list.groups.iter().enumerate() {
        if i > 0 {
            rows = rows.push(widgets::divider(t));
        }
        rows = rows.push(
            container(
                row![
                    text(group.heading.clone())
                        .size(12)
                        .font(fonts::INTER_MEDIUM)
                        .color(t.text_soft())
                        .width(Length::Fill),
                    text(group.net.clone()).size(12).color(t.text_muted),
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
            rows = rows
                .push(widgets::divider(t))
                .push(table_row(app, r, visible));
        }
    }
    // So as linhas rolam (B7). Altura `Shrink`: o iced limita a rolagem ao que sobra abaixo dos
    // filtros, do resumo e do cabecalho, e com poucas linhas o card acaba nelas em vez de
    // esticar ate o pe da janela. A barra flutua sobre o respiro de 20 da direita (sem
    // `spacing`): reservar a largura dela estreitaria as linhas e a coluna Valor sairia da
    // linha do cabecalho. O id e o da casca, para a troca de tela voltar ao topo.
    let scroll = scrollable(rows)
        .id(shell::CONTENT_SCROLL)
        .direction(scrollable::Direction::Vertical(
            scrollable::Scrollbar::new()
                .width(6)
                .scroller_width(6)
                .margin(3),
        ))
        .style(theme::scrollbar(t))
        .width(Length::Fill)
        .height(Length::Shrink);
    // `padding(1)`: as faixas de dia sao opacas e de largura cheia, e o iced desenha a borda por
    // dentro; sem o respiro elas, os divisores e o hover da ultima linha cobrem a borda do card.
    widgets::card(t, column![header, widgets::divider(t), scroll])
        .padding(1)
        .clip(true)
        .into()
}

fn table_row<'a>(app: &'a App, r: &'a ListRow, visible: Visible) -> Element<'a, Message> {
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
        // Neutra: recorrencia e um fato da linha, nao o foco da tela, e o dourado fica para o
        // que marca o agora (principio 2 da direcao Grafite · Mesa).
        description =
            description.push(widgets::tag_icon(t, TagTone::Neutral, icons::REPEAT, label));
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
    let content = columns(
        visible,
        [
            description.into(),
            category.into(),
            author_cell(app, r),
            payment.into(),
            value_cell(t, r),
        ],
    );
    hover_row(
        app,
        &r.id,
        content,
        [0, 20],
        RowEdge::Flush,
        Some(size::ROW_RICH),
    )
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
        widgets::empty_icon(t, icons::FUNNEL_X),
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
        // Hub sem nada: o vazio da issue 02 (com "Conectar um celular"), sem o resumo do mes.
        let hub = build_transactions(&Dataset::default(), "2026-09-24", "2026-09", &none, false);
        let (title, _, clear) = empty_copy(&hub, &none);
        assert_eq!(title, "Nenhum lançamento sincronizado");
        assert!(!clear);
        assert!(!shows_summary(&hub));
        // Mes sem lancamento: o resumo fica e diz "0 lançamentos".
        let ds = fixtures();
        let may = build_transactions(&ds, "2026-09-24", "2026-05", &none, false);
        assert_eq!(
            empty_copy(&may, &none),
            (
                "Nada em maio".to_string(),
                "Nenhum lançamento neste mês".to_string(),
                false
            )
        );
        assert!(shows_summary(&may));
        assert_eq!(count_word(may.list.count), "lançamentos");
        // Vazio por filtro: como estava, com "Limpar filtros"; o resumo conta o filtrado (0),
        // nao os lancamentos do mes.
        let filters = Filters {
            kind: TypeFilter::Income,
            category: Some("C1".into()),
            ..Filters::default()
        };
        let sep = build_transactions(&ds, "2026-09-24", "2026-09", &filters, false);
        assert_eq!(sep.list.count, 0);
        let (title, body, clear) = empty_copy(&sep, &filters);
        assert_eq!(title, "Nada com esses filtros");
        assert!(body.contains("em setembro"));
        assert!(clear);
        assert!(shows_summary(&sep));
        let all = build_transactions(&ds, "2026-09-24", "2026-09", &none, false);
        assert!(all.list.count > 0);
        assert_eq!(count_word(1), "lançamento");
    }

    #[test]
    fn pagamento_sai_antes_do_autor_e_na_largura_minima_os_dois_sairam() {
        let both = Visible {
            author: true,
            payment: true,
        };
        // Janela inicial (1440 - 232 - 64).
        assert_eq!(visible_columns(1144.0), both);
        assert_eq!(visible_columns(PAYMENT_MIN_WIDTH), both);
        // Entre os dois limites: so Pagamento saiu (900 cortava "Cartão de crédito").
        assert_eq!(
            visible_columns(900.0),
            Visible {
                author: true,
                payment: false
            }
        );
        // Janela minima (1024 - 232 - 64).
        assert_eq!(
            visible_columns(728.0),
            Visible {
                author: false,
                payment: false
            }
        );
    }

    #[test]
    fn tabela_monta_nas_tres_faixas_de_largura() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = crate::ui::app::tests::app_with_fixtures(dir.path());
        app.screen = crate::ui::app::Screen::Transactions;
        for width in [1024.0, 1100.0, 1440.0] {
            app.window_size.w = width;
            let _ = view(&app);
        }
    }
}
