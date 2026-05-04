use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use futures_util::Stream;
use tokio::sync::{broadcast, Mutex};
use tokio_stream::{wrappers::BroadcastStream, StreamExt};
use tonic::{transport::Server, Request, Response, Status};
use tracing::{error, info, warn};

use crate::block_constructor::build_enriched_payload;
use crate::proof::{backend_from_env, backend_label, generate_artifacts, ProverBackend};
use crate::proto::rollup::{
    rollup_service_server::{RollupService, RollupServiceServer}, BatchPayload, PublishBatchResponse,
    StreamRequest, SubmitTxsRequest, SubmitTxsResponse,
};
use crate::state::RocksDbStateManager;
use crate::trace::{append_lifecycle, persist_trace, verify_trace_hash, TraceLifecycleStatus};
use crate::tx_engine::{SimpleTransactionEngine, TransactionEngine};
use crate::types::{normalize_transactions, SequencerTransaction};

#[derive(Debug, Clone, serde::Serialize)]
struct ExecutorMetrics {
    mode: String,
    execution_performed: bool,
    prover_backend: String,
    batch_count: u64,
    received_tx_count: u64,
    forwarded_batch_count: u64,
    duration_s: u64,
    execution_times_ms: Vec<u64>,
    proof_generation_times_ms: Vec<u64>,
    total_proved_txs: u64,
    trace_id: String,
    #[serde(skip)]
    start_time: Instant,
}

#[derive(Debug, Clone, serde::Serialize)]
struct ExecutorBatchMetricsRow {
    experiment_id: String,
    batch_id: String,
    trace_id: String,
    tx_count: u64,
    batch_data_bytes: usize,
    state_diff_count: usize,
    state_diff_bytes: usize,
    unique_touched_accounts: usize,
    repeated_touched_accounts: usize,
    total_gas_limit: u64,
    total_gas_price_wei: u64,
    fee_proxy_wei: u128,
    execution_time_ms: u64,
    proof_time_ms: u64,
    proof_bytes: usize,
    journal_bytes: usize,
}

fn touched_account_stats(
    diffs: &[crate::types::StateDiff],
) -> (usize, usize) {
    let mut touched_account_counts: HashMap<[u8; 20], usize> = HashMap::new();
    for diff in diffs {
        let entry = touched_account_counts.entry(diff.account).or_insert(0);
        *entry += 1;
    }
    let unique_touched_accounts = touched_account_counts.len();
    let repeated_touched_accounts = touched_account_counts.values().filter(|v| **v > 1).count();
    (unique_touched_accounts, repeated_touched_accounts)
}

#[derive(Clone)]
struct ExecutorGrpcService {
    tx: broadcast::Sender<BatchPayload>,
    metrics: Arc<Mutex<HashMap<String, ExecutorMetrics>>>,
    engine: Arc<Mutex<SimpleTransactionEngine<RocksDbStateManager>>>,
    prover_backend: ProverBackend,
    trace_root: PathBuf,
}

#[tonic::async_trait]
impl RollupService for ExecutorGrpcService {
    type StreamBatchesStream = Pin<Box<dyn Stream<Item = Result<BatchPayload, Status>> + Send + 'static>>;

    async fn stream_batches(
        &self,
        _request: Request<StreamRequest>,
    ) -> Result<Response<Self::StreamBatchesStream>, Status> {
        let rx = self.tx.subscribe();
        let stream = BroadcastStream::new(rx).filter_map(|item| match item {
            Ok(payload) => Some(Ok(payload)),
            Err(e) => {
                warn!("Dropping broadcast stream message: {e}");
                None
            }
        });

        Ok(Response::new(Box::pin(stream)))
    }

    async fn submit_transactions(
        &self,
        request: Request<SubmitTxsRequest>,
    ) -> Result<Response<SubmitTxsResponse>, Status> {
        Ok(Response::new(SubmitTxsResponse {
            accepted: request.into_inner().txs.len() as u64,
        }))
    }

