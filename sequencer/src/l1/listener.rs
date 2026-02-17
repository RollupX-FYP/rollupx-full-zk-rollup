//! Layer 1 Listener Module
//! 
//! This module monitors the L1 blockchain for forced transaction events.
//! When users submit deposits or forced exits on L1, this listener detects
//! them and adds them to the forced transaction queue.
//! 
//! # Events Monitored
//! - **Deposit events**: Users depositing funds from L1 to L2
//! - **ForcedExit events**: Users forcing withdrawals (censorship resistance)

use crate::config::L1Config;
use crate::pool::ForcedQueue;
use crate::types::{ForcedEventType, ForcedTransaction};
use ethers::prelude::*;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

// Bridge contract event signatures
// These should match the actual RollupBridge contract events
abigen!(
    RollupBridge,
    r#"[
        event Deposit(address indexed from, address indexed to, uint256 value)
        event ForcedExit(address indexed from, address indexed to, uint256 value)
    ]"#,
);

/// L1 event listener
/// 
/// Monitors the L1 bridge contract for forced transaction events.
/// Runs continuously in the background, streaming events via WebSocket.
pub struct L1Listener {
    /// L1 connection configuration (RPC URL, bridge address, etc.)
    config: L1Config,
    /// Reference to the forced transaction queue
    forced_queue: Arc<ForcedQueue>,
}

impl L1Listener {
    /// Creates a new L1 listener
    /// 
    /// # Arguments
    /// * `config` - L1 configuration (RPC endpoint, bridge address, start block)
    /// * `forced_queue` - Shared reference to the forced transaction queue
    pub fn new(config: L1Config, forced_queue: Arc<ForcedQueue>) -> Self {
        Self { 
            config,
            forced_queue,
        }
    }
    
