use super::{async_trait, SearchBackend};
use crate::types::{SearchRequest, SearchResult};
use reqwest::Client;
use serde::Deserialize;

pub struct BraveBackend {
    api_key: String,
    client: Client,
}

#[derive(Deserialize)]
struct BraveResponse {
    #[serde(default)]
    web: Option<BraveWeb>,
}

#[derive(Deserialize)]
struct BraveWeb {
    #[serde(default)]
    results: Vec<BraveResult>,
}

#[derive(Deserialize)]
struct BraveResult {
    title: String,
    url: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    page_age: Option<String>,
}

impl BraveBackend {
    pub fn from_env() -> Option<Self> {
        let api_key = std::env::var("BRAVE_API_KEY").ok()?;
        Some(Self {
            api_key,
            client: Client::new(),
        })
    }
}

#[async_trait]
impl SearchBackend for BraveBackend {
    fn name(&self) -> &str {
        "brave"
    }

    fn is_available(&self) -> bool {
        true
    }

    fn cost_per_query(&self) -> f64 {
        0.005 // $5/1000 queries
    }

    async fn search(&self, request: &SearchRequest) -> anyhow::Result<Vec<SearchResult>> {
        let mut url = format!(
            "https://api.search.brave.com/res/v1/web/search?q={}&count={}",
            urlencoding::encode(&request.query),
            request.max_results
        );

        if let Some(ref tr) = request.time_range {
            let freshness = match tr {
                crate::types::TimeRange::Day => "pd",
                crate::types::TimeRange::Week => "pw",
                crate::types::TimeRange::Month => "pm",
                crate::types::TimeRange::Year => "py",
            };
            url.push_str(&format!("&freshness={}", freshness));
        }

        let resp: BraveResponse = self
            .client
            .get(&url)
            .header("X-Subscription-Token", &self.api_key)
            .header("Accept", "application/json")
            .send()
            .await?
            .json()
            .await?;

        let results = resp
            .web
            .map(|w| w.results)
            .unwrap_or_default()
            .into_iter()
            .enumerate()
            .map(|(i, r)| SearchResult {
                title: r.title,
                url: r.url,
                snippet: r.description,
                source: "brave".into(),
                score: 1.0 - (i as f64 * 0.05),
                published_date: r.page_age,
            })
            .collect();

        Ok(results)
    }
}
