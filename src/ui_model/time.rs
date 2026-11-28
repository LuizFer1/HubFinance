//! Datas para exibicao. `SystemTime::now()` aqui e so exibicao, nunca decisao.

use time::format_description::well_known::Rfc3339;
use time::macros::format_description;
use time::{OffsetDateTime, UtcOffset};

use crate::dashboard::periods::relative_secs;

pub fn parse(rfc3339: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(rfc3339, &Rfc3339).ok()
}

pub fn format_time(t: OffsetDateTime, offset: UtcOffset) -> String {
    t.to_offset(offset)
        .format(format_description!("[hour]:[minute]"))
        .unwrap_or_default()
}

/// "há 3 min" em relacao a `now` (as faixas sao as de `periods::relative_secs`).
pub fn relative_to(t: OffsetDateTime, now: OffsetDateTime) -> String {
    relative_secs((now - t).whole_seconds())
}

/// `relative_to` com o relogio atual; data ilegivel ou ausente vira "nunca".
pub fn relative(rfc3339: Option<&str>) -> String {
    match rfc3339.and_then(parse) {
        Some(t) => relative_to(t, OffsetDateTime::now_utc()),
        None => "nunca".to_string(),
    }
}

/// "25 nov 2026, 14:32" no fuso local: data e hora da conversa, porque todas se chamam "Analise
/// de <mes>" e so o momento as distingue. Ilegivel (ou vazio, registro antigo) vira `None` e a
/// linha mostra so o titulo.
pub fn date_time(rfc3339: &str, offset: UtcOffset) -> Option<String> {
    let t = parse(rfc3339)?.to_offset(offset);
    let month = crate::dashboard::periods::month_short(&format!(
        "{:04}-{:02}",
        t.year(),
        u8::from(t.month())
    ));
    Some(format!(
        "{} {month} {}, {}",
        t.day(),
        t.year(),
        format_time(t, offset)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn relativo_em_faixas() {
        let now = datetime!(2026-10-01 12:00 UTC);
        assert_eq!(
            relative_to(datetime!(2026-10-01 11:59:30 UTC), now),
            "agora há pouco"
        );
        assert_eq!(
            relative_to(datetime!(2026-10-01 11:58 UTC), now),
            "há 2 min"
        );
        assert_eq!(relative_to(datetime!(2026-10-01 10:30 UTC), now), "há 1 h");
        assert_eq!(
            relative_to(datetime!(2026-09-30 11:00 UTC), now),
            "há 1 dia"
        );
        assert_eq!(
            relative_to(datetime!(2026-09-28 12:00 UTC), now),
            "há 3 dias"
        );
        // Relogio do aparelho adiantado nao vira "ha -2 min".
        assert_eq!(
            relative_to(datetime!(2026-10-01 12:05 UTC), now),
            "agora há pouco"
        );
    }

    #[test]
    fn data_e_hora_no_fuso_local() {
        let br = UtcOffset::from_hms(-3, 0, 0).unwrap();
        assert_eq!(
            date_time("2026-11-25T10:00:00.000Z", br).as_deref(),
            Some("25 nov 2026, 07:00")
        );
        // A virada do dia segue o fuso, nao o UTC.
        assert_eq!(
            date_time("2026-12-01T01:30:00Z", br).as_deref(),
            Some("30 nov 2026, 22:30")
        );
        assert_eq!(date_time("", br), None);
        assert_eq!(date_time("ontem", br), None);
    }

    #[test]
    fn ilegivel_vira_nunca() {
        assert_eq!(relative(None), "nunca");
        assert_eq!(relative(Some("ontem")), "nunca");
    }
}
