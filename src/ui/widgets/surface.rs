//! Superficies: card, caixa interna. A cor e a borda vem de `theme`; aqui so o respiro e a
//! largura padrao, para cada tela parar de montar o proprio `container` com o estilo.

use iced::widget::text::Wrapping;
use iced::widget::{
    Column, Container, Space, column, container, mouse_area, opaque, row, stack, text,
};
use iced::{Alignment, Color, Element, Length, Padding};

use crate::ui::theme::{self, Tokens, Tone, radius, size, space, type_scale};
use crate::ui::{fonts, icons};

/// Card (nivel 0): `surface`, raio 8, borda `card_border`, padding 20 e largura cheia. Devolve o
/// `Container` para a tela trocar o padding (os cards do Dashboard tem respiros proprios) ou
/// acrescentar `clip`.
pub fn card<'a, M: 'a>(t: &'static Tokens, content: impl Into<Element<'a, M>>) -> Container<'a, M> {
    container(content)
        .padding(space::CARD)
        .width(Length::Fill)
        .style(theme::card(t))
}

/// Caixa interna (dentro de card ou dialogo): `surface_2`, raio 6, sem borda nem linha
/// interativa (o hover a 4 % some sobre ela). Padding 12; a previa do envelope usa 4 para o
/// cursor da rolagem nao encostar no canto.
pub fn inset<'a, M: 'a>(
    t: &'static Tokens,
    content: impl Into<Element<'a, M>>,
) -> Container<'a, M> {
    container(content)
        .padding(space::M)
        .width(Length::Fill)
        .style(theme::inset(t))
}

/// Divisoria horizontal de 1 px no divisor, na largura toda (linhas de lista, topo da barra de
/// titulo). E container e nao borda porque o iced nao tem borda de um lado so.
pub fn divider<'a, M: 'a>(t: &'static Tokens) -> Element<'a, M> {
    container(Space::new())
        .width(Length::Fill)
        .height(size::BORDER)
        .style(theme::fill(t.divider(), 0.0))
        .into()
}

/// Separador vertical de 1 x 20 no divisor, com 4 px de margem dos lados (barra de filtros).
pub fn vdivider<'a, M: 'a>(t: &'static Tokens) -> Element<'a, M> {
    container(
        container(Space::new())
            .width(size::BORDER)
            .height(20)
            .style(theme::fill(t.divider(), 0.0)),
    )
    .padding([0.0, space::XS])
    .into()
}

/// Linha de ajuste: rotulo 14 em `fg` a esquerda, ocupando o que sobra, com um detalhe
/// opcional embaixo (frase de apoio, caminho de pasta), e o controle a direita. Altura minima
/// de 52 px (`size::ROW_RICH`), e nao fixa: o detalhe pode quebrar (o iced 0.14 nao tem
/// reticencias, e um caminho cortado nao serve) e a linha cresce com ele. Um `Space` de 52 de
/// altura e largura zero segura a altura (o `container` do iced nao tem altura minima); ele
/// fica numa linha de fora, sem espacamento, porque o `spacing` do iced vale tambem depois de
/// filho de largura zero e empurraria o rotulo 16 px para dentro, desalinhando-o das notas
/// soltas da tela.
pub fn settings_row<'a, M: 'a>(
    t: &'static Tokens,
    label: &str,
    detail: Option<Element<'a, M>>,
    control: impl Into<Element<'a, M>>,
) -> Element<'a, M> {
    let mut left = column![text(label.to_string()).size(type_scale::BODY).color(t.text)].spacing(2);
    if let Some(detail) = detail {
        left = left.push(detail);
    }
    let line = row![
        container(left.padding([space::S, 0.0])).width(Length::Fill),
        control.into()
    ]
    .spacing(space::L)
    .align_y(Alignment::Center)
    .width(Length::Fill);
    row![Space::new().width(0).height(size::ROW_RICH), line]
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .into()
}

/// Titulo de secao, fora do card: titulo 16 Medium, pergunta 13 `text_muted` ao lado (vazia
/// nao entra) e um aparte 12 a direita. `top` e o respiro acima, que o Resumo varia por secao.
pub fn section_heading<'a, M: 'a>(
    t: &'static Tokens,
    title: &str,
    question: &str,
    aside: Option<&str>,
    top: f32,
) -> Element<'a, M> {
    let mut line = row![
        text(title.to_string())
            .size(type_scale::SECTION)
            .font(fonts::INTER_MEDIUM)
            .wrapping(Wrapping::None)
            .color(t.text)
    ]
    .spacing(space::M)
    .align_y(Alignment::End);
    if !question.is_empty() {
        line = line.push(
            text(question.to_string())
                .size(type_scale::LABEL)
                .color(t.text_muted),
        );
    }
    if let Some(aside) = aside {
        line = line.push(
            container(
                text(aside.to_string())
                    .size(type_scale::META)
                    .wrapping(Wrapping::None)
                    .color(t.text_muted),
            )
            .width(Length::Fill)
            .align_x(Alignment::End),
        );
    }
    container(line)
        .padding(Padding {
            top,
            ..Padding::ZERO
        })
        .into()
}

