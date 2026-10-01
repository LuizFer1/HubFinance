//! Fontes embutidas: Inter Hub (Inter 4.1 com `tnum` congelado) e Phosphor regular.
//!
//! Embutidas com `include_bytes!` porque o hub nunca baixa nada em runtime; a origem e a
//! licenca de cada arquivo estao em `assets/fonts/` e em `scripts/freeze-inter.sh`.

use iced::Font;
use iced::font::{Family, Weight};

pub const INTER_REGULAR_BYTES: &[u8] = include_bytes!("../../assets/fonts/InterHub-Regular.ttf");
pub const INTER_MEDIUM_BYTES: &[u8] = include_bytes!("../../assets/fonts/InterHub-Medium.ttf");
pub const INTER_SEMIBOLD_BYTES: &[u8] = include_bytes!("../../assets/fonts/InterHub-SemiBold.ttf");
pub const PHOSPHOR_BYTES: &[u8] = include_bytes!("../../assets/fonts/Phosphor.ttf");

/// "Inter Hub", nao "Inter": a familia foi renomeada ao congelar `tnum` para nunca disputar
/// com uma Inter instalada no sistema, que teria digitos proporcionais.
pub const INTER: Font = Font {
    family: Family::Name("Inter Hub"),
    weight: Weight::Normal,
    ..Font::DEFAULT
};
pub const INTER_MEDIUM: Font = Font {
    family: Family::Name("Inter Hub"),
    weight: Weight::Medium,
    ..Font::DEFAULT
};
pub const INTER_SEMIBOLD: Font = Font {
    family: Family::Name("Inter Hub"),
    weight: Weight::Semibold,
    ..Font::DEFAULT
};
pub const PHOSPHOR: Font = Font::with_name("Phosphor");
