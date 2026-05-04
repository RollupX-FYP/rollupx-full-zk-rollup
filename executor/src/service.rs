use std::collections::HashMap;
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
    rollup_service_server::{RollupService, RollupServiceServer},
    BatchPayload, PublishBatchResponse, StreamRequest, SubmitTxsRequest, SubmitTxsResponse,
};
use crate::state::RocksDbStateManager;
use crate::trace::{append_lifecycle, persist_trace, verify_trace_hash, TraceLifecycleStatus};
use crate::tx_engine::{SimpleTransactionEngine, TransactionEngine};
use crate::types::{normalize_transactions, SequencerTransaction};

#[derive(Debug, Clone, serde::Serialize)]
struct ExecutorMetrics {
    mode: String,
    execution_performed: bool,
    run_id: String,
    prover_backend: String,
    batch_count: u64,
    received_tx_count: u64,
    forwarded_batch_count: u64,
    duration_s: u64,
    execution_times_ms: Vec<u64>,
    trace_persist_times_ms: Vec<u64>,
    proof_generation_times_ms: Vec<u64>,
    total_processing_times_ms: Vec<u64>,
    sealed_to_proved_times_ms: Vec<u64>,
    sealed_to_published_times_ms: Vec<u64>,
    proof_sizes_bytes: Vec<u64>,
    journal_sizes_bytes: Vec<u64>,
    proof_modes: Vec<String>,
    total_proved_txs: u64,
    trace_id: String,
    #[serde(skip)]
    start_time: Instant,
}

#[derive(Clone)]
struct ExecutorGrpcService {
    tx: broadcast::Sender<BatchPayload>,
    metrics: Arc<Mutex<HashMap<String, ExecutorMetrics>>>,
    engine: Arc<Mutex<SimpleTransactionEngine<RocksDbStateManager>>>,
    prover_backend: ProverBackend,
    trace_root: PathBuf,
}

fn now_ms() -> u64 {
    chrono::Utc::now().timestamp_millis().max(0) as u64
}

fn metrics_run_dir(experiment_id: &str, run_id: &str) -> PathBuf {
    let root =
        PathBuf::from(std::env::var("METRICS_ROOT").unwrap_or_else(|_| "metrics".to_string()));
    if !experiment_id.is_empty() && !run_id.is_empty() {
        if root.file_name().and_then(|v| v.to_str()) == Some(run_id)
            && root
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|v| v.to_str())
                == Some(experiment_id)
        {
            return root;
        }
        return root.join(experiment_id).join(run_id);
    }
    root
}

#[tonic::async_trait]
impl RollupService for ExecutorGrpcService {
    type StreamBatchesStream =
        Pin<Box<dyn Stream<Item = Result<BatchPayload, Status>> + Send + 'static>>;

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
        let experiment_id = payload.experiment_id.clone();
        let run_id = payload.run_id.clone();
        let sealed_at_unix_ms = payload.sealed_at_unix_ms;
        let received_at_unix_ms = now_ms();

        let parsed_txs: Vec<SequencerTransaction> = serde_json::from_slice(&payload.batch_data)
            .map_err(|e| Status::invalid_argument(format!("invalid batch_data JSON: {e}")))?;
        let txs = normalize_transactions(parsed_txs)
            .map_err(|e| Status::invalid_argument(format!("invalid transaction payload: {e}")))?;

        let total_start = Instant::now();
        let execution_start = Instant::now();
        let trace = self
            .engine
            .lock()
            .await
            .execute_batch(&batch_id, txs)
            .map_err(|e| Status::internal(format!("execution failure: {e}")))?;
        let execution_ms = execution_start.elapsed().as_millis() as u64;

        append_lifecycle(
            &self.trace_root,
            &trace,
            TraceLifecycleStatus::Generated,
            None,
            None,
        )
        .map_err(|e| Status::internal(format!("trace lifecycle(generated) failure: {e}")))?;

        let persist_start = Instant::now();
        let persisted = persist_trace(&self.trace_root, &trace)
            .map_err(|e| Status::internal(format!("trace persistence failure: {e}")))?;

        verify_trace_hash(&persisted.trace_path, &persisted.sha256_hex)
            .map_err(|e| Status::internal(format!("trace integrity check failed: {e}")))?;
        let trace_persist_ms = persist_start.elapsed().as_millis() as u64;

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
        let proof_generation_ms = proof_start.elapsed().as_millis() as u64;
        let proved_at_unix_ms = now_ms();

        append_lifecycle(
            &self.trace_root,
            &trace,
            TraceLifecycleStatus::Proved,
            Some(&persisted.trace_path),
            Some(&persisted.sha256_hex),
        )
        .map_err(|e| Status::internal(format!("trace lifecycle(proved) failure: {e}")))?;

