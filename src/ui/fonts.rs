//! Fontes embutidas: Inter Hub (Inter 4.1 com `tnum` congelado) e Phosphor regular.
//!
//! Embutidas com `include_bytes!` porque o hub nunca baixa nada em runtime; a origem e a
//! licenca de cada arquivo estao em `assets/fonts/` e em `scripts/freeze-inter.sh`.

use iced::Font;
use iced::font::{Family, Weight};

pub const INTER_LIGHT_BYTES: &[u8] = include_bytes!("../../assets/fonts/InterHub-Light.ttf");
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
/// Inter 300 dos numeros grandes do Dashboard Grafite.
pub const INTER_LIGHT: Font = Font {
    family: Family::Name("Inter Hub"),
    weight: Weight::Light,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Familia como o `fontdb` do iced a resolve: nome tipografico (id 16) e, sem ele, o nome
    /// de familia (id 1). Na Light o id 1 e "Inter Hub Light"; so o 16 a junta a "Inter Hub".
    fn family_and_weight(bytes: &[u8]) -> (String, u16) {
        let face = ttf_parser::Face::parse(bytes, 0).expect("fonte valida");
        let find = |id: u16| {
            face.names()
                .into_iter()
                .filter(|n| n.name_id == id && n.is_unicode())
                .find_map(|n| n.to_string())
        };
        let family = find(ttf_parser::name_id::TYPOGRAPHIC_FAMILY)
            .or_else(|| find(ttf_parser::name_id::FAMILY))
            .expect("nome de familia");
        (family, face.weight().to_number())
    }

    #[test]
    fn inter_light_e_a_familia_inter_hub() {
        assert_eq!(
            family_and_weight(INTER_LIGHT_BYTES),
            ("Inter Hub".to_string(), 300)
        );
        assert_eq!(INTER_LIGHT.family, INTER.family);
        assert_eq!(INTER_LIGHT.weight, Weight::Light);
    }

    #[test]
    fn pesos_vendorizados_sao_todos_inter_hub() {
        for (bytes, weight) in [
            (INTER_REGULAR_BYTES, 400),
            (INTER_MEDIUM_BYTES, 500),
            (INTER_SEMIBOLD_BYTES, 600),
        ] {
            assert_eq!(family_and_weight(bytes), ("Inter Hub".to_string(), weight));
        }
    }
}