/// Largura maxima do texto de um estado vazio (relatorio 8.7).
const EMPTY_TEXT_MAX: f32 = 420.0;

/// Texto de estado vazio: titulo 18 Medium e corpo 14 `text_muted` ate 420 px, 8 px entre os
/// dois. Devolve a `Column` (sem espaco proprio) para a tela acrescentar embaixo a linha de
/// espera ou a acao com o respiro que ja tinha. `centered` alinha ao centro (vazio sozinho no
/// card); sem ele, a esquerda (ao lado do esboco em canvas).
pub fn empty_copy<'a, M: 'a>(
    t: &'static Tokens,
    title: &str,
    body: &str,
    centered: bool,
) -> Column<'a, M> {
    let align = if centered {
        Alignment::Center
    } else {
        Alignment::Start
    };
    column![
        text(title.to_string())
            .size(type_scale::LEAD)
            .font(fonts::INTER_MEDIUM)
            .color(t.text),
        Space::new().height(space::S),
        text(body.to_string())
            .size(type_scale::BODY)
            .line_height(1.5)
            .color(t.text_muted)
            .align_x(align),
    ]
    .max_width(EMPTY_TEXT_MAX)
    .align_x(align)
}

/// Icone de estado vazio: o glifo de 20 num circulo de 44 com contorno de 1,5 em
/// `neutral_700` (o celular dos usuarios conectados).
pub fn empty_icon<'a, M: 'a>(t: &'static Tokens, glyph: &'static str) -> Element<'a, M> {
    let ring = t.neutral_700;
    container(icons::icon(glyph, 20.0, ring))
        .center(size::CTRL_L)
        .style(move |_: &iced::Theme| container::Style {
            border: iced::Border {
                color: ring,
                // 1,5 e o anel do avatar (spec 9): o icone de vazio repete o circulo do
                // avatar dos conectados, e nao a borda de 1 px dos cards.
                width: 1.5,
                radius: (size::CTRL_L / 2.0).into(),
            },
            ..container::Style::default()
        })
        .into()
}

/// Largura minima da coluna de texto do card com esboco, ao lado do desenho.
const SKETCH_TEXT_MIN: f32 = 260.0;

/// Card de primeiro uso com esboco (Resumo e Reservas): o desenho tracejado a esquerda e o texto
/// (`copy`, com a acao abaixo) a direita, 56 entre os dois e respiro de 56 x 32. E o card do
/// Resumo, validado; as Reservas usam o mesmo. O `flex-wrap` do prototipo vira uma quebra manual
/// (o iced nao tem): numa area de conteudo `content_width` estreita demais para esboco, vao e
/// 260 de texto, o texto desce para baixo do desenho. Na janela minima (728 de conteudo) os dois
/// ainda cabem lado a lado.
pub fn empty_sketch_card<'a, M: 'a>(
    t: &'static Tokens,
    sketch: Element<'a, M>,
    sketch_width: f32,
    copy: Column<'a, M>,
    action: Option<Element<'a, M>>,
    content_width: f32,
) -> Element<'a, M> {
    let copy = match action {
        Some(action) => copy.push(Space::new().height(space::XL)).push(action),
        None => copy,
    };
    let inner = content_width - 2.0 * space::XXXL;
    let body: Element<'a, M> = if inner >= sketch_width + space::GIANT + SKETCH_TEXT_MIN {
        row![sketch, copy]
            .spacing(space::GIANT)
            .align_y(Alignment::Center)
            .into()
    } else {
        column![sketch, copy].spacing(space::XXXL).into()
    };
    card(t, body).padding([space::GIANT, space::XXXL]).into()
}

