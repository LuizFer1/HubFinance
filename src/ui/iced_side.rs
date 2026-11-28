//! O que o `App` guarda e so existe por causa do iced: matriz do QR, geometria em cache dos
//! graficos, o tema do iced, as texturas das fotos e o mapa de ids de janela. Fica junto num
//! campo so para o estado da janela ficar sem tipo de toolkit (a Tarefa 6 separa de vez).

use std::collections::HashMap;

use iced::widget::{canvas, image, qr_code};
use iced::window;

use super::theme::Tokens;
use crate::ui_model::window::WindowId;

pub struct IcedSide {
    /// Reconstruido so quando o token muda: gerar a matriz a cada quadro e desperdicio.
    pub(super) qr: Option<qr_code::Data>,
    /// Geometria dos graficos; limpa quando dados, mes, tema ou "Ocultar valores" mudam (os
    /// rotulos em R$ ficam gravados nela).
    pub(super) pace_cache: canvas::Cache,
    pub(super) history_cache: canvas::Cache,
    pub(super) empty_cache: canvas::Cache,
    /// Grafico empilhado e esqueleto vazio da tela Reservas.
    pub(super) reserves_cache: canvas::Cache,
    pub(super) reserves_empty_cache: canvas::Cache,
    /// `Theme::custom` gera a paleta estendida a cada chamada; guardado para nao refazer isso
    /// a cada quadro.
    pub(super) iced_theme: iced::Theme,
    /// `device_id` -> textura da foto (o `App` guarda os bytes; o `Handle` reaproveita a
    /// textura enquanto o mesmo valor viver, entao so se refaz quando a foto muda).
    pub(super) avatars: HashMap<String, image::Handle>,
    /// `user_id` -> textura da foto do perfil.
    pub(super) user_avatars: HashMap<String, image::Handle>,
    /// `WindowId` do modelo -> id do iced. O modelo so conhece o primeiro.
    windows: HashMap<WindowId, window::Id>,
}

impl IcedSide {
    pub fn new(tokens: &Tokens) -> IcedSide {
        IcedSide {
            qr: None,
            pace_cache: canvas::Cache::new(),
            history_cache: canvas::Cache::new(),
            empty_cache: canvas::Cache::new(),
            reserves_cache: canvas::Cache::new(),
            reserves_empty_cache: canvas::Cache::new(),
            iced_theme: tokens.iced_theme(),
            avatars: HashMap::new(),
            user_avatars: HashMap::new(),
            windows: HashMap::new(),
        }
    }

    pub(super) fn clear_charts(&self) {
        self.pace_cache.clear();
        self.history_cache.clear();
        self.empty_cache.clear();
        self.reserves_cache.clear();
        self.reserves_empty_cache.clear();
    }

    /// Anota o id do iced e devolve o que o modelo guarda.
    pub(super) fn register(&mut self, id: window::Id) -> WindowId {
        let model = model_id(id);
        self.windows.insert(model, id);
        model
    }

    pub(super) fn forget(&mut self, id: WindowId) {
        self.windows.remove(&id);
    }

    pub(super) fn iced_id(&self, id: WindowId) -> Option<window::Id> {
        self.windows.get(&id).copied()
    }
}

/// O id do modelo de uma janela do iced. O iced numera as janelas com um contador; o `Display`
/// do `window::Id` e esse numero, e as assinaturas (que nao capturam estado) precisam traduzir
/// sem consultar o mapa.
pub fn model_id(id: window::Id) -> WindowId {
    WindowId(id.to_string().parse().unwrap_or(0))
}
