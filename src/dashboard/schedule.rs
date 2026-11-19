//! Agenda de uma serie recorrente: competencias, data de cada ocorrencia e chave logica. Port
//! de `src/domain/recurrence/schedule.ts` do app; o hub tem de chegar nas mesmas datas e nas
//! mesmas chaves, senao o id estavel da ocorrencia diverge e ela conta duas vezes.
//!
//! Semanal e quinzenal: a competencia e a propria data (`YYYY-MM-DD`). As demais: `YYYY-MM`,
//! ancorado no mes de `startOn` (bimestral a partir de janeiro e jan, mar, mai..., nunca fev).

use super::contract::{Frequency, Schedule, is_iso_date};
use super::periods::{day_of_month_clamped, nth_business_day, shift_day, shift_month};

impl Frequency {
    /// `FREQUENCY_DAYS` do app; `None` para as que andam em meses.
    pub fn step_days(self) -> Option<i64> {
        match self {
            Self::Weekly => Some(7),
            Self::Biweekly => Some(14),
            _ => None,
        }
    }

    /// `FREQUENCY_MONTHS` do app; `None` para semanal e quinzenal.
    pub fn step_months(self) -> Option<i32> {
        match self {
            Self::Monthly => Some(1),
            Self::Bimonthly => Some(2),
            Self::Quarterly => Some(3),
            Self::Semiannual => Some(6),
            Self::Annual => Some(12),
            Self::Weekly | Self::Biweekly => None,
        }
    }
}

#[cfg(test)] // vetor de paridade com o app; a projecao usa `Frequency::parse`
/// `isKnownFrequency` do app. Valor de versao futura nao e conhecido: a serie fica parada.
pub fn is_known_frequency(frequency: &str) -> bool {
    Frequency::parse(frequency).is_some()
}

/// `periodOf` do app: a competencia que contem `date` no ritmo da serie. A chave semanal nao
/// pode ser o mes, senao duas ocorrencias do mesmo mes colidiriam no id.
pub fn period_of(frequency: Frequency, date: &str) -> String {
    if frequency.is_day_step() {
        date.to_string()
    } else {
        date.get(..7).unwrap_or(date).to_string()
    }
}

#[cfg(test)] // vetor de paridade com o app; a projecao nao volta periodos
/// `previousPeriod` do app: o dia ou o mes antes (nao um passo da serie).
pub fn previous_period(period: &str) -> String {
    if period.len() == 10 {
        shift_day(period, -1)
    } else {
        shift_month(period, -1)
    }
}

/// `stepPeriod` do app: `times` passos da serie a partir de `period`.
pub fn step_period(frequency: Frequency, period: &str, times: i32) -> String {
    match (frequency.step_days(), frequency.step_months()) {
        (Some(days), _) => shift_day(period, days * i64::from(times)),
        (None, Some(months)) => shift_month(period, months.saturating_mul(times)),
        (None, None) => period.to_string(),
    }
}

/// `eachPeriod` do app: competencias de `start_on` ate `until`, no passo da frequencia.
///
/// `start_on` e `until` tem de estar na forma `YYYY-MM-DD`; fora dela devolve vazio. O app
/// confia no tipo; aqui `shift_day`/`shift_month` devolvem a entrada crua quando nao entendem,
/// e um laco que nao anda travaria o hub. Pelo mesmo motivo o laco para se o passo nao avancar
/// (ano 9999).
pub fn each_period(start_on: &str, until: &str, frequency: Frequency) -> Vec<String> {
    let mut periods = Vec::new();
    if !is_iso_date(start_on) || !is_iso_date(until) {
        return periods;
    }
    let (mut cursor, end) = if frequency.is_day_step() {
        // 7 e 14 dias nao dependem do calendario: andar de data em data nao deriva.
        (start_on.to_string(), until.to_string())
    } else {
        (start_on[..7].to_string(), until[..7].to_string())
    };
    while cursor <= end {
        let next = step_period(frequency, &cursor, 1);
        periods.push(cursor.clone());
        if next <= cursor {
            break;
        }
        cursor = next;
    }
    periods
}

/// `occurrenceOn` do app: a data civil da ocorrencia. Competencia ja em forma de data
/// (semanal/quinzenal) e a propria ocorrencia: a agenda do mes nao se aplica, mesmo que a
/// serie a tenha. Mensal sem agenda nao tem data (`None`); o contrato nao deixa isso chegar.
pub fn occurrence_on(period: &str, schedule: Option<Schedule>) -> Option<String> {
    if period.len() == 10 {
        return Some(period.to_string());
    }
    Some(match schedule? {
        Schedule::DayOfMonth(n) => day_of_month_clamped(period, i64::from(n)),
        Schedule::NthBusinessDay(n) => nth_business_day(period, i64::from(n)),
    })
}

/// `occurrenceKey` do app: duas linhas com a mesma chave sao a mesma ocorrencia; o id
/// estavel deriva disto.
pub fn occurrence_key(recurrence_id: &str, period: &str) -> String {
    format!("{recurrence_id}:{period}")
}

/// `periodOfKey` do app: o que vem depois do ultimo `:`.
pub fn period_of_key(key: &str) -> &str {
    key.rfind(':').map_or(key, |i| &key[i + 1..])
}

