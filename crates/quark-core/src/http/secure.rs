//! Request guards for the loopback API: CORS, Host and bearer-token checks.

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use axum::Router;
use axum::extract::{Request, State};
use axum::http::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HOST};
use axum::http::{HeaderValue, Method};
use axum::middleware::{Next, from_fn_with_state};
use axum::response::{IntoResponse, Response};
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::error::ApiError;

#[derive(Clone)]
pub struct SecurityConfig {
    pub port: u16,
    pub token: String,
    pub allowed_origins: Vec<HeaderValue>,
}

impl fmt::Debug for SecurityConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecurityConfig")
            .field("port", &self.port)
            .field("token", &"<redacted>")
            .field("allowed_origins", &self.allowed_origins)
            .finish()
    }
}

/// 32 random bytes as 64 lowercase hex characters.
pub fn generate_token() -> anyhow::Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).context("could not read system randomness")?;
    Ok(bytes
        .iter()
        .flat_map(|byte| [byte >> 4, byte & 0xf])
        .filter_map(|nibble| char::from_digit(u32::from(nibble), 16))
        .collect())
}

/// Origins the packaged webview (and, in debug builds, the Vite dev server) sends.
pub fn desktop_origins(debug: bool) -> Vec<HeaderValue> {
    let mut origins = vec![
        HeaderValue::from_static("tauri://localhost"),
        HeaderValue::from_static("http://tauri.localhost"),
    ];
    if debug {
        origins.push(HeaderValue::from_static("http://localhost:5173"));
    }
    origins
}

/// True when `url` stays on one of the desktop origins; the webview may navigate there.
pub fn is_app_origin(url: &str, debug: bool) -> bool {
    let after_scheme = url.find("://").map_or(0, |at| at + 3);
    let end = url[after_scheme..]
        .find(['/', '?', '#'])
        .map_or(url.len(), |at| after_scheme + at);
    let origin = &url[..end];
    desktop_origins(debug)
        .iter()
        .any(|allowed| allowed.as_bytes() == origin.as_bytes())
}

/// Layers, innermost first: token check, Host check, CORS.
pub fn secure(router: Router, config: SecurityConfig) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(config.allowed_origins.clone()))
        .allow_methods([Method::GET, Method::POST, Method::DELETE])
        .allow_headers([AUTHORIZATION, CONTENT_TYPE, ACCEPT])
        .max_age(Duration::from_secs(86400));
    let config = Arc::new(config);
    router
        .layer(from_fn_with_state(Arc::clone(&config), require_token))
        .layer(from_fn_with_state(config, require_host))
        .layer(cors)
}

async fn require_host(
    State(config): State<Arc<SecurityConfig>>,
    request: Request,
    next: Next,
) -> Response {
    let expected = format!("127.0.0.1:{}", config.port);
    match request.headers().get(HOST) {
        Some(host) if host.as_bytes() == expected.as_bytes() => next.run(request).await,
        _ => ApiError::forbidden("Forbidden host").into_response(),
    }
}

async fn require_token(
    State(config): State<Arc<SecurityConfig>>,
    request: Request,
    next: Next,
) -> Response {
    let presented = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    match presented {
        Some(token) if constant_time_eq(token.as_bytes(), config.token.as_bytes()) => {
            next.run(request).await
        }
        _ => ApiError::unauthorized("Unauthorized").into_response(),
    }
}

/// Compares every byte once the lengths match, so timing reveals only the length.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |diff, (x, y)| diff | (x ^ y)) == 0
}

#[cfg(test)]
mod tests;
