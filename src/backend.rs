//! Backend bot for message indexing
//!
//! This module implements the backend that monitors Telegram chats
//! and indexes messages using the Indexer.

// Constants
/// Maximum length for status messages before truncation (to fit in Telegram's 4096 limit)
pub const STATUS_MESSAGE_LENGTH_LIMIT: usize = 4000;
/// Batch size for indexing messages during history download
const DOWNLOAD_BATCH_SIZE: usize = 1000;
/// Batch size for progress callbacks during message fetching (independent from indexing batches)
const FETCH_PROGRESS_BATCH_SIZE: usize = 100;

use crate::config::BackendConfig;
use crate::indexer::Indexer;
use crate::session::ClientSession;
use crate::types::{DownloadProgress, IndexMsg, Result, SearchResult};
use crate::utils::{brief_content, escape_content, get_share_id};
use dashmap::DashMap;
use grammers_client::Client;
use grammers_client::client::UpdatesConfiguration;
use grammers_client::types::update::Message as UpdateMessage; // Update message type
use grammers_client::types::update::{MessageDeletion, Update};
use grammers_mtsender::{ConnectionParams, SenderPool};
use std::collections::HashSet;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Backend bot for indexing messages
pub struct BackendBot {
    /// Backend ID
    id: String,

    /// Session reference (for getting storage/API credentials)
    session: Arc<ClientSession>,

    /// Telegram client (cloneable, set when run() starts)
    client: std::sync::OnceLock<Client>,

    /// Chat ID to name cache (shared from session)
    chat_cache: Arc<DashMap<i64, String>>,

    /// Search indexer
    indexer: Arc<Indexer>,

    /// Set of chat IDs being monitored
    monitored_chats: Arc<DashMap<i64, ()>>,

    /// Set of chat IDs excluded from monitoring
    excluded_chats: HashSet<i64>,

    /// Track newest message per chat
    newest_msg: Arc<DashMap<i64, IndexMsg>>,

    /// Configuration
    monitor_all: bool,
}

impl BackendBot {
    /// Create a new backend bot
    pub async fn new(
        backend_id: &str,
        config: &BackendConfig,
        session: Arc<ClientSession>,
        indexer: Arc<Indexer>,
    ) -> Result<Self> {
        info!("Creating backend bot: {}", backend_id);

        // Get all indexed chats to monitor (doesn't require a client)
        let indexed_chats = indexer.list_indexed_chats().await?;
        let monitored_chats = Arc::new(DashMap::new());
        for chat_id in indexed_chats {
            monitored_chats.insert(chat_id, ());
        }

        // Parse excluded chats
        let excluded_chats: HashSet<i64> = config
            .config
            .excluded_chats
            .iter()
            .map(|&id| get_share_id(id))
            .collect();

        Ok(Self {
            id: backend_id.to_string(),
            session: session.clone(),
            client: std::sync::OnceLock::new(),
            chat_cache: session.chat_cache(),
            indexer,
            monitored_chats,
            excluded_chats,
            newest_msg: Arc::new(DashMap::new()),
            monitor_all: config.config.monitor_all,
        })
    }

    /// Initialize backend and validate monitored chats
    pub async fn initialize(&self) -> Result<()> {
        info!("Initializing backend bot: {}", self.id);
        // Cache and access hash population moved to run() to use same dialog iteration
        Ok(())
    }

