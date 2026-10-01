//! Glifos do Phosphor regular (`@phosphor-icons/web@2.1.1`) e o mapa chave do app -> glifo.
//!
//! Cada icone e um `text` com o codepoint na fonte Phosphor embutida. Os codepoints foram
//! lidos de `src/regular/style.css` do mesmo pacote (`.ph.ph-house-line:before{content:"\e2c4"}`);
//! trocar a versao da fonte sem conferir a tabela desenharia glifos errados sem erro nenhum.

// Parte das constantes so ganha uso nas telas do plano 2b (Dashboard e Lancamentos).
#![allow(dead_code)]

use iced::widget::text;
use iced::{Color, Element};

use super::fonts;

// Interface.
pub const HOUSE_LINE: &str = "\u{E2C4}";
pub const CHART_PIE_SLICE: &str = "\u{E15A}";
pub const LIST_BULLETS: &str = "\u{E2F2}";
pub const QR_CODE: &str = "\u{E3E6}";
pub const MOON: &str = "\u{E330}";
pub const SUN: &str = "\u{E472}";
pub const MINUS: &str = "\u{E32A}";
pub const SQUARE: &str = "\u{E45E}";
pub const X: &str = "\u{E4F6}";
pub const CARET_LEFT: &str = "\u{E138}";
pub const CARET_RIGHT: &str = "\u{E13A}";
pub const CARET_DOWN: &str = "\u{E136}";
pub const CLOCK: &str = "\u{E19A}";
pub const TREND_UP: &str = "\u{E4AE}";
pub const TREND_DOWN: &str = "\u{E4AC}";
pub const ARROW_DOWN_LEFT: &str = "\u{E040}";
pub const ARROW_UP_RIGHT: &str = "\u{E092}";
pub const ARROW_RIGHT: &str = "\u{E06C}";
pub const CHECK: &str = "\u{E182}";
pub const FUNNEL_X: &str = "\u{E26C}";
pub const REPEAT: &str = "\u{E3F6}";
pub const LOCK_SIMPLE: &str = "\u{E308}";
pub const COPY: &str = "\u{E1CA}";
pub const HOUSE_SIMPLE: &str = "\u{E2C6}";
pub const DEVICE_MOBILE: &str = "\u{E1E0}";
pub const USER_MINUS: &str = "\u{E4CE}";
pub const CHECK_CIRCLE: &str = "\u{E184}";
pub const WARNING_CIRCLE: &str = "\u{E4E2}";
pub const KEY: &str = "\u{E2D6}";
pub const CERTIFICATE: &str = "\u{E766}";
pub const ARROWS_CLOCKWISE: &str = "\u{E094}";
pub const CIRCLE_DASHED: &str = "\u{E602}";

// Alvos das chaves do app (`icon-set.ts`).
pub const BABY: &str = "\u{E774}";
pub const MONEY: &str = "\u{E588}";
pub const BOOK_OPEN: &str = "\u{E0E6}";
pub const BRIEFCASE: &str = "\u{E0EE}";
pub const BUS: &str = "\u{E106}";
pub const CALENDAR_BLANK: &str = "\u{E10A}";
pub const CAR: &str = "\u{E112}";
pub const COFFEE: &str = "\u{E1C2}";
pub const CREDIT_CARD: &str = "\u{E1D2}";
pub const DOG: &str = "\u{E74A}";
pub const BARBELL: &str = "\u{E0B6}";
pub const FILM_STRIP: &str = "\u{E792}";
pub const GAS_PUMP: &str = "\u{E768}";
pub const GAME_CONTROLLER: &str = "\u{E26E}";
pub const GIFT: &str = "\u{E276}";
pub const GRADUATION_CAP: &str = "\u{E62C}";
pub const HEARTBEAT: &str = "\u{E2AC}";
pub const HOUSE: &str = "\u{E2C2}";
pub const BANK: &str = "\u{E0B4}";
pub const MUSIC_NOTES: &str = "\u{E340}";
pub const PAW_PRINT: &str = "\u{E648}";
pub const PIGGY_BANK: &str = "\u{EA04}";
pub const PILL: &str = "\u{E700}";
pub const AIRPLANE: &str = "\u{E002}";
pub const RECEIPT: &str = "\u{E3EC}";
pub const SCISSORS: &str = "\u{EAE0}";
pub const T_SHIRT: &str = "\u{E670}";
pub const SHOPPING_BAG: &str = "\u{E416}";
pub const SHOPPING_CART: &str = "\u{E41E}";
pub const TAG: &str = "\u{E478}";
pub const FORK_KNIFE: &str = "\u{E262}";
pub const WALLET: &str = "\u{E68A}";
pub const WIFI_HIGH: &str = "\u{E4EA}";
pub const LIGHTNING: &str = "\u{E2DE}";

