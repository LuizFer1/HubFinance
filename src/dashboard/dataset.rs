//! As linhas do hub interpretadas, em memoria, com as regras de visibilidade do app.
//!
//! O `Dataset` e carregado da store e atualizado por `seq`: cada linha que chega substitui a
//! versao anterior da mesma `(tabela, id)`. LWW ja decidiu no hub qual versao vale; aqui so se
//! interpreta a mais nova, mesmo que ela seja pior (fora do contrato) que a anterior.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::SystemTime;

use base64::Engine;

use super::contract::{
    self, AssetKind, Book, ContractError, Kind, MovementKind, ReportState, ReserveKind,
};

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
    /// `None` = conta normalmente (inclui valor que esta versao nao conhece).
    pub report_state: Option<ReportState>,
    /// Conta da linha; ausente ou desconhecido vale `Personal` (ver `Book`).
    pub book: Book,
}

impl Transaction {
    /// A regra unica de "conta nos totais" (`countsInTotals` do app). Toda soma do dashboard
    /// passa por aqui; a lista continua mostrando a linha, marcada.
    pub fn counts_in_totals(&self) -> bool {
        self.report_state.is_none()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Category {
    pub id: String,
    pub deleted: bool,
    pub name: String,
    pub color: String,
    pub icon: String,
    /// So vale em tombstone; ver `resolve_category_id`.
    pub merged_into: Option<String>,
    /// Conta da linha; ausente ou desconhecido vale `Personal` (ver `Book`).
    pub book: Book,
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
    /// So vale em tombstone; ver `resolve_payment_method_id`.
    pub merged_into: Option<String>,
    /// Conta da linha; ausente ou desconhecido vale `Personal` (ver `Book`).
    pub book: Book,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recurrence {
    pub id: String,
    pub deleted: bool,
    pub frequency: String,
    /// Conta da linha; ausente ou desconhecido vale `Personal` (ver `Book`).
    pub book: Book,
}

/// Meta mensal de uma categoria. Pode haver mais de uma por categoria (cada aparelho escreve a
/// sua e o app as converge; depois de uma fusao de padroes sobram duas): quem escolhe e
/// `budgets::current_budgets`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Budget {
    pub id: String,
    pub deleted: bool,
    pub category_id: String,
    pub limit_minor: i64,
    /// HLC da linha; `None` perde para qualquer versao conhecida.
    pub updated_at: Option<String>,
    /// Conta da linha; ausente ou desconhecido vale `Personal` (ver `Book`).
    pub book: Book,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reserve {
    pub id: String,
    pub deleted: bool,
    pub kind: ReserveKind,
    pub name: String,
    pub icon: String,
    pub color: String,
    pub goal_minor: Option<i64>,
    pub multiple: Option<u8>,
    /// Nulo no JSON vira vazio: para a conta, "nenhuma categoria" e "nao configurado" sao o
    /// mesmo.
    pub essential_category_ids: Vec<String>,
    pub due_month: Option<String>,
    pub recurring_amount_minor: Option<i64>,
    /// Primeiro mes em que o deposito mensal vale; o app so materializa a partir dele.
    /// Nao entra em ritmo/ETA: o app tambem nao o usa na projecao, so na materializacao do
    /// deposito (`planDeposits` em deposits.ts).
    pub recurring_since: Option<String>,
    /// Custo essencial digitado no app quando nao havia historico.
    pub essential_override_minor: Option<i64>,
    /// Conta da linha; ausente ou desconhecido vale `Personal` (ver `Book`).
    pub book: Book,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReserveMovement {
    pub id: String,
    pub deleted: bool,
    pub reserve_id: String,
    pub kind: MovementKind,
    pub amount_minor: i64,
    pub occurred_on: String,
    pub user_id: Option<String>,
    pub description: Option<String>,
    /// Nulo no JSON vira `false`: sem a tag "Mensal".
    pub recurring: bool,
    /// Conta da linha; ausente ou desconhecido vale `Personal` (ver `Book`).
    pub book: Book,
}

/// Linha de `books`: a conta comercial (nome, cor e icone). Existe uma linha viva `business`
/// quando o app criou a conta; e ela que liga o seletor Pessoal | Comercial da janela.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BookAccount {
    pub id: String,
    pub deleted: bool,
    pub kind: Book,
    pub name: String,
    pub color: String,
    pub icon: String,
}

/// Bem do negocio (`assets`). Nunca e receita nem despesa.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asset {
    pub id: String,
    pub deleted: bool,
    pub kind: AssetKind,
    pub name: String,
    pub value_minor: i64,
    pub acquired_on: String,
    pub note: Option<String>,
    /// Conta da linha; ausente ou desconhecido vale `Personal` (ver `Book`).
    pub book: Book,
}

/// Divida do negocio (`debts`): `balance_minor` e o saldo devedor atual.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Debt {
    pub id: String,
    pub deleted: bool,
    pub name: String,
    pub creditor: Option<String>,
    pub principal_minor: i64,
    pub balance_minor: i64,
    pub due_on: Option<String>,
    /// Conta da linha; ausente ou desconhecido vale `Personal` (ver `Book`).
    pub book: Book,
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
    pub reserves: HashMap<String, Reserve>,
    pub reserve_movements: HashMap<String, ReserveMovement>,
    pub budgets: HashMap<String, Budget>,
    /// So a H2 (painel "Negocio") le estas tres; entram aqui junto com o contrato para a linha
    /// fora dele ser contada desde ja.
    pub books: HashMap<String, BookAccount>,
    pub assets: HashMap<String, Asset>,
    pub debts: HashMap<String, Debt>,
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
/// As duas de reservas seguem o formato que o app grava (`reserve.ts`), nao um contrato do hub.
pub const RESERVES: &str = "reserves";
pub const RESERVE_MOVEMENTS: &str = "reserveMovements";
/// Metas por categoria (`budget.ts` do app).
pub const BUDGETS: &str = "budgets";
/// Conta comercial e seus bens e dividas (`book.ts`, `asset.ts`, `debt.ts` do app).
pub const BOOKS: &str = "books";
pub const ASSETS: &str = "assets";
pub const DEBTS: &str = "debts";

/// Teto da cadeia de `mergedInto` (o mesmo de `resolve.ts` do app). Nenhuma fusao produz ciclo,
/// mas as linhas sao eternas e podem trazer um; sem o teto, a janela travaria num laco.
const MAX_HOPS: usize = 8;

/// Segue `merged_into` so em tombstone, ate `MAX_HOPS`; devolve o id onde parou. Em linha viva
/// o campo e dado inconsistente: segui-lo esconderia uma linha que a pessoa ve no celular.
fn follow<'a>(id: Option<&'a str>, redirect: impl Fn(&str) -> Option<&'a str>) -> Option<&'a str> {
    let mut cur = id?;
    for _ in 0..MAX_HOPS {
        match redirect(cur) {
            Some(next) => cur = next,
            None => break,
        }
    }
    Some(cur)
}

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
                    let report_state = f.report_state();
                    let book = f.book();
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
                            report_state,
                            book,
                        },
                    );
                }),
                CATEGORIES => contract::parse_category(&row.data).map(|f| {
                    let book = f.book();
                    self.categories.insert(
                        id.clone(),
                        Category {
                            id: id.clone(),
                            deleted,
                            name: f.name,
                            color: f.color,
                            icon: f.icon,
                            merged_into: f.merged_into,
                            book,
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
                    let book = f.book();
                    self.payment_methods.insert(
                        id.clone(),
                        PaymentMethod {
                            id: id.clone(),
                            deleted,
                            name: f.name,
                            icon: f.icon,
                            merged_into: f.merged_into,
                            book,
                        },
                    );
                }),
                RECURRENCES => contract::parse_recurrence(&row.data).map(|f| {
                    let book = f.book();
                    self.recurrences.insert(
                        id.clone(),
                        Recurrence {
                            id: id.clone(),
                            deleted,
                            frequency: f.frequency,
                            book,
                        },
                    );
                }),
                RESERVES => contract::parse_reserve(&row.data).map(|f| {
                    let recurring_amount_minor = f.recurring_amount_minor();
                    let recurring_since = f.recurring_since().map(str::to_string);
                    let book = f.book();
                    self.reserves.insert(
                        id.clone(),
                        Reserve {
                            id: id.clone(),
                            deleted,
                            kind: f.kind,
                            name: f.name,
                            icon: f.icon,
                            color: f.color,
                            goal_minor: f.goal_minor,
                            multiple: f.multiple,
                            essential_category_ids: f.essential_category_ids.unwrap_or_default(),
                            due_month: f.due_month,
                            recurring_amount_minor,
                            recurring_since,
                            essential_override_minor: f.essential_override_minor,
                            book,
                        },
                    );
                }),
                RESERVE_MOVEMENTS => contract::parse_reserve_movement(&row.data).map(|f| {
                    self.reserve_movements.insert(
                        id.clone(),
                        ReserveMovement {
                            id: id.clone(),
                            deleted,
                            reserve_id: f.reserve_id,
                            kind: f.kind,
                            amount_minor: f.amount_minor,
                            occurred_on: f.occurred_on,
                            user_id: f.user_id,
                            description: f.description,
                            recurring: f.recurring.unwrap_or(false),
                            book: f.book,
                        },
                    );
                }),
                BUDGETS => contract::parse_budget(&row.data).map(|f| {
                    let book = f.book();
                    self.budgets.insert(
                        id.clone(),
                        Budget {
                            id: id.clone(),
                            deleted,
                            category_id: f.category_id,
                            limit_minor: f.limit_minor,
                            updated_at: f.updated_at,
                            book,
                        },
                    );
                }),
                BOOKS => contract::parse_book(&row.data).map(|f| {
                    let kind = f.kind();
                    self.books.insert(
                        id.clone(),
                        BookAccount {
                            id: id.clone(),
                            deleted,
                            kind,
                            name: f.name,
                            color: f.color,
                            icon: f.icon,
                        },
                    );
                }),
                ASSETS => contract::parse_asset(&row.data).map(|f| {
                    let book = f.book();
                    self.assets.insert(
                        id.clone(),
                        Asset {
                            id: id.clone(),
                            deleted,
                            kind: f.kind,
                            name: f.name,
                            value_minor: f.value_minor,
                            acquired_on: f.acquired_on,
                            note: f.note,
                            book,
                        },
                    );
                }),
                DEBTS => contract::parse_debt(&row.data).map(|f| {
                    let book = f.book();
                    self.debts.insert(
                        id.clone(),
                        Debt {
                            id: id.clone(),
                            deleted,
                            name: f.name,
                            creditor: f.creditor,
                            principal_minor: f.principal_minor,
                            balance_minor: f.balance_minor,
                            due_on: f.due_on,
                            book,
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
            RESERVES => {
                self.reserves.remove(id);
            }
            RESERVE_MOVEMENTS => {
                self.reserve_movements.remove(id);
            }
            BUDGETS => {
                self.budgets.remove(id);
            }
            BOOKS => {
                self.books.remove(id);
            }
            ASSETS => {
                self.assets.remove(id);
            }
            DEBTS => {
                self.debts.remove(id);
            }
            _ => {}
        }
    }

    /// Existe linha `books` viva de uma conta comercial: so entao a janela mostra o seletor
    /// Pessoal | Comercial. Derivado das linhas (como no app), nao de flag local: um hub que
    /// recebeu a linha pelo sync ja mostra a conta.
    pub fn has_business_book(&self) -> bool {
        self.books
            .values()
            .any(|b| !b.deleted && b.kind == Book::Business)
    }

    /// Linha `books` viva da conta comercial. Mais de uma (dois aparelhos criaram antes de
    /// sincronizar) nao acontece porque o id e estavel por `kind`; se acontecer, a de menor id
    /// vence para o nome nao oscilar entre quadros.
    pub fn business_book(&self) -> Option<&BookAccount> {
        self.books
            .values()
            .filter(|b| !b.deleted && b.kind == Book::Business)
            .min_by(|a, b| a.id.cmp(&b.id))
    }

    /// A visao de UMA conta: so as linhas dela, e e sobre este `Dataset` que `aggregate`,
    /// `budgets`, `list`, `reserves` e `periods` calculam. Sem isso, lancamento comercial
    /// entraria nos totais pessoais (e o contrario).
    ///
    /// `users`, `books` e `ignored` passam inteiros: o perfil e a pessoa e vale nas duas contas,
    /// `books` liga o seletor, e uma linha fora do contrato nao tem `book` confiavel para ser
    /// atribuida a uma conta. Referencia a categoria ou forma de pagamento de *outra* conta
    /// vira `None` (os rotulos "Sem categoria" / "Sem forma de pagamento"), nunca "removida":
    /// a linha existe, so nao e desta conta, e o app mostra o mesmo porque o estado filtrado
    /// dele tambem nao a tem.
    pub fn for_book(&self, book: Book) -> Dataset {
        let foreign_category = |id: &str| {
            let resolved = self.resolve_category_id(Some(id)).unwrap_or(id);
            [id, resolved]
                .iter()
                .any(|i| self.categories.get(*i).is_some_and(|c| c.book != book))
        };
        let foreign_method = |id: &str| {
            let resolved = self.resolve_payment_method_id(Some(id)).unwrap_or(id);
            [id, resolved]
                .iter()
                .any(|i| self.payment_methods.get(*i).is_some_and(|p| p.book != book))
        };
        fn only<T: Clone>(
            rows: &HashMap<String, T>,
            keep: impl Fn(&T) -> bool,
        ) -> HashMap<String, T> {
            rows.iter()
                .filter(|(_, r)| keep(r))
                .map(|(id, r)| (id.clone(), r.clone()))
                .collect()
        }
        Dataset {
            transactions: self
                .transactions
                .iter()
                .filter(|(_, t)| t.book == book)
                .map(|(id, t)| {
                    let mut t = t.clone();
                    if t.category_id.as_deref().is_some_and(&foreign_category) {
                        t.category_id = None;
                    }
                    if t.payment_method_id.as_deref().is_some_and(&foreign_method) {
                        t.payment_method_id = None;
                    }
                    (id.clone(), t)
                })
                .collect(),
            categories: only(&self.categories, |r| r.book == book),
            users: self.users.clone(),
            payment_methods: only(&self.payment_methods, |r| r.book == book),
            recurrences: only(&self.recurrences, |r| r.book == book),
            reserves: self
                .reserves
                .iter()
                .filter(|(_, r)| r.book == book)
                .map(|(id, r)| {
                    let mut r = r.clone();
                    r.essential_category_ids.retain(|c| !foreign_category(c));
                    (id.clone(), r)
                })
                .collect(),
            reserve_movements: only(&self.reserve_movements, |r| r.book == book),
            budgets: only(&self.budgets, |r| r.book == book),
            books: self.books.clone(),
            assets: only(&self.assets, |r| r.book == book),
            debts: only(&self.debts, |r| r.book == book),
            ignored: self.ignored.clone(),
            loaded_seq: self.loaded_seq,
            refreshed_at: self.refreshed_at,
        }
    }

    /// `deleted_at IS NULL`, igual ao `isAlive` do app.
    pub fn alive_transactions(&self) -> impl Iterator<Item = &Transaction> {
        self.transactions.values().filter(|t| !t.deleted)
    }

    /// A unica porta de resolucao de categoria. O app funde as copias dos padroes em tombstones
    /// com `mergedInto` e nao regrava os lancamentos antigos (resolve na leitura); sem seguir o
    /// campo aqui, eles cairiam em "Categoria removida" enquanto o celular mostra o nome.
    pub fn resolve_category_id<'a>(&'a self, id: Option<&'a str>) -> Option<&'a str> {
        follow(id, |cur| {
            self.categories
                .get(cur)
                .filter(|c| c.deleted)
                .and_then(|c| c.merged_into.as_deref())
        })
    }

    /// Como `resolve_category_id`, para formas de pagamento.
    pub fn resolve_payment_method_id<'a>(&'a self, id: Option<&'a str>) -> Option<&'a str> {
        follow(id, |cur| {
            self.payment_methods
                .get(cur)
                .filter(|p| p.deleted)
                .and_then(|p| p.merged_into.as_deref())
        })
    }

    /// Categoria visivel ou `None` (`findCategory`), depois de seguir `mergedInto`: cor e
    /// icone de registro apagado nao voltam a tela.
    pub fn find_category(&self, id: Option<&str>) -> Option<&Category> {
        self.resolve_category_id(id)
            .and_then(|id| self.categories.get(id))
            .filter(|c| !c.deleted)
    }

    /// Perfil visivel ou `None` (`findUser`). Sem autor e autor apagado colapsam de proposito:
    /// distinguir vazaria "esse perfil existiu" sem ganho nenhum.
    pub fn find_user(&self, id: Option<&str>) -> Option<&User> {
        id.and_then(|id| self.users.get(id)).filter(|u| !u.deleted)
    }

    pub fn find_payment_method(&self, id: Option<&str>) -> Option<&PaymentMethod> {
        self.resolve_payment_method_id(id)
            .and_then(|id| self.payment_methods.get(id))
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

    /// `deleted_at IS NULL`; a escolha de uma meta por categoria e de `budgets`.
    pub fn alive_budgets(&self) -> impl Iterator<Item = &Budget> {
        self.budgets.values().filter(|b| !b.deleted)
    }

    pub fn alive_reserves(&self) -> impl Iterator<Item = &Reserve> {
        self.reserves.values().filter(|r| !r.deleted)
    }

    /// Vivas e de reserva viva: a regra unica de visibilidade das movimentacoes. A de reserva
    /// apagada ou ausente some em vez de virar "Reserva removida" porque o total do app soma
    /// reservas vivas: dinheiro sem destino num total que o app nao mostra faria o hub e o
    /// celular discordarem.
    pub fn alive_reserve_movements(&self) -> impl Iterator<Item = &ReserveMovement> {
        self.reserve_movements
            .values()
            .filter(|m| !m.deleted && self.reserves.get(&m.reserve_id).is_some_and(|r| !r.deleted))
    }

    /// Reservas vivas: o contador da lateral.
    pub fn reserve_count(&self) -> usize {
        self.alive_reserves().count()
    }

    pub fn has_reserves(&self) -> bool {
        self.alive_reserves().next().is_some()
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

/// `data:image/webp;base64,...` -> bytes. Prefixo fora da lista, base64 quebrado ou bytes que
/// nao comecam com a assinatura de PNG, JPEG ou WebP -> `None` (a tela mostra a inicial). A
/// assinatura e conferida aqui porque o iced nao avisa quando uma imagem nao decodifica: sem
/// ela, foto corrompida viraria um circulo vazio em vez da inicial.
pub fn avatar_bytes(data_uri: &str) -> Option<Vec<u8>> {
    let payload = AVATAR_PREFIXES
        .iter()
        .find_map(|prefix| data_uri.strip_prefix(prefix))?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload.trim())
        .ok()?;
    let png = bytes.starts_with(&[0x89, b'P', b'N', b'G']);
    let jpeg = bytes.starts_with(&[0xFF, 0xD8, 0xFF]);
    let webp = bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP";
    (png || jpeg || webp).then_some(bytes)
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

    fn reserve(name: &str) -> Value {
        json!({
            "kind": "goal",
            "name": name,
            "icon": "plane",
            "color": "sky",
            "targetMinor": 500000,
            "essentialCategoryIds": null
        })
    }

    fn movement(reserve_id: &str) -> Value {
        json!({
            "reserveId": reserve_id,
            "amountMinor": 50000,
            "occurredOn": "2026-09-06",
            "userId": "U1",
            "description": null,
            "reason": null,
            "recurring": null
        })
    }

    #[test]
    fn aplica_reserva_e_movimentacao() {
        let mut ds = Dataset::default();
        let report = ds.apply([
            raw(RESERVES, "RE1", None, 3, reserve("Viagem")),
            raw(RESERVE_MOVEMENTS, "M01", None, 4, movement("RE1")),
        ]);
        assert_eq!(
            report,
            ApplyReport {
                applied: 2,
                ignored_total: 0
            }
        );
        assert_eq!(ds.reserves.len(), 1);
        assert_eq!(ds.reserve_movements.len(), 1);
        assert_eq!(ds.loaded_seq, 4);
        assert!(ds.reserves["RE1"].essential_category_ids.is_empty());
        assert!(!ds.reserve_movements["M01"].recurring);
        assert_eq!(ds.alive_reserve_movements().count(), 1);
        // Reserva nao e lancamento: o estado vazio do Dashboard continua.
        assert!(ds.is_empty());
        assert!(ds.has_reserves());
    }

    // Linha exata que o app grava (fixtures.fake.ts): colunas base `id`, `createdAt`,
    // `updatedAt` (HLC) e `deletedAt` junto dos campos da entidade. Foi este formato que
    // antes caia inteiro fora do contrato e sumia do dashboard.
    fn app_goal() -> Value {
        json!({
            "id": "01JZ0000000000000000000RE1",
            "createdAt": "2026-08-07T12:00:00.000Z",
            "updatedAt": "1754568000000-0000-01J9F3K2M7QX8YB4TVWZ0DCEHZ",
            "deletedAt": null,
            "kind": "goal",
            "name": "Viagem",
            "icon": "gift",
            "color": "rose",
            "targetMinor": 500000,
            "multiple": null,
            "essentialCategoryIds": null,
            "essentialOverrideMinor": null,
            "deadline": "2027-07",
            "recurring": { "amountMinor": 30000, "day": 10, "since": "2026-11" }
        })
    }

    #[test]
    fn reserva_como_o_app_grava_passa_inteira() {
        let emergency = json!({
            "id": "01JZ0000000000000000000RE2",
            "createdAt": "2026-08-07T12:00:00.000Z",
            "updatedAt": "1754568000000-0001-01J9F3K2M7QX8YB4TVWZ0DCEHZ",
            "deletedAt": null,
            "kind": "emergency",
            "name": "Reserva de emergência",
            "icon": "lifebuoy",
            "color": "violet",
            "targetMinor": null,
            "multiple": 6,
            "essentialCategoryIds": ["C1", "C2"],
            "essentialOverrideMinor": 350000,
            "deadline": null,
            "recurring": null
        });
        let re1 = "01JZ0000000000000000000RE1";
        let re2 = "01JZ0000000000000000000RE2";
        let mut ds = Dataset::default();
        let report = ds.apply([
            raw(RESERVES, re1, None, 1, app_goal()),
            raw(RESERVES, re2, None, 2, emergency),
        ]);
        assert_eq!(report.ignored_total, 0);
        assert_eq!(report.applied, 2);
        assert!(ds.ignored.is_empty());
        assert_eq!(ds.alive_reserves().count(), 2);
        let g = &ds.reserves[re1];
        assert_eq!(g.name, "Viagem");
        assert_eq!(g.icon, "gift");
        assert_eq!(g.color, "rose");
        assert_eq!(g.goal_minor, Some(500_000));
        assert_eq!(g.due_month.as_deref(), Some("2027-07"));
        assert_eq!(g.recurring_amount_minor, Some(30_000));
        assert_eq!(g.recurring_since.as_deref(), Some("2026-11"));
        assert_eq!(g.essential_override_minor, None);
        let e = &ds.reserves[re2];
        assert_eq!(e.name, "Reserva de emergência");
        assert_eq!(e.icon, "lifebuoy");
        assert_eq!(e.color, "violet");
        assert_eq!(e.essential_override_minor, Some(350_000));
        assert_eq!(e.multiple, Some(6));
        assert_eq!(e.essential_category_ids, vec!["C1", "C2"]);
        assert_eq!(e.recurring_amount_minor, None);
    }

    #[test]
    fn movimentacao_como_o_app_grava_passa_inteira() {
        // `reason` nao e lido pelo hub, mas tem de passar sem derrubar a linha. Nao ha
        // `kind`: o sinal de `amountMinor` e que diz deposito ou retirada.
        let mv = |id: &str,
                  hlc: &str,
                  amount: i64,
                  on: &str,
                  user: Value,
                  desc: Value,
                  reason: Value,
                  rec: bool| {
            json!({
                "id": id,
                "createdAt": "2026-09-06T12:00:00.000Z",
                "updatedAt": hlc,
                "deletedAt": null,
                "reserveId": "01JZ0000000000000000000RE1",
                "amountMinor": amount,
                "occurredOn": on,
                "userId": user,
                "description": desc,
                "reason": reason,
                "recurring": rec
            })
        };
        let us1 = "01JZ0000000000000000000US1";
        let hlc = "1757160000000-0000-01J9F3K2M7QX8YB4TVWZ0DCEHZ";
        let mut ds = Dataset::default();
        let report = ds.apply([
            raw(RESERVES, "01JZ0000000000000000000RE1", None, 1, app_goal()),
            raw(
                RESERVE_MOVEMENTS,
                "01JZ0000000000000000000MV1",
                None,
                2,
                mv(
                    "01JZ0000000000000000000MV1",
                    hlc,
                    50_000,
                    "2026-09-06",
                    json!(us1),
                    Value::Null,
                    Value::Null,
                    true,
                ),
            ),
            raw(
                RESERVE_MOVEMENTS,
                "01JZ0000000000000000000MV2",
                None,
                3,
                mv(
                    "01JZ0000000000000000000MV2",
                    hlc,
                    -15_000,
                    "2026-10-05",
                    Value::Null,
                    Value::Null,
                    json!("health"),
                    false,
                ),
            ),
            raw(
                RESERVE_MOVEMENTS,
                "01JZ0000000000000000000MV3",
                None,
                4,
                mv(
                    "01JZ0000000000000000000MV3",
                    hlc,
                    -35_000,
                    "2026-10-06",
                    Value::Null,
                    json!("Reserva excluída"),
                    json!("other"),
                    false,
                ),
            ),
        ]);
        assert_eq!(report.ignored_total, 0);
        assert_eq!(report.applied, 4);
        assert!(ds.ignored.is_empty());
        assert_eq!(ds.alive_reserve_movements().count(), 3);
        let m1 = &ds.reserve_movements["01JZ0000000000000000000MV1"];
        assert_eq!(m1.kind, MovementKind::Deposit);
        assert_eq!(m1.amount_minor, 50_000);
        assert!(m1.recurring);
        assert_eq!(m1.user_id.as_deref(), Some(us1));
        let m2 = &ds.reserve_movements["01JZ0000000000000000000MV2"];
        assert_eq!(m2.kind, MovementKind::Withdrawal);
        assert_eq!(m2.amount_minor, 15_000);
        assert!(!m2.recurring);
        let m3 = &ds.reserve_movements["01JZ0000000000000000000MV3"];
        assert_eq!(m3.kind, MovementKind::Withdrawal);
        assert_eq!(m3.amount_minor, 35_000);
        assert_eq!(m3.description.as_deref(), Some("Reserva excluída"));
    }

    #[test]
    fn reserva_apagada_fica_marcada_e_fora_das_vivas() {
        let mut ds = Dataset::default();
        ds.apply([raw(RESERVES, "RE1", Some("x"), 1, reserve("Velha"))]);
        assert_eq!(ds.reserves.len(), 1);
        assert_eq!(ds.alive_reserves().count(), 0);
        assert_eq!(ds.reserve_count(), 0);
        assert!(!ds.has_reserves());
        assert!(!Dataset::default().has_reserves());
        ds.apply([raw(RESERVES, "RE2", None, 2, reserve("Nova"))]);
        assert_eq!(ds.reserve_count(), 1);
    }

    #[test]
    fn visibilidade_das_movimentacoes() {
        let mut ds = Dataset::default();
        ds.apply([
            raw(RESERVES, "RV", None, 1, reserve("Viva")),
            raw(RESERVES, "RD", Some("x"), 2, reserve("Apagada")),
            raw(RESERVE_MOVEMENTS, "M1", None, 3, movement("RV")),
            raw(RESERVE_MOVEMENTS, "M2", Some("x"), 4, movement("RV")),
            raw(RESERVE_MOVEMENTS, "M3", None, 5, movement("RD")),
            raw(RESERVE_MOVEMENTS, "M4", None, 6, movement("RX")),
        ]);
        let ids: Vec<&str> = ds
            .alive_reserve_movements()
            .map(|m| m.id.as_str())
            .collect();
        assert_eq!(ids, vec!["M1"]);
        // Movimentacao sem destino e valida: nao conta como fora do contrato.
        assert_eq!(ds.ignored_total(), 0);
    }

    #[test]
    fn movimentacao_invalida_tira_a_valida_e_conta() {
        let mut ds = Dataset::default();
        ds.apply([
            raw(RESERVES, "RE1", None, 1, reserve("Viagem")),
            raw(RESERVE_MOVEMENTS, "M01", None, 2, movement("RE1")),
        ]);
        let mut bad = movement("RE1");
        bad["amountMinor"] = json!("x");
        let report = ds.apply([raw(RESERVE_MOVEMENTS, "M01", None, 3, bad)]);
        assert!(!ds.reserve_movements.contains_key("M01"));
        assert!(ds.ignored[RESERVE_MOVEMENTS].contains("M01"));
        assert_eq!(report.ignored_total, 1);
        ds.apply([raw(RESERVE_MOVEMENTS, "M01", None, 4, movement("RE1"))]);
        assert!(ds.reserve_movements.contains_key("M01"));
        assert!(ds.ignored.is_empty());
    }

    /// `C1` viva; `C6` fundida em `C1`; `C7 -> C6 -> C1`; `C8 <-> C9` em ciclo; `C10` viva com
    /// `mergedInto` (inconsistente); `C11` fundida numa apagada sem destino (`C12`).
    fn merged() -> Dataset {
        let mut ds = Dataset::default();
        let fused = |name: &str, into: &str| json!({ "name": name, "color": "red", "icon": "tag", "mergedInto": into });
        ds.apply([
            raw(CATEGORIES, "C1", None, 1, category("Alimentação")),
            raw(CATEGORIES, "C6", Some("x"), 2, fused("Copia", "C1")),
            raw(CATEGORIES, "C7", Some("x"), 3, fused("Copia velha", "C6")),
            raw(CATEGORIES, "C8", Some("x"), 4, fused("Ciclo A", "C9")),
            raw(CATEGORIES, "C9", Some("x"), 5, fused("Ciclo B", "C8")),
            raw(CATEGORIES, "C10", None, 6, fused("Viva", "C1")),
            raw(CATEGORIES, "C11", Some("x"), 7, fused("Copia", "C12")),
            raw(CATEGORIES, "C12", Some("x"), 8, category("Apagada")),
            raw(
                PAYMENT_METHODS,
                "M1",
                None,
                9,
                json!({ "name": "Pix", "icon": "zap" }),
            ),
            raw(
                PAYMENT_METHODS,
                "M6",
                Some("x"),
                10,
                json!({ "name": "Pix", "icon": "zap", "mergedInto": "M1" }),
            ),
            raw(
                PAYMENT_METHODS,
                "M7",
                Some("x"),
                11,
                json!({ "name": "Cheque", "icon": "receipt", "mergedInto": "M8" }),
            ),
            raw(
                PAYMENT_METHODS,
                "M8",
                Some("x"),
                12,
                json!({ "name": "Cheque", "icon": "receipt" }),
            ),
        ]);
        ds
    }

    #[test]
    fn tombstone_fundida_resolve_para_o_destino() {
        let ds = merged();
        assert_eq!(ds.resolve_category_id(Some("C6")), Some("C1"));
        assert_eq!(
            ds.find_category(Some("C6")).map(|c| c.id.as_str()),
            Some("C1")
        );
        assert_eq!(ds.category_name(Some("C6")), "Alimentação");
    }

    #[test]
    fn cadeia_de_dois_saltos() {
        let ds = merged();
        assert_eq!(ds.resolve_category_id(Some("C7")), Some("C1"));
        assert_eq!(ds.category_name(Some("C7")), "Alimentação");
    }

    #[test]
    fn ciclo_termina_no_teto_de_saltos() {
        let ds = merged();
        // MAX_HOPS par: oito saltos a partir de C8 voltam para C8.
        assert_eq!(ds.resolve_category_id(Some("C8")), Some("C8"));
        assert_eq!(ds.find_category(Some("C8")), None);
        assert_eq!(ds.category_name(Some("C9")), "Categoria removida");
    }

    #[test]
    fn merged_into_em_linha_viva_e_ignorado() {
        let ds = merged();
        assert_eq!(ds.resolve_category_id(Some("C10")), Some("C10"));
        assert_eq!(ds.category_name(Some("C10")), "Viva");
    }

    #[test]
    fn destino_apagado_e_categoria_removida() {
        let ds = merged();
        assert_eq!(ds.resolve_category_id(Some("C11")), Some("C12"));
        assert_eq!(ds.find_category(Some("C11")), None);
        assert_eq!(ds.category_name(Some("C11")), "Categoria removida");
        assert_eq!(ds.resolve_category_id(None), None);
        assert_eq!(ds.category_name(None), "Sem categoria");
        // Id que nao existe fica como esta.
        assert_eq!(ds.resolve_category_id(Some("ausente")), Some("ausente"));
    }

    #[test]
    fn forma_de_pagamento_fundida() {
        let ds = merged();
        assert_eq!(ds.resolve_payment_method_id(Some("M6")), Some("M1"));
        assert_eq!(
            ds.find_payment_method(Some("M6")).map(|p| p.id.as_str()),
            Some("M1")
        );
        assert_eq!(ds.payment_method_name(Some("M6")), "Pix");
        assert_eq!(ds.find_payment_method(Some("M7")), None);
        assert_eq!(ds.payment_method_name(Some("M7")), "Forma removida");
        assert_eq!(ds.resolve_payment_method_id(None), None);
        assert_eq!(ds.payment_method_name(None), "Sem forma de pagamento");
    }

    #[test]
    fn merged_into_com_tipo_errado_e_ignorada() {
        let mut ds = Dataset::default();
        let report = ds.apply([raw(
            CATEGORIES,
            "C6",
            Some("x"),
            1,
            json!({ "name": "Copia", "color": "red", "icon": "tag", "mergedInto": 5 }),
        )]);
        assert_eq!(report.ignored_total, 1);
        assert!(ds.ignored[CATEGORIES].contains("C6"));
        assert!(!ds.categories.contains_key("C6"));
    }

    // ---- conta pessoal x comercial ----

    use crate::dashboard::aggregate::{expense_by_category, month_summary};

    const BIZ_C: &str = "01HZZZZZZZZZZZZZZZZZZZZZC9";
    const BIZ_M: &str = "01HZZZZZZZZZZZZZZZZZZZZZM9";

    fn biz(mut v: Value) -> Value {
        v["book"] = json!("business");
        v
    }

    fn tx_in(kind: &str, amount: i64, category: Option<&str>, method: Option<&str>) -> Value {
        json!({
            "kind": kind, "description": "x", "amountMinor": amount,
            "occurredOn": "2026-09-14", "categoryId": category, "paymentMethodId": method
        })
    }

    /// Duas contas povoadas: pessoal (receita 5.000, despesa 1.000) e comercial (receita 90.000,
    /// despesa 30.000), cada uma com a sua categoria e forma de pagamento.
    fn two_books() -> Dataset {
        let mut ds = Dataset::default();
        ds.apply([
            raw(CATEGORIES, C1, None, 1, category("Mercado")),
            raw(CATEGORIES, BIZ_C, None, 2, biz(category("Vendas"))),
            raw(
                PAYMENT_METHODS,
                "01HZZZZZZZZZZZZZZZZZZZZZM1",
                None,
                3,
                json!({ "name": "Pix", "icon": "zap" }),
            ),
            raw(
                PAYMENT_METHODS,
                BIZ_M,
                None,
                4,
                biz(json!({ "name": "Maquininha", "icon": "card" })),
            ),
            raw(
                TRANSACTIONS,
                "T01",
                None,
                5,
                tx_in("income", 5_000, Some(C1), None),
            ),
            raw(
                TRANSACTIONS,
                "T02",
                None,
                6,
                tx_in("expense", 1_000, Some(C1), None),
            ),
            raw(
                TRANSACTIONS,
                "T03",
                None,
                7,
                biz(tx_in("income", 90_000, Some(BIZ_C), Some(BIZ_M))),
            ),
            raw(
                TRANSACTIONS,
                "T04",
                None,
                8,
                biz(tx_in("expense", 30_000, Some(BIZ_C), None)),
            ),
            raw(USERS, "01HZZZZZZZZZZZZZZZZZZZZZP1", None, 9, user("Ana")),
        ]);
        ds
    }

    #[test]
    fn book_da_linha_vem_do_contrato_e_degrada_para_pessoal() {
        let mut ds = Dataset::default();
        let mut future = tx("a");
        future["book"] = json!("filial");
        ds.apply([
            raw(TRANSACTIONS, "A", None, 1, tx("a")),
            raw(TRANSACTIONS, "B", None, 2, biz(tx("b"))),
            raw(TRANSACTIONS, "C", None, 3, future),
        ]);
        assert_eq!(ds.transactions["A"].book, Book::Personal);
        assert_eq!(ds.transactions["B"].book, Book::Business);
        assert_eq!(ds.transactions["C"].book, Book::Personal);
    }

    #[test]
    fn totais_pessoais_nao_mudam_com_linhas_comerciais_presentes() {
        let both = two_books();
        let mut only_personal = Dataset::default();
        only_personal.apply([
            raw(CATEGORIES, C1, None, 1, category("Mercado")),
            raw(
                TRANSACTIONS,
                "T01",
                None,
                5,
                tx_in("income", 5_000, Some(C1), None),
            ),
            raw(
                TRANSACTIONS,
                "T02",
                None,
                6,
                tx_in("expense", 1_000, Some(C1), None),
            ),
        ]);
        let with_business = month_summary(&both.for_book(Book::Personal), "2026-09");
        let without = month_summary(&only_personal, "2026-09");
        assert_eq!(with_business, without);
        assert_eq!(with_business.current.income_minor, 5_000);
        assert_eq!(with_business.current.expense_minor, 1_000);
    }

    #[test]
    fn totais_comerciais_so_somam_business() {
        let view = two_books().for_book(Book::Business);
        let s = month_summary(&view, "2026-09");
        assert_eq!(s.current.income_minor, 90_000);
        assert_eq!(s.current.expense_minor, 30_000);
        assert_eq!(view.alive_transactions().count(), 2);
        assert_eq!(view.categories.len(), 1);
        assert_eq!(view.payment_methods.len(), 1);
    }

    #[test]
    fn linha_sem_book_e_pessoal_na_visao_pessoal() {
        let ds = two_books();
        assert_eq!(ds.for_book(Book::Personal).alive_transactions().count(), 2);
        assert!(ds.for_book(Book::Personal).categories.contains_key(C1));
        assert!(!ds.for_book(Book::Business).categories.contains_key(C1));
    }

    #[test]
    fn referencia_de_outra_conta_resolve_para_sem_categoria() {
        let mut ds = two_books();
        ds.apply([
            // Lancamento comercial apontando para categoria e forma pessoais.
            raw(
                TRANSACTIONS,
                "X1",
                None,
                10,
                biz(tx_in(
                    "expense",
                    700,
                    Some(C1),
                    Some("01HZZZZZZZZZZZZZZZZZZZZZM1"),
                )),
            ),
            // E o contrario.
            raw(
                TRANSACTIONS,
                "X2",
                None,
                11,
                tx_in("expense", 800, Some(BIZ_C), Some(BIZ_M)),
            ),
        ]);
        let business = ds.for_book(Book::Business);
        let x1 = &business.transactions["X1"];
        assert_eq!(
            business.category_name(x1.category_id.as_deref()),
            "Sem categoria"
        );
        assert_eq!(
            business.payment_method_name(x1.payment_method_id.as_deref()),
            "Sem forma de pagamento"
        );
        let slices = expense_by_category(&business, "2026-09");
        let sem = slices.iter().find(|s| s.name == "Sem categoria").unwrap();
        assert_eq!(sem.amount_minor, 700);

        let personal = ds.for_book(Book::Personal);
        let x2 = &personal.transactions["X2"];
        assert_eq!(
            personal.category_name(x2.category_id.as_deref()),
            "Sem categoria"
        );
        assert_eq!(
            personal.payment_method_name(x2.payment_method_id.as_deref()),
            "Sem forma de pagamento"
        );
        // Referencia morta da propria conta continua "removida": so a de outra conta e "sem".
        assert_eq!(
            personal.category_name(Some("ausente")),
            "Categoria removida"
        );
    }

    #[test]
    fn perfil_e_compartilhado_e_linha_ignorada_nao_some_da_visao() {
        let mut ds = two_books();
        let mut bad = tx("ruim");
        bad["amountMinor"] = json!("x");
        ds.apply([raw(TRANSACTIONS, "BAD", None, 20, biz(bad))]);
        for book in [Book::Personal, Book::Business] {
            let view = ds.for_book(book);
            assert_eq!(view.alive_users().len(), 1);
            assert_eq!(view.ignored_total(), 1);
            assert_eq!(view.loaded_seq, 20);
        }
    }

    #[test]
    fn reservas_e_movimentos_ficam_na_propria_conta() {
        let mut ds = Dataset::default();
        let reserve = json!({ "kind": "goal", "name": "Caixa", "icon": "i", "color": "c" });
        let movement = |reserve_id: &str| json!({ "reserveId": reserve_id, "amountMinor": 500, "occurredOn": "2026-09-01" });
        ds.apply([
            raw(RESERVES, "R1", None, 1, reserve.clone()),
            raw(RESERVES, "R2", None, 2, biz(reserve)),
            raw(RESERVE_MOVEMENTS, "M1", None, 3, movement("R1")),
            raw(RESERVE_MOVEMENTS, "M2", None, 4, biz(movement("R2"))),
        ]);
        let personal = ds.for_book(Book::Personal);
        assert_eq!(personal.alive_reserves().count(), 1);
        assert_eq!(personal.alive_reserve_movements().count(), 1);
        let business = ds.for_book(Book::Business);
        assert_eq!(business.alive_reserves().count(), 1);
        assert_eq!(business.alive_reserve_movements().count(), 1);
        assert!(business.reserves.contains_key("R2"));
    }

    #[test]
    fn conta_comercial_so_existe_com_linha_books_viva() {
        let mut ds = Dataset::default();
        assert!(!ds.has_business_book());
        let book = json!({ "kind": "business", "name": "Padaria", "color": "amber", "icon": "s" });
        ds.apply([raw(BOOKS, "B1", Some("x"), 1, book.clone())]);
        assert!(!ds.has_business_book());
        ds.apply([raw(BOOKS, "B1", None, 2, book)]);
        assert!(ds.has_business_book());
        assert_eq!(ds.business_book().map(|b| b.name.as_str()), Some("Padaria"));
        // `kind` de versao futura nao abre o seletor.
        let mut other = Dataset::default();
        other.apply([raw(
            BOOKS,
            "B2",
            None,
            1,
            json!({ "kind": "filial", "name": "F", "color": "c", "icon": "i" }),
        )]);
        assert!(!other.has_business_book());
        // A visao carrega `books`: o seletor nao some ao trocar de conta.
        assert!(ds.for_book(Book::Business).has_business_book());
        assert!(ds.for_book(Book::Personal).has_business_book());
    }

    #[test]
    fn bens_e_dividas_entram_e_linha_fora_do_contrato_e_contada() {
        let mut ds = Dataset::default();
        let asset = |kind: &str| {
            json!({
                "kind": kind, "name": "n", "valueMinor": 1000,
                "acquiredOn": "2026-09-01", "book": "business"
            })
        };
        let report = ds.apply([
            raw(ASSETS, "A1", None, 1, asset("equipment")),
            raw(ASSETS, "A2", None, 2, asset("imovel")),
            raw(
                DEBTS,
                "D1",
                None,
                3,
                json!({ "name": "d", "principalMinor": 10, "balanceMinor": 5, "book": "business" }),
            ),
            raw(
                DEBTS,
                "D2",
                None,
                4,
                json!({ "name": "d", "principalMinor": 10, "balanceMinor": "5" }),
            ),
            raw(BOOKS, "B1", None, 5, json!({ "kind": "business" })),
        ]);
        assert_eq!(report.applied, 5);
        assert_eq!(ds.assets.len(), 1);
        assert_eq!(ds.debts.len(), 1);
        assert!(ds.books.is_empty());
        assert_eq!(report.ignored_total, 3);
        assert!(ds.ignored[ASSETS].contains("A2"));
        assert!(ds.ignored[DEBTS].contains("D2"));
        assert!(ds.ignored[BOOKS].contains("B1"));
        // Bens e dividas nao sao lancamentos: nao movem total nenhum.
        assert!(ds.is_empty());
        let business = ds.for_book(Book::Business);
        assert_eq!((business.assets.len(), business.debts.len()), (1, 1));
        let personal = ds.for_book(Book::Personal);
        assert_eq!((personal.assets.len(), personal.debts.len()), (0, 0));
    }

    #[test]
    fn avatar_decodifica_so_os_prefixos_do_app() {
        let png = avatar_bytes("data:image/png;base64,iVBORw0KGgo=").unwrap();
        assert_eq!(&png[..4], &[0x89, b'P', b'N', b'G']);
        assert!(avatar_bytes("data:image/jpeg;base64,/9j/4AAQ").is_some());
        assert!(avatar_bytes("data:image/webp;base64,UklGRgAAAABXRUJQ").is_some());
        // Base64 valido mas sem assinatura de imagem: sem foto.
        assert_eq!(avatar_bytes("data:image/png;base64,AAAA"), None);
        assert_eq!(avatar_bytes("data:image/webp;base64,UklGRg=="), None);
        assert_eq!(avatar_bytes("data:image/svg+xml;base64,PHN2Zz4="), None);
        assert_eq!(avatar_bytes("data:image/png;base64,%%%nao-e-base64"), None);
        assert_eq!(avatar_bytes("https://exemplo/foto.png"), None);
    }
}
