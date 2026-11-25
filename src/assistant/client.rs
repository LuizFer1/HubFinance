//! Cliente OpenAI-compativel (`/v1/chat/completions` com `stream: true`). Unico modulo com
//! `ureq`. Fala com `Destination` e nada mais: nao ha como apontar para outra URL.

use std::io::Read;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::AssistantError;
use super::destination::Destination;
use super::provider::{CancelFlag, ChatOutcome, ChatRequest, Provider};
use super::secret::Secret;
use super::sse::{Event, SseParser};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// Ate os cabecalhos da resposta. Mais folgado que o resto porque um modelo local frio carrega
/// os pesos e le o prompt inteiro antes de emitir qualquer coisa.
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(120);
/// Sem byte novo por 60 s durante o stream = timeout (modelo local lento ainda manda pedacos).
const READ_IDLE_TIMEOUT: Duration = Duration::from_secs(60);
/// Teto do corpo inteiro. O `ureq` so tem orcamento *total* para o corpo (nao reinicia a cada
/// leitura), entao o "sem dado por 60 s" e feito por `consume_stream`; este teto so garante que
/// a thread de leitura de uma conexao parada acabe um dia, ja que `read` nao e interrompivel.
const BODY_TOTAL_CAP: Duration = Duration::from_secs(15 * 60);
/// De quanto em quanto tempo o laco confere o "Parar" enquanto espera dado da rede.
const CANCEL_POLL: Duration = Duration::from_millis(100);
/// Identificacao do app no OpenRouter: nome do produto, nenhum dado do usuario.
const APP_TITLE: &str = "HubFinance";

pub struct OpenAiCompatible {
    destination: Destination,
    api_key: Option<Secret>,
    deny_data_collection: bool,
    agent: ureq::Agent,
}

impl OpenAiCompatible {
    pub fn new(destination: Destination, api_key: Option<Secret>, deny: bool) -> Self {
        let mut builder = ureq::Agent::config_builder()
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .timeout_recv_response(Some(RESPONSE_TIMEOUT))
            .timeout_recv_body(Some(BODY_TOTAL_CAP))
            // 4xx/5xx chegam como resposta normal: o corpo explica (politica de dados, modelo
            // ausente) e `status_error` decide a mensagem.
            .http_status_as_error(false)
            // Redirecionar levaria o Bearer e o envelope para onde o destino mandar.
            .max_redirects(0);
        builder = if destination.is_cloud() {
            builder.https_only(true)
        } else {
            // Loopback nunca passa por proxy do ambiente (HTTP_PROXY): o envelope sairia da
            // maquina mesmo com o modelo "local".
            builder.proxy(None)
        };
        OpenAiCompatible {
            destination,
            api_key,
            deny_data_collection: deny,
            agent: builder.build().into(),
        }
    }

    fn auth(&self) -> Result<Option<String>, AssistantError> {
        match (&self.destination, &self.api_key) {
            (Destination::Cloud, None) => Err(AssistantError::MissingKey),
            (Destination::Cloud, Some(k)) => Ok(Some(format!("Bearer {}", k.expose()))),
            (Destination::Local { .. }, _) => Ok(None),
        }
    }
}

pub(crate) fn body(req: &ChatRequest, d: &Destination, deny: bool) -> Value {
    let messages: Vec<Value> = req
        .messages
        .iter()
        .map(|t| json!({ "role": t.role.as_str(), "content": t.content }))
        .collect();
    let mut b = json!({
        "model": req.model,
        "messages": messages,
        "max_tokens": req.max_tokens,
        "stream": true,
        "stream_options": { "include_usage": true },
    });
    if d.is_cloud() {
        b["provider"] = json!({ "data_collection": if deny { "deny" } else { "allow" } });
    }
    b
}

pub(crate) fn status_error(
    status: u16,
    body: &str,
    d: &Destination,
    model: &str,
) -> AssistantError {
    let lower = body.to_lowercase();
    match (status, d) {
        (401 | 403, Destination::Cloud) => AssistantError::BadKey,
        (402, _) | (429, Destination::Cloud) => AssistantError::NoCredit,
        (404, Destination::Cloud) if lower.contains("data policy") => {
            AssistantError::NoProviderForPolicy
        }
        (404, Destination::Local { .. }) if lower.contains("not found") => {
            AssistantError::ModelMissing(model.to_string())
        }
        _ => AssistantError::Other(format!("HTTP {status}")),
    }
}