    async fn publish_batch(
        &self,
        request: Request<BatchPayload>,
    ) -> Result<Response<PublishBatchResponse>, Status> {
        let payload = request.into_inner();
        let batch_id = payload.batch_id.clone();
        let experiment_id = if payload.experiment_id.is_empty() {
            std::env::var("EXPERIMENT_ID").unwrap_or_else(|_| "default".to_string())
        } else {
            payload.experiment_id.clone()
        };

        let parsed_txs: Vec<SequencerTransaction> = serde_json::from_slice(&payload.batch_data)
            .map_err(|e| Status::invalid_argument(format!("invalid batch_data JSON: {e}")))?;
        let txs = normalize_transactions(parsed_txs)
            .map_err(|e| Status::invalid_argument(format!("invalid transaction payload: {e}")))?;

        let execution_start = Instant::now();
        let trace = self
            .engine
            .lock()
            .await
            .execute_batch(&batch_id, txs)
            .map_err(|e| Status::internal(format!("execution failure: {e}")))?;
        let execution_ms = execution_start.elapsed().as_millis() as u64;

        append_lifecycle(&self.trace_root, &trace, TraceLifecycleStatus::Generated, None, None)
            .map_err(|e| Status::internal(format!("trace lifecycle(generated) failure: {e}")))?;

        let persisted = persist_trace(&self.trace_root, &trace)
            .map_err(|e| Status::internal(format!("trace persistence failure: {e}")))?;

        verify_trace_hash(&persisted.trace_path, &persisted.sha256_hex)
            .map_err(|e| Status::internal(format!("trace integrity check failed: {e}")))?;

        append_lifecycle(
            &self.trace_root,
            &trace,
            TraceLifecycleStatus::Persisted,
            Some(&persisted.trace_path),
            Some(&persisted.sha256_hex),
        )
        .map_err(|e| Status::internal(format!("trace lifecycle(persisted) failure: {e}")))?;

        let proof_start = Instant::now();
        let artifacts = generate_artifacts(&trace, &self.prover_backend)
            .map_err(|e| Status::internal(format!("proof generation failure: {e}")))?;
        let proof_ms = proof_start.elapsed().as_millis() as u64;
        let proof_bytes = artifacts.proof_bytes;
        let journal_bytes = artifacts.journal_bytes;
        let batch_data_len = payload.batch_data.len();

        append_lifecycle(
            &self.trace_root,
            &trace,
            TraceLifecycleStatus::Proved,
            Some(&persisted.trace_path),
            Some(&persisted.sha256_hex),
        )
        .map_err(|e| Status::internal(format!("trace lifecycle(proved) failure: {e}")))?;

        let proved_txs = trace.executed_transactions.len() as u64;
        let total_gas_limit: u64 = trace.executed_transactions.iter().map(|tx| tx.gas_limit).sum();
        let total_gas_price_wei: u64 =
            trace.executed_transactions.iter().map(|tx| tx.gas_price).sum();
        let fee_proxy_wei: u128 = trace
            .executed_transactions
            .iter()
            .map(|tx| (tx.gas_price as u128).saturating_mul(tx.gas_limit as u128))
            .sum();
        let (unique_touched_accounts, repeated_touched_accounts) =
            touched_account_stats(&trace.state_diffs);
        let state_diff_bytes = serde_json::to_vec(&trace.state_diffs)
            .map(|v| v.len())
            .unwrap_or_default();

        let enriched = build_enriched_payload(payload, &trace, artifacts.da_commitment, artifacts.proof);

        self.tx.send(enriched).map_err(|e| {
            Status::unavailable(format!("no stream subscribers available to receive batch: {e}"))
        })?;

        append_lifecycle(
            &self.trace_root,
            &trace,
            TraceLifecycleStatus::Published,
            Some(&persisted.trace_path),
            Some(&persisted.sha256_hex),
        )
        .map_err(|e| Status::internal(format!("trace lifecycle(published) failure: {e}")))?;

        info!("Executed and published batch {batch_id} with {proved_txs} transactions (trace_id={})", trace.trace_id);

        let batch_row = ExecutorBatchMetricsRow {
            experiment_id: experiment_id.clone(),
            batch_id: batch_id.clone(),
            trace_id: trace.trace_id.clone(),
            tx_count: proved_txs,
            batch_data_bytes: batch_data_len,
            state_diff_count: trace.state_diffs.len(),
            state_diff_bytes,
            unique_touched_accounts,
            repeated_touched_accounts,
            total_gas_limit,
            total_gas_price_wei,
            fee_proxy_wei,
            execution_time_ms: execution_ms,
            proof_time_ms: proof_ms,
            proof_bytes,
            journal_bytes,
        };
        let metrics_root = std::env::var("METRICS_ROOT").unwrap_or_else(|_| "metrics".to_string());
        let batch_metrics_path = std::path::PathBuf::from(&metrics_root).join("executor_batch_metrics.jsonl");
        if let Some(parent) = batch_metrics_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(line) = serde_json::to_string(&batch_row) {
            let _ = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(batch_metrics_path)
                .and_then(|mut f| writeln!(f, "{line}"));
        }

        let metrics_clone = self.metrics.clone();
        let prover_backend = backend_label(&self.prover_backend).to_string();
        let trace_id = trace.trace_id.clone();
        tokio::spawn(async move {
            let mut metrics_guard = metrics_clone.lock().await;
            let metrics = metrics_guard
                .entry(experiment_id.clone())
                .or_insert_with(|| ExecutorMetrics {
                    mode: "grpc_executor".to_string(),
                    execution_performed: true,
                    prover_backend,
                    batch_count: 0,
                    received_tx_count: 0,
                    forwarded_batch_count: 0,
                    duration_s: 0,
                    execution_times_ms: vec![],
                    proof_generation_times_ms: vec![],
                    total_proved_txs: 0,
                    trace_id,
                    start_time: Instant::now(),
                });

            metrics.batch_count += 1;
            metrics.received_tx_count += proved_txs;
            metrics.forwarded_batch_count += 1;
            metrics.total_proved_txs += proved_txs;
            metrics.execution_times_ms.push(execution_ms);
            metrics.proof_generation_times_ms.push(proof_ms);
            metrics.duration_s = metrics.start_time.elapsed().as_secs();

            let metrics_root = std::env::var("METRICS_ROOT").unwrap_or_else(|_| "metrics".to_string());
            let filename = format!("executor_{}.json", experiment_id);
            let filepath = std::path::PathBuf::from(&metrics_root).join(filename);

            if let Err(e) = tokio::fs::create_dir_all(&metrics_root).await {
                error!("Failed to create metrics directory {}: {}", metrics_root, e);
                return;
            }

            if let Ok(json) = serde_json::to_string_pretty(metrics) {
                if let Err(e) = tokio::fs::write(&filepath, json).await {
                    error!("Failed to write executor metrics to {}: {}", filepath.display(), e);
                }
            }
        });

        Ok(Response::new(PublishBatchResponse {
            accepted: true,
            message: "batch executed and accepted".to_string(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::touched_account_stats;
    use crate::types::{StateDiff, WitnessPathElement};

    #[test]
    fn touched_account_stats_counts_unique_and_repeated_accounts() {
        let a = [1u8; 20];
        let b = [2u8; 20];
        let diffs = vec![
            StateDiff {
                account: a,
                old_balance: 0,
                new_balance: 1,
                old_nonce: 0,
                new_nonce: 1,
                merkle_proof: vec![],
                witness_path: Vec::<WitnessPathElement>::new(),
                leaf_encoding: "default".to_string(),
            },
            StateDiff {
                account: a,
                old_balance: 1,
                new_balance: 2,
                old_nonce: 1,
                new_nonce: 2,
                merkle_proof: vec![],
                witness_path: Vec::<WitnessPathElement>::new(),
                leaf_encoding: "default".to_string(),
            },
            StateDiff {
                account: b,
                old_balance: 0,
                new_balance: 3,
                old_nonce: 0,
                new_nonce: 1,
                merkle_proof: vec![],
                witness_path: Vec::<WitnessPathElement>::new(),
                leaf_encoding: "default".to_string(),
            },
        ];
        let (unique, repeated) = touched_account_stats(&diffs);
        assert_eq!(unique, 2);
        assert_eq!(repeated, 1);
    }
}

pub async fn run_server_from_env() -> Result<()> {
    let bind_addr = std::env::var("EXECUTOR_GRPC_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:50051".to_string())
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid EXECUTOR_GRPC_ADDR: {e}"))?;

    let trace_root = PathBuf::from(std::env::var("TRACE_ROOT").unwrap_or_else(|_| "trace_artifacts".to_string()));
    let state_db_path = std::env::var("STATE_DB_PATH").unwrap_or_else(|_| "state_db".to_string());

    let (tx, _rx) = broadcast::channel::<BatchPayload>(1024);
    let metrics = Arc::new(Mutex::new(HashMap::new()));
    let state = RocksDbStateManager::open(&state_db_path)
        .map_err(|e| anyhow::anyhow!("failed to initialize RocksDB state backend: {e}"))?;
    let engine = Arc::new(Mutex::new(SimpleTransactionEngine::new(state)));
    let prover_backend = backend_from_env()?;

    let service = ExecutorGrpcService {
        tx,
        metrics,
        engine,
        prover_backend,
        trace_root,
    };

    info!("Executor gRPC server listening on {bind_addr}");
    info!("Using RocksDB state backend at {}", state_db_path);
    Server::builder()
        .add_service(RollupServiceServer::new(service))
        .serve(bind_addr)
        .await
        .map_err(|e| anyhow::anyhow!("executor gRPC server failed: {e}"))?;

    Ok(())
}
