//! Forced Transaction Queue Module
//! 
//! This module implements a queue for forced transactions from Layer 1.
//! Forced transactions (deposits and forced exits) must be included in batches
//! to maintain censorship resistance.

use crate::ForcedTransaction;
use std::collections::VecDeque;
use tokio::sync::RwLock;

/// Queue for forced transactions from L1
/// 
/// Stores forced transactions (deposits and forced exits) that originated from L1.
/// These transactions bypass normal validation and MUST be included in batches.
/// This ensures censorship resistance - users can always force inclusion via L1.
pub struct ForcedQueue {
    /// Queue of forced transactions, protected by a read-write lock
    transactions: RwLock<VecDeque<ForcedTransaction>>,
}

impl ForcedQueue {
    /// Creates a new empty forced transaction queue
    pub fn new() -> Self {
        Self {
            transactions: RwLock::new(VecDeque::new()),
        }
    }
    
    /// Add a forced transaction from L1
    /// 
    /// Called by the L1 listener when it detects a deposit or forced exit event.
    /// These transactions are added to the queue to be included in the next batch.
    /// 
    /// # Arguments
    /// * `tx` - The forced transaction to add
    pub async fn add(&self, tx: ForcedTransaction) {
        // Acquire write lock to add transaction
        let mut txs = self.transactions.write().await;
        txs.push_back(tx);
    }
    
    /// Get all forced transactions and clear the queue
    /// 
    /// Called by the batch engine to retrieve all pending forced transactions.
    /// Forced transactions are ALWAYS included first in batches (before normal txs).
    /// The queue is cleared after retrieval.
    /// 
    /// # Returns
    /// All forced transactions currently in the queue
    pub async fn get_all(&self) -> Vec<ForcedTransaction> {
        // Acquire write lock to drain all transactions
        let mut txs = self.transactions.write().await;
        // Drain all transactions (clear the queue)
        txs.drain(..).collect()
    }
}