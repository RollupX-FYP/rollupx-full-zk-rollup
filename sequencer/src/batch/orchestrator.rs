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
use std::io::Write;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration, Instant};
use tracing::{debug, error, info, warn};

#[derive(Debug, serde::Serialize)]
struct SequencerBatchMetricsRow {
    batch_id: u64,
    experiment_id: String,
    sealed_at_ms: u64,
    seal_reason: String,
    configured_max_batch_size: usize,
    configured_min_batch_size: usize,
    configured_timeout_ms: u64,
    tx_count: usize,
    forced_tx_count: usize,
    normal_tx_count: usize,
    total_gas_limit: u64,
    gas_limit_max: u64,
    gas_limit_utilization: f64,
    total_gas_price_wei: String,
    fee_proxy_wei: String,
    oldest_tx_wait_ms: u64,
    batch_data_bytes: usize,
}

#[derive(Debug, Clone)]
struct ProducedBatch {
    batch: Batch,
    total_gas_limit: u64,
    total_gas_price_wei: u128,
    fee_proxy_wei: u128,
    oldest_tx_wait_ms: u64,
    batch_data_bytes: usize,
}

pub struct BatchOrchestrator {
    forced_queue: Arc<ForcedQueue>,
    tx_pool: Arc<TransactionPool>,
    scheduler: Scheduler,
    batch_engine: RwLock<BatchEngine>,
    trigger: BatchTrigger,
    registry: Arc<Registry>,
    config: BatchConfig,
    executor_grpc_url: String,
}

