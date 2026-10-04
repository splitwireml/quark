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
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode, header};
    use axum::routing::get;
    use tower::ServiceExt;

    const APP_ORIGIN: &str = "tauri://localhost";

    fn guarded() -> Router {
        let config = SecurityConfig {
            port: 4242,
            token: "secret".into(),
            allowed_origins: desktop_origins(false),
        };
        secure(
            Router::new().route("/ping", get(|| async { "pong" })),
            config,
        )
    }

    #[test]
    fn debug_output_redacts_the_token() {
        let config = SecurityConfig {
            port: 4242,
            token: "0123456789abcdef".into(),
            allowed_origins: desktop_origins(false),
        };
        let output = format!("{config:?}");
        assert!(output.contains("<redacted>"));
        assert!(!output.contains("0123456789abcdef"));
    }

    fn get_request(host: &str, origin: Option<&str>, bearer: Option<&str>) -> Request<Body> {
        let mut builder = Request::get("/ping").header(header::HOST, host);
        if let Some(origin) = origin {
            builder = builder.header(header::ORIGIN, origin);
        }
        if let Some(bearer) = bearer {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {bearer}"));
        }
        builder.body(Body::empty()).unwrap()
    }

    async fn detail(response: axum::response::Response) -> String {
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        body["detail"].as_str().unwrap().to_owned()
    }

    #[tokio::test]
    async fn preflight_from_app_origin_passes() {
        let request = Request::options("/ping")
            .header(header::HOST, "127.0.0.1:4242")
            .header(header::ORIGIN, APP_ORIGIN)
            .header(header::ACCESS_CONTROL_REQUEST_METHOD, "DELETE")
            .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "authorization")
            .body(Body::empty())
            .unwrap();
        let response = guarded().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let headers = response.headers();
        assert_eq!(headers[header::ACCESS_CONTROL_ALLOW_ORIGIN], APP_ORIGIN);
        assert_eq!(headers[header::ACCESS_CONTROL_MAX_AGE], "86400");
        let methods = headers[header::ACCESS_CONTROL_ALLOW_METHODS]
            .to_str()
            .unwrap();
        assert!(
            ["GET", "POST", "DELETE"]
                .iter()
                .all(|m| methods.contains(m))
        );
        let allowed = headers[header::ACCESS_CONTROL_ALLOW_HEADERS]
            .to_str()
            .unwrap();
        assert!(
            ["authorization", "content-type", "accept"]
                .iter()
                .all(|h| allowed.contains(h))
        );
    }

    #[test]
    fn desktop_origins_are_exact() {
        let release: Vec<_> = desktop_origins(false)
            .iter()
            .map(|origin| origin.to_str().unwrap().to_owned())
            .collect();
        assert_eq!(release, ["tauri://localhost", "http://tauri.localhost"]);
        let debug: Vec<_> = desktop_origins(true)
            .iter()
            .map(|origin| origin.to_str().unwrap().to_owned())
            .collect();
        assert_eq!(
            debug,
            [
                "tauri://localhost",
                "http://tauri.localhost",
                "http://localhost:5173"
            ]
        );
    }

    #[tokio::test]
    async fn preflight_from_windows_app_origin_passes() {
        let request = Request::options("/ping")
            .header(header::HOST, "127.0.0.1:4242")
            .header(header::ORIGIN, "http://tauri.localhost")
            .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
            .body(Body::empty())
            .unwrap();
        let response = guarded().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN],
            "http://tauri.localhost"
        );
    }

    #[test]
    fn navigation_stays_on_app_origin() {
        assert!(is_app_origin("tauri://localhost/x", false));
        assert!(is_app_origin("http://tauri.localhost/x", false));
        assert!(!is_app_origin("https://evil.example", false));
        assert!(!is_app_origin("http://127.0.0.1:1234", false));
        assert!(!is_app_origin("http://localhost:5173", false));
        assert!(is_app_origin("http://localhost:5173", true));
        assert!(is_app_origin("http://localhost:5173/index.html?a=b", true));
    }

    #[tokio::test]
    async fn foreign_origin_gets_no_cors_headers() {
        let request = get_request(
            "127.0.0.1:4242",
            Some("https://evil.example"),
            Some("secret"),
        );
        let response = guarded().oneshot(request).await.unwrap();
        assert!(
            !response
                .headers()
                .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        );

        let preflight = Request::options("/ping")
            .header(header::HOST, "127.0.0.1:4242")
            .header(header::ORIGIN, "https://evil.example")
            .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
            .body(Body::empty())
            .unwrap();
        let response = guarded().oneshot(preflight).await.unwrap();
        assert!(
            !response
                .headers()
                .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        );
    }

    #[tokio::test]
    async fn wrong_host_is_403() {
        for host in ["evil.example:4242", "127.0.0.1:1", "localhost:4242"] {
            let response = guarded()
                .oneshot(get_request(host, None, Some("secret")))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::FORBIDDEN, "{host}");
            assert_eq!(detail(response).await, "Forbidden host");
        }
    }

    #[tokio::test]
    async fn wrong_host_without_token_is_forbidden() {
        let response = guarded()
            .oneshot(get_request("evil.example:1234", None, None))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(detail(response).await, "Forbidden host");
    }

    #[tokio::test]
    async fn missing_or_wrong_token_is_401_with_cors_headers() {
        for bearer in [None, Some("wrong"), Some("secre"), Some("secrets")] {
            let response = guarded()
                .oneshot(get_request("127.0.0.1:4242", Some(APP_ORIGIN), bearer))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{bearer:?}");
            assert_eq!(
                response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN],
                APP_ORIGIN
            );
            assert_eq!(detail(response).await, "Unauthorized");
        }
    }

    #[tokio::test]
    async fn valid_request_passes() {
        let response = guarded()
            .oneshot(get_request(
                "127.0.0.1:4242",
                Some(APP_ORIGIN),
                Some("secret"),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(&bytes[..], b"pong");
    }

    #[test]
    fn token_is_64_hex_and_unique() {
        let (first, second) = (generate_token().unwrap(), generate_token().unwrap());
        assert_eq!(first.len(), 64);
        assert!(
            first
                .bytes()
                .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
        );
        assert_ne!(first, second);
    }
}
