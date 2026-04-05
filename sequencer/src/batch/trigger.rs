//! Batch Trigger Module
//!
//! This module implements the batch trigger logic that determines when the
//! orchestrator should seal and produce a new batch. It supports three
//! independent trigger conditions:
//!
//! 1. **Time-based trigger**: Fires after the configured timeout interval has
//!    elapsed since the last batch, ensuring batches are produced even during
//!    low-traffic periods.
//!
//! 2. **Size-based trigger**: Fires when the combined pending transaction count
//!    (normal + forced) meets or exceeds the configured `max_batch_size`,
//!    ensuring full batches are produced promptly during high traffic.
//!
//! 3. **Forced-transaction trigger**: Fires immediately when any forced
//!    transactions (deposits or forced exits from L1) are pending, ensuring
//!    censorship resistance by processing L1 operations without delay.
//!
//! # Trigger Priority
//! The triggers are evaluated in this order:
//! ```text
//! Forced Tx Present? → YES → SEAL IMMEDIATELY
//!                    → NO  → Pool Size ≥ Max? → YES → SEAL
//!                                             → NO  → Timeout Expired? → YES → SEAL
//!                                                                       → NO  → WAIT
//! ```

use crate::config::BatchConfig;
use crate::pool::{ForcedQueue, TransactionPool};
use std::sync::Arc;
use tokio::time::Instant;

/// Reason why a batch seal was triggered
///
/// Used for logging and metadata to track why each batch was created.
/// This information is stored in the batch registry for auditing.
#[derive(Debug, Clone, PartialEq)]
pub enum TriggerReason {
    /// Batch sealed because forced transactions from L1 are pending.
    /// These must be processed immediately for censorship resistance.
    ForcedTransactions,

    /// Batch sealed because the pool size met or exceeded `max_batch_size`.
    /// This is the normal high-throughput trigger.
    SizeThreshold,

    /// Batch sealed because the timeout interval expired.
    /// Ensures batches are produced even during low activity.
    Timeout,
}

impl std::fmt::Display for TriggerReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TriggerReason::ForcedTransactions => write!(f, "ForcedTransactions"),
            TriggerReason::SizeThreshold => write!(f, "SizeThreshold"),
            TriggerReason::Timeout => write!(f, "Timeout"),
        }
    }
}

/// Batch trigger that determines when to seal a new batch
///
/// Evaluates the three trigger conditions (forced, size, timeout) against
/// the current state of the transaction pools and timing.
///
/// # Usage
/// ```ignore
/// let trigger = BatchTrigger::new(config, tx_pool, forced_queue);
/// // In the main loop:
/// if let Some(reason) = trigger.should_seal(last_batch_time).await {
///     // Produce a batch
///     trigger.reset(&mut last_batch_time);
/// }
/// ```
pub struct BatchTrigger {
    /// Batch configuration containing size limits and timeout settings
    config: BatchConfig,
    /// Reference to the normal transaction pool for size checking
    tx_pool: Arc<TransactionPool>,
    /// Reference to the forced transaction queue for immediate trigger
    forced_queue: Arc<ForcedQueue>,
}

impl BatchTrigger {
    /// Creates a new batch trigger
    ///
    /// # Arguments
    /// * `config` - Batch configuration (max_batch_size, timeout_interval_ms, etc.)
    /// * `tx_pool` - Shared reference to the normal transaction pool
    /// * `forced_queue` - Shared reference to the forced transaction queue
    pub fn new(
        config: BatchConfig,
        tx_pool: Arc<TransactionPool>,
        forced_queue: Arc<ForcedQueue>,
    ) -> Self {
        Self {
            config,
            tx_pool,
            forced_queue,
        }
    }

    /// Evaluate whether a batch should be sealed right now
    ///
    /// Checks all three trigger conditions in priority order:
    /// 1. Forced transactions present → seal immediately
    /// 2. Pool size ≥ max_batch_size → seal for throughput
    /// 3. Timeout expired and we have at least `min_batch_size` txs → seal
    ///
    /// # Arguments
    /// * `last_batch_time` - When the last batch was produced (for timeout calculation)
    ///
    /// # Returns
    /// * `Some(TriggerReason)` if a batch should be sealed, with the reason
    /// * `None` if no trigger condition is met
    pub async fn should_seal(&self, last_batch_time: Instant) -> Option<TriggerReason> {
        // Priority 1: Forced transactions from L1 → immediate seal
        // Censorship resistance requires these to be processed without delay
        let forced_count = self.forced_queue.pending_count().await;
        if forced_count > 0 {
            return Some(TriggerReason::ForcedTransactions);
        }

        // Priority 2: Pool size threshold → seal when full
        let normal_count = self.tx_pool.pending_count().await;
        if normal_count >= self.config.max_batch_size {
            return Some(TriggerReason::SizeThreshold);
        }

        // Priority 3: Timeout expired → seal partial batch
        // Only if we have at least `min_batch_size` transactions to avoid
        // producing near-empty batches during very low traffic
        let timeout_ms = self.config.timeout_interval_ms;
        let elapsed = last_batch_time.elapsed();
        if elapsed >= tokio::time::Duration::from_millis(timeout_ms) {
            // Even on timeout, require minimum transactions to avoid empty batches
            // (unless forced txs are present, which is already handled above)
            if normal_count >= self.config.min_batch_size {
                return Some(TriggerReason::Timeout);
            }
            // If below min_batch_size but timeout expired, only seal if there
            // are ANY transactions at all — don't let them wait forever
            if normal_count > 0 {
                return Some(TriggerReason::Timeout);
            }
        }

        // No trigger condition met — wait
        None
    }

    /// Reset the batch timer after producing a batch
    ///
    /// # Arguments
    /// * `last_batch_time` - Mutable reference to the timer to reset
    pub fn reset(&self, last_batch_time: &mut Instant) {
        *last_batch_time = Instant::now();
    }
}