impl BatchOrchestrator {
    pub fn new(
        forced_queue: Arc<ForcedQueue>,
        tx_pool: Arc<TransactionPool>,
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
            config: batch_config,
            executor_grpc_url,
        }
    }

    fn now_unix_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    fn tx_timestamp_to_ms(timestamp: u64) -> u64 {
        if timestamp >= 1_000_000_000_000 {
            timestamp
        } else {
            timestamp.saturating_mul(1000)
        }
    }

    fn append_batch_metrics_row(&self, row: &SequencerBatchMetricsRow) {
        let metrics_root = std::env::var("METRICS_ROOT").unwrap_or_else(|_| "metrics".to_string());
        let path = std::path::PathBuf::from(metrics_root).join("sequencer_batch_metrics.jsonl");
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(line) = serde_json::to_string(row) {
            let _ = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .and_then(|mut f| writeln!(f, "{line}"));
        }
    }

    async fn publish_batch_to_executor(
        &self,
        batch: &Batch,
    ) -> anyhow::Result<PublishBatchResponse> {
        let batch_data = serde_json::to_vec(&batch.transactions)
            .map_err(|e| anyhow::anyhow!("serialize batch transactions for gRPC: {e}"))?;
        let payload = BatchPayload {
            batch_id: batch.batch_id.to_string(),
            batch_data,
            pre_state_root: batch.prev_state_root.as_bytes().to_vec(),
            post_state_root: batch.prev_state_root.as_bytes().to_vec(),
            da_commitment: Vec::new(),
            proof: Vec::new(),
            experiment_id: std::env::var("EXPERIMENT_ID").unwrap_or_default(),
        };
        let mut client = RollupServiceClient::connect(self.executor_grpc_url.clone())
            .await
            .map_err(|e| anyhow::anyhow!("connect executor gRPC {}: {e}", self.executor_grpc_url))?;
        let response = client
            .publish_batch(tonic::Request::new(payload))
            .await
            .map_err(|e| anyhow::anyhow!("publish_batch RPC failed: {e}"))?;
        Ok(response.into_inner())
    }

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
        match self.registry.get_next_batch_id().await {
            Ok(next_id) => {
                let mut engine = self.batch_engine.write().await;
                engine.set_next_batch_id(next_id);
                info!("Recovered next batch id from registry: {}", next_id);
            }
            Err(err) => warn!(
                "Failed to recover next batch id from registry (defaulting to 1): {:?}",
                err
            ),
        }

        let mut last_batch_time = Instant::now();
        loop {
            sleep(Duration::from_millis(100)).await;
            let trigger_reason = match self.trigger.should_seal(last_batch_time).await {
                Some(reason) => reason,
                None => continue,
            };
            debug!(
                "Batch trigger fired: {} ({}ms since last batch)",
                trigger_reason,
                last_batch_time.elapsed().as_millis()
            );
            match self.produce_batch().await {
                Ok(Some(produced)) => {
                    let batch = produced.batch;
                    let tx_count = batch.transactions.len();
                    let forced_count = batch
                        .transactions
                        .iter()
                        .filter(|tx| matches!(tx, Transaction::Forced(_)))
                        .count();
                    info!(
                        "Batch #{} sealed: {} txs ({} forced, {} normal) | trigger: {} | policy: {}",
                        batch.batch_id,
                        tx_count,
                        forced_count,
                        tx_count.saturating_sub(forced_count),
                        trigger_reason,
                        self.scheduler.policy_name()
                    );

                    let metadata = BatchMetadata {
                        batch_id: batch.batch_id,
                        tx_count,
                        forced_tx_count: forced_count,
                        timestamp: batch.timestamp,
                        scheduling_policy: self.scheduler.policy_name().to_string(),
                    };
                    if let Err(e) = self.registry.store(&metadata).await {
                        error!("Failed to store batch metadata: {:?}", e);
                    }

                    let gas_limit_utilization = if self.config.max_gas_limit == 0 {
                        0.0
                    } else {
                        produced.total_gas_limit as f64 / self.config.max_gas_limit as f64
                    };
                    self.append_batch_metrics_row(&SequencerBatchMetricsRow {
                        batch_id: batch.batch_id,
                        experiment_id: std::env::var("EXPERIMENT_ID").unwrap_or_default(),
                        sealed_at_ms: Self::now_unix_ms(),
                        seal_reason: match trigger_reason {
                            TriggerReason::ForcedTransactions => "ForcedTransactions".to_string(),
                            TriggerReason::SizeThreshold => "SizeThreshold".to_string(),
                            TriggerReason::Timeout => "Timeout".to_string(),
                        },
                        configured_max_batch_size: self.config.max_batch_size,
                        configured_min_batch_size: self.config.min_batch_size,
                        configured_timeout_ms: self.config.timeout_interval_ms,
                        tx_count,
                        forced_tx_count: forced_count,
                        normal_tx_count: tx_count.saturating_sub(forced_count),
                        total_gas_limit: produced.total_gas_limit,
                        gas_limit_max: self.config.max_gas_limit,
                        gas_limit_utilization,
                        total_gas_price_wei: produced.total_gas_price_wei.to_string(),
                        fee_proxy_wei: produced.fee_proxy_wei.to_string(),
                        oldest_tx_wait_ms: produced.oldest_tx_wait_ms,
                        batch_data_bytes: produced.batch_data_bytes,
                    });

                    match self.publish_batch_to_executor(&batch).await {
                        Ok(response) if response.accepted => info!(
                            "Published batch #{} to executor over gRPC ({})",
                            batch.batch_id, self.executor_grpc_url
                        ),
                        Ok(response) => warn!(
                            "Executor rejected batch #{} over gRPC: {}",
                            batch.batch_id, response.message
                        ),
                        Err(e) => warn!(
                            "Failed to publish batch #{} to executor gRPC {}: {:?}",
                            batch.batch_id, self.executor_grpc_url, e
                        ),
                    }
                    self.trigger.reset(&mut last_batch_time);
                }
                Ok(None) => {
                    debug!("Trigger fired but no transactions available for batching");
                    self.trigger.reset(&mut last_batch_time);
                }
                Err(e) => warn!("Failed to produce batch: {:?}", e),
            }
        }
    }

    async fn produce_batch(&self) -> anyhow::Result<Option<ProducedBatch>> {
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

        let max_normal_txs = self
            .config
            .max_batch_size
            .saturating_sub(accepted_forced_txs.len());
        let normal_txs = self.tx_pool.get_pending(max_normal_txs).await;
        let mut accepted_normal_txs = Vec::new();
        let mut combined_for_gas_check = accepted_forced_txs.clone();
        for tx in normal_txs {
            let wrapped_tx = Transaction::Normal(tx);
            if engine.can_add_transaction(&combined_for_gas_check, &wrapped_tx) {
                combined_for_gas_check.push(wrapped_tx.clone());
                accepted_normal_txs.push(wrapped_tx);
            } else {
                debug!("Gas limit reached, stopping transaction addition");
                break;
            }
        }
        drop(engine);
        if accepted_forced_txs.is_empty() && accepted_normal_txs.is_empty() {
            return Ok(None);
        }

        let forced_inner: Vec<_> = accepted_forced_txs
            .into_iter()
            .filter_map(|tx| match tx {
                Transaction::Forced(inner) => Some(inner),
                _ => None,
            })
            .collect();
        let normal_inner: Vec<_> = accepted_normal_txs
            .into_iter()
            .filter_map(|tx| match tx {
                Transaction::Normal(inner) => Some(inner),
                _ => None,
            })
            .collect();

        let ordered_txs = self.scheduler.schedule(forced_inner, normal_inner);
        let total_gas_limit: u64 = ordered_txs.iter().map(|tx| tx.gas_limit()).sum();
        debug!("Batch total gas: {} / {}", total_gas_limit, self.config.max_gas_limit);

        let mut total_gas_price_wei: u128 = 0;
        let mut fee_proxy_wei: u128 = 0;
        let mut oldest_tx_ts_ms = u64::MAX;
        let now_ms = Self::now_unix_ms();
        for tx in &ordered_txs {
            match tx {
                Transaction::Normal(inner) => {
                    total_gas_price_wei =
                        total_gas_price_wei.saturating_add(inner.gas_price.as_u128());
                    fee_proxy_wei = fee_proxy_wei.saturating_add(
                        inner
                            .gas_price
                            .as_u128()
                            .saturating_mul(inner.gas_limit as u128),
                    );
                    oldest_tx_ts_ms = oldest_tx_ts_ms.min(Self::tx_timestamp_to_ms(inner.timestamp));
                }
                Transaction::Forced(inner) => {
                    oldest_tx_ts_ms = oldest_tx_ts_ms.min(Self::tx_timestamp_to_ms(inner.timestamp));
                }
            }
        }
        let oldest_tx_wait_ms = if oldest_tx_ts_ms == u64::MAX {
            0
        } else {
            now_ms.saturating_sub(oldest_tx_ts_ms)
        };
        let batch_data_bytes = serde_json::to_vec(&ordered_txs).map(|v| v.len()).unwrap_or(0);
        let mut engine = self.batch_engine.write().await;
        let batch = engine.create_batch(ordered_txs);

        Ok(Some(ProducedBatch {
            batch,
            total_gas_limit,
            total_gas_price_wei,
            fee_proxy_wei,
            oldest_tx_wait_ms,
            batch_data_bytes,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::BatchOrchestrator;

    #[test]
    fn tx_timestamp_to_ms_converts_seconds_and_preserves_milliseconds() {
        assert_eq!(BatchOrchestrator::tx_timestamp_to_ms(1_700_000_000), 1_700_000_000_000);
        assert_eq!(
            BatchOrchestrator::tx_timestamp_to_ms(1_700_000_000_123),
            1_700_000_000_123
        );
    }
}
