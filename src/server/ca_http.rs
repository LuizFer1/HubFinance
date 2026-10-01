//! Porta HTTP simples (7778): a CA e o guia de instalacao. Existe porque a CA tem que ser
//! baixavel antes de o celular confiar no HTTPS. Sem CORS: e navegacao, nao `fetch`.

use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex, PoisonError, RwLock};

use axum::Router;
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use tokio::sync::mpsc;

use super::state::{Clock, ServerEvent};
use crate::config::PWA_URL;
use crate::pairing::token::{TokenBook, TokenError};

#[derive(Clone)]
pub struct CaState {
    pub ca_pem: Arc<String>,
    pub tokens: Arc<Mutex<TokenBook>>,
    /// O nucleo atualiza quando o IP muda; o link do passo 3 tem que apontar para o atual.
    pub primary_ip: Arc<RwLock<Option<Ipv4Addr>>>,
    pub https_port: u16,
    pub now: Clock,
    /// Para avisar o nucleo quando o guia invalida o token (a janela tira o QR da tela).
    pub events: mpsc::UnboundedSender<ServerEvent>,
}

pub fn ca_router(state: CaState) -> Router {
    Router::new()
        .route("/", get(guide))
        .route("/ca.crt", get(ca_crt))
        .route("/p/{token}", get(pair_guide))
        .with_state(state)
}

async fn ca_crt(State(state): State<CaState>) -> Response {
    (
        [
            (header::CONTENT_TYPE, "application/x-x509-ca-cert"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"HubFinance-CA.crt\"",
            ),
        ],
        state.ca_pem.as_str().to_owned(),
    )
        .into_response()
}

fn primary_ip(state: &CaState) -> Option<Ipv4Addr> {
    // Lock envenenado so significa que um escritor caiu no meio; o valor ainda serve.
    *state
        .primary_ip
        .read()
        .unwrap_or_else(PoisonError::into_inner)
}

async fn guide(State(state): State<CaState>) -> Html<String> {
    Html(guide_html(primary_ip(&state), state.https_port, None))
}

/// So mostra o token se ele for o ativo: quem chega aqui por um QR velho ve "expirado", e nao
/// um link que vai falhar no app. Ver a pagina nao consome o token; errar gasta do orcamento
/// proprio do guia (`check_guide`), que acaba invalidando o token.
async fn pair_guide(State(state): State<CaState>, Path(token): Path<String>) -> Response {
    let checked = state
        .tokens
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .check_guide(&token, (state.now)());
    if checked == Err(TokenError::Exhausted) {
        // Receiver caido = nucleo fechando: o aviso so se perde.
        let _ = state.events.send(ServerEvent::TokenExhausted);
    }
    if checked.is_ok() {
        let token = TokenBook::normalize(&token);
        Html(guide_html(
            primary_ip(&state),
            state.https_port,
            Some(&token),
        ))
        .into_response()
    } else {
        (StatusCode::NOT_FOUND, Html(expired_html())).into_response()
    }
}

const STYLE: &str = "body{font-family:system-ui,sans-serif;max-width:40rem;margin:0 auto;\
padding:1rem;line-height:1.5;background:#111;color:#eee}a{color:#8ab4f8}\
.btn{display:block;text-align:center;padding:1rem;margin:1rem 0;border-radius:.5rem;\
background:#2b5cd6;color:#fff;text-decoration:none;font-size:1.2rem}\
code{background:#222;padding:.1rem .3rem;border-radius:.2rem}h2{margin-top:2rem}";

fn page(body: &str) -> String {
    format!(
        "<!doctype html><html lang=\"pt-BR\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
<title>HubFinance</title><style>{STYLE}</style></head><body>{body}</body></html>"
    )
}

/// Pagina unica, sem JS nem recurso externo: o celular ainda nao confia em nada alem dela.
/// `pair` so e `Some` com o token ativo (validado pelo chamador), que e Crockford — nao ha o
/// que escapar.
pub fn guide_html(ip: Option<Ipv4Addr>, https_port: u16, pair: Option<&str>) -> String {
    let host = ip.map_or_else(|| "127.0.0.1".to_string(), |ip| ip.to_string());
    let address = format!("{host}:{https_port}");
    let mut body = format!(
        "<h1>HubFinance</h1>\
<p>Para o HomeFinance sincronizar com este computador, o celular precisa confiar no \
certificado do hub. Isso é feito uma vez só.</p>\
<a class=\"btn\" href=\"/ca.crt\">Baixar certificado</a>\
<h2>Android</h2><ol>\
<li>Toque em <b>Baixar certificado</b> acima.</li>\
<li>Configurações → Segurança → Criptografia e credenciais → Instalar um certificado → \
Certificado de CA → <i>Instalar mesmo assim</i> → escolha <code>HubFinance-CA.crt</code>.</li>\
</ol>\
<h2>iPhone</h2><ol>\
<li>No Safari, toque em <b>Baixar certificado</b> acima → <i>Permitir</i>.</li>\
<li>Ajustes → Geral → VPN e Gerenciamento de Dispositivo → <i>HubFinance CA</i> → Instalar.</li>\
<li>Ajustes → Geral → Sobre → Ajustes de Confiança de Certificado → ative \
<i>HubFinance CA</i>.</li>\
</ol>\
<p>Endereço do hub no app: <code>{address}</code></p>"
    );
    match pair {
        Some(token) => body.push_str(&format!(
            "<h2>3. Abrir o HomeFinance</h2>\
<p>Com o certificado instalado, abra o app já com o endereço e o código preenchidos:</p>\
<a class=\"btn\" href=\"{PWA_URL}?hub={address}&token={token}\">Abrir o HomeFinance</a>\
<p>Ou digite no app o endereço <code>{address}</code> e o código <code>{token}</code>.</p>"
        )),
        None => body.push_str(
            "<p>Depois, abra o HomeFinance e leia o QR mostrado na janela do hub (ou digite o \
endereço e o código).</p>",
        ),
    }
    page(&body)
}