/// Todas as constantes, para o teste garantir que nenhuma ficou vazia.
pub const ALL: [&str; 66] = [
    HOUSE_LINE,
    CHART_PIE_SLICE,
    LIST_BULLETS,
    QR_CODE,
    MOON,
    SUN,
    MINUS,
    SQUARE,
    X,
    CARET_LEFT,
    CARET_RIGHT,
    CARET_DOWN,
    CLOCK,
    TREND_UP,
    TREND_DOWN,
    ARROW_DOWN_LEFT,
    ARROW_UP_RIGHT,
    ARROW_RIGHT,
    CHECK,
    FUNNEL_X,
    REPEAT,
    LOCK_SIMPLE,
    COPY,
    HOUSE_SIMPLE,
    DEVICE_MOBILE,
    USER_MINUS,
    CHECK_CIRCLE,
    WARNING_CIRCLE,
    KEY,
    CERTIFICATE,
    ARROWS_CLOCKWISE,
    CIRCLE_DASHED,
    BABY,
    MONEY,
    BOOK_OPEN,
    BRIEFCASE,
    BUS,
    CALENDAR_BLANK,
    CAR,
    COFFEE,
    CREDIT_CARD,
    DOG,
    BARBELL,
    FILM_STRIP,
    GAS_PUMP,
    GAME_CONTROLLER,
    GIFT,
    GRADUATION_CAP,
    HEARTBEAT,
    HOUSE,
    BANK,
    MUSIC_NOTES,
    PAW_PRINT,
    PIGGY_BANK,
    PILL,
    AIRPLANE,
    RECEIPT,
    SCISSORS,
    T_SHIRT,
    SHOPPING_BAG,
    SHOPPING_CART,
    TAG,
    FORK_KNIFE,
    WALLET,
    WIFI_HIGH,
    LIGHTNING,
];

/// As 36 chaves de `HomeFinance_Mobile/src/features/icons/icon-set.ts`.
pub const APP_ICON_KEYS: [&str; 36] = [
    "baby",
    "banknote",
    "book",
    "briefcase",
    "bus",
    "calendar",
    "car",
    "chevron-down",
    "coffee",
    "credit-card",
    "dog",
    "dumbbell",
    "film",
    "fuel",
    "gamepad",
    "gift",
    "graduation",
    "health",
    "house",
    "landmark",
    "music",
    "paw-print",
    "piggy-bank",
    "pill",
    "plane",
    "receipt",
    "scissors",
    "shirt",
    "shopping-bag",
    "shopping-cart",
    "smartphone",
    "tag",
    "utensils",
    "wallet",
    "wifi",
    "zap",
];

/// Chave gravada pelo app (`icon-set.ts`) -> glifo. A chave e do app, nao do Phosphor: trocar
/// de biblioteca nao pode reescrever linha nenhuma. Desconhecida -> `circle-dashed`, como o
/// `FALLBACK_ICON` do app: uma versao mais nova pode gravar chave que esta nao conhece.
pub fn glyph(app_key: &str) -> &'static str {
    match app_key {
        "baby" => BABY,
        "banknote" => MONEY,
        "book" => BOOK_OPEN,
        "briefcase" => BRIEFCASE,
        "bus" => BUS,
        "calendar" => CALENDAR_BLANK,
        "car" => CAR,
        "chevron-down" => CARET_DOWN,
        "coffee" => COFFEE,
        "credit-card" => CREDIT_CARD,
        "dog" => DOG,
        "dumbbell" => BARBELL,
        "film" => FILM_STRIP,
        "fuel" => GAS_PUMP,
        "gamepad" => GAME_CONTROLLER,
        "gift" => GIFT,
        "graduation" => GRADUATION_CAP,
        "health" => HEARTBEAT,
        "house" => HOUSE,
        "landmark" => BANK,
        "music" => MUSIC_NOTES,
        "paw-print" => PAW_PRINT,
        "piggy-bank" => PIGGY_BANK,
        "pill" => PILL,
        "plane" => AIRPLANE,
        "receipt" => RECEIPT,
        "scissors" => SCISSORS,
        "shirt" => T_SHIRT,
        "shopping-bag" => SHOPPING_BAG,
        "shopping-cart" => SHOPPING_CART,
        "smartphone" => DEVICE_MOBILE,
        "tag" => TAG,
        "utensils" => FORK_KNIFE,
        "wallet" => WALLET,
        "wifi" => WIFI_HIGH,
        "zap" => LIGHTNING,
        _ => CIRCLE_DASHED,
    }
}

/// `text(glyph)` na fonte Phosphor. Tamanho em px e cor explicitos: icone nunca herda.
/// Altura de linha 1.0 para a caixa do icone ter exatamente `size`, como o `font-size` do
/// protótipo; com a altura padrao (1.3) o icone desceria em relacao ao texto ao lado.
pub fn icon<'a, M: 'a>(glyph: &'static str, size: f32, color: Color) -> Element<'a, M> {
    text(glyph)
        .font(fonts::PHOSPHOR)
        .size(size)
        .line_height(1.0)
        .color(color)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chaves_conhecidas_viram_o_glifo_do_app() {
        assert_eq!(glyph("utensils"), FORK_KNIFE);
        assert_eq!(glyph("zap"), LIGHTNING);
        assert_eq!(glyph("paw-print"), PAW_PRINT);
    }

    #[test]
    fn chave_desconhecida_ou_vazia_vira_circle_dashed() {
        assert_eq!(glyph("nao-existe"), CIRCLE_DASHED);
        assert_eq!(glyph(""), CIRCLE_DASHED);
    }

    #[test]
    fn toda_chave_do_app_resolve() {
        for key in APP_ICON_KEYS {
            assert_ne!(glyph(key), CIRCLE_DASHED, "{key}");
        }
    }

    #[test]
    fn nenhuma_constante_vazia_e_todas_na_area_de_uso_privado() {
        for g in ALL {
            let mut chars = g.chars();
            let c = chars.next().expect("glifo vazio");
            assert!(chars.next().is_none(), "{g:?} tem mais de um caractere");
            assert!(('\u{E000}'..='\u{F8FF}').contains(&c), "{c:?}");
        }
    }
}
