use sequencer::{
    api::Server,
    config::Config,
    state::StateCache,
};
use tracing::info;

/// The main entry point for the sequencer application.
///
/// This function initializes logging, loads the application configuration,
/// sets up the state cache, and starts the API server.
#[tokio::main] // Marks the async main function to be run by the Tokio runtime.
async fn main() -> anyhow::Result<()> {
    // Initialize logging using tracing_subscriber.
    // This sets up a default formatter that prints logs to stdout.
    tracing_subscriber::fmt::init();
    
    // Load the application configuration from the specified TOML file.
    // The `?` operator propagates any errors that occur during loading.
    let config = Config::load("config/default.toml")?;
    // Log the loaded configuration for debugging and informational purposes.
    info!("Sequencer starting with config: {:?}", config);
    
    // Initialize the state cache.
    // This cache is used to store and retrieve application state efficiently.
    let state_cache = StateCache::new();
    
    // Create a new API server instance.
    // It takes the loaded configuration and the initialized state cache.
    let server = Server::new(config, state_cache);
    // Start the API server. This will typically bind to a port and begin
    // listening for incoming requests. The `?` operator propagates any
    // errors that occur during server startup.
    server.start().await?;
    
    // Return `Ok(())` to indicate successful execution of the main function.
    Ok(())
}