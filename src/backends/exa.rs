use super::{async_trait, SearchBackend};
use crate::types::{SearchRequest, SearchResult};
use reqwest::Client;
use serde::{Deserialize, Serialize};

pub struct ExaBackend {
    api_key: String,
    client: Client,
}

#[derive(Serialize)]
struct ExaRequest {
    query: String,
    num_results: usize,
    #[serde(rename = "type")]
    search_type: String,
    contents: ExaContents,
}

#[derive(Serialize)]
struct ExaContents {
    text: ExaText,
}

#[derive(Serialize)]
struct ExaText {
    max_characters: usize,
}

#[derive(Deserialize)]
struct ExaResponse {
    #[serde(default)]
    results: Vec<ExaResult>,
}

#[derive(Deserialize)]
struct ExaResult {
    #[serde(default)]
    title: String,
    url: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    score: Option<f64>,
    #[serde(default, rename = "publishedDate")]
    published_date: Option<String>,
}

impl ExaBackend {
    pub fn from_env() -> Option<Self> {
        let api_key = std::env::var("EXA_API_KEY").ok()?;
        Some(Self {
            api_key,
            client: Client::new(),
        })
    }
}

#[async_trait]
impl SearchBackend for ExaBackend {
    fn name(&self) -> &str {
        "exa"
    }

    fn is_available(&self) -> bool {
        true
    }

    fn cost_per_query(&self) -> f64 {
        0.003 // $3/1000 queries
    }

    async fn search(&self, request: &SearchRequest) -> anyhow::Result<Vec<SearchResult>> {
        let body = ExaRequest {
            query: request.query.clone(),
            num_results: request.max_results,
            search_type: "auto".into(),
            contents: ExaContents {
                text: ExaText {
                    max_characters: 300,
                },
            },
        };

        let resp: ExaResponse = self
            .client
            .post("https://api.exa.ai/search")
            .header("x-api-key", &self.api_key)
            .json(&body)
            .send()
            .await?
            .json()
            .await?;

        let results = resp
            .results
            .into_iter()
            .enumerate()
            .map(|(i, r)| SearchResult {
                title: r.title,
                url: r.url,
                snippet: if r.text.is_empty() {
                    "(no snippet)".into()
                } else {
                    r.text
                },
                source: "exa".into(),
                score: r.score.unwrap_or(1.0 - (i as f64 * 0.05)),
                published_date: r.published_date,
            })
            .collect();

        Ok(results)
    }
}
