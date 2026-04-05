//! RollupX Sequencer — Main Entry Point
//!
//! The sequencer is the central component of the RollupX zk-rollup system.
//! It receives user transactions, validates them, orders them using a
//! configurable scheduling policy, and produces sealed batches for execution.
//!
//! # Startup Sequence
//! 1. Initialize logging
//! 2. Load configuration from TOML file
//! 3. Initialize shared resources (state cache, transaction pools)
//! 4. Initialize the batch metadata registry (SQLite)
//! 5. Start the L1 event listener (background task)
//! 6. Start the batch orchestrator (background task)
//! 7. Start the API server (foreground — blocks until shutdown)
//!
//! # Graceful Shutdown
//! The sequencer handles `Ctrl+C` signals to shut down gracefully.

use sequencer::{
    api::Server,
    config::Config,
    state::StateCache,
    pool::{ForcedQueue, TransactionPool},
    l1::L1Listener,
    registry::Registry,
};
use std::sync::Arc;
use tracing::info;

/// The main entry point for the sequencer application.
///
/// Initializes all components, wires them together, and starts
/// the background tasks and API server.
#[tokio::main] // Marks the async main function to be run by the Tokio runtime.
async fn main() -> anyhow::Result<()> {
    // ── Step 1: Initialize Logging ─────────────────────────────────────
    // Sets up a default tracing subscriber that prints logs to stdout
    // with timestamps and log levels.
    tracing_subscriber::fmt::init();

    // ── Step 2: Load Configuration ─────────────────────────────────────
    // Parse the TOML configuration file into structured config types.
    let config = Config::load("config/default.toml")?;
    info!("Sequencer starting with config: {:?}", config);

    // ── Step 3: Initialize Shared Resources ────────────────────────────
    // All shared state is created here and passed to components that need it.

    // State cache: stores account balances and nonces for fast validation
    let state_cache = StateCache::new();

    // Transaction pool: stores normal pending transactions from users
    let tx_pool = Arc::new(TransactionPool::new());

    // Forced queue: stores priority transactions from L1 (deposits, forced exits)
    let forced_queue = Arc::new(ForcedQueue::new());

    // ── Step 4: Initialize Batch Metadata Registry ─────────────────────
    // Opens a SQLite database and creates the schema if needed.
    // The registry persists batch metadata for auditing and monitoring.
    let registry = Arc::new(
        Registry::new(&config.database.url).await?
    );
    info!("Batch registry initialized");

    // ── Step 5: Start L1 Event Listener ────────────────────────────────
    // Spawns a background task that monitors the L1 bridge contract
    // for deposit and forced exit events.
    let l1_listener = L1Listener::new(config.l1.clone(), forced_queue.clone());

    tokio::spawn(async move {
        if let Err(e) = l1_listener.start().await {
            tracing::error!("L1 listener error: {:?}", e);
        }
    });
    info!("L1 event listener started");

    // ── Step 6: Start Batch Orchestrator ───────────────────────────────
    // Spawns a background task that coordinates batch production by:
    //   - Evaluating trigger conditions (timeout, size, forced txs)
    //   - Pulling transactions from pools
    //   - Ordering them via the configured scheduling policy
    //   - Creating sealed batches
    //   - Storing metadata in the registry
    let orchestrator = sequencer::BatchOrchestrator::new(
        forced_queue.clone(),
        tx_pool.clone(),
        config.batch.clone(),
        config.scheduling.to_policy_type(),
        registry.clone(),
    );

    tokio::spawn(async move {
        if let Err(e) = orchestrator.start().await {
            tracing::error!("Batch orchestrator error: {:?}", e);
        }
    });
    info!("Batch orchestrator started");

    // ── Step 7: Start API Server ───────────────────────────────────────
    // Create and start the JSON-RPC API server.
    // This runs in the foreground and blocks until the server shuts down.
    // Pass shared resources needed for handling user transactions.
    let server = Server::new(config, state_cache, tx_pool);

    // Use tokio::select! to run the server alongside a shutdown signal handler.
    // When Ctrl+C is pressed, the server shuts down gracefully.
    tokio::select! {
        result = server.start() => {
            if let Err(e) = result {
                tracing::error!("API server error: {:?}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Shutdown signal received, shutting down gracefully...");
        }
    }

    info!("Sequencer shut down successfully");
    Ok(())
}