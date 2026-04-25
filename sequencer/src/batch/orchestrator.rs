//! Batch Orchestrator Module
//!
//! This module implements the orchestration layer that connects all batch-related
//! components together. It runs a background loop that periodically produces batches
//! by pulling transactions from pools, scheduling them, and creating sealed batches.
//!
//! # Architecture Flow
//! ```text
//! ┌─────────────┐    ┌──────────────┐    ┌───────────┐    ┌─────────────┐
//! │ BatchTrigger │───→│ Pull from    │───→│ Scheduler │───→│ BatchEngine │
//! │ (evaluates)  │    │ ForcedQueue  │    │ (orders)  │    │ (seals)     │
//! └─────────────┘    │ + TxPool     │    └───────────┘    └──────┬──────┘
//!                    └──────────────┘                            │
//!                                                               ▼
//!                                                     ┌─────────────────┐
//!                                                     │ Registry        │
//!                                                     │ (stores meta)   │
//!                                                     └─────────────────┘
//! ```
//!
//! # Trigger Conditions
//! 1. **Forced transactions**: Seal immediately when L1 transactions are pending
//! 2. **Size threshold**: Seal when pool size ≥ `max_batch_size`
//! 3. **Timeout**: Seal after `timeout_interval_ms` if transactions are available

use crate::{
    pool::{ForcedQueue, TransactionPool},
    scheduler::{Scheduler, SchedulingPolicyType, create_policy},
    batch::{BatchEngine, trigger::BatchTrigger},
    registry::Registry,
    config::BatchConfig,
    Batch, BatchMetadata, Transaction,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration, Instant};
use tracing::{info, debug, warn, error};

/// Batch orchestrator
///
/// Coordinates the entire batch production pipeline by:
/// 1. Evaluating trigger conditions (via `BatchTrigger`)
/// 2. Pulling transactions from pools (`ForcedQueue` + `TransactionPool`)
/// 3. Ordering them using the configured scheduling policy (via `Scheduler`)
/// 4. Creating sealed batches (via `BatchEngine`)
/// 5. Persisting batch metadata (via `Registry`)
///
/// # Lifecycle
/// The orchestrator runs an infinite async loop, sleeping briefly between
/// iterations to avoid busy-waiting. It is designed to be spawned as a
/// background tokio task and will run until the process shuts down.
pub struct BatchOrchestrator {
    /// Forced transaction queue (L1-originated transactions)
    forced_queue: Arc<ForcedQueue>,
    /// Normal transaction pool (user-submitted transactions)
    tx_pool: Arc<TransactionPool>,
    /// Scheduler for ordering transactions within batches
    scheduler: Scheduler,
    /// Batch engine for creating sealed batches (wrapped in RwLock for mutable access)
    batch_engine: RwLock<BatchEngine>,
    /// Batch trigger for determining when to seal batches
    trigger: BatchTrigger,
    /// Batch metadata registry for persistent storage
    registry: Arc<Registry>,
    /// Batch configuration (size limits, timeout, etc.)
    config: BatchConfig,
}

impl BatchOrchestrator {
    /// Creates a new batch orchestrator
    ///
    /// # Arguments
    /// * `forced_queue` - Shared reference to the forced transaction queue
    /// * `tx_pool` - Shared reference to the normal transaction pool
    /// * `batch_config` - Batch configuration settings
    /// * `scheduling_policy` - Scheduling policy type (FCFS, FeePriority, TimeBoost, or FairBFT)
    /// * `registry` - Shared reference to the batch metadata registry
    pub fn new(
        forced_queue: Arc<ForcedQueue>,
        tx_pool: Arc<TransactionPool>,
        batch_config: BatchConfig,
        scheduling_policy: SchedulingPolicyType,
        registry: Arc<Registry>,
    ) -> Self {
        // Create policy instance using factory function
        let policy = create_policy(scheduling_policy);

        // Create the batch trigger with access to both pools
        let trigger = BatchTrigger::new(
            batch_config.clone(),
            tx_pool.clone(),
            forced_queue.clone(),
        );

        Self {
            forced_queue,
            tx_pool,
            scheduler: Scheduler::new(policy),
            batch_engine: RwLock::new(BatchEngine::new(batch_config.clone())),
            trigger,
            registry,
            config: batch_config,
        }
    }

