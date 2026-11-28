//! Vitrine de debug: `HUBFINANCE_DEMO=componentes` abre a janela direto aqui, com um exemplo de
//! cada estado, para o dono comparar com o artboard Componentes. Nunca existe em release.
//!
//! Hover e pressionado dependem do mouse e foco do Tab (so ligado na Tarefa 13), entao a vitrine
//! os desenha parados: `Button::forced_state` usa `button_colors(Hover/Pressed)` e o foco e o
//! mesmo anel de `focus::ring`. As colunas seguem o artboard: Padrão, Hover, Pressionado, Foco,
//! Desabilitado.

use std::sync::{Arc, LazyLock};

use gpui::{AnyElement, Div, IntoElement, SharedString, div, prelude::*, px};

use super::avatar::{Avatar, photo};
use super::badge::Badge;
use super::button::{Button, IconButton, State, Variant};
use super::card::Card;
use super::category_tile::CategoryTile;
use super::chip::{Chip, Pill};
use super::empty::EmptyState;
use super::focus;
use super::money::{Money, SIZES};
use super::month_picker::{MonthPicker, PartialChip};
use super::progress::Progress;
use super::radio::Radio;
use super::segmented::{Preview, Segmented, Surface};
use super::stat::Stat;
use super::status_chip::{Kind, StatusChip};
use super::switch::Switch;
use super::tooltip::{Line, TooltipView};
use crate::dashboard::money::{AXIS_MASK, MASK};
use crate::gui::fonts;
use crate::gui::icons::Icon;
use crate::gui::theme::Tokens;

/// `HUBFINANCE_DEMO=componentes`.
/// Foto da vitrine montada uma vez: `photo` copia e faz hash de todos os bytes a cada chamada.
static DEMO_PHOTO: LazyLock<Option<Arc<gpui::Image>>> =
    LazyLock::new(|| photo(include_bytes!("../../../assets/brand/hubfinance-512.png")));

pub fn requested() -> bool {
    std::env::var_os("HUBFINANCE_DEMO")
        .is_some_and(|v| v == "componentes" || v == "dados" || v == "campos")
}

/// `HUBFINANCE_DEMO=dados`: so a secao "Dados", sem rolar (a captura do smoke nao rola).
fn only_data() -> bool {
    std::env::var_os("HUBFINANCE_DEMO").is_some_and(|v| v == "dados")
}

/// `HUBFINANCE_DEMO=campos`: so a secao de campos, dialogo e aviso, acima da dobra.
fn only_fields() -> bool {
    std::env::var_os("HUBFINANCE_DEMO").is_some_and(|v| v == "campos")
}

fn section(title: &'static str, t: &'static Tokens) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(16.0))
        .p(px(24.0))
        .rounded(px(24.0))
        .bg(t.surface)
        .when_some(t.card_ring.clone(), |d, r| d.shadow(r.shadows.to_vec()))
        .child(
            div()
                .text_size(px(13.0))
                .font_weight(fonts::weight(600))
                .text_color(t.muted)
                .child(title.to_uppercase()),
        )
}

fn caption(text: &'static str, t: &'static Tokens) -> Div {
    div()
        .text_size(px(12.0))
        .text_color(t.muted)
        .child(SharedString::from(text))
}

/// O controle com o anel de foco desenhado a forca (`gap` = fundo atras dele).
fn focused(el: impl IntoElement, t: &'static Tokens, gap: gpui::Hsla) -> AnyElement {
    div()
        .rounded_full()
        .shadow(focus::ring(t, gap))
        .child(el)
        .into_any_element()
}

/// Liga o handle de Tab ao controle, quando a vitrine tem `Fields`.
fn with_focus<T>(
    el: T,
    handle: Option<&gpui::FocusHandle>,
    f: impl FnOnce(T, &gpui::FocusHandle) -> T,
) -> T {
    match handle {
        Some(h) => f(el, h),
        None => el,
    }
}

const COL: f32 = 120.0;
/// 25 botoes (5 linhas x 5 colunas), 3 icones, 4 interruptores, 3 radios e 2 pilulas.
const STOPS: usize = 37;

