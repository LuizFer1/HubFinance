//! Orquestra o assistente no nucleo do hub: guarda configuracao e conversas, prepara o pedido
//! ao modelo e grava o resultado. Nao faz I/O de rede: o hub roda `Job` num `spawn_blocking`
//! com o provedor e devolve o resultado em `finish`. Assim o fluxo inteiro e testavel com o
//! `FakeProvider`.

use std::path::{Path, PathBuf};

use super::AssistantError;
use super::client::OpenAiCompatible;
use super::destination::Destination;
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
    /// A politica de retencao que foi mandada ao provedor neste envio (`None` no local). Capturada
    /// aqui, junto do `to_cloud`: reler a configuracao em `record` registraria a politica de
    /// depois, nao a que de fato valeu.
    deny_data_collection: Option<bool>,
}

pub struct AssistantService {
    dir: PathBuf,
    secrets: Box<dyn SecretStore>,
    view: AssistantView,
    inflight: Option<Inflight>,
    /// Conversa que pediu um envio enquanto outra resposta ainda estava a caminho (iniciar uma
    /// conversa com outra em andamento cancela a anterior, mas o cancelamento so vale quando
    /// o trabalhador termina). Sem isto o pedido de abertura da conversa nova era descartado e
    /// ela ficava sem resposta. `finish` o retoma se a conversa ainda for a aberta.
    pending_send: Option<String>,
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
        let key_encrypted = secrets.encrypted();
        AssistantService {
            dir: dir.to_path_buf(),
            secrets,
            view: AssistantView {
                settings,
                has_key,
                key_encrypted,
                ..AssistantView::default()
            },
            inflight: None,
            pending_send: None,
            stash: Vec::new(),
        }
    }

    pub fn view(&self) -> &AssistantView {
        &self.view
    }

    /// Destino atual (`None` = sem assistente utilizavel).
    pub fn destination(&self) -> Option<Destination> {
        self.view.settings.destination()
    }

    /// Se a gravacao falha, a view segue com a configuracao antiga: mostrar a nova como valida
    /// faria o usuario acreditar que a nuvem esta ligada (ou desligada) quando, depois de
    /// reabrir o hub, nao estaria.
    pub fn save_settings(&mut self, s: Settings) {
        if let Err(e) = s.save(&self.dir) {
            tracing::warn!("nao foi possivel gravar assistant.json: {e}");
            self.view.last_error =
                Some("Não foi possível salvar a configuração do assistente.".into());
            return;
        }
        self.view.last_error = None;
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
        self.view.last_error = match r {
            Ok(()) => None,
            Err(e) => {
                // So o erro do sistema, nunca o valor da chave.
                tracing::warn!("nao foi possivel gravar a chave: {e}");
                Some(if key.is_empty() {
                    "Não foi possível apagar a chave.".into()
                } else {
                    "Não foi possível salvar a chave.".into()
                })
            }
        };
        self.view.has_key = self.secrets.load().ok().flatten().is_some();
    }

    /// `listed` e o destino que foi consultado. Se o usuario trocou de provedor ou de porta
    /// enquanto a lista vinha, ela e de outro lugar e nao pode aparecer como a do atual.
    pub fn set_models(&mut self, listed: &Destination, r: Result<Vec<String>, AssistantError>) {
        if self.destination().as_ref() != Some(listed) {
            return;
        }
        self.view.models = Some(r.map_err(|e| e.to_string()));
    }

    /// O usuario viu a previa e aprovou o envio da conversa `id` para a nuvem. Vale so se `id`
    /// ainda e a conversa aberta (o clique pode chegar depois de ela fechar ou de abrir outra)
    /// e so enquanto o destino e a nuvem: a previa mostra o que sairia **para a nuvem**, e uma
    /// aprovacao dada com o modelo local nao a cobre se o usuario trocar de provedor depois.
    /// Devolve se a aprovacao foi registrada.
    pub fn grant_cloud_consent(&mut self, id: &str) -> bool {
        let cloud = self.destination().is_some_and(|d| d.is_cloud());
        match self.view.open.as_mut() {
            Some(o) if cloud && o.id == id => {
                o.needs_cloud_consent = false;
                true
            }
            _ => false,
        }
    }

    /// A conversa aberta termina numa pergunta sem resposta. Aprovar a previa de uma conversa
    /// que ja foi respondida (por exemplo, no modelo local, antes de trocar para a nuvem) so
    /// libera as proximas perguntas: reenviar o historico terminado numa resposta do modelo
    /// seria um pedido sem pergunta.
    pub fn has_unanswered_question(&self) -> bool {
        self.view
            .open
            .as_ref()
            .and_then(|o| o.messages.last())
            .is_some_and(|m| m.role == MessageRole::User)
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
        self.view.last_error = None;
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
            // Lembra para quem era, mas so se for outra conversa: um reenvio para a que ja esta
            // sendo respondida a mandaria duas vezes.
            self.pending_send = self.view.open.as_ref().map(|o| o.id.clone()).filter(|id| {
                self.inflight
                    .as_ref()
                    .is_none_or(|f| f.conversation_id != *id)
            });
            return None;
        }
        let to_cloud = self
            .view
            .settings
            .destination()
            .is_some_and(|d| d.is_cloud());
        let open = self.view.open.as_ref()?;
        // A porta da nuvem: sem a aprovacao da previa nada e montado, quanto mais enviado.
        if to_cloud && open.needs_cloud_consent {
            return None;
        }
        let provider = self.provider()?;
        let model = self.view.settings.model()?;
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
            deny_data_collection: to_cloud.then_some(self.view.settings.cloud.deny_data_collection),
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

    /// Grava o resultado e, se outra conversa ficou esperando a vez (`pending_send`), devolve o
    /// proximo trabalho: quem chama (o nucleo) precisa dispara-lo.
    pub fn finish(
        &mut self,
        reply_id: String,
        result: Result<ChatOutcome, AssistantError>,
    ) -> Option<Job> {
        self.record(reply_id, result);
        let waiting = self.pending_send.take()?;
        let open = self.view.open.as_ref().filter(|o| o.id == waiting)?;
        // So ha o que enviar se a ultima fala for do usuario; senao o pedido ja foi atendido.
        if open.messages.last().map(|m| m.role) != Some(MessageRole::User) {
            return None;
        }
        // O consentimento da nuvem e as demais regras valem como em qualquer envio.
        self.prepare_send()
    }

    fn record(&mut self, reply_id: String, result: Result<ChatOutcome, AssistantError>) {
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
                deny_data_collection: inflight.deny_data_collection,
                tokens_in: o.tokens_in,
                tokens_out: o.tokens_out,
                interrupted: o.interrupted,
            },
            Err(e) => MessageView {
                id: reply_id,
                role: MessageRole::Error,
                content: e.to_string(),
                sent_to_cloud: false,
                deny_data_collection: None,
                tokens_in: None,
                tokens_out: None,
                interrupted: false,
            },
        };
        // Conversa apagada enquanto o modelo respondia: o resultado nao tem para onde ir.
        let Some(target) = self.conversation_mut(&inflight.conversation_id) else {
            return;
        };
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
        deny_data_collection: None,
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

    fn cloud_svc(dir: &std::path::Path) -> AssistantService {
        let mut s = AssistantService::open(dir, Box::new(FakeSecretStore::default()));
        let mut st = s.view().settings.clone();
        st.provider = ProviderChoice::Cloud;
        st.cloud.enabled = true;
        s.save_settings(st);
        s.save_key("sk-or-x");
        s
    }

    /// Abre A, deixa a resposta dela a caminho e abre B por cima (o que cancela A).
    fn a_respondendo_e_b_aberta(s: &mut AssistantService) -> (Job, String) {
        s.new_conversation(Book::Personal, "a".into(), "ENV-A".into());
        let job_a = s.prepare_send().expect("job de A");
        s.new_conversation(Book::Personal, "b".into(), "ENV-B".into());
        let b = s.view().open.as_ref().unwrap().id.clone();
        // O pedido de abertura de B chega com A ainda ocupada.
        assert!(s.prepare_send().is_none());
        (job_a, b)
    }

    #[test]
    fn conversa_nova_com_outra_respondendo_envia_a_abertura_quando_a_outra_termina() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = svc(dir.path());
        let (job_a, b) = a_respondendo_e_b_aberta(&mut s);
        let next = s
            .finish(job_a.reply_id, Ok(ChatOutcome::default()))
            .expect("B recebe a vez");
        assert!(s.view().busy);
        assert!(next.request.messages[0].content.contains("ENV-B"));
        assert_eq!(s.view().open.as_ref().unwrap().id, b);
        // A resposta da B cai na B.
        s.finish(next.reply_id, Ok(ChatOutcome::default()));
        assert_eq!(s.view().open.as_ref().unwrap().messages.len(), 2);
        // E nada fica pendente depois.
        assert!(s.pending_send.is_none());
    }

    #[test]
    fn pedido_pendente_de_conversa_apagada_ou_fechada_nao_envia_nada() {
        let dir = tempfile::tempdir().unwrap();

        let mut s = svc(dir.path());
        let (job_a, b) = a_respondendo_e_b_aberta(&mut s);
        s.delete_conversation(&b);
        assert!(
            s.finish(job_a.reply_id, Ok(ChatOutcome::default()))
                .is_none()
        );
        assert!(!s.view().busy);

        let mut s = svc(dir.path());
        let (job_a, _b) = a_respondendo_e_b_aberta(&mut s);
        s.close_conversation();
        assert!(
            s.finish(job_a.reply_id, Ok(ChatOutcome::default()))
                .is_none()
        );
        assert!(!s.view().busy);
    }

    #[test]
    fn reenvio_para_a_conversa_que_ja_responde_nao_manda_duas_vezes() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = svc(dir.path());
        s.new_conversation(Book::Personal, "a".into(), "E".into());
        let job = s.prepare_send().expect("job");
        assert!(s.prepare_send().is_none());
        assert!(s.finish(job.reply_id, Ok(ChatOutcome::default())).is_none());
    }

    #[test]
    fn pedido_pendente_da_nuvem_sem_consentimento_nao_gera_job() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = cloud_svc(dir.path());
        s.new_conversation(Book::Personal, "a".into(), "ENV-A".into());
        let id_a = s.view().open.as_ref().unwrap().id.clone();
        assert!(s.grant_cloud_consent(&id_a));
        let job_a = s.prepare_send().expect("job de A");
        s.new_conversation(Book::Personal, "b".into(), "ENV-B".into());
        assert!(s.prepare_send().is_none());
        assert!(
            s.finish(job_a.reply_id, Ok(ChatOutcome::default()))
                .is_none()
        );
        assert!(!s.view().busy);
    }

    #[test]
    fn nuvem_sem_consentimento_nao_gera_job_e_com_ele_gera() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = cloud_svc(dir.path());
        assert!(s.view().has_key);
        s.new_conversation(Book::Personal, "t".into(), "E".into());
        assert!(s.view().open.as_ref().unwrap().needs_cloud_consent);
        assert!(s.prepare_send().is_none(), "sem a previa aprovada nada sai");
        assert!(!s.view().busy);
        let id = s.view().open.as_ref().unwrap().id.clone();
        assert!(s.grant_cloud_consent(&id));
        let p = FakeProvider::answering(&["ok"]);
        run(&mut s, &p);
        let open = s.view().open.as_ref().unwrap();
        assert!(!open.needs_cloud_consent);
        assert!(open.messages[1].sent_to_cloud);
        s.save_key("  ");
        assert!(!s.view().has_key);
    }

    #[test]
    fn pergunta_sem_resposta_so_existe_se_a_ultima_fala_e_do_usuario() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = svc(dir.path());
        assert!(!s.has_unanswered_question(), "sem conversa aberta");
        s.new_conversation(Book::Personal, "t".into(), "E".into());
        assert!(s.has_unanswered_question());
        run(&mut s, &FakeProvider::answering(&["oi"]));
        assert!(!s.has_unanswered_question());
        s.push_user("e agora?");
        assert!(s.has_unanswered_question());
    }

    #[test]
    fn resposta_da_nuvem_grava_a_politica_do_envio_e_a_local_nao_grava_nenhuma() {
        let dir = tempfile::tempdir().unwrap();
        for deny in [true, false] {
            let mut s = cloud_svc(dir.path());
            let mut st = s.view().settings.clone();
            st.cloud.deny_data_collection = deny;
            s.save_settings(st);
            s.new_conversation(Book::Personal, "t".into(), "E".into());
            let id = s.view().open.as_ref().unwrap().id.clone();
            assert!(s.grant_cloud_consent(&id));
            run(&mut s, &FakeProvider::answering(&["ok"]));
            let answer = s.view().open.as_ref().unwrap().messages[1].clone();
            assert!(answer.sent_to_cloud);
            assert_eq!(answer.deny_data_collection, Some(deny));
        }
        let mut s = svc(dir.path());
        s.new_conversation(Book::Personal, "t".into(), "E".into());
        run(&mut s, &FakeProvider::answering(&["ok"]));
        let answer = s.view().open.as_ref().unwrap().messages[1].clone();
        assert!(!answer.sent_to_cloud);
        assert_eq!(answer.deny_data_collection, None);
    }

    #[test]
    fn politica_registrada_e_a_do_envio_mesmo_se_mudar_antes_da_resposta() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = cloud_svc(dir.path());
        s.new_conversation(Book::Personal, "t".into(), "E".into());
        let id = s.view().open.as_ref().unwrap().id.clone();
        assert!(s.grant_cloud_consent(&id));
        let job = s.prepare_send().expect("job");
        // Com a resposta a caminho, a pessoa afrouxa a politica e ate volta ao modelo local.
        let mut st = s.view().settings.clone();
        st.cloud.deny_data_collection = false;
        st.provider = ProviderChoice::Local;
        s.save_settings(st);
        s.finish(job.reply_id, Ok(ChatOutcome::default()));
        let answer = s.view().open.as_ref().unwrap().messages[1].clone();
        assert!(answer.sent_to_cloud);
        assert_eq!(answer.deny_data_collection, Some(true));
    }

    #[test]
    fn erro_da_nuvem_nao_leva_selo_nem_politica() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = cloud_svc(dir.path());
        s.new_conversation(Book::Personal, "t".into(), "E".into());
        let id = s.view().open.as_ref().unwrap().id.clone();
        assert!(s.grant_cloud_consent(&id));
        let mut p = FakeProvider::answering(&[]);
        p.fail = Some(AssistantError::Offline);
        run(&mut s, &p);
        let last = s.view().open.as_ref().unwrap().messages[1].clone();
        assert_eq!(last.role, MessageRole::Error);
        assert!(!last.sent_to_cloud);
        assert_eq!(last.deny_data_collection, None);
    }

    #[test]
    fn consentimento_vale_so_para_a_conversa_aprovada() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = cloud_svc(dir.path());
        s.new_conversation(Book::Personal, "a".into(), "E".into());
        let a = s.view().open.as_ref().unwrap().id.clone();
        assert!(s.grant_cloud_consent(&a));
        s.new_conversation(Book::Personal, "b".into(), "E".into());
        assert!(s.prepare_send().is_none());
        // Aprovar a conversa que ja nao esta aberta nao libera a aberta.
        assert!(!s.grant_cloud_consent(&a));
        assert!(!s.grant_cloud_consent("inexistente"));
        assert!(s.prepare_send().is_none());
        assert!(s.view().open.as_ref().unwrap().needs_cloud_consent);
    }

    #[test]
    fn aprovacao_dada_com_o_modelo_local_nao_vale_para_a_nuvem() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = svc(dir.path());
        s.new_conversation(Book::Personal, "t".into(), "E".into());
        let id = s.view().open.as_ref().unwrap().id.clone();
        assert!(!s.grant_cloud_consent(&id));
        let mut st = s.view().settings.clone();
        st.provider = ProviderChoice::Cloud;
        st.cloud.enabled = true;
        s.save_settings(st);
        s.save_key("sk-or-x");
        assert!(s.prepare_send().is_none(), "precisa de aprovacao nova");
        assert!(s.grant_cloud_consent(&id));
        assert!(s.prepare_send().is_some());
    }

    #[test]
    fn local_nunca_precisa_de_consentimento() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = svc(dir.path());
        s.new_conversation(Book::Personal, "t".into(), "E".into());
        assert!(s.view().open.as_ref().unwrap().needs_cloud_consent);
        assert!(s.prepare_send().is_some());
    }

    #[test]
    fn lista_de_modelos_de_outro_destino_e_descartada() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = svc(dir.path());
        let listed = Destination::local(11434);
        // O usuario trocou a porta enquanto a lista vinha.
        let mut st = s.view().settings.clone();
        st.local.port = 11500;
        s.save_settings(st);
        s.set_models(&listed, Ok(vec!["velho".into()]));
        assert_eq!(s.view().models, None);
        s.set_models(&Destination::local(11500), Ok(vec!["novo".into()]));
        assert_eq!(s.view().models, Some(Ok(vec!["novo".into()])));
        s.set_models(
            &Destination::local(11500),
            Err(AssistantError::LocalUnreachable(11500)),
        );
        assert!(matches!(s.view().models, Some(Err(_))));
    }

    #[test]
    fn falha_ao_gravar_configuracao_mantem_a_antiga_e_avisa() {
        let dir = tempfile::tempdir().unwrap();
        // `data_dir` aponta para um arquivo: nao ha como criar `assistant.json` dentro dele.
        let file = dir.path().join("arquivo");
        std::fs::write(&file, "x").unwrap();
        let mut s = AssistantService::open(&file, Box::new(FakeSecretStore::default()));
        let mut st = s.view().settings.clone();
        st.provider = ProviderChoice::Local;
        s.save_settings(st);
        assert_eq!(s.view().settings, Settings::default());
        assert_eq!(
            s.view().last_error.as_deref(),
            Some("Não foi possível salvar a configuração do assistente.")
        );
        // Dar certo limpa o aviso.
        s.new_conversation(Book::Personal, "t".into(), "E".into());
        assert_eq!(s.view().last_error, None);
    }

    #[test]
    fn gravar_com_sucesso_limpa_o_aviso() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = AssistantService::open(dir.path(), Box::new(FailingStore));
        s.save_key("sk-or-x");
        assert_eq!(
            s.view().last_error.as_deref(),
            Some("Não foi possível salvar a chave.")
        );
        assert!(!s.view().has_key);
        s.save_settings(Settings::default());
        assert_eq!(s.view().last_error, None);
    }

    struct FailingStore;

    impl SecretStore for FailingStore {
        fn load(&self) -> std::io::Result<Option<String>> {
            Ok(None)
        }
        fn save(&self, _: &str) -> std::io::Result<()> {
            Err(std::io::Error::other("negado"))
        }
        fn clear(&self) -> std::io::Result<()> {
            Err(std::io::Error::other("negado"))
        }
        fn encrypted(&self) -> bool {
            true
        }
    }

    #[test]
    fn view_diz_se_a_chave_fica_cifrada() {
        struct Plain;
        impl SecretStore for Plain {
            fn load(&self) -> std::io::Result<Option<String>> {
                Ok(None)
            }
            fn save(&self, _: &str) -> std::io::Result<()> {
                Ok(())
            }
            fn clear(&self) -> std::io::Result<()> {
                Ok(())
            }
            fn encrypted(&self) -> bool {
                false
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let s = AssistantService::open(dir.path(), Box::new(Plain));
        assert!(!s.view().key_encrypted);
        let s = AssistantService::open(dir.path(), Box::new(FakeSecretStore::default()));
        assert!(s.view().key_encrypted);
    }

    #[test]
    fn configuracao_persiste_no_diretorio() {
        let dir = tempfile::tempdir().unwrap();
        let s = svc(dir.path());
        let again = AssistantService::open(dir.path(), Box::new(FakeSecretStore::default()));
        assert_eq!(again.view().settings, s.view().settings);
    }
}
