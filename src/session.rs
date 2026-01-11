//! Telegram session management
//!
//! This module provides session storage and authentication helpers.

use crate::config::ProxyConfig;
use crate::types::{Error, Result};
use crate::utils::get_share_id;
use dashmap::DashMap;
use grammers_client::{Client, SignInError};
use grammers_mtsender::{ConnectionParams, SenderPool};
use grammers_session::storages::SqliteSession;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use tracing::info;

/// Telegram session configuration
pub struct ClientSession {
    /// Session name for logging
    name: String,

    /// SQLite session storage
    session_storage: Arc<SqliteSession>,

    /// API ID
    api_id: i32,

    /// API hash
    api_hash: String,

    /// Proxy configuration
    proxy: Option<String>,

    /// Chat ID to name cache (populated during access hash population)
    chat_cache: Arc<DashMap<i64, String>>,
}

impl ClientSession {
    /// Create a new session
    pub async fn new(
        session_file: &Path,
        name: String,
        api_id: i32,
        api_hash: &str,
        proxy: Option<ProxyConfig>,
    ) -> Result<Self> {
        info!("Creating session: {}", name);

        // Load or create SQLite session
        let session_storage = Arc::new(
            SqliteSession::open(session_file)
                .map_err(|e| Error::Config(format!("Failed to open session: {}", e)))?,
        );

        // Convert proxy config to URL string
        let proxy_url = if let Some(p) = proxy {
            // Validate proxy scheme - grammers only supports socks5
            if p.scheme.starts_with("http") {
                return Err(Error::Config(format!(
                    "HTTP proxy is not supported by grammers. Please use SOCKS5 proxy instead. \
                    Current config: {}://{}:{}",
                    p.scheme, p.host, p.port
                )));
            }

            let url = if let (Some(username), Some(password)) = (p.username, p.password) {
                format!(
                    "{}://{}:{}@{}:{}",
                    p.scheme, username, password, p.host, p.port
                )
            } else {
                format!("{}://{}:{}", p.scheme, p.host, p.port)
            };

            info!("Using proxy: {}", url);
            Some(url)
        } else {
            None
        };

        Ok(Self {
            name,
            session_storage,
            api_id,
            api_hash: api_hash.to_string(),
            proxy: proxy_url,
            chat_cache: Arc::new(DashMap::new()),
        })
    }

    /// Authenticate the session if needed
    pub async fn start(&self, phone: &str) -> Result<()> {
        info!("Authenticating session: {}", self.name);

        // Create temporary client for authentication
        let pool = if let Some(ref proxy_url) = self.proxy {
            let params = ConnectionParams {
                proxy_url: Some(proxy_url.clone()),
                ..Default::default()
            };
            SenderPool::with_configuration(Arc::clone(&self.session_storage), self.api_id, params)
        } else {
            SenderPool::new(Arc::clone(&self.session_storage), self.api_id)
        };
        let client = Client::new(&pool);
        let SenderPool { runner, .. } = pool;

        // Spawn runner
        let runner_task = tokio::spawn(runner.run());

        // Check if already authorized
        if client
            .is_authorized()
            .await
            .map_err(|e| Error::Telegram(format!("Failed to check authorization: {}", e)))?
        {
            info!("Session {} is already authorized", self.name);
            return Ok(());
        }

        info!("Session {} needs authentication", self.name);

        // Request login code
        let token = client
            .request_login_code(phone, &self.api_hash)
            .await
            .map_err(|e| Error::Telegram(format!("Failed to request login code: {}", e)))?;

        // Prompt for code
        eprint!("Enter the verification code sent to {}: ", phone);
        std::io::stderr().flush().map_err(Error::Io)?;

        let mut code = String::new();
        std::io::stdin().read_line(&mut code).map_err(Error::Io)?;

        // Sign in with code
        match client.sign_in(&token, code.trim()).await {
            Ok(_) => {
                info!("Signed in successfully");
            }
            Err(SignInError::PasswordRequired(password_token)) => {
                // 2FA required
                let hint = password_token.hint().unwrap_or("None");
                let password = rpassword::prompt_password(format!(
                    "Enter your 2FA password (hint: {}): ",
                    hint
                ))
                .map_err(Error::Io)?;

                client
                    .check_password(password_token, password.trim())
                    .await
                    .map_err(|e| {
                        Error::Telegram(format!("Password authentication failed: {}", e))
                    })?;

                info!("Signed in successfully with 2FA");
            }
            Err(e) => {
                return Err(Error::Telegram(format!("Sign in failed: {}", e)));
            }
        }

        // Cleanup: drop client and abort runner task
        drop(client);
        runner_task.abort();

        Ok(())
    }

    /// Populate access hashes and chat name cache by fetching dialogs
    /// Should be called after authentication to ensure clients can access channels
    pub async fn populate_access_hashes(&self) -> Result<usize> {
        info!(
            "Populating access hashes and chat cache for session: {}",
            self.name
        );

        // Create temporary client for fetching dialogs
        let pool = if let Some(ref proxy_url) = self.proxy {
            let params = ConnectionParams {
                proxy_url: Some(proxy_url.clone()),
                ..Default::default()
            };
            SenderPool::with_configuration(Arc::clone(&self.session_storage), self.api_id, params)
        } else {
            SenderPool::new(Arc::clone(&self.session_storage), self.api_id)
        };
        let client = Client::new(&pool);
        let SenderPool { runner, .. } = pool;

        // Spawn runner
        let runner_task = tokio::spawn(runner.run());

        // Fetch all dialogs to populate access hashes and chat names
        let mut dialog_count = 0;
        let mut dialogs = client.iter_dialogs();
        while let Some(dialog) = dialogs
            .next()
            .await
            .map_err(|e| Error::Telegram(format!("Failed to iterate dialogs: {}", e)))?
        {
            dialog_count += 1;

            // Populate chat name cache
            let peer = dialog.peer();
            let chat_id = peer.id().bot_api_dialog_id();
            let share_id = get_share_id(chat_id);
            if let Some(name) = peer.name() {
                self.chat_cache.insert(share_id, name.to_string());
            }
        }

        info!(
            "Populated access hashes and {} chat names for session {}",
            self.chat_cache.len(),
            self.name
        );

        // Cleanup: drop client and abort runner task
        drop(client);
        runner_task.abort();

        Ok(dialog_count)
    }

    /// Get session name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get session storage
    pub fn session_storage(&self) -> Arc<SqliteSession> {
        Arc::clone(&self.session_storage)
    }

    /// Get API ID
    pub fn api_id(&self) -> i32 {
        self.api_id
    }

    /// Get API hash
    pub fn api_hash(&self) -> &str {
        &self.api_hash
    }

    /// Get proxy URL
    pub fn proxy(&self) -> Option<&String> {
        self.proxy.as_ref()
    }

    /// Get chat name cache
    pub fn chat_cache(&self) -> Arc<DashMap<i64, String>> {
        Arc::clone(&self.chat_cache)
    }
}