/// Falha antes de existir resposta (conexao, DNS, TLS, cabecalhos).
pub(crate) fn transport_error(e: &ureq::Error, d: &Destination) -> AssistantError {
    match (d, e) {
        (_, ureq::Error::Timeout(_)) => AssistantError::Timeout,
        (Destination::Local { port }, _) => AssistantError::LocalUnreachable(*port),
        (
            Destination::Cloud,
            ureq::Error::Io(_) | ureq::Error::HostNotFound | ureq::Error::ConnectionFailed,
        ) => AssistantError::Offline,
        (Destination::Cloud, other) => AssistantError::Other(other.to_string()),
    }
}

/// Erro de leitura no meio do stream (a resposta ja comecou): so se sabe que a conexao caiu.
fn stream_failure(d: &Destination) -> AssistantError {
    match d {
        Destination::Local { port } => AssistantError::LocalUnreachable(*port),
        Destination::Cloud => AssistantError::Offline,
    }
}

/// O que a thread da requisicao entrega ao laco que consome o stream.
#[derive(Debug, PartialEq, Eq)]
enum Chunk {
    Data(Vec<u8>),
    End,
    TimedOut,
    Failed,
    /// A requisicao falhou antes de haver stream (conexao, TLS) ou o status nao foi 200; o erro
    /// ja vem traduzido.
    Rejected(AssistantError),
}

/// Tudo o que a thread precisa para fazer a requisicao sozinha.
struct Call {
    agent: ureq::Agent,
    url: String,
    auth: Option<String>,
    body: Value,
    destination: Destination,
    model: String,
}

/// A requisicao inteira (envio, espera dos cabecalhos, corpo do erro, leitura do stream) roda
/// numa thread: `send` e `read` bloqueiam e nao tem como ser interrompidos, e um modelo frio
/// pode levar ate 2 min so para devolver os cabecalhos. Com o canal, `consume_stream` confere o
/// "Parar" desde o primeiro instante e aplica o prazo "sem dado por 60 s". Se o consumidor sumir
/// (cancelou), o proximo `send` falha e a thread encerra, fechando a conexao; no pior caso ela
/// morre no prazo do `ureq` (`RESPONSE_TIMEOUT` ou `BODY_TOTAL_CAP`).
fn spawn_call(call: Call) -> std::io::Result<Receiver<Chunk>> {
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("assistant-sse".into())
        .spawn(move || run_call(call, &tx))?;
    Ok(rx)
}

fn run_call(call: Call, tx: &Sender<Chunk>) {
    let mut request = call.agent.post(&call.url).header("X-Title", APP_TITLE);
    if let Some(a) = &call.auth {
        request = request.header("Authorization", a);
    }
    let resp = match request.send_json(&call.body) {
        Ok(r) => r,
        Err(e) => {
            let _ = tx.send(Chunk::Rejected(transport_error(&e, &call.destination)));
            return;
        }
    };
    let status = resp.status().as_u16();
    let mut reader = resp.into_body().into_reader();
    if status != 200 {
        let mut raw = Vec::new();
        let _ = (&mut reader).take(16 * 1024).read_to_end(&mut raw);
        let text = String::from_utf8_lossy(&raw);
        let _ = tx.send(Chunk::Rejected(status_error(
            status,
            &text,
            &call.destination,
            &call.model,
        )));
        return;
    }
    pump(&mut reader, tx);
}

