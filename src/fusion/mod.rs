use crate::types::SearchResult;
use std::collections::HashSet;

/// Deduplicate results by URL and re-rank by normalized score.
pub fn fuse(results: Vec<SearchResult>, max_results: usize) -> Vec<SearchResult> {
    let mut seen_urls: HashSet<String> = HashSet::new();
    let mut deduped: Vec<SearchResult> = Vec::new();

    for result in results {
        let normalized_url = normalize_url(&result.url);
        if seen_urls.contains(&normalized_url) {
            // Boost score of existing result when multiple backends agree
            if let Some(existing) = deduped
                .iter_mut()
                .find(|r| normalize_url(&r.url) == normalized_url)
            {
                existing.score = (existing.score + result.score) / 2.0 + 0.1; // Agreement bonus
                                                                              // Keep the longer snippet
                if result.snippet.len() > existing.snippet.len() {
                    existing.snippet = result.snippet;
                }
            }
            continue;
        }
        seen_urls.insert(normalized_url);
        deduped.push(result);
    }

    // Sort by score descending
    deduped.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    deduped.truncate(max_results);
    deduped
}

/// Normalize URL for deduplication (strip trailing slash, www, protocol).
fn normalize_url(url: &str) -> String {
    url.trim()
        .to_lowercase()
        .replace("https://", "")
        .replace("http://", "")
        .replace("www.", "")
        .trim_end_matches('/')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(title: &str, url: &str, source: &str, score: f64) -> SearchResult {
        SearchResult {
            title: title.into(),
            url: url.into(),
            snippet: format!("snippet for {}", title),
            source: source.into(),
            score,
            published_date: None,
        }
    }

    #[test]
    fn test_dedup_by_url() {
        let results = vec![
            result("Rust Lang", "https://www.rust-lang.org/", "brave", 0.9),
            result("Rust Programming", "https://rust-lang.org", "exa", 0.8),
            result(
                "Cargo Book",
                "https://doc.rust-lang.org/cargo/",
                "brave",
                0.7,
            ),
        ];

        let fused = fuse(results, 10);
        assert_eq!(fused.len(), 2); // Deduped rust-lang.org
        assert!(fused[0].score > 0.9); // Agreement bonus
    }

    #[test]
    fn test_max_results() {
        let results: Vec<SearchResult> = (0..20)
            .map(|i| {
                result(
                    &format!("Result {}", i),
                    &format!("https://example.com/{}", i),
                    "brave",
                    1.0 - i as f64 * 0.05,
                )
            })
            .collect();

        let fused = fuse(results, 5);
        assert_eq!(fused.len(), 5);
    }

    #[test]
    fn test_score_ordering() {
        let results = vec![
            result("Low", "https://low.com", "brave", 0.3),
            result("High", "https://high.com", "exa", 0.9),
            result("Mid", "https://mid.com", "tavily", 0.6),
        ];

        let fused = fuse(results, 10);
        assert_eq!(fused[0].title, "High");
        assert_eq!(fused[1].title, "Mid");
        assert_eq!(fused[2].title, "Low");
    }
}
