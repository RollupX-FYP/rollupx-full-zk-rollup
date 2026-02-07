use sequencer::{
    api::Server,
    config::Config,
    state::StateCache,
    pool::{ForcedQueue, TransactionPool},
    l1::L1Listener,
};
use std::sync::Arc;
use tracing::info;

/// The main entry point for the sequencer application.
///
/// This function initializes logging, loads the application configuration,
/// sets up shared resources (state cache, transaction pools), starts the L1
/// event listener in the background, and starts the API server.
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
    
    // Initialize shared resources
    // All shared state is created here and passed to components that need it
    
    // State cache: stores account balances and nonces for validation
    let state_cache = StateCache::new();
    
    // Transaction pool: stores normal pending transactions from users
    let tx_pool = Arc::new(TransactionPool::new());
    
    // Forced queue: stores priority transactions from L1 (deposits, forced exits)
    let forced_queue = Arc::new(ForcedQueue::new());
    
    // Create the L1 event listener
    let l1_listener = L1Listener::new(config.l1.clone(), forced_queue.clone());
    
    // Start the L1 listener in the background
    // This spawns a new async task that monitors L1 for forced transactions
    tokio::spawn(async move {
        if let Err(e) = l1_listener.start().await {
            tracing::error!("L1 listener error: {:?}", e);
        }
    });
    info!("L1 event listener started");
    
    // Create a new API server instance.
    // Pass all shared resources so the server can validate and store transactions.
    let server = Server::new(config, state_cache, tx_pool, forced_queue);
    // Start the API server. This will typically bind to a port and begin
    // listening for incoming requests. The `?` operator propagates any
    // errors that occur during server startup.
    server.start().await?;
    
    // Return `Ok(())` to indicate successful execution of the main function.
    Ok(())
}