pub fn render(t: &'static Tokens, fields: Option<&Fields>) -> AnyElement {
    let gap = t.surface;
    let stop = |i: usize| fields.and_then(|f| f.stops.get(i));
    let rows: [(&str, Variant); 5] = [
        ("Primário", Variant::Primary),
        ("Contorno", Variant::Outline),
        ("Fantasma", Variant::Ghost),
        ("Perigo", Variant::Danger),
        ("Destrutivo", Variant::DangerFilled),
    ];
    let mut buttons = section("Botões", t);
    buttons = buttons.child(
        div()
            .flex()
            .gap(px(12.0))
            .child(div().w(px(100.0)))
            .children(
                ["Padrão", "Hover", "Pressionado", "Foco", "Desabilitado"]
                    .map(|h| div().w(px(COL)).child(caption(h, t))),
            ),
    );
    for (n, (name, v)) in rows.into_iter().enumerate() {
        let mk = |col: usize, state: Option<State>, disabled: bool| {
            let b = Button::new(SharedString::from(format!("b{n}{col}")), "Continuar", v, t)
                .ring_gap(gap)
                .disabled(disabled);
            let b = match stop(n * 5 + col) {
                Some(h) => b.focus(h),
                None => b,
            };
            match state {
                Some(s) => b.forced_state(s),
                None => b,
            }
        };
        buttons = buttons.child(
            div()
                .flex()
                .items_center()
                .gap(px(12.0))
                .child(
                    div()
                        .w(px(100.0))
                        .text_size(px(13.0))
                        .text_color(t.muted)
                        .child(name),
                )
                .child(div().w(px(COL)).child(mk(0, None, false)))
                .child(div().w(px(COL)).child(mk(1, Some(State::Hover), false)))
                .child(div().w(px(COL)).child(mk(2, Some(State::Pressed), false)))
                .child(div().w(px(COL)).child(focused(mk(3, None, false), t, gap)))
                .child(div().w(px(COL)).child(mk(4, None, true))),
        );
    }
    buttons = buttons.child(
        div()
            .flex()
            .items_center()
            .gap(px(12.0))
            .child(with_focus(
                IconButton::new("ib1", Icon::ChevronLeft, t).ring_gap(gap),
                stop(25),
                IconButton::focus,
            ))
            .child(focused(
                with_focus(
                    IconButton::new("ib3", Icon::ChevronLeft, t).ring_gap(gap),
                    stop(26),
                    IconButton::focus,
                ),
                t,
                gap,
            ))
            .child(with_focus(
                IconButton::new("ib2", Icon::ChevronRight, t).disabled(true),
                stop(27),
                IconButton::focus,
            ))
            .child(Button::new("bi", "Com ícone", Variant::Outline, t).icon(Icon::Copy)),
    );

    let controls = section("Controles", t)
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(24.0))
                .child(
                    Segmented::new(
                        "seg1",
                        vec!["Escuro".into(), "Claro".into()],
                        usize::from(!t.dark),
                        t,
                    )
                    .width(210.0),
                )
                .child(
                    Segmented::new("seg2", vec!["Mês".into(), "Ano".into()], 0, t)
                        .surface(Surface::Raised)
                        .width(210.0),
                ),
        )
        .child(
            // Hover num item inativo e foco num ativo (o ativo nao muda com o mouse).
            div()
                .flex()
                .items_center()
                .gap(px(24.0))
                .child(
                    Segmented::new("seg3", vec!["Escuro".into(), "Claro".into()], 0, t)
                        .preview(1, Preview::Hover)
                        .width(210.0),
                )
                .child(
                    Segmented::new("seg4", vec!["Escuro".into(), "Claro".into()], 0, t)
                        .preview(0, Preview::Focus)
                        .width(210.0),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(14.0))
                .child(with_focus(
                    Switch::new("sw1", true, t).ring_gap(gap),
                    stop(28),
                    Switch::focus,
                ))
                .child(with_focus(
                    Switch::new("sw2", false, t),
                    stop(29),
                    Switch::focus,
                ))
                .child(with_focus(
                    Switch::new("sw3", true, t).disabled(true),
                    stop(30),
                    Switch::focus,
                ))
                .child(focused(
                    with_focus(
                        Switch::new("sw4", true, t).ring_gap(gap),
                        stop(31),
                        Switch::focus,
                    ),
                    t,
                    gap,
                ))
                .child(div().w(px(24.0)))
                .child(with_focus(
                    Radio::new("r1", false, t),
                    stop(32),
                    Radio::focus,
                ))
                .child(with_focus(
                    Radio::new("r2", true, t),
                    stop(33),
                    Radio::focus,
                ))
                .child(focused(
                    with_focus(
                        Radio::new("r3", false, t).ring_gap(gap),
                        stop(34),
                        Radio::focus,
                    ),
                    t,
                    gap,
                )),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.0))
                .child(Chip::new("Últimos 12 meses", t))
                .child(with_focus(
                    Pill::new("pill", "12 meses", t).ring_gap(gap),
                    stop(35),
                    Pill::focus,
                ))
                .child(focused(
                    with_focus(
                        Pill::new("pill2", "12 meses", t).ring_gap(gap),
                        stop(36),
                        Pill::focus,
                    ),
                    t,
                    gap,
                ))
                .child(Badge::new(2, t))
                .child(Badge::new(2, t).dot()),
        );

    if only_fields() {
        return div()
            .id("showcase")
            .size_full()
            .overflow_y_scroll()
            .p(px(24.0))
            .children(fields.map(|f| interaction_section(t, f)))
            .into_any_element();
    }
    let data = data_section(t);
    if only_data() {
        return div()
            .id("showcase")
            .size_full()
            .overflow_y_scroll()
            .p(px(24.0))
            .child(data)
            .into_any_element();
    }
    div()
        .id("showcase")
        .size_full()
        .overflow_y_scroll()
        .p(px(24.0))
        .flex()
        .flex_col()
        .gap(px(20.0))
        .child(buttons)
        .child(controls)
        .child(data)
        .children(fields.map(|f| interaction_section(t, f)))
        .into_any_element()
}

