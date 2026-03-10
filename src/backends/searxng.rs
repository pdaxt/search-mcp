use super::{async_trait, SearchBackend};
use crate::types::{SearchRequest, SearchResult};
use reqwest::Client;
use serde::Deserialize;

pub struct SearxngBackend {
    base_url: String,
    client: Client,
}

#[derive(Deserialize)]
struct SearxngResponse {
    #[serde(default)]
    results: Vec<SearxngResult>,
}

#[derive(Deserialize)]
struct SearxngResult {
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    score: f64,
    #[serde(default)]
    #[serde(rename = "publishedDate")]
    published_date: Option<String>,
    #[serde(default)]
    engine: String,
}

impl SearxngBackend {
    pub fn from_env() -> Option<Self> {
        let base_url = std::env::var("SEARXNG_URL").ok()?;
        Some(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: Client::new(),
        })
    }
}

#[async_trait]
impl SearchBackend for SearxngBackend {
    fn name(&self) -> &str {
        "searxng"
    }

    fn is_available(&self) -> bool {
        true
    }

    fn cost_per_query(&self) -> f64 {
        0.0 // Free (self-hosted)
    }

    async fn search(&self, request: &SearchRequest) -> anyhow::Result<Vec<SearchResult>> {
        let category = match request.category {
            crate::types::SearchCategory::General => "general",
            crate::types::SearchCategory::News => "news",
            crate::types::SearchCategory::Academic => "science",
            crate::types::SearchCategory::Code => "it",
            crate::types::SearchCategory::Images => "images",
        };

        let mut url = format!(
            "{}/search?q={}&format=json&categories={}&pageno=1",
            self.base_url,
            urlencoding::encode(&request.query),
            category
        );

        if let Some(ref tr) = request.time_range {
            let range = match tr {
                crate::types::TimeRange::Day => "day",
                crate::types::TimeRange::Week => "week",
                crate::types::TimeRange::Month => "month",
                crate::types::TimeRange::Year => "year",
            };
            url.push_str(&format!("&time_range={}", range));
        }

        let resp: SearxngResponse = self
            .client
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .await?
            .json()
            .await?;

        let results = resp
            .results
            .into_iter()
            .take(request.max_results)
            .enumerate()
            .map(|(i, r)| SearchResult {
                title: r.title,
                url: r.url,
                snippet: r.content,
                source: format!("searxng:{}", r.engine),
                score: if r.score > 0.0 {
                    r.score.min(1.0)
                } else {
                    1.0 - (i as f64 * 0.05)
                },
                published_date: r.published_date,
            })
            .collect();

        Ok(results)
    }
}