#[cfg(test)]
mod tests {
    use super::*;

    mod each_period {
        use super::*;

        #[test]
        fn mensal_preenche_de_start_ate_until() {
            assert_eq!(
                each_period("2026-01-15", "2026-03-20", Frequency::Monthly),
                ["2026-01", "2026-02", "2026-03"]
            );
        }

        #[test]
        fn bimestral_respeita_a_ancora() {
            assert_eq!(
                each_period("2026-01-05", "2026-07-01", Frequency::Bimonthly),
                ["2026-01", "2026-03", "2026-05", "2026-07"]
            );
        }

        #[test]
        fn anual_so_aniversarios() {
            assert_eq!(
                each_period("2024-03-10", "2026-08-01", Frequency::Annual),
                ["2024-03", "2025-03", "2026-03"]
            );
        }
    }

    mod occurrence_on {
        use super::*;

        #[test]
        fn dia_fixo_e_dia_util() {
            assert_eq!(
                occurrence_on("2026-08", Some(Schedule::DayOfMonth(10))).as_deref(),
                Some("2026-08-10")
            );
            assert_eq!(
                occurrence_on("2026-08", Some(Schedule::NthBusinessDay(5))).as_deref(),
                Some("2026-08-07")
            );
        }
    }

    mod occurrence_key {
        use super::*;

        #[test]
        fn compoe_id_da_serie_e_competencia() {
            assert_eq!(occurrence_key("ABC", "2026-08"), "ABC:2026-08");
        }
    }

    mod semanal_e_quinzenal {
        use super::*;

        #[test]
        fn semanal_anda_de_7_em_7_dias_atravessando_mes_e_ano() {
            assert_eq!(
                each_period("2026-12-21", "2027-01-12", Frequency::Weekly),
                ["2026-12-21", "2026-12-28", "2027-01-04", "2027-01-11"]
            );
        }

        #[test]
        fn quinzenal_anda_de_14_em_14_dias_a_partir_da_ancora() {
            assert_eq!(
                each_period("2026-01-20", "2026-03-20", Frequency::Biweekly),
                [
                    "2026-01-20",
                    "2026-02-03",
                    "2026-02-17",
                    "2026-03-03",
                    "2026-03-17"
                ]
            );
        }

        #[test]
        fn start_depois_de_until_nao_gera_nada() {
            assert!(each_period("2026-05-10", "2026-05-09", Frequency::Weekly).is_empty());
        }

        #[test]
        fn a_competencia_ja_e_a_data_occurrence_on_ignora_quando_e_dia() {
            assert_eq!(
                occurrence_on("2026-03-17", Some(Schedule::NthBusinessDay(5))).as_deref(),
                Some("2026-03-17")
            );
            assert_eq!(
                occurrence_on("2026-03-17", Some(Schedule::DayOfMonth(31))).as_deref(),
                Some("2026-03-17")
            );
        }

        /// Adaptado: o enum `Frequency` nao representa valor desconhecido, entao o "nao gera" e
        /// o `parse` recusar (a serie fica sem `rule`; `recurrence.rs` cobre que nao projeta).
        #[test]
        fn frequencia_desconhecida_versao_futura_nao_gera_e_nao_trava() {
            assert_eq!(Frequency::parse("fortnightly"), None);
            assert!(!is_known_frequency("fortnightly"));
            assert!(is_known_frequency("biweekly"));
        }

        #[test]
        fn period_of_e_previous_period_seguem_o_formato_da_competencia() {
            assert_eq!(period_of(Frequency::Weekly, "2026-03-17"), "2026-03-17");
            assert_eq!(period_of(Frequency::Monthly, "2026-03-17"), "2026-03");
            assert_eq!(previous_period("2026-03-01"), "2026-02-28");
            assert_eq!(previous_period("2026-03"), "2026-02");
        }
    }

    mod bordas_do_hub {
        use super::*;

        #[test]
        fn data_fora_da_forma_nao_gera_e_nao_trava() {
            assert!(each_period("2026-1-01", "2026-12-31", Frequency::Weekly).is_empty());
            assert!(each_period("2026-01-01", "2026-12", Frequency::Monthly).is_empty());
        }

        #[test]
        fn fim_do_calendario_para_o_laco() {
            let got = each_period("9999-12-20", "9999-12-31", Frequency::Weekly);
            assert_eq!(got, ["9999-12-20", "9999-12-27"]);
        }

        #[test]
        fn dia_impossivel_anda_como_o_date_utc_do_app() {
            assert_eq!(
                each_period("2026-02-31", "2026-03-20", Frequency::Weekly),
                ["2026-02-31", "2026-03-10", "2026-03-17"]
            );
        }

        #[test]
        fn mensal_sem_agenda_nao_tem_data() {
            assert_eq!(occurrence_on("2026-08", None), None);
        }

        #[test]
        fn period_of_key_pega_o_ultimo_trecho() {
            assert_eq!(period_of_key("R1:2026-11"), "2026-11");
            assert_eq!(period_of_key("R1:2026-11-02"), "2026-11-02");
            assert_eq!(period_of_key("sem-dois-pontos"), "sem-dois-pontos");
        }
    }
}