    /// Run the event loop to monitor messages
    pub async fn run(&self) -> Result<()> {
        info!("Starting event loop for backend: {}", self.id);

        // Create SenderPool and Client for all operations (not just updates)
        let pool = Self::create_sender_pool(&self.session);
        let client = Client::new(&pool);
        let SenderPool {
            runner, updates, ..
        } = pool;

        // Store the client for use by other methods (download_history, etc.)
        self.client
            .set(client.clone())
            .map_err(|_| crate::types::Error::Config("Client already initialized".to_string()))?;

        // Spawn the sender pool runner task
        tokio::spawn(runner.run());

        let updates_client = client;

        // Log monitored chats (cache already populated by session)
        for entry in self.monitored_chats.iter() {
            let chat_id = *entry.key();
            if let Some(name) = self.chat_cache.get(&chat_id) {
                info!("Ready to monitor \"{}\" ({})", name.value(), chat_id);
            } else {
                info!("Ready to monitor chat {} (name not in cache)", chat_id);
            }
        }

        let mut updates = updates_client.stream_updates(
            updates,
            UpdatesConfiguration {
                catch_up: false, // Don't fetch old updates - only receive new ones from now
                ..Default::default()
            },
        );

        info!("Backend streaming updates, waiting for messages...");
        loop {
            match updates.next().await {
                Ok(update) => {
                    match update {
                        Update::NewMessage(message) => {
                            let chat_id = message.peer_id().bot_api_dialog_id();
                            let share_id = crate::utils::get_share_id(chat_id);
                            debug!("Backend received new message from chat {}", share_id);
                            if let Err(e) = self.handle_new_message(message).await {
                                error!("Error handling new message: {}", e);
                            }
                        }
                        Update::MessageEdited(message) => {
                            let chat_id = message.peer_id().bot_api_dialog_id();
                            let share_id = crate::utils::get_share_id(chat_id);
                            debug!("Backend received edited message from chat {}", share_id);
                            if let Err(e) = self.handle_message_edited(message).await {
                                error!("Error handling edited message: {}", e);
                            }
                        }
                        Update::MessageDeleted(deletion) => {
                            debug!("Backend received message deletion");
                            if let Err(e) = self.handle_message_deleted(deletion).await {
                                error!("Error handling deleted message: {}", e);
                            }
                        }
                        _ => {
                            // Log other update types at debug level
                            debug!(
                                "Backend received other update: {:?}",
                                std::any::type_name_of_val(&update)
                            );
                        }
                    }
                }
                Err(e) => {
                    error!("Error getting update: {}", e);
                    // Break on error - will cause backend task to exit
                    break;
                }
            }
        }

        warn!("Backend '{}' event loop exited", self.id);
        Ok(())
    }

    /// Search messages
    pub async fn search(
        &self,
        query: &str,
        chats: Option<&[i64]>,
        page_len: usize,
        page_num: usize,
    ) -> Result<SearchResult> {
        self.indexer.search(query, chats, page_len, page_num).await
    }

    /// Get a random message
    pub async fn rand_msg(&self) -> Result<Option<IndexMsg>> {
        self.indexer.retrieve_random_document().await
    }

    /// Get the client, returning an error if not initialized
    fn get_client(&self) -> Result<&Client> {
        self.client.get().ok_or_else(|| {
            crate::types::Error::Config(
                "Backend client not initialized. Make sure run() is called first.".to_string(),
            )
        })
    }

    /// Check if index is empty (optionally for a specific chat)
    pub async fn is_empty(&self, chat_id: Option<i64>) -> Result<bool> {
        if let Some(chat_id) = chat_id {
            // Check if specific chat has any documents
            let results = self.indexer.search("*", Some(&[chat_id]), 1, 1).await?;
            Ok(results.total_results == 0)
        } else {
            // Check if entire index is empty
            let chats = self.indexer.list_indexed_chats().await?;
            Ok(chats.is_empty())
        }
    }

    /// Find peer by share_id in dialogs
    async fn find_peer_in_dialogs(
        &self,
        share_id: i64,
    ) -> Result<Option<grammers_client::types::Peer>> {
        let client = self.get_client()?;
        let mut dialogs = client.iter_dialogs();

        while let Some(dialog) = dialogs.next().await.map_err(|e| {
            crate::types::Error::Telegram(format!("Failed to iterate dialogs: {}", e))
        })? {
            let peer = dialog.peer();
            let chat_id = peer.id().bot_api_dialog_id();
            let peer_share_id = get_share_id(chat_id);

            if peer_share_id == share_id {
                return Ok(Some(peer.clone()));
            }
        }

        Ok(None)
    }

