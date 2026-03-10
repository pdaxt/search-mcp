use crate::backends::Router;
use crate::fusion;
use crate::types::{SearchCategory, SearchRequest, SearchResponse, TimeRange};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, ServerCapabilities, ServerInfo},
    tool, tool_router,
    transport::io::stdio,
    ServerHandler, ServiceExt,
};
use serde::Deserialize;
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone)]
#[allow(dead_code)]
pub struct SearchService {
    router: Arc<Router>,
    tool_router: ToolRouter<Self>,
}

#[derive(Deserialize, rmcp::schemars::JsonSchema)]
pub struct SearchParams {
    /// Search query
    pub query: String,
    /// Max results to return (default: 10)
    #[serde(default = "default_max")]
    pub max_results: usize,
    /// Category: general, news, academic, code, images
    #[serde(default)]
    pub category: Option<String>,
    /// Time range: day, week, month, year
    #[serde(default)]
    pub time_range: Option<String>,
    /// Specific backends (comma-separated): brave, exa, tavily, searxng, duckduckgo
    #[serde(default)]
    pub backends: Option<String>,
}

fn default_max() -> usize {
    10
}

#[derive(Deserialize, rmcp::schemars::JsonSchema)]
pub struct BatchParams {
    /// Queries separated by newlines
    pub queries: String,
    /// Max results per query (default: 5)
    #[serde(default = "default_batch_max")]
    pub max_per_query: Option<usize>,
}

fn default_batch_max() -> Option<usize> {
    Some(5)
}

#[tool_router]
impl SearchService {
    pub fn new(router: Router) -> Self {
        Self {
            router: Arc::new(router),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        description = "Search the web. Aggregates results from Brave, Exa, SearXNG, Tavily, and DuckDuckGo with caching and deduplication."
    )]
    async fn search(
        &self,
        Parameters(params): Parameters<SearchParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let cat = params
            .category
            .as_deref()
            .map(parse_category)
            .unwrap_or_default();
        let tr = params.time_range.as_deref().and_then(parse_time_range);
        let backend_list: Vec<String> = params
            .backends
            .map(|b| b.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_default();

        let request = SearchRequest {
            query: params.query.clone(),
            max_results: params.max_results,
            category: cat,
            time_range: tr,
            backends: backend_list,
        };

        let start = Instant::now();

        // Check cache first
        if let Some(cached) = self.router.cache.get(&params.query) {
            let elapsed = start.elapsed().as_millis() as u64;
            self.router.cache.log_query(
                &params.query,
                &cached.backends_used,
                true,
                cached.results.len(),
                elapsed,
            );
            return Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&cached).unwrap_or_default(),
            )]));
        }

        // Search
        match self.router.search(&request).await {
            Ok(raw_results) => {
                let backends_used = raw_results
                    .iter()
                    .map(|r| r.source.split(':').next().unwrap_or("unknown").to_string())
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>();

                let fused = fusion::fuse(raw_results, params.max_results);
                let elapsed = start.elapsed().as_millis() as u64;

                let response = SearchResponse {
                    results: fused,
                    query: params.query.clone(),
                    backends_used: backends_used.clone(),
                    cached: false,
                    total_time_ms: elapsed,
                };

                self.router.cache.set(&params.query, &response);
                self.router.cache.log_query(
                    &params.query,
                    &backends_used,
                    false,
                    response.results.len(),
                    elapsed,
                );

                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&response).unwrap_or_default(),
                )]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                "Search failed: {}",
                e
            ))])),
        }
    }

    #[tool(description = "Show available search backends, cache stats, and configuration.")]
    async fn search_status(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        let backends = self.router.available_backends();
        let stats = self.router.cache.stats();

        let status = serde_json::json!({
            "backends": backends,
            "cache": {
                "memory_entries": stats.memory_entries,
                "db_entries": stats.db_entries,
                "total_queries": stats.total_queries,
                "cache_hits": stats.cache_hits,
                "hit_rate": format!("{:.1}%", stats.hit_rate * 100.0),
            },
            "version": env!("CARGO_PKG_VERSION"),
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&status).unwrap_or_default(),
        )]))
    }

    #[tool(description = "Run multiple search queries in batch. Returns results for each query.")]
    async fn search_batch(
        &self,
        Parameters(params): Parameters<BatchParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let max = params.max_per_query.unwrap_or(5);
        let mut all_responses = Vec::new();

        for query in params.queries.lines().filter(|l| !l.trim().is_empty()) {
            let query = query.trim();
            let request = SearchRequest {
                query: query.to_string(),
                max_results: max,
                category: SearchCategory::default(),
                time_range: None,
                backends: vec![],
            };

            let start = Instant::now();

            if let Some(cached) = self.router.cache.get(query) {
                all_responses.push(cached);
                continue;
            }

            match self.router.search(&request).await {
                Ok(raw) => {
                    let backends_used: Vec<String> = raw
                        .iter()
                        .map(|r| r.source.split(':').next().unwrap_or("unknown").to_string())
                        .collect::<std::collections::HashSet<_>>()
                        .into_iter()
                        .collect();

                    let fused = fusion::fuse(raw, max);
                    let elapsed = start.elapsed().as_millis() as u64;

                    let response = SearchResponse {
                        results: fused,
                        query: query.to_string(),
                        backends_used,
                        cached: false,
                        total_time_ms: elapsed,
                    };
                    self.router.cache.set(query, &response);
                    all_responses.push(response);
                }
                Err(e) => {
                    tracing::warn!("batch query '{}' failed: {}", query, e);
                    all_responses.push(SearchResponse {
                        results: vec![],
                        query: query.to_string(),
                        backends_used: vec![],
                        cached: false,
                        total_time_ms: start.elapsed().as_millis() as u64,
                    });
                }
            }
        }

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&all_responses).unwrap_or_default(),
        )]))
    }
}

impl ServerHandler for SearchService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "AI-native search MCP server. Aggregates Brave, Exa, SearXNG, Tavily, and DuckDuckGo \
                 with intelligent caching and result fusion. Set API keys via env vars: \
                 BRAVE_API_KEY, EXA_API_KEY, TAVILY_API_KEY, SEARXNG_URL."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            ..Default::default()
        }
    }
}

pub async fn run_server(router: Router) -> anyhow::Result<()> {
    let service = SearchService::new(router);
    let server = service.serve(stdio()).await.inspect_err(|e| {
        tracing::error!("MCP server error: {:?}", e);
    })?;
    server.waiting().await?;
    Ok(())
}

fn parse_category(s: &str) -> SearchCategory {
    match s.to_lowercase().as_str() {
        "news" => SearchCategory::News,
        "academic" | "science" => SearchCategory::Academic,
        "code" | "it" => SearchCategory::Code,
        "images" => SearchCategory::Images,
        _ => SearchCategory::General,
    }
}

fn parse_time_range(s: &str) -> Option<TimeRange> {
    match s.to_lowercase().as_str() {
        "day" | "d" | "24h" => Some(TimeRange::Day),
        "week" | "w" | "7d" => Some(TimeRange::Week),
        "month" | "m" | "30d" => Some(TimeRange::Month),
        "year" | "y" | "365d" => Some(TimeRange::Year),
        _ => None,
    }
}
