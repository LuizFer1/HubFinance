//! Icones de traco do design (`assets/icons/*.svg`, gerados por `scripts/extract-icons.py`).
//!
//! Cada variante e um arquivo embutido em `assets`; a cor vem de `currentColor`, entao quem
//! desenha escolhe. Variantes `W*` sao a mesma forma com outra espessura de traco: o GPUI nao
//! muda a espessura na hora de desenhar. Categorias do app continuam no Phosphor
//! (`ui_model::icons`).

// Usado pelas telas a partir da Tarefa 11.
#![allow(dead_code)] // usado na Tarefa 11

use gpui::{Hsla, Pixels, Styled as _, Svg, svg};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    AlertCircle,
    ArrowDownLeft,
    ArrowDownLeftW18,
    ArrowUpRight,
    ArrowUpRightW22,
    ArrowsUpdown,
    Box,
    Briefcase,
    Bulb,
    Calendar,
    Car,
    Cart,
    ChartBars,
    Check,
    CheckW26,
    CheckW3,
    CheckW32,
    ChevronDown,
    ChevronDownW24,
    ChevronLeft,
    ChevronRight,
    Close,
    CloseW2,
    Copy,
    CopyW2,
    CreditCard,
    Dashboard,
    ExternalLink,
    Eye,
    EyeOff,
    Filter,
    Folder,
    Gift,
    Globe,
    HalfCircleWarning,
    Heart,
    Hub,
    Info,
    List,
    Lock,
    Moon,
    Palette,
    People,
    Phone,
    Pie,
    Plane,
    Plus,
    Power,
    Pulse,
    QrCode,
    Receipt,
    Repeat,
    RepeatW22,
    Scale,
    Settings,
    Shield,
    ShieldCheck,
    ShieldW2,
    Sparkle,
    SparkleW2,
    Sun,
    Sync,
    SyncW18,
    Timer,
    Trash,
    TrendUp,
    Warning,
    Wifi,
}

impl Icon {
    pub const ALL: &[Icon] = &[
        Icon::AlertCircle,
        Icon::ArrowDownLeft,
        Icon::ArrowDownLeftW18,
        Icon::ArrowUpRight,
        Icon::ArrowUpRightW22,
        Icon::ArrowsUpdown,
        Icon::Box,
        Icon::Briefcase,
        Icon::Bulb,
        Icon::Calendar,
        Icon::Car,
        Icon::Cart,
        Icon::ChartBars,
        Icon::Check,
        Icon::CheckW26,
        Icon::CheckW3,
        Icon::CheckW32,
        Icon::ChevronDown,
        Icon::ChevronDownW24,
        Icon::ChevronLeft,
        Icon::ChevronRight,
        Icon::Close,
        Icon::CloseW2,
        Icon::Copy,
        Icon::CopyW2,
        Icon::CreditCard,
        Icon::Dashboard,
        Icon::ExternalLink,
        Icon::Eye,
        Icon::EyeOff,
        Icon::Filter,
        Icon::Folder,
        Icon::Gift,
        Icon::Globe,
        Icon::HalfCircleWarning,
        Icon::Heart,
        Icon::Hub,
        Icon::Info,
        Icon::List,
        Icon::Lock,
        Icon::Moon,
        Icon::Palette,
        Icon::People,
        Icon::Phone,
        Icon::Pie,
        Icon::Plane,
        Icon::Plus,
        Icon::Power,
        Icon::Pulse,
        Icon::QrCode,
        Icon::Receipt,
        Icon::Repeat,
        Icon::RepeatW22,
        Icon::Scale,
        Icon::Settings,
        Icon::Shield,
        Icon::ShieldCheck,
        Icon::ShieldW2,
        Icon::Sparkle,
        Icon::SparkleW2,
        Icon::Sun,
        Icon::Sync,
        Icon::SyncW18,
        Icon::Timer,
        Icon::Trash,
        Icon::TrendUp,
        Icon::Warning,
        Icon::Wifi,
    ];

