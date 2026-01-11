//! Configuration types for TG Searcher

use crate::types::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tokio::fs;

/// Full configuration loaded from YAML
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub common: CommonConfig,
    pub sessions: Vec<SessionConfig>,
    pub backends: Vec<BackendConfig>,
    pub frontends: Vec<FrontendConfig>,
}

/// Common configuration shared across components
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CommonConfig {
    /// Application name
    pub name: String,

    /// Runtime directory for storing sessions and indexes
    pub runtime_dir: PathBuf,

    /// Telegram API ID
    pub api_id: i32,

    /// Telegram API hash
    pub api_hash: String,

    /// Optional proxy configuration (format: "scheme://host:port" or "scheme://user:pass@host:port")
    #[serde(default)]
    pub proxy: Option<String>,
}

impl CommonConfig {
    /// Get the session directory path
    pub fn session_dir(&self) -> PathBuf {
        self.runtime_dir.join(&self.name).join("session")
    }

    /// Get the index directory path
    pub fn index_dir(&self) -> PathBuf {
        self.runtime_dir.join(&self.name).join("index")
    }

    /// Ensure all required directories exist
    pub async fn ensure_dirs_exist(&self) -> Result<()> {
        let base = self.runtime_dir.join(&self.name);
        fs::create_dir_all(&base).await?;
        fs::create_dir_all(self.session_dir()).await?;
        fs::create_dir_all(self.index_dir()).await?;
        Ok(())
    }

    /// Parse proxy string into components
    ///
    /// Supports formats:
    /// - "socks5://host:port"
    /// - "socks5://user:pass@host:port"
    ///
    /// Note: HTTP proxies are NOT supported by grammers and will be rejected during session creation.
    pub fn parse_proxy(&self) -> Option<ProxyConfig> {
        self.proxy.as_ref().map(|proxy_str| {
            // Simple URL parsing - in production might want to use url crate
            let parts: Vec<&str> = proxy_str.split("://").collect();
            if parts.len() != 2 {
                return ProxyConfig {
                    scheme: "socks5".to_string(),
                    host: "localhost".to_string(),
                    port: 1080,
                    username: None,
                    password: None,
                };
            }

            let scheme = parts[0].to_string();
            let rest = parts[1];

            // Check for authentication
            let (auth, host_port) = if let Some(at_pos) = rest.rfind('@') {
                let auth_part = &rest[..at_pos];
                let host_part = &rest[at_pos + 1..];

                let auth_parts: Vec<&str> = auth_part.split(':').collect();
                let (username, password) = if auth_parts.len() == 2 {
                    (
                        Some(auth_parts[0].to_string()),
                        Some(auth_parts[1].to_string()),
                    )
                } else {
                    (None, None)
                };

                ((username, password), host_part)
            } else {
                ((None, None), rest)
            };

            // Parse host and port
            let host_parts: Vec<&str> = host_port.split(':').collect();
            let host = host_parts[0].to_string();
            let port = if host_parts.len() == 2 {
                host_parts[1].parse().unwrap_or(1080)
            } else if scheme == "http" {
                8080
            } else {
                1080
            };

            ProxyConfig {
                scheme,
                host,
                port,
                username: auth.0,
                password: auth.1,
            }
        })
    }
}

/// Proxy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub scheme: String,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
}

/// Session configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SessionConfig {
    /// Session name (used as filename)
    pub name: String,

    /// Phone number for authentication
    pub phone: String,
}

/// Backend configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BackendConfig {
    /// Backend ID (unique identifier)
    pub id: String,

    /// Session to use for this backend
    pub use_session: String,

    /// Optional backend-specific config
    #[serde(default)]
    pub config: BackendBotConfig,
}

/// Backend bot configuration
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct BackendBotConfig {
    /// Monitor all chats except excluded ones
    #[serde(default)]
    pub monitor_all: bool,

    /// Chat IDs to exclude from monitoring (when monitor_all is true)
    #[serde(default)]
    pub excluded_chats: HashSet<i64>,
}

/// Frontend configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FrontendConfig {
    /// Frontend ID (unique identifier)
    pub id: String,

    /// Backend to use for this frontend
    pub use_backend: String,

    /// Frontend-specific config
    pub config: BotFrontendConfig,

    /// Frontend type (kept for compatibility, only "bot" supported)
    #[serde(default = "default_frontend_type")]
    #[serde(rename = "type")]
    pub frontend_type: String,
}