/// Estados dos campos da vitrine: criados uma vez com a janela (um `InputState` e uma entidade
/// e nao pode nascer a cada quadro).
pub struct Fields {
    input: gpui::Entity<gpui_component::input::InputState>,
    secret: gpui::Entity<gpui_component::input::InputState>,
    area: gpui::Entity<gpui_component::input::TextareaState>,
    // Handles com `tab_stop(true)`, criados uma vez: sem eles so os campos entram no Tab.
    button: gpui::FocusHandle,
    // Controles das Tarefas 11 e 12 (botoes, interruptores, radios, pilulas), na ordem de leitura.
    stops: Vec<gpui::FocusHandle>,
    modal: gpui::FocusHandle,
    modal_close: gpui::FocusHandle,
    toast: super::toast::ToastFocus,
}

impl Fields {
    pub fn new(window: &mut gpui::Window, cx: &mut gpui::App) -> Fields {
        use super::text_field::{new_area, new_input};
        Fields {
            input: new_input(window, cx, "Modelo", false, |_, _| {}),
            secret: new_input(window, cx, "Chave da API", true, |_, _| {}),
            area: new_area(window, cx, "Pergunte sobre os achados…", |_, _| {}, |_| {}),
            button: cx.focus_handle().tab_stop(true),
            stops: (0..STOPS)
                .map(|_| cx.focus_handle().tab_stop(true))
                .collect(),
            modal: cx.focus_handle(),
            modal_close: cx.focus_handle().tab_stop(true),
            toast: super::toast::ToastFocus::new(cx),
        }
    }
}