pub fn expired_html() -> String {
    page(
        "<h1>Código expirado</h1>\
<p>Código expirado — gere outro no hub e leia o QR de novo.</p>\
<p><a href=\"/\">Ver só o guia do certificado</a></p>",
    )
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use super::*;

    const PEM: &str = "-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----\n";

    fn t0() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_759_344_000)
    }

    fn state() -> CaState {
        CaState {
            ca_pem: Arc::new(PEM.to_string()),
            tokens: Arc::new(Mutex::new(TokenBook::default())),
            primary_ip: Arc::new(RwLock::new(Some(Ipv4Addr::new(192, 168, 0, 5)))),
            https_port: 7777,
            now: Arc::new(t0),
            events: mpsc::unbounded_channel().0,
        }
    }

    async fn get(state: &CaState, uri: &str) -> (StatusCode, header::HeaderMap, String) {
        let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
        let res = ca_router(state.clone()).oneshot(req).await.unwrap();
        let status = res.status();
        let headers = res.headers().clone();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        (status, headers, String::from_utf8(bytes.to_vec()).unwrap())
    }

    #[tokio::test]
    async fn ca_crt_e_baixavel() {
        let (status, headers, body) = get(&state(), "/ca.crt").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers[header::CONTENT_TYPE], "application/x-x509-ca-cert");
        assert!(
            headers[header::CONTENT_DISPOSITION]
                .to_str()
                .unwrap()
                .contains("HubFinance-CA.crt")
        );
        assert_eq!(body, PEM);
    }

    #[tokio::test]
    async fn guia_publico_nao_tem_link_de_pareamento() {
        let (status, headers, body) = get(&state(), "/").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers[header::CONTENT_TYPE], "text/html; charset=utf-8");
        assert!(body.contains("Baixar certificado"));
        assert!(body.contains("/ca.crt"));
        assert!(body.contains("192.168.0.5:7777"));
        assert!(!body.contains("luizfer1.github.io/homefinance/?hub="));
    }

    #[tokio::test]
    async fn guia_com_token_ativo_tem_o_deep_link() {
        let state = state();
        let token = state.tokens.lock().unwrap().issue(t0()).value;
        let (status, _, body) = get(&state, &format!("/p/{token}")).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains(&format!(
            "https://luizfer1.github.io/homefinance/?hub=192.168.0.5:7777&token={token}"
        )));

        // Minusculo com hifen e o mesmo codigo.
        let typed = format!(
            "{}-{}",
            token[..3].to_lowercase(),
            token[3..].to_lowercase()
        );
        let (status, _, _) = get(&state, &format!("/p/{typed}")).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn abrir_o_guia_nao_consome_nem_gasta_tentativa() {
        let state = state();
        let token = state.tokens.lock().unwrap().issue(t0()).value;
        let wrong = if token == "000000" {
            "111111"
        } else {
            "000000"
        };
        for _ in 0..6 {
            assert_eq!(
                get(&state, &format!("/p/{wrong}")).await.0,
                StatusCode::NOT_FOUND
            );
            assert_eq!(get(&state, &format!("/p/{token}")).await.0, StatusCode::OK);
        }
        // O token continua vivo e com as cinco tentativas.
        assert_eq!(state.tokens.lock().unwrap().consume(&token, t0()), Ok(()));
    }

    #[tokio::test]
    async fn vinte_erros_no_guia_matam_o_token() {
        let (events, mut rx) = mpsc::unbounded_channel();
        let state = CaState { events, ..state() };
        let token = state.tokens.lock().unwrap().issue(t0()).value;
        let wrong = if token == "000000" {
            "111111"
        } else {
            "000000"
        };
        for _ in 0..21 {
            assert_eq!(
                get(&state, &format!("/p/{wrong}")).await.0,
                StatusCode::NOT_FOUND
            );
        }
        // O 22o, mesmo certo, ja nao abre o guia, e o pareamento tambem nao aceita mais.
        assert_eq!(
            get(&state, &format!("/p/{token}")).await.0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            state.tokens.lock().unwrap().consume(&token, t0()),
            Err(TokenError::Missing)
        );
        assert!(matches!(rx.try_recv(), Ok(ServerEvent::TokenExhausted)));
        assert!(rx.try_recv().is_err(), "um aviso so");
    }

    #[tokio::test]
    async fn token_inexistente_ou_expirado_e_404() {
        let state = state();
        let (status, _, body) = get(&state, "/p/ABCDEF").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body.contains("Código expirado"));

        // Emitido ha 6 minutos: ja venceu.
        let token = state
            .tokens
            .lock()
            .unwrap()
            .issue(t0() - Duration::from_secs(6 * 60))
            .value;
        let (status, _, body) = get(&state, &format!("/p/{token}")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body.contains("Código expirado"));
    }
}