/// Copia o corpo para o canal ate o fim, um erro ou o consumidor sumir.
fn pump(reader: &mut impl Read, tx: &Sender<Chunk>) {
    let mut buf = [0u8; 4096];
    loop {
        let msg = match reader.read(&mut buf) {
            Ok(0) => Chunk::End,
            Ok(n) => Chunk::Data(buf[..n].to_vec()),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => {
                // O ureq embrulha o proprio erro (o teto do corpo) num `io::Error`.
                let timed_out = e.kind() == std::io::ErrorKind::TimedOut
                    || matches!(ureq::Error::from(e), ureq::Error::Timeout(_));
                if timed_out {
                    Chunk::TimedOut
                } else {
                    Chunk::Failed
                }
            }
        };
        let last = !matches!(msg, Chunk::Data(_));
        if tx.send(msg).is_err() || last {
            return;
        }
    }
}

/// Devolve o maior prefixo UTF-8 valido de `pending` (removendo-o do buffer). Um caractere
/// partido entre duas leituras fica para a proxima; bytes invalidos de verdade viram U+FFFD em
/// vez de travar o buffer para sempre.
fn take_valid_utf8(pending: &mut Vec<u8>) -> String {
    let mut out = String::new();
    loop {
        match std::str::from_utf8(pending) {
            Ok(s) => {
                out.push_str(s);
                pending.clear();
                return out;
            }
            Err(e) => {
                let valid = e.valid_up_to();
                out.push_str(&String::from_utf8_lossy(&pending[..valid]));
                match e.error_len() {
                    Some(bad) => {
                        out.push('\u{FFFD}');
                        pending.drain(..valid + bad);
                    }
                    None => {
                        pending.drain(..valid);
                        return out;
                    }
                }
            }
        }
    }
}

/// Aplica os eventos ao resultado. `Ok(true)` = chegou o `[DONE]`. `at_eof`: o ultimo pedaco
/// de uma conexao cortada pode ser meia linha de JSON; isso nao e erro de formato, e so uma
/// resposta interrompida.
fn apply_events(
    events: Vec<Event>,
    out: &mut ChatOutcome,
    on_delta: &mut dyn FnMut(&str),
    d: &Destination,
    model: &str,
    at_eof: bool,
) -> Result<bool, AssistantError> {
    for ev in events {
        match ev {
            Event::Delta(piece) => {
                on_delta(&piece);
                out.text.push_str(&piece);
            }
            Event::Usage {
                tokens_in,
                tokens_out,
            } => {
                out.tokens_in = Some(tokens_in);
                out.tokens_out = Some(tokens_out);
            }
            Event::Error { code, message } => {
                let status = code.and_then(|c| u16::try_from(c).ok()).unwrap_or(500);
                return Err(match status_error(status, &message, d, model) {
                    // Sem mapeamento conhecido, a mensagem do provedor diz mais que "HTTP 500".
                    AssistantError::Other(_) if !message.is_empty() => {
                        AssistantError::Other(message)
                    }
                    known => known,
                });
            }
            Event::Malformed if at_eof => {}
            Event::Malformed => return Err(AssistantError::Malformed),
            Event::Done => return Ok(true),
        }
    }
    Ok(false)
}

/// Traduz os pedacos de bytes em texto para a UI. `first`: espera maxima pelo primeiro byte
/// (cabecalhos incluidos); `idle`: depois disso, sem dado novo por esse tempo vira `Timeout`.
/// O "Parar" e conferido a cada `CANCEL_POLL`, em qualquer fase. Conexao que fecha sem
/// `[DONE]` devolve o texto ate ali com `interrupted`: a resposta esta incompleta.
fn consume_stream(
    rx: &Receiver<Chunk>,
    first: Duration,
    idle: Duration,
    d: &Destination,
    model: &str,
    on_delta: &mut dyn FnMut(&str),
    cancel: &CancelFlag,
) -> Result<ChatOutcome, AssistantError> {
    let mut parser = SseParser::default();
    let mut out = ChatOutcome::default();
    let mut pending: Vec<u8> = Vec::new();
    let mut budget = first;
    let mut last_data = Instant::now();
    loop {
        if cancel.is_cancelled() {
            out.interrupted = true;
            return Ok(out);
        }
        let left = budget.saturating_sub(last_data.elapsed());
        if left.is_zero() {
            return Err(AssistantError::Timeout);
        }
        let bytes = match rx.recv_timeout(left.min(CANCEL_POLL)) {
            Ok(Chunk::Data(b)) => b,
            Ok(Chunk::End) => {
                // Descarrega a ultima linha, que pode nao ter terminado em `\n`.
                let tail = take_valid_utf8(&mut pending);
                let events = parser.push(&format!("{tail}\n"));
                if !apply_events(events, &mut out, on_delta, d, model, true)? {
                    out.interrupted = true;
                }
                return Ok(out);
            }
            Ok(Chunk::TimedOut) => return Err(AssistantError::Timeout),
            Ok(Chunk::Rejected(e)) => return Err(e),
            Ok(Chunk::Failed) | Err(RecvTimeoutError::Disconnected) => {
                return Err(stream_failure(d));
            }
            Err(RecvTimeoutError::Timeout) => continue,
        };
        budget = idle;
        last_data = Instant::now();
        pending.extend_from_slice(&bytes);
        let text = take_valid_utf8(&mut pending);
        let events = parser.push(&text);
        if apply_events(events, &mut out, on_delta, d, model, false)? {
            return Ok(out);
        }
    }
}

