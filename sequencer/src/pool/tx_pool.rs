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
use ethers::types::Address;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};
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

#[derive(Debug, Clone, Default)]
pub struct BlobPackSelection {
    pub selected: Vec<PooledTransaction>,
    pub selected_bytes: usize,
    pub eligible_tx_count: usize,
    pub eligible_bytes: usize,
    pub ineligible_nonce_gap_count: usize,
    pub nonce_chain_truncated_senders: usize,
    pub low_fill_reason: Option<String>,
    pub age_guard_triggered_count: usize,
    pub best_fit_selected_count: usize,
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

    /// Retrieve a nonce-safe blob-packed subset of pending transactions.
    ///
    /// Selection happens in two phases:
    /// 1) per-sender contiguous nonce eligibility from expected nonce
    /// 2) fill-first greedy packing over eligible transactions
    pub async fn take_blob_packed_nonce_safe(
        &self,
        max_count: usize,
        max_bytes: usize,
        expected_nonces: HashMap<Address, u64>,
    ) -> BlobPackSelection {
        let mut txs = self.transactions.write().await;
        let drained: Vec<_> = txs.drain(..).enumerate().collect();
        let mut per_sender: HashMap<Address, Vec<(usize, PooledTransaction, usize)>> =
            HashMap::new();
        for (idx, tx) in drained {
            let size = tx.estimated_encoded_bytes();
            per_sender
                .entry(tx.tx.from)
                .or_default()
                .push((idx, tx, size));
        }

        let mut eligible: Vec<(usize, PooledTransaction, usize)> = Vec::new();
        let mut ineligible: Vec<(usize, PooledTransaction, usize)> = Vec::new();
        let mut ineligible_nonce_gap_count = 0usize;
        let mut nonce_chain_truncated_senders = 0usize;

        for (sender, mut sender_txs) in per_sender {
            sender_txs.sort_by(|a, b| match a.1.tx.nonce.cmp(&b.1.tx.nonce) {
                std::cmp::Ordering::Equal => a.0.cmp(&b.0),
                other => other,
            });
            // The sequencer state cache is pessimistically advanced when a
            // transaction is admitted to the pool. For scheduling, the first
            // executable pending nonce is therefore the lower of the cache
            // nonce and the earliest pending nonce for that sender.
            let earliest_pending_nonce = sender_txs
                .first()
                .map(|(_, tx, _)| tx.tx.nonce)
                .unwrap_or_else(|| expected_nonces.get(&sender).copied().unwrap_or(0));
            let mut expected = expected_nonces
                .get(&sender)
                .copied()
                .unwrap_or(earliest_pending_nonce)
                .min(earliest_pending_nonce);
            let mut saw_gap = false;

            for entry in sender_txs {
                if saw_gap {
                    ineligible.push(entry);
                    continue;
                }
                match entry.1.tx.nonce.cmp(&expected) {
                    std::cmp::Ordering::Equal => {
                        expected = expected.saturating_add(1);
                        eligible.push(entry);
                    }
                    _ => {
                        saw_gap = true;
                        nonce_chain_truncated_senders =
                            nonce_chain_truncated_senders.saturating_add(1);
                        ineligible_nonce_gap_count = ineligible_nonce_gap_count.saturating_add(1);
                        ineligible.push(entry);
                    }
                }
            }
        }

        let eligible_tx_count = eligible.len();
        let eligible_bytes = eligible.iter().map(|(_, _, s)| *s).sum::<usize>();

        eligible.sort_by(|a, b| match b.2.cmp(&a.2) {
            std::cmp::Ordering::Equal => match b.1.tx.gas_price.cmp(&a.1.tx.gas_price) {
                std::cmp::Ordering::Equal => match a.1.arrived_at.cmp(&b.1.arrived_at) {
                    std::cmp::Ordering::Equal => a.0.cmp(&b.0),
                    other => other,
                },
                other => other,
            },
            other => other,
        });

        let mut selected = Vec::new();
        let mut selected_bytes = 0usize;
        let mut remainder = ineligible;
        let mut count_blocked = false;
        for entry in eligible {
            if selected.len() >= max_count {
                count_blocked = true;
                remainder.push(entry);
                continue;
            }
            if selected_bytes.saturating_add(entry.2) <= max_bytes {
                selected_bytes = selected_bytes.saturating_add(entry.2);
                selected.push(entry);
            } else {
                remainder.push(entry);
            }
        }

        selected.sort_by(|a, b| a.0.cmp(&b.0));
        remainder.sort_by(|a, b| a.0.cmp(&b.0));
        for (_, tx, _) in remainder {
            txs.push_back(tx);
        }

        let low_fill_reason = if ineligible_nonce_gap_count > 0 {
            Some("nonce_gaps".to_string())
        } else if count_blocked {
            Some("count_cap".to_string())
        } else if selected_bytes < max_bytes && eligible_bytes < max_bytes {
            Some("insufficient_eligible_bytes".to_string())
        } else {
            None
        };

        BlobPackSelection {
            selected: selected.into_iter().map(|(_, tx, _)| tx).collect(),
            selected_bytes,
            eligible_tx_count,
            eligible_bytes,
            ineligible_nonce_gap_count,
            nonce_chain_truncated_senders,
            low_fill_reason,
            age_guard_triggered_count: 0,
            best_fit_selected_count: 0,
        }
    }

