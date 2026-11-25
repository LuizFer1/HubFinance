//! Historico do assistente em `assistant.db`, separado do banco de sync: conversa sobre dinheiro
//! nao e linha sincronizada, nao ganha `seq`, nao vai a celular nenhum, e "apagar dados de
//! aparelho" nao a alcanca. Unico modulo com rusqlite fora de `store/`; nunca abre o banco dele.

use std::path::Path;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension, params};

use super::view::{ConversationSummary, MessageRole, MessageView, OpenConversation};
use crate::dashboard::contract::Book;

pub const DB_FILE: &str = "assistant.db";
const SCHEMA_VERSION: i32 = 1;

/// `seq` (e nao `created_at`) manda na ordem: `created_at` e texto de relogio de parede, que
/// pode andar para tras (ajuste de hora) e e igual para duas mensagens no mesmo milissegundo;
/// com ele a pergunta poderia aparecer depois da resposta. `created_at` fica so para exibir.
const SCHEMA_V1: &str = "
CREATE TABLE conversations (
  seq INTEGER PRIMARY KEY AUTOINCREMENT,
  id TEXT NOT NULL UNIQUE,
  book TEXT NOT NULL,
  created_at TEXT NOT NULL,
  envelope TEXT NOT NULL,
  title TEXT NOT NULL
);
CREATE TABLE messages (
  seq INTEGER PRIMARY KEY AUTOINCREMENT,
  id TEXT NOT NULL UNIQUE,
  conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
  role TEXT NOT NULL,
  content TEXT NOT NULL,
  sent_to_cloud INTEGER NOT NULL DEFAULT 0,
  deny_data_collection INTEGER,
  provider TEXT,
  model TEXT,
  tokens_in INTEGER,
  tokens_out INTEGER,
  interrupted INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL
);
CREATE INDEX messages_by_conversation ON messages (conversation_id, seq);
";

#[derive(Debug, thiserror::Error)]
pub enum HistoryError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

pub struct History {
    conn: Mutex<Connection>,
}

fn book_str(b: Book) -> &'static str {
    match b {
        Book::Personal => "personal",
        Book::Business => "business",
    }
}

fn book_of(s: &str) -> Book {
    if s == "business" {
        Book::Business
    } else {
        Book::Personal
    }
}

fn role_str(r: MessageRole) -> &'static str {
    match r {
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::Error => "error",
    }
}

/// Valor desconhecido (banco de uma versao futura) degrada para erro em vez de derrubar a
/// leitura: aparece como mensagem de erro e nunca vai ao modelo.
fn role_of(s: &str) -> MessageRole {
    match s {
        "user" => MessageRole::User,
        "assistant" => MessageRole::Assistant,
        _ => MessageRole::Error,
    }
}

/// Guardado por `user_version`: reabrir um banco ja migrado nao reexecuta o DDL. Versao nova =
/// mais um bloco `if version < N`, cada um na sua transacao.
fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    let version: i32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version < 1 {
        conn.execute_batch(&format!(
            "BEGIN;\n{SCHEMA_V1}\nPRAGMA user_version = {SCHEMA_VERSION};\nCOMMIT;"
        ))?;
    }
    Ok(())
}

fn insert_conversation(
    c: &Connection,
    id: &str,
    book: Book,
    at: &str,
    envelope: &str,
    title: &str,
) -> rusqlite::Result<()> {
    c.execute(
        "INSERT INTO conversations (id, book, created_at, envelope, title)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, book_str(book), at, envelope, title],
    )
    .map(|_| ())
}

fn insert_message(
    c: &Connection,
    conversation: &str,
    m: &MessageView,
    at: &str,
) -> rusqlite::Result<()> {
    c.execute(
        "INSERT INTO messages (id, conversation_id, role, content, sent_to_cloud,
           deny_data_collection, provider, model, tokens_in, tokens_out, interrupted, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            m.id,
            conversation,
            role_str(m.role),
            m.content,
            m.sent_to_cloud,
            m.deny_data_collection,
            m.provider,
            m.model,
            m.tokens_in,
            m.tokens_out,
            m.interrupted,
            at
        ],
    )
    .map(|_| ())
}

