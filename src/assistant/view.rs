//! Estado do assistente publicado no `Snapshot`. A UI so le isto e manda `Command::Assistant`.

use super::prompt::{self, Role, Turn};
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
    /// "local" ou "cloud" e o modelo que **de fato respondeu**, so nas respostas do assistente.
    /// Ficam na mensagem pelo mesmo motivo da politica: trocar a configuracao depois nao pode
    /// reescrever quem respondeu. Texto, e nao enum: valor de versao futura degrada, nao quebra.
    pub provider: Option<String>,
    pub model: Option<String>,
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
    /// O que o modelo recebe: sistema (regras + envelope) e a janela do historico. E a unica
    /// montagem do pedido: o envio e a previa da nuvem leem daqui, entao o que a pessoa
    /// aprova e exatamente o que sai (erros nunca vao ao modelo).
    pub fn payload(&self) -> Vec<Turn> {
        let history: Vec<Turn> = self
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
        prompt::messages(&self.envelope, &history)
    }

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
    /// A chave fica cifrada no disco (DPAPI). `false` = texto puro: o dialogo avisa.
    pub key_encrypted: bool,
    /// `None` = nunca listado; `Some(Err)` = falhou (mensagem pt-BR).
    pub models: Option<Result<Vec<String>, String>>,
    /// A porta do Ollama de onde `models` veio: o dialogo so mostra a lista se for a porta que
    /// esta no campo agora.
    pub models_port: Option<u16>,
    /// "Testar conexao" esperando a resposta do Ollama (A7: o botao mostra o carregando).
    pub listing: bool,
    /// Quantas gravacoes do dialogo (`AssistantCommand::Configure`) o nucleo ja terminou, com
    /// sucesso ou nao. O dialogo guarda o numero esperado e fecha quando ele chega sem erro.
    pub configured: u64,
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
            provider: None,
            model: None,
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
