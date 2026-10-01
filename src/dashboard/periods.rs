//! Meses e datas em pt-BR, tudo sobre strings `YYYY-MM` / `YYYY-MM-DD` como o `periods.ts`
//! do app. Nenhuma funcao le relogio: "hoje" e "agora" chegam por parametro.
//!
//! Entrada fora do formato nunca entra em panico: volta crua (datas) ou vazia (nomes de mes).
//! O contrato de campos ja recusa `occurredOn` mal formado, mas dia impossivel (`2026-02-31`)
//! passa de proposito (o hub nao julga regra de negocio) e precisa aparecer como veio.

use time::format_description::well_known::Rfc3339;
use time::{Date, Month, OffsetDateTime, UtcOffset};

const LONG: [&str; 12] = [
    "janeiro",
    "fevereiro",
    "março",
    "abril",
    "maio",
    "junho",
    "julho",
    "agosto",
    "setembro",
    "outubro",
    "novembro",
    "dezembro",
];

const SHORT: [&str; 12] = [
    "jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez",
];

/// Inicial maiuscula, como o prototipo (`WD`): o dia da semana abre o rotulo.
const WEEKDAY_SHORT: [&str; 7] = ["Dom", "Seg", "Ter", "Qua", "Qui", "Sex", "Sáb"];

/// `"2026-09"` -> `(2026, 9)`; mes fora de 1..=12 ou formato errado -> `None`.
fn parse_month(month: &str) -> Option<(i32, u8)> {
    let (y, m) = month.split_once('-')?;
    if y.len() != 4 || m.len() != 2 {
        return None;
    }
    let y: i32 = y.parse().ok()?;
    let m: u8 = m.parse().ok()?;
    (1..=12).contains(&m).then_some((y, m))
}

/// Data valida de calendario; `2026-02-31` -> `None`.
fn parse_date(date: &str) -> Option<Date> {
    if date.len() != 10 || date.as_bytes()[7] != b'-' {
        return None;
    }
    let (y, m) = parse_month(&date[..7])?;
    let d: u8 = date[8..].parse().ok()?;
    Date::from_calendar_date(y, Month::try_from(m).ok()?, d).ok()
}

fn month_index(month: &str) -> Option<usize> {
    parse_month(month).map(|(_, m)| usize::from(m) - 1)
}

pub fn month_of(date: &str) -> String {
    date.get(..7).unwrap_or(date).to_string()
}

/// Soma `delta` meses. Mes invalido volta como veio: quem chama so passa meses que ele
/// mesmo gerou, e um rotulo errado e melhor que um panico na janela.
pub fn shift_month(month: &str, delta: i32) -> String {
    let Some((y, m)) = parse_month(month) else {
        return month.to_string();
    };
    let total = y * 12 + i32::from(m) - 1 + delta;
    format!(
        "{:04}-{:02}",
        total.div_euclid(12),
        total.rem_euclid(12) + 1
    )
}

/// Os `n` meses terminando em `month`, o mais antigo primeiro (eixo das barras).
pub fn last_months(month: &str, n: usize) -> Vec<String> {
    let n = i32::try_from(n).unwrap_or(i32::MAX);
    (0..n).rev().map(|back| shift_month(month, -back)).collect()
}

/// Os 12 meses navegaveis: terminam no mes de `today`.
pub fn month_window(today: &str) -> Vec<String> {
    last_months(&month_of(today), 12)
}

/// "setembro"; mes invalido -> "".
pub fn month_long(month: &str) -> &'static str {
    month_index(month).map_or("", |i| LONG[i])
}

/// "set"; mes invalido -> "".
pub fn month_short(month: &str) -> &'static str {
    month_index(month).map_or("", |i| SHORT[i])
}

/// "setembro 2026" (minusculas, como o pt-BR escreve); invalido volta cru.
pub fn month_label(month: &str) -> String {
    match parse_month(month) {
        Some((y, m)) => format!("{} {y}", LONG[usize::from(m) - 1]),
        None => month.to_string(),
    }
}

/// "Setembro 2026": o seletor de mes (`text-transform: capitalize` do prototipo).
pub fn month_label_capitalized(month: &str) -> String {
    let label = month_label(month);
    let mut chars = label.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => label,
    }
}

fn weekday(date: Date) -> &'static str {
    WEEKDAY_SHORT[usize::from(date.weekday().number_days_from_sunday())]
}

/// "Qui, 24 de setembro": cabecalho do grupo do dia.
pub fn day_heading(date: &str) -> String {
    match parse_date(date) {
        Some(d) => format!(
            "{}, {} de {}",
            weekday(d),
            d.day(),
            LONG[usize::from(u8::from(d.month())) - 1]
        ),
        None => date.to_string(),
    }
}

/// "Qui, 24 set": linha compacta (dia sem zero a esquerda).
pub fn short_date(date: &str) -> String {
    match parse_date(date) {
        Some(d) => format!(
            "{}, {} {}",
            weekday(d),
            d.day(),
            SHORT[usize::from(u8::from(d.month())) - 1]
        ),
        None => date.to_string(),
    }
}

