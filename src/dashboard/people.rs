//! Pessoa x aparelho, para "Usuarios conectados" na tela Conexao.
//!
//! O aparelho vem da tabela `devices` do hub; a pessoa, da linha `users[user_id]` que o app
//! sincroniza. Os dois so se encontram aqui, e so para exibicao.

use std::time::SystemTime;

use time::format_description::well_known::Rfc3339;
use time::{OffsetDateTime, UtcOffset};

use super::colors;
use super::dataset::Dataset;

/// Copia pura de `hub::snapshot::DeviceView`: este modulo nao conhece o hub. O
/// `From<&DeviceView>` vive no hub.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceInfo {
    pub device_id: String,
    pub name: String,
    pub user_id: Option<String>,
    pub paired_at: String,
    pub last_seen_at: Option<String>,
    pub last_push_at: Option<String>,
    pub last_pull_at: Option<String>,
    pub revoked: bool,
}

/// Uma linha de "Usuarios conectados".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Person {
    pub device_id: String,
    /// Nome do perfil; sem perfil conhecido, o nome do aparelho.
    pub display_name: String,
    /// Token de cor do perfil; `None` sem perfil (avatar em `neutral_600`, sem tag).
    pub color: Option<String>,
    pub color_name: Option<String>,
    pub avatar_uri: Option<String>,
    pub device_name: String,
    pub paired_at: String,
    pub last_seen_at: Option<String>,
    /// O perfil do `user_id` ja chegou por sync e esta vivo.
    pub user_known: bool,
}

/// So aparelhos ativos, mais recentes primeiro. Dois aparelhos da mesma pessoa sao duas
/// linhas: remover e por aparelho.
pub fn people(dataset: &Dataset, devices: &[DeviceInfo]) -> Vec<Person> {
    let mut active: Vec<&DeviceInfo> = devices.iter().filter(|d| !d.revoked).collect();
    active.sort_by(|a, b| {
        b.paired_at
            .cmp(&a.paired_at)
            .then_with(|| a.device_id.cmp(&b.device_id))
    });
    active
        .into_iter()
        .map(|d| {
            let user = dataset.find_user(d.user_id.as_deref());
            Person {
                device_id: d.device_id.clone(),
                display_name: user.map_or_else(|| d.name.clone(), |u| u.name.clone()),
                color: user.map(|u| u.color.clone()),
                color_name: user.map(|u| colors::color_name(&u.color).to_string()),
                avatar_uri: user.and_then(|u| u.avatar.clone()),
                device_name: d.name.clone(),
                paired_at: d.paired_at.clone(),
                last_seen_at: d.last_seen_at.clone(),
                user_known: user.is_some(),
            }
        })
        .collect()
}

/// Online = alguma requisicao autenticada nos ultimos 10 min.
pub const ONLINE_WINDOW_SECS: i64 = 10 * 60;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Presence {
    Online { ago: String },
    Seen { ago: String },
    Never,
}

impl Presence {
    /// Texto da meta: "Online · há 2 min", "Visto há 3 dias" ou, sem nenhuma requisicao
    /// ainda, "Ainda não sincronizou" (perfil conhecido) / "Aguardando o primeiro sync".
    pub fn label(&self, user_known: bool) -> String {
        match self {
            Presence::Online { ago } => format!("Online · {ago}"),
            Presence::Seen { ago } => format!("Visto {ago}"),
            Presence::Never if user_known => "Ainda não sincronizou".into(),
            Presence::Never => "Aguardando o primeiro sync".into(),
        }
    }

    pub fn is_online(&self) -> bool {
        matches!(self, Presence::Online { .. })
    }
}

/// `now` injetado: so exibicao, nunca decisao.
pub fn presence(last_seen_at: Option<&str>, now: SystemTime) -> Presence {
    let Some(seen) = last_seen_at.and_then(|s| OffsetDateTime::parse(s, &Rfc3339).ok()) else {
        return Presence::Never;
    };
    let now = OffsetDateTime::from(now);
    let secs = (now - seen).whole_seconds().max(0);
    let ago = ago(secs);
    if secs <= ONLINE_WINDOW_SECS {
        Presence::Online { ago }
    } else {
        Presence::Seen { ago }
    }
}

/// "agora há pouco", "há 2 min", "há 1 h", "há 1 dia", "há 3 dias".
pub fn ago(secs: i64) -> String {
    match secs.max(0) {
        0..60 => "agora há pouco".to_string(),
        s @ 60..3600 => format!("há {} min", s / 60),
        s @ 3600..86_400 => format!("há {} h", s / 3600),
        86_400..172_800 => "há 1 dia".to_string(),
        s => format!("há {} dias", s / 86_400),
    }
}

const MONTHS_SHORT: [&str; 12] = [
    "jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez",
];

