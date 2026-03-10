use super::{async_trait, SearchBackend};
use crate::types::{SearchRequest, SearchResult};
use reqwest::Client;
use serde::{Deserialize, Serialize};

pub struct TavilyBackend {
    api_key: String,
    client: Client,
}

#[derive(Serialize)]
struct TavilyRequest {
    api_key: String,
    query: String,
    max_results: usize,
    search_depth: String,
    include_answer: bool,
}

#[derive(Deserialize)]
struct TavilyResponse {
    #[serde(default)]
    results: Vec<TavilyResult>,
}

#[derive(Deserialize)]
struct TavilyResult {
    title: String,
    url: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    score: f64,
    #[serde(default)]
    published_date: Option<String>,
}

impl TavilyBackend {
    pub fn from_env() -> Option<Self> {
        let api_key = std::env::var("TAVILY_API_KEY").ok()?;
        Some(Self {
            api_key,
            client: Client::new(),
        })
    }
}

#[async_trait]
impl SearchBackend for TavilyBackend {
    fn name(&self) -> &str {
        "tavily"
    }

    fn is_available(&self) -> bool {
        true
    }

    fn cost_per_query(&self) -> f64 {
        0.008 // ~$8/1000 credits
    }

    async fn search(&self, request: &SearchRequest) -> anyhow::Result<Vec<SearchResult>> {
        let body = TavilyRequest {
            api_key: self.api_key.clone(),
            query: request.query.clone(),
            max_results: request.max_results,
            search_depth: "basic".into(),
            include_answer: false,
        };

        let resp: TavilyResponse = self
            .client
            .post("https://api.tavily.com/search")
            .json(&body)
            .send()
            .await?
            .json()
            .await?;

        let results = resp
            .results
            .into_iter()
            .map(|r| SearchResult {
                title: r.title,
                url: r.url,
                snippet: r.content,
                source: "tavily".into(),
                score: r.score,
                published_date: r.published_date,
            })
            .collect();

        Ok(results)
    }
}