/// Estado vazio centrado: icone, `empty_copy` e no maximo uma acao (navegar ou limpar filtro:
/// o hub nunca cria dado). A tela poe o respiro em volta.
pub fn empty_state<'a, M: 'a>(
    t: &'static Tokens,
    icon: Element<'a, M>,
    title: &str,
    body: &str,
    action: Option<Element<'a, M>>,
) -> Element<'a, M> {
    let mut col = column![
        icon,
        Space::new().height(space::M),
        empty_copy(t, title, body, true)
    ]
    .align_x(Alignment::Center);
    if let Some(action) = action {
        col = col.push(Space::new().height(space::L)).push(action);
    }
    col.into()
}

/// Margem minima do dialogo para a borda da janela (24 de cada lado).
const DIALOG_MARGIN: f32 = 48.0;

/// Largura do dialogo numa janela de `window_width`: ate `max_width`, deixando a margem, e
/// nunca abaixo de 240.
fn dialog_width(window_width: f32, max_width: f32) -> f32 {
    (window_width - DIALOG_MARGIN).clamp(240.0, max_width.max(240.0))
}

/// Dialogo sobre o scrim (nivel 2): caixa `surface` de raio 12 com `shadow_lg` e padding 24,
/// ate `max_width` de largura (menos a margem na janela estreita) e, com `height`, altura fixa.
/// A caixa e `opaque`: o clique dentro dela nao chega ao scrim, que fecha com `dismiss`.
/// `scrim` e a fracao da opacidade do fundo (`Motion::scrim`): so ele anima, a caixa entra
/// inteira, para o texto nunca aparecer pela metade.
pub fn dialog<'a, M: Clone + 'a>(
    t: &'static Tokens,
    content: impl Into<Element<'a, M>>,
    window_width: f32,
    max_width: f32,
    height: Option<f32>,
    dismiss: M,
    scrim: f32,
) -> Element<'a, M> {
    let dialog_box = container(content)
        .width(dialog_width(window_width, max_width))
        .padding(space::DIALOG)
        .style(theme::dialog(t));
    let dialog_box = match height {
        Some(h) => dialog_box.height(h),
        None => dialog_box,
    };
    let scrim = opaque(
        mouse_area(
            container(Space::new())
                .width(Length::Fill)
                .height(Length::Fill)
                .style(theme::scrim(scrim)),
        )
        .on_press(dismiss),
    );
    stack![scrim, container(opaque(dialog_box)).center(Length::Fill)]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Cor do icone de um tom: a de texto de cada um (`accent` cru nao vira texto nem icone).
fn tone_fg(t: &Tokens, tone: Tone) -> Color {
    match tone {
        Tone::Accent => t.text_accent(),
        Tone::Income => t.income_fg,
        Tone::Expense => t.expense_fg,
        Tone::Warning => t.warning,
    }
}

/// Estilo do banner: a tinta de 10 % do tom sobre o `parent` (opaca, entao o pai precisa ser o
/// fundo de verdade: `bg` no topo da pagina, `surface` dentro de card), raio 8, sem contorno.
pub fn banner_style(
    t: &'static Tokens,
    tone: Tone,
    parent: Color,
) -> impl Fn(&iced::Theme) -> container::Style {
    move |_| container::Style {
        background: Some(t.tint(tone, parent).into()),
        border: iced::Border {
            radius: radius::CONTROL.into(),
            ..iced::Border::default()
        },
        text_color: Some(t.text),
        ..container::Style::default()
    }
}

