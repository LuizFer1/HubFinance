//! Estado do assistente publicado no `Snapshot`. A UI so le isto e manda `Command::Assistant`.

use super::settings::Settings;
use crate::dashboard::contract::Book;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageView {
    pub id: String,
    pub role: MessageRole,
    pub content: String,
    pub sent_to_cloud: bool,
    /// Politica de retencao que valia **no envio** (`Some(true)` = o provedor foi instruido a
    /// nao guardar nem treinar). `None` = nao foi para a nuvem. Fica na mensagem e nao na
    /// configuracao: trocar o interruptor depois nao pode reescrever o que ja aconteceu.
    pub deny_data_collection: Option<bool>,
    pub tokens_in: Option<u32>,
    pub tokens_out: Option<u32>,
    pub interrupted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConversationSummary {
    pub id: String,
    pub book: Book,
    pub title: String,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenConversation {
    pub id: String,
    pub book: Book,
    pub envelope: String,
    pub messages: Vec<MessageView>,
    /// Ainda nao saiu nada desta conversa para a nuvem: enquanto for `true` o nucleo **nao**
    /// envia a conversa para a nuvem (`prepare_send` devolve `None`) ate o usuario ver a previa
    /// e aprovar (`grant_cloud_consent`). Local nunca precisa (Q5).
    pub needs_cloud_consent: bool,
}

impl OpenConversation {
    pub fn tokens_total(&self) -> (u32, u32) {
        self.messages.iter().fold((0, 0), |(i, o), m| {
            (
                i.saturating_add(m.tokens_in.unwrap_or(0)),
                o.saturating_add(m.tokens_out.unwrap_or(0)),
            )
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AssistantView {
    pub settings: Settings,
    pub has_key: bool,
    /// A chave fica cifrada no disco (DPAPI). `false` = texto puro: o painel avisa.
    pub key_encrypted: bool,
    /// `None` = nunca listado; `Some(Err)` = falhou (mensagem pt-BR).
    pub models: Option<Result<Vec<String>, String>>,
    pub conversations: Vec<ConversationSummary>,
    pub open: Option<OpenConversation>,
    /// Texto chegando agora; vira `MessageView` no fim.
    pub streaming: Option<String>,
    pub busy: bool,
    /// Falha de gravacao (configuracao ou chave) em pt-BR, para a tela mostrar em vez de a
    /// escolha do usuario parecer salva. Some na proxima acao que der certo.
    pub last_error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(i: Option<u32>, o: Option<u32>) -> MessageView {
        MessageView {
            id: "x".into(),
            role: MessageRole::Assistant,
            content: String::new(),
            sent_to_cloud: false,
            deny_data_collection: None,
            tokens_in: i,
            tokens_out: o,
            interrupted: false,
        }
    }

    #[test]
    fn total_de_tokens_soma_so_o_que_veio() {
        let open = OpenConversation {
            id: "c".into(),
            book: Book::Personal,
            envelope: String::new(),
            messages: vec![
                msg(Some(10), Some(2)),
                msg(None, None),
                msg(Some(5), Some(1)),
            ],
            needs_cloud_consent: true,
        };
        assert_eq!(open.tokens_total(), (15, 3));
    }
}
