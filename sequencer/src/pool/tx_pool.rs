//! Transaction Pool Module
//!
//! This module implements a pool for pending user transactions.
//! Transactions are stored in a FIFO queue and retrieved by the batch engine.
//!
//! # Thread Safety
//! All operations are protected by a `RwLock`, allowing concurrent reads
//! (e.g., checking pool size) while ensuring exclusive access during writes
//! (e.g., adding or draining transactions).

use crate::PooledTransaction;
use std::collections::VecDeque;
use tokio::sync::RwLock;

use std::sync::atomic::{AtomicU64, Ordering};

/// Pool for pending user transactions
///
/// Stores validated transactions in a FIFO queue waiting to be batched.
/// Uses `VecDeque` for efficient O(1) insertion at the back and removal
/// from the front. Protected by `RwLock` for concurrent access from the
/// API server (writes) and batch orchestrator (reads/drains).
pub struct TransactionPool {
    /// Queue of pending transactions, protected by a read-write lock.
    /// Invariant: transactions are ordered by insertion time (FIFO).
    transactions: RwLock<VecDeque<PooledTransaction>>,
    /// Total transactions received since startup
    total_received: AtomicU64,
}

impl TransactionPool {
    /// Creates a new empty transaction pool
    pub fn new() -> Self {
        Self {
            transactions: RwLock::new(VecDeque::new()),
            total_received: AtomicU64::new(0),
        }
    }

    /// Add a validated transaction to the pool
    ///
    /// Transactions are added to the back of the queue (FIFO ordering).
    /// Called by the API server after a transaction passes validation.
    ///
    /// # Arguments
    /// * `tx` - The validated user transaction to add
    pub async fn add(&self, tx: PooledTransaction) {
        // Acquire write lock to add transaction
        let mut txs = self.transactions.write().await;
        txs.push_back(tx);
        self.total_received.fetch_add(1, Ordering::Relaxed);
    }

    /// Get total transactions received
    pub fn total_received(&self) -> u64 {
        self.total_received.load(Ordering::Relaxed)
    }

    /// Retrieve pending transactions for batching
    ///
    /// Removes and returns up to `max` transactions from the front of the queue.
    /// Called by the batch orchestrator when creating a new batch.
    ///
    /// # Arguments
    /// * `max` - Maximum number of transactions to retrieve
    ///
    /// # Returns
    /// A vector of up to `max` transactions (may be fewer if pool has less)
    pub async fn get_pending(&self, max: usize) -> Vec<PooledTransaction> {
        // Acquire write lock to drain transactions
        let mut txs = self.transactions.write().await;
        let len = txs.len();
        // Drain up to `max` transactions from the front
        txs.drain(..max.min(len)).collect()
    }

    /// Retrieve a blob-packed subset of pending transactions.
    ///
    /// The selector greedily prefers larger encoded payloads first so the
    /// selected set has a better chance of filling the target byte budget.
    /// Transactions that are not selected are preserved in their original
    /// arrival order.
    pub async fn take_blob_packed(
        &self,
        max_count: usize,
        max_bytes: usize,
    ) -> (Vec<PooledTransaction>, usize) {
        let mut txs = self.transactions.write().await;
        let drained: Vec<_> = txs.drain(..).enumerate().collect();
        let mut candidates: Vec<_> = drained
            .into_iter()
            .map(|(idx, tx)| {
                let size = tx.estimated_encoded_bytes();
                (idx, tx, size)
            })
            .collect();

        candidates.sort_by(|a, b| {
            match b.2.cmp(&a.2) {
                std::cmp::Ordering::Equal => match b.1.tx.gas_price.cmp(&a.1.tx.gas_price) {
                    std::cmp::Ordering::Equal => a.0.cmp(&b.0),
                    other => other,
                },
                other => other,
            }
        });

        let mut selected = Vec::new();
        let mut remainder = Vec::new();
        let mut used_bytes = 0usize;

        for (idx, tx, size) in candidates {
            let fits_count = selected.len() < max_count;
            let fits_bytes = used_bytes.saturating_add(size) <= max_bytes;
            if fits_count && fits_bytes {
                used_bytes = used_bytes.saturating_add(size);
                selected.push((idx, tx));
            } else {
                remainder.push((idx, tx));
            }
        }

        selected.sort_by(|a, b| a.0.cmp(&b.0));
        remainder.sort_by(|a, b| a.0.cmp(&b.0));
        for (_, tx) in remainder {
            txs.push_back(tx);
        }

        (selected.into_iter().map(|(_, tx)| tx).collect(), used_bytes)
    }