    /// Download chat history and index it
    ///
    /// The `progress_callback` is called while fetching messages with progress information.
    /// The callback should be quick as it blocks the download loop.
    pub async fn download_history<F>(
        &self,
        chat_id: i64,
        min_id: Option<i32>,
        max_id: Option<i32>,
        progress_callback: Option<F>,
    ) -> Result<usize>
    where
        F: Fn(DownloadProgress),
    {
        let share_id = get_share_id(chat_id);
        info!(
            "Downloading history from {} (min_id={:?}, max_id={:?})",
            share_id, min_id, max_id
        );

        // Add to monitored chats
        self.monitored_chats.insert(share_id, ());

        // Find the chat in dialogs to get proper peer info
        let chat = self.find_peer_in_dialogs(share_id).await?.ok_or_else(|| {
            crate::types::Error::EntityNotFound(format!(
                "Chat {} not found in dialogs. Make sure you have access to this chat.",
                share_id
            ))
        })?;

        // Iterate messages (fetches from newest to oldest by default).
        // We stream-fetch and index in the same loop (no buffering / reordering required).
        let client = self.get_client()?;
        let mut message_iter = client.iter_messages(&chat).offset_id(max_id.unwrap_or(0));

        let mut fetched_count: usize = 0;
        let mut indexed_count: usize = 0;
        let mut newest: Option<IndexMsg> = None;
        let mut batch: Vec<IndexMsg> = Vec::new();
        let mut fetched_last_msg_id: i32 = 0;

        info!(
            "Downloading history from chat {} (streaming fetch + index)...",
            share_id
        );

        while let Some(message) = message_iter.next().await.map_err(|e| {
            crate::types::Error::Telegram(format!("Failed to iterate messages: {}", e))
        })? {
            let msg_id = message.id();

            // Check min/max bounds (iterator is newest -> oldest)
            if let Some(min) = min_id
                && msg_id < min
            {
                break;
            }
            if let Some(max) = max_id
                && msg_id > max
            {
                continue;
            }

            fetched_last_msg_id = msg_id;
            fetched_count += 1;

            if let Some(ref callback) = progress_callback
                && fetched_count.is_multiple_of(FETCH_PROGRESS_BATCH_SIZE)
            {
                info!("Update: fetched {fetched_count} messages");
                callback(DownloadProgress {
                    downloaded: fetched_count,
                    chat_id: share_id,
                    latest_msg_id: fetched_last_msg_id,
                });
            }

            // Extract text and index if present
            let text = message.text();
            if let Some(content) = self.extract_text(text) {
                // Create IndexMsg from iter_messages result
                let chat_id = message.peer_id().bot_api_dialog_id();
                let share_id = get_share_id(chat_id);
                let sender = message
                    .sender()
                    .and_then(|p| p.name())
                    .unwrap_or("Unknown")
                    .to_string();
                let post_time = message.date();

                let index_msg = IndexMsg {
                    content,
                    url: format!("https://t.me/c/{}/{}", share_id, msg_id),
                    chat_id: share_id,
                    post_time,
                    sender,
                };

                // Track newest (by post_time, independent of fetch order)
                if newest.is_none() || index_msg.post_time > newest.as_ref().unwrap().post_time {
                    newest = Some(index_msg.clone());
                }

                batch.push(index_msg);
                indexed_count += 1;

                if batch.len() >= DOWNLOAD_BATCH_SIZE {
                    self.indexer.add_documents_batch(batch).await?;
                    batch = Vec::new();
                    info!(
                        "Indexed {} messages from {} (up to msg_id {})",
                        indexed_count, share_id, msg_id
                    );
                }
            }
        }

        if let Some(ref callback) = progress_callback {
            callback(DownloadProgress {
                downloaded: fetched_count,
                chat_id: share_id,
                latest_msg_id: fetched_last_msg_id,
            });
        }

        // Commit remaining messages in batch
        if !batch.is_empty() {
            self.indexer.add_documents_batch(batch).await?;
        }

        // Update newest message
        if let Some(msg) = newest {
            self.newest_msg.insert(share_id, msg);
        }

        info!(
            "Download complete for {}: fetched {}, indexed {}",
            share_id, fetched_count, indexed_count
        );
        Ok(indexed_count)
    }

