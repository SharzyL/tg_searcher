//! TG Searcher - Telegram message search bot
//!
//! A server to provide Telegram message searching with full-text search
//! and Chinese word segmentation support.

mod backend;
mod config;
mod frontend;
mod indexer;
mod session;
mod storage;
mod types;
mod utils;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use tracing::{error, info, warn};

#[derive(Parser, Debug)]
#[command(name = "tg-searcher")]
#[command(about = "A server to provide Telegram message searching")]
#[command(version)]
struct Args {
    /// Clear existing index
    #[arg(long)]
    clear: bool,

    /// Path to config file
    #[arg(short = 'c', long, default_value = "searcher.yaml")]
    config: PathBuf,

    /// Enable debug logging
    #[arg(long)]
    debug: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    init_logging(args.debug);

    info!("Starting tg-searcher, reading config {:?}", args.config);
    if args.clear {
        warn!("Will clear existing index");
    }

    // Load configuration
    let config = config::Config::from_file(&args.config)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to load config: {}", e))?;

    // Ensure directories exist
    config
        .common
        .ensure_dirs_exist()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create directories: {}", e))?;

    // Initialize sessions
    let mut sessions = std::collections::HashMap::new();
    for session_config in &config.sessions {
        let session_file = config
            .common
            .session_dir()
            .join(format!("{}.session", session_config.name));

        let session = session::ClientSession::new(
            &session_file,
            session_config.name.clone(),
            config.common.api_id,
            &config.common.api_hash,
            config.common.parse_proxy(),
        )
        .await
        .map_err(|e| {
            anyhow::anyhow!("Failed to create session '{}': {}", session_config.name, e)
        })?;

        // Start the session (login)
        session.start(&session_config.phone).await.map_err(|e| {
            anyhow::anyhow!("Failed to start session '{}': {}", session_config.name, e)
        })?;

        // Populate access hashes by fetching all dialogs
        // This ensures backends can access channels without warnings
        session.populate_access_hashes().await.map_err(|e| {
            anyhow::anyhow!(
                "Failed to populate access hashes for session '{}': {}",
                session_config.name,
                e
            )
        })?;

        sessions.insert(session_config.name.clone(), std::sync::Arc::new(session));
    }

    info!("Created {} session(s)", sessions.len());

    // Initialize backends with indexers
    let mut backends = std::collections::HashMap::new();
    let mut backend_tasks = Vec::new();

    for backend_config in &config.backends {
        // Get the session for this backend
        let session = sessions
            .get(&backend_config.use_session)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Backend '{}' references unknown session '{}'",
                    backend_config.id,
                    backend_config.use_session
                )
            })?
            .clone();

        // Create indexer for this backend
        let index_dir = config.common.index_dir().join(&backend_config.id);
        let indexer = std::sync::Arc::new(
            indexer::Indexer::new(&index_dir, args.clear)
                .await
                .map_err(|e| {
                    anyhow::anyhow!(
                        "Failed to create indexer for '{}': {}",
                        backend_config.id,
                        e
                    )
                })?,
        );

        // Create backend
        let backend =
            backend::BackendBot::new(&backend_config.id, backend_config, session, indexer)
                .await
                .map_err(|e| {
                    anyhow::anyhow!("Failed to create backend '{}': {}", backend_config.id, e)
                })?;

        let backend_arc = std::sync::Arc::new(backend);
        backends.insert(backend_config.id.clone(), backend_arc.clone());
    }

    info!("Created {} backend(s)", backends.len());

    // Initialize and start all backends
    for backend in backends.values() {
        backend.initialize().await.map_err(|e| {
            anyhow::anyhow!("Failed to initialize backend '{}': {}", backend.id(), e)
        })?;

        // Spawn backend event loop
        let backend_clone = backend.clone();
        let backend_id = backend.id().to_string();
        backend_tasks.push(tokio::spawn(async move {
            if let Err(e) = backend_clone.run().await {
                error!("Backend '{}' event loop error: {}", backend_id, e);
            }
        }));
    }

    info!("Started all backend event loops");

    // Initialize frontends with storage
    let mut frontend_tasks = Vec::new();
    let mut frontend_count = config.frontends.len();

    for frontend_config in &config.frontends {
        // Get the backend for this frontend
        let backend = backends
            .get(&frontend_config.use_backend)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Frontend '{}' references unknown backend '{}'",
                    frontend_config.id,
                    frontend_config.use_backend
                )
            })?
            .clone();

        // Create storage for this frontend (in-memory)
        let storage: std::sync::Arc<dyn storage::Storage> =
            std::sync::Arc::new(storage::InMemoryStorage::new());

        // Create frontend
        let mut frontend = frontend::BotFrontend::new(
            &frontend_config.id,
            frontend_config,
            backend,
            storage,
            &config.common,
        )
        .await
        .map_err(|e| {
            anyhow::anyhow!("Failed to create frontend '{}': {}", frontend_config.id, e)
        })?;

        // Initialize frontend (authenticate bot)
        frontend.initialize().await.map_err(|e| {
            anyhow::anyhow!(
                "Failed to initialize frontend '{}': {}",
                frontend_config.id,
                e
            )
        })?;

        // Spawn frontend event loop
        let frontend_id = frontend_config.id.clone();
        frontend_tasks.push(tokio::spawn(async move {
            if let Err(e) = frontend.run().await {
                error!("Frontend '{}' event loop error: {}", frontend_id, e);
            }
        }));
        frontend_count += 1;
    }

    info!("Created {} frontend(s)", frontend_count);

    if frontend_count == 0 {
        return Err(anyhow::anyhow!("No frontends configured"));
    }

    info!("Initialization complete. Press Ctrl+C to stop.");

    // Wait for Ctrl+C signal
    tokio::signal::ctrl_c().await?;

    info!("Shutting down...");

    // Note: Background tasks will be automatically cancelled when main exits
    // In a production system, you might want to handle graceful shutdown of tasks

    Ok(())
}

fn init_logging(debug: bool) {
    use tracing_subscriber::{EnvFilter, fmt, prelude::*};

    // Set app logs to info/debug, but suppress verbose third-party logs
    let default_filter = "grammers_mtsender=warn,grammers_mtproto=warn,grammers_client=warn,grammers_session=warn,tantivy=warn";
    let filter = if debug {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new(format!("info,{}", default_filter))
    };

    let stdout_layer = fmt::layer();

    tracing_subscriber::registry()
        .with(filter)
        .with(stdout_layer)
        .init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_args_parsing() {
        // Test default values
        let args = Args::parse_from(&["tg-searcher"]);
        assert!(!args.clear);
        assert_eq!(args.config, PathBuf::from("searcher.yaml"));
        assert!(!args.debug);

        // Test with flags
        let args = Args::parse_from(&["tg-searcher", "--clear", "--debug", "-c", "test.yaml"]);
        assert!(args.clear);
        assert_eq!(args.config, PathBuf::from("test.yaml"));
        assert!(args.debug);
    }
}