impl Provider for OpenAiCompatible {
    fn list_models(&self) -> Result<Vec<String>, AssistantError> {
        // So o local lista: buscar o catalogo do OpenRouter seria trafego antes de pergunta (Q12).
        if self.destination.is_cloud() {
            return Ok(Vec::new());
        }
        let url = format!("{}/models", self.destination.base_url());
        let mut resp = self
            .agent
            .get(&url)
            .call()
            .map_err(|e| transport_error(&e, &self.destination))?;
        if !resp.status().is_success() {
            return Err(AssistantError::Other(format!(
                "HTTP {}",
                resp.status().as_u16()
            )));
        }
        let v: Value = resp
            .body_mut()
            .read_json()
            .map_err(|_| AssistantError::Malformed)?;
        let mut names: Vec<String> = v["data"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|m| m["id"].as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        Ok(names)
    }

    fn chat(
        &self,
        req: &ChatRequest,
        on_delta: &mut dyn FnMut(&str),
        cancel: &CancelFlag,
    ) -> Result<ChatOutcome, AssistantError> {
        // "Parar" apertado antes de o pedido sair: nada deve ir para a rede (nem, na nuvem,
        // custar tokens) so para ser descartado no primeiro pedaco.
        if cancel.is_cancelled() {
            return Ok(ChatOutcome {
                interrupted: true,
                ..ChatOutcome::default()
            });
        }
        let call = Call {
            agent: self.agent.clone(),
            url: format!("{}/chat/completions", self.destination.base_url()),
            auth: self.auth()?,
            body: body(req, &self.destination, self.deny_data_collection),
            destination: self.destination.clone(),
            model: req.model.clone(),
        };
        let rx = spawn_call(call).map_err(|e| AssistantError::Other(e.to_string()))?;
        consume_stream(
            &rx,
            RESPONSE_TIMEOUT,
            READ_IDLE_TIMEOUT,
            &self.destination,
            &req.model,
            on_delta,
            cancel,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::prompt::{Role, Turn};

    fn req() -> ChatRequest {
        ChatRequest {
            model: "m".into(),
            messages: vec![Turn {
                role: Role::User,
                content: "oi".into(),
            }],
            max_tokens: 1024,
        }
    }

    #[test]
    fn cancelado_antes_de_enviar_nao_faz_pedido() {
        // Porta 1 recusaria a conexao: se o pedido saisse, o resultado seria `Err`.
        let client = OpenAiCompatible::new(Destination::local(1), None, true);
        let cancel = CancelFlag::default();
        cancel.cancel();
        let mut deltas = 0;
        let out = client
            .chat(&req(), &mut |_| deltas += 1, &cancel)
            .expect("interrompida, nao erro");
        assert!(out.interrupted);
        assert_eq!(out.text, "");
        assert_eq!(deltas, 0);
    }

    #[test]
    fn corpo_local_nao_leva_politica_de_provedor() {
        let b = body(&req(), &Destination::local(11434), true);
        assert_eq!(b["stream"], true);
        assert_eq!(b["max_tokens"], 1024);
        assert_eq!(b["messages"][0]["role"], "user");
        assert!(b.get("provider").is_none());
    }

    #[test]
    fn corpo_nuvem_leva_deny_quando_ligado() {
        let b = body(&req(), &Destination::cloud(), true);
        assert_eq!(b["provider"]["data_collection"], "deny");
        let b = body(&req(), &Destination::cloud(), false);
        assert_eq!(b["provider"]["data_collection"], "allow");
    }

    #[test]
    fn status_vira_erro_certo() {
        let d = Destination::cloud();
        assert_eq!(status_error(401, "", &d, "m"), AssistantError::BadKey);
        assert_eq!(status_error(403, "", &d, "m"), AssistantError::BadKey);
        assert_eq!(status_error(402, "", &d, "m"), AssistantError::NoCredit);
        assert_eq!(status_error(429, "", &d, "m"), AssistantError::NoCredit);
        assert_eq!(
            status_error(404, "No endpoints found matching your data policy", &d, "m"),
            AssistantError::NoProviderForPolicy
        );
        let l = Destination::local(11434);
        assert_eq!(
            status_error(404, "model \"m\" not found", &l, "m"),
            AssistantError::ModelMissing("m".into())
        );
        // 401 no local nao e "chave recusada": nao ha chave.
        assert_eq!(
            status_error(401, "", &l, "m"),
            AssistantError::Other("HTTP 401".into())
        );
    }

    #[test]
    fn falha_de_transporte_vira_erro_certo() {
        let local = Destination::local(11434);
        let cloud = Destination::cloud();
        let refused = ureq::Error::Io(std::io::ErrorKind::ConnectionRefused.into());
        assert_eq!(
            transport_error(&refused, &local),
            AssistantError::LocalUnreachable(11434)
        );
        assert_eq!(transport_error(&refused, &cloud), AssistantError::Offline);
        assert_eq!(
            transport_error(&ureq::Error::HostNotFound, &cloud),
            AssistantError::Offline
        );
        let timeout = ureq::Error::Timeout(ureq::Timeout::Connect);
        assert_eq!(transport_error(&timeout, &local), AssistantError::Timeout);
        assert_eq!(transport_error(&timeout, &cloud), AssistantError::Timeout);
        assert!(matches!(
            transport_error(&ureq::Error::BadUri("x".into()), &cloud),
            AssistantError::Other(_)
        ));
    }

    #[test]
    fn caractere_partido_entre_leituras_fica_no_buffer() {
        let bytes = "aé".as_bytes(); // 'é' ocupa 2 bytes
        let mut pending = bytes[..2].to_vec();
        assert_eq!(take_valid_utf8(&mut pending), "a");
        assert_eq!(pending, vec![bytes[1]]);
        pending.push(bytes[2]);
        assert_eq!(take_valid_utf8(&mut pending), "é");
        assert!(pending.is_empty());
    }

    #[test]
    fn byte_invalido_nao_trava_o_buffer() {
        let mut pending = vec![b'a', 0xFF, b'b'];
        assert_eq!(take_valid_utf8(&mut pending), "a\u{FFFD}b");
        assert!(pending.is_empty());
    }

    fn sse(json: &str) -> Chunk {
        Chunk::Data(format!("data: {json}\n\n").into_bytes())
    }

    fn run(
        chunks: Vec<Chunk>,
        idle: Duration,
        cancel: &CancelFlag,
    ) -> (Result<ChatOutcome, AssistantError>, Vec<String>) {
        let (tx, rx) = mpsc::channel();
        for c in chunks {
            tx.send(c).unwrap();
        }
        // `tx` fica vivo ate o fim: sem `End`, o laco tem de acabar pelo prazo ou pelo "Parar".
        let mut seen = Vec::new();
        let r = consume_stream(
            &rx,
            idle,
            idle,
            &Destination::local(11434),
            "m",
            &mut |s| seen.push(s.to_string()),
            cancel,
        );
        drop(tx);
        (r, seen)
    }

    #[test]
    fn stream_completo_junta_texto_e_uso() {
        let (r, seen) = run(
            vec![
                sse(r#"{"choices":[{"delta":{"content":"Ol"}}]}"#),
                sse(r#"{"choices":[{"delta":{"content":"á"}}]}"#),
                sse(r#"{"choices":[],"usage":{"prompt_tokens":9,"completion_tokens":2}}"#),
                Chunk::Data(b"data: [DONE]\n\n".to_vec()),
            ],
            Duration::from_secs(5),
            &CancelFlag::default(),
        );
        let out = r.unwrap();
        assert_eq!(out.text, "Olá");
        assert_eq!((out.tokens_in, out.tokens_out), (Some(9), Some(2)));
        assert!(!out.interrupted);
        assert_eq!(seen, vec!["Ol", "á"]);
    }

    #[test]
    fn sem_dado_pelo_prazo_e_timeout() {
        let (r, _) = run(vec![], Duration::from_millis(150), &CancelFlag::default());
        assert_eq!(r.unwrap_err(), AssistantError::Timeout);
    }

    #[test]
    fn parar_interrompe_mesmo_sem_dado_novo() {
        let cancel = CancelFlag::default();
        cancel.cancel();
        let (r, _) = run(vec![], Duration::from_secs(30), &cancel);
        assert!(r.unwrap().interrupted);
    }

    #[test]
    fn conexao_que_cai_vira_erro_do_destino() {
        let (r, _) = run(
            vec![Chunk::Failed],
            Duration::from_secs(5),
            &CancelFlag::default(),
        );
        assert_eq!(r.unwrap_err(), AssistantError::LocalUnreachable(11434));
        let (r, _) = run(
            vec![Chunk::TimedOut],
            Duration::from_secs(5),
            &CancelFlag::default(),
        );
        assert_eq!(r.unwrap_err(), AssistantError::Timeout);
    }

    #[test]
    fn erro_no_meio_do_stream_usa_o_mapeamento_de_status() {
        let (r, _) = run(
            vec![sse(r#"{"error":{"code":402,"message":"sem saldo"}}"#)],
            Duration::from_secs(5),
            &CancelFlag::default(),
        );
        assert_eq!(r.unwrap_err(), AssistantError::NoCredit);
        let (r, _) = run(
            vec![sse(r#"{"error":{"message":"modelo caiu"}}"#)],
            Duration::from_secs(5),
            &CancelFlag::default(),
        );
        assert_eq!(r.unwrap_err(), AssistantError::Other("modelo caiu".into()));
    }

    #[test]
    fn json_quebrado_no_stream_e_formato_invalido() {
        let (r, _) = run(
            vec![Chunk::Data(b"data: {oops\n\n".to_vec())],
            Duration::from_secs(5),
            &CancelFlag::default(),
        );
        assert_eq!(r.unwrap_err(), AssistantError::Malformed);
    }

    #[test]
    fn ultima_linha_sem_quebra_e_descarregada_no_fim_mas_resposta_fica_interrompida() {
        let (r, seen) = run(
            vec![
                sse(r#"{"choices":[{"delta":{"content":"a"}}]}"#),
                // Sem `\n\n` final e sem `[DONE]`: a conexao fechou no meio.
                Chunk::Data(br#"data: {"choices":[{"delta":{"content":"b"}}]}"#.to_vec()),
                Chunk::End,
            ],
            Duration::from_secs(5),
            &CancelFlag::default(),
        );
        let out = r.unwrap();
        assert_eq!(out.text, "ab");
        assert!(out.interrupted);
        assert_eq!(seen, vec!["a", "b"]);
    }

    #[test]
    fn fim_sem_done_e_interrompido_e_com_done_nao() {
        let (r, _) = run(
            vec![
                sse(r#"{"choices":[{"delta":{"content":"a"}}]}"#),
                Chunk::End,
            ],
            Duration::from_secs(5),
            &CancelFlag::default(),
        );
        assert!(r.unwrap().interrupted);
        let (r, _) = run(
            vec![
                sse(r#"{"choices":[{"delta":{"content":"a"}}]}"#),
                Chunk::Data(b"data: [DONE]\n\n".to_vec()),
                Chunk::End,
            ],
            Duration::from_secs(5),
            &CancelFlag::default(),
        );
        assert!(!r.unwrap().interrupted);
    }

    #[test]
    fn meia_linha_no_fim_nao_vira_erro_de_formato() {
        let (r, _) = run(
            vec![
                Chunk::Data(b"data: {\"choices\":[{\"del".to_vec()),
                Chunk::End,
            ],
            Duration::from_secs(5),
            &CancelFlag::default(),
        );
        let out = r.unwrap();
        assert!(out.interrupted);
        assert!(out.text.is_empty());
    }

    #[test]
    fn requisicao_recusada_chega_como_erro() {
        let (r, _) = run(
            vec![Chunk::Rejected(AssistantError::BadKey)],
            Duration::from_secs(5),
            &CancelFlag::default(),
        );
        assert_eq!(r.unwrap_err(), AssistantError::BadKey);
    }

    #[test]
    fn parar_vale_enquanto_espera_os_cabecalhos() {
        // Canal mudo (a thread ainda esta no `send`, ou no corpo do erro): o "Parar" feito de
        // fora tem de acordar o laco bem antes do prazo de 30 s.
        let (tx, rx) = mpsc::channel::<Chunk>();
        let cancel = CancelFlag::default();
        let remote = cancel.clone();
        let t = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            remote.cancel();
        });
        let started = Instant::now();
        let out = consume_stream(
            &rx,
            Duration::from_secs(30),
            Duration::from_secs(30),
            &Destination::local(11434),
            "m",
            &mut |_| {},
            &cancel,
        )
        .unwrap();
        t.join().unwrap();
        drop(tx);
        assert!(out.interrupted);
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn prazo_do_primeiro_byte_e_separado_do_prazo_ocioso() {
        // Antes de qualquer dado vale `first`; so depois do primeiro pedaco passa a valer `idle`.
        let (tx, rx) = mpsc::channel::<Chunk>();
        tx.send(sse(r#"{"choices":[{"delta":{"content":"a"}}]}"#))
            .unwrap();
        let started = Instant::now();
        let r = consume_stream(
            &rx,
            Duration::from_secs(30),
            Duration::from_millis(150),
            &Destination::local(11434),
            "m",
            &mut |_| {},
            &CancelFlag::default(),
        );
        assert_eq!(r.unwrap_err(), AssistantError::Timeout);
        assert!(started.elapsed() < Duration::from_secs(5));
        drop(tx);
    }

    #[test]
    fn pump_entrega_dados_e_fim() {
        let (tx, rx) = mpsc::channel();
        let mut reader = std::io::Cursor::new(b"abc".to_vec());
        pump(&mut reader, &tx);
        assert_eq!(rx.recv().unwrap(), Chunk::Data(b"abc".to_vec()));
        assert_eq!(rx.recv().unwrap(), Chunk::End);
    }

    #[test]
    fn pump_classifica_erro_de_leitura() {
        struct Failing(std::io::ErrorKind);
        impl Read for Failing {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(self.0.into())
            }
        }
        let (tx, rx) = mpsc::channel();
        pump(&mut Failing(std::io::ErrorKind::TimedOut), &tx);
        assert_eq!(rx.recv().unwrap(), Chunk::TimedOut);
        let (tx, rx) = mpsc::channel();
        pump(&mut Failing(std::io::ErrorKind::ConnectionReset), &tx);
        assert_eq!(rx.recv().unwrap(), Chunk::Failed);
    }

    #[test]
    fn nuvem_sem_chave_falha_antes_de_qualquer_rede() {
        let c = OpenAiCompatible::new(Destination::cloud(), None, true);
        let r = c.chat(&req(), &mut |_| {}, &CancelFlag::default());
        assert_eq!(r.unwrap_err(), AssistantError::MissingKey);
    }

    #[test]
    fn nuvem_nao_lista_modelos_sem_rede() {
        let c = OpenAiCompatible::new(Destination::cloud(), Some(Secret::new("k".into())), true);
        assert_eq!(c.list_models().unwrap(), Vec::<String>::new());
    }
}
