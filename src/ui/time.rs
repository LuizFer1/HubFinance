//! Datas para exibicao. `SystemTime::now()` aqui e so exibicao, nunca decisao.

use time::format_description::well_known::Rfc3339;
use time::macros::format_description;
use time::{OffsetDateTime, UtcOffset};

pub fn parse(rfc3339: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(rfc3339, &Rfc3339).ok()
}

pub fn format_time(t: OffsetDateTime, offset: UtcOffset) -> String {
    t.to_offset(offset)
        .format(format_description!("[hour]:[minute]"))
        .unwrap_or_default()
}

/// "há 3 min" em relacao a `now`.
pub fn relative_to(t: OffsetDateTime, now: OffsetDateTime) -> String {
    let secs = (now - t).whole_seconds().max(0);
    match secs {
        0..60 => "agora há pouco".to_string(),
        60..3600 => format!("há {} min", secs / 60),
        3600..86_400 => format!("há {} h", secs / 3600),
        86_400..172_800 => "há 1 dia".to_string(),
        _ => format!("há {} dias", secs / 86_400),
    }
}

/// `relative_to` com o relogio atual; data ilegivel ou ausente vira "nunca".
pub fn relative(rfc3339: Option<&str>) -> String {
    match rfc3339.and_then(parse) {
        Some(t) => relative_to(t, OffsetDateTime::now_utc()),
        None => "nunca".to_string(),
    }
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
    fn ilegivel_vira_nunca() {
        assert_eq!(relative(None), "nunca");
        assert_eq!(relative(Some("ontem")), "nunca");
    }
}
