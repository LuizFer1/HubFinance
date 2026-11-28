//! O que o `App` guarda e so existe por causa do iced: matriz do QR, geometria em cache dos
//! graficos, o tema do iced, as texturas das fotos e o mapa de ids de janela. Fica junto num
//! campo so para o estado da janela ficar sem tipo de toolkit (a Tarefa 6 separa de vez).

use std::collections::HashMap;

use iced::widget::{canvas, image, qr_code};
use iced::window;

use super::analysis_view::CachedReply;
use super::app::Avatar;
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
    /// Paleta que se desenha: `Tokens` guarda `iced::Color`, entao mora do lado do toolkit.
    pub(super) tokens: &'static Tokens,
    /// `device_id` -> textura da foto (o `App` guarda os bytes; o `Handle` reaproveita a
    /// textura enquanto o mesmo valor viver, entao so se refaz quando a foto muda).
    pub(super) avatars: HashMap<String, image::Handle>,
    /// `user_id` -> textura da foto do perfil.
    pub(super) user_avatars: HashMap<String, image::Handle>,
    /// Respostas do assistente ja interpretadas (Markdown), pelo id da mensagem. O Markdown do
    /// iced desenha a partir de itens que precisam viver fora dele; o `view` so le, e quem monta
    /// e `App::refresh_replies`, a cada snapshot e a cada troca do olho.
    pub(super) replies: HashMap<String, CachedReply>,
    /// `WindowId` do modelo -> id do iced. O modelo so conhece o primeiro.
    windows: HashMap<WindowId, window::Id>,
}

impl IcedSide {
    pub fn new(tokens: &'static Tokens) -> IcedSide {
        IcedSide {
            qr: None,
            pace_cache: canvas::Cache::new(),
            history_cache: canvas::Cache::new(),
            empty_cache: canvas::Cache::new(),
            reserves_cache: canvas::Cache::new(),
            reserves_empty_cache: canvas::Cache::new(),
            iced_theme: tokens.iced_theme(),
            tokens,
            avatars: HashMap::new(),
            user_avatars: HashMap::new(),
            replies: HashMap::new(),
            windows: HashMap::new(),
        }
    }

    /// Refaz a matriz do QR a partir da URL do pareamento (`None` apaga).
    pub(super) fn sync_qr(&mut self, url: Option<&str>) {
        self.qr = url.and_then(|u| qr_code::Data::new(u).ok());
    }

    pub(super) fn sync_device_textures(&mut self, avatars: &HashMap<String, Avatar>) {
        sync_textures(&mut self.avatars, avatars);
    }

    pub(super) fn sync_user_textures(&mut self, avatars: &HashMap<String, Avatar>) {
        sync_textures(&mut self.user_avatars, avatars);
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

/// Reaproveita a textura enquanto os bytes forem os mesmos. O `Handle` NAO e identificado pelos
/// bytes: `Handle::from_bytes` da um id novo a cada chamada, e a GPU sobe a foto de novo para
/// cada id. Por isso o handle antigo e mantido e so refeito quando os bytes mudam. So a chave nova ou de foto
/// trocada ganha textura; chave que saiu do conjunto perde a dela.
fn sync_textures(handles: &mut HashMap<String, image::Handle>, avatars: &HashMap<String, Avatar>) {
    handles.retain(|key, _| avatars.contains_key(key));
    for (key, avatar) in avatars {
        let stale = handles.get(key).is_none_or(
            |h| !matches!(h, image::Handle::Bytes(_, b) if b.as_ref() == avatar.bytes.as_slice()),
        );
        if stale {
            handles.insert(key.clone(), image::Handle::from_bytes(avatar.bytes.clone()));
        }
    }
}
