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

/// L1 event listener
/// 
/// Monitors the L1 bridge contract for forced transaction events.
/// Runs continuously in the background, polling for new events.
pub struct L1Listener {
    /// L1 connection configuration (RPC URL, bridge address, etc.)
    config: L1Config,
}

impl L1Listener {
    /// Creates a new L1 listener
    /// 
    /// # Arguments
    /// * `config` - L1 configuration (RPC endpoint, bridge address, start block)
    pub fn new(config: L1Config) -> Self {
        Self { config }
    }
    
    /// Start listening for L1 events
    /// 
    /// # Planned Functionality
    /// 1. Connect to L1 using the configured RPC endpoint
    /// 2. Subscribe to events from the bridge contract
    /// 3. For each event:
    ///    - Parse the event data (from, to, value, type)
    ///    - Create a ForcedTransaction
    ///    - Add to the forced queue
    /// 4. Handle reorgs and missed events
    /// 
    /// # Returns
    /// Runs indefinitely, or returns an error if connection fails
    pub async fn start(&self) -> anyhow::Result<()> {
        // TODO: Connect to L1 and listen for events
        // - Use ethers-rs to connect to L1 RPC
        // - Filter events from bridge_address starting at start_block
        // - Parse Deposit and ForcedExit events
        // - Add parsed events to forced transaction queue
        Ok(())
    }
}