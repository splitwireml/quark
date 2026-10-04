use std::error::Error;
use std::net::TcpListener;
use std::path::{Path, PathBuf};

use quark_core::api::router;
use quark_core::engine::Dirs;
use quark_core::secure::{SecurityConfig, desktop_origins, generate_token, secure};
use quark_core::state::AppState;
use tauri::{App, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{Builder, Rotation};

type SetupResult<T = ()> = Result<T, Box<dyn Error>>;

/// Builds and runs the Tauri app: starts the guarded local API, then opens the main window.
#[allow(
    clippy::expect_used,
    reason = "a failed Tauri startup is unrecoverable"
)]
pub fn run() {
    tauri::Builder::default()
        .setup(start)
        .build(tauri::generate_context!())
        .expect("error while building the Quark desktop app")
        .run(|handle, event| {
            if let RunEvent::Exit = event
                && let Some(state) = handle.try_state::<AppState>()
            {
                state.shutdown();
            }
        });
}

fn start(app: &mut App) -> SetupResult {
    let paths = app.path();
    let data = match std::env::var_os("QUARK_DATA_DIR") {
        Some(folder) => PathBuf::from(folder),
        None => paths.app_local_data_dir()?,
    };
    let cache = paths.app_cache_dir()?;
    let log_guard = init_logging(&paths.app_log_dir()?)?;
    app.manage(log_guard);

    let state = AppState::load(Dirs { data, cache })?;
    app.manage(state.clone());

    let listener = TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let port = listener.local_addr()?.port();
    let token = generate_token()?;
    let api = secure(
        router(state),
        SecurityConfig {
            port,
            token: token.clone(),
            allowed_origins: desktop_origins(cfg!(debug_assertions)),
        },
    );
    tauri::async_runtime::spawn(async move {
        let served = async { axum::serve(tokio::net::TcpListener::from_std(listener)?, api).await };
        if let Err(error) = served.await {
            tracing::error!(%error, "the local API stopped");
        }
    });

    open_window(app, port, &token)?;
    tracing::info!("listening on 127.0.0.1:{port}");
    Ok(())
}

/// Writes `quark.log.<date>` files, keeping the newest three. The guard flushes on drop.
fn init_logging(folder: &Path) -> SetupResult<WorkerGuard> {
    let appender = Builder::new()
        .rotation(Rotation::DAILY)
        .filename_prefix("quark.log")
        .max_log_files(3)
        .build(folder)?;
    let (writer, guard) = tracing_appender::non_blocking(appender);
    // A subscriber installed earlier in the process keeps working, so the error is ignorable.
    tracing_subscriber::fmt()
        .with_writer(writer)
        .with_ansi(false)
        .try_init()
        .ok();
    Ok(guard)
}

fn open_window(app: &App, port: u16, token: &str) -> SetupResult {
    let script = format!(
        "window.__QUARK_API__ = Object.freeze({{ base: {}, token: {} }});",
        serde_json::json!(format!("http://127.0.0.1:{port}")),
        serde_json::json!(token),
    );
    let origins = desktop_origins(cfg!(debug_assertions));
    WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("Quark")
        .inner_size(1440.0, 900.0)
        .min_inner_size(1024.0, 680.0)
        .initialization_script(script)
        .on_navigation(move |url| {
            let origin = format!("{}://{}", url.scheme(), url.authority());
            origins
                .iter()
                .any(|allowed| allowed.as_bytes() == origin.as_bytes())
        })
        .build()?;
    Ok(())
}
