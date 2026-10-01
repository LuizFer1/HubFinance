//! HTTP(S): API do hub e porta da CA; unico modulo que conhece axum.

pub mod auth;
pub mod ca_http;
pub mod errors;
pub mod info;
pub mod pair;
pub mod router;
pub mod state;
pub mod sync;