/// Depois de apagar: no WAL o texto apagado ainda estaria nos quadros antigos (e `secure_delete`
/// so zera o arquivo principal). O checkpoint com TRUNCATE leva tudo para o arquivo principal e
/// esvazia o `-wal`. Se algum leitor impedir (`busy`), o texto ainda pode estar no WAL: avisa.
fn purge(c: &Connection) -> rusqlite::Result<()> {
    let busy: i64 = c.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| r.get(0))?;
    if busy != 0 {
        tracing::warn!("o historico do assistente nao pode ser compactado agora (banco ocupado)");
    }
    Ok(())
}

impl History {
    pub fn open(dir: &Path) -> Result<History, HistoryError> {
        std::fs::create_dir_all(dir)?;
        let conn = Connection::open(dir.join(DB_FILE))?;
        // Sem `foreign_keys` o `ON DELETE CASCADE` e ignorado e apagar uma conversa deixaria as
        // mensagens (com o texto da conversa) no arquivo.
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let _mode: String =
            conn.pragma_update_and_check(None, "journal_mode", "WAL", |r| r.get(0))?;
        // O usuario apaga conversa sobre dinheiro esperando que ela suma. Sem `secure_delete` o
        // SQLite so marca as paginas como livres e o texto continua no arquivo.
        conn.pragma_update(None, "secure_delete", "ON")?;
        // NORMAL e seguro sob WAL (so uma queda de energia pode perder o ultimo commit, nunca
        // corromper) e evita um fsync por mensagem; o historico nao justifica o custo do FULL.
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.busy_timeout(Duration::from_secs(5))?;
        migrate(&conn)?;
        Ok(History {
            conn: Mutex::new(conn),
        })
    }

    fn with<T>(
        &self,
        f: impl FnOnce(&Connection) -> rusqlite::Result<T>,
    ) -> Result<T, HistoryError> {
        // Um panic noutra chamada nao invalida o banco: SQLite so confirma transacoes inteiras.
        let conn = self.conn.lock().unwrap_or_else(PoisonError::into_inner);
        Ok(f(&conn)?)
    }

    /// A conversa e a pergunta de abertura entram juntas ou nenhuma entra: uma conversa sem a
    /// abertura apareceria na lista, vazia, depois de reabrir o hub.
    pub fn create_with_opening(
        &self,
        id: &str,
        book: Book,
        at: &str,
        envelope: &str,
        title: &str,
        opening: &MessageView,
    ) -> Result<(), HistoryError> {
        self.with(|c| {
            let tx = c.unchecked_transaction()?;
            insert_conversation(&tx, id, book, at, envelope, title)?;
            insert_message(&tx, id, opening, at)?;
            tx.commit()
        })
    }

    #[cfg(test)]
    pub fn create(
        &self,
        id: &str,
        book: Book,
        at: &str,
        envelope: &str,
        title: &str,
    ) -> Result<(), HistoryError> {
        self.with(|c| insert_conversation(c, id, book, at, envelope, title))
    }

    pub fn append(
        &self,
        conversation: &str,
        m: &MessageView,
        at: &str,
    ) -> Result<(), HistoryError> {
        self.with(|c| insert_message(c, conversation, m, at))
    }

    /// Remove uma mensagem (a de erro, em "Tentar de novo").
    pub fn delete_message(&self, id: &str) -> Result<(), HistoryError> {
        self.with(|c| {
            c.execute("DELETE FROM messages WHERE id = ?1", params![id])?;
            purge(c)
        })
    }

    /// Mais nova primeiro.
    pub fn list(&self) -> Result<Vec<ConversationSummary>, HistoryError> {
        self.with(|c| {
            let mut st = c.prepare(
                "SELECT id, book, title, created_at FROM conversations ORDER BY seq DESC",
            )?;
            let rows = st.query_map([], |r| {
                Ok(ConversationSummary {
                    id: r.get(0)?,
                    book: book_of(&r.get::<_, String>(1)?),
                    title: r.get(2)?,
                    created_at: r.get(3)?,
                })
            })?;
            rows.collect()
        })
    }

