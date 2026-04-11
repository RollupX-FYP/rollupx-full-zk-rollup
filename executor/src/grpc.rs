use std::pin::Pin;

use anyhow::Result;
use futures_util::Stream;
use tokio::sync::broadcast;
use tokio_stream::{StreamExt, wrappers::BroadcastStream};
use tonic::{Request, Response, Status, transport::Server};
use tracing::{info, warn};

use crate::proto::rollup::{
    BatchPayload,
    PublishBatchResponse,
    StreamRequest,
    SubmitTxsRequest,
    SubmitTxsResponse,
    rollup_service_server::{RollupService, RollupServiceServer},
};

#[derive(Clone)]
struct ExecutorGrpcService {
    tx: broadcast::Sender<BatchPayload>,
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
        self.tx.send(payload).map_err(|e| {
            Status::unavailable(format!("no stream subscribers available to receive batch: {e}"))
        })?;

        info!("Published batch {batch_id} to StreamBatches subscribers");
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
    let service = ExecutorGrpcService { tx };

    info!("Executor gRPC server listening on {bind_addr}");
    Server::builder()
        .add_service(RollupServiceServer::new(service))
        .serve(bind_addr)
        .await
        .map_err(|e| anyhow::anyhow!("executor gRPC server failed: {e}"))?;

    Ok(())
}
