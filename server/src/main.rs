//! Monica Cloud — self-hosted, zero-knowledge sync server for Monica Pass.
//!
//! Layout follows vaultwarden: `config` (env), `db` (diesel models and
//! migrations), `auth` (tokens and guards), `api` (identity, core, admin, web).

mod api;
mod auth;
mod config;
mod crypto;
mod db;
mod error;
mod jobs;
mod ratelimit;
mod storage;
mod util;

use std::{net::SocketAddr, process::exit, sync::Arc, time::Instant};

use tracing_subscriber::EnvFilter;

pub use config::CONFIG;

use crate::{api::AppState, db::Db, ratelimit::RateLimiter, storage::Storage};

#[tokio::main]
async fn main() {
    let env_file = std::env::var("ENV_FILE").unwrap_or_else(|_| ".env".into());
    let env_loaded = dotenvy::from_path(&env_file).is_ok();

    tracing_subscriber::fmt()
        // Plain output in containers and log files; colors only on an interactive terminal.
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stdout()))
        .with_env_filter(
            EnvFilter::try_from_env("LOG_LEVEL")
                .unwrap_or_else(|_| EnvFilter::new(format!("{},tower_http=warn", CONFIG.log_level))),
        )
        .init();

    tracing::info!("Monica Cloud {} starting", config::VERSION);
    if env_loaded {
        tracing::info!("loaded environment from {env_file}");
    }

    if let Err(e) = std::fs::create_dir_all(&CONFIG.data_folder) {
        fail(format!("cannot create data folder {}: {e}", CONFIG.data_folder.display()));
    }
    if let Err(e) = auth::init_keys() {
        fail(format!("cannot initialise token signing key: {e}"));
    }
    let db = Db::connect(&CONFIG.database_url).unwrap_or_else(|e| fail(e));
    if let Err(e) = db.run(db::models::ServerSettings::load).await {
        fail(format!("cannot load settings: {e}"));
    }
    let storage = Storage::new(CONFIG.blobs_folder(), CONFIG.tmp_folder())
        .unwrap_or_else(|e| fail(format!("cannot prepare blob storage: {e}")));

    let state = AppState {
        db,
        storage,
        login_limiter: Arc::new(RateLimiter::new(CONFIG.login_ratelimit_per_minute)),
        started_at: Instant::now(),
    };
    jobs::spawn(state.clone());

    let addr = format!("{}:{}", CONFIG.address, CONFIG.port);
    let listener =
        tokio::net::TcpListener::bind(&addr).await.unwrap_or_else(|e| fail(format!("cannot listen on {addr}: {e}")));
    tracing::info!("listening on http://{addr}");
    if !CONFIG.web_vault_folder.join("index.html").exists() {
        tracing::warn!("WebUI not found in {}; only the API is served", CONFIG.web_vault_folder.display());
    }

    let app = api::router(state).into_make_service_with_connect_info::<SocketAddr>();
    if let Err(e) = axum::serve(listener, app).with_graceful_shutdown(shutdown_signal()).await {
        fail(format!("server error: {e}"));
    }
    tracing::info!("shut down cleanly");
}

fn fail(message: impl std::fmt::Display) -> ! {
    tracing::error!("{message}");
    eprintln!("Error: {message}");
    exit(1)
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            signal.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
