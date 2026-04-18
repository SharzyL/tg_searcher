//! Frontend bot for user interaction
//!
//! This module implements the Telegram bot that handles user commands
//! and provides search functionality.

// Constants
/// Maximum number of chat buttons to show in /chats command (to avoid hitting Telegram limits)
const MAX_CHAT_BUTTONS: usize = 10;

use crate::backend::BackendBot;
use crate::config::{BotFrontendConfig, FrontendConfig};
use crate::session::ClientSession;
use crate::storage::Storage;
use crate::types::{Result, SearchResult};
use crate::utils::MessageBuilder;
use crate::utils::remove_first_word;
use grammers_client::client::UpdatesConfiguration;
use grammers_client::types::update::{CallbackQuery, Update};
use grammers_client::{Client, InputMessage, button, reply_markup};
use grammers_mtsender::{ConnectionParams, SenderPool};
use grammers_session::defs::{PeerId, PeerKind};
use grammers_tl_types as tl;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, error, info, warn};

/// Callback data for disabled/non-interactive buttons
const NOOP_CALLBACK: &[u8] = b"noop";

/// Bot frontend for user interaction
pub struct BotFrontend {
    /// Frontend ID
    id: String,

    /// Backend bot reference
    backend: Arc<BackendBot>,

    /// Session reference (for API credentials)
    session: Arc<ClientSession>,

    /// Bot client (set during run, used temporarily)
    client: Option<Client>,

    /// Storage for pagination state
    storage: Arc<dyn Storage>,

    /// Configuration
    config: BotFrontendConfig,

    /// Admin user ID
    admin_id: i64,

    /// Bot username (set during run)
    username: Option<String>,

    /// Whether to register bot commands on startup
    register_commands: bool,
}

impl BotFrontend {
    /// Create a new bot frontend
    pub async fn new(
        frontend_id: &str,
        config: &FrontendConfig,
        backend: Arc<BackendBot>,
        storage: Arc<dyn Storage>,
        common_config: &crate::config::CommonConfig,
        register_commands: bool,
    ) -> Result<Self> {
        // Create a separate session for the bot frontend
        let session_file = common_config
            .session_dir()
            .join(format!("frontend_{}.session", frontend_id));

        let session = Arc::new(
            crate::session::ClientSession::new(
                &session_file,
                format!("frontend_{}", frontend_id),
                common_config.api_id,
                &common_config.api_hash,
                common_config.parse_proxy(),
            )
            .await?,
        );

        Ok(Self {
            id: frontend_id.to_string(),
            backend,
            session,
            client: None,
            storage,
            config: config.config.clone(),
            admin_id: config.config.admin_id,
            username: None,
            register_commands,
        })
    }

    /// Initialize the bot (just a placeholder, real init happens in run)
    pub async fn initialize(&mut self) -> Result<()> {
        debug!("[{}] frontend initialized", self.id);
        Ok(())
    }

    /// Run the bot event loop
    pub async fn run(&mut self) -> Result<()> {
        // Create SenderPool and Client for this bot (all in one place)
        let pool = Self::create_sender_pool(&self.session);
        let client = Client::new(&pool);
        let SenderPool {
            runner, updates, ..
        } = pool;

        // Spawn the sender pool runner task
        tokio::spawn(runner.run());

        // Authenticate as bot
        if !client.is_authorized().await.map_err(|e| {
            crate::types::Error::Telegram(format!("Failed to check bot authorization: {}", e))
        })? {
            info!("[{}] bot signing in with token", self.id);
            client
                .bot_sign_in(&self.config.bot_token, self.session.api_hash())
                .await
                .map_err(|e| crate::types::Error::Telegram(format!("Bot sign in failed: {}", e)))?;
        }

        // Get bot info
        let me = client
            .get_me()
            .await
            .map_err(|e| crate::types::Error::Telegram(format!("Failed to get bot info: {}", e)))?;

        let username = me
            .username()
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("bot_{}", self.id));

        info!("[{}] bot authenticated, username: {}", self.id, username);

        // Register bot commands
        if self.register_commands {
            self.register_bot_commands(&client).await;
        }

        // Store client and username
        self.client = Some(client);
        self.username = Some(username.clone());

        // Include `/stat` output in the greeting for quick visibility.
        // Use a smaller limit to leave room for the greeting header/footer within Telegram limits.
        let index_status = match self
            .backend
            .get_index_status(crate::backend::STATUS_MESSAGE_LENGTH_LIMIT.saturating_sub(600))
            .await
        {
            Ok(status) => status,
            Err(e) => {
                warn!(
                    "[{}] failed to generate index status for greeting: {}",
                    self.id, e
                );
                format!(
                    "Backend: <b>{}</b>\nMonitored chats: <b>{}</b>",
                    self.backend.id(),
                    self.backend.monitored_chats_count()
                )
            }
        };

        // Send greeting message to admin
        let greeting = format!(
            "🤖 TG Searcher bot <b>{}</b> is now online!\n\n\
            {}\n\n\
            ⏳ Populating chat cache...",
            username, index_status
        );

        let greeting_msg_id = match self.send_message(self.admin_id, &greeting, None).await {
            Ok(msg_id) => msg_id,
            Err(e) => {
                warn!(
                    "[{}] failed to send greeting message to admin: {}",
                    self.id, e
                );
                -1 // Invalid message ID
            }
        };