    /// Start the batch orchestrator background loop
    ///
    /// Runs continuously, checking trigger conditions and producing batches
    /// when appropriate. The loop follows this pattern:
    ///
    /// ```text
    /// loop {
    ///     sleep(100ms)                    // Avoid busy-waiting
    ///     trigger.should_seal()?          // Check all trigger conditions
    ///     → produce_batch()              // Pull, schedule, seal
    ///     → store metadata in registry   // Persist batch info
    ///     → reset timer                  // Prepare for next batch
    /// }
    /// ```
    ///
    /// # Returns
    /// An error if the orchestrator encounters an unrecoverable failure
    pub async fn start(self) -> anyhow::Result<()> {
        info!("Batch orchestrator starting...");
        info!(
            "Configuration: max_batch_size={}, timeout_interval_ms={}, min_batch_size={}, max_gas_limit={}",
            self.config.max_batch_size,
            self.config.timeout_interval_ms,
            self.config.min_batch_size,
            self.config.max_gas_limit
        );
        info!("Scheduling policy: {}", self.scheduler.policy_name());

        let mut last_batch_time = Instant::now();

        loop {
            // Sleep for a short interval to avoid busy-waiting.
            // 100ms provides a good balance between responsiveness and CPU usage.
            sleep(Duration::from_millis(100)).await;

            // Evaluate all trigger conditions
            let trigger_reason = match self.trigger.should_seal(last_batch_time).await {
                Some(reason) => reason,
                None => continue, // No trigger fired — wait for next cycle
            };

            debug!("Batch trigger fired: {} ({}ms since last batch)",
                   trigger_reason,
                   last_batch_time.elapsed().as_millis());

            // Attempt to produce a batch
            match self.produce_batch().await {
                Ok(Some(batch)) => {
                    let tx_count = batch.transactions.len();
                    let forced_count = batch.transactions.iter()
                        .filter(|tx| matches!(tx, Transaction::Forced(_)))
                        .count();

                    info!(
                        "Batch #{} sealed: {} txs ({} forced, {} normal) | trigger: {} | policy: {}",
                        batch.batch_id,
                        tx_count,
                        forced_count,
                        tx_count - forced_count,
                        trigger_reason,
                        self.scheduler.policy_name()
                    );

                    // Store batch metadata in the registry for persistent tracking
                    let metadata = BatchMetadata {
                        batch_id: batch.batch_id,
                        tx_count,
                        forced_tx_count: forced_count,
                        timestamp: batch.timestamp,
                        scheduling_policy: self.scheduler.policy_name().to_string(),
                    };

                    if let Err(e) = self.registry.store(&metadata).await {
                        error!("Failed to store batch metadata: {:?}", e);
                        // Continue processing — metadata storage failure is non-fatal
                    }

                    // TODO: Send batch to executor component
                    // In the full system, this would be:
                    // executor_channel.send(batch).await?;

                    // Reset timer after successful batch creation
                    self.trigger.reset(&mut last_batch_time);
                }
                Ok(None) => {
                    // Trigger fired but no transactions were available.
                    // This can happen if the trigger evaluation and the drain
                    // race with each other. Reset timer and continue.
                    debug!("Trigger fired but no transactions available for batching");
                    self.trigger.reset(&mut last_batch_time);
                }
                Err(e) => {
                    warn!("Failed to produce batch: {:?}", e);
                    // Don't reset timer on error — will retry on next trigger
                }
            }
        }
    }

