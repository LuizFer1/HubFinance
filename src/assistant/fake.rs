//! Provedor de roteiro para os testes: devolve os deltas na ordem, opcionalmente cancela depois
//! de N pedacos, ou falha.

use std::sync::Mutex;

use super::AssistantError;
use super::provider::{CancelFlag, ChatOutcome, ChatRequest, Provider};

pub struct FakeProvider {
    pub deltas: Vec<String>,
    pub usage: Option<(u32, u32)>,
    pub fail: Option<AssistantError>,
    pub cancel_after: Option<usize>,
    pub models: Vec<String>,
    pub seen: Mutex<Vec<ChatRequest>>,
}

impl FakeProvider {
    pub fn answering(parts: &[&str]) -> Self {
        FakeProvider {
            deltas: parts.iter().map(|s| s.to_string()).collect(),
            usage: Some((100, 20)),
            fail: None,
            cancel_after: None,
            models: vec!["llama3.1:8b".into()],
            seen: Mutex::new(Vec::new()),
        }
    }
}

impl Provider for FakeProvider {
    fn list_models(&self) -> Result<Vec<String>, AssistantError> {
        Ok(self.models.clone())
    }

    fn chat(
        &self,
        req: &ChatRequest,
        on_delta: &mut dyn FnMut(&str),
        cancel: &CancelFlag,
    ) -> Result<ChatOutcome, AssistantError> {
        if let Ok(mut s) = self.seen.lock() {
            s.push(req.clone());
        }
        if let Some(e) = &self.fail {
            return Err(e.clone());
        }
        let mut text = String::new();
        for (i, d) in self.deltas.iter().enumerate() {
            if self.cancel_after == Some(i) {
                cancel.cancel();
            }
            if cancel.is_cancelled() {
                return Ok(ChatOutcome {
                    text,
                    interrupted: true,
                    ..ChatOutcome::default()
                });
            }
            on_delta(d);
            text.push_str(d);
        }
        Ok(ChatOutcome {
            text,
            tokens_in: self.usage.map(|u| u.0),
            tokens_out: self.usage.map(|u| u.1),
            interrupted: false,
        })
    }
}
