//! Parser incremental do SSE de `/chat/completions` (OpenAI, Ollama `/v1`, OpenRouter). Puro:
//! recebe pedacos de texto como chegam da rede e devolve eventos; uma linha partida entre dois
//! pedacos fica no buffer ate o `\n`.

use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Delta(String),
    Usage { tokens_in: u32, tokens_out: u32 },
    Error { code: Option<i64>, message: String },
    Malformed,
    Done,
}

#[derive(Default)]
pub struct SseParser {
    buf: String,
}

/// Cada linha `data:` e tratada como um JSON completo. O SSE do padrao permite juntar varias
/// linhas `data:` num evento e so despachar na linha em branco, mas OpenAI, Ollama `/v1` e
/// OpenRouter mandam um JSON por linha `data:`; implementar a juncao seria codigo sem uso e
/// com mais chance de esconder um erro de formato real.
impl SseParser {
    pub fn push(&mut self, chunk: &str) -> Vec<Event> {
        self.buf.push_str(chunk);
        let mut out = Vec::new();
        while let Some(nl) = self.buf.find('\n') {
            let line: String = self.buf.drain(..=nl).collect();
            let line = line.trim_end_matches(['\n', '\r']);
            let Some(data) = line.strip_prefix("data:") else {
                continue;
            };
            let data = data.trim_start();
            if data == "[DONE]" {
                out.push(Event::Done);
                continue;
            }
            match serde_json::from_str::<Value>(data) {
                Ok(v) => parse_value(&v, &mut out),
                Err(_) => out.push(Event::Malformed),
            }
        }
        out
    }
}

fn parse_value(v: &Value, out: &mut Vec<Event>) {
    if let Some(e) = v.get("error") {
        out.push(Event::Error {
            code: e.get("code").and_then(Value::as_i64),
            message: e
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        });
        return;
    }
    if let Some(text) = v
        .pointer("/choices/0/delta/content")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        out.push(Event::Delta(text.to_string()));
    }
    if let Some(u) = v.get("usage").filter(|u| !u.is_null()) {
        let n = |k: &str| {
            u.get(k)
                .and_then(Value::as_u64)
                .and_then(|n| u32::try_from(n).ok())
        };
        if let (Some(i), Some(o)) = (n("prompt_tokens"), n("completion_tokens")) {
            out.push(Event::Usage {
                tokens_in: i,
                tokens_out: o,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delta_usage_e_fim() {
        let mut p = SseParser::default();
        let ev = p.push(
            "data: {\"choices\":[{\"delta\":{\"content\":\"Olá\"}}]}\n\n\
             data: {\"choices\":[],\"usage\":{\"prompt_tokens\":12,\"completion_tokens\":3}}\n\n\
             data: [DONE]\n\n",
        );
        assert_eq!(
            ev,
            vec![
                Event::Delta("Olá".into()),
                Event::Usage {
                    tokens_in: 12,
                    tokens_out: 3
                },
                Event::Done
            ]
        );
    }

    #[test]
    fn linha_partida_entre_chunks() {
        let mut p = SseParser::default();
        assert!(p.push("data: {\"choices\":[{\"delta\":{\"con").is_empty());
        assert_eq!(
            p.push("tent\":\"ab\"}}]}\n\n"),
            vec![Event::Delta("ab".into())]
        );
    }

    #[test]
    fn comentario_e_delta_vazio_sao_ignorados() {
        let mut p = SseParser::default();
        let ev = p.push(": OPENROUTER PROCESSING\n\ndata: {\"choices\":[{\"delta\":{}}]}\n\n");
        assert!(ev.is_empty());
    }

    #[test]
    fn erro_no_meio_do_stream() {
        let mut p = SseParser::default();
        let ev = p.push("data: {\"error\":{\"message\":\"sem saldo\",\"code\":402}}\n\n");
        assert_eq!(
            ev,
            vec![Event::Error {
                code: Some(402),
                message: "sem saldo".into()
            }]
        );
    }

    #[test]
    fn json_invalido_vira_erro_de_formato() {
        let mut p = SseParser::default();
        assert_eq!(p.push("data: {oops\n\n"), vec![Event::Malformed]);
    }
}
