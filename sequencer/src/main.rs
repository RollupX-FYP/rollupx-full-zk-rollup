use sequencer::{
    api::Server,
    config::Config,
    state::StateCache,
};
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Load config
    let config = Config::load("config/default.toml")?;
    info!("Sequencer starting with config: {:?}", config);
    
    // Initialize state cache
    let state_cache = StateCache::new();
    
    // Start API server
    let server = Server::new(config, state_cache);
    server.start().await?;
    
    Ok(())
}