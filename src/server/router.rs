//! Rotas da API HTTPS.

use axum::Router;
use axum::routing::{get, post};

use super::state::AppState;
use super::{info, pair};

/// Rotas `/v1/*`, sem CORS nem limites: quem sobe o servidor aplica as camadas.
pub fn api_router(state: AppState) -> Router {
    Router::new()
        .route("/v1/info", get(info::info))
        .route("/v1/pair", post(pair::pair))
        .route("/v1/me", get(info::me))
        .with_state(state)
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, SystemTime};

    use axum::body::Body;
    use axum::http::{Request, StatusCode, header};
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use tokio::sync::mpsc;
    use tower::ServiceExt;

    use super::*;
    use crate::pairing::key::hash_key;
    use crate::pairing::token::TokenBook;
    use crate::server::state::ServerEvent;
    use crate::store::Store;

    pub const DEVICE_A: &str = "01HZZZZZZZZZZZZZZZZZZZZZZA";
    pub const DEVICE_B: &str = "01HZZZZZZZZZZZZZZZZZZZZZZB";

    pub struct TestApp {
        pub router: Router,
        pub state: AppState,
        pub events: mpsc::UnboundedReceiver<ServerEvent>,
    }

    pub fn t0() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_759_344_000)
    }

    pub fn test_state() -> (AppState, mpsc::UnboundedReceiver<ServerEvent>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let state = AppState {
            store: Arc::new(Store::open_in_memory().unwrap()),
            tokens: Arc::new(Mutex::new(TokenBook::default())),
            events: tx,
            now: Arc::new(t0),
        };
        (state, rx)
    }

    pub fn test_app() -> TestApp {
        let (state, events) = test_state();
        TestApp {
            router: api_router(state.clone()),
            state,
            events,
        }
    }

    impl TestApp {
        pub fn issue_token(&self) -> String {
            self.state.tokens.lock().unwrap().issue(t0()).value
        }

        pub fn drain_events(&mut self) -> Vec<ServerEvent> {
            let mut out = Vec::new();
            while let Ok(event) = self.events.try_recv() {
                out.push(event);
            }
            out
        }

        /// Pareia `device_id` com um token novo e devolve a chave.
        pub async fn pair(&self, device_id: &str, name: &str) -> String {
            let token = self.issue_token();
            let (status, body) = self
                .call(
                    "POST",
                    "/v1/pair",
                    Some(json!({ "token": token, "deviceId": device_id, "name": name })),
                    None,
                )
                .await;
            assert_eq!(status, StatusCode::CREATED, "{body}");
            body["key"].as_str().unwrap().to_string()
        }

        pub async fn call(
            &self,
            method: &str,
            uri: &str,
            body: Option<Value>,
            key: Option<&str>,
        ) -> (StatusCode, Value) {
            let body = body.map(|b| b.to_string());
            self.raw(method, uri, body, key).await
        }

        pub async fn raw(
            &self,
            method: &str,
            uri: &str,
            body: Option<String>,
            key: Option<&str>,
        ) -> (StatusCode, Value) {
            let mut req = Request::builder().method(method).uri(uri);
            if let Some(key) = key {
                req = req.header(header::AUTHORIZATION, format!("Bearer {key}"));
            }
            let req = match body {
                Some(body) => req
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body)),
                None => req.body(Body::empty()),
            }
            .unwrap();
            let res = self.router.clone().oneshot(req).await.unwrap();
            let status = res.status();
            let bytes = res.into_body().collect().await.unwrap().to_bytes();
            let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
            (status, value)
        }
    }

    #[tokio::test]
    async fn info_e_publico() {
        let app = test_app();
        let (status, body) = app.call("GET", "/v1/info", None, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["name"], "HubFinance");
        assert_eq!(body["protocol"], 1);
        assert_eq!(body["epoch"], app.state.store.epoch());
    }

    #[tokio::test]
    async fn pair_sem_token_ativo_e_401() {
        let app = test_app();
        let (status, body) = app
            .call(
                "POST",
                "/v1/pair",
                Some(json!({ "token": "ABCDEF", "deviceId": DEVICE_A, "name": "A" })),
                None,
            )
            .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["error"], "invalid_token");
    }

    #[tokio::test]
    async fn pair_feliz_grava_hash_e_avisa() {
        let mut app = test_app();
        let token = app.issue_token();
        let (status, body) = app
            .call(
                "POST",
                "/v1/pair",
                Some(json!({ "token": token, "deviceId": DEVICE_A, "name": "  Pixel da Ana " })),
                None,
            )
            .await;
        assert_eq!(status, StatusCode::CREATED);
        let key = body["key"].as_str().unwrap();
        assert_eq!(key.len(), 64);
        assert!(key.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_eq!(body["deviceId"], DEVICE_A);
        assert_eq!(body["hubName"], "HubFinance");
        assert_eq!(body["epoch"], app.state.store.epoch());

        let devices = app.state.store.list_devices().unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].name, "Pixel da Ana");
        assert_eq!(devices[0].key_hash, hash_key(key));
        assert!(matches!(
            app.drain_events().as_slice(),
            [ServerEvent::Paired { device }] if device.device_id == DEVICE_A
        ));

        // Uso unico.
        let (status, _) = app
            .call(
                "POST",
                "/v1/pair",
                Some(json!({ "token": token, "deviceId": DEVICE_B, "name": "B" })),
                None,
            )
            .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn cinco_erros_invalidam_e_avisam() {
        let mut app = test_app();
        let token = app.issue_token();
        let wrong = if token == "000000" {
            "111111"
        } else {
            "000000"
        };
        for _ in 0..5 {
            let (status, body) = app
                .call(
                    "POST",
                    "/v1/pair",
                    Some(json!({ "token": wrong, "deviceId": DEVICE_A, "name": "A" })),
                    None,
                )
                .await;
            assert_eq!(status, StatusCode::UNAUTHORIZED);
            assert_eq!(body["error"], "invalid_token");
        }
        assert!(matches!(
            app.drain_events().as_slice(),
            [ServerEvent::TokenExhausted]
        ));
        let (status, _) = app
            .call(
                "POST",
                "/v1/pair",
                Some(json!({ "token": token, "deviceId": DEVICE_A, "name": "A" })),
                None,
            )
            .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn pair_com_pedido_invalido_e_400_e_nao_gasta_o_token() {
        let app = test_app();
        let token = app.issue_token();
        for body in [
            json!({ "token": token, "deviceId": "curto", "name": "A" }),
            json!({ "token": token, "deviceId": DEVICE_A, "name": "   " }),
            json!({ "token": token, "deviceId": DEVICE_A, "name": "x".repeat(65) }),
            json!({ "token": token, "deviceId": DEVICE_A }),
        ] {
            let (status, resp) = app.call("POST", "/v1/pair", Some(body), None).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{resp}");
            assert_eq!(resp["error"], "invalid_request");
        }
        let (status, resp) = app
            .raw("POST", "/v1/pair", Some("{nao e json".into()), None)
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(resp["error"], "invalid_request");
        // O token continua valendo.
        let (status, _) = app
            .call(
                "POST",
                "/v1/pair",
                Some(json!({ "token": token, "deviceId": DEVICE_A, "name": "A" })),
                None,
            )
            .await;
        assert_eq!(status, StatusCode::CREATED);
    }

    #[tokio::test]
    async fn me_exige_chave_ativa() {
        let mut app = test_app();
        let (status, body) = app.call("GET", "/v1/me", None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["error"], "unauthorized");
        let (status, _) = app.call("GET", "/v1/me", None, Some(&"f".repeat(64))).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        let key = app.pair(DEVICE_A, "Pixel da Ana").await;
        let (status, body) = app.call("GET", "/v1/me", None, Some(&key)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["deviceId"], DEVICE_A);
        assert_eq!(body["name"], "Pixel da Ana");
        assert_eq!(body["pairedAt"], "2025-10-01T18:40:00Z");
        assert_eq!(body["epoch"], app.state.store.epoch());
        let device = &app.state.store.list_devices().unwrap()[0];
        assert!(device.last_seen_at.is_some());

        app.state
            .store
            .revoke_device(DEVICE_A, "2026-10-01T19:00:00Z")
            .unwrap();
        app.drain_events();
        let (status, body) = app.call("GET", "/v1/me", None, Some(&key)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["error"], "unauthorized");
        assert!(matches!(
            app.drain_events().as_slice(),
            [ServerEvent::RevokedAttempt { name }] if name == "Pixel da Ana"
        ));
    }
}