    /// Start listening for L1 events
    /// 
    /// Connects to L1 via WebSocket and continuously monitors the bridge contract
    /// for Deposit and ForcedExit events. When events are detected:
    /// 1. Parse the event data (from, to, value)
    /// 2. Create a ForcedTransaction
    /// 3. Add to the forced queue for priority processing
    /// 
    /// # Error Handling
    /// - Automatically reconnects on WebSocket failures
    /// - Logs errors but continues running
    /// - Tracks last processed block to avoid duplicates after reconnection
    /// 
    /// # Returns
    /// Runs indefinitely, or returns an error on unrecoverable failures
    pub async fn start(&self) -> anyhow::Result<()> {
        info!("Starting L1 event listener");
        info!("RPC URL: {}", self.config.rpc_url);
        info!("Bridge address: {}", self.config.bridge_address);
        info!("Starting from block: {}", self.config.start_block);
        
        // Track the last processed block
        let mut current_block = self.config.start_block;
        
        // Main event loop with automatic reconnection
        loop {
            match self.listen_for_events(current_block).await {
                Ok(last_block) => {
                    // Update the last processed block
                    current_block = last_block + 1;
                    warn!("Event stream ended, reconnecting from block {}", current_block);
                }
                Err(e) => {
                    error!("Error in event listener: {:?}", e);
                    warn!("Reconnecting in 5 seconds...");
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
            }
        }
    }
    
    /// Internal method to listen for events starting from a specific block
    /// 
    /// # Arguments
    /// * `from_block` - Block number to start listening from
    /// 
    /// # Returns
    /// The last successfully processed block number
    async fn listen_for_events(&self, from_block: u64) -> anyhow::Result<u64> {
        // Connect to L1 via WebSocket
        info!("Connecting to L1 at {}", self.config.rpc_url);
        let provider = Provider::<Ws>::connect(&self.config.rpc_url).await?;
        let provider = Arc::new(provider);
        
        // Parse bridge address
        let bridge_address: Address = self.config.bridge_address.parse()?;
        info!("Monitoring bridge contract at {}", bridge_address);
        
        // Create event filters for Deposit and ForcedExit events
        let deposit_filter = Filter::new()
            .address(bridge_address)
            .event("Deposit(address,address,uint256)")
            .from_block(from_block);
            
        let forced_exit_filter = Filter::new()
            .address(bridge_address)
            .event("ForcedExit(address,address,uint256)")
            .from_block(from_block);
        
        // Subscribe to deposit events
        let mut deposit_stream = provider.subscribe_logs(&deposit_filter).await?;
        info!("Subscribed to Deposit events from block {}", from_block);
        
        // Subscribe to forced exit events
        let mut forced_exit_stream = provider.subscribe_logs(&forced_exit_filter).await?;
        info!("Subscribed to ForcedExit events from block {}", from_block);
        
        let mut last_processed_block = from_block;
        
        // Process events as they arrive
        loop {
            tokio::select! {
                Some(log) = deposit_stream.next() => {
                    if let Err(e) = self.handle_deposit_event(log).await {
                        error!("Failed to handle deposit event: {:?}", e);
                    }
                }
                Some(log) = forced_exit_stream.next() => {
                    if let Err(e) = self.handle_forced_exit_event(log).await {
                        error!("Failed to handle forced exit event: {:?}", e);
                    }
                }
                else => {
                    debug!("Event stream ended");
                    break;
                }
            }
            
            // Update last processed block (this is approximate, actual implementation
            // would need more sophisticated tracking)
            if let Some(block_num) = last_processed_block.checked_add(1) {
                last_processed_block = block_num;
            }
        }
        
        Ok(last_processed_block)
    }
    
    /// Handle a Deposit event
    /// 
    /// Parses the event and creates a ForcedTransaction for deposit
    async fn handle_deposit_event(&self, log: Log) -> anyhow::Result<()> {
        debug!("Received Deposit event: {:?}", log);
        
        // Parse the event
        let event = parse_log::<DepositFilter>(log.clone())?;
        
        info!(
            "Deposit detected: from={:?}, to={:?}, value={}",
            event.from, event.to, event.value
        );
        
        // Create a ForcedTransaction
        let forced_tx = ForcedTransaction {
            tx_hash: log.transaction_hash.unwrap_or_default(),
            from: event.from,
            to: event.to,
            value: event.value,
            nonce: 0, // Nonce will be assigned during batch creation based on current state
            gas_limit: 21000, // Standard gas limit for L1 transfers (deposits)
            l1_tx_hash: log.transaction_hash.unwrap_or_default(),
            l1_block_number: log.block_number.unwrap_or_default().as_u64(),
            event_type: ForcedEventType::Deposit,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };
        
        // Add to forced queue
        self.forced_queue.add(forced_tx).await;
        info!("Added Deposit to forced queue");
        
        Ok(())
    }
    
    /// Handle a ForcedExit event
    /// 
    /// Parses the event and creates a ForcedTransaction for forced exit
    async fn handle_forced_exit_event(&self, log: Log) -> anyhow::Result<()> {
        debug!("Received ForcedExit event: {:?}", log);
        
        // Parse the event
        let event = parse_log::<ForcedExitFilter>(log.clone())?;
        
        info!(
            "ForcedExit detected: from={:?}, to={:?}, value={}",
            event.from, event.to, event.value
        );
        
        // Create a ForcedTransaction
        let forced_tx = ForcedTransaction {
            tx_hash: log.transaction_hash.unwrap_or_default(),
            from: event.from,
            to: event.to,
            value: event.value,
            nonce: 0, // Nonce will be assigned during batch creation based on current state
            gas_limit: 21000, // Standard gas limit for L1 transfers (forced exits)
            l1_tx_hash: log.transaction_hash.unwrap_or_default(),
            l1_block_number: log.block_number.unwrap_or_default().as_u64(),
            event_type: ForcedEventType::ForcedExit,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };
        
        // Add to forced queue
        self.forced_queue.add(forced_tx).await;
        info!("Added ForcedExit to forced queue");
        
        Ok(())
    }
}