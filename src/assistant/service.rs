//! Orquestra o assistente no nucleo do hub: guarda configuracao e conversas, prepara o pedido
//! ao modelo e grava o resultado. Nao faz I/O de rede: o hub roda `Job` num `spawn_blocking`
//! com o provedor e devolve o resultado em `finish`. Assim o fluxo inteiro e testavel com o
//! `FakeProvider`.

use std::path::{Path, PathBuf};

use super::AssistantError;
use super::client::OpenAiCompatible;
use super::prompt::{self, MAX_OUTPUT_TOKENS, OPENING_REQUEST, Role, Turn};
use super::provider::{CancelFlag, ChatOutcome, ChatRequest, Provider};
use super::secret::Secret;
use super::settings::Settings;
use super::view::{AssistantView, ConversationSummary, MessageRole, MessageView, OpenConversation};
use crate::dashboard::contract::Book;
use crate::desktop::secret::SecretStore;

pub struct Job {
    pub request: ChatRequest,
    pub provider: Box<dyn Provider>,
    pub cancel: CancelFlag,
    pub reply_id: String,
}

/// A resposta em andamento. Guardar a conversa e o destino **no envio** (e nao reler ao
/// terminar) evita que a resposta caia na conversa errada se o usuario abrir outra enquanto o
/// modelo escreve, e que `sent_to_cloud` mude de valor se a configuracao for trocada no meio.
struct Inflight {
    conversation_id: String,
    cancel: CancelFlag,
    to_cloud: bool,
}

pub struct AssistantService {
    dir: PathBuf,
    secrets: Box<dyn SecretStore>,
    view: AssistantView,
    inflight: Option<Inflight>,
    /// Conversas fechadas desta sessao (Issue 06 troca por `history`).
    stash: Vec<OpenConversation>,
}

fn new_id() -> String {
    ulid::Ulid::generate().to_string()
}

impl AssistantService {
    pub fn open(dir: &Path, secrets: Box<dyn SecretStore>) -> Self {
        let settings = Settings::load(dir);
        let has_key = secrets.load().ok().flatten().is_some();
        AssistantService {
            dir: dir.to_path_buf(),
            secrets,
            view: AssistantView {
                settings,
                has_key,
                ..AssistantView::default()
            },
            inflight: None,
            stash: Vec::new(),
        }
    }

    pub fn view(&self) -> &AssistantView {
        &self.view
    }

    pub fn save_settings(&mut self, s: Settings) {
        if let Err(e) = s.save(&self.dir) {
            tracing::warn!("nao foi possivel gravar assistant.json: {e}");
        }
        if s.provider != self.view.settings.provider
            || s.local.port != self.view.settings.local.port
        {
            self.view.models = None;
        }
        self.view.settings = s;
    }

    pub fn save_key(&mut self, key: &str) {
        let key = key.trim();
        let r = if key.is_empty() {
            self.secrets.clear()
        } else {
            self.secrets.save(key)
        };
        if let Err(e) = r {
            tracing::warn!("nao foi possivel gravar a chave: {e}");
        }
        self.view.has_key = self.secrets.load().ok().flatten().is_some();
    }

    pub fn set_models(&mut self, r: Result<Vec<String>, AssistantError>) {
        self.view.models = Some(r.map_err(|e| e.to_string()));
    }

    /// Provedor real para o destino atual; `None` sem assistente utilizavel.
    pub fn provider(&self) -> Option<Box<dyn Provider>> {
        let d = self.view.settings.destination()?;
        let key = if d.is_cloud() {
            self.secrets.load().ok().flatten().map(Secret::new)
        } else {
            None
        };
        Some(Box::new(OpenAiCompatible::new(
            d,
            key,
            self.view.settings.cloud.deny_data_collection,
        )))
    }

    pub fn new_conversation(&mut self, book: Book, title: String, envelope: String) {
        self.leave_open();
        let id = new_id();
        self.view.conversations.insert(
            0,
            ConversationSummary {
                id: id.clone(),
                book,
                title,
                created_at: String::new(),
            },
        );
        self.view.open = Some(OpenConversation {
            id,
            book,
            envelope,
            messages: vec![user_message(OPENING_REQUEST)],
            needs_cloud_consent: true,
        });
    }

    /// Sem efeito com resposta em andamento: a pergunta ficaria sem par e o proximo
    /// `prepare_send` a mandaria junto com a resposta que chegar.
    pub fn push_user(&mut self, text: &str) {
        if self.view.busy {
            return;
        }
        if let Some(open) = self.view.open.as_mut() {
            open.messages.push(user_message(text));
        }
    }