        let proved_txs = trace.executed_transactions.len() as u64;
        let proof_size_bytes = artifacts.proof_bytes as u64;
        let journal_size_bytes = artifacts.journal_bytes as u64;
        let proof_mode = artifacts.proof_mode.clone();

        let enriched =
            build_enriched_payload(payload, &trace, artifacts.da_commitment, artifacts.proof);

        self.tx.send(enriched).map_err(|e| {
            Status::unavailable(format!(
                "no stream subscribers available to receive batch: {e}"
            ))
        })?;
        let published_at_unix_ms = now_ms();
        let total_processing_ms = total_start.elapsed().as_millis() as u64;

        append_lifecycle(
            &self.trace_root,
            &trace,
            TraceLifecycleStatus::Published,
            Some(&persisted.trace_path),
            Some(&persisted.sha256_hex),
        )
        .map_err(|e| Status::internal(format!("trace lifecycle(published) failure: {e}")))?;

        info!(
            "Executed and published batch {batch_id} with {proved_txs} transactions (trace_id={})",
            trace.trace_id
        );

        if !experiment_id.is_empty() {
            let metrics_clone = self.metrics.clone();
            let prover_backend = backend_label(&self.prover_backend).to_string();
            let trace_id = trace.trace_id.clone();
            tokio::spawn(async move {
                let mut metrics_guard = metrics_clone.lock().await;
                let metrics_key = format!("{}:{}", experiment_id, run_id);
                let metrics = metrics_guard
                    .entry(metrics_key)
                    .or_insert_with(|| ExecutorMetrics {
                        mode: "grpc_executor".to_string(),
                        execution_performed: true,
                        run_id: run_id.clone(),
                        prover_backend,
                        batch_count: 0,
                        received_tx_count: 0,
                        forwarded_batch_count: 0,
                        duration_s: 0,
                        execution_times_ms: vec![],
                        trace_persist_times_ms: vec![],
                        proof_generation_times_ms: vec![],
                        total_processing_times_ms: vec![],
                        sealed_to_proved_times_ms: vec![],
                        sealed_to_published_times_ms: vec![],
                        proof_sizes_bytes: vec![],
                        journal_sizes_bytes: vec![],
                        proof_modes: vec![],
                        total_proved_txs: 0,
                        trace_id,
                        start_time: Instant::now(),
                    });

                metrics.batch_count += 1;
                metrics.received_tx_count += proved_txs;
                metrics.forwarded_batch_count += 1;
                metrics.total_proved_txs += proved_txs;
                metrics.execution_times_ms.push(execution_ms);
                metrics.trace_persist_times_ms.push(trace_persist_ms);
                metrics.proof_generation_times_ms.push(proof_generation_ms);
                metrics.total_processing_times_ms.push(total_processing_ms);
                if sealed_at_unix_ms > 0 {
                    metrics
                        .sealed_to_proved_times_ms
                        .push(proved_at_unix_ms.saturating_sub(sealed_at_unix_ms));
                    metrics
                        .sealed_to_published_times_ms
                        .push(published_at_unix_ms.saturating_sub(sealed_at_unix_ms));
                } else {
                    metrics
                        .sealed_to_proved_times_ms
                        .push(proved_at_unix_ms.saturating_sub(received_at_unix_ms));
                    metrics
                        .sealed_to_published_times_ms
                        .push(published_at_unix_ms.saturating_sub(received_at_unix_ms));
                }
                metrics.proof_sizes_bytes.push(proof_size_bytes);
                metrics.journal_sizes_bytes.push(journal_size_bytes);
                metrics.proof_modes.push(proof_mode);
                metrics.duration_s = metrics.start_time.elapsed().as_secs();

                let filename = format!("executor_{}.json", experiment_id);
                let metrics_dir = metrics_run_dir(&experiment_id, &run_id);
                let filepath = metrics_dir.join(filename);

                if let Err(e) = tokio::fs::create_dir_all(&metrics_dir).await {
                    error!(
                        "Failed to create metrics directory {}: {}",
                        metrics_dir.display(),
                        e
                    );
                    return;
                }

                if let Ok(json) = serde_json::to_string_pretty(metrics) {
                    if let Err(e) = tokio::fs::write(&filepath, json).await {
                        error!(
                            "Failed to write executor metrics to {}: {}",
                            filepath.display(),
                            e
                        );
                    }
                }
            });
        }

        Ok(Response::new(PublishBatchResponse {
            accepted: true,
            message: "batch executed and accepted".to_string(),
        }))
    }
}

pub async fn run_server_from_env() -> Result<()> {
    let bind_addr = std::env::var("EXECUTOR_GRPC_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:50051".to_string())
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid EXECUTOR_GRPC_ADDR: {e}"))?;

    let trace_root = PathBuf::from(
        std::env::var("TRACE_ROOT").unwrap_or_else(|_| "trace_artifacts".to_string()),
    );
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