    /// `needs_cloud_consent` volta a `true` a menos que a conversa ja tenha saido para a nuvem:
    /// a aprovacao da previa e do estado da tela, nao do disco, e perguntar de novo depois de
    /// reabrir o hub e o lado seguro (a pessoa pode ter trocado de provedor nesse meio tempo).
    pub fn load(&self, id: &str) -> Result<Option<OpenConversation>, HistoryError> {
        self.with(|c| {
            let head: Option<(String, String)> = c
                .query_row(
                    "SELECT book, envelope FROM conversations WHERE id = ?1",
                    params![id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            let Some((book, envelope)) = head else {
                return Ok(None);
            };
            let mut st = c.prepare(
                "SELECT id, role, content, sent_to_cloud, deny_data_collection, provider, model,
                        tokens_in, tokens_out, interrupted
                 FROM messages WHERE conversation_id = ?1 ORDER BY seq",
            )?;
            let messages = st
                .query_map(params![id], |r| {
                    Ok(MessageView {
                        id: r.get(0)?,
                        role: role_of(&r.get::<_, String>(1)?),
                        content: r.get(2)?,
                        sent_to_cloud: r.get(3)?,
                        deny_data_collection: r.get(4)?,
                        provider: r.get(5)?,
                        model: r.get(6)?,
                        tokens_in: r.get(7)?,
                        tokens_out: r.get(8)?,
                        interrupted: r.get(9)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let needs_cloud_consent = !messages.iter().any(|m| m.sent_to_cloud);
            Ok(Some(OpenConversation {
                id: id.to_string(),
                book: book_of(&book),
                envelope,
                messages,
                needs_cloud_consent,
            }))
        })
    }

    pub fn delete(&self, id: &str) -> Result<(), HistoryError> {
        self.with(|c| {
            c.execute("DELETE FROM conversations WHERE id = ?1", params![id])?;
            purge(c)
        })
    }

    pub fn delete_all(&self) -> Result<(), HistoryError> {
        self.with(|c| {
            c.execute_batch("DELETE FROM conversations;")?;
            purge(c)
        })
    }

    #[cfg(test)]
    pub fn message_count(&self) -> Result<i64, HistoryError> {
        self.with(|c| c.query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(id: &str, role: MessageRole, s: &str) -> MessageView {
        MessageView {
            id: id.into(),
            role,
            content: s.into(),
            sent_to_cloud: false,
            deny_data_collection: None,
            provider: None,
            model: None,
            tokens_in: Some(10),
            tokens_out: Some(2),
            interrupted: false,
        }
    }

    #[test]
    fn grava_e_reabre_com_o_envelope_congelado() {
        let dir = tempfile::tempdir().unwrap();
        let h = History::open(dir.path()).unwrap();
        h.create(
            "C1",
            Book::Personal,
            "2026-11-25T10:00:00.000Z",
            "ENV-ANTIGO",
            "Análise",
        )
        .unwrap();
        h.append(
            "C1",
            &msg("M1", MessageRole::User, "oi"),
            "2026-11-25T10:00:01.000Z",
        )
        .unwrap();
        h.append(
            "C1",
            &msg("M2", MessageRole::Assistant, "olá"),
            "2026-11-25T10:00:02.000Z",
        )
        .unwrap();
        drop(h);
        let h = History::open(dir.path()).unwrap();
        let list = h.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title, "Análise");
        assert_eq!(list[0].created_at, "2026-11-25T10:00:00.000Z");
        let c = h.load("C1").unwrap().unwrap();
        assert_eq!(c.envelope, "ENV-ANTIGO");
        assert_eq!(c.messages.len(), 2);
        assert_eq!(c.messages[1].tokens_in, Some(10));
        assert!(c.needs_cloud_consent);
    }

    #[test]
    fn apagar_uma_leva_as_mensagens_e_apagar_todas_zera() {
        let dir = tempfile::tempdir().unwrap();
        let h = History::open(dir.path()).unwrap();
        h.create("C1", Book::Personal, "t", "E", "a").unwrap();
        h.create("C2", Book::Business, "t", "E", "b").unwrap();
        h.append("C1", &msg("M1", MessageRole::User, "x"), "t")
            .unwrap();
        h.append("C2", &msg("M2", MessageRole::User, "y"), "t")
            .unwrap();
        h.delete("C1").unwrap();
        assert!(h.load("C1").unwrap().is_none());
        assert_eq!(h.message_count().unwrap(), 1, "so a de C2 sobra");
        h.delete_all().unwrap();
        assert!(h.list().unwrap().is_empty());
        assert_eq!(h.message_count().unwrap(), 0);
    }

    #[test]
    fn nao_toca_no_banco_de_sync() {
        let dir = tempfile::tempdir().unwrap();
        let h = History::open(dir.path()).unwrap();
        h.create("C1", Book::Personal, "t", "E", "a").unwrap();
        h.append("C1", &msg("M1", MessageRole::User, "x"), "t")
            .unwrap();
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(!names.is_empty());
        assert!(names.iter().all(|n| n.starts_with(DB_FILE)), "{names:?}");
    }

    #[test]
    fn ordem_vale_a_da_gravacao_mesmo_com_relogio_igual_ou_para_tras() {
        let dir = tempfile::tempdir().unwrap();
        let h = History::open(dir.path()).unwrap();
        // Ids em ordem contraria a da gravacao e horas iguais ou decrescentes.
        h.create(
            "Z",
            Book::Personal,
            "2026-11-25T10:00:00.000Z",
            "E",
            "primeira",
        )
        .unwrap();
        h.create(
            "A",
            Book::Personal,
            "2026-11-25T09:00:00.000Z",
            "E",
            "segunda",
        )
        .unwrap();
        let same = "2026-11-25T10:00:05.000Z";
        h.append("A", &msg("Y", MessageRole::User, "p"), same)
            .unwrap();
        h.append("A", &msg("B", MessageRole::Assistant, "r"), same)
            .unwrap();
        h.append(
            "A",
            &msg("C", MessageRole::User, "q"),
            "2026-11-25T09:59:00.000Z",
        )
        .unwrap();
        let titles: Vec<_> = h.list().unwrap().into_iter().map(|c| c.title).collect();
        assert_eq!(titles, ["segunda", "primeira"], "mais nova primeiro");
        let ids: Vec<_> = h
            .load("A")
            .unwrap()
            .unwrap()
            .messages
            .into_iter()
            .map(|m| m.id)
            .collect();
        assert_eq!(ids, ["Y", "B", "C"]);
    }

    #[test]
    fn guarda_selo_e_politica_e_o_selo_dispensa_nova_aprovacao() {
        let dir = tempfile::tempdir().unwrap();
        let h = History::open(dir.path()).unwrap();
        h.create("C1", Book::Business, "t", "E", "a").unwrap();
        let mut m = msg("M1", MessageRole::Assistant, "ok");
        m.sent_to_cloud = true;
        m.deny_data_collection = Some(true);
        m.provider = Some("cloud".into());
        m.model = Some("openai/gpt-4o".into());
        m.interrupted = true;
        h.append("C1", &m, "t").unwrap();
        let c = h.load("C1").unwrap().unwrap();
        assert_eq!(c.book, Book::Business);
        assert_eq!(c.messages[0], m);
        assert!(!c.needs_cloud_consent);
    }

    #[test]
    fn apagar_mensagem_tira_so_ela_e_conversa_inexistente_nao_existe() {
        let dir = tempfile::tempdir().unwrap();
        let h = History::open(dir.path()).unwrap();
        h.create("C1", Book::Personal, "t", "E", "a").unwrap();
        h.append("C1", &msg("M1", MessageRole::User, "x"), "t")
            .unwrap();
        h.append("C1", &msg("M2", MessageRole::Error, "falhou"), "t")
            .unwrap();
        h.delete_message("M2").unwrap();
        assert_eq!(h.load("C1").unwrap().unwrap().messages.len(), 1);
        assert!(h.load("nao-existe").unwrap().is_none());
        // Sem a conversa, a chave estrangeira recusa a mensagem.
        assert!(
            h.append("nao-existe", &msg("M3", MessageRole::User, "x"), "t")
                .is_err()
        );
    }

    #[test]
    fn banco_novo_nasce_na_versao_atual_e_reabrir_nao_reexecuta_a_migracao() {
        let dir = tempfile::tempdir().unwrap();
        drop(History::open(dir.path()).unwrap());
        let h = History::open(dir.path()).unwrap();
        let v = h
            .with(|c| c.query_row("PRAGMA user_version", [], |r| r.get::<_, i32>(0)))
            .unwrap();
        assert_eq!(v, SCHEMA_VERSION);
    }

    #[test]
    fn abrir_em_caminho_que_e_arquivo_da_erro_de_io() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("arquivo");
        std::fs::write(&file, "x").unwrap();
        assert!(matches!(History::open(&file), Err(HistoryError::Io(_))));
    }

    #[test]
    fn conversa_e_abertura_entram_juntas_ou_nenhuma_entra() {
        let dir = tempfile::tempdir().unwrap();
        let h = History::open(dir.path()).unwrap();
        h.create_with_opening(
            "C0",
            Book::Personal,
            "t",
            "E",
            "a",
            &msg("M1", MessageRole::User, "abertura"),
        )
        .unwrap();
        assert_eq!(h.load("C0").unwrap().unwrap().messages.len(), 1);
        // Id de mensagem repetido: a insercao da abertura falha depois de a conversa entrar.
        let err = h.create_with_opening(
            "C1",
            Book::Personal,
            "t",
            "E",
            "b",
            &msg("M1", MessageRole::User, "abertura"),
        );
        assert!(err.is_err());
        assert!(
            h.load("C1").unwrap().is_none(),
            "a conversa nao ficou sem abertura"
        );
        assert_eq!(h.list().unwrap().len(), 1);
        assert_eq!(h.message_count().unwrap(), 1);
    }

    /// Texto bruto de `assistant.db` e do `-wal` (se existir).
    fn raw_bytes(dir: &Path) -> Vec<u8> {
        let mut all = std::fs::read(dir.join(DB_FILE)).unwrap();
        if let Ok(wal) = std::fs::read(dir.join(format!("{DB_FILE}-wal"))) {
            all.extend(wal);
        }
        all
    }

    fn contains(hay: &[u8], needle: &str) -> bool {
        hay.windows(needle.len()).any(|w| w == needle.as_bytes())
    }

    #[test]
    fn apagar_apaga_de_verdade_do_db_e_do_wal() {
        let secret = "SENTINELA-UNICA-7f3a9c";
        for how in ["uma", "todas", "mensagem"] {
            let dir = tempfile::tempdir().unwrap();
            let h = History::open(dir.path()).unwrap();
            h.create_with_opening(
                "C1",
                Book::Personal,
                "t",
                &format!("envelope {secret}"),
                &format!("titulo {secret}"),
                &msg("M1", MessageRole::User, &format!("pergunta {secret}")),
            )
            .unwrap();
            h.append(
                "C1",
                &msg("M2", MessageRole::Error, &format!("erro {secret}")),
                "t",
            )
            .unwrap();
            // Controle: antes de apagar, o texto esta mesmo no arquivo (senao o teste nao prova nada).
            assert!(contains(&raw_bytes(dir.path()), secret));
            match how {
                "uma" => h.delete("C1").unwrap(),
                "todas" => h.delete_all().unwrap(),
                _ => {
                    h.delete_message("M2").unwrap();
                    h.delete_message("M1").unwrap();
                    h.delete("C1").unwrap();
                }
            }
            assert!(
                !contains(&raw_bytes(dir.path()), secret),
                "{how}: o texto apagado continua no arquivo"
            );
        }
    }
}