    /// Clear index (optionally for specific chats)
    ///
    /// Removes chats from monitoring and deletes all their documents from the index.
    ///
    /// Returns the list of chat IDs that were cleared
    pub async fn clear(&self, chat_ids: Option<&[i64]>) -> Result<Vec<i64>> {
        let cleared = if let Some(chat_ids) = chat_ids {
            let mut cleared_chats = Vec::new();
            for &chat_id in chat_ids {
                // Only clear if chat is actually being monitored
                if self.monitored_chats.remove(&chat_id).is_some() {
                    // Delete documents from index
                    self.indexer.delete_chat_documents(chat_id).await?;
                    info!(
                        "Cleared chat {} from monitoring and deleted documents from index",
                        chat_id
                    );
                    self.newest_msg.remove(&chat_id);
                    cleared_chats.push(chat_id);
                }
            }
            cleared_chats
        } else {
            // Clear all - get list of all monitored chats before clearing
            let all_chats: Vec<i64> = self
                .monitored_chats
                .iter()
                .map(|entry| *entry.key())
                .collect();

            // Delete documents for each chat
            for &chat_id in &all_chats {
                self.indexer.delete_chat_documents(chat_id).await?;
            }

            info!(
                "Cleared all {} monitored chats and deleted documents from index",
                all_chats.len()
            );
            self.monitored_chats.clear();
            self.newest_msg.clear();
            all_chats
        };
        Ok(cleared)
    }

    /// Find chat IDs by name substring
    pub async fn find_chat_id(&self, query: &str) -> Result<Vec<i64>> {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        // Search in cache instead of iterating all dialogs
        for entry in self.chat_cache.iter() {
            let chat_id = *entry.key();
            let chat_name = entry.value();

            if chat_name.to_lowercase().contains(&query_lower) {
                results.push(chat_id);
            }
        }

        // Sort by chat ID for consistency
        results.sort();

        Ok(results)
    }

    /// Get cache entry count
    pub fn get_cache_info(&self) -> usize {
        self.chat_cache.len()
    }

    /// Refresh chat name cache by iterating through all dialogs (non-blocking)
    /// Returns immediately and spawns background task
    pub fn refresh_chat_names_async(&self) {
        // Get the client - if not initialized, silently return (can't refresh yet)
        let client = match self.client.get() {
            Some(c) => c.clone(),
            None => {
                warn!("Cannot refresh chat names: client not initialized yet");
                return;
            }
        };
        let chat_cache = Arc::clone(&self.chat_cache);

        tokio::spawn(async move {
            info!("Background: Refreshing chat name cache...");
            let mut count = 0;
            let mut dialogs = client.iter_dialogs();

            while let Some(dialog) = dialogs.next().await.ok().flatten() {
                let peer = dialog.peer();
                let chat_id = peer.id().bot_api_dialog_id();
                let share_id = get_share_id(chat_id);

                if let Some(name) = peer.name() {
                    chat_cache.insert(share_id, name.to_string());
                    count += 1;
                }
            }

            info!("Background: Refreshed {} chat names in cache", count);
        });
    }

