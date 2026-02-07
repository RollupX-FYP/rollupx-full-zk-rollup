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
}