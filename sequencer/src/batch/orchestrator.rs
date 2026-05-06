use crate::{
    batch::{trigger::TriggerReason, BatchEngine, BatchTrigger},
    config::BatchConfig,
    pool::{ForcedQueue, TransactionPool},
    proto::rollup::{
        rollup_service_client::RollupServiceClient, BatchPayload, PublishBatchResponse,
    },
    registry::Registry,
    scheduler::{create_policy, Scheduler, SchedulingPolicyType},
    Batch, BatchMetadata, Transaction,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration, Instant};
use tracing::{debug, error, info, warn};

#[derive(Debug, serde::Serialize)]
struct SequencerBatchMetricsRow {
    batch_id: u64,
    experiment_id: String,
    sealed_at_ms: u64,
    batch_created_time_ms: u64,
    seal_reason: String,
    
    // Scheduler Metadata
    batch_policy: String,
    scheduling_policy: String,
    scheduler_config: serde_json::Value,
    
    // Batch Composition
    tx_count: usize,
    forced_tx_count: usize,
    normal_tx_count: usize,
    mempool_depth_at_batch: usize,
    
    // Resource Utilization
    total_gas_limit: u64,
    gas_limit_max: u64,
    gas_limit_utilization: f64,
    estimated_batch_bytes: usize,
    blob_utilization: f64,
    total_gas_price_wei: String,
    fee_proxy_wei: String,
    
    // DA Estimates (Pre-Enrichment)
    estimated_da_bytes_pre_enrichment: usize,
    raw_tx_bytes: usize,

    // Wait Time Distribution
    wait_time_p50_ms: u64,
    wait_time_p95_ms: u64,
    wait_time_p99_ms: u64,
    wait_time_max_ms: u64,
    wait_time_min_ms: u64,
    wait_time_mean_ms: f64,
    wait_time_std_dev_ms: f64,
    jains_fairness_index: f64,

    // MEV Metrics
    actual_batch_fee_wei: String,
    optimal_batch_fee_wei: String,
    ordering_efficiency: f64,
    reordering_events: u32,
    max_reorder_distance: usize,

    // State Cache Diagnostics
    cache_hit_rate: f64,
    stale_nonce_rejections: u32,
    cache_age_ms: u64,

    // Pool Snapshot
    pool_depth_at_seal: usize,
    pool_depth_after_seal: usize,
    forced_queue_depth: usize,
    pool_growth_rate_tps: f64,
    time_since_last_seal_ms: u64,
    tx_arrival_time_ms: u64,
}

#[derive(Debug, Clone)]
struct ProducedBatch {
    batch: Batch,
    batch_created_time_ms: u64,
    mempool_depth_at_batch: usize,
    tx_arrival_time_ms: u64,
    total_gas_limit: u64,
    total_gas_price_wei: u128,
    fee_proxy_wei: u128,
    wait_times: crate::WaitTimeDistribution,
    mev: crate::MevMetrics,
    estimated_da_bytes_pre_enrichment: usize,
    raw_tx_bytes: usize,
    estimated_batch_bytes: usize,
    blob_utilization: f64,
}

pub struct BatchOrchestrator {
    forced_queue: Arc<ForcedQueue>,
    tx_pool: Arc<TransactionPool>,
    scheduler: Scheduler,
    batch_engine: RwLock<BatchEngine>,
    trigger: BatchTrigger,
    registry: Arc<Registry>,
    state_cache: crate::state::StateCache,
    config: BatchConfig,
    executor_grpc_url: String,
}