    /// Get index status as HTML string
    pub async fn get_index_status(&self, length_limit: usize) -> Result<String> {
        let mut sb = String::new();
        let overflow_msg =
            "\n\nDue to Telegram message length limit, some chat statistics are not displayed";

        // Get document counts per chat (efficient single-pass)
        let chat_counts = self.indexer.get_chat_document_counts().await?;
        let total_docs: usize = chat_counts.values().sum();

        sb.push_str(&format!(
            "Backend \"{}\" (session: \"{}\") total messages: <b>{}</b>\n\n",
            self.id,
            self.session.name(),
            total_docs
        ));
        let mut cur_len = sb.len();

        if self.monitor_all {
            let excluded_msg = format!(
                "{} chats excluded from indexing\n",
                self.excluded_chats.len()
            );
            if cur_len + excluded_msg.len() < length_limit - overflow_msg.len() {
                sb.push_str(&excluded_msg);
                cur_len += excluded_msg.len();

                for &chat_id in &self.excluded_chats {
                    let line = format!("- {}\n", self.format_dialog_html(chat_id).await?);
                    if cur_len + line.len() >= length_limit - overflow_msg.len() {
                        sb.push_str(overflow_msg);
                        return Ok(sb);
                    }
                    sb.push_str(&line);
                    cur_len += line.len();
                }
                sb.push('\n');
                cur_len += 1;
            }
        }

        let monitor_msg = format!("Total {} chats indexed:\n", self.monitored_chats.len());
        if cur_len + monitor_msg.len() < length_limit - overflow_msg.len() {
            sb.push_str(&monitor_msg);
            cur_len += monitor_msg.len();
        }

        for entry in self.monitored_chats.iter() {
            let chat_id = *entry.key();
            let mut msg_for_chat = String::new();

            // Get message count from the counts map
            let num = chat_counts.get(&chat_id).copied().unwrap_or(0);

            msg_for_chat.push_str(&format!(
                "- {} ({} messages)\n",
                self.format_dialog_html(chat_id).await?,
                num
            ));

            if let Some(newest_msg) = self.newest_msg.get(&chat_id) {
                msg_for_chat.push_str(&format!(
                    "  Latest: <a href=\"{}\">{}</a>\n",
                    newest_msg.url,
                    brief_content(&newest_msg.content, 60)
                ));
            }

            if cur_len + msg_for_chat.len() >= length_limit - overflow_msg.len() {
                sb.push_str(overflow_msg);
                break;
            }

            sb.push_str(&msg_for_chat);
            cur_len += msg_for_chat.len();
        }

        Ok(sb)
    }

    /// Format chat as HTML link
    pub async fn format_dialog_html(&self, chat_id: i64) -> Result<String> {
        let name = self.translate_chat_id(chat_id).await?;
        let escaped_name = html_escape::encode_text(&name);
        Ok(format!(
            "<a href=\"https://t.me/c/{}/99999999\">{}</a> ({})",
            chat_id, escaped_name, chat_id
        ))
    }

    /// Check if a chat should be monitored
    fn should_monitor(&self, chat_id: i64) -> bool {
        let share_id = get_share_id(chat_id);
        if self.monitor_all {
            !self.excluded_chats.contains(&share_id)
        } else {
            self.monitored_chats.contains_key(&share_id)
        }
    }

    /// Extract text from message and escape HTML
    fn extract_text(&self, raw_text: &str) -> Option<String> {
        let trimmed = raw_text.trim();
        if !trimmed.is_empty() {
            Some(escape_content(trimmed))
        } else {
            None
        }
    }

    /// Convert grammers UpdateMessage to IndexMsg
    fn message_to_index_msg(&self, message: &UpdateMessage, content: String) -> Result<IndexMsg> {
        let chat_id = message.peer_id().bot_api_dialog_id();
        let share_id = get_share_id(chat_id);
        let msg_id = message.id();

        // Get sender name from sender if available
        let sender = message
            .sender()
            .and_then(|p| p.name())
            .unwrap_or("Unknown")
            .to_string();

        // Get post time
        let post_time = message.date();

        Ok(IndexMsg {
            content,
            url: format!("https://t.me/c/{}/{}", share_id, msg_id),
            chat_id: share_id,
            post_time,
            sender,
        })
    }

    /// Handle new message event
    async fn handle_new_message(&self, message: UpdateMessage) -> Result<()> {
        let chat_id = message.peer_id().bot_api_dialog_id();
        let share_id = get_share_id(chat_id);

        // Check if we should monitor this chat
        if !self.should_monitor(share_id) {
            return Ok(());
        }

        // Extract text
        let text = message.text();
        if let Some(content) = self.extract_text(text) {
            let index_msg = self.message_to_index_msg(&message, content.clone())?;

            // Add to index
            self.indexer.add_document(index_msg.clone()).await?;

            // Update newest message
            self.newest_msg.insert(share_id, index_msg);

            // Log with brief excerpt
            let brief = brief_content(&content, 20);
            info!("Indexed new message in chat {}: {:?}", share_id, brief);
        }

        Ok(())
    }