    /// Get the number of pending transactions in the pool
    ///
    /// This is a non-blocking read used by the batch trigger to decide
    /// whether a size-based batch seal should occur.
    ///
    /// # Returns
    /// The number of transactions currently waiting in the pool
    pub async fn pending_count(&self) -> usize {
        let txs = self.transactions.read().await;
        txs.len()
    }

    /// Check if the pool is empty
    ///
    /// Convenience method used by the orchestrator to skip batch production
    /// when there is nothing to process.
    ///
    /// # Returns
    /// `true` if the pool contains no pending transactions
    pub async fn is_empty(&self) -> bool {
        let txs = self.transactions.read().await;
        txs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{UserTransaction, PooledTransaction};
    use ethers::types::{Address, Signature, U256};

    fn make_tx(gas_price: u64, value: u64, timestamp: u64, boost_bid: Option<U256>) -> PooledTransaction {
        let tx = UserTransaction {
            from: Address::random(),
            to: Address::random(),
            value: U256::from(value),
            nonce: 1,
            gas_limit: 21_000,
            gas_price: U256::from(gas_price),
            signature: Signature { r: U256::zero(), s: U256::zero(), v: 27 },
            timestamp,
            boost_bid,
        };
        PooledTransaction {
            tx,
            arrived_at: timestamp,
            pool_entry_at: timestamp + 1,
            validation_latency_ms: 1,
        }
    }

    #[tokio::test]
    async fn test_pool_total_received() {
        let pool = TransactionPool::new();
        assert_eq!(pool.total_received(), 0);

        let tx = UserTransaction {
            from: Address::random(),
            to: Address::random(),
            value: U256::from(100),
            nonce: 1,
            gas_limit: 21000,
            gas_price: U256::from(10),
            signature: Signature { r: U256::zero(), s: U256::zero(), v: 0 },
            timestamp: 1000,
            boost_bid: None,
        };
        let ptx = PooledTransaction {
            tx,
            arrived_at: 1000,
            pool_entry_at: 1001,
            validation_latency_ms: 1,
        };

        pool.add(ptx).await;
        assert_eq!(pool.total_received(), 1);
        assert_eq!(pool.pending_count().await, 1);
        
        // Draining shouldn't reset total_received
        pool.get_pending(1).await;
        assert_eq!(pool.total_received(), 1);
        assert_eq!(pool.pending_count().await, 0);
    }

    #[tokio::test]
    async fn take_blob_packed_prefers_larger_transactions_and_preserves_remainder() {
        let pool = TransactionPool::new();
        let small = make_tx(1, 1, 1, None);
        let medium = make_tx(2, 10, 2, None);
        let large = make_tx(
            3,
            10,
            3,
            Some(U256::from_dec_str(
                "1000000000000000000000000000000000000000000000000000000000000000",
            )
            .unwrap()),
        );

        assert!(large.estimated_encoded_bytes() > medium.estimated_encoded_bytes());

        pool.add(small.clone()).await;
        pool.add(medium.clone()).await;
        pool.add(large.clone()).await;

        let (selected, used_bytes) = pool.take_blob_packed(2, usize::MAX).await;

        assert_eq!(selected.len(), 2);
        assert!(used_bytes > 0);
        assert!(selected.iter().any(|tx| tx.tx.gas_price == large.tx.gas_price));
        assert!(selected.iter().any(|tx| tx.tx.gas_price == medium.tx.gas_price));
        assert!(selected.iter().all(|tx| tx.tx.gas_price != small.tx.gas_price));

        let remaining = pool.get_pending(10).await;
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].tx.gas_price, small.tx.gas_price);
    }
}