/// Campos, etapas, dialogo e toast.
fn interaction_section(t: &'static Tokens, f: &Fields) -> impl IntoElement {
    use super::modal::Modal;
    use super::stepper::Stepper;
    use super::text_field::{TextArea, TextField};
    use super::toast::Toast;
    section("Campos, diálogo e aviso", t)
        .child(TextField::new(&f.input, t).mono(true).ring_gap(t.surface))
        .child(
            TextField::new(&f.secret, t)
                .mono(true)
                .text_size(14.0)
                .ring_gap(t.surface),
        )
        .child(
            div()
                .flex()
                .px(px(14.0))
                .rounded(px(12.0))
                .bg(t.surface_2)
                .child(TextArea::new(&f.area, t)),
        )
        .child(Stepper::new(["Confirmar", "Dados", "Revisar"], 1, t))
        .child(
            Button::new("demo-continuar", "Continuar", Variant::Primary, t)
                .ring_gap(t.surface)
                .focus(&f.button),
        )
        .child(
            div()
                .relative()
                .w_full()
                .h(px(420.0))
                .overflow_hidden()
                .rounded(px(16.0))
                .child(
                    Modal::new("demo-modal", t)
                        .header(Stepper::new(["Confirmar", "Dados", "Revisar"], 0, t))
                        .title("Remover Lucas do hub?")
                        .reduce_motion(true)
                        .focus(&f.modal)
                        .close_focus(&f.modal_close)
                        .on_close(|_, _| {})
                        .child(
                            div()
                                .text_size(px(14.0))
                                .text_color(t.muted)
                                .child("O celular deixa de sincronizar com este hub na hora."),
                        ),
                ),
        )
        .child(
            // O toast se posiciona no ancestral relativo: aqui, uma faixa so para ele.
            div().relative().w_full().h(px(110.0)).child(
                Toast::new("demo-toast", "Lucas foi removido do hub", t)
                    .detail("44\u{a0}itens apagados")
                    .action("Desfazer", |_, _| {})
                    .seconds_left(6)
                    .bar(0.62)
                    .focus(&f.toast)
                    .on_close(|_, _| {}),
            ),
        )
}

/// Componentes de dado: cards, valores (visivel e oculto), avatares, tiles, chips, seletor de
/// mes, tooltips e estado vazio.
fn data_section(t: &'static Tokens) -> impl IntoElement {
    let row = || div().flex().items_center().gap(px(16.0));
    let tip = |v: TooltipView| {
        div().child(
            // O tooltip e uma view; na vitrine desenhamos o conteudo direto, sem hover.
            v.render_static(),
        )
    };
    section("Dados", t)
        .child(charts_row(t))
        .child(
            row()
                .child(
                    Card::new(t)
                        .child(
                            Stat::new("stat-visivel", "Receitas", "R$ 14.180,00", t)
                                .note("novembro"),
                        )
                        .w(px(220.0)),
                )
                .child(
                    Card::new(t)
                        .child(Stat::new("stat-oculto", "Receitas", MASK, t))
                        .w(px(220.0)),
                )
                .child(
                    Card::highlight(t)
                        .gap(px(10.0))
                        .child(
                            Money::new("money-meta", "R$ 6,3 mil", 20.0, t).color(t.on_highlight),
                        )
                        .child(Progress::new(3.0, 4.0, t).fill(t.on_highlight))
                        .w(px(220.0)),
                ),
        )
        .child(
            row()
                .items_baseline()
                .children(SIZES.map(|z| Money::new(("visivel", z as u64), "R$ 12.345,67", z, t)))
                .child(Money::new("money-saida", "−R$ 96,40", 15.0, t)),
        )
        .child(
            row()
                .items_baseline()
                .children(SIZES.map(|z| Money::new(("oculto", z as u64), MASK, z, t)))
                .child(Money::new("money-pct", "75,6%", 15.0, t))
                .child(Money::new("oculto-num", AXIS_MASK, 15.0, t)),
        )
        .child(
            row()
                .child(Avatar::new("Luiz", "sky", t))
                .child(Avatar::new("Ágata", "rose", t).size(48.0))
                .child(Avatar::new("Foto", "sky", t).photo(DEMO_PHOTO.clone()))
                .child(CategoryTile::new("shopping-cart", "orange", t))
                .child(CategoryTile::new("desconhecida", "violet", t))
                .child(StatusChip::new(Kind::Ok, "Emergência coberta", t))
                .child(StatusChip::new(Kind::Warning, "Ainda não coberta", t)),
        )
        .child(
            row()
                .child(MonthPicker::new("mp1", "Novembro 2026", false, t))
                .child(MonthPicker::new("mp2", "Outubro 2026", true, t))
                .child(PartialChip::new("Parcial · até 27 nov", t)),
        )
        .child(
            row()
                .items_start()
                .child(tip(TooltipView::rows(
                    "Outubro",
                    vec![
                        Line::Pair {
                            label: "Receitas".into(),
                            value: "R$ 14.180,00".into(),
                            bold: false,
                        },
                        Line::Pair {
                            label: "Sobrou".into(),
                            value: "R$ 2.870,40".into(),
                            bold: true,
                        },
                    ],
                    t,
                )))
                .child(tip(TooltipView::text(
                    "Média das despesas essenciais dos últimos 6 meses.",
                    t,
                ))),
        )
        .child(
            row()
                .items_start()
                .child(
                    Card::new(t)
                        .child(EmptyState::new(
                            Icon::Pulse,
                            "Sem movimentações neste mês",
                            "Nada foi guardado ou retirado em maio\u{a0}de\u{a0}2025.",
                            t,
                        ))
                        .px(px(24.0))
                        .py(px(32.0))
                        .w(px(300.0)),
                )
                .child(
                    Card::new(t)
                        .child(
                            EmptyState::new(
                                Icon::Calendar,
                                "Nenhum lançamento neste mês",
                                "Nada foi registrado em maio\u{a0}de\u{a0}2025.",
                                t,
                            )
                            .action(Button::new(
                                "vazio-acao",
                                "Ir para o mês atual",
                                Variant::Outline,
                                t,
                            )),
                        )
                        .px(px(24.0))
                        .py(px(32.0))
                        .w(px(300.0)),
                ),
        )
}

