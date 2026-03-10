<div align="center">

# search-mcp

**AI-native web search as an MCP server. One tool, five engines, intelligent caching.**

[![CI](https://github.com/pdaxt/search-mcp/actions/workflows/ci.yml/badge.svg)](https://github.com/pdaxt/search-mcp/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-search--mcp-orange.svg)](https://github.com/pdaxt/search-mcp)

</div>

---

## The Problem

Every AI agent needs web search. But search APIs are expensive ($5-10/1K queries), each has different formats, and your agents keep searching the same things. You're paying 3x what you should and juggling 5 different API keys.

## The Solution

`search-mcp` is a Rust MCP server that aggregates **Brave, Exa, SearXNG, Tavily, and DuckDuckGo** into a single `search` tool. It caches aggressively (50-80% hit rate), routes to the cheapest backend first, and deduplicates results across engines. One binary, zero Python dependencies.

## Quick Start

```bash
# Build from source
git clone https://github.com/pdaxt/search-mcp.git
cd search-mcp
cargo install --path .
```

Add to your Claude Code config (`~/.claude.json`):

```json
{
  "mcpServers": {
    "search": {
      "command": "search-mcp",
      "env": {
        "BRAVE_API_KEY": "your-key",
        "EXA_API_KEY": "your-key"
      }
    }
  }
}
```

## Features

| Feature | Description |
|---------|-------------|
| **5 backends** | Brave, Exa, Tavily, SearXNG, DuckDuckGo |
| **Cost-aware routing** | Cheapest backend first, fallback on failure |
| **Two-tier cache** | In-memory (DashMap) + persistent (SQLite) |
| **Result fusion** | Dedup by URL, score normalization, agreement bonus |
| **Batch search** | Multiple queries in one call |
| **Categories** | general, news, academic, code, images |
| **Time filtering** | day, week, month, year |
| **Query analytics** | Hit rate, latency, backend usage stats |
| **Single binary** | No Python, no Node, no Docker required |

## MCP Tools

### `search`

Search the web with automatic backend selection and caching.

```json
{
  "query": "rust async runtime comparison 2026",
  "max_results": 10,
  "category": "code",
  "time_range": "month",
  "backends": "brave,exa"
}
```

### `search_status`

Show available backends, cache stats, and configuration.

### `search_batch`

Run multiple queries in one call. Cached results are returned instantly.

```json
{
  "queries": "tokio vs async-std\nrust error handling best practices\nrmcp mcp server tutorial",
  "max_per_query": 5
}
```

## Environment Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `BRAVE_API_KEY` | No | Brave Search API key ([get one](https://brave.com/search/api/)) |
| `EXA_API_KEY` | No | Exa neural search API key ([get one](https://exa.ai/)) |
| `TAVILY_API_KEY` | No | Tavily AI search API key ([get one](https://tavily.com/)) |
| `SEARXNG_URL` | No | Self-hosted SearXNG instance URL |

DuckDuckGo is always available as a free fallback (no API key needed).

**At least one backend** should be configured for useful results. Brave is recommended as the best quality-to-cost ratio.

## How It Works

```
Agent calls search("rust mcp tutorial")
        │
   ┌────▼────────────┐
   │  Cache Check     │ ← SQLite + in-memory (DashMap)
   │  (normalized     │
   │   query hash)    │
   └────┬────────────┘
        │ miss
   ┌────▼────────────┐
   │  Cost Router     │ ← Sort backends by $/query
   │  Free first:     │   SearXNG ($0) → DuckDuckGo ($0)
   │  Then paid:      │   Exa ($0.003) → Brave ($0.005) → Tavily ($0.008)
   └────┬────────────┘
        │
   ┌────▼────────────┐
   │  Result Fusion   │ ← Dedup by URL, normalize scores
   │  Agreement bonus │   Results from 2+ engines score higher
   └────┬────────────┘
        │
   ┌────▼────────────┐
   │  Cache Store     │ ← Save for TTL (default: 1 hour)
   └─────────────────┘
```

## Cost Comparison

| Approach | 10K queries/month | Notes |
|----------|-------------------|-------|
| Brave API direct | $50 | No caching |
| Tavily direct | $80 | No caching |
| **search-mcp** (Brave + cache) | **$15-25** | 50-80% cache hit rate |
| **search-mcp** (SearXNG + Brave fallback) | **$5-10** | SearXNG handles most queries free |

## Configuration

```bash
# Custom cache directory
search-mcp --cache-dir /tmp/search-cache

# Shorter cache TTL (10 minutes)
search-mcp --cache-ttl 600
```

## Architecture

```
src/
├── main.rs              # CLI + MCP server startup
├── server.rs            # MCP tool definitions (search, search_status, search_batch)
├── types.rs             # SearchRequest, SearchResult, SearchResponse
├── backends/
│   ├── mod.rs           # Router (cost-aware backend selection)
│   ├── brave.rs         # Brave Search API
│   ├── exa.rs           # Exa neural search API
│   ├── tavily.rs        # Tavily AI search API
│   ├── searxng.rs       # SearXNG metasearch
│   └── duckduckgo.rs    # DuckDuckGo Instant Answers (free fallback)
├── cache/
│   └── mod.rs           # Two-tier cache (DashMap + SQLite)
└── fusion/
    └── mod.rs           # Result dedup + score normalization
```

## Contributing

```bash
cargo test        # Run tests
cargo clippy      # Lint
cargo fmt         # Format
```

PRs welcome. Add new backends by implementing the `SearchBackend` trait.

## License

MIT