/// "24 set": a tag "Parcial · até 24 set".
pub fn partial_label(date: &str) -> String {
    match parse_date(date) {
        Some(d) => format!(
            "{} {}",
            d.day(),
            SHORT[usize::from(u8::from(d.month())) - 1]
        ),
        None => date.to_string(),
    }
}

/// "12 mar 2026" no fuso de quem olha (um pareamento as 22 h de Brasilia e do dia em que a
/// pessoa pareou, nao do dia seguinte em UTC). Data ilegivel volta crua.
pub fn paired_label(rfc3339: &str, offset: UtcOffset) -> String {
    match OffsetDateTime::parse(rfc3339, &Rfc3339) {
        Ok(t) => {
            let t = t.to_offset(offset);
            let month = SHORT[usize::from(u8::from(t.month())) - 1];
            format!("{} {month} {}", t.day(), t.year())
        }
        Err(_) => rfc3339.to_string(),
    }
}

/// "agora há pouco", "há 2 min", "há 1 h", "há 1 dia", "há 3 dias". Recebe segundos (quem
/// chama faz `agora - t`); negativo, de relogio adiantado, vira "agora há pouco".
pub fn relative_secs(secs: i64) -> String {
    match secs.max(0) {
        0..60 => "agora há pouco".to_string(),
        s @ 60..3600 => format!("há {} min", s / 60),
        s @ 3600..86_400 => format!("há {} h", s / 3600),
        86_400..172_800 => "há 1 dia".to_string(),
        s => format!("há {} dias", s / 86_400),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mes_da_data() {
        assert_eq!(month_of("2026-09-14"), "2026-09");
    }

    #[test]
    fn deslocar_mes_na_virada_de_ano() {
        assert_eq!(shift_month("2026-01", -1), "2025-12");
        assert_eq!(shift_month("2025-12", 1), "2026-01");
        assert_eq!(shift_month("2026-03", -14), "2025-01");
        assert_eq!(shift_month("2026-03", 0), "2026-03");
        assert_eq!(shift_month("2026-03", 24), "2028-03");
        // Mes invalido volta como veio, sem panico.
        assert_eq!(shift_month("2026-13", 1), "2026-13");
    }

    #[test]
    fn ultimos_meses_atravessam_o_ano() {
        assert_eq!(
            last_months("2026-01", 6),
            vec![
                "2025-08", "2025-09", "2025-10", "2025-11", "2025-12", "2026-01"
            ]
        );
        assert!(last_months("2026-01", 0).is_empty());
    }

    #[test]
    fn janela_de_doze_meses() {
        let window = month_window("2026-09-24");
        assert_eq!(window.len(), 12);
        assert_eq!(window[0], "2025-10");
        assert_eq!(window[11], "2026-09");
        let unique: std::collections::BTreeSet<&String> = window.iter().collect();
        assert_eq!(unique.len(), 12);
    }

    #[test]
    fn rotulos_de_mes() {
        assert_eq!(month_label("2026-09"), "setembro 2026");
        assert_eq!(month_label_capitalized("2026-09"), "Setembro 2026");
        assert_eq!(month_label_capitalized("2026-03"), "Março 2026");
        assert_eq!(month_long("2026-09"), "setembro");
        assert_eq!(month_short("2026-09"), "set");
        assert_eq!(month_short("2026-13"), "");
        assert_eq!(month_long("2026-00"), "");
        assert_eq!(month_long("lixo"), "");
        assert_eq!(month_label("lixo"), "lixo");
    }

    #[test]
    fn cabecalho_do_dia_e_data_curta() {
        // 2026-09-24 e quinta.
        assert_eq!(day_heading("2026-09-24"), "Qui, 24 de setembro");
        assert_eq!(day_heading("2026-02-31"), "2026-02-31");
        assert_eq!(short_date("2026-09-24"), "Qui, 24 set");
        assert_eq!(short_date("2026-01-04"), "Dom, 4 jan");
        assert_eq!(short_date("2026-08-01"), "Sáb, 1 ago");
        assert_eq!(short_date("2026-02-31"), "2026-02-31");
        assert_eq!(partial_label("2026-09-24"), "24 set");
        assert_eq!(partial_label("2026-09-04"), "4 set");
    }

    #[test]
    fn rotulo_de_pareamento() {
        assert_eq!(
            paired_label("2026-03-12T18:00:00Z", UtcOffset::UTC),
            "12 mar 2026"
        );
        // No fuso de Brasilia, 01:00 UTC ainda e o dia anterior.
        let brt = UtcOffset::from_hms(-3, 0, 0).unwrap();
        assert_eq!(paired_label("2026-03-12T01:00:00Z", brt), "11 mar 2026");
        assert_eq!(paired_label("ontem", UtcOffset::UTC), "ontem");
    }

    #[test]
    fn relativo_em_faixas() {
        assert_eq!(relative_secs(30), "agora há pouco");
        assert_eq!(relative_secs(90), "há 1 min");
        assert_eq!(relative_secs(7200), "há 2 h");
        assert_eq!(relative_secs(86_400), "há 1 dia");
        assert_eq!(relative_secs(3 * 86_400), "há 3 dias");
        // Relogio adiantado nao vira "ha -2 min".
        assert_eq!(relative_secs(-120), "agora há pouco");
    }
}
