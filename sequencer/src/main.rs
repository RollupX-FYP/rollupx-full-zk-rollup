use sequencer::{
    api::Server,
    config::Config,
};
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Load config
    let config = Config::load("config/default.toml")?;
    info!("Sequencer starting with config: {:?}", config);
    
    // Start API server
    let server = Server::new(config);
    server.start().await?;
    
    Ok(())
}