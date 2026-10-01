//! As linhas do hub interpretadas, em memoria, com as regras de visibilidade do app.
//!
//! O `Dataset` e carregado da store e atualizado por `seq`: cada linha que chega substitui a
//! versao anterior da mesma `(tabela, id)`. LWW ja decidiu no hub qual versao vale; aqui so se
//! interpreta a mais nova, mesmo que ela seja pior (fora do contrato) que a anterior.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::SystemTime;

use base64::Engine;

use super::contract::{self, ContractError, Kind};

/// Uma linha como a store guarda: `id` e `deleted_at` vem das colunas ja validadas pela
/// fatia 1; `data` e o JSON opaco do app.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawRow {
    pub table: String,
    pub id: String,
    pub deleted_at: Option<String>,
    pub seq: i64,
    pub data: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transaction {
    pub id: String,
    pub deleted: bool,
    pub kind: Kind,
    pub description: String,
    pub amount_minor: i64,
    pub occurred_on: String,
    pub category_id: Option<String>,
    pub payment_method_id: Option<String>,
    pub user_id: Option<String>,
    pub recurrence_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Category {
    pub id: String,
    pub deleted: bool,
    pub name: String,
    pub color: String,
    pub icon: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct User {
    pub id: String,
    pub deleted: bool,
    pub name: String,
    pub color: String,
    pub avatar: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaymentMethod {
    pub id: String,
    pub deleted: bool,
    pub name: String,
    pub icon: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recurrence {
    pub id: String,
    pub deleted: bool,
    pub frequency: String,
}

/// O que um `apply` fez: linhas aplicadas (validas ou nao) e total de ignoradas depois dele.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ApplyReport {
    pub applied: usize,
    pub ignored_total: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Dataset {
    /// Inclusive apagadas: a lista precisa dizer "Categoria removida" e uma linha revivida so
    /// troca a marca.
    pub transactions: HashMap<String, Transaction>,
    pub categories: HashMap<String, Category>,
    pub users: HashMap<String, User>,
    pub payment_methods: HashMap<String, PaymentMethod>,
    pub recurrences: HashMap<String, Recurrence>,
    /// Tabela -> ids cuja versao mais nova esta fora do contrato. Conjunto, nao contador: uma
    /// versao valida depois tira a linha daqui.
    pub ignored: BTreeMap<String, BTreeSet<String>>,
    /// Maior `seq` ja aplicado: o proximo refresh pede so `seq > loaded_seq`.
    pub loaded_seq: i64,
    /// So exibicao.
    pub refreshed_at: Option<SystemTime>,
}

/// Nomes das tabelas como o app grava (`RowMap` em `app-state.ts`).
pub const TRANSACTIONS: &str = "transactions";
pub const CATEGORIES: &str = "categories";
pub const USERS: &str = "users";
pub const PAYMENT_METHODS: &str = "paymentMethods";
pub const RECURRENCES: &str = "recurrences";

impl Dataset {
    /// Aplica na ordem recebida (a store entrega por `seq`). Tabela fora do contrato e pulada
    /// sem contar: o app sincroniza tabelas que o dashboard nao le (`recurrenceAdjustments`...).
    pub fn apply(&mut self, rows: impl IntoIterator<Item = RawRow>) -> ApplyReport {
        let mut applied = 0;
        for row in rows {
            self.loaded_seq = self.loaded_seq.max(row.seq);
            let deleted = row.deleted_at.is_some();
            let id = row.id.clone();
            let result: Result<(), ContractError> = match row.table.as_str() {
                TRANSACTIONS => contract::parse_transaction(&row.data).map(|f| {
                    self.transactions.insert(
                        id.clone(),
                        Transaction {
                            id: id.clone(),
                            deleted,
                            kind: f.kind,
                            description: f.description,
                            amount_minor: f.amount_minor,
                            occurred_on: f.occurred_on,
                            category_id: f.category_id,
                            payment_method_id: f.payment_method_id,
                            user_id: f.user_id,
                            recurrence_id: f.recurrence_id,
                        },
                    );
                }),
                CATEGORIES => contract::parse_category(&row.data).map(|f| {
                    self.categories.insert(
                        id.clone(),
                        Category {
                            id: id.clone(),
                            deleted,
                            name: f.name,
                            color: f.color,
                            icon: f.icon,
                        },
                    );
                }),
                USERS => contract::parse_user(&row.data).map(|f| {
                    self.users.insert(
                        id.clone(),
                        User {
                            id: id.clone(),
                            deleted,
                            name: f.name,
                            color: f.color,
                            avatar: f.avatar,
                        },
                    );
                }),
                PAYMENT_METHODS => contract::parse_payment_method(&row.data).map(|f| {
                    self.payment_methods.insert(
                        id.clone(),
                        PaymentMethod {
                            id: id.clone(),
                            deleted,
                            name: f.name,
                            icon: f.icon,
                        },
                    );
                }),
                RECURRENCES => contract::parse_recurrence(&row.data).map(|f| {
                    self.recurrences.insert(
                        id.clone(),
                        Recurrence {
                            id: id.clone(),
                            deleted,
                            frequency: f.frequency,
                        },
                    );
                }),
                _ => continue,
            };
            applied += 1;
            match result {
                Ok(()) => {
                    if let Some(set) = self.ignored.get_mut(&row.table) {
                        set.remove(&id);
                        if set.is_empty() {
                            self.ignored.remove(&row.table);
                        }
                    }
                }
                Err(e) => {
                    // A versao mais nova e a verdade (LWW): a anterior valida sai tambem.
                    self.remove(&row.table, &id);
                    tracing::warn!("dashboard: {}/{id} ignorada: {e}", row.table);
                    self.ignored.entry(row.table).or_default().insert(id);
                }
            }
        }
        ApplyReport {
            applied,
            ignored_total: self.ignored_total(),
        }
    }

    fn remove(&mut self, table: &str, id: &str) {
        match table {
            TRANSACTIONS => {
                self.transactions.remove(id);
            }
            CATEGORIES => {
                self.categories.remove(id);
            }
            USERS => {
                self.users.remove(id);
            }
            PAYMENT_METHODS => {
                self.payment_methods.remove(id);
            }
            RECURRENCES => {
                self.recurrences.remove(id);
            }
            _ => {}
        }
    }

    /// `deleted_at IS NULL`, igual ao `isAlive` do app.
    pub fn alive_transactions(&self) -> impl Iterator<Item = &Transaction> {
        self.transactions.values().filter(|t| !t.deleted)
    }

    /// Categoria visivel ou `None` (`findCategory`): cor e icone de registro apagado nao
    /// voltam a tela.
    pub fn find_category(&self, id: Option<&str>) -> Option<&Category> {
        id.and_then(|id| self.categories.get(id))
            .filter(|c| !c.deleted)
    }

    /// Perfil visivel ou `None` (`findUser`). Sem autor e autor apagado colapsam de proposito:
    /// distinguir vazaria "esse perfil existiu" sem ganho nenhum.
    pub fn find_user(&self, id: Option<&str>) -> Option<&User> {
        id.and_then(|id| self.users.get(id)).filter(|u| !u.deleted)
    }

    pub fn find_payment_method(&self, id: Option<&str>) -> Option<&PaymentMethod> {
        id.and_then(|id| self.payment_methods.get(id))
            .filter(|p| !p.deleted)
    }

    /// Rotulo da coluna Categoria (`resolveCategoryName`): nulo e referencia morta tem
    /// rotulos diferentes.
    pub fn category_name(&self, id: Option<&str>) -> String {
        match id {
            None => "Sem categoria".into(),
            Some(_) => self
                .find_category(id)
                .map_or_else(|| "Categoria removida".into(), |c| c.name.clone()),
        }
    }

    /// Rotulo da coluna Pagamento (`resolvePaymentMethodName`).
    pub fn payment_method_name(&self, id: Option<&str>) -> String {
        match id {
            None => "Sem forma de pagamento".into(),
            Some(_) => self
                .find_payment_method(id)
                .map_or_else(|| "Forma removida".into(), |p| p.name.clone()),
        }
    }

    /// Tag de recorrencia: `None` = sem tag. Serie ausente, apagada ou de frequencia
    /// desconhecida vira "Recorrente": o lancamento continua sendo de uma serie.
    pub fn recurrence_label(&self, id: Option<&str>) -> Option<String> {
        let id = id?;
        let label = self
            .recurrences
            .get(id)
            .filter(|r| !r.deleted)
            .and_then(|r| frequency_label(&r.frequency))
            .unwrap_or("Recorrente");
        Some(label.to_string())
    }

    pub fn ignored_total(&self) -> usize {
        self.ignored.values().map(BTreeSet::len).sum()
    }

    /// Nenhuma transacao viva: o estado "Sem dados" das telas.
    pub fn is_empty(&self) -> bool {
        self.alive_transactions().next().is_none()
    }

    /// Perfis vivos por nome e depois id. `to_lowercase` sem remover acento: "Érico" fica
    /// depois de "Luiz" (o `localeCompare` do app o poria entre "Bia" e "Luiz"). Limite aceito
    /// para nao trazer uma crate de colacao por causa de uma lista de chips.
    pub fn alive_users(&self) -> Vec<&User> {
        let mut users: Vec<&User> = self.users.values().filter(|u| !u.deleted).collect();
        users.sort_by(|a, b| (a.name.to_lowercase(), &a.id).cmp(&(b.name.to_lowercase(), &b.id)));
        users
    }
}

/// `FREQUENCY_LABELS` do app (`recurrence.ts`).
pub fn frequency_label(frequency: &str) -> Option<&'static str> {
    Some(match frequency {
        "monthly" => "Mensal",
        "bimonthly" => "Bimestral",
        "quarterly" => "Trimestral",
        "semiannual" => "Semestral",
        "annual" => "Anual",
        _ => return None,
    })
}

/// Prefixos que o app aceita (`ALLOWED` em `avatar-view.tsx`). SVG fica de fora la e aqui.
const AVATAR_PREFIXES: [&str; 3] = [
    "data:image/webp;base64,",
    "data:image/jpeg;base64,",
    "data:image/png;base64,",
];

/// `data:image/webp;base64,...` -> bytes. Prefixo fora da lista ou base64 quebrado -> `None`
/// (a tela mostra a inicial).
pub fn avatar_bytes(data_uri: &str) -> Option<Vec<u8>> {
    let payload = AVATAR_PREFIXES
        .iter()
        .find_map(|prefix| data_uri.strip_prefix(prefix))?;
    base64::engine::general_purpose::STANDARD
        .decode(payload.trim())
        .ok()
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    const T1: &str = "01HZZZZZZZZZZZZZZZZZZZZZT1";
    const C1: &str = "01HZZZZZZZZZZZZZZZZZZZZZC1";
    const R1: &str = "01HZZZZZZZZZZZZZZZZZZZZZR1";

    fn raw(table: &str, id: &str, deleted: Option<&str>, seq: i64, data: Value) -> RawRow {
        RawRow {
            table: table.into(),
            id: id.into(),
            deleted_at: deleted.map(Into::into),
            seq,
            data: data.to_string(),
        }
    }

    fn tx(description: &str) -> Value {
        json!({
            "kind": "expense",
            "description": description,
            "amountMinor": 1000,
            "occurredOn": "2026-09-14",
            "categoryId": C1
        })
    }

    fn category(name: &str) -> Value {
        json!({ "name": name, "color": "orange", "icon": "utensils" })
    }

    fn user(name: &str) -> Value {
        json!({ "name": name, "color": "sky" })
    }

    #[test]
    fn aplica_transacao_valida() {
        let mut ds = Dataset::default();
        let report = ds.apply([raw(TRANSACTIONS, T1, None, 7, tx("Mercado"))]);
        assert_eq!(
            report,
            ApplyReport {
                applied: 1,
                ignored_total: 0
            }
        );
        assert_eq!(ds.transactions.len(), 1);
        assert_eq!(ds.alive_transactions().count(), 1);
        assert_eq!(ds.loaded_seq, 7);
        assert!(!ds.is_empty());
    }

    #[test]
    fn seq_maior_substitui() {
        let mut ds = Dataset::default();
        ds.apply([raw(TRANSACTIONS, T1, None, 1, tx("Mercado"))]);
        ds.apply([raw(TRANSACTIONS, T1, None, 2, tx("Feira"))]);
        assert_eq!(ds.transactions.len(), 1);
        assert_eq!(ds.transactions[T1].description, "Feira");
        assert_eq!(ds.loaded_seq, 2);
    }

    #[test]
    fn apagada_fica_marcada_e_revive() {
        let mut ds = Dataset::default();
        ds.apply([raw(
            TRANSACTIONS,
            T1,
            Some("2026-09-15T00:00:00Z"),
            1,
            tx("Mercado"),
        )]);
        assert_eq!(ds.transactions.len(), 1);
        assert_eq!(ds.alive_transactions().count(), 0);
        assert!(ds.is_empty());
        ds.apply([raw(TRANSACTIONS, T1, None, 2, tx("Mercado"))]);
        assert_eq!(ds.alive_transactions().count(), 1);
    }

    #[test]
    fn versao_invalida_tira_a_valida_e_conta() {
        let mut ds = Dataset::default();
        ds.apply([raw(TRANSACTIONS, T1, None, 1, tx("Mercado"))]);
        let mut bad = tx("Mercado");
        bad["amountMinor"] = json!("x");
        let report = ds.apply([raw(TRANSACTIONS, T1, None, 2, bad)]);
        assert!(!ds.transactions.contains_key(T1));
        assert!(ds.ignored[TRANSACTIONS].contains(T1));
        assert_eq!(report.ignored_total, 1);
        assert_eq!(ds.ignored_total(), 1);

        let report = ds.apply([raw(TRANSACTIONS, T1, None, 3, tx("Mercado"))]);
        assert!(ds.transactions.contains_key(T1));
        assert!(ds.ignored.is_empty());
        assert_eq!(report.ignored_total, 0);
    }

    #[test]
    fn recorrencias_entram_e_tabela_desconhecida_e_pulada() {
        let mut ds = Dataset::default();
        let report = ds.apply([
            raw(RECURRENCES, R1, None, 1, json!({ "frequency": "monthly" })),
            raw(
                "recurrenceAdjustments",
                "01HZZZZZZZZZZZZZZZZZZZZZA1",
                None,
                2,
                json!({ "x": 1 }),
            ),
        ]);
        assert_eq!(report.applied, 1);
        assert_eq!(ds.recurrences.len(), 1);
        assert_eq!(ds.ignored_total(), 0);
        // O seq da tabela pulada conta: o proximo refresh nao a pede de novo.
        assert_eq!(ds.loaded_seq, 2);
    }

    #[test]
    fn referencias_mortas() {
        let mut ds = Dataset::default();
        let c_dead = "01HZZZZZZZZZZZZZZZZZZZZZC2";
        let pm = "01HZZZZZZZZZZZZZZZZZZZZZM1";
        let pm_dead = "01HZZZZZZZZZZZZZZZZZZZZZM2";
        let u_dead = "01HZZZZZZZZZZZZZZZZZZZZZP2";
        ds.apply([
            raw(CATEGORIES, C1, None, 1, category("Mercado")),
            raw(CATEGORIES, c_dead, Some("x"), 2, category("Antiga")),
            raw(
                PAYMENT_METHODS,
                pm,
                None,
                3,
                json!({ "name": "Pix", "icon": "zap" }),
            ),
            raw(
                PAYMENT_METHODS,
                pm_dead,
                Some("x"),
                4,
                json!({ "name": "Cheque", "icon": "receipt" }),
            ),
            raw(USERS, u_dead, Some("x"), 5, user("Ex")),
        ]);
        assert_eq!(
            ds.find_category(Some(C1)).map(|c| c.name.as_str()),
            Some("Mercado")
        );
        assert_eq!(ds.find_category(Some(c_dead)), None);
        assert_eq!(ds.find_category(None), None);
        assert_eq!(ds.category_name(None), "Sem categoria");
        assert_eq!(ds.category_name(Some(c_dead)), "Categoria removida");
        assert_eq!(ds.category_name(Some("ausente")), "Categoria removida");
        assert_eq!(ds.category_name(Some(C1)), "Mercado");
        assert_eq!(ds.payment_method_name(None), "Sem forma de pagamento");
        assert_eq!(ds.payment_method_name(Some(pm_dead)), "Forma removida");
        assert_eq!(ds.payment_method_name(Some(pm)), "Pix");
        assert_eq!(ds.find_user(Some(u_dead)), None);
        assert_eq!(ds.find_user(None), None);
    }

    #[test]
    fn rotulo_de_recorrencia() {
        let mut ds = Dataset::default();
        let weekly = "01HZZZZZZZZZZZZZZZZZZZZZR2";
        ds.apply([
            raw(RECURRENCES, R1, None, 1, json!({ "frequency": "monthly" })),
            raw(
                RECURRENCES,
                weekly,
                None,
                2,
                json!({ "frequency": "weekly" }),
            ),
        ]);
        assert_eq!(ds.recurrence_label(None), None);
        assert_eq!(ds.recurrence_label(Some(R1)).as_deref(), Some("Mensal"));
        assert_eq!(
            ds.recurrence_label(Some("ausente")).as_deref(),
            Some("Recorrente")
        );
        assert_eq!(
            ds.recurrence_label(Some(weekly)).as_deref(),
            Some("Recorrente")
        );
    }

    #[test]
    fn so_tombstones_e_vazio() {
        let mut ds = Dataset::default();
        ds.apply([raw(TRANSACTIONS, T1, Some("x"), 1, tx("Mercado"))]);
        assert!(ds.is_empty());
        assert!(Dataset::default().is_empty());
    }

    #[test]
    fn perfis_vivos_em_ordem_de_nome() {
        let mut ds = Dataset::default();
        ds.apply([
            raw(USERS, "01HZZZZZZZZZZZZZZZZZZZZZP4", None, 1, user("Luiz")),
            raw(USERS, "01HZZZZZZZZZZZZZZZZZZZZZP3", None, 2, user("bia")),
            raw(USERS, "01HZZZZZZZZZZZZZZZZZZZZZP2", None, 3, user("Ana")),
            raw(USERS, "01HZZZZZZZZZZZZZZZZZZZZZP1", None, 4, user("Ana")),
            raw(
                USERS,
                "01HZZZZZZZZZZZZZZZZZZZZZP5",
                Some("x"),
                5,
                user("Apagado"),
            ),
        ]);
        let names: Vec<(&str, &str)> = ds
            .alive_users()
            .iter()
            .map(|u| (u.name.as_str(), &u.id[24..]))
            .collect();
        assert_eq!(
            names,
            vec![("Ana", "P1"), ("Ana", "P2"), ("bia", "P3"), ("Luiz", "P4")]
        );
    }

    #[test]
    fn avatar_decodifica_so_os_prefixos_do_app() {
        let png = avatar_bytes("data:image/png;base64,iVBORw0KGgo=").unwrap();
        assert_eq!(&png[..4], &[0x89, b'P', b'N', b'G']);
        assert!(avatar_bytes("data:image/jpeg;base64,/9j/4AAQ").is_some());
        assert!(avatar_bytes("data:image/webp;base64,UklGRg==").is_some());
        assert_eq!(avatar_bytes("data:image/svg+xml;base64,PHN2Zz4="), None);
        assert_eq!(avatar_bytes("data:image/png;base64,%%%nao-e-base64"), None);
        assert_eq!(avatar_bytes("https://exemplo/foto.png"), None);
    }
}