/// "12 mar 2026" no fuso de quem olha. Data ilegivel volta crua.
pub fn paired_label(rfc3339: &str, offset: UtcOffset) -> String {
    match OffsetDateTime::parse(rfc3339, &Rfc3339) {
        Ok(t) => {
            let t = t.to_offset(offset);
            let month = MONTHS_SHORT[usize::from(u8::from(t.month())) - 1];
            format!("{} {month} {}", t.day(), t.year())
        }
        Err(_) => rfc3339.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use serde_json::json;

    use super::*;
    use crate::dashboard::dataset::{RawRow, USERS};

    const USER: &str = "01HZZZZZZZZZZZZZZZZZZZZZP1";

    fn device(id: &str, name: &str, user_id: Option<&str>, paired_at: &str) -> DeviceInfo {
        DeviceInfo {
            device_id: id.into(),
            name: name.into(),
            user_id: user_id.map(Into::into),
            paired_at: paired_at.into(),
            last_seen_at: None,
            last_push_at: None,
            last_pull_at: None,
            revoked: false,
        }
    }

    fn dataset_with_ana(deleted: bool) -> Dataset {
        let mut ds = Dataset::default();
        ds.apply([RawRow {
            table: USERS.into(),
            id: USER.into(),
            deleted_at: deleted.then(|| "x".to_string()),
            seq: 1,
            data:
                json!({ "name": "Ana", "color": "fuchsia", "avatar": "data:image/png;base64,AAAA" })
                    .to_string(),
        }]);
        ds
    }

    #[test]
    fn perfil_conhecido_vira_a_pessoa() {
        let ds = dataset_with_ana(false);
        let list = people(
            &ds,
            &[device("D1", "Pixel 7", Some(USER), "2026-03-12T10:00:00Z")],
        );
        let p = &list[0];
        assert_eq!(p.display_name, "Ana");
        assert_eq!(p.color.as_deref(), Some("fuchsia"));
        assert_eq!(p.color_name.as_deref(), Some("Magenta"));
        assert!(p.avatar_uri.is_some());
        assert_eq!(p.device_name, "Pixel 7");
        assert!(p.user_known);
    }

    #[test]
    fn perfil_ainda_nao_sincronizado_ou_apagado_usa_o_aparelho() {
        for ds in [Dataset::default(), dataset_with_ana(true)] {
            let list = people(
                &ds,
                &[device("D1", "Pixel 7", Some(USER), "2026-03-12T10:00:00Z")],
            );
            let p = &list[0];
            assert_eq!(p.display_name, "Pixel 7");
            assert_eq!(p.color, None);
            assert_eq!(p.color_name, None);
            assert_eq!(p.avatar_uri, None);
            assert!(!p.user_known);
        }
    }

    #[test]
    fn sem_user_id_usa_o_aparelho() {
        let ds = dataset_with_ana(false);
        let list = people(&ds, &[device("D1", "Moto G", None, "2026-03-12T10:00:00Z")]);
        assert_eq!(list[0].display_name, "Moto G");
        assert!(!list[0].user_known);
    }

    #[test]
    fn revogado_fica_fora_e_mesma_pessoa_em_dois_aparelhos_sao_duas_linhas() {
        let ds = dataset_with_ana(false);
        let mut revoked = device("D3", "Velho", Some(USER), "2026-03-14T10:00:00Z");
        revoked.revoked = true;
        let list = people(
            &ds,
            &[
                device("D1", "Pixel 7", Some(USER), "2026-03-12T10:00:00Z"),
                device("D2", "Tablet", Some(USER), "2026-03-13T10:00:00Z"),
                revoked,
            ],
        );
        let ids: Vec<&str> = list.iter().map(|p| p.device_id.as_str()).collect();
        assert_eq!(ids, vec!["D2", "D1"]);
        assert!(list.iter().all(|p| p.display_name == "Ana"));
    }

    #[test]
    fn presenca() {
        let now = SystemTime::from(time::macros::datetime!(2026-10-01 12:00 UTC));
        let two_min = presence(Some("2026-10-01T11:58:00Z"), now);
        assert_eq!(
            two_min,
            Presence::Online {
                ago: "há 2 min".into()
            }
        );
        assert_eq!(two_min.label(true), "Online · há 2 min");
        let three_days = presence(Some("2026-09-28T12:00:00Z"), now);
        assert_eq!(three_days.label(true), "Visto há 3 dias");
        assert!(!three_days.is_online());
        assert_eq!(presence(None, now), Presence::Never);
        assert_eq!(Presence::Never.label(true), "Ainda não sincronizou");
        assert_eq!(Presence::Never.label(false), "Aguardando o primeiro sync");
        // Limite: exatamente 10 min ainda e online; 11 nao.
        let ten = now - Duration::from_secs(600);
        let ten = OffsetDateTime::from(ten).format(&Rfc3339).unwrap();
        assert!(presence(Some(&ten), now).is_online());
        assert!(!presence(Some("2026-10-01T11:49:00Z"), now).is_online());
        assert_eq!(presence(Some("lixo"), now), Presence::Never);
    }

    #[test]
    fn rotulo_de_pareamento() {
        assert_eq!(
            paired_label("2026-03-12T10:00:00Z", UtcOffset::UTC),
            "12 mar 2026"
        );
        // No fuso de Brasilia, 01:00 UTC ainda e o dia anterior.
        let brt = UtcOffset::from_hms(-3, 0, 0).unwrap();
        assert_eq!(paired_label("2026-03-12T01:00:00Z", brt), "11 mar 2026");
        assert_eq!(paired_label("ontem", UtcOffset::UTC), "ontem");
    }
}