    pub fn path(self) -> &'static str {
        match self {
            Icon::AlertCircle => "icons/alert-circle.svg",
            Icon::ArrowDownLeft => "icons/arrow-down-left.svg",
            Icon::ArrowDownLeftW18 => "icons/arrow-down-left-w1_8.svg",
            Icon::ArrowUpRight => "icons/arrow-up-right.svg",
            Icon::ArrowUpRightW22 => "icons/arrow-up-right-w2_2.svg",
            Icon::ArrowsUpdown => "icons/arrows-updown.svg",
            Icon::Box => "icons/box.svg",
            Icon::Briefcase => "icons/briefcase.svg",
            Icon::Bulb => "icons/bulb.svg",
            Icon::Calendar => "icons/calendar.svg",
            Icon::Car => "icons/car.svg",
            Icon::Cart => "icons/cart.svg",
            Icon::ChartBars => "icons/chart-bars.svg",
            Icon::Check => "icons/check.svg",
            Icon::CheckW26 => "icons/check-w2_6.svg",
            Icon::CheckW3 => "icons/check-w3.svg",
            Icon::CheckW32 => "icons/check-w3_2.svg",
            Icon::ChevronDown => "icons/chevron-down.svg",
            Icon::ChevronDownW24 => "icons/chevron-down-w2_4.svg",
            Icon::ChevronLeft => "icons/chevron-left.svg",
            Icon::ChevronRight => "icons/chevron-right.svg",
            Icon::Close => "icons/close.svg",
            Icon::CloseW2 => "icons/close-w2.svg",
            Icon::Copy => "icons/copy.svg",
            Icon::CopyW2 => "icons/copy-w2.svg",
            Icon::CreditCard => "icons/credit-card.svg",
            Icon::Dashboard => "icons/dashboard.svg",
            Icon::ExternalLink => "icons/external-link.svg",
            Icon::Eye => "icons/eye.svg",
            Icon::EyeOff => "icons/eye-off.svg",
            Icon::Filter => "icons/filter.svg",
            Icon::Folder => "icons/folder.svg",
            Icon::Gift => "icons/gift.svg",
            Icon::Globe => "icons/globe.svg",
            Icon::HalfCircleWarning => "icons/half-circle-warning.svg",
            Icon::Heart => "icons/heart.svg",
            Icon::Hub => "icons/hub.svg",
            Icon::Info => "icons/info.svg",
            Icon::List => "icons/list.svg",
            Icon::Lock => "icons/lock.svg",
            Icon::Moon => "icons/moon.svg",
            Icon::Palette => "icons/palette.svg",
            Icon::People => "icons/people.svg",
            Icon::Phone => "icons/phone.svg",
            Icon::Pie => "icons/pie.svg",
            Icon::Plane => "icons/plane.svg",
            Icon::Plus => "icons/plus.svg",
            Icon::Power => "icons/power.svg",
            Icon::Pulse => "icons/pulse.svg",
            Icon::QrCode => "icons/qr-code.svg",
            Icon::Receipt => "icons/receipt.svg",
            Icon::Repeat => "icons/repeat.svg",
            Icon::RepeatW22 => "icons/repeat-w2_2.svg",
            Icon::Scale => "icons/scale.svg",
            Icon::Settings => "icons/settings.svg",
            Icon::Shield => "icons/shield.svg",
            Icon::ShieldCheck => "icons/shield-check.svg",
            Icon::ShieldW2 => "icons/shield-w2.svg",
            Icon::Sparkle => "icons/sparkle.svg",
            Icon::SparkleW2 => "icons/sparkle-w2.svg",
            Icon::Sun => "icons/sun.svg",
            Icon::Sync => "icons/sync.svg",
            Icon::SyncW18 => "icons/sync-w1_8.svg",
            Icon::Timer => "icons/timer.svg",
            Icon::Trash => "icons/trash.svg",
            Icon::TrendUp => "icons/trend-up.svg",
            Icon::Warning => "icons/warning.svg",
            Icon::Wifi => "icons/wifi.svg",
        }
    }
}

/// Icone `size` x `size` na cor dada.
pub fn icon(i: Icon, size: impl Into<Pixels>, color: Hsla) -> Svg {
    let size = size.into();
    svg()
        .path(i.path())
        .size(size)
        .text_color(color)
        .flex_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::assets::{FILES, HubAssets};
    use gpui::AssetSource as _;

    #[test]
    fn todo_icone_tem_arquivo_embutido() {
        for i in Icon::ALL {
            let bytes = HubAssets.load(i.path()).expect("load").expect("arquivo");
            assert!(bytes.starts_with(b"<svg"), "{:?}", i);
        }
    }

    #[test]
    fn nenhum_arquivo_de_icone_orfao() {
        let used: Vec<&str> = Icon::ALL.iter().map(|i| i.path()).collect();
        for (name, _) in FILES {
            assert!(used.contains(name), "{name} sem variante em Icon");
        }
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icons");
        let on_disk = std::fs::read_dir(dir).expect("dir").count();
        assert_eq!(on_disk, Icon::ALL.len(), "arquivo em assets/icons sem Icon");
    }
}