impl BatchOrchestrator {
    pub fn new(
        forced_queue: Arc<ForcedQueue>,
        tx_pool: Arc<TransactionPool>,
        state_cache: crate::state::StateCache,
        batch_config: BatchConfig,
        scheduling_policy: SchedulingPolicyType,
        registry: Arc<Registry>,
        executor_grpc_url: String,
    ) -> Self {
        let policy = create_policy(scheduling_policy);
        let trigger = BatchTrigger::new(batch_config.clone(), tx_pool.clone(), forced_queue.clone());
        Self {
            forced_queue,
            tx_pool,
            scheduler: Scheduler::new(policy),
            batch_engine: RwLock::new(BatchEngine::new(batch_config.clone())),
            trigger,
            registry,
            state_cache,
            config: batch_config,
            executor_grpc_url,
        }
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        info!("Batch orchestrator started");
        
        let last_batch_id = match self.registry.get_next_batch_id().await {
            Ok(id) => {
                let last = id.saturating_sub(1);
                if last > 0 {
                    info!("Resuming from batch #{}", last);
                } else {
                    info!("No previous batches found, starting from #1");
                }
                id
            }
            Err(err) => {
                warn!("Failed to recover next batch id from registry (defaulting to 1): {:?}", err);
                1
            }
        };
        
        {
            let mut engine = self.batch_engine.write().await;
            engine.set_next_batch_id(last_batch_id);
        }

        let mut last_batch_time = Instant::now();
        let mut last_batch_time_ms = Self::now_unix_ms();
        let mut last_total_received = self.tx_pool.total_received();

        loop {
            sleep(Duration::from_millis(100)).await;
            let trigger_reason = match self.trigger.should_seal(last_batch_time).await {
                Some(reason) => reason,
                None => continue,
            };
            
            debug!("Batch trigger fired: {} ({}ms since last batch)", trigger_reason, last_batch_time.elapsed().as_millis());
            
            match self.produce_batch(trigger_reason.clone()).await {
                Ok(Some(produced)) => {
                    let batch = produced.batch;
                    let tx_count = batch.transactions.len();
                    let forced_count = batch.transactions.iter().filter(|tx| matches!(tx, Transaction::Forced(_))).count();
                    
                    info!("Batch #{} sealed: {} txs ({} forced, {} normal) | trigger: {} | policy: {}", 
                        batch.batch_id, tx_count, forced_count, tx_count.saturating_sub(forced_count), trigger_reason, self.scheduler.policy_name());

                    if let Err(e) = self.registry.store(&BatchMetadata {
                        batch_id: batch.batch_id,
                        tx_count,
                        forced_tx_count: forced_count,
                        scheduling_policy: self.scheduler.policy_name().to_string(),
                        timestamp: batch.timestamp,
                    }).await {
                        error!("Failed to store batch metadata: {:?}", e);
                    }

                    let pool_depth_at_seal = self.tx_pool.pending_count().await;
                    let forced_queue_depth = self.forced_queue.get_all().await.len();
                    
                    let now_ms = Self::now_unix_ms();
                    let current_total_received = self.tx_pool.total_received();
                    let arrivals = current_total_received.saturating_sub(last_total_received);
                    let interval_ms = now_ms.saturating_sub(last_batch_time_ms);
                    let growth_rate = if interval_ms > 0 {
                        (arrivals as f64 * 1000.0) / interval_ms as f64
                    } else {
                        0.0
                    };

                    let state_cache_metrics = self.state_cache_metrics().await;
                    let gas_limit_utilization = if self.config.max_gas_limit == 0 {
                        0.0
                    } else {
                        produced.total_gas_limit as f64 / self.config.max_gas_limit as f64
                    };

                    self.append_batch_metrics_row(&SequencerBatchMetricsRow {
                        batch_id: batch.batch_id,
                        experiment_id: std::env::var("EXPERIMENT_ID").unwrap_or_default(),
                        sealed_at_ms: now_ms,
                        batch_created_time_ms: produced.batch_created_time_ms,
                        seal_reason: trigger_reason.to_string(),
                        batch_policy: self.config.batch_policy.clone(),
                        scheduling_policy: self.scheduler.policy_name().to_string(),
                        scheduler_config: self.scheduler.policy_config(),
                        tx_count,
                        forced_tx_count: forced_count,
                        normal_tx_count: tx_count.saturating_sub(forced_count),
                        mempool_depth_at_batch: produced.mempool_depth_at_batch,
                        total_gas_limit: produced.total_gas_limit,
                        gas_limit_max: self.config.max_gas_limit,
                        gas_limit_utilization,
                        estimated_batch_bytes: produced.estimated_batch_bytes,
                        blob_utilization: produced.blob_utilization,
                        total_gas_price_wei: produced.total_gas_price_wei.to_string(),
                        fee_proxy_wei: produced.fee_proxy_wei.to_string(),
                        estimated_da_bytes_pre_enrichment: produced.estimated_da_bytes_pre_enrichment,
                        raw_tx_bytes: produced.raw_tx_bytes,
                        wait_time_p50_ms: produced.wait_times.p50_wait_ms,
                        wait_time_p95_ms: produced.wait_times.p95_wait_ms,
                        wait_time_p99_ms: produced.wait_times.p99_wait_ms,
                        wait_time_max_ms: produced.wait_times.max_wait_ms,
                        wait_time_min_ms: produced.wait_times.min_wait_ms,
                        wait_time_mean_ms: produced.wait_times.mean_wait_ms,
                        wait_time_std_dev_ms: produced.wait_times.std_dev_wait_ms,
                        jains_fairness_index: produced.wait_times.jains_fairness_index,
                        actual_batch_fee_wei: produced.mev.actual_batch_fee_wei.to_string(),
                        optimal_batch_fee_wei: produced.mev.optimal_batch_fee_wei.to_string(),
                        ordering_efficiency: produced.mev.ordering_efficiency,
                        reordering_events: produced.mev.reordering_events,
                        max_reorder_distance: produced.mev.max_reorder_distance,
                        cache_hit_rate: state_cache_metrics.cache_hit_rate,
                        stale_nonce_rejections: state_cache_metrics.stale_nonce_rejections,
                        cache_age_ms: state_cache_metrics.cache_age_ms,
                        pool_depth_at_seal,
                        pool_depth_after_seal: self.tx_pool.pending_count().await,
                        forced_queue_depth,
                        pool_growth_rate_tps: growth_rate,
                        time_since_last_seal_ms: interval_ms,
                        tx_arrival_time_ms: produced.tx_arrival_time_ms,
                    });

                    last_batch_time_ms = now_ms;
                    last_total_received = current_total_received;
                    last_batch_time = Instant::now();

                    match self.publish_batch_to_executor(&batch).await {
                        Ok(response) if response.accepted => info!("Published batch #{} to executor", batch.batch_id),
                        Ok(_) => warn!("Batch #{} rejected by executor", batch.batch_id),
                        Err(e) => error!("Failed to publish batch #{}: {:?}", batch.batch_id, e),
                    }
                }
                Ok(None) => {
                    // No transactions to batch
                }
                Err(e) => {
                    error!("Error producing batch: {:?}", e);
                }
            }
        }
    }