    /// Retrieve a nonce-safe subset using best-fit blob packing with an age guard.
    ///
    /// At each step, only the next nonce-safe transaction from each sender is
    /// selectable. Old candidates can override best-fit selection to avoid
    /// starvation, while still respecting byte and count limits.
    pub async fn take_blob_packed_best_fit_age_guard(
        &self,
        max_count: usize,
        max_bytes: usize,
        expected_nonces: HashMap<Address, u64>,
        age_guard_ms: u64,
    ) -> BlobPackSelection {
        let mut txs = self.transactions.write().await;
        let drained: Vec<_> = txs.drain(..).enumerate().collect();
        let mut per_sender: HashMap<Address, Vec<(usize, PooledTransaction, usize)>> =
            HashMap::new();
        for (idx, tx) in drained {
            let size = tx.estimated_encoded_bytes();
            per_sender
                .entry(tx.tx.from)
                .or_default()
                .push((idx, tx, size));
        }

        let mut chains: HashMap<Address, Vec<(usize, PooledTransaction, usize)>> = HashMap::new();
        let mut ineligible: Vec<(usize, PooledTransaction, usize)> = Vec::new();
        let mut ineligible_nonce_gap_count = 0usize;
        let mut nonce_chain_truncated_senders = 0usize;

        for (sender, mut sender_txs) in per_sender {
            sender_txs.sort_by(|a, b| match a.1.tx.nonce.cmp(&b.1.tx.nonce) {
                std::cmp::Ordering::Equal => a.0.cmp(&b.0),
                other => other,
            });
            let earliest_pending_nonce = sender_txs
                .first()
                .map(|(_, tx, _)| tx.tx.nonce)
                .unwrap_or_else(|| expected_nonces.get(&sender).copied().unwrap_or(0));
            let mut expected = expected_nonces
                .get(&sender)
                .copied()
                .unwrap_or(earliest_pending_nonce)
                .min(earliest_pending_nonce);
            let mut saw_gap = false;
            let mut chain = Vec::new();

            for entry in sender_txs {
                if saw_gap {
                    ineligible.push(entry);
                    continue;
                }
                match entry.1.tx.nonce.cmp(&expected) {
                    std::cmp::Ordering::Equal => {
                        expected = expected.saturating_add(1);
                        chain.push(entry);
                    }
                    _ => {
                        saw_gap = true;
                        nonce_chain_truncated_senders =
                            nonce_chain_truncated_senders.saturating_add(1);
                        ineligible_nonce_gap_count = ineligible_nonce_gap_count.saturating_add(1);
                        ineligible.push(entry);
                    }
                }
            }
            if !chain.is_empty() {
                chains.insert(sender, chain);
            }
        }

        let eligible_tx_count = chains.values().map(|chain| chain.len()).sum::<usize>();
        let eligible_bytes = chains
            .values()
            .flat_map(|chain| chain.iter())
            .map(|(_, _, size)| *size)
            .sum::<usize>();
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or(0);

        let mut selected: Vec<(usize, PooledTransaction, usize)> = Vec::new();
        let mut selected_bytes = 0usize;
        let mut chain_pos: HashMap<Address, usize> = HashMap::new();
        let mut age_guard_triggered_count = 0usize;
        let mut best_fit_selected_count = 0usize;
        let mut count_blocked = false;

        loop {
            if selected.len() >= max_count {
                count_blocked = true;
                break;
            }
            let remaining_bytes = max_bytes.saturating_sub(selected_bytes);
            let mut candidates: Vec<(Address, usize, u64, usize)> = Vec::new();
            for (sender, chain) in chains.iter() {
                let pos = *chain_pos.get(sender).unwrap_or(&0);
                if let Some((_, tx, size)) = chain.get(pos) {
                    if *size <= remaining_bytes {
                        let age_ms = now_ms.saturating_sub(tx.arrived_at);
                        candidates.push((*sender, *size, age_ms, pos));
                    }
                }
            }
            if candidates.is_empty() {
                break;
            }

            if let Some((sender, _, _, _)) = candidates
                .iter()
                .filter(|(_, _, age_ms, _)| *age_ms >= age_guard_ms)
                .max_by(|a, b| match a.2.cmp(&b.2) {
                    std::cmp::Ordering::Equal => b.1.cmp(&a.1),
                    other => other,
                })
            {
                age_guard_triggered_count = age_guard_triggered_count.saturating_add(1);
                let pos = *chain_pos.get(sender).unwrap_or(&0);
                if let Some(entry) = chains.get(sender).and_then(|chain| chain.get(pos)).cloned() {
                    selected_bytes = selected_bytes.saturating_add(entry.2);
                    selected.push(entry);
                    chain_pos.insert(*sender, pos.saturating_add(1));
                } else {
                    break;
                }
            } else {
                // Bounded subset fill over currently nonce-safe heads. This avoids
                // the classic greedy trap where one large tx leaves a gap that two
                // smaller txs could have filled exactly.
                let count_limit = max_count.saturating_sub(selected.len());
                let mut dp: Vec<Option<(usize, Vec<usize>)>> = vec![None; remaining_bytes + 1];
                dp[0] = Some((0, Vec::new()));

                for (candidate_idx, (_, size, _, _)) in candidates.iter().enumerate() {
                    for bytes in (*size..=remaining_bytes).rev() {
                        let Some((prev_count, mut prev_indices)) = dp[bytes - *size].clone() else {
                            continue;
                        };
                        if prev_count >= count_limit {
                            continue;
                        }
                        prev_indices.push(candidate_idx);
                        let next = (prev_count + 1, prev_indices);
                        let should_replace = match &dp[bytes] {
                            Some((existing_count, _)) => next.0 > *existing_count,
                            None => true,
                        };
                        if should_replace {
                            dp[bytes] = Some(next);
                        }
                    }
                }

                let best = dp
                    .iter()
                    .enumerate()
                    .filter_map(|(bytes, entry)| {
                        entry
                            .as_ref()
                            .map(|(count, indices)| (bytes, *count, indices.clone()))
                    })
                    .max_by(|a, b| match a.0.cmp(&b.0) {
                        std::cmp::Ordering::Equal => a.1.cmp(&b.1),
                        other => other,
                    });

                let Some((_, _, mut chosen_indices)) = best else {
                    break;
                };
                if chosen_indices.is_empty() {
                    break;
                }

                chosen_indices.sort_by_key(|idx| {
                    let sender = candidates[*idx].0;
                    let pos = *chain_pos.get(&sender).unwrap_or(&0);
                    chains
                        .get(&sender)
                        .and_then(|chain| chain.get(pos))
                        .map(|entry| entry.0)
                        .unwrap_or(usize::MAX)
                });

                for candidate_idx in chosen_indices {
                    let sender = candidates[candidate_idx].0;
                    let pos = *chain_pos.get(&sender).unwrap_or(&0);
                    if let Some(entry) = chains
                        .get(&sender)
                        .and_then(|chain| chain.get(pos))
                        .cloned()
                    {
                        selected_bytes = selected_bytes.saturating_add(entry.2);
                        selected.push(entry);
                        chain_pos.insert(sender, pos.saturating_add(1));
                        best_fit_selected_count = best_fit_selected_count.saturating_add(1);
                    }
                }
            }
        }

        let mut remainder = ineligible;
        for (sender, chain) in chains {
            let pos = *chain_pos.get(&sender).unwrap_or(&0);
            for entry in chain.into_iter().skip(pos) {
                remainder.push(entry);
            }
        }

        selected.sort_by(|a, b| a.0.cmp(&b.0));
        remainder.sort_by(|a, b| a.0.cmp(&b.0));
        for (_, tx, _) in remainder {
            txs.push_back(tx);
        }

        let low_fill_reason = if ineligible_nonce_gap_count > 0 {
            Some("nonce_gaps".to_string())
        } else if count_blocked {
            Some("count_cap".to_string())
        } else if selected_bytes < max_bytes && eligible_bytes < max_bytes {
            Some("insufficient_eligible_bytes".to_string())
        } else if selected_bytes < max_bytes {
            Some("insufficient_fit".to_string())
        } else {
            None
        };

        BlobPackSelection {
            selected: selected.into_iter().map(|(_, tx, _)| tx).collect(),
            selected_bytes,
            eligible_tx_count,
            eligible_bytes,
            ineligible_nonce_gap_count,
            nonce_chain_truncated_senders,
            low_fill_reason,
            age_guard_triggered_count,
            best_fit_selected_count,
        }
    }

