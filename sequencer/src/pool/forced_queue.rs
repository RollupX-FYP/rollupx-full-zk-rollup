//! Forced Transaction Queue Module
//!
//! This module implements a queue for forced transactions from Layer 1.
//! Forced transactions (deposits and forced exits) must be included in batches
//! to maintain censorship resistance — this is a core guarantee of the rollup.
//!
//! # Priority
//! Forced transactions are ALWAYS processed before normal user transactions
//! in every batch, regardless of the scheduling policy in effect.

use crate::ForcedTransaction;
use std::collections::VecDeque;
use tokio::sync::RwLock;

/// Queue for forced transactions from L1
///
/// Stores forced transactions (deposits and forced exits) that originated from L1.
/// These transactions bypass normal validation and MUST be included in batches.
/// This ensures censorship resistance — users can always force inclusion via L1.
///
/// # Ordering Guarantee
/// Forced transactions are processed in the order they were received from L1,
/// preserving the L1 event ordering.
pub struct ForcedQueue {
    /// Queue of forced transactions, protected by a read-write lock.
    /// Invariant: transactions are ordered by L1 arrival time.
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
    /// Called by the batch orchestrator to retrieve all pending forced transactions.
    /// Forced transactions are ALWAYS included first in batches (before normal txs).
    /// The queue is cleared after retrieval so they are not processed twice.
    ///
    /// # Returns
    /// All forced transactions currently in the queue
    pub async fn get_all(&self) -> Vec<ForcedTransaction> {
        // Acquire write lock to drain all transactions
        let mut txs = self.transactions.write().await;
        // Drain all transactions (clear the queue)
        txs.drain(..).collect()
    }

    /// Get the number of pending forced transactions in the queue
    ///
    /// Used by the batch trigger to detect the presence of forced transactions,
    /// which should trigger an immediate batch seal for censorship resistance.
    ///
    /// # Returns
    /// The number of forced transactions waiting to be processed
    pub async fn pending_count(&self) -> usize {
        let txs = self.transactions.read().await;
        txs.len()
    }

    /// Check if the queue is empty
    ///
    /// Convenience method used by the orchestrator to quickly check
    /// whether any forced transactions are pending.
    ///
    /// # Returns
    /// `true` if no forced transactions are waiting
    pub async fn is_empty(&self) -> bool {
        let txs = self.transactions.read().await;
        txs.is_empty()
    }
}