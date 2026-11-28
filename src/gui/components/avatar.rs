//! Avatar: foto (bytes decodificados uma vez por perfil) ou inicial sobre a cor do perfil.
//!
//! A foto vira `Arc<gpui::Image>` **uma vez**, fora do `render` (`photo`): o GPUI guarda a
//! textura pelo id do `Image`, entao um `Arc` novo a cada quadro decodificaria a foto de novo
//! 60 vezes por segundo.

use std::sync::Arc;

use gpui::{
    App, Hsla, Image, ImageFormat, IntoElement, ObjectFit, ParentElement, RenderOnce, SharedString,
    Styled, StyledImage, Window, div, img, px,
};

use crate::dashboard::colors::{Mode, token_color};
use crate::gui::fonts;
use crate::gui::theme::{Tokens, rgb};

/// Formato pela assinatura dos bytes (os mesmos que `dashboard::dataset::avatar_bytes` aceita).
pub fn format_of(bytes: &[u8]) -> Option<ImageFormat> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some(ImageFormat::Png)
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(ImageFormat::Jpeg)
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some(ImageFormat::Webp)
    } else {
        None
    }
}

/// Foto pronta para o `img()`. `None` = formato desconhecido (cai na inicial).
pub fn photo(bytes: &[u8]) -> Option<Arc<Image>> {
    format_of(bytes).map(|f| Arc::new(Image::from_bytes(f, bytes.to_vec())))
}

/// Primeira letra maiuscula do nome, ou "?" se vazio. Pela primeira *letra*, nao pelo primeiro
/// byte: "Ágata" nao pode virar meio caractere.
pub fn initial(name: &str) -> String {
    name.trim()
        .chars()
        .find(|c| c.is_alphanumeric())
        .map_or_else(|| "?".to_string(), |c| c.to_uppercase().collect())
}

/// Cor do perfil: token gravado na linha, no tema atual.
pub fn profile_color(token: &str, tokens: &Tokens) -> Hsla {
    let mode = if tokens.dark { Mode::Dark } else { Mode::Light };
    let c = token_color(token, mode);
    rgb((u32::from(c.0) << 16) | (u32::from(c.1) << 8) | u32::from(c.2))
}

#[derive(IntoElement)]
pub struct Avatar {
    name: SharedString,
    color_token: SharedString,
    photo: Option<Arc<Image>>,
    size: f32,
    tokens: &'static Tokens,
}

impl Avatar {
    pub fn new(
        name: impl Into<SharedString>,
        color_token: impl Into<SharedString>,
        tokens: &'static Tokens,
    ) -> Avatar {
        Avatar {
            name: name.into(),
            color_token: color_token.into(),
            photo: None,
            size: 36.0,
            tokens,
        }
    }

    pub fn photo(mut self, photo: Option<Arc<Image>>) -> Avatar {
        self.photo = photo;
        self
    }

    pub fn size(mut self, size: f32) -> Avatar {
        self.size = size;
        self
    }
}

impl RenderOnce for Avatar {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let t = self.tokens;
        let base = div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(self.size))
            .rounded_full()
            .overflow_hidden();
        match self.photo {
            Some(photo) => base.child(img(photo).size_full().object_fit(ObjectFit::Cover)),
            None => base
                .bg(profile_color(&self.color_token, t))
                // Texto escuro sobre a cor do perfil: as cores sao claras o bastante nos dois temas.
                .text_color(t.on_highlight)
                .text_size(px(self.size * 0.42))
                .font_weight(fonts::weight(600))
                .child(SharedString::from(initial(&self.name))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formato_pela_assinatura() {
        assert_eq!(
            format_of(&[0x89, b'P', b'N', b'G', 0]),
            Some(ImageFormat::Png)
        );
        assert_eq!(format_of(&[0xFF, 0xD8, 0xFF, 0]), Some(ImageFormat::Jpeg));
        assert_eq!(format_of(b"RIFF\0\0\0\0WEBPxx"), Some(ImageFormat::Webp));
        assert_eq!(format_of(b"GIF89a"), None);
        assert!(photo(b"nada").is_none());
    }

    #[test]
    fn inicial_e_a_primeira_letra_maiuscula() {
        assert_eq!(initial("luiz"), "L");
        assert_eq!(initial("  ágata"), "Á");
        assert_eq!(initial(""), "?");
        assert_eq!(initial("— "), "?");
    }
}
