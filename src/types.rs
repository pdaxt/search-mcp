use serde::{Deserialize, Serialize};

/// A single search result from any backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub source: String,
    pub score: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_date: Option<String>,
}

/// A search request with all parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    #[serde(default = "default_max_results")]
    pub max_results: usize,
    #[serde(default)]
    pub category: SearchCategory,
    #[serde(default)]
    pub time_range: Option<TimeRange>,
    /// Prefer specific backends (empty = auto-route)
    #[serde(default)]
    pub backends: Vec<String>,
}

fn default_max_results() -> usize {
    10
}

/// Search response with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
    pub query: String,
    pub backends_used: Vec<String>,
    pub cached: bool,
    pub total_time_ms: u64,
}

/// Search categories for routing.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SearchCategory {
    #[default]
    General,
    News,
    Academic,
    Code,
    Images,
}

/// Time range filter.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TimeRange {
    Day,
    Week,
    Month,
    Year,
}

// BackendStatus is used by the search_status tool via serde_json::json!
// and doesn't need a dedicated struct.

impl std::fmt::Display for SearchCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::General => write!(f, "general"),
            Self::News => write!(f, "news"),
            Self::Academic => write!(f, "academic"),
            Self::Code => write!(f, "code"),
            Self::Images => write!(f, "images"),
        }
    }
}

impl std::fmt::Display for TimeRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Day => write!(f, "day"),
            Self::Week => write!(f, "week"),
            Self::Month => write!(f, "month"),
            Self::Year => write!(f, "year"),
        }
    }
}
