use crate::{Batch, Transaction, config::BatchConfig};
use ethers::types::H256;

pub struct BatchEngine {
    config: BatchConfig,
    next_batch_id: u64,
}

impl BatchEngine {
    pub fn new(config: BatchConfig) -> Self {
        Self {
            config,
            next_batch_id: 1,
        }
    }
    
    pub fn create_batch(&mut self, transactions: Vec<Transaction>) -> Batch {
        let batch = Batch {
            batch_id: self.next_batch_id,
            transactions,
            prev_state_root: H256::zero(),
            timestamp: chrono::Utc::now().timestamp() as u64,
        };
        
        self.next_batch_id += 1;
        batch
    }
}