    pub async fn pending_senders(&self) -> Vec<Address> {
        let txs = self.transactions.read().await;
        let mut seen: HashMap<Address, ()> = HashMap::new();
        for tx in txs.iter() {
            seen.entry(tx.tx.from).or_insert(());
        }
        seen.into_keys().collect()
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
    use crate::{PooledTransaction, UserTransaction};
    use ethers::types::{Address, Signature, U256};

    fn make_tx(
        gas_price: u64,
        value: u64,
        timestamp: u64,
        boost_bid: Option<U256>,
    ) -> PooledTransaction {
        let tx = UserTransaction {
            from: Address::random(),
            to: Address::random(),
            value: U256::from(value),
            nonce: 1,
            gas_limit: 21_000,
            gas_price: U256::from(gas_price),
            signature: Signature {
                r: U256::zero(),
                s: U256::zero(),
                v: 27,
            },
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

    fn make_sender_tx(
        sender: Address,
        nonce: u64,
        value: u64,
        arrived_at: u64,
    ) -> PooledTransaction {
        let tx = UserTransaction {
            from: sender,
            to: Address::random(),
            value: U256::from(value),
            nonce,
            gas_limit: 21_000,
            gas_price: U256::from(1),
            signature: Signature {
                r: U256::zero(),
                s: U256::zero(),
                v: 27,
            },
            timestamp: arrived_at,
            boost_bid: None,
        };
        PooledTransaction {
            tx,
            arrived_at,
            pool_entry_at: arrived_at,
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
            signature: Signature {
                r: U256::zero(),
                s: U256::zero(),
                v: 0,
            },
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
    async fn take_blob_packed_nonce_safe_prefers_larger_transactions_and_preserves_remainder() {
        let pool = TransactionPool::new();
        let mut small = make_tx(1, 1, 1, None);
        let mut medium = make_tx(2, 10, 2, None);
        let mut large = make_tx(
            3,
            10,
            3,
            Some(
                U256::from_dec_str(
                    "1000000000000000000000000000000000000000000000000000000000000000",
                )
                .unwrap(),
            ),
        );
        small.tx.nonce = 0;
        medium.tx.nonce = 0;
        large.tx.nonce = 0;

        assert!(large.estimated_encoded_bytes() > medium.estimated_encoded_bytes());

        pool.add(small.clone()).await;
        pool.add(medium.clone()).await;
        pool.add(large.clone()).await;

        let mut expected = HashMap::new();
        expected.insert(small.tx.from, 0);
        expected.insert(medium.tx.from, 0);
        expected.insert(large.tx.from, 0);
        let selection = pool
            .take_blob_packed_nonce_safe(2, usize::MAX, expected)
            .await;
        let selected = selection.selected;

        assert_eq!(selected.len(), 2);
        assert!(selection.selected_bytes > 0);
        assert!(
            selected
                .iter()
                .any(|tx| tx.tx.gas_price == large.tx.gas_price)
        );
        assert!(
            selected
                .iter()
                .any(|tx| tx.tx.gas_price == medium.tx.gas_price)
        );
        assert!(
            selected
                .iter()
                .all(|tx| tx.tx.gas_price != small.tx.gas_price)
        );

        let remaining = pool.get_pending(10).await;
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].tx.gas_price, small.tx.gas_price);
    }

    #[tokio::test]
    async fn take_blob_packed_nonce_safe_handles_nonce_gaps() {
        let pool = TransactionPool::new();
        let sender = Address::random();
        let mut tx0 = make_tx(1, 1, 1, None);
        tx0.tx.from = sender;
        tx0.tx.nonce = 0;
        let mut tx2 = make_tx(2, 1, 2, None);
        tx2.tx.from = sender;
        tx2.tx.nonce = 2;
        pool.add(tx0.clone()).await;
        pool.add(tx2.clone()).await;

        let mut expected = HashMap::new();
        expected.insert(sender, 0);
        let selection = pool
            .take_blob_packed_nonce_safe(10, usize::MAX, expected)
            .await;
        assert_eq!(selection.selected.len(), 1);
        assert_eq!(selection.selected[0].tx.nonce, 0);
        assert_eq!(selection.ineligible_nonce_gap_count, 1);
        assert_eq!(selection.low_fill_reason.as_deref(), Some("nonce_gaps"));

        let remaining = pool.get_pending(10).await;
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].tx.nonce, 2);
    }

    #[tokio::test]
    async fn take_blob_packed_nonce_safe_allows_contiguous_chain() {
        let pool = TransactionPool::new();
        let sender = Address::random();
        for i in 0..3u64 {
            let mut tx = make_tx(10 + i, 1, i + 1, None);
            tx.tx.from = sender;
            tx.tx.nonce = i;
            pool.add(tx).await;
        }

        let mut expected = HashMap::new();
        expected.insert(sender, 0);
        let selection = pool
            .take_blob_packed_nonce_safe(10, usize::MAX, expected)
            .await;
        assert_eq!(selection.selected.len(), 3);
        assert_eq!(selection.selected[0].tx.nonce, 0);
        assert_eq!(selection.selected[1].tx.nonce, 1);
        assert_eq!(selection.selected[2].tx.nonce, 2);
    }

    #[tokio::test]
    async fn take_blob_packed_nonce_safe_handles_multiple_senders() {
        let pool = TransactionPool::new();
        let sender_a = Address::random();
        let sender_b = Address::random();

        let mut a0 = make_tx(10, 1, 1, None);
        a0.tx.from = sender_a;
        a0.tx.nonce = 0;
        let mut a1 = make_tx(11, 1, 2, None);
        a1.tx.from = sender_a;
        a1.tx.nonce = 1;
        let mut b0 = make_tx(20, 1, 3, None);
        b0.tx.from = sender_b;
        b0.tx.nonce = 0;

        pool.add(a0).await;
        pool.add(a1).await;
        pool.add(b0).await;

        let mut expected = HashMap::new();
        expected.insert(sender_a, 0);
        expected.insert(sender_b, 0);
        let selection = pool
            .take_blob_packed_nonce_safe(10, usize::MAX, expected)
            .await;
        assert_eq!(selection.selected.len(), 3);
        assert_eq!(selection.ineligible_nonce_gap_count, 0);
    }

    #[tokio::test]
    async fn take_blob_packed_nonce_safe_skips_oversized_but_keeps_others() {
        let pool = TransactionPool::new();
        let sender_a = Address::random();
        let sender_b = Address::random();

        let mut large = make_tx(
            50,
            10,
            1,
            Some(
                U256::from_dec_str(
                    "1000000000000000000000000000000000000000000000000000000000000000",
                )
                .unwrap(),
            ),
        );
        large.tx.from = sender_a;
        large.tx.nonce = 0;

        let mut small = make_tx(5, 1, 2, None);
        small.tx.from = sender_b;
        small.tx.nonce = 0;

        let large_bytes = large.estimated_encoded_bytes();
        let small_bytes = small.estimated_encoded_bytes();

        pool.add(large.clone()).await;
        pool.add(small.clone()).await;

        let mut expected = HashMap::new();
        expected.insert(sender_a, 0);
        expected.insert(sender_b, 0);
        let selection = pool
            .take_blob_packed_nonce_safe(10, small_bytes, expected)
            .await;

        assert_eq!(selection.selected.len(), 1);
        assert_eq!(selection.selected[0].tx.from, sender_b);
        assert!(selection.selected_bytes <= small_bytes);
        assert!(large_bytes > selection.selected_bytes);

        let remaining = pool.get_pending(10).await;
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].tx.from, sender_a);
    }

    #[tokio::test]
    async fn take_blob_packed_best_fit_uses_age_guard_for_old_candidate() {
        let pool = TransactionPool::new();
        let old_sender = Address::random();
        let new_sender = Address::random();
        let old_tx = make_sender_tx(old_sender, 0, 1, 1);
        let new_tx = make_sender_tx(new_sender, 0, 1000000000000000000, u64::MAX);

        pool.add(new_tx).await;
        pool.add(old_tx).await;

        let mut expected = HashMap::new();
        expected.insert(old_sender, 0);
        expected.insert(new_sender, 0);

        let selection = pool
            .take_blob_packed_best_fit_age_guard(1, usize::MAX, expected, 1)
            .await;

        assert_eq!(selection.selected.len(), 1);
        assert_eq!(selection.selected[0].tx.from, old_sender);
        assert_eq!(selection.age_guard_triggered_count, 1);
    }

    #[tokio::test]
    async fn take_blob_packed_best_fit_fills_gap_better_than_largest_first() {
        let pool = TransactionPool::new();
        let large_sender = Address::random();
        let medium_sender = Address::random();
        let small_sender = Address::random();

        let mut large = make_tx(
            30,
            1,
            u64::MAX - 10,
            Some(
                U256::from_dec_str(
                    "1000000000000000000000000000000000000000000000000000000000000000",
                )
                .unwrap(),
            ),
        );
        let mut medium = make_tx(
            20,
            1,
            u64::MAX - 10,
            Some(U256::from_dec_str("100000000000000000000000000000").unwrap()),
        );
        let mut small = make_tx(10, 1, u64::MAX - 10, None);
        large.tx.from = large_sender;
        medium.tx.from = medium_sender;
        small.tx.from = small_sender;
        large.tx.nonce = 0;
        medium.tx.nonce = 0;
        small.tx.nonce = 0;

        let large_bytes = large.estimated_encoded_bytes();
        let medium_bytes = medium.estimated_encoded_bytes();
        let small_bytes = small.estimated_encoded_bytes();
        let max_bytes = medium_bytes.saturating_add(small_bytes);
        assert!(large_bytes <= max_bytes);
        assert!(max_bytes.saturating_sub(large_bytes) < small_bytes);

        pool.add(large.clone()).await;
        pool.add(medium.clone()).await;
        pool.add(small.clone()).await;

        let mut expected = HashMap::new();
        expected.insert(large_sender, 0);
        expected.insert(medium_sender, 0);
        expected.insert(small_sender, 0);

        let selection = pool
            .take_blob_packed_best_fit_age_guard(3, max_bytes, expected, u64::MAX)
            .await;

        assert_eq!(selection.selected.len(), 2);
        assert_eq!(selection.selected_bytes, max_bytes);
        assert!(
            selection
                .selected
                .iter()
                .any(|tx| tx.tx.from == medium_sender)
        );
        assert!(
            selection
                .selected
                .iter()
                .any(|tx| tx.tx.from == small_sender)
        );
        assert!(
            !selection
                .selected
                .iter()
                .any(|tx| tx.tx.from == large_sender)
        );
        assert_eq!(selection.best_fit_selected_count, 2);
    }
}
