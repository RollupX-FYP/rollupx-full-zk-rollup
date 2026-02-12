//! Batch Engine Module
//! 
//! This module is responsible for creating sealed batches from transactions.
//! Each batch is assigned a unique sequential ID and timestamp.

use crate::{Batch, Transaction, config::BatchConfig};
use ethers::types::H256;

/// Batch creation engine
/// 
/// Creates sealed batches from ordered transactions.
/// Maintains a sequential batch ID counter.
pub struct BatchEngine {
    /// Batch configuration (max size, limits, etc.)
    config: BatchConfig,
    /// Next batch ID to assign (starts at 1, increments for each batch)
    next_batch_id: u64,
}

impl BatchEngine {
    /// Creates a new batch engine
    /// 
    /// # Arguments
    /// * `config` - Batch configuration settings
    pub fn new(config: BatchConfig) -> Self {
        Self {
            config,
            next_batch_id: 1, // Batches start from ID 1
        }
    }
    
    /// Create a new batch from transactions
    /// 
    /// Seals the transactions into a batch with a unique ID and timestamp.
    /// The batch ID is automatically incremented for the next batch.
    /// 
    /// # Arguments
    /// * `transactions` - Ordered list of transactions (forced first, then normal)
    /// 
    /// # Returns
    /// A sealed `Batch` ready to be executed and posted to L1
    pub fn create_batch(&mut self, transactions: Vec<Transaction>) -> Batch {
        // Create the batch structure
        let batch = Batch {
            batch_id: self.next_batch_id,
            transactions,
            prev_state_root: H256::zero(), // TODO: Track actual state root
            timestamp: chrono::Utc::now().timestamp() as u64,
        };
        
        // Increment ID for next batch
        self.next_batch_id += 1;
        batch
    }
    
    /// Check if adding a transaction would exceed the gas limit
    /// 
    /// Used by the orchestrator to enforce gas limits when building batches.
    /// 
    /// # Arguments
    /// * `current_txs` - Transactions already in the batch
    /// * `new_tx` - Transaction being considered for addition
    /// 
    /// # Returns
    /// `true` if adding the new transaction would keep total gas under the limit,
    /// `false` if it would exceed the configured `max_gas_limit`
    pub fn can_add_transaction(&self, current_txs: &[Transaction], new_tx: &Transaction) -> bool {
        let current_gas: u64 = current_txs.iter().map(|tx| tx.gas_limit()).sum();
        let total_gas = current_gas.saturating_add(new_tx.gas_limit());
        total_gas <= self.config.max_gas_limit
    }
}