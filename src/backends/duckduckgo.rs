use super::{async_trait, SearchBackend};
use crate::types::{SearchRequest, SearchResult};
use reqwest::Client;
use serde::Deserialize;

/// DuckDuckGo Instant Answer API (free, no key needed).
/// Note: This returns instant answers, not full web results.
/// It's the free fallback when no paid APIs are configured.
pub struct DuckDuckGoBackend {
    client: Client,
}

#[derive(Deserialize)]
struct DdgResponse {
    #[serde(rename = "AbstractText", default)]
    abstract_text: String,
    #[serde(rename = "AbstractURL", default)]
    abstract_url: String,
    #[serde(rename = "AbstractSource", default)]
    abstract_source: String,
    #[serde(rename = "Heading", default)]
    heading: String,
    #[serde(rename = "RelatedTopics", default)]
    related_topics: Vec<DdgTopic>,
}

#[derive(Deserialize)]
struct DdgTopic {
    #[serde(rename = "Text", default)]
    text: String,
    #[serde(rename = "FirstURL", default)]
    first_url: String,
}

impl DuckDuckGoBackend {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

#[async_trait]
impl SearchBackend for DuckDuckGoBackend {
    fn name(&self) -> &str {
        "duckduckgo"
    }

    fn is_available(&self) -> bool {
        true // Always available
    }

    fn cost_per_query(&self) -> f64 {
        0.0 // Free
    }

    async fn search(&self, request: &SearchRequest) -> anyhow::Result<Vec<SearchResult>> {
        let url = format!(
            "https://api.duckduckgo.com/?q={}&format=json&no_html=1&skip_disambig=1",
            urlencoding::encode(&request.query),
        );

        let resp: DdgResponse = self.client.get(&url).send().await?.json().await?;

        let mut results = Vec::new();

        // Add abstract if available
        if !resp.abstract_text.is_empty() {
            results.push(SearchResult {
                title: resp.heading.clone(),
                url: resp.abstract_url.clone(),
                snippet: resp.abstract_text,
                source: format!("duckduckgo:{}", resp.abstract_source),
                score: 1.0,
                published_date: None,
            });
        }

        // Add related topics
        for (i, topic) in resp.related_topics.into_iter().enumerate() {
            if results.len() >= request.max_results {
                break;
            }
            if !topic.first_url.is_empty() {
                // Extract title from text (first sentence before the dash)
                let title = topic
                    .text
                    .split(" - ")
                    .next()
                    .unwrap_or(&topic.text)
                    .to_string();
                results.push(SearchResult {
                    title,
                    url: topic.first_url,
                    snippet: topic.text,
                    source: "duckduckgo".into(),
                    score: 0.8 - (i as f64 * 0.05),
                    published_date: None,
                });
            }
        }

        Ok(results)
    }
}
