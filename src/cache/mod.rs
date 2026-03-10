use crate::types::SearchResponse;
use dashmap::DashMap;
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Two-tier cache: in-memory (DashMap) + persistent (SQLite).
pub struct Cache {
    memory: DashMap<String, CacheEntry>,
    db: Arc<Mutex<Connection>>,
    ttl_secs: u64,
}

struct CacheEntry {
    response: SearchResponse,
    timestamp: u64,
}

impl Cache {
    pub fn new(dir: &str, ttl_secs: u64) -> anyhow::Result<Self> {
        let db_path = format!("{}/cache.db", dir);
        let conn = Connection::open(&db_path)?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS search_cache (
                key TEXT PRIMARY KEY,
                response TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_cache_created ON search_cache(created_at);

            CREATE TABLE IF NOT EXISTS query_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                query TEXT NOT NULL,
                backends TEXT NOT NULL,
                cached INTEGER NOT NULL,
                results_count INTEGER NOT NULL,
                latency_ms INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            );",
        )?;

        // Purge expired entries
        let now = now_secs();
        conn.execute(
            "DELETE FROM search_cache WHERE created_at < ?1",
            [now.saturating_sub(ttl_secs)],
        )?;

        Ok(Self {
            memory: DashMap::new(),
            db: Arc::new(Mutex::new(conn)),
            ttl_secs,
        })
    }

    /// Get cached response. Checks memory first, then SQLite.
    pub fn get(&self, query: &str) -> Option<SearchResponse> {
        let key = cache_key(query);
        let now = now_secs();

        // Check memory cache
        if let Some(entry) = self.memory.get(&key) {
            if now - entry.timestamp < self.ttl_secs {
                let mut resp = entry.response.clone();
                resp.cached = true;
                return Some(resp);
            }
            drop(entry);
            self.memory.remove(&key);
        }

        // Check SQLite
        let db = self.db.lock().ok()?;
        let mut stmt = db
            .prepare("SELECT response FROM search_cache WHERE key = ?1 AND created_at > ?2")
            .ok()?;
        let result: Option<String> = stmt
            .query_row(
                rusqlite::params![key, now.saturating_sub(self.ttl_secs)],
                |row| row.get(0),
            )
            .ok();

        if let Some(json) = result {
            if let Ok(mut resp) = serde_json::from_str::<SearchResponse>(&json) {
                resp.cached = true;
                // Promote to memory cache
                self.memory.insert(
                    key,
                    CacheEntry {
                        response: resp.clone(),
                        timestamp: now,
                    },
                );
                return Some(resp);
            }
        }

        None
    }

    /// Store response in both caches.
    pub fn set(&self, query: &str, response: &SearchResponse) {
        let key = cache_key(query);
        let now = now_secs();

        // Memory cache
        self.memory.insert(
            key.clone(),
            CacheEntry {
                response: response.clone(),
                timestamp: now,
            },
        );

        // SQLite cache
        if let Ok(db) = self.db.lock() {
            let json = serde_json::to_string(response).unwrap_or_default();
            let _ = db.execute(
                "INSERT OR REPLACE INTO search_cache (key, response, created_at) VALUES (?1, ?2, ?3)",
                rusqlite::params![key, json, now],
            );
        }
    }

    /// Log a query for analytics.
    pub fn log_query(
        &self,
        query: &str,
        backends: &[String],
        cached: bool,
        results_count: usize,
        latency_ms: u64,
    ) {
        if let Ok(db) = self.db.lock() {
            let _ = db.execute(
                "INSERT INTO query_log (query, backends, cached, results_count, latency_ms, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    query,
                    backends.join(","),
                    cached as i32,
                    results_count as i64,
                    latency_ms as i64,
                    now_secs() as i64,
                ],
            );
        }
    }

    /// Get cache stats.
    pub fn stats(&self) -> CacheStats {
        let memory_entries = self.memory.len();
        let (db_entries, total_queries, cache_hits) = self
            .db
            .lock()
            .map(|db| {
                let db_count: i64 = db
                    .query_row("SELECT COUNT(*) FROM search_cache", [], |r| r.get(0))
                    .unwrap_or(0);
                let total: i64 = db
                    .query_row("SELECT COUNT(*) FROM query_log", [], |r| r.get(0))
                    .unwrap_or(0);
                let hits: i64 = db
                    .query_row("SELECT COUNT(*) FROM query_log WHERE cached = 1", [], |r| {
                        r.get(0)
                    })
                    .unwrap_or(0);
                (db_count as u64, total as u64, hits as u64)
            })
            .unwrap_or((0, 0, 0));

        CacheStats {
            memory_entries: memory_entries as u64,
            db_entries,
            total_queries,
            cache_hits,
            hit_rate: if total_queries > 0 {
                cache_hits as f64 / total_queries as f64
            } else {
                0.0
            },
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct CacheStats {
    pub memory_entries: u64,
    pub db_entries: u64,
    pub total_queries: u64,
    pub cache_hits: u64,
    pub hit_rate: f64,
}

/// Normalize query and generate cache key.
fn cache_key(query: &str) -> String {
    let normalized = query.trim().to_lowercase();
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    hex::encode(hasher.finalize())
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
