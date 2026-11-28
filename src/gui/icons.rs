//! Icones de traco do design (`assets/icons/*.svg`, gerados por `scripts/extract-icons.py`).
//!
//! Cada variante e um arquivo embutido em `assets`; a cor vem de `currentColor`, entao quem
//! desenha escolhe. O nome puro (`Close`) e a espessura mais usada do design; `W*` e a mesma
//! forma com outra espessura (`CloseW4` = 4 px): o GPUI nao muda a espessura ao desenhar. O doc
//! de cada variante traz a espessura e onde o design a usa. `Check` e 4 px (selos do assistente);
//! para o check fino de lista use `CheckW26`. Categorias do app continuam no Phosphor
//! (`ui_model::icons`).

// Usado pelas telas a partir da Tarefa 15; ate la so parte das variantes tem uso.
#![allow(dead_code)] // usado na Tarefa 15

use gpui::{Hsla, Pixels, Styled as _, Svg, svg};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    /// Traco 2 px; 2 uso(s) no design, p.ex. Sistema, SistemaClaro.
    AlertCircle,
    /// Traco 2 px; 7 uso(s) no design, p.ex. Reservas, Componentes, Lancamentos.
    ArrowDownLeft,
    /// Traco 1.8 px; 3 uso(s) no design, p.ex. DashboardClaro, DashboardJanela1100, Main.
    ArrowDownLeftW18,
    /// Traco 2 px; 4 uso(s) no design, p.ex. Reservas, Componentes, Lancamentos.
    ArrowUpRight,
    /// Traco 2.2 px; 3 uso(s) no design, p.ex. DashboardClaro, DashboardJanela1100, Main.
    ArrowUpRightW22,
    /// Traco 1.8 px; 2 uso(s) no design, p.ex. Estados, Reservas.
    ArrowsUpdown,
    /// Traco 1.8 px; 2 uso(s) no design, p.ex. Estados, Reservas.
    Box,
    /// Traco 2.4 px; 3 uso(s) no design, p.ex. DashboardClaro, DashboardJanela1100, Main.
    Briefcase,
    /// Traco 1.8 px; 13 uso(s) no design, p.ex. Analise, AnaliseConversa, AnaliseConversaClaro.
    Bulb,
    /// Traco 1.8 px; 5 uso(s) no design, p.ex. Componentes, DashboardClaro, DashboardJanela1100.
    Calendar,
    /// Traco 1.8 px; 4 uso(s) no design, p.ex. DashboardClaro, DashboardJanela1100, Main.
    Car,
    /// Traco 1.8 px; 3 uso(s) no design, p.ex. DashboardClaro, DashboardJanela1100, Main.
    Cart,
    /// Traco 1.8 px; 3 uso(s) no design, p.ex. DashboardClaro, DashboardJanela1100, Main.
    ChartBars,
    /// Traco 4 px; 43 uso(s) no design, p.ex. AssistenteVe, Analise, AnaliseConversa.
    Check,
    /// Traco 2.6 px; 5 uso(s) no design, p.ex. Sistema, SistemaClaro, Componentes.
    CheckW26,
    /// Traco 3 px; 5 uso(s) no design, p.ex. Componentes, DashboardClaro, DashboardJanela1100.
    CheckW3,
    /// Traco 3.2 px; 6 uso(s) no design, p.ex. RemoverPasso3, Estados, RemoverPasso2.
    CheckW32,
    /// Traco 2 px; 4 uso(s) no design, p.ex. AssistenteConfigurar, Componentes, Estados.
    ChevronDown,
    /// Traco 2.4 px; 2 uso(s) no design, p.ex. Componentes, Conexao.
    ChevronDownW24,
    /// Traco 2 px; 16 uso(s) no design, p.ex. AnaliseConversa, AnaliseConversaClaro, Estados.
    ChevronLeft,
    /// Traco 2 px; 73 uso(s) no design, p.ex. AnaliseConversa, AnaliseConversaClaro, Analise.
    ChevronRight,
    /// Traco 2 px; 8 uso(s) no design, p.ex. AssistenteConfigurar, AssistenteVe, Estados.
    Close,
    /// Traco 4 px; 5 uso(s) no design, p.ex. AssistenteVe, Ajustes.
    CloseW4,
    /// Traco 1.8 px; 5 uso(s) no design, p.ex. Ajustes, AnaliseConversa, AnaliseConversaClaro.
    Copy,
    /// Traco 2 px; 2 uso(s) no design, p.ex. Conexao.
    CopyW2,
    /// Traco 1.8 px; 1 uso(s) no design, p.ex. Reservas.
    CreditCard,
    /// Traco 1.8 px; 11 uso(s) no design, p.ex. Ajustes, Analise, AnaliseConversa.
    Dashboard,
    /// Traco 2 px; 1 uso(s) no design, p.ex. Ajustes.
    ExternalLink,
    /// Traco 1.8 px; 11 uso(s) no design, p.ex. Analise, Ajustes, AnaliseConversa.
    Eye,
    /// Traco 1.8 px; 11 uso(s) no design, p.ex. Ajustes, Analise, AnaliseConversa.
    EyeOff,
    /// Traco 1.8 px; 1 uso(s) no design, p.ex. Estados.
    Filter,
    /// Traco 1.8 px; 4 uso(s) no design, p.ex. Ajustes, Sistema, SistemaClaro.
    Folder,
    /// Traco 1.8 px; 1 uso(s) no design, p.ex. Reservas.
    Gift,
    /// Traco 1.8 px; 1 uso(s) no design, p.ex. Conexao.
    Globe,
    /// Traco 2 px; 9 uso(s) no design, p.ex. Analise, AnaliseConversa, AnaliseConversaClaro.
    HalfCircleWarning,
    /// Traco 1.8 px; 3 uso(s) no design, p.ex. DashboardClaro, DashboardJanela1100, Main.
    Heart,
    /// Traco 2 px; 41 uso(s) no design, p.ex. Sistema, SistemaClaro, Componentes.
    Hub,
    /// Traco 1.8 px; 6 uso(s) no design, p.ex. Ajustes, Componentes, Conexao.
    Info,
    /// Traco 1.8 px; 16 uso(s) no design, p.ex. Componentes, Ajustes, Analise.
    List,
    /// Traco 1.8 px; 4 uso(s) no design, p.ex. Ajustes, Analise, AnaliseConversa.
    Lock,
    /// Traco 1.8 px; 11 uso(s) no design, p.ex. Ajustes, Analise, AnaliseConversa.
    Moon,
    /// Traco 1.8 px; 1 uso(s) no design, p.ex. Ajustes.
    Palette,
    /// Traco 1.8 px; 5 uso(s) no design, p.ex. Conexao, DashboardClaro, DashboardJanela1100.
    People,
    /// Traco 1.8 px; 1 uso(s) no design, p.ex. Conexao.
    Phone,
    /// Traco 1.8 px; 3 uso(s) no design, p.ex. DashboardClaro, DashboardJanela1100, Main.
    Pie,
    /// Traco 1.8 px; 1 uso(s) no design, p.ex. Reservas.
    Plane,
    /// Traco 2 px; 2 uso(s) no design, p.ex. AnaliseConversa, AnaliseConversaClaro.
    Plus,
    /// Traco 1.8 px; 1 uso(s) no design, p.ex. Ajustes.
    Power,
    /// Traco 1.8 px; 4 uso(s) no design, p.ex. Conexao, DashboardClaro, DashboardJanela1100.
    Pulse,
    /// Traco 1.5 px; 1 uso(s) no design, p.ex. Estados.
    QrCode,
    /// Traco 1.8 px; 3 uso(s) no design, p.ex. DashboardClaro, DashboardJanela1100, Main.
    Receipt,
    /// Traco 2.2 px; 17 uso(s) no design, p.ex. DashboardClaro, DashboardJanela1100, Main.
    Repeat,
    /// Traco 2 px; 1 uso(s) no design, p.ex. Componentes.
    RepeatW2,
    /// Traco 2 px; 1 uso(s) no design, p.ex. Lancamentos.
    Scale,
    /// Traco 1.8 px; 13 uso(s) no design, p.ex. Analise, AnaliseConversa, AnaliseConversaClaro.
    Settings,
    /// Traco 1.8 px; 16 uso(s) no design, p.ex. DashboardClaro, DashboardJanela1100, Main.
    Shield,
    /// Traco 1.8 px; 1 uso(s) no design, p.ex. Conexao.
    ShieldCheck,
    /// Traco 2 px; 2 uso(s) no design, p.ex. Estados, Reservas.
    ShieldW2,
    /// Traco 2 px; 10 uso(s) no design, p.ex. AnaliseConversa, AnaliseConversaClaro, Analise.
    Sparkle,
    /// Traco 1.8 px; 3 uso(s) no design, p.ex. Ajustes, Analise, Estados.
    SparkleW18,
    /// Traco 1.8 px; 11 uso(s) no design, p.ex. Ajustes, Analise, AnaliseConversa.
    Sun,
    /// Traco 2 px; 10 uso(s) no design, p.ex. Ajustes, Analise, AnaliseConversa.
    Sync,
    /// Traco 1.8 px; 2 uso(s) no design, p.ex. Componentes, Estados.
    SyncW18,
    /// Traco 2 px; 1 uso(s) no design, p.ex. Conexao.
    Timer,
    /// Traco 1.8 px; 2 uso(s) no design, p.ex. Analise.
    Trash,
    /// Traco 1.8 px; 2 uso(s) no design, p.ex. Estados, Reservas.
    TrendUp,
    /// Traco 2 px; 1 uso(s) no design, p.ex. RemoverPasso3.
    Warning,
    /// Traco 1.8 px; 11 uso(s) no design, p.ex. Ajustes, Analise, AnaliseConversa.
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
        Icon::CloseW4,
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
        Icon::RepeatW2,
        Icon::Scale,
        Icon::Settings,
        Icon::Shield,
        Icon::ShieldCheck,
        Icon::ShieldW2,
        Icon::Sparkle,
        Icon::SparkleW18,
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
            Icon::CloseW4 => "icons/close-w4.svg",
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
            Icon::RepeatW2 => "icons/repeat-w2.svg",
            Icon::Scale => "icons/scale.svg",
            Icon::Settings => "icons/settings.svg",
            Icon::Shield => "icons/shield.svg",
            Icon::ShieldCheck => "icons/shield-check.svg",
            Icon::ShieldW2 => "icons/shield-w2.svg",
            Icon::Sparkle => "icons/sparkle.svg",
            Icon::SparkleW18 => "icons/sparkle-w1_8.svg",
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
