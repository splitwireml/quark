//! Serves the API on loopback with no token, for tests and browser development.

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Context;
use quark_core::api::router;
use quark_core::engine::Dirs;
use quark_core::state::AppState;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

const USAGE: &str = "usage: quark-dev-server [--data-dir DIR] [--cache-dir DIR] [--port PORT]";

struct Options {
    port: u16,
    data: PathBuf,
    cache: Option<PathBuf>,
}

/// `None` means the arguments were not understood and the usage text should be shown.
fn parse(mut args: impl Iterator<Item = String>) -> Option<Options> {
    let mut options = Options {
        port: 8000,
        data: PathBuf::from("data"),
        cache: None,
    };
    while let Some(flag) = args.next() {
        let value = args.next()?;
        match flag.as_str() {
            "--port" => options.port = value.parse().ok()?,
            "--data-dir" => options.data = value.into(),
            "--cache-dir" => options.cache = Some(value.into()),
            _ => return None,
        }
    }
    Some(options)
}

#[tokio::main]
async fn main() -> anyhow::Result<ExitCode> {
    let Some(options) = parse(std::env::args().skip(1)) else {
        eprintln!("{USAGE}");
        return Ok(ExitCode::from(2));
    };
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cache = options.cache.unwrap_or_else(|| options.data.join("cache"));
    let state = AppState::load(Dirs {
        data: options.data,
        cache,
    })
    .context("loading app state")?;
    let listener = TcpListener::bind(("127.0.0.1", options.port))
        .await
        .context("binding the loopback port")?;
    let address = listener.local_addr().context("reading the bound address")?;
    println!("QUARK_LISTENING {address}");
    std::io::stdout().flush().context("flushing stdout")?;

    axum::serve(listener, router(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .context("serving")?;
    Ok(ExitCode::SUCCESS)
}
