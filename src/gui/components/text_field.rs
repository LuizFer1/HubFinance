//! Campos de texto do design sobre o `gpui-component` (`InputState`/`Input`, `TextareaState`/
//! `Textarea`): edicao, selecao, IME e colar vem dele; o quadro (44 px, raio 12, `surface_2`,
//! borda `divider`, anel de foco de 2 px) e do design e e desenhado aqui, por isso o `Input`
//! entra com `appearance(false)` (o tema do `gpui-component` nao tem o anel afastado).
//!
//! O `ui_model` continua dono do texto: cada mudanca sai pelo `on_change` (quem cria o estado
//! manda o `Msg`; a chave do assistente vira `Secret` la, nunca aqui) e a tela devolve valor
//! vindo do modelo por `sync_value`, que so escreve se o texto diferir, porque `set_value`
//! recoloca o cursor e um eco do proprio digitado faria o cursor pular.

use gpui::{
    App, AppContext as _, Entity, FocusHandle, Focusable as _, Hsla, IntoElement, RenderOnce,
    SharedString, Window, div, prelude::*, px,
};
use gpui_component::input::{Input, InputEvent, InputState, Textarea, TextareaState};

use super::focus;
use crate::gui::fonts;
use crate::gui::theme::Tokens;

pub const HEIGHT: f32 = 44.0;
pub const RADIUS: f32 = 12.0;
/// Linhas do chat: comeca em 1 e cresce ate 6 (spec).
pub const AREA_MIN_ROWS: usize = 1;
pub const AREA_MAX_ROWS: usize = 6;
/// Familia monoespacada dos campos de chave e modelo (artboard Assistente).
pub const MONO: &str = "Cascadia Mono";

/// Cria o estado de um campo de uma linha; `masked` = senha. `on_change` recebe o texto atual a
/// cada edicao.
pub fn new_input(
    window: &mut Window,
    cx: &mut App,
    placeholder: impl Into<SharedString>,
    masked: bool,
    on_change: impl Fn(String, &mut App) + 'static,
) -> Entity<InputState> {
    let placeholder = placeholder.into();
    let state = cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder(placeholder)
            .masked(masked)
    });
    cx.subscribe(&state, move |state, event: &InputEvent, cx| {
        if matches!(event, InputEvent::Change) {
            on_change(state.read(cx).value().to_string(), cx);
        }
    })
    .detach();
    state
}

/// Cria o estado do campo do chat. Enter envia (`PressEnter`), Shift+Enter quebra a linha.
/// `on_submit` roda no Enter; `on_change` a cada edicao.
pub fn new_area(
    window: &mut Window,
    cx: &mut App,
    placeholder: impl Into<SharedString>,
    on_change: impl Fn(String, &mut App) + 'static,
    on_submit: impl Fn(&mut App) + 'static,
) -> Entity<TextareaState> {
    let placeholder = placeholder.into();
    let state = cx.new(|cx| {
        TextareaState::new(window, cx)
            .auto_grow(AREA_MIN_ROWS, AREA_MAX_ROWS)
            .placeholder(placeholder)
            .submit_on_enter(true)
    });
    cx.subscribe(&state, move |state, event: &InputEvent, cx| match event {
        InputEvent::Change => on_change(state.read(cx).value().to_string(), cx),
        InputEvent::PressEnter { shift: false, .. } => on_submit(cx),
        _ => {}
    })
    .detach();
    state
}

/// Devolve ao campo um valor que veio do modelo, sem mexer no cursor se ja for o mesmo texto.
pub fn sync_value(state: &Entity<InputState>, value: &str, window: &mut Window, cx: &mut App) {
    if state.read(cx).value().as_ref() != value {
        let value = value.to_string();
        state.update(cx, |s, cx| s.set_value(value, window, cx));
    }
}

/// O mesmo para o campo do chat (limpar depois de enviar).
pub fn sync_area(state: &Entity<TextareaState>, value: &str, window: &mut Window, cx: &mut App) {
    if state.read(cx).value().as_ref() != value {
        let value = value.to_string();
        state.update(cx, |s, cx| s.set_value(value, window, cx));
    }
}

