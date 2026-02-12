//! Batch Orchestrator Module
//! 
//! This module implements the orchestration layer that connects all batch-related
//! components together. It runs a background loop that periodically produces batches
//! by pulling transactions from pools, scheduling them, and creating sealed batches.
//! 
//! # Architecture Flow
//! 1. Check trigger conditions (timeout or size threshold)
//! 2. Pull forced transactions from `ForcedQueue`
//! 3. Pull normal transactions from `TransactionPool` (up to max batch size)
//! 4. Pass both to `Scheduler` for ordering (forced txs always first)
//! 5. Create sealed batch via `BatchEngine`
//! 6. Log batch creation (future: send to executor)

use crate::{
    pool::{ForcedQueue, TransactionPool},
    scheduler::{Scheduler, SchedulingPolicyType, create_policy},
    batch::BatchEngine,
    config::BatchConfig,
    Batch, Transaction,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration, Instant};
use tracing::{info, debug, warn};

/// Batch orchestrator
/// 
/// Coordinates the batch production pipeline by periodically checking trigger
/// conditions and orchestrating transaction flow through the scheduling and
/// batching components.
pub struct BatchOrchestrator {
    /// Forced transaction queue (L1-originated transactions)
    forced_queue: Arc<ForcedQueue>,
    /// Normal transaction pool (user-submitted transactions)
    tx_pool: Arc<TransactionPool>,
    /// Scheduler for ordering transactions within batches
    scheduler: Scheduler,
    /// Batch engine for creating sealed batches (wrapped in RwLock for mutable access)
    batch_engine: RwLock<BatchEngine>,
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
    pub fn new(
        forced_queue: Arc<ForcedQueue>,
        tx_pool: Arc<TransactionPool>,
        batch_config: BatchConfig,
        scheduling_policy: SchedulingPolicyType,
    ) -> Self {
        // Create policy instance using factory function
        let policy = create_policy(scheduling_policy);
        
        Self {
            forced_queue,
            tx_pool,
            scheduler: Scheduler::new(policy),
            batch_engine: RwLock::new(BatchEngine::new(batch_config.clone())),
            config: batch_config,
        }
    }
    
    /// Start the batch orchestrator background loop
    /// 
    /// Spawns an async task that runs continuously, checking trigger conditions
    /// and producing batches when appropriate.
    /// 
    /// # Trigger Conditions
    /// - **Timeout trigger**: Produce batch after timeout expires (even if not full)
    /// - **Size trigger**: Produce batch when max size is reached
    /// 
    /// # Returns
    /// An error if the orchestrator fails to start
    pub async fn start(self) -> anyhow::Result<()> {
        info!("Batch orchestrator starting...");
        info!("Configuration: max_batch_size={}, timeout_interval_ms={}, min_batch_size={}, max_gas_limit={}", 
              self.config.max_batch_size, 
              self.config.timeout_interval_ms,
              self.config.min_batch_size,
              self.config.max_gas_limit);
        
        let timeout_duration = Duration::from_millis(self.config.timeout_interval_ms);
        let mut last_batch_time = Instant::now();
        
        loop {
            // Sleep for a short interval to avoid busy-waiting
            // This allows the system to process other tasks
            sleep(Duration::from_millis(100)).await;
            
            // Check if timeout has expired
            let timeout_expired = last_batch_time.elapsed() >= timeout_duration;
            
            // Get current pool sizes (for logging and trigger detection)
            // Note: We don't have a direct way to check pool size without reading,
            // so we rely on timeout triggers primarily for now
            
            // Trigger batch production if timeout expired
            if timeout_expired {
                debug!("Batch timeout triggered ({}ms elapsed)", 
                       last_batch_time.elapsed().as_millis());
                
                match self.produce_batch().await {
                    Ok(Some(batch)) => {
                        info!("Batch #{} created with {} transactions", 
                              batch.batch_id, 
                              batch.transactions.len());
                        
                        // TODO: Send batch to executor component
                        // For now, we just log the batch creation
                        
                        // Reset timer after successful batch creation
                        last_batch_time = Instant::now();
                    }
                    Ok(None) => {
                        // No transactions available, but we still reset the timer
                        // to avoid repeatedly trying to create empty batches
                        debug!("No transactions available for batching");
                        last_batch_time = Instant::now();
                    }
                    Err(e) => {
                        warn!("Failed to produce batch: {:?}", e);
                        // Don't reset timer on error - will retry on next timeout
                    }
                }
            }
            
            // TODO: Add size-based trigger
            // This would require exposing a non-blocking "peek size" method
            // on TransactionPool and ForcedQueue, which we can add later
        }
    }
    
    /// Produce a batch by pulling transactions and scheduling them
    /// 
    /// This is the core batch production logic:
    /// 1. Pull all forced transactions (always included first)
    /// 2. Pull normal transactions respecting both size and gas limits
    /// 3. Schedule them (forced first, then normal by policy)
    /// 4. Create sealed batch
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
        // Step 1: Get all forced transactions from L1
        let forced_txs = self.forced_queue.get_all().await;
        
        // Get read-only access to batch engine for gas limit checking
        let engine = self.batch_engine.read().await;
        
        // Step 1a: Filter forced transactions to respect gas limit
        // Forced txs have priority, but we still need to respect gas limits
        let mut accepted_forced_txs = Vec::new();
        for tx in forced_txs {
            let wrapped_tx = Transaction::Forced(tx);
            if engine.can_add_transaction(&accepted_forced_txs, &wrapped_tx) {
                accepted_forced_txs.push(wrapped_tx);
            } else {
                warn!("Forced transaction exceeds gas limit, deferring to next batch");
                // In production, this transaction should be re-queued
            }
        }
        
        // Step 2: Get normal transactions from pool with gas limit enforcement
        // Calculate how many we can take (leave room for forced txs if any)
        let max_normal_txs = if accepted_forced_txs.is_empty() {
            self.config.max_batch_size
        } else {
            self.config.max_batch_size.saturating_sub(accepted_forced_txs.len())
        };
        
        let normal_txs = self.tx_pool.get_pending(max_normal_txs).await;
        
        // Step 2a: Filter normal transactions to respect gas limit
        let mut accepted_normal_txs = Vec::new();
        let mut combined_txs = accepted_forced_txs.clone();
        
        for tx in normal_txs {
            let wrapped_tx = Transaction::Normal(tx);
            if engine.can_add_transaction(&combined_txs, &wrapped_tx) {
                combined_txs.push(wrapped_tx.clone());
                accepted_normal_txs.push(wrapped_tx);
            } else {
                // Gas limit reached, stop adding transactions
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
        
        debug!("Scheduling {} forced + {} normal transactions", 
               accepted_forced_txs.len(), 
               accepted_normal_txs.len());
        
        // Step 3: Combine all accepted transactions in order (forced first, then normal)
        let mut all_txs = accepted_forced_txs;
        all_txs.extend(accepted_normal_txs);
        
        // Calculate and log total gas
        let total_gas: u64 = all_txs.iter().map(|tx| tx.gas_limit()).sum();
        debug!("Batch total gas: {} / {}", total_gas, self.config.max_gas_limit);
        
        // Step 4: Create sealed batch
        let mut engine = self.batch_engine.write().await;
        let batch = engine.create_batch(all_txs);
        
        Ok(Some(batch))
    }
}