    /// Handle message edited event
    async fn handle_message_edited(&self, message: UpdateMessage) -> Result<()> {
        let chat_id = message.peer_id().bot_api_dialog_id();
        let share_id = get_share_id(chat_id);

        // Check if we should monitor this chat
        if !self.should_monitor(share_id) {
            return Ok(());
        }

        // Extract new text
        let text = message.text();
        if let Some(content) = self.extract_text(text) {
            let msg_id = message.id();
            let url = format!("https://t.me/c/{}/{}", share_id, msg_id);

            // Update in index
            self.indexer.update_document(&url, &content).await?;

            // Log with brief excerpt
            let brief = brief_content(&content, 20);
            info!("Updated edited message in chat {}: {:?}", share_id, brief);
        }

        Ok(())
    }

    /// Handle message deleted event
    async fn handle_message_deleted(&self, deletion: MessageDeletion) -> Result<()> {
        // MessageDeletion only has channel_id for channels, not for regular chats
        // For now, we'll need to track deletions differently or skip non-channel deletions
        if let Some(channel_id) = deletion.channel_id() {
            let share_id = get_share_id(channel_id);

            // Check if we should monitor this chat
            if !self.should_monitor(share_id) {
                return Ok(());
            }

            // Delete each message from index
            for msg_id in deletion.messages() {
                let url = format!("https://t.me/c/{}/{}", share_id, msg_id);
                self.indexer.delete_document(&url).await?;
            }

            info!(
                "Deleted {} messages from channel {}",
                deletion.messages().len(),
                share_id
            );
        } else {
            // For non-channel deletions, we can't determine which chat they're from
            // This is a limitation of the Telegram API
            warn!("Received deletion for non-channel chat, cannot process");
        }

        Ok(())
    }

    /// Get backend ID
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get monitored chats count
    pub fn monitored_chats_count(&self) -> usize {
        self.monitored_chats.len()
    }

    /// Get list of monitored chat IDs with their names
    pub async fn get_monitored_chats(&self) -> Result<Vec<(i64, String)>> {
        let mut chats = Vec::new();
        for entry in self.monitored_chats.iter() {
            let chat_id = *entry.key();
            let chat_name = self.translate_chat_id(chat_id).await?;
            chats.push((chat_id, chat_name));
        }

        // Sort by chat name for better UX
        chats.sort_by(|a, b| a.1.cmp(&b.1));

        Ok(chats)
    }

    /// Create a SenderPool with proxy configuration from session
    fn create_sender_pool(session: &ClientSession) -> SenderPool {
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

    /// Translate chat ID to name (with caching)
    pub async fn translate_chat_id(&self, chat_id: i64) -> Result<String> {
        // Check cache (populated by session during initialization)
        if let Some(name) = self.chat_cache.get(&chat_id) {
            return Ok(name.clone());
        }

        // If not in cache, return generic name
        Ok(format!("Chat_{}", chat_id))
    }

    /// Resolve username or chat ID string to chat ID
    pub async fn str_to_chat_id(&self, s: &str) -> Result<i64> {
        // Try parsing as integer first
        if let Ok(id) = s.parse::<i64>() {
            return Ok(get_share_id(id));
        }

        // Strip URL prefixes if present
        let username = s
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_start_matches("t.me/")
            .trim_start_matches('@');

        // Resolve username
        let client = self.get_client()?;
        let peer = client
            .resolve_username(username)
            .await
            .map_err(|e| {
                crate::types::Error::Telegram(format!("Failed to resolve username: {}", e))
            })?
            .ok_or_else(|| crate::types::Error::EntityNotFound(username.to_string()))?;

        Ok(get_share_id(peer.id().bot_api_dialog_id()))
    }
}
