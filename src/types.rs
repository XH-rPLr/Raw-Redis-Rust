// src/types.rs
use bytes::Bytes;
use tokio::time::Instant;

/// The type of data stored in the cache
#[derive(Debug, Clone, PartialEq)]
pub enum RedisData {
    String(Bytes),
    List(Vec<Bytes>),
}

/// A value stored in the cache with optional expiration
#[derive(Debug, Clone)]
pub struct CacheValue {
    pub value: RedisData,
    pub expiry: Option<Instant>,
}

/// Helper function to check if a cache value has expired
impl CacheValue {
    pub fn is_expired(&self) -> bool {
        if let Some(expiry) = self.expiry {
            return Instant::now() >= expiry;
        }
        false
    }
}
