//! Meses e datas em pt-BR, tudo sobre strings `YYYY-MM` / `YYYY-MM-DD` como o `periods.ts`
//! do app. Nenhuma funcao le relogio: "hoje" e "agora" chegam por parametro.
//!
//! Entrada fora do formato nunca entra em panico: volta crua (datas) ou vazia (nomes de mes).
//! O contrato de campos ja recusa `occurredOn` mal formado, mas dia impossivel (`2026-02-31`)
//! passa de proposito (o hub nao julga regra de negocio) e precisa aparecer como veio.

use time::format_description::well_known::Rfc3339;
use time::{Date, Month, OffsetDateTime, UtcOffset, Weekday};

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

/// Meses de `from` (exclusivo) ate `to` (inclusivo): "do proximo mes ate o prazo". Negativo
/// quando `to` ja passou. Invalido -> 0: quem chama trata 0 como "sem prazo util".
pub fn months_between(from: &str, to: &str) -> i32 {
    match (parse_month(from), parse_month(to)) {
        (Some((fy, fm)), Some((ty, tm))) => (ty - fy) * 12 + i32::from(tm) - i32::from(fm),
        _ => 0,
    }
}

/// "jul 2027": prazo da caixinha e mes da projecao; invalido volta cru.
pub fn month_short_year(month: &str) -> String {
    match parse_month(month) {
        Some((y, m)) => format!("{} {y}", SHORT[usize::from(m) - 1]),
        None => month.to_string(),
    }
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

/// Dia juliano de uma data na forma `YYYY-MM-DD`, com o dia **normalizado como o
/// `Date.UTC` do app**: `2026-02-31` e 3 de marco, `2026-03-00` e 28 de fevereiro. O app faz
/// toda a aritmetica de dias com `Date.UTC`, que nunca recusa; se o hub recusasse, uma data
/// assim daria outro dia (ou laco parado) na projecao. Fora da forma -> `None`.
fn day_number(date: &str) -> Option<i32> {
    if !super::contract::is_iso_date(date) {
        return None;
    }
    let (y, m) = parse_month(&date[..7])?;
    let d: i32 = date[8..].parse().ok()?;
    let first = Date::from_calendar_date(y, Month::try_from(m).ok()?, 1).ok()?;
    Some(first.to_julian_day() + d - 1)
}

fn date_of(day: i32) -> Option<Date> {
    Date::from_julian_day(day).ok()
}

fn iso(date: Date) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        date.year(),
        u8::from(date.month()),
        date.day()
    )
}

/// `shiftDay` do app: soma `delta` dias. Fora da forma `YYYY-MM-DD` volta crua; quem anda em
/// laco (`while cursor <= until`) so pode partir de data que `contract::is_iso_date` aceita,
/// e para essas o passo sempre avanca.
pub fn shift_day(date: &str, delta: i64) -> String {
    let shifted = day_number(date)
        .and_then(|n| i32::try_from(delta).ok().and_then(|d| n.checked_add(d)))
        .and_then(date_of);
    shifted.map_or_else(|| date.to_string(), iso)
}

/// `daysBetween` do app: dias corridos de `from` ate `to` (positivo quando `to` e depois).
/// Data fora da forma -> 0 ("hoje"), nunca panico.
pub fn days_between(from: &str, to: &str) -> i64 {
    match (day_number(from), day_number(to)) {
        (Some(a), Some(b)) => i64::from(b) - i64::from(a),
        _ => 0,
    }
}

/// `daysInMonth` do app; mes invalido -> 0 (quem divide por ele tem de tratar).
pub fn days_in_month(month: &str) -> u8 {
    parse_month(month)
        .and_then(|(y, m)| {
            Month::try_from(m)
                .ok()
                .map(|m| time::util::days_in_month(m, y))
        })
        .unwrap_or(0)
}

/// `lastDayOfMonth` do app: `"2026-02"` -> `"2026-02-28"`. Mes invalido volta cru.
pub fn last_day_of_month(month: &str) -> String {
    match days_in_month(month) {
        0 => month.to_string(),
        last => format!("{month}-{last:02}"),
    }
}

/// `dayOfMonthClamped` do app: dia `n` do mes, limitado ao ultimo. `n <= 0` (lixo de versao
/// futura chegando por sync) vale 1, como `Math.max(1, Math.trunc(n) || 1)`. Mes invalido
/// volta cru.
pub fn day_of_month_clamped(month: &str, n: i64) -> String {
    match days_in_month(month) {
        0 => month.to_string(),
        last => format!("{month}-{:02}", n.clamp(1, i64::from(last))),
    }
}

/// `isBusinessDay` do app: segunda a sexta, sem feriados (o app decidiu assim: a tabela de
/// feriados muda todo ano). Data fora da forma -> `false`.
pub fn is_business_day(date: &str) -> bool {
    day_number(date)
        .and_then(date_of)
        .is_some_and(|d| !matches!(d.weekday(), Weekday::Saturday | Weekday::Sunday))
}

