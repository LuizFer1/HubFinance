//! O que o `State` guarda e so existe por causa do iced: matriz do QR, geometria em cache dos
//! graficos, o tema do iced, as texturas das fotos e o mapa de ids de janela. Fica junto num
//! campo so, ao lado do modelo (`State`), para o estado da janela ficar sem tipo de toolkit.

use std::collections::HashMap;

use iced::widget::{canvas, image, qr_code};
use iced::window;

use super::analysis_view::CachedReply;
use super::theme::Tokens;
use crate::ui_model::app::{App, Avatar};
use crate::ui_model::window::WindowId;

pub struct IcedSide {
    /// Reconstruido so quando o token muda: gerar a matriz a cada quadro e desperdicio.
    pub(super) qr: Option<qr_code::Data>,
    /// A URL de onde `qr` saiu: a matriz so e refeita quando ela muda.
    qr_url: Option<String>,
    /// Ultimo `charts_epoch` do modelo visto: quando ele sobe, a geometria em cache envelheceu.
    pub(super) charts_seen: u32,
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
    /// `device_id` -> textura da foto (o `State` guarda os bytes; o `Handle` reaproveita a
    /// textura enquanto o mesmo valor viver, entao so se refaz quando a foto muda).
    pub(super) avatars: HashMap<String, image::Handle>,
    /// `user_id` -> textura da foto do perfil.
    pub(super) user_avatars: HashMap<String, image::Handle>,
    /// Respostas do assistente ja interpretadas (Markdown), pelo id da mensagem. O Markdown do
    /// iced desenha a partir de itens que precisam viver fora dele; o `view` so le, e quem monta
    /// e `IcedSide::sync_replies`, chamado por `State::sync`, a cada snapshot e a cada troca do
    /// olho.
    pub(super) replies: HashMap<String, CachedReply>,
    /// `WindowId` do modelo -> id do iced. O modelo so conhece o primeiro.
    windows: HashMap<WindowId, window::Id>,
}

impl IcedSide {
    pub fn new(tokens: &'static Tokens) -> IcedSide {
        IcedSide {
            qr: None,
            qr_url: None,
            charts_seen: 0,
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

    /// Refaz a matriz do QR a partir da URL do pareamento (`None` apaga), so se a URL mudou:
    /// gerar a matriz a cada mensagem seria desperdicio.
    pub(super) fn sync_qr(&mut self, url: Option<&str>) {
        if self.qr_url.as_deref() == url {
            return;
        }
        self.qr_url = url.map(str::to_string);
        self.qr = url.and_then(|u| qr_code::Data::new(u).ok());
    }

    /// Interpreta as respostas da conversa aberta que ainda nao estao no cache (ou que mudaram
    /// de texto, ou de olho) e esquece as de conversas que sairam da tela. So o que muda e
    /// refeito: o parse do Markdown nao roda a cada quadro.
    pub(super) fn sync_replies(&mut self, model: &App) {
        use crate::assistant::view::MessageRole;
        let hidden = model.hide_values;
        let Some(open) = model.snapshot.assistant.open.as_ref() else {
            self.replies.clear();
            return;
        };
        self.replies.retain(|id, _| {
            open.messages
                .iter()
                .any(|m| m.role == MessageRole::Assistant && &m.id == id)
        });
        for m in &open.messages {
            if m.role != MessageRole::Assistant {
                continue;
            }
            let fresh = self
                .replies
                .get(&m.id)
                .is_some_and(|c| c.hidden == hidden && c.content == m.content);
            if !fresh {
                self.replies.insert(
                    m.id.clone(),
                    super::analysis_view::render_reply(&m.content, hidden),
                );
            }
        }
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

    /// Esquece os ids de janelas que o modelo ja deu por fechadas.
    pub(super) fn forget_except(&mut self, current: Option<WindowId>) {
        self.windows.retain(|id, _| Some(*id) == current);
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
/// cada id. Por isso o handle antigo e mantido e so refeito quando os bytes mudam. So a chave
/// nova ou de foto trocada ganha textura; chave que saiu do conjunto perde a dela.
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