/// Um exemplo de cada grafico da Tarefa 16 (Historico, rosca, anel, area empilhada).
fn charts_row(t: &'static Tokens) -> impl IntoElement {
    use crate::gui::charts::bars::{BarMonth, BarTip, GroupedBars, MiniBars};
    use crate::gui::charts::donut::Donut;
    use crate::gui::charts::ring::Ring;
    use crate::gui::charts::stacked::StackedArea;

    let months: Vec<BarMonth> = [
        None,
        None,
        Some((900_000, 700_000)),
        Some((1_200_000, 800_000)),
        Some((1_000_000, 1_100_000)),
        Some((1_418_000, 1_131_000)),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, values)| BarMonth {
        values,
        // Exemplo do design (artboard Main/Componentes): Outubro com os tres valores.
        tip: values
            .map(|_| BarTip::month("Outubro", "R$ 14.180,00", "R$ 11.309,60", "R$ 2.870,40")),
        partial: i == 5,
    })
    .collect();
    div()
        .flex()
        .items_end()
        .gap(px(24.0))
        .child(div().w(px(260.0)).child(GroupedBars::new(
            "hist-demo",
            "Historico: receitas e despesas por mes",
            months,
            1_600_000,
            140.0,
            t,
        )))
        .child(Donut::new(
            "Para onde foi",
            vec![500, 300, 150, 40, 9],
            t.chart[..5].to_vec(),
            150.0,
            t.divider,
        ))
        .child(
            div()
                .w(px(160.0))
                .p(px(8.0))
                .rounded(px(12.0))
                .bg(t.highlight)
                .child(
                    MiniBars::new(
                        "Despesas por mes",
                        vec![Some(40), Some(70), None, Some(55), Some(90), Some(60)],
                        100,
                        64.0,
                        t,
                    )
                    .partial(Some(5)),
                ),
        )
        .child(Ring::new(
            "Reserva de emergencia",
            0.756,
            104.0,
            t.accent,
            t.divider,
        ))
        .child(
            div().w(px(260.0)).child(
                StackedArea::new(
                    "Evolucao",
                    vec![vec![100, 200, 300], vec![50, 100, 100]],
                    vec![t.chart[0], t.chart[2]],
                    600,
                    120.0,
                    t.surface,
                )
                .partial(true),
            ),
        )
}