/// Cor da borda: com foco continua `divider`. O design so acrescenta o anel (`focus-visible`)
/// sem mexer no border do campo; trocar a borda aqui daria dois acentos empilhados.
pub fn border_color(_focused: bool, t: &Tokens) -> Hsla {
    t.divider
}

#[derive(IntoElement)]
pub struct TextField {
    state: Entity<InputState>,
    tokens: &'static Tokens,
    mono: bool,
    text_size: Option<f32>,
    ring_gap: Option<Hsla>,
    aria_label: Option<SharedString>,
    disabled: bool,
}

impl TextField {
    pub fn new(state: &Entity<InputState>, tokens: &'static Tokens) -> TextField {
        TextField {
            state: state.clone(),
            tokens,
            mono: false,
            text_size: None,
            ring_gap: None,
            aria_label: None,
            disabled: false,
        }
    }

    pub fn mono(mut self, mono: bool) -> TextField {
        self.mono = mono;
        self
    }

    /// Tamanho da fonte em px. Separado da familia: no artboard Assistente o campo do modelo e
    /// mono a 13 px e o da chave e mono a 14 px.
    pub fn text_size(mut self, size: f32) -> TextField {
        self.text_size = Some(size);
        self
    }

    /// Cor do fundo atras do campo (o afastamento do anel de foco).
    pub fn ring_gap(mut self, gap: Hsla) -> TextField {
        self.ring_gap = Some(gap);
        self
    }

    pub fn aria_label(mut self, label: impl Into<SharedString>) -> TextField {
        self.aria_label = Some(label.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> TextField {
        self.disabled = disabled;
        self
    }

    pub fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.state.focus_handle(cx)
    }
}

impl RenderOnce for TextField {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = self.tokens;
        let focused = self.state.focus_handle(cx).contains_focused(window, cx);
        let mut input = Input::new(&self.state)
            .appearance(false)
            .bordered(false)
            .focus_bordered(false)
            .disabled(self.disabled);
        if let Some(label) = self.aria_label {
            input = input.aria_label(label);
        }
        div()
            .flex()
            .items_center()
            .w_full()
            .h(px(HEIGHT))
            .px(px(14.0))
            .rounded(px(RADIUS))
            .bg(t.surface_2)
            .border_1()
            .border_color(border_color(focused, t))
            .text_size(px(self.text_size.unwrap_or(if self.mono {
                13.0
            } else {
                14.0
            })))
            .text_color(t.text)
            .when(self.mono, |d| d.font_family(MONO))
            .when(!self.mono, |d| d.font_family(fonts::FAMILY))
            .when(focused, |d| {
                d.shadow(focus::ring(t, self.ring_gap.unwrap_or(t.bg)))
            })
            .when(self.disabled, |d| d.opacity(0.4))
            .child(input)
    }
}

/// Campo do chat: sem quadro proprio (o cartao do chat o fornece), transparente, 14 px.
#[derive(IntoElement)]
pub struct TextArea {
    state: Entity<TextareaState>,
    tokens: &'static Tokens,
    aria_label: Option<SharedString>,
}

impl TextArea {
    pub fn new(state: &Entity<TextareaState>, tokens: &'static Tokens) -> TextArea {
        TextArea {
            state: state.clone(),
            tokens,
            aria_label: None,
        }
    }

    pub fn aria_label(mut self, label: impl Into<SharedString>) -> TextArea {
        self.aria_label = Some(label.into());
        self
    }
}

impl RenderOnce for TextArea {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let t = self.tokens;
        let mut area = Textarea::new(&self.state).appearance(false).bordered(false);
        if let Some(label) = self.aria_label {
            area = area.aria_label(label);
        }
        div()
            .flex_1()
            .min_w(px(0.0))
            .py(px(6.0))
            .text_size(px(14.0))
            .text_color(t.text)
            .font_family(fonts::FAMILY)
            .child(area)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::theme::{DARK, LIGHT};

    #[test]
    fn borda_continua_divider_com_foco() {
        for t in [&*DARK, &*LIGHT] {
            assert_eq!(border_color(true, t), t.divider);
            assert_eq!(border_color(false, t), t.divider);
        }
    }

    #[test]
    fn chat_cresce_de_uma_a_seis_linhas() {
        assert_eq!((AREA_MIN_ROWS, AREA_MAX_ROWS), (1, 6));
        assert_eq!(HEIGHT, 44.0);
    }
}
