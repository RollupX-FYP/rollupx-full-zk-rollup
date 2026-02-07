//! Transaction Pool Module
//! 
//! This module implements a pool for pending user transactions.
//! Transactions are stored in a FIFO queue and retrieved by the batch engine.

use crate::UserTransaction;
use std::collections::VecDeque;
use tokio::sync::RwLock;

/// Pool for pending user transactions
/// 
/// Stores validated transactions in a FIFO queue waiting to be batched.
/// Uses VecDeque for efficient insertion at the back and removal from the front.
/// Protected by RwLock for concurrent access.
pub struct TransactionPool {
    /// Queue of pending transactions, protected by a read-write lock
    transactions: RwLock<VecDeque<UserTransaction>>,
}

impl TransactionPool {
    /// Creates a new empty transaction pool
    pub fn new() -> Self {
        Self {
            transactions: RwLock::new(VecDeque::new()),
        }
    }
    
    /// Add a validated transaction to the pool
    /// 
    /// Transactions are added to the back of the queue (FIFO ordering).
    /// Called by the API server after a transaction passes validation.
    /// 
    /// # Arguments
    /// * `tx` - The validated user transaction to add
    pub async fn add(&self, tx: UserTransaction) {
        // Acquire write lock to add transaction
        let mut txs = self.transactions.write().await;
        txs.push_back(tx);
    }
    
    /// Retrieve pending transactions for batching
    /// 
    /// Removes and returns up to `max` transactions from the front of the queue.
    /// Called by the batch engine when creating a new batch.
    /// 
    /// # Arguments
    /// * `max` - Maximum number of transactions to retrieve
    /// 
    /// # Returns
    /// A vector of up to `max` transactions (may be fewer if pool has less)
    pub async fn get_pending(&self, max: usize) -> Vec<UserTransaction> {
        // Acquire write lock to drain transactions
        let mut txs = self.transactions.write().await;
        let len = txs.len();
        // Drain up to `max` transactions from the front
        txs.drain(..max.min(len)).collect()
    }
}