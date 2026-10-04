//! Transport self-test: with `QUARK_SELF_TEST=1` the page calls the guarded API from inside the
//! webview, and the process exits 0 once the API saw the final request or 1 after 30 seconds.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use axum::Router;
use axum::http::StatusCode;
use axum::routing::post;
use tauri::{AppHandle, Runtime, WebviewWindow};

const TIMEOUT: Duration = Duration::from_secs(30);
const POLL: Duration = Duration::from_millis(100);

/// Runs in the page: an authenticated read, then a POST that tells the API the transport works.
const SCRIPT: &str = "(async () => {
  const { base, token } = window.__QUARK_API__;
  const headers = { Authorization: `Bearer ${token}` };
  const projects = await fetch(`${base}/api/projects`, { headers });
  if (!projects.ok) throw new Error(`GET /api/projects answered ${projects.status}`);
  await projects.json();
  const done = await fetch(`${base}/api/self-test`, { method: 'POST', headers });
  if (!done.ok) throw new Error(`POST /api/self-test answered ${done.status}`);
})();";

#[derive(Clone)]
pub(crate) struct SelfTest {
    passed: Arc<AtomicBool>,
}

impl SelfTest {
    pub(crate) fn from_env() -> Option<Self> {
        (std::env::var_os("QUARK_SELF_TEST")? == "1").then(|| Self {
            passed: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Adds the route; call before `secure` wraps the router so the same guards apply.
    pub(crate) fn mount(&self, api: Router) -> Router {
        let passed = Arc::clone(&self.passed);
        api.route(
            "/api/self-test",
            post(move || async move {
                passed.store(true, Ordering::SeqCst);
                StatusCode::NO_CONTENT
            }),
        )
    }

    /// Polls on a plain thread, so no async runtime or lock is involved, then exits the app.
    pub(crate) fn watch<R: Runtime>(&self, app: AppHandle<R>) {
        let passed = Arc::clone(&self.passed);
        std::thread::spawn(move || {
            let deadline = Instant::now() + TIMEOUT;
            while !passed.load(Ordering::SeqCst) && Instant::now() < deadline {
                std::thread::sleep(POLL);
            }
            let is_passed = passed.load(Ordering::SeqCst);
            tracing::info!(is_passed, "transport self-test finished");
            app.exit(i32::from(!is_passed));
        });
    }
}

/// Starts the page-side checks once the page has loaded.
pub(crate) fn run_in<R: Runtime>(window: &WebviewWindow<R>) {
    if let Err(error) = window.eval(SCRIPT) {
        tracing::error!(%error, "could not start the transport self-test");
    }
}
