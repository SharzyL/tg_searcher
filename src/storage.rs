//! Storage abstraction for query state and pagination
//!
//! This module provides a storage interface that can be implemented
//! with different backends (in-memory, Redis, etc.)

use async_trait::async_trait;
use dashmap::DashMap;
use std::sync::Arc;

/// Storage trait for persisting bot state
#[async_trait]
pub trait Storage: Send + Sync {
    /// Store a value
    async fn set(&self, key: &str, value: &str) -> crate::types::Result<()>;

    /// Retrieve a value
    async fn get(&self, key: &str) -> crate::types::Result<Option<String>>;

    /// Delete a value
    #[allow(dead_code)]
    async fn delete(&self, key: &str) -> crate::types::Result<()>;

    /// Clear all data
    #[allow(dead_code)]
    async fn clear(&self) -> crate::types::Result<()>;
}

/// In-memory storage implementation
#[derive(Clone)]
pub struct InMemoryStorage {
    data: Arc<DashMap<String, String>>,
}

impl InMemoryStorage {
    /// Create a new in-memory storage
    pub fn new() -> Self {
        Self {
            data: Arc::new(DashMap::new()),
        }
    }
}

impl Default for InMemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Storage for InMemoryStorage {
    async fn set(&self, key: &str, value: &str) -> crate::types::Result<()> {
        self.data.insert(key.to_string(), value.to_string());
        Ok(())
    }

    async fn get(&self, key: &str) -> crate::types::Result<Option<String>> {
        Ok(self.data.get(key).map(|v| v.clone()))
    }

    async fn delete(&self, key: &str) -> crate::types::Result<()> {
        self.data.remove(key);
        Ok(())
    }

    async fn clear(&self) -> crate::types::Result<()> {
        self.data.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_storage() {
        let storage = InMemoryStorage::new();

        storage.set("key1", "value1").await.unwrap();
        assert_eq!(
            storage.get("key1").await.unwrap(),
            Some("value1".to_string())
        );

        storage.delete("key1").await.unwrap();
        assert_eq!(storage.get("key1").await.unwrap(), None);

        storage.set("key2", "value2").await.unwrap();
        storage.clear().await.unwrap();
        assert_eq!(storage.get("key2").await.unwrap(), None);
    }
}
