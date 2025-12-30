use crate::{
    config::Config,
    validation::Validator,
    pool::TransactionPool,
    state::StateCache,
    UserTransaction,
    SoftConfirmation,
    ConfirmationStatus,
};
use axum::{Router, routing::post, Json, extract::State};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tracing::{info, warn, error};

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    validator: Arc<Validator>,
    tx_pool: Arc<TransactionPool>,
    state_cache: StateCache,
}

pub struct Server {
    config: Config,
    state: AppState,
}

impl Server {
    pub fn new(config: Config, state_cache: StateCache) -> Self {
        let validator = Arc::new(Validator::new(state_cache.clone()));
        let tx_pool = Arc::new(TransactionPool::new());
        
        let state = AppState {
            validator,
            tx_pool,
            state_cache,
        };
        
        Self { config, state }
    }
    
    pub async fn start(self) -> anyhow::Result<()> {
        let app = Router::new()
            .route("/", post(handle_rpc))
            .with_state(self.state);
        
        let addr = format!("{}:{}", self.config.api.host, self.config.api.port);
        info!("API server listening on {}", addr);
        
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(listener, app).await?;
        
        Ok(())
    }
}

/// JSON-RPC request structure
#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    method: String,
    params: Value,
    id: Value,
}

/// JSON-RPC response structure
#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
    id: Value,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

async fn handle_rpc(
    State(state): State<AppState>,
    Json(request): Json<JsonRpcRequest>,
) -> Json<JsonRpcResponse> {
    info!("Received RPC request: {}", request.method);
    
    match request.method.as_str() {
        "sendTransaction" => handle_send_transaction(state, request).await,
        _ => Json(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code: -32601,
                message: "Method not found".to_string(),
            }),
            id: request.id,
        }),
    }
}

async fn handle_send_transaction(
    state: AppState,
    request: JsonRpcRequest,
) -> Json<JsonRpcResponse> {
    // Deserialize the transaction from params
    let tx: UserTransaction = match serde_json::from_value(request.params.clone()) {
        Ok(tx) => tx,
        Err(e) => {
            error!("Failed to deserialize transaction: {}", e);
            return Json(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                result: None,
                error: Some(JsonRpcError {
                    code: -32602,
                    message: format!("Invalid params: {}", e),
                }),
                id: request.id,
            });
        }
    };
    
    let tx_hash = tx.hash();
    info!("Processing transaction {:?} from {:?}", tx_hash, tx.from);
    
    // Validate the transaction
    match state.validator.validate(&tx).await {
        Ok(()) => {
            info!("Transaction {:?} validated successfully", tx_hash);
            
            // Update state cache: increment nonce
            state.state_cache.increment_nonce(&tx.from).await;
            
            // Add to transaction pool
            state.tx_pool.add(tx.clone()).await;
            info!("Transaction {:?} added to pool", tx_hash);
            
            // Create soft confirmation
            let confirmation = SoftConfirmation {
                tx_hash,
                status: ConfirmationStatus::Accepted,
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            };
            
            Json(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                result: Some(serde_json::to_value(confirmation).unwrap()),
                error: None,
                id: request.id,
            })
        }
        Err(validation_error) => {
            warn!(
                "Transaction {:?} validation failed: {}",
                tx_hash, validation_error
            );
            
            // Create rejection confirmation
            let confirmation = SoftConfirmation {
                tx_hash,
                status: ConfirmationStatus::Rejected {
                    reason: validation_error.to_string(),
                },
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            };
            
            Json(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                result: Some(serde_json::to_value(confirmation).unwrap()),
                error: None,
                id: request.id,
            })
        }
    }
}