        // Spawn task to update greeting when cache is ready
        if greeting_msg_id > 0 {
            let backend = Arc::clone(&self.backend);
            let admin_id = self.admin_id;
            let client = self.client.clone();
            let username_clone = username.clone();
            let index_status_clone = index_status.clone();
            let frontend_id = self.id.clone();

            tokio::spawn(async move {
                // Get cache info (cache is always ready after session initialization)
                let cache_count = backend.get_cache_info();
                let cache_status = format!("✅ Chat cache ready ({} chats)", cache_count);

                // Update greeting message
                let updated_greeting = format!(
                    "🤖 TG Searcher bot <b>{}</b> is now online!\n\n\
                    {}\n\n\
                    {}",
                    username_clone, index_status_clone, cache_status
                );

                // Edit the greeting message
                if let Some(client) = client {
                    use crate::utils::get_share_id;
                    use grammers_client::InputMessage;
                    use grammers_tl_types as tl;

                    // Note: This may fail if admin hasn't started the bot or for group admins
                    // We use access_hash = 0 which works for users who've interacted with the bot
                    let peer = if admin_id > 0 {
                        tl::enums::InputPeer::User(tl::types::InputPeerUser {
                            user_id: admin_id,
                            access_hash: 0,
                        })
                    } else {
                        let channel_id = get_share_id(admin_id);
                        tl::enums::InputPeer::Channel(tl::types::InputPeerChannel {
                            channel_id,
                            access_hash: 0,
                        })
                    };

                    let input = InputMessage::new().html(&updated_greeting);
                    if let Err(e) = client.edit_message(peer, greeting_msg_id, input).await {
                        warn!(
                            "[{}] failed to update greeting message: {}. \
                            If you're the admin, send /start to the bot first.",
                            frontend_id, e
                        );
                    }
                }
            });
        }

        // Create update stream using the stored client
        let client_ref = self.client.as_ref().unwrap();
        let mut updates = client_ref.stream_updates(
            updates,
            UpdatesConfiguration {
                catch_up: true,
                ..Default::default()
            },
        );

        loop {
            match updates.next().await {
                Ok(update) => {
                    match update {
                        Update::NewMessage(message) if !message.outgoing() => {
                            if let Err(e) = self.handle_update_message(message).await {
                                error!("[{}] error handling bot message: {}", self.id, e);
                            }
                        }
                        Update::CallbackQuery(query) => {
                            if let Err(e) = self.handle_update_callback(query).await {
                                error!("[{}] error handling bot callback: {}", self.id, e);
                            }
                        }
                        _ => {
                            // Ignore other update types
                        }
                    }
                }
                Err(e) => {
                    error!("[{}] error getting bot update: {}", self.id, e);
                    break;
                }
            }
        }