/// Banner: icone no tom, mensagem 13 em `fg` e as acoes a direita, sobre a tinta do tom.
pub fn banner<'a, M: 'a>(
    t: &'static Tokens,
    tone: Tone,
    parent: Color,
    glyph: &'static str,
    message: impl text::IntoFragment<'a>,
    actions: Vec<Element<'a, M>>,
) -> Element<'a, M> {
    let mut line = row![
        icons::icon(glyph, 18.0, tone_fg(t, tone)),
        text(message)
            .size(type_scale::LABEL)
            .color(t.text)
            .width(Length::Fill),
    ]
    .spacing(space::M)
    .align_y(Alignment::Center);
    for action in actions {
        line = line.push(action);
    }
    container(line)
        .padding([10.0, 14.0])
        .width(Length::Fill)
        .style(banner_style(t, tone, parent))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::{DARK, LIGHT};

    #[test]
    fn card_e_caixa_montam_nos_dois_temas() {
        for t in [&DARK, &LIGHT] {
            let _: Element<'_, ()> = card(t, "conteudo").into();
            let _: Element<'_, ()> = inset(t, "conteudo").padding(4).into();
            let _: Element<'_, ()> = divider(t);
            let _: Element<'_, ()> = vdivider(t);
            let _: Element<'_, ()> = settings_row(t, "Tema", None, "controle");
            let _: Element<'_, ()> =
                settings_row(t, "Dados em", Some(text(r"C:\dados").into()), "Copiar");
            let _: Element<'_, ()> = section_heading(t, "Título", "pergunta", Some("aparte"), 8.0);
            let _: Element<'_, ()> = section_heading(t, "Título", "", None, 0.0);
            let _: Element<'_, ()> = empty_state(
                t,
                empty_icon(t, icons::DEVICE_MOBILE),
                "Nenhum usuário conectado",
                "Quando um celular abrir o endereço, a pessoa aparece aqui",
                Some(text("Ação").into()),
            );
            let _: Element<'_, ()> = empty_copy(t, "Nada", "Corpo", false).into();
            let _: Element<'_, u8> = dialog(t, text("Corpo"), 1280.0, 460.0, None, 1, 0.5);
            let _: Element<'_, u8> = dialog(t, text("Corpo"), 1280.0, 720.0, Some(640.0), 1, 1.0);
            let _: Element<'_, ()> = banner(
                t,
                Tone::Warning,
                t.bg,
                icons::WARNING_CIRCLE,
                "Os dados mudaram",
                vec![text("Ação").into()],
            );
        }
    }

    #[test]
    fn banner_tinge_o_pai_a_10_por_cento_sem_contorno() {
        for t in [&DARK, &LIGHT] {
            let theme = t.iced_theme();
            for tone in [Tone::Accent, Tone::Income, Tone::Expense, Tone::Warning] {
                for parent in [t.bg, t.surface] {
                    let s = banner_style(t, tone, parent)(&theme);
                    assert_eq!(s.background, Some(t.tint(tone, parent).into()));
                    assert_eq!(s.border.width, 0.0);
                    assert_eq!(s.border.radius, radius::CONTROL.into());
                }
            }
        }
    }

    /// Diálogo com raio 12 (eram 14) e a sombra grande; o contorno e o da sombra.
    #[test]
    fn caixa_do_dialogo_tem_raio_12_e_sombra_grande() {
        for t in [&DARK, &LIGHT] {
            let s = theme::dialog(t)(&t.iced_theme());
            assert_eq!(s.border.radius, radius::DIALOG.into());
            assert_eq!(s.shadow, t.shadow_lg().1);
        }
    }

    #[test]
    fn largura_do_dialogo_deixa_margem_e_tem_piso() {
        assert_eq!(dialog_width(1280.0, 460.0), 460.0);
        assert_eq!(dialog_width(500.0, 720.0), 452.0);
        assert_eq!(dialog_width(200.0, 460.0), 240.0);
    }
}