fn default_frontend_type() -> String {
    "bot".to_string()
}

/// Bot frontend configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BotFrontendConfig {
    /// Telegram bot token
    pub bot_token: String,

    /// Admin user ID
    pub admin_id: i64,

    /// Number of results per page
    #[serde(default = "default_page_len")]
    pub page_len: usize,

    /// Disable in-memory storage (no pagination state)
    #[serde(default)]
    pub no_storage: bool,

    /// Private mode (only allow whitelisted users)
    #[serde(default)]
    pub private_mode: bool,

    /// Whitelist of allowed user IDs (when private_mode is true)
    #[serde(default)]
    pub private_whitelist: HashSet<i64>,
}

fn default_page_len() -> usize {
    10
}

impl Config {
    /// Load configuration from a YAML file
    pub async fn from_file(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path).await?;
        let config: Config = serde_yaml::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }

    /// Validate configuration
    fn validate(&self) -> Result<()> {
        // Check for duplicate session names
        let mut session_names = HashSet::new();
        for session in &self.sessions {
            if !session_names.insert(&session.name) {
                return Err(Error::Config(format!(
                    "Duplicate session name: {}",
                    session.name
                )));
            }
        }

        // Check for duplicate backend IDs
        let mut backend_ids = HashSet::new();
        for backend in &self.backends {
            if !backend_ids.insert(&backend.id) {
                return Err(Error::Config(format!(
                    "Duplicate backend ID: {}",
                    backend.id
                )));
            }

            // Check that referenced session exists
            if !session_names.contains(&backend.use_session) {
                return Err(Error::Config(format!(
                    "Backend {} references non-existent session: {}",
                    backend.id, backend.use_session
                )));
            }
        }

        // Check for duplicate frontend IDs
        let mut frontend_ids = HashSet::new();
        for frontend in &self.frontends {
            if !frontend_ids.insert(&frontend.id) {
                return Err(Error::Config(format!(
                    "Duplicate frontend ID: {}",
                    frontend.id
                )));
            }

            // Check that referenced backend exists
            if !backend_ids.contains(&frontend.use_backend) {
                return Err(Error::Config(format!(
                    "Frontend {} references non-existent backend: {}",
                    frontend.id, frontend.use_backend
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proxy_parsing() {
        let config = CommonConfig {
            name: "test".to_string(),
            runtime_dir: PathBuf::from("/tmp"),
            api_id: 123,
            api_hash: "abc".to_string(),
            proxy: Some("socks5://user:pass@localhost:1080".to_string()),
        };

        let proxy = config.parse_proxy().unwrap();
        assert_eq!(proxy.scheme, "socks5");
        assert_eq!(proxy.host, "localhost");
        assert_eq!(proxy.port, 1080);
        assert_eq!(proxy.username, Some("user".to_string()));
        assert_eq!(proxy.password, Some("pass".to_string()));
    }

    #[test]
    fn test_proxy_parsing_no_auth() {
        let config = CommonConfig {
            name: "test".to_string(),
            runtime_dir: PathBuf::from("/tmp"),
            api_id: 123,
            api_hash: "abc".to_string(),
            proxy: Some("socks5://localhost:1080".to_string()),
        };

        let proxy = config.parse_proxy().unwrap();
        assert_eq!(proxy.scheme, "socks5");
        assert_eq!(proxy.host, "localhost");
        assert_eq!(proxy.port, 1080);
        assert!(proxy.username.is_none());
        assert!(proxy.password.is_none());
    }

    #[test]
    fn test_http_proxy_parsing() {
        // HTTP proxy can be parsed but will be rejected during session creation
        let config = CommonConfig {
            name: "test".to_string(),
            runtime_dir: PathBuf::from("/tmp"),
            api_id: 123,
            api_hash: "abc".to_string(),
            proxy: Some("http://localhost:8080".to_string()),
        };

        let proxy = config.parse_proxy().unwrap();
        assert_eq!(proxy.scheme, "http");
        assert_eq!(proxy.host, "localhost");
        assert_eq!(proxy.port, 8080);

        // Note: This parsing succeeds, but ClientSession::new() will return an error
        // when it detects an HTTP proxy scheme
    }
}
