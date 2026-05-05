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
    ResetStateRequest, ResetStateResponse, StreamRequest, SubmitTxsRequest, SubmitTxsResponse,
};
use crate::state::{RocksDbStateManager, StateManager};
use crate::trace::{append_lifecycle, persist_trace, verify_trace_hash, TraceLifecycleStatus};
use crate::tx_engine::{SimpleTransactionEngine, TransactionEngine};
use crate::types::{normalize_transactions, SequencerTransaction, StreamingStats};


#[derive(Debug, Clone, serde::Serialize)]
struct ExecutorMetrics {
    mode: String,
    execution_performed: bool,
    prover_backend: String,
    batch_count: u64,
    received_tx_count: u64,
    total_proved_txs: u64,
    duration_s: u64,
    
    execution_stats: StreamingStats,
    proof_generation_stats: StreamingStats,
    
    proof_mode_violations: u32,
    experiment_valid: bool,
    
    initial_state_hash: String,
    final_state_hash: String,
    state_was_reset: bool,
    
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
    
    execution_phases: crate::types::ExecutionPhaseBreakdown,
    prover_metrics: Option<crate::proof::ProofMetadataMetrics>,
    
    trace_write_ms: u64,
    proof_read_ms: u64,
    
    total_execution_ms: u64,
    total_proof_ms: u64,
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

    async fn reset_state(
        &self,
        _request: Request<ResetStateRequest>,
    ) -> Result<Response<ResetStateResponse>, Status> {
        let mut engine = self.engine.lock().await;
        engine.state.reset_to_genesis().map_err(|e| {
            Status::internal(format!("failed to reset state: {e}"))
        })?;
        
        Ok(Response::new(ResetStateResponse {
            success: true,
            message: "state reset to genesis".to_string(),
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

        let initial_root = {
            let engine = self.engine.lock().await;
            hex::encode(engine.state.current_root())
        };

        let trace = self
            .engine
            .lock()
            .await
            .execute_batch(&batch_id, txs)
            .map_err(|e| Status::internal(format!("execution failure: {e}")))?;
        
        let final_root = hex::encode(trace.public_inputs.final_root);

        append_lifecycle(&self.trace_root, &trace, TraceLifecycleStatus::Generated, None, None)
            .map_err(|e| Status::internal(format!("trace lifecycle(generated) failure: {e}")))?;

        let trace_write_start = Instant::now();
        let persisted = persist_trace(&self.trace_root, &trace)
            .map_err(|e| Status::internal(format!("trace persistence failure: {e}")))?;
        let trace_write_ms = trace_write_start.elapsed().as_millis() as u64;

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
        let proof_read_ms = proof_start.elapsed().as_millis() as u64;
        
        let proof_ms = artifacts.metadata.total_prover_wall_ms;
        let proof_bytes = artifacts.proof_bytes;
        let journal_bytes = artifacts.journal_bytes;
        let batch_data_len = payload.batch_data.len();

        // Fallback enforcement
        let require_real = std::env::var("REQUIRE_REAL_PROOFS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let is_violation = artifacts.metadata.proof_mode != "groth16";
        if require_real && is_violation {
            return Err(Status::failed_precondition(format!(
                "Real proof required but prover used mode: {}",
                artifacts.metadata.proof_mode
            )));
        }

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
            execution_phases: trace.execution_phases.clone(),
            prover_metrics: Some(artifacts.metadata.clone()),
            trace_write_ms,
            proof_read_ms,
            total_execution_ms: trace.execution_phases.total_execution_ms as u64,
            total_proof_ms: proof_ms,
            proof_bytes,
            journal_bytes,
        };
        let metrics_root = std::env::var("METRICS_ROOT").unwrap_or_else(|_| "metrics".to_string());
        let batch_metrics_path = std::path::PathBuf::from(&metrics_root).join("executor_batch_metrics.jsonl");
        if let Some(parent) = batch_metrics_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                warn!("Failed to create executor metrics directory {}: {e}", parent.display());
            }
        }
        match serde_json::to_string(&batch_row) {
            Ok(line) => {
                if let Err(e) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&batch_metrics_path)
                    .and_then(|mut f| writeln!(f, "{line}"))
                {
                    warn!(
                        "Failed to write executor batch metrics to {}: {e}",
                        batch_metrics_path.display()
                    );
                } else {
                    info!(
                        "Wrote executor batch metrics for batch {} to {}",
                        batch_id,
                        batch_metrics_path.display()
                    );
                }
            }
            Err(e) => warn!("Failed to serialize executor batch metrics for batch {batch_id}: {e}"),
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
                    total_proved_txs: 0,
                    duration_s: 0,
                    execution_stats: StreamingStats::default(),
                    proof_generation_stats: StreamingStats::default(),
                    proof_mode_violations: 0,
                    experiment_valid: true,
                    initial_state_hash: initial_root,
                    final_state_hash: String::new(),
                    state_was_reset: false, // Updated if reset endpoint is called
                    trace_id,
                    start_time: Instant::now(),
                });

            metrics.batch_count += 1;
            metrics.received_tx_count += proved_txs;
            metrics.total_proved_txs += proved_txs;
            metrics.execution_stats.update(trace.execution_phases.total_execution_ms);
            metrics.proof_generation_stats.update(proof_ms as f64);
            metrics.duration_s = metrics.start_time.elapsed().as_secs();
            metrics.final_state_hash = final_root;
            
            if is_violation {
                metrics.proof_mode_violations += 1;
                if require_real {
                    metrics.experiment_valid = false;
                }
            }

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