        warn!("[{}] event loop exited", self.id);
        Ok(())
    }

    /// Handle incoming bot message
    async fn handle_update_message(
        &self,
        message: grammers_client::types::update::Message,
    ) -> Result<()> {
        let text = message.text();
        if text.is_empty() {
            return Ok(());
        }

        // Get chat info - use peer_id().bot_api_dialog_id() like in backend
        let peer_id = message.peer_id();
        let chat_id = peer_id.bot_api_dialog_id();
        let is_private = peer_id.kind() == PeerKind::User;

        // Get sender info
        let sender_id = if let Some(sender_peer) = message.sender() {
            sender_peer.id().bot_api_dialog_id()
        } else {
            warn!("[{}] message without sender", self.id);
            return Ok(());
        };

        // Check private mode and whitelist (admin is always allowed)
        if self.config.private_mode
            && sender_id != self.admin_id
            && !self.config.private_whitelist.contains(&sender_id)
        {
            warn!(
                "[{}] unauthorized user {} tried to use bot",
                self.id, sender_id
            );
            return Ok(());
        }

        let reply_to = message.reply_to_message_id();

        // Route to admin or normal handler, catch errors and send to user
        let result = if sender_id == self.admin_id {
            self.handle_admin_message(chat_id, is_private, text, reply_to)
                .await
        } else {
            self.handle_normal_message(chat_id, is_private, text, reply_to)
                .await
        };

        if let Err(e) = result {
            error!("[{}] error handling message: {}", self.id, e);
            // Format error message for user (simplify technical jargon)
            let error_msg = match &e {
                crate::types::Error::EntityNotFound(entity) => {
                    format!("❌ Not found: {}", entity)
                }
                crate::types::Error::Config(msg) => {
                    format!("❌ {}", msg)
                }
                _ => {
                    format!("❌ Error: {}", e)
                }
            };
            if let Err(send_err) = self.send_message(chat_id, &error_msg, None).await {
                error!(
                    "[{}] failed to send error message to user: {}",
                    self.id, send_err
                );
            }
        }

        Ok(())
    }

    /// Handle callback query (button press)
    async fn handle_update_callback(&self, query: CallbackQuery) -> Result<()> {
        // Extract callback data
        let data = query.data();
        if data.is_empty() {
            return Ok(());
        }

        let data_str = String::from_utf8_lossy(data);

        // Get chat ID and message ID from raw update
        let (chat_id, message_id) = match &query.raw {
            tl::enums::Update::BotCallbackQuery(update) => {
                let peer_id: PeerId = update.peer.clone().into();
                (peer_id.bot_api_dialog_id(), update.msg_id)
            }
            _ => {
                warn!("[{}] callback query not from bot", self.id);
                return Ok(());
            }
        };

        debug!(
            "[{}] callback query from {}: {}",
            self.id, chat_id, data_str
        );

        // Answer the callback query to remove loading state
        if let Err(e) = query.answer().send().await {
            warn!("[{}] failed to answer callback query: {}", self.id, e);
        }

        // Handle the callback
        self.handle_callback(chat_id, message_id, &data_str).await?;

        Ok(())
    }

    /// Handle callback query (button press)
    async fn handle_callback(&self, chat_id: i64, message_id: i32, data: &str) -> Result<()> {
        // Ignore noop callbacks (disabled buttons)
        if data == std::str::from_utf8(NOOP_CALLBACK).unwrap_or("noop") {
            debug!("[{}] ignoring noop callback from chat {}", self.id, chat_id);
            return Ok(());
        }

        info!(
            "[{}] callback ({}) from chat {}, data={}",
            self.id, message_id, chat_id, data
        );

        let parts: Vec<&str> = data.split('=').collect();
        if parts.len() != 2 {
            warn!("[{}] invalid callback data: {}", self.id, data);
            return Ok(());
        }

        match parts[0] {
            "search_page" => {
                let page_num: usize = parts[1].parse().unwrap_or(1);
                self.handle_search_page(chat_id, message_id, page_num)
                    .await?;
            }
            "select_chat" => {
                let chat_id_selected: i64 = parts[1].parse().unwrap_or(0);
                self.handle_select_chat(chat_id, message_id, chat_id_selected)
                    .await?;
            }
            _ => {
                warn!("[{}] unknown callback data: {}", self.id, data);
            }
        }

        Ok(())
    }

    /// Handle search pagination
    async fn handle_search_page(
        &self,
        chat_id: i64,
        message_id: i32,
        page_num: usize,
    ) -> Result<()> {
        // Retrieve query from storage
        let query_key = format!("{}:query_text:{}:{}", self.id, chat_id, message_id);
        let chats_key = format!("{}:query_chats:{}:{}", self.id, chat_id, message_id);

        let query = self.storage.get(&query_key).await?;
        let chats_str = self.storage.get(&chats_key).await?;

        if let Some(q) = query {
            let chats: Option<Vec<i64>> =
                chats_str.map(|s| s.split(',').filter_map(|id| id.parse().ok()).collect());

            info!(
                "[{}] query [{}] (chats={:?}) turned to page {}",
                self.id, q, chats, page_num
            );

            let start_time = Instant::now();
            let result = self
                .backend
                .search(&q, chats.as_deref(), self.config.page_len, page_num)
                .await?;
            let used_time = start_time.elapsed().as_secs_f64();

            let buttons = self.render_buttons(&result, page_num);
            let message = self
                .render_response_message(&result, used_time, buttons)
                .await?;

            // Edit message with new page
            self.edit_input_message(chat_id, message_id, message)
                .await?;
            info!(
                "[{}] updated search results to page {} ({} results)",
                self.id, page_num, result.total_results
            );
        }

        Ok(())
    }

    /// Handle chat selection
    async fn handle_select_chat(
        &self,
        chat_id: i64,
        message_id: i32,
        selected_chat_id: i64,
    ) -> Result<()> {
        let chat_name = self.backend.translate_chat_id(selected_chat_id).await?;
        let response = format!(
            "Reply to this message to operate on {} ({})",
            chat_name, selected_chat_id
        );

        // Store selected chat
        let key = format!("{}:select_chat:{}:{}", self.id, chat_id, message_id);
        self.storage
            .set(&key, &selected_chat_id.to_string())
            .await?;

        // Edit message
        self.edit_message(chat_id, message_id, &response, None)
            .await?;
        debug!(
            "[{}] selected chat: {} ({})",
            self.id, chat_name, selected_chat_id
        );

        Ok(())
    }

    /// Handle normal user message
    async fn handle_normal_message(
        &self,
        chat_id: i64,
        is_private: bool,
        text: &str,
        reply_to: Option<i32>,
    ) -> Result<()> {
        info!(
            "[{}] handling message in chat {}: {}",
            self.id, chat_id, text
        );

        let trimmed = text.trim();

        if trimmed.is_empty() || trimmed.starts_with("/start") {
            return Ok(());
        } else if trimmed.starts_with("/random") {
            self.handle_random(chat_id).await?;
        } else if trimmed.starts_with("/chats") {
            self.handle_chats(chat_id, trimmed).await?;
        } else if trimmed.starts_with("/search") {
            self.handle_search(chat_id, 0, trimmed, reply_to).await?;
        } else if trimmed.starts_with("/") {
            let cmd = trimmed.split_whitespace().next().unwrap_or("");
            let response = format!("❌ Unknown command: {}", cmd);
            self.send_message(chat_id, &response, None).await?;
            warn!("[{}] unknown command: {}", self.id, cmd);
        } else if is_private {
            // Plain text search (only in private chats)
            self.handle_search(chat_id, 0, trimmed, reply_to).await?;
        }

        Ok(())
    }

    /// Handle admin message
    async fn handle_admin_message(
        &self,
        chat_id: i64,
        is_private: bool,
        text: &str,
        reply_to: Option<i32>,
    ) -> Result<()> {
        let trimmed = text.trim();

        if trimmed.starts_with("/stat") {
            self.handle_stat(chat_id).await?;
        } else if trimmed.starts_with("/download_chat") {
            self.handle_download_chat(chat_id, trimmed, reply_to)
                .await?;
        } else if trimmed.starts_with("/monitor_chat") {
            self.handle_monitor_chat(chat_id, trimmed, reply_to).await?;
        } else if trimmed.starts_with("/clear") {
            self.handle_clear(chat_id, trimmed, reply_to).await?;
        } else if trimmed.starts_with("/refresh_chat_names") {
            self.handle_refresh_chat_names(chat_id).await?;
        } else if trimmed.starts_with("/find_chat_id") {
            self.handle_find_chat_id(chat_id, trimmed).await?;
        } else {
            // Fallback to normal handler
            self.handle_normal_message(chat_id, is_private, text, reply_to)
                .await?;
        }

        Ok(())
    }

    /// /random - Get random message
    async fn handle_random(&self, chat_id: i64) -> Result<()> {
        match self.backend.rand_msg().await? {
            Some(msg) => {
                let chat_name = self.backend.translate_chat_id(msg.chat_id).await?;
                let response = format!(
                    "Random message: <b>{} [{}]</b>\n{}",
                    chat_name, msg.post_time, msg.url
                );
                self.send_message(chat_id, &response, None).await?;
                info!("[{}] sent random message from {}", self.id, chat_name);
            }
            None => {
                let response = "❌ Index is empty";
                self.send_message(chat_id, response, None).await?;
                info!("[{}] index is empty", self.id);
            }
        }
        Ok(())
    }

    /// /chats - List monitored chats
    async fn handle_chats(&self, chat_id: i64, text: &str) -> Result<()> {
        let keyword = remove_first_word(text);

        if self.backend.monitored_chats_count() == 0 {
            let response =
                "No monitored chats. Use /download_chat or /monitor_chat to start monitoring";
            self.send_message(chat_id, response, None).await?;
            return Ok(());
        }

        // Get all monitored chats
        let all_chats = self.backend.get_monitored_chats().await?;

        // Filter by keyword if provided
        let chats: Vec<_> = if keyword.is_empty() {
            all_chats
        } else {
            let keyword_lower = keyword.to_lowercase();
            all_chats
                .into_iter()
                .filter(|(_, name)| name.to_lowercase().contains(&keyword_lower))
                .collect()
        };

        if chats.is_empty() {
            let response = if keyword.is_empty() {
                "No monitored chats found.".to_string()
            } else {
                format!("No monitored chats matching \"{}\"", keyword)
            };
            self.send_message(chat_id, &response, None).await?;
            return Ok(());
        }

        // Create response with inline buttons (limit to MAX_CHAT_BUTTONS to avoid hitting Telegram limits)
        let display_chats = &chats[..chats.len().min(MAX_CHAT_BUTTONS)];

        let mut response = if keyword.is_empty() {
            format!("Monitored chats ({}):\n\n", chats.len())
        } else {
            format!(
                "Monitored chats matching \"{}\" ({}):\n\n",
                keyword,
                chats.len()
            )
        };

        response.push_str("Select a chat to search within it:");

        // Create inline buttons - one per row
        let buttons: Vec<Vec<(String, String)>> = display_chats
            .iter()
            .map(|(chat_id, chat_name)| {
                vec![(chat_name.to_string(), format!("select_chat={}", chat_id))]
            })
            .collect();

        if chats.len() > MAX_CHAT_BUTTONS {
            response.push_str(&format!(
                "\n\nShowing first {} of {} chats. Use /chats <keyword> to filter.",
                MAX_CHAT_BUTTONS,
                chats.len()
            ));
        }

        self.send_message(chat_id, &response, Some(buttons)).await?;
        info!(
            "[{}] sent chat list with {} buttons",
            self.id,
            display_chats.len()
        );

        Ok(())
    }

    /// /search or plain text - Search messages
    async fn handle_search(
        &self,
        chat_id: i64,
        _message_id: i32,
        text: &str,
        reply_to: Option<i32>,
    ) -> Result<()> {
        if self.backend.is_empty(None).await? {
            let response = "Index is empty. Please use /download_chat to build the index first";
            self.send_message(chat_id, response, None).await?;
            return Ok(());
        }

        // Parse query
        let mut query = text.to_string();
        if query.starts_with('/') || query.starts_with('@') {
            if let Some(space_pos) = query.find(' ') {
                query = query[space_pos + 1..].to_string();
            } else {
                query.clear();
            }
        }

        if query.is_empty() {
            return Ok(());
        }

        // Get selected chat from reply
        let chats = self.query_selected_chat(chat_id, reply_to).await?;

        info!(
            "[{}] search \"{}\" within chats {:?} (None means all)",
            self.id, query, chats
        );

        let start_time = Instant::now();
        let result = self
            .backend
            .search(&query, chats.as_deref(), self.config.page_len, 1)
            .await?;
        let used_time = start_time.elapsed().as_secs_f64();

        let buttons = self.render_buttons(&result, 1);
        let message = self
            .render_response_message(&result, used_time, buttons)
            .await?;

        // Send search results and get message_id; fall back to HTML on failure
        let sent_message_id = self.send_input_message(chat_id, message).await?;
        info!(
            "[{}] sent search results: {} hits",
            self.id, result.total_results
        );

        // Store query for pagination
        let query_key = format!("{}:query_text:{}:{}", self.id, chat_id, sent_message_id);
        self.storage.set(&query_key, &query).await?;

        if let Some(chats_vec) = chats {
            let chats_str = chats_vec
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(",");
            let chats_key = format!("{}:query_chats:{}:{}", self.id, chat_id, sent_message_id);
            self.storage.set(&chats_key, &chats_str).await?;
        }

        Ok(())
    }

    /// /stat - Get index status
    async fn handle_stat(&self, chat_id: i64) -> Result<()> {
        let status = self
            .backend
            .get_index_status(crate::backend::STATUS_MESSAGE_LENGTH_LIMIT)
            .await?;
        self.send_message(chat_id, &status, None).await?;
        info!("[{}] sent index status", self.id);
        Ok(())
    }

    /// /download_chat - Download and index chat history
    async fn handle_download_chat(
        &self,
        chat_id: i64,
        text: &str,
        reply_to: Option<i32>,
    ) -> Result<()> {
        // Parse arguments using shell-words
        let args = shell_words::split(text)
            .map_err(|e| crate::types::Error::Config(format!("Failed to parse command: {}", e)))?;

        let mut min_id: Option<i32> = None;
        let mut max_id: Option<i32> = None;
        let mut chat_args = Vec::new();

        let mut i = 1; // Skip command itself
        while i < args.len() {
            let arg = args[i].as_str();

            // Telegram-friendly forms: min=123 max=456
            if let Some(v) = arg.strip_prefix("min=") {
                min_id = v.parse().ok();
                i += 1;
                continue;
            }
            if let Some(v) = arg.strip_prefix("max=") {
                max_id = v.parse().ok();
                i += 1;
                continue;
            }

            chat_args.push(args[i].clone());
            i += 1;
        }

        // Get chat IDs
        let (ids, failed) = if chat_args.is_empty() {
            match self.query_selected_chat(chat_id, reply_to).await? {
                Some(selected_ids) => (selected_ids, Vec::new()),
                None => (Vec::new(), Vec::new()),
            }
        } else {
            self.chat_ids_from_args(&chat_args).await
        };

        // Report failed chats
        if !failed.is_empty() {
            let response = format!("❌ Could not resolve: {}", failed.join(", "));
            self.send_message(chat_id, &response, None).await?;
        }

        if ids.is_empty() {
            self.send_message(chat_id, "❌ No chats specified", None)
                .await?;
            return Ok(());
        }

        for &target_chat_id in &ids {
            info!(
                "[{}] start downloading history of chat {} (min={:?}, max={:?})",
                self.id, target_chat_id, min_id, max_id
            );

            // Check if chat already has indexed documents
            let is_empty = self.backend.is_empty(Some(target_chat_id)).await?;
            if !is_empty && min_id.is_none() && max_id.is_none() {
                let warning = format!(
                    "⚠️ Chat {} already has indexed messages.\n\n\
                    To download history:\n\
                    1. Use /clear {} first to remove existing index, OR\n\
                    2. Specify min_id or max_id to download specific range\n\n\
                    Example: /download_chat {} min=12345",
                    target_chat_id, target_chat_id, target_chat_id
                );
                self.send_message(chat_id, &warning, None).await?;
                continue;
            }

            // Send initial progress message
            let progress_msg_id = self
                .send_message(
                    chat_id,
                    &format!("📥 Starting history fetch from chat {}...", target_chat_id),
                    None,
                )
                .await?;

            // Create channel for progress updates
            let (progress_tx, mut progress_rx) =
                tokio::sync::mpsc::unbounded_channel::<crate::types::DownloadProgress>();

            // Spawn task to edit progress message
            let frontend_chat_id = chat_id;
            let send_client = self.client.clone().ok_or_else(|| {
                crate::types::Error::Config("Frontend client not initialized".to_string())
            })?;
            let callback_task = tokio::spawn(async move {
                while let Some(progress) = progress_rx.recv().await {
                    let msg = format!(
                        "📥 Fetching history from chat {}...\n{} messages fetched (latest: msg_id {})",
                        progress.chat_id, progress.downloaded, progress.latest_msg_id
                    );
                    // Ignore errors in progress updates
                    let _ = Self::edit_message_with_client(
                        &send_client,
                        frontend_chat_id,
                        progress_msg_id,
                        &msg,
                        None,
                    )
                    .await;
                }
            });

            // Create progress callback that sends to channel
            let progress_callback = move |progress: crate::types::DownloadProgress| {
                // Send is non-blocking for unbounded channels
                let _ = progress_tx.send(progress);
            };

            let result = self
                .backend
                .download_history(target_chat_id, min_id, max_id, Some(progress_callback))
                .await?;

            callback_task.await?;

            // Edit final message with completion status
            let response = format!(
                "✅ Downloaded {} messages from chat {} (msg_id {}..{})",
                result.indexed_count, target_chat_id, result.min_msg_id, result.max_msg_id
            );
            self.edit_message(chat_id, progress_msg_id, &response, None)
                .await?;
            debug!(
                "[{}] downloaded {} messages from chat {} (msg_id {}..{})",
                self.id, result.indexed_count, target_chat_id, result.min_msg_id, result.max_msg_id
            );
        }

        Ok(())
    }

    /// /monitor_chat - Add chat to monitoring
    async fn handle_monitor_chat(
        &self,
        chat_id: i64,
        text: &str,
        reply_to: Option<i32>,
    ) -> Result<()> {
        let args = shell_words::split(text)
            .map_err(|e| crate::types::Error::Config(format!("Failed to parse command: {}", e)))?;

        let chat_args: Vec<String> = args.into_iter().skip(1).collect();

        let (ids, failed) = if chat_args.is_empty() {
            match self.query_selected_chat(chat_id, reply_to).await? {
                Some(selected_ids) => (selected_ids, Vec::new()),
                None => (Vec::new(), Vec::new()),
            }
        } else {
            self.chat_ids_from_args(&chat_args).await
        };

        // Report failed chats
        if !failed.is_empty() {
            let response = format!("❌ Could not resolve: {}", failed.join(", "));
            self.send_message(chat_id, &response, None).await?;
        }

        if !ids.is_empty() {
            for &target_chat_id in &ids {
                info!(
                    "[{}] add chat {} to monitored_chats",
                    self.id, target_chat_id
                );
                let chat_html = self.backend.format_dialog_html(target_chat_id).await?;
                let response = format!("{} has been added to monitoring list", chat_html);
                self.send_message(chat_id, &response, None).await?;
                // TODO: Actually add to backend monitored_chats
            }
        }

        Ok(())
    }

    /// /clear - Clear index
    async fn handle_clear(&self, chat_id: i64, text: &str, reply_to: Option<i32>) -> Result<()> {
        let args = shell_words::split(text)
            .map_err(|e| crate::types::Error::Config(format!("Failed to parse command: {}", e)))?;

        let chat_args: Vec<String> = args.into_iter().skip(1).collect();

        let clear_all = chat_args.len() == 1 && chat_args[0].to_lowercase() == "all";

        if !clear_all && chat_args.is_empty() {
            let selected = self.query_selected_chat(chat_id, reply_to).await?;
            if selected.is_none() {
                let response = "Use /clear all to clear all indexes, or use /clear [CHAT ...] to specify chat names or IDs to delete";
                self.send_message(chat_id, response, None).await?;
                return Ok(());
            }
        }

        if clear_all {
            let cleared = self.backend.clear(None).await?;
            let response = format!(
                "✅ Cleared {} chat(s) from monitoring and deleted documents from index",
                cleared.len()
            );
            self.send_message(chat_id, &response, None).await?;
            info!(
                "[{}] all indexes cleared ({} chats)",
                self.id,
                cleared.len()
            );
        } else {
            let (ids, failed) = if chat_args.is_empty() {
                match self.query_selected_chat(chat_id, reply_to).await? {
                    Some(selected_ids) => (selected_ids, Vec::new()),
                    None => (Vec::new(), Vec::new()),
                }
            } else {
                self.chat_ids_from_args(&chat_args).await
            };

            // Report failed chats
            if !failed.is_empty() {
                let response = format!("❌ Could not resolve: {}", failed.join(", "));
                self.send_message(chat_id, &response, None).await?;
            }

            if !ids.is_empty() {
                let cleared = self.backend.clear(Some(&ids)).await?;

                // Report which chats were actually cleared
                if cleared.is_empty() {
                    self.send_message(
                        chat_id,
                        "❌ None of the specified chats were being monitored",
                        None,
                    )
                    .await?;
                } else {
                    // Send confirmation
                    let mut response_parts = Vec::new();
                    for &target_chat_id in &cleared {
                        let chat_html = self.backend.format_dialog_html(target_chat_id).await?;
                        response_parts.push(format!(
                            "✅ Cleared {} and deleted documents from index",
                            chat_html
                        ));
                    }
                    let response = response_parts.join("\n");
                    self.send_message(chat_id, &response, None).await?;

                    // Report which chats were not monitored
                    let not_cleared: Vec<i64> = ids
                        .iter()
                        .filter(|id| !cleared.contains(id))
                        .copied()
                        .collect();

                    if !not_cleared.is_empty() {
                        let not_monitored_names: Vec<String> =
                            not_cleared.iter().map(|id| id.to_string()).collect();
                        let response =
                            format!("⚠️ Not monitored: {}", not_monitored_names.join(", "));
                        self.send_message(chat_id, &response, None).await?;
                    }
                }
            }
        }

        Ok(())
    }

    /// /refresh_chat_names - Refresh chat name cache
    async fn handle_refresh_chat_names(&self, chat_id: i64) -> Result<()> {
        // Start refresh in background (non-blocking)
        self.backend.refresh_chat_names_async();

        let count = self.backend.get_cache_info();
        let response = format!(
            "Chat name cache refresh started in background.\n\n\
            Current cache: {} chats\n\n\
            The cache will update automatically. You can continue using the bot normally.",
            count
        );

        self.send_message(chat_id, &response, None).await?;
        debug!("[{}] started background chat name cache refresh", self.id);

        Ok(())
    }

    /// /find_chat_id - Find chat by name
    async fn handle_find_chat_id(&self, chat_id: i64, text: &str) -> Result<()> {
        let query = text.trim_start_matches("/find_chat_id").trim();

        if query.is_empty() {
            self.send_message(chat_id, "❌ Keyword cannot be empty", None)
                .await?;
            return Ok(());
        }

        let found_chat_ids = self.backend.find_chat_id(query).await?;

        // Get cache info
        let cache_count = self.backend.get_cache_info();
        let cache_info = format!("\n\n<i>Cache: {} chats</i>", cache_count);

        let mut response_parts = Vec::new();

        for &found_chat_id in found_chat_ids.iter().take(50) {
            let chat_name = self.backend.translate_chat_id(found_chat_id).await?;
            let escaped_name = html_escape::encode_text(&chat_name);
            response_parts.push(format!(
                "{}: <code>{}</code>\n",
                escaped_name, found_chat_id
            ));
        }

        let mut response = if response_parts.is_empty() {
            format!("No chats found with \"{}\" in title", query)
        } else {
            response_parts.join("")
        };

        // Add cache info and refresh hint
        response.push_str(&cache_info);
        response.push_str("\n\nUse /refresh_chat_names to update the cache.");

        self.send_message(chat_id, &response, None).await?;
        info!(
            "[{}] sent find results: {} chats",
            self.id,
            found_chat_ids.len()
        );

        Ok(())
    }

    /// Query selected chat from reply
    async fn query_selected_chat(
        &self,
        chat_id: i64,
        reply_to: Option<i32>,
    ) -> Result<Option<Vec<i64>>> {
        if let Some(reply_msg_id) = reply_to {
            let key = format!("{}:select_chat:{}:{}", self.id, chat_id, reply_msg_id);
            if let Some(stored) = self.storage.get(&key).await?
                && let Ok(selected_id) = stored.parse::<i64>()
            {
                return Ok(Some(vec![selected_id]));
            }
        }
        Ok(None)
    }

    /// Convert chat arguments to chat IDs
    /// Returns (successful_ids, failed_chats) tuple
    async fn chat_ids_from_args(&self, chats: &[String]) -> (Vec<i64>, Vec<String>) {
        let mut ids = Vec::new();
        let mut failed = Vec::new();

        for chat in chats {
            match self.backend.str_to_chat_id(chat).await {
                Ok(id) => ids.push(id),
                Err(e) => {
                    error!("[{}] failed to resolve chat {}: {}", self.id, chat, e);
                    failed.push(chat.clone());
                }
            }
        }

        (ids, failed)
    }

    /// Render search results
    async fn render_response_message(
        &self,
        result: &SearchResult,
        used_time: f64,
        buttons: Vec<Vec<(String, String)>>,
    ) -> Result<InputMessage> {
        let mut builder = MessageBuilder::new();

        builder.push(&format!(
            "Found {} results in {:.0} ms:\n\n",
            result.total_results,
            used_time * 1000.0
        ));

        // Pre-translate unique chat IDs to avoid redundant lookups
        let unique_chat_ids: std::collections::HashSet<_> =
            result.hits.iter().map(|hit| hit.msg.chat_id).collect();

        let mut chat_names = std::collections::HashMap::new();
        for &chat_id in &unique_chat_ids {
            let name = self.backend.translate_chat_id(chat_id).await?;
            chat_names.insert(chat_id, name);
        }

        for hit in &result.hits {
            let chat_title = &chat_names[&hit.msg.chat_id];
            let mark = builder.mark();
            builder.push(chat_title);
            if !hit.msg.sender.is_empty() {
                builder.push(" (");
                builder.push_underline(&hit.msg.sender);
                builder.push(")");
            }
            builder.push(&format!(" [{}]", hit.msg.post_time));
            builder.push_bold_since(mark);
            builder.push("\n");

            builder.push_highlighted_snippet(&hit.snippet, &hit.msg.url);
            builder.push("\n\n");
        }

        let (text, entities) = builder.build();
        let mut message = InputMessage::new().text(text).fmt_entities(entities);

        if !buttons.is_empty() {
            let markup = Self::create_inline_buttons_static(buttons);
            message = message.reply_markup(&markup);
        }

        Ok(message)
    }

    /// Register bot commands with Telegram
    async fn register_bot_commands(&self, client: &Client) {
        info!("[{}] registering bot commands", self.id);
        let commands = vec![
            tl::enums::BotCommand::Command(tl::types::BotCommand {
                command: "search".to_string(),
                description: "[query] - Search messages".to_string(),
            }),
            tl::enums::BotCommand::Command(tl::types::BotCommand {
                command: "chats".to_string(),
                description: "[keyword] - List indexed chats".to_string(),
            }),
            tl::enums::BotCommand::Command(tl::types::BotCommand {
                command: "random".to_string(),
                description: "Get a random message".to_string(),
            }),
        ];
        let admin_commands = vec![
            tl::enums::BotCommand::Command(tl::types::BotCommand {
                command: "search".to_string(),
                description: "[query] - Search messages".to_string(),
            }),
            tl::enums::BotCommand::Command(tl::types::BotCommand {
                command: "chats".to_string(),
                description: "[keyword] - List indexed chats".to_string(),
            }),
            tl::enums::BotCommand::Command(tl::types::BotCommand {
                command: "random".to_string(),
                description: "Get a random message".to_string(),
            }),
            tl::enums::BotCommand::Command(tl::types::BotCommand {
                command: "stat".to_string(),
                description: "Show index statistics".to_string(),
            }),
            tl::enums::BotCommand::Command(tl::types::BotCommand {
                command: "download_chat".to_string(),
                description: "[chat ...] [min= max=] - Download chat history".to_string(),
            }),
            tl::enums::BotCommand::Command(tl::types::BotCommand {
                command: "monitor_chat".to_string(),
                description: "[chat ...] - Start monitoring a chat".to_string(),
            }),
            tl::enums::BotCommand::Command(tl::types::BotCommand {
                command: "clear".to_string(),
                description: "[chat ... | all] - Clear index for a chat".to_string(),
            }),
            tl::enums::BotCommand::Command(tl::types::BotCommand {
                command: "refresh_chat_names".to_string(),
                description: "Refresh chat name cache".to_string(),
            }),
            tl::enums::BotCommand::Command(tl::types::BotCommand {
                command: "find_chat_id".to_string(),
                description: "<keyword> - Find chat ID by name".to_string(),
            }),
        ];

        // Register default commands (visible to all users)
        match client
            .invoke(&tl::functions::bots::SetBotCommands {
                scope: tl::enums::BotCommandScope::Default,
                lang_code: String::new(),
                commands,
            })
            .await
        {
            Ok(_) => debug!("[{}] registered default bot commands", self.id),
            Err(e) => warn!(
                "[{}] failed to register default bot commands: {}",
                self.id, e
            ),
        }

        // Register admin commands (visible only to admin in PM)
        let admin_peer = Self::chat_id_to_input_peer_static(self.admin_id);
        match client
            .invoke(&tl::functions::bots::SetBotCommands {
                scope: tl::enums::BotCommandScope::Peer(tl::types::BotCommandScopePeer {
                    peer: admin_peer,
                }),
                lang_code: String::new(),
                commands: admin_commands,
            })
            .await
        {
            Ok(_) => debug!("[{}] registered admin bot commands", self.id),
            Err(e) => warn!("[{}] failed to register admin bot commands: {}", self.id, e),
        }
    }

    /// Convert chat_id to InputPeer for message sending
    /// Note: access_hash = 0 works for bots when sending to users who've messaged the bot
    /// or channels/groups the bot is a member of
    /// Convert chat ID to InputPeer (static helper)
    fn chat_id_to_input_peer_static(chat_id: i64) -> tl::enums::InputPeer {
        use crate::utils::get_share_id;
        use grammers_tl_types as tl;

        if chat_id > 0 {
            // Positive ID = user
            tl::enums::InputPeer::User(tl::types::InputPeerUser {
                user_id: chat_id,
                access_hash: 0,
            })
        } else {
            // Negative ID = channel/supergroup - convert to share_id
            let channel_id = get_share_id(chat_id);
            tl::enums::InputPeer::Channel(tl::types::InputPeerChannel {
                channel_id,
                access_hash: 0,
            })
        }
    }

    /// Create inline button markup from button rows (static helper)
    fn create_inline_buttons_static(
        button_rows: Vec<Vec<(String, String)>>,
    ) -> reply_markup::Inline {
        let rows: Vec<Vec<button::Inline>> = button_rows
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|(label, data)| {
                        if !data.is_empty() {
                            button::inline(label, data.as_bytes())
                        } else {
                            // Empty data means disabled button (just label)
                            button::inline(label, NOOP_CALLBACK)
                        }
                    })
                    .collect()
            })
            .collect();
        reply_markup::inline(rows)
    }

    /// Render pagination buttons
    fn render_buttons(
        &self,
        result: &SearchResult,
        cur_page_num: usize,
    ) -> Vec<Vec<(String, String)>> {
        let total_pages = result.total_results.div_ceil(self.config.page_len);

        let former = if cur_page_num == 1 {
            (" ".to_string(), "".to_string())
        } else {
            (
                "Previous".to_string(),
                format!("search_page={}", cur_page_num - 1),
            )
        };

        let next = if result.is_last_page {
            (" ".to_string(), "".to_string())
        } else {
            (
                "Next".to_string(),
                format!("search_page={}", cur_page_num + 1),
            )
        };

        vec![vec![
            former,
            (
                format!("{} / {}", cur_page_num, total_pages),
                "".to_string(),
            ),
            next,
        ]]
    }

    /// Send a message to a chat (static helper)
    async fn send_message_with_client(
        client: &Client,
        chat_id: i64,
        text: &str,
        buttons: Option<Vec<Vec<(String, String)>>>,
    ) -> Result<i32> {
        // Create InputPeer using helper
        let peer = Self::chat_id_to_input_peer_static(chat_id);

        // Create message with HTML formatting
        let mut message = InputMessage::new().html(text);

        // Add inline buttons if provided
        if let Some(button_rows) = buttons {
            let markup = Self::create_inline_buttons_static(button_rows);
            message = message.reply_markup(&markup);
        }

        // Send message
        let sent = client
            .send_message(peer, message)
            .await
            .map_err(|e| crate::types::Error::Telegram(format!("Failed to send message: {}", e)))?;

        Ok(sent.id())
    }

    /// Send a message to a chat
    async fn send_message(
        &self,
        chat_id: i64,
        text: &str,
        buttons: Option<Vec<Vec<(String, String)>>>,
    ) -> Result<i32> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| crate::types::Error::Config("Bot client not initialized".to_string()))?;
        Self::send_message_with_client(client, chat_id, text, buttons).await
    }

    /// Send a pre-built InputMessage to a chat
    async fn send_input_message(&self, chat_id: i64, message: InputMessage) -> Result<i32> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| crate::types::Error::Config("Bot client not initialized".to_string()))?;
        let peer = Self::chat_id_to_input_peer_static(chat_id);
        let sent = client
            .send_message(peer, message)
            .await
            .map_err(|e| crate::types::Error::Telegram(format!("Failed to send message: {}", e)))?;
        Ok(sent.id())
    }

    /// Edit a message with a pre-built InputMessage
    async fn edit_input_message(
        &self,
        chat_id: i64,
        message_id: i32,
        message: InputMessage,
    ) -> Result<()> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| crate::types::Error::Config("Bot client not initialized".to_string()))?;
        let chat = Self::chat_id_to_input_peer_static(chat_id);
        client
            .edit_message(chat, message_id, message)
            .await
            .map_err(|e| crate::types::Error::Telegram(format!("Failed to edit message: {}", e)))?;
        Ok(())
    }

    /// Edit a message (static helper)
    async fn edit_message_with_client(
        client: &Client,
        chat_id: i64,
        message_id: i32,
        text: &str,
        buttons: Option<Vec<Vec<(String, String)>>>,
    ) -> Result<()> {
        // Create InputPeer using helper
        let chat = Self::chat_id_to_input_peer_static(chat_id);

        // Create input message with HTML formatting
        let mut input = InputMessage::new().html(text);

        // Add inline buttons if provided
        if let Some(button_rows) = buttons {
            let markup = Self::create_inline_buttons_static(button_rows);
            input = input.reply_markup(&markup);
        }

        // Edit message
        client
            .edit_message(chat, message_id, input)
            .await
            .map_err(|e| crate::types::Error::Telegram(format!("Failed to edit message: {}", e)))?;

        Ok(())
    }

    /// Edit a message
    async fn edit_message(
        &self,
        chat_id: i64,
        message_id: i32,
        text: &str,
        buttons: Option<Vec<Vec<(String, String)>>>,
    ) -> Result<()> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| crate::types::Error::Config("Bot client not initialized".to_string()))?;
        Self::edit_message_with_client(client, chat_id, message_id, text, buttons).await
    }

    /// Create a SenderPool with proxy configuration from session
    fn create_sender_pool(session: &Arc<ClientSession>) -> SenderPool {
        if let Some(proxy_url) = session.proxy() {
            let params = ConnectionParams {
                proxy_url: Some(proxy_url.to_string()),
                ..Default::default()
            };
            SenderPool::with_configuration(session.session_storage(), session.api_id(), params)
        } else {
            SenderPool::new(session.session_storage(), session.api_id())
        }
    }
}
