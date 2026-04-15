use std::pin::Pin;
use std::sync::Arc;
use std::collections::HashMap;
use std::time::Instant;

use anyhow::Result;
use futures_util::Stream;
use tokio::sync::{broadcast, Mutex};
use tokio_stream::{StreamExt, wrappers::BroadcastStream};
use tonic::{Request, Response, Status, transport::Server};
use tracing::{info, warn, error};

use crate::proto::rollup::{
    BatchPayload,
    PublishBatchResponse,
    StreamRequest,
    SubmitTxsRequest,
    SubmitTxsResponse,
    rollup_service_server::{RollupService, RollupServiceServer},
};

#[derive(Debug, Clone, serde::Serialize)]
struct ExecutorMetrics {
    mode: String,
    execution_performed: bool,
    prover_backend: String,
    batch_count: u64,
    received_tx_count: u64,
    forwarded_batch_count: u64,
    duration_s: u64,
    proof_generation_times_ms: Vec<u64>,
    total_proved_txs: u64,
    #[serde(skip)]
    start_time: Instant,
}

#[derive(Clone)]
struct ExecutorGrpcService {
    tx: broadcast::Sender<BatchPayload>,
    metrics: Arc<Mutex<HashMap<String, ExecutorMetrics>>>,
}

#[tonic::async_trait]
impl RollupService for ExecutorGrpcService {
    type StreamBatchesStream = Pin<Box<dyn Stream<Item = Result<BatchPayload, Status>> + Send + 'static>>;

    async fn stream_batches(
        &self,
        _request: Request<StreamRequest>,
    ) -> Result<Response<Self::StreamBatchesStream>, Status> {
        let rx = self.tx.subscribe();
        let stream = BroadcastStream::new(rx).filter_map(|item| {
            match item {
                Ok(payload) => Some(Ok(payload)),
                Err(e) => {
                    warn!("Dropping broadcast stream message: {e}");
                    None
                }
            }
        });

        Ok(Response::new(Box::pin(stream)))
    }

    async fn submit_transactions(
        &self,
        request: Request<SubmitTxsRequest>,
    ) -> Result<Response<SubmitTxsResponse>, Status> {
        let accepted = request.into_inner().txs.len() as u64;
        Ok(Response::new(SubmitTxsResponse { accepted }))
    }

    async fn publish_batch(
        &self,
        request: Request<BatchPayload>,
    ) -> Result<Response<PublishBatchResponse>, Status> {
        let payload = request.into_inner();
        let batch_id = payload.batch_id.clone();
        let experiment_id = payload.experiment_id.clone();
        
        let tx_count = if !payload.batch_data.is_empty() {
            serde_json::from_slice::<Vec<serde_json::Value>>(&payload.batch_data)
                .map(|v| v.len() as u64)
                .unwrap_or(0)
        } else {
            0
        };

        self.tx.send(payload).map_err(|e| {
            Status::unavailable(format!("no stream subscribers available to receive batch: {e}"))
        })?;

        info!("Published batch {batch_id} to StreamBatches subscribers");
        
        // Update metrics
        if !experiment_id.is_empty() {
            let metrics_clone = self.metrics.clone();
            tokio::spawn(async move {
                let mut metrics_guard = metrics_clone.lock().await;
                let metrics = metrics_guard.entry(experiment_id.clone()).or_insert_with(|| ExecutorMetrics {
                    mode: "grpc_passthrough".to_string(),
                    execution_performed: false,
                    prover_backend: "none".to_string(),
                    batch_count: 0,
                    received_tx_count: 0,
                    forwarded_batch_count: 0,
                    duration_s: 0,
                    proof_generation_times_ms: vec![],
                    total_proved_txs: 0,
                    start_time: Instant::now(),
                });
                
                metrics.batch_count += 1;
                metrics.received_tx_count += tx_count;
                metrics.forwarded_batch_count += 1;
                metrics.duration_s = metrics.start_time.elapsed().as_secs();
                
                // Write metrics file
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
        }

        Ok(Response::new(PublishBatchResponse {
            accepted: true,
            message: "batch accepted".to_string(),
        }))
    }
}

pub async fn run_server_from_env() -> Result<()> {
    let bind_addr = std::env::var("EXECUTOR_GRPC_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:50051".to_string())
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid EXECUTOR_GRPC_ADDR: {e}"))?;

    let (tx, _rx) = broadcast::channel::<BatchPayload>(1024);
    let metrics = Arc::new(Mutex::new(HashMap::new()));
    let service = ExecutorGrpcService { tx, metrics };

    info!("Executor gRPC server listening on {bind_addr}");
    Server::builder()
        .add_service(RollupServiceServer::new(service))
        .serve(bind_addr)
        .await
        .map_err(|e| anyhow::anyhow!("executor gRPC server failed: {e}"))?;

    Ok(())
}
