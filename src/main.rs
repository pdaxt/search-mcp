mod backends;
mod cache;
mod fusion;
mod server;
mod types;

use clap::Parser;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "search-mcp", about = "AI-native search MCP server")]
struct Cli {
    /// Cache directory
    #[arg(long, default_value = "~/.cache/search-mcp")]
    cache_dir: String,

    /// Max cache age in seconds (default: 1 hour)
    #[arg(long, default_value_t = 3600)]
    cache_ttl: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("search_mcp=info".parse()?))
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    let cache_dir = cli.cache_dir.replace('~', &dirs_home());
    std::fs::create_dir_all(&cache_dir)?;

    let cache = cache::Cache::new(&cache_dir, cli.cache_ttl)?;
    let router = backends::Router::from_env(cache);

    tracing::info!("search-mcp v{} starting", env!("CARGO_PKG_VERSION"));
    tracing::info!("backends: {}", router.available_backends().join(", "));

    server::run_server(router).await
}

fn dirs_home() -> String {
    std::env::var("HOME").unwrap_or_else(|_| "/tmp".into())
}