    async fn produce_batch(&self, _trigger_reason: TriggerReason) -> anyhow::Result<Option<ProducedBatch>> {
        let forced_txs = self.forced_queue.get_all().await;
        let engine = self.batch_engine.read().await;
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
        for tx in deferred_forced_txs {
            self.forced_queue.add(tx).await;
        }

        let normal_pool_depth = self.tx_pool.pending_count().await;
        let target_batch_size = self.trigger.target_batch_size_for_depth(normal_pool_depth);
        let max_normal_txs = target_batch_size.saturating_sub(accepted_forced_txs.len());
        let mempool_depth_at_batch = normal_pool_depth.saturating_add(accepted_forced_txs.len());
        let remaining_blob_bytes = self
            .config
            .blob_target_bytes
            .saturating_sub(accepted_forced_txs.iter().map(|tx| tx.estimated_encoded_bytes()).sum::<usize>());

        let (normal_txs, _packed_blob_bytes) = if self.scheduler.policy_name() == "BlobPacking" {
            self.tx_pool
                .take_blob_packed(max_normal_txs, remaining_blob_bytes)
                .await
        } else {
            (self.tx_pool.get_pending(max_normal_txs).await, 0usize)
        };
        let mut accepted_normal_txs = Vec::new();
        let mut combined_for_gas_check = accepted_forced_txs.clone();
        let mut deferred_normal_txs = Vec::new();
        for tx in normal_txs {
            let wrapped_tx = Transaction::Normal(tx);
            if engine.can_add_transaction(&combined_for_gas_check, &wrapped_tx) {
                combined_for_gas_check.push(wrapped_tx.clone());
                accepted_normal_txs.push(wrapped_tx);
            } else {
                debug!("Gas limit reached, stopping transaction addition");
                if let Transaction::Normal(inner) = wrapped_tx {
                    deferred_normal_txs.push(inner);
                }
            }
        }
        for tx in deferred_normal_txs {
            self.tx_pool.add(tx).await;
        }
        drop(engine);
        
        if accepted_forced_txs.is_empty() && accepted_normal_txs.is_empty() {
            return Ok(None);
        }

        let forced_inner: Vec<_> = accepted_forced_txs.into_iter().filter_map(|tx| match tx {
            Transaction::Forced(inner) => Some(inner),
            _ => None,
        }).collect();
        let normal_inner: Vec<_> = accepted_normal_txs.into_iter().filter_map(|tx| match tx {
            Transaction::Normal(inner) => Some(inner),
            _ => None,
        }).collect();

        let ordered_txs = self.scheduler.schedule(forced_inner, normal_inner);
        let total_gas_limit: u64 = ordered_txs.iter().map(|tx| tx.gas_limit()).sum();
        
        let mut total_gas_price_wei: u128 = 0;
        let mut fee_proxy_wei: u128 = 0;
        let mut wait_times = Vec::new();
        let now_ms = Self::now_unix_ms();

        for tx in &ordered_txs {
            match tx {
                Transaction::Normal(ptx) => {
                    total_gas_price_wei = total_gas_price_wei.saturating_add(ptx.tx.gas_price.as_u128());
                    fee_proxy_wei = fee_proxy_wei.saturating_add(ptx.tx.gas_price.as_u128().saturating_mul(ptx.tx.gas_limit as u128));
                    wait_times.push(now_ms.saturating_sub(ptx.arrived_at));
                }
                Transaction::Forced(inner) => {
                    wait_times.push(now_ms.saturating_sub(inner.timestamp.saturating_mul(1000)));
                }
            }
        }

        let wait_dist = Self::calculate_distribution_metrics(&wait_times);

        let mut sorted_by_fee = ordered_txs.clone();
        sorted_by_fee.sort_by(|a, b| {
            let fee_a = match a {
                Transaction::Normal(ptx) => ptx.tx.gas_price,
                Transaction::Forced(_) => ethers::types::U256::zero(),
            };
            let fee_b = match b {
                Transaction::Normal(ptx) => ptx.tx.gas_price,
                Transaction::Forced(_) => ethers::types::U256::zero(),
            };
            fee_b.cmp(&fee_a)
        });

        let mut actual_fee = ethers::types::U256::zero();
        let mut optimal_fee = ethers::types::U256::zero();
        let reordering_events = 0;
        let max_reorder_distance = 0;

        for tx in &ordered_txs {
            if let Transaction::Normal(ptx) = tx {
                actual_fee = actual_fee.saturating_add(ptx.tx.gas_price);
            }
        }

        for tx in &sorted_by_fee {
            if let Transaction::Normal(ptx) = tx {
                optimal_fee = optimal_fee.saturating_add(ptx.tx.gas_price);
            }
        }

        let ordering_efficiency = if optimal_fee.is_zero() { 1.0 } else { actual_fee.as_u128() as f64 / optimal_fee.as_u128() as f64 };

        let mev = crate::MevMetrics {
            actual_batch_fee_wei: actual_fee,
            optimal_batch_fee_wei: optimal_fee,
            ordering_efficiency,
            reordering_events,
            max_reorder_distance,
        };

        let raw_tx_bytes = serde_json::to_vec(&ordered_txs).map(|v| v.len()).unwrap_or(0);
        let estimated_da_bytes_pre_enrichment = raw_tx_bytes + 128;
        let estimated_batch_bytes = raw_tx_bytes;
        let blob_utilization = if self.config.blob_target_bytes == 0 {
            0.0
        } else {
            estimated_batch_bytes as f64 / self.config.blob_target_bytes as f64
        };
        let batch_created_time_ms = Self::now_unix_ms();
        let tx_arrival_time_ms = ordered_txs
            .iter()
            .map(|tx| tx.arrival_timestamp_ms())
            .min()
            .unwrap_or(0);

        let mut engine = self.batch_engine.write().await;
        let batch = engine.create_batch(ordered_txs);

        Ok(Some(ProducedBatch {
            batch,
            batch_created_time_ms,
            mempool_depth_at_batch,
            tx_arrival_time_ms,
            total_gas_limit,
            total_gas_price_wei,
            fee_proxy_wei,
            wait_times: wait_dist,
            mev,
            estimated_da_bytes_pre_enrichment,
            raw_tx_bytes,
            estimated_batch_bytes,
            blob_utilization,
        }))
    }

