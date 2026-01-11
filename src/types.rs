//! Core types for TG Searcher

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::task::JoinError;

/// Main error type for TG Searcher
#[derive(Error, Debug)]
pub enum Error {
    #[error("Telegram client error: {0}")]
    Telegram(String),

    #[error("Index error: {0}")]
    Index(String),

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

/// Message to be indexed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexMsg {
    /// Message text content
    pub content: String,

    /// URL to the message (format: https://t.me/c/{share_id}/{msg_id})
    pub url: String,

    /// Chat ID (normalized share_id)
    pub chat_id: i64,

    /// Message timestamp
    pub post_time: DateTime<Utc>,

    /// Sender's name
    pub sender: String,
}

/// Search result hit with highlighting
#[derive(Debug, Clone)]
pub struct SearchHit {
    /// The indexed message
    pub msg: IndexMsg,

    /// Highlighted content (HTML with highlights)
    pub highlighted: String,
}

/// Search results with pagination info
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Search hits
    pub hits: Vec<SearchHit>,

    /// Whether this is the last page
    pub is_last_page: bool,

    /// Total number of results
    pub total_results: usize,
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