    /// Monta o pedido da ultima mensagem do usuario. `None` se nao ha provedor, modelo ou
    /// conversa aberta, ou se ja ha resposta em andamento.
    pub fn prepare_send(&mut self) -> Option<Job> {
        if self.view.busy {
            return None;
        }
        let provider = self.provider()?;
        let model = self.view.settings.model()?;
        let to_cloud = self
            .view
            .settings
            .destination()
            .is_some_and(|d| d.is_cloud());
        let open = self.view.open.as_ref()?;
        let history: Vec<Turn> = open
            .messages
            .iter()
            .filter_map(|m| match m.role {
                MessageRole::User => Some(Turn {
                    role: Role::User,
                    content: m.content.clone(),
                }),
                MessageRole::Assistant => Some(Turn {
                    role: Role::Assistant,
                    content: m.content.clone(),
                }),
                MessageRole::Error => None,
            })
            .collect();
        let request = ChatRequest {
            model,
            messages: prompt::messages(&open.envelope, &history),
            max_tokens: MAX_OUTPUT_TOKENS,
        };
        let cancel = CancelFlag::default();
        self.inflight = Some(Inflight {
            conversation_id: open.id.clone(),
            cancel: cancel.clone(),
            to_cloud,
        });
        self.view.busy = true;
        self.view.streaming = Some(String::new());
        Some(Job {
            request,
            provider,
            cancel,
            reply_id: new_id(),
        })
    }

    /// O pedaco so aparece se a conversa aberta ainda e a que esta sendo respondida.
    pub fn delta(&mut self, d: &str) {
        let same = self
            .inflight
            .as_ref()
            .zip(self.view.open.as_ref())
            .is_some_and(|(f, o)| f.conversation_id == o.id);
        if same && let Some(s) = self.view.streaming.as_mut() {
            s.push_str(d);
        }
    }

    pub fn cancel(&mut self) {
        if let Some(f) = &self.inflight {
            f.cancel.cancel();
        }
    }

    pub fn finish(&mut self, reply_id: String, result: Result<ChatOutcome, AssistantError>) {
        self.view.busy = false;
        self.view.streaming = None;
        let Some(inflight) = self.inflight.take() else {
            return;
        };
        let msg = match result {
            Ok(o) => MessageView {
                id: reply_id,
                role: MessageRole::Assistant,
                content: o.text,
                sent_to_cloud: inflight.to_cloud,
                tokens_in: o.tokens_in,
                tokens_out: o.tokens_out,
                interrupted: o.interrupted,
            },
            Err(e) => MessageView {
                id: reply_id,
                role: MessageRole::Error,
                content: e.to_string(),
                sent_to_cloud: false,
                tokens_in: None,
                tokens_out: None,
                interrupted: false,
            },
        };
        // Conversa apagada enquanto o modelo respondia: o resultado nao tem para onde ir.
        let Some(target) = self.conversation_mut(&inflight.conversation_id) else {
            return;
        };
        if inflight.to_cloud && msg.role == MessageRole::Assistant {
            target.needs_cloud_consent = false;
        }
        target.messages.push(msg);
    }

    /// "Tentar de novo": tira a ultima mensagem de erro e reenvia.
    pub fn drop_last_error(&mut self) {
        if self.view.busy {
            return;
        }
        if let Some(open) = self.view.open.as_mut()
            && open
                .messages
                .last()
                .is_some_and(|m| m.role == MessageRole::Error)
        {
            open.messages.pop();
        }
    }

    pub fn open_conversation(&mut self, id: &str) {
        if self.view.open.as_ref().is_some_and(|o| o.id == id) {
            return;
        }
        if let Some(i) = self.stash.iter().position(|c| c.id == id) {
            self.leave_open();
            let c = self.stash.remove(i);
            self.view.open = Some(c);
        }
    }

    pub fn delete_conversation(&mut self, id: &str) {
        if self
            .inflight
            .as_ref()
            .is_some_and(|f| f.conversation_id == id)
        {
            self.cancel();
        }
        self.stash.retain(|c| c.id != id);
        self.view.conversations.retain(|c| c.id != id);
        if self.view.open.as_ref().is_some_and(|o| o.id == id) {
            self.view.open = None;
        }
    }

    pub fn delete_all(&mut self) {
        self.cancel();
        self.stash.clear();
        self.view.conversations.clear();
        self.view.open = None;
    }