/// `nthBusinessDayOfMonth` do app: o `n`-esimo dia util; sem tantos, o ultimo util do mes
/// (melhor um lancamento no fim do mes do que engolir a competencia). `n <= 0` vale 1. Mes
/// invalido volta cru.
pub fn nth_business_day(month: &str, n: i64) -> String {
    let last = days_in_month(month);
    if last == 0 {
        return month.to_string();
    }
    let target = n.max(1);
    let mut count = 0;
    let mut last_business = format!("{month}-01");
    for day in 1..=last {
        let date = format!("{month}-{day:02}");
        if is_business_day(&date) {
            count += 1;
            if count == target {
                return date;
            }
            last_business = date;
        }
    }
    last_business
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
    fn meses_ate_o_prazo() {
        assert_eq!(months_between("2026-09", "2027-07"), 10);
        assert_eq!(months_between("2026-09", "2026-10"), 1);
        assert_eq!(months_between("2026-09", "2026-09"), 0);
        assert_eq!(months_between("2026-09", "2026-06"), -3);
        assert_eq!(months_between("2026-12", "2027-01"), 1);
        assert_eq!(months_between("x", "2027-01"), 0);
        assert_eq!(months_between("2026-09", "2027-13"), 0);
    }

    #[test]
    fn mes_curto_com_ano() {
        assert_eq!(month_short_year("2027-07"), "jul 2027");
        assert_eq!(month_short_year("2026-11"), "nov 2026");
        assert_eq!(month_short_year("2027-13"), "2027-13");
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

    // ---- dias (calendar.test.ts, periods.test.ts, insights.test.ts do app) ----

    #[test]
    fn shift_day_anda_e_atravessa_mes_e_ano() {
        assert_eq!(shift_day("2026-08-10", 1), "2026-08-11");
        assert_eq!(shift_day("2026-08-10", -1), "2026-08-09");
        assert_eq!(shift_day("2026-08-10", 7), "2026-08-17");
        assert_eq!(shift_day("2026-08-31", 1), "2026-09-01");
        assert_eq!(shift_day("2026-01-01", -1), "2025-12-31");
        assert_eq!(shift_day("2024-02-28", 1), "2024-02-29");
        assert_eq!(shift_day("2026-02-28", 1), "2026-03-01");
        assert_eq!(shift_day("2026-02-28", 0), "2026-02-28");
    }

    #[test]
    fn shift_day_normaliza_dia_impossivel_como_o_js() {
        // `Date.UTC(2026, 1, 31)` e 3 de marco: o app normaliza, o hub tambem, senao uma serie
        // com `startOn` assim ficaria parada no laco semanal da H2b.
        assert_eq!(shift_day("2026-02-31", 0), "2026-03-03");
        assert_eq!(shift_day("2026-02-31", 7), "2026-03-10");
        // Fora da forma `YYYY-MM-DD` volta crua, sem panico.
        assert_eq!(shift_day("ontem", 1), "ontem");
        assert_eq!(shift_day("2026-13-01", 1), "2026-13-01");
    }

    #[test]
    fn days_in_month_conhece_bissexto() {
        assert_eq!(days_in_month("2028-02"), 29);
        assert_eq!(days_in_month("2026-02"), 28);
        assert_eq!(days_in_month("2026-09"), 30);
        assert_eq!(days_in_month("2026-12"), 31);
        assert_eq!(days_in_month("2026-13"), 0);
    }

    #[test]
    fn days_between_conta_dias_corridos() {
        assert_eq!(days_between("2026-09-24", "2026-09-30"), 6);
        assert_eq!(days_between("2026-09-30", "2026-10-01"), 1);
        assert_eq!(days_between("2026-10-01", "2026-09-30"), -1);
        assert_eq!(days_between("2026-12-31", "2027-01-01"), 1);
        assert_eq!(days_between("2026-11-19", "2026-11-19"), 0);
        assert_eq!(days_between("lixo", "2026-11-19"), 0);
    }

    // ---- business-day.test.ts (os 7 casos) ----

    #[test]
    fn segunda_a_sexta_sao_uteis_sabado_e_domingo_nao() {
        // 2026-08-10 = segunda, 11=ter, ..., 14=sex, 15=sab, 16=dom
        assert!(is_business_day("2026-08-10"));
        assert!(is_business_day("2026-08-14"));
        assert!(!is_business_day("2026-08-15"));
        assert!(!is_business_day("2026-08-16"));
    }

    #[test]
    fn respeita_o_dia_pedido() {
        assert_eq!(day_of_month_clamped("2026-08", 5), "2026-08-05");
    }

    #[test]
    fn trinta_e_um_de_fevereiro_vira_o_ultimo_dia_do_mes() {
        assert_eq!(day_of_month_clamped("2026-02", 31), "2026-02-28");
        assert_eq!(day_of_month_clamped("2024-02", 31), "2024-02-29");
    }

    #[test]
    fn primeiro_dia_util_de_agosto_2026_e_dia_3_mes_comeca_no_sabado() {
        // 2026-08-01 sab, 02 dom, 03 seg
        assert_eq!(nth_business_day("2026-08", 1), "2026-08-03");
    }

    #[test]
    fn quinto_dia_util_de_agosto_2026_e_dia_7() {
        // 3,4,5,6,7
        assert_eq!(nth_business_day("2026-08", 5), "2026-08-07");
    }

    #[test]
    fn n_maior_que_os_uteis_do_mes_devolve_o_ultimo_util() {
        assert_eq!(nth_business_day("2026-08", 99), "2026-08-31");
    }

    #[test]
    fn conhece_fevereiro_e_meses_de_30() {
        assert_eq!(last_day_of_month("2026-02"), "2026-02-28");
        assert_eq!(last_day_of_month("2026-04"), "2026-04-30");
    }

    #[test]
    fn n_invalido_cai_no_dia_1() {
        // `Math.max(1, Math.trunc(n) || 1)` do app: zero e negativo viram 1.
        assert_eq!(day_of_month_clamped("2026-08", 0), "2026-08-01");
        assert_eq!(day_of_month_clamped("2026-08", -3), "2026-08-01");
        assert_eq!(nth_business_day("2026-08", 0), "2026-08-03");
        // Mes invalido volta cru.
        assert_eq!(day_of_month_clamped("2026-13", 5), "2026-13");
        assert_eq!(nth_business_day("x", 1), "x");
        assert_eq!(last_day_of_month("x"), "x");
    }
}
