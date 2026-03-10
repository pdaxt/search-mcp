mod brave;
mod duckduckgo;
mod exa;
mod searxng;
mod tavily;

use crate::cache::Cache;
use crate::types::{SearchRequest, SearchResult};
use std::sync::Arc;

/// Trait that all search backends implement.
#[async_trait::async_trait]
pub trait SearchBackend: Send + Sync {
    fn name(&self) -> &str;
    fn is_available(&self) -> bool;
    fn cost_per_query(&self) -> f64;
    async fn search(&self, request: &SearchRequest) -> anyhow::Result<Vec<SearchResult>>;
}

/// Routes queries to the best backend(s) based on cost, availability, and category.
pub struct Router {
    backends: Vec<Box<dyn SearchBackend>>,
    pub cache: Arc<Cache>,
}

impl Router {
    /// Build router from environment variables.
    /// Each backend checks for its own API key in env.
    pub fn from_env(cache: Cache) -> Self {
        let mut backends: Vec<Box<dyn SearchBackend>> = Vec::new();

        if let Some(b) = brave::BraveBackend::from_env() {
            tracing::info!("brave backend enabled");
            backends.push(Box::new(b));
        }
        if let Some(b) = exa::ExaBackend::from_env() {
            tracing::info!("exa backend enabled");
            backends.push(Box::new(b));
        }
        if let Some(b) = tavily::TavilyBackend::from_env() {
            tracing::info!("tavily backend enabled");
            backends.push(Box::new(b));
        }
        if let Some(b) = searxng::SearxngBackend::from_env() {
            tracing::info!("searxng backend enabled");
            backends.push(Box::new(b));
        }

        // DuckDuckGo is always available (no API key needed)
        tracing::info!("duckduckgo backend enabled (fallback)");
        backends.push(Box::new(duckduckgo::DuckDuckGoBackend::new()));

        Self {
            backends,
            cache: Arc::new(cache),
        }
    }

    /// Search using the best available backend(s).
    pub async fn search(&self, request: &SearchRequest) -> anyhow::Result<Vec<SearchResult>> {
        // If specific backends requested, use those
        if !request.backends.is_empty() {
            let selected: Vec<&dyn SearchBackend> = self
                .backends
                .iter()
                .filter(|b| {
                    request
                        .backends
                        .iter()
                        .any(|n| n.eq_ignore_ascii_case(b.name()))
                })
                .map(|b| b.as_ref())
                .collect();

            if !selected.is_empty() {
                return self.search_multiple(&selected, request).await;
            }
        }

        // Auto-route: use cheapest available backend first
        let mut available: Vec<&dyn SearchBackend> = self
            .backends
            .iter()
            .filter(|b| b.is_available())
            .map(|b| b.as_ref())
            .collect();

        // Sort by cost (cheapest first)
        available.sort_by(|a, b| a.cost_per_query().partial_cmp(&b.cost_per_query()).unwrap());

        if available.is_empty() {
            anyhow::bail!("no search backends available");
        }

        // Try cheapest first, fall back to next if it fails
        for backend in &available {
            match backend.search(request).await {
                Ok(results) if !results.is_empty() => return Ok(results),
                Ok(_) => {
                    tracing::debug!("{} returned empty results, trying next", backend.name());
                }
                Err(e) => {
                    tracing::warn!("{} failed: {}", backend.name(), e);
                }
            }
        }

        // Last resort: return empty
        Ok(vec![])
    }

    /// Search multiple backends and merge results.
    async fn search_multiple(
        &self,
        backends: &[&dyn SearchBackend],
        request: &SearchRequest,
    ) -> anyhow::Result<Vec<SearchResult>> {
        let mut all_results = Vec::new();

        // Run all backends concurrently
        let _futures: Vec<_> = backends
            .iter()
            .map(|b| {
                let req = request.clone();
                let name = b.name().to_string();
                let backend = *b;
                async move {
                    match backend.search(&req).await {
                        Ok(results) => results,
                        Err(e) => {
                            tracing::warn!("{} failed: {}", name, e);
                            vec![]
                        }
                    }
                }
            })
            .collect();

        // We can't use join_all on trait objects easily, so sequential for now
        for backend in backends {
            match backend.search(request).await {
                Ok(mut results) => all_results.append(&mut results),
                Err(e) => tracing::warn!("{} failed: {}", backend.name(), e),
            }
        }

        Ok(all_results)
    }

    pub fn available_backends(&self) -> Vec<String> {
        self.backends
            .iter()
            .filter(|b| b.is_available())
            .map(|b| b.name().to_string())
            .collect()
    }
}

// Re-export async_trait for backends
pub use async_trait::async_trait;