    async fn state_cache_metrics(&self) -> crate::StateCacheMetrics {
        self.state_cache.collect_metrics()
    }

    async fn publish_batch_to_executor(&self, batch: &Batch) -> anyhow::Result<PublishBatchResponse> {
        let mut client = RollupServiceClient::connect(self.executor_grpc_url.clone()).await?;
        let payload = BatchPayload {
            batch_id: batch.batch_id.to_string(),
            batch_data: serde_json::to_vec(&batch.transactions)?,
            pre_state_root: vec![0u8; 32],
            post_state_root: vec![0u8; 32],
            da_commitment: vec![0u8; 32],
            proof: vec![0u8; 32],
            experiment_id: std::env::var("EXPERIMENT_ID").unwrap_or_else(|_| "default".to_string()),
        };
        let request = tonic::Request::new(payload);
        let response = client.publish_batch(request).await?;
        Ok(response.into_inner())
    }

    fn now_unix_ms() -> u64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64
    }

    fn calculate_distribution_metrics(wait_times: &[u64]) -> crate::WaitTimeDistribution {
        if wait_times.is_empty() {
            return crate::WaitTimeDistribution {
                p50_wait_ms: 0, p95_wait_ms: 0, p99_wait_ms: 0, max_wait_ms: 0, min_wait_ms: 0, mean_wait_ms: 0.0, std_dev_wait_ms: 0.0, jains_fairness_index: 1.0,
            };
        }
        let mut sorted = wait_times.to_vec();
        sorted.sort_unstable();
        let n = sorted.len();
        let p50 = sorted[n / 2];
        let p95 = sorted[(n as f64 * 0.95) as usize % n];
        let p99 = sorted[(n as f64 * 0.99) as usize % n];
        let max = *sorted.last().unwrap_or(&0);
        let min = *sorted.first().unwrap_or(&0);
        let sum: u64 = sorted.iter().sum();
        let mean = sum as f64 / n as f64;
        let variance = sorted.iter().map(|&x| { let diff = x as f64 - mean; diff * diff }).sum::<f64>() / n as f64;
        let std_dev = variance.sqrt();
        let sum_sq = sorted.iter().map(|&x| (x as f64).powi(2)).sum::<f64>();
        let jains = if sum_sq == 0.0 { 1.0 } else { (sum as f64).powi(2) / (n as f64 * sum_sq) };
        crate::WaitTimeDistribution {
            p50_wait_ms: p50, p95_wait_ms: p95, p99_wait_ms: p99, max_wait_ms: max, min_wait_ms: min, mean_wait_ms: mean, std_dev_wait_ms: std_dev, jains_fairness_index: jains,
        }
    }

    fn append_batch_metrics_row(&self, row: &SequencerBatchMetricsRow) {
        let metrics_root = std::env::var("METRICS_ROOT").unwrap_or_else(|_| "metrics".to_string());
        let experiment_id = std::env::var("EXPERIMENT_ID").unwrap_or_else(|_| "default".to_string());
        let path = std::path::Path::new(&metrics_root).join(format!("sequencer_batches_{}.jsonl", experiment_id));
        if let Some(parent) = path.parent() { std::fs::create_dir_all(parent).ok(); }
        let file = std::fs::OpenOptions::new().create(true).append(true).open(path);
        if let Ok(mut file) = file {
            use std::io::Write;
            if let Ok(json) = serde_json::to_string(row) { writeln!(file, "{}", json).ok(); }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_distribution_metrics_empty() {
        let dist = BatchOrchestrator::calculate_distribution_metrics(&[]);
        assert_eq!(dist.p50_wait_ms, 0);
        assert_eq!(dist.jains_fairness_index, 1.0);
    }

    #[test]
    fn test_calculate_distribution_metrics_perfectly_fair() {
        let wait_times = vec![100, 100, 100, 100];
        let dist = BatchOrchestrator::calculate_distribution_metrics(&wait_times);
        assert_eq!(dist.p50_wait_ms, 100);
        assert_eq!(dist.mean_wait_ms, 100.0);
        assert_eq!(dist.std_dev_wait_ms, 0.0);
        // Jain's index should be 1.0 for equal values
        assert!((dist.jains_fairness_index - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_calculate_distribution_metrics_unfair() {
        // One user waits 1000ms, others wait 10ms
        let wait_times = vec![1000, 10, 10, 10];
        let dist = BatchOrchestrator::calculate_distribution_metrics(&wait_times);
        assert_eq!(dist.p50_wait_ms, 10);
        assert_eq!(dist.max_wait_ms, 1000);
        assert!(dist.jains_fairness_index < 0.5); // Should be significantly less than 1
    }

    #[test]
    fn test_now_unix_ms() {
        let now = BatchOrchestrator::now_unix_ms();
        assert!(now > 1_700_000_000_000); // Sane value for 2024+
    }
}