    /// Produce a batch by pulling transactions and scheduling them
    ///
    /// This is the core batch production logic:
    /// 1. Pull all forced transactions (always included first)
    /// 2. Pull normal transactions respecting both size and gas limits
    /// 3. Pass through the scheduler for policy-based ordering
    /// 4. Create sealed batch via the batch engine
    ///
    /// # Gas Limit Enforcement
    /// The engine tracks cumulative gas consumption as transactions are added,
    /// ensuring no batch exceeds the configured gas limit that would make L1
    /// verification prohibitively expensive.
    ///
    /// # Returns
    /// * `Ok(Some(Batch))` if a batch was created
    /// * `Ok(None)` if no transactions were available
    /// * `Err` if batch creation failed
    async fn produce_batch(&self) -> anyhow::Result<Option<Batch>> {
        // Step 1: Drain all forced transactions from L1
        // These have absolute priority and cannot be skipped
        let forced_txs = self.forced_queue.get_all().await;

        // Get read-only access to batch engine for gas limit checking
        let engine = self.batch_engine.read().await;

        // Step 1a: Filter forced transactions to respect gas limit.
        // Even forced txs must respect gas limits to ensure the batch
        // can be verified on L1. Excess forced txs are deferred.
        let mut accepted_forced_txs = Vec::new();
        let mut deferred_forced_txs = Vec::new();

        for tx in forced_txs {
            let wrapped_tx = Transaction::Forced(tx.clone());
            if engine.can_add_transaction(&accepted_forced_txs, &wrapped_tx) {
                accepted_forced_txs.push(wrapped_tx);
            } else {
                warn!("Forced transaction exceeds gas limit, deferring to next batch");
                deferred_forced_txs.push(tx);
            }
        }

        // Re-queue any deferred forced transactions so they aren't lost.
        // They will be picked up in the next batch production cycle.
        for tx in deferred_forced_txs {
            self.forced_queue.add(tx).await;
        }

        // Step 2: Pull normal transactions from pool with size limit enforcement.
        // Leave room for forced txs that were already accepted.
        let max_normal_txs = self.config.max_batch_size
            .saturating_sub(accepted_forced_txs.len());

        let normal_txs = self.tx_pool.get_pending(max_normal_txs).await;

        // Step 2a: Filter normal transactions to respect gas limit.
        // Track cumulative gas alongside accepted forced txs.
        let mut accepted_normal_txs = Vec::new();
        let mut combined_for_gas_check = accepted_forced_txs.clone();

        for tx in normal_txs {
            let wrapped_tx = Transaction::Normal(tx);
            if engine.can_add_transaction(&combined_for_gas_check, &wrapped_tx) {
                combined_for_gas_check.push(wrapped_tx.clone());
                accepted_normal_txs.push(wrapped_tx);
            } else {
                // Gas limit reached — stop adding transactions.
                // Remaining transactions stay in the pool for the next batch.
                debug!("Gas limit reached, stopping transaction addition");
                break;
            }
        }

        // Release the read lock before scheduling
        drop(engine);

        // If no transactions at all, return None
        if accepted_forced_txs.is_empty() && accepted_normal_txs.is_empty() {
            return Ok(None);
        }

        debug!(
            "Scheduling {} forced + {} normal transactions",
            accepted_forced_txs.len(),
            accepted_normal_txs.len()
        );

        // Step 3: Pass through the scheduler for policy-based ordering.
        // The scheduler ensures:
        //   - ALL forced transactions come first (preserving L1 order)
        //   - Normal transactions are ordered by the configured policy
        //     (FCFS, FeePriority, TimeBoost, or FairBFT)

        // Extract the inner types from the Transaction enum for the scheduler
        let forced_inner: Vec<_> = accepted_forced_txs.into_iter()
            .filter_map(|tx| match tx {
                Transaction::Forced(inner) => Some(inner),
                _ => None,
            })
            .collect();

        let normal_inner: Vec<_> = accepted_normal_txs.into_iter()
            .filter_map(|tx| match tx {
                Transaction::Normal(inner) => Some(inner),
                _ => None,
            })
            .collect();

        // Delegate ordering to the scheduler (applies the configured policy)
        let ordered_txs = self.scheduler.schedule(forced_inner, normal_inner);

        // Calculate and log total gas for the batch
        let total_gas: u64 = ordered_txs.iter().map(|tx| tx.gas_limit()).sum();
        debug!("Batch total gas: {} / {}", total_gas, self.config.max_gas_limit);

        // Step 4: Create the sealed batch via the batch engine
        let mut engine = self.batch_engine.write().await;
        let batch = engine.create_batch(ordered_txs);

        Ok(Some(batch))
    }
}