    pub fn close_conversation(&mut self) {
        self.leave_open();
    }

    /// Guarda a conversa aberta antes de abrir outra. Sair de uma conversa que ainda esta sendo
    /// respondida para a resposta: o texto parcial fica nela como "interrompida" em vez de
    /// continuar escrevendo (e custando tokens) para uma tela que ninguem olha.
    fn leave_open(&mut self) {
        let Some(prev) = self.view.open.take() else {
            return;
        };
        if self
            .inflight
            .as_ref()
            .is_some_and(|f| f.conversation_id == prev.id)
        {
            self.cancel();
        }
        self.stash.push(prev);
    }

    fn conversation_mut(&mut self, id: &str) -> Option<&mut OpenConversation> {
        if let Some(open) = self.view.open.as_mut().filter(|o| o.id == id) {
            return Some(open);
        }
        self.stash.iter_mut().find(|c| c.id == id)
    }
}

fn user_message(text: &str) -> MessageView {
    MessageView {
        id: new_id(),
        role: MessageRole::User,
        content: text.to_string(),
        sent_to_cloud: false,
        tokens_in: None,
        tokens_out: None,
        interrupted: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::fake::FakeProvider;
    use crate::assistant::settings::ProviderChoice;
    use crate::dashboard::contract::Book;
    use crate::desktop::secret::FakeSecretStore;

    fn svc(dir: &std::path::Path) -> AssistantService {
        let mut s = AssistantService::open(dir, Box::new(FakeSecretStore::default()));
        let mut st = s.view().settings.clone();
        st.provider = ProviderChoice::Local;
        st.local.model = Some("m".into());
        s.save_settings(st);
        s
    }

    fn run(s: &mut AssistantService, p: &FakeProvider) {
        let job = s.prepare_send().expect("tem job");
        let mut deltas = String::new();
        let res = p.chat(&job.request, &mut |d| deltas.push_str(d), &job.cancel);
        s.finish(job.reply_id, res);
    }

    #[test]
    fn nova_conversa_pede_a_abertura_e_grava_resposta_e_tokens() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = svc(dir.path());
        s.new_conversation(Book::Personal, "Análise de 2026-11".into(), "ENV".into());
        let p = FakeProvider::answering(&["Olá", " mundo"]);
        run(&mut s, &p);
        let open = s.view().open.clone().unwrap();
        assert_eq!(open.messages.len(), 2);
        assert_eq!(open.messages[0].content, OPENING_REQUEST);
        assert_eq!(open.messages[1].content, "Olá mundo");
        assert_eq!(open.messages[1].tokens_out, Some(20));
        let seen = p.seen.lock().unwrap();
        assert!(seen[0].messages[0].content.contains("ENV"));
        assert!(!s.view().busy);
        assert_eq!(s.view().streaming, None);
    }

    #[test]
    fn cancelar_grava_interrompida() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = svc(dir.path());
        s.new_conversation(Book::Personal, "t".into(), "ENV".into());
        let mut p = FakeProvider::answering(&["a", "b", "c"]);
        p.cancel_after = Some(1);
        run(&mut s, &p);
        let last = s.view().open.clone().unwrap().messages.pop().unwrap();
        assert!(last.interrupted);
        assert_eq!(last.content, "a");
    }

    #[test]
    fn erro_vira_mensagem_de_erro() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = svc(dir.path());
        s.new_conversation(Book::Personal, "t".into(), "ENV".into());
        let mut p = FakeProvider::answering(&[]);
        p.fail = Some(AssistantError::LocalUnreachable(11434));
        run(&mut s, &p);
        let last = s.view().open.clone().unwrap().messages.pop().unwrap();
        assert_eq!(last.role, MessageRole::Error);
        assert!(last.content.contains("Ollama"));
    }

    #[test]
    fn sem_provedor_nao_ha_job() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = AssistantService::open(dir.path(), Box::new(FakeSecretStore::default()));
        s.new_conversation(Book::Personal, "t".into(), "ENV".into());
        assert!(s.prepare_send().is_none());
    }

    #[test]
    fn nao_comeca_segundo_job_enquanto_ocupado() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = svc(dir.path());
        s.new_conversation(Book::Personal, "t".into(), "ENV".into());
        let job = s.prepare_send().expect("primeiro job");
        assert!(s.view().busy);
        assert!(s.prepare_send().is_none());
        // Pergunta digitada no meio da resposta nao entra na conversa.
        s.push_user("outra");
        assert_eq!(s.view().open.as_ref().unwrap().messages.len(), 1);
        s.finish(job.reply_id, Ok(ChatOutcome::default()));
        assert!(!s.view().busy);
        assert!(s.prepare_send().is_some());
    }

    #[test]
    fn tentar_de_novo_tira_o_erro_e_reenvia_a_mesma_pergunta() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = svc(dir.path());
        s.new_conversation(Book::Personal, "t".into(), "ENV".into());
        let mut p = FakeProvider::answering(&[]);
        p.fail = Some(AssistantError::Offline);
        run(&mut s, &p);
        s.drop_last_error();
        assert_eq!(s.view().open.as_ref().unwrap().messages.len(), 1);
        let ok = FakeProvider::answering(&["ok"]);
        run(&mut s, &ok);
        let seen = ok.seen.lock().unwrap();
        // Sistema + a pergunta de abertura; o erro nunca vai ao modelo.
        assert_eq!(seen[0].messages.len(), 2);
        assert_eq!(s.view().open.as_ref().unwrap().messages[1].content, "ok");
    }

    #[test]
    fn resposta_cai_na_conversa_que_a_pediu_mesmo_com_outra_aberta() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = svc(dir.path());
        s.new_conversation(Book::Personal, "a".into(), "ENV-A".into());
        let first = s.view().open.as_ref().unwrap().id.clone();
        let job = s.prepare_send().expect("job");
        s.new_conversation(Book::Business, "b".into(), "ENV-B".into());
        // Trocar de conversa para a resposta que estava a caminho.
        assert!(job.cancel.is_cancelled());
        s.delta("nao deve aparecer");
        assert_eq!(s.view().streaming.as_deref(), Some(""));
        s.finish(
            job.reply_id,
            Ok(ChatOutcome {
                text: "parcial".into(),
                interrupted: true,
                ..ChatOutcome::default()
            }),
        );
        assert_eq!(s.view().open.as_ref().unwrap().messages.len(), 1);
        s.open_conversation(&first);
        let back = s.view().open.as_ref().unwrap();
        assert_eq!(back.messages.len(), 2);
        assert_eq!(back.messages[1].content, "parcial");
        assert!(back.messages[1].interrupted);
    }

    #[test]
    fn conversa_apagada_durante_a_resposta_descarta_o_resultado() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = svc(dir.path());
        s.new_conversation(Book::Personal, "a".into(), "E".into());
        let id = s.view().open.as_ref().unwrap().id.clone();
        let job = s.prepare_send().expect("job");
        s.delete_conversation(&id);
        assert!(job.cancel.is_cancelled());
        s.finish(job.reply_id, Ok(ChatOutcome::default()));
        assert!(s.view().open.is_none());
        assert!(s.view().conversations.is_empty());
        assert!(!s.view().busy);
    }

    #[test]
    fn deltas_acumulam_no_streaming_e_somem_no_fim() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = svc(dir.path());
        s.new_conversation(Book::Personal, "t".into(), "E".into());
        let job = s.prepare_send().expect("job");
        s.delta("Ol");
        s.delta("á");
        assert_eq!(s.view().streaming.as_deref(), Some("Olá"));
        s.finish(job.reply_id, Ok(ChatOutcome::default()));
        assert_eq!(s.view().streaming, None);
    }

    #[test]
    fn nuvem_so_marca_consentimento_dado_apos_resposta_da_nuvem() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = AssistantService::open(dir.path(), Box::new(FakeSecretStore::default()));
        let mut st = s.view().settings.clone();
        st.provider = ProviderChoice::Cloud;
        st.cloud.enabled = true;
        s.save_settings(st);
        s.save_key("sk-or-x");
        assert!(s.view().has_key);
        s.new_conversation(Book::Personal, "t".into(), "E".into());
        assert!(s.view().open.as_ref().unwrap().needs_cloud_consent);
        let p = FakeProvider::answering(&["ok"]);
        run(&mut s, &p);
        let open = s.view().open.as_ref().unwrap();
        assert!(!open.needs_cloud_consent);
        assert!(open.messages[1].sent_to_cloud);
        s.save_key("  ");
        assert!(!s.view().has_key);
    }

    #[test]
    fn configuracao_persiste_no_diretorio() {
        let dir = tempfile::tempdir().unwrap();
        let s = svc(dir.path());
        let again = AssistantService::open(dir.path(), Box::new(FakeSecretStore::default()));
        assert_eq!(again.view().settings, s.view().settings);
    }
}
