//! Core types for TG Searcher

use thiserror::Error;
use tokio::task::JoinError;

// Re-export index types used by other modules in this crate.
pub use tg_searcher_index::HighlightedSnippet;

/// Main error type for TG Searcher
#[derive(Error, Debug)]
pub enum Error {
    #[error("Telegram client error: {0}")]
    Telegram(String),

    #[error("Index error: {0}")]
    Index(#[from] tg_searcher_index::Error),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Entity not found: {0}")]
    EntityNotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML parsing error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("Task join eror: {0}")]
    Join(#[from] JoinError),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Result type alias
pub type Result<T> = std::result::Result<T, Error>;

/// Result of a download_history operation
#[derive(Debug, Clone)]
pub struct DownloadResult {
    /// Number of messages indexed
    pub indexed_count: usize,

    /// Minimum message ID fetched (oldest)
    pub min_msg_id: i32,

    /// Maximum message ID fetched (newest)
    pub max_msg_id: i32,
}

/// Progress update for download_history operation
#[derive(Debug, Clone)]
pub struct DownloadProgress {
    /// Number of messages downloaded so far
    pub downloaded: usize,

    /// Chat ID being downloaded
    pub chat_id: i64,

    /// Latest message ID being processed
    pub latest_msg_id: i32,
}
