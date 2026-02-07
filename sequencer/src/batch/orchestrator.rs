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
    scheduler::Scheduler,
    batch::BatchEngine,
    config::BatchConfig,
    Batch,
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
    /// * `scheduling_policy` - Scheduling policy ("FCFS" or "FeePriority")
    pub fn new(
        forced_queue: Arc<ForcedQueue>,
        tx_pool: Arc<TransactionPool>,
        batch_config: BatchConfig,
        scheduling_policy: String,
    ) -> Self {
        Self {
            forced_queue,
            tx_pool,
            scheduler: Scheduler::new(scheduling_policy.clone()),
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
        info!("Configuration: max_batch_size={}, timeout_interval_ms={}, min_batch_size={}", 
              self.config.max_batch_size, 
              self.config.timeout_interval_ms,
              self.config.min_batch_size);
        
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
    /// 2. Pull up to max_batch_size normal transactions
    /// 3. Schedule them (forced first, then normal by policy)
    /// 4. Create sealed batch
    /// 
    /// # Returns
    /// * `Ok(Some(Batch))` if a batch was created
    /// * `Ok(None)` if no transactions were available
    /// * `Err` if batch creation failed
    async fn produce_batch(&self) -> anyhow::Result<Option<Batch>> {
        // Step 1: Get all forced transactions from L1
        let forced_txs = self.forced_queue.get_all().await;
        
        // Step 2: Get normal transactions from pool
        // Calculate how many we can take (leave room for forced txs if any)
        let max_normal_txs = if forced_txs.is_empty() {
            self.config.max_batch_size
        } else {
            self.config.max_batch_size.saturating_sub(forced_txs.len())
        };
        
        let normal_txs = self.tx_pool.get_pending(max_normal_txs).await;
        
        // If no transactions at all, return None
        if forced_txs.is_empty() && normal_txs.is_empty() {
            return Ok(None);
        }
        
        debug!("Scheduling {} forced + {} normal transactions", 
               forced_txs.len(), 
               normal_txs.len());
        
        // Step 3: Schedule transactions (forced first, then normal by policy)
        let scheduled_txs = self.scheduler.schedule(forced_txs, normal_txs);
        
        // Step 4: Create sealed batch
        let mut engine = self.batch_engine.write().await;
        let batch = engine.create_batch(scheduled_txs);
        
        Ok(Some(batch))
    }
}
