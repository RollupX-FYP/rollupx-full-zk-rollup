//! API Server Module
//! 
//! This module implements a JSON-RPC server for handling transaction submissions.
//! It provides an HTTP endpoint that accepts transactions, validates them,
//! and adds them to the transaction pool if valid.

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

/// Shared application state that is accessible across all request handlers
/// 
/// This struct holds references to key components that need to be shared
/// across multiple concurrent requests:
/// - `validator`: Validates incoming transactions
/// - `tx_pool`: Stores pending transactions waiting to be batched
/// - `state_cache`: Maintains account state (balances, nonces)
#[derive(Clone)]
pub struct AppState {
    validator: Arc<Validator>,
    tx_pool: Arc<TransactionPool>,
    state_cache: StateCache,
}

/// The main API server struct
/// 
/// Encapsulates the server configuration and application state.
/// The server manages the HTTP endpoint for receiving transactions.
pub struct Server {
    config: Config,
    state: AppState,
}

impl Server {
    /// Creates a new API server instance
    /// 
    /// # Arguments
    /// * `config` - Server configuration (host, port, etc.)
    /// * `state_cache` - The state cache for account data
    /// 
    /// # Returns
    /// A new `Server` instance with initialized components
    pub fn new(config: Config, state_cache: StateCache) -> Self {
        // Initialize the transaction validator with access to state
        let validator = Arc::new(Validator::new(state_cache.clone()));
        
        // Create a new transaction pool to store pending transactions
        let tx_pool = Arc::new(TransactionPool::new());
        
        // Bundle all shared state into AppState
        let state = AppState {
            validator,
            tx_pool,
            state_cache,
        };
        
        Self { config, state }
    }
    
    /// Starts the API server and begins listening for incoming requests
    /// 
    /// This method:
    /// 1. Creates an Axum router with a single POST endpoint at "/"
    /// 2. Binds the router to the configured host and port
    /// 3. Starts serving requests asynchronously
    /// 
    /// # Returns
    /// `Ok(())` if the server starts successfully, or an error if binding fails
    pub async fn start(self) -> anyhow::Result<()> {
        // Create the router with a single POST endpoint that handles JSON-RPC requests
        let app = Router::new()
            .route("/", post(handle_rpc))
            .with_state(self.state);
        
        // Format the listening address from config
        let addr = format!("{}:{}", self.config.api.host, self.config.api.port);
        info!("API server listening on {}", addr);
        
        // Bind to the TCP address and start serving
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(listener, app).await?;
        
        Ok(())
    }
}

/// JSON-RPC 2.0 request structure
/// 
/// Represents an incoming JSON-RPC request. The structure follows the
/// JSON-RPC 2.0 specification:
/// - `jsonrpc`: Protocol version (should be "2.0")
/// - `method`: The RPC method to call (e.g., "sendTransaction")
/// - `params`: Method parameters (arbitrary JSON value)
/// - `id`: Request identifier for matching responses
#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    method: String,
    params: Value,
    id: Value,
}

/// JSON-RPC 2.0 response structure
/// 
/// Represents a JSON-RPC response sent back to the client.
/// Either `result` or `error` will be populated, but not both:
/// - `jsonrpc`: Protocol version ("2.0")
/// - `result`: Successful result (contains SoftConfirmation on success)
/// - `error`: Error information if the request failed
/// - `id`: Request identifier matching the original request
#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
    id: Value,
}

/// JSON-RPC error object
/// 
/// Contains error information when a request fails:
/// - `code`: Error code (e.g., -32601 for method not found, -32602 for invalid params)
/// - `message`: Human-readable error description
#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

/// Main RPC request handler
/// 
/// This function is called for every POST request to the "/" endpoint.
/// It routes the request to the appropriate handler based on the method name.
/// 
/// # Arguments
/// * `state` - Shared application state (injected by Axum)
/// * `request` - The JSON-RPC request
/// 
/// # Returns
/// A JSON-RPC response (either success or error)
async fn handle_rpc(
    State(state): State<AppState>,
    Json(request): Json<JsonRpcRequest>,
) -> Json<JsonRpcResponse> {
    info!("Received RPC request: {}", request.method);
    
    // Route to the appropriate handler based on the method name
    match request.method.as_str() {
        "sendTransaction" => handle_send_transaction(state, request).await,
        // Return "Method not found" error for unsupported methods
        _ => Json(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code: -32601, // Standard JSON-RPC error code for method not found
                message: "Method not found".to_string(),
            }),
            id: request.id,
        }),
    }
}

/// Handles the "sendTransaction" RPC method
/// 
/// This function:
/// 1. Deserializes the transaction from the request parameters
/// 2. Validates the transaction (signature, nonce, balance)
/// 3. If valid: adds to the pool and returns a soft confirmation
/// 4. If invalid: returns a rejection confirmation with the reason
/// 
/// # Arguments
/// * `state` - Shared application state
/// * `request` - The JSON-RPC request containing the transaction
/// 
/// # Returns
/// A JSON-RPC response containing a SoftConfirmation (accepted or rejected)
async fn handle_send_transaction(
    state: AppState,
    request: JsonRpcRequest,
) -> Json<JsonRpcResponse> {
    // Step 1: Deserialize the transaction from the request parameters
    let tx: UserTransaction = match serde_json::from_value(request.params.clone()) {
        Ok(tx) => tx,
        Err(e) => {
            error!("Failed to deserialize transaction: {}", e);
            // Return invalid params error if deserialization fails
            return Json(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                result: None,
                error: Some(JsonRpcError {
                    code: -32602, // Standard JSON-RPC error code for invalid params
                    message: format!("Invalid params: {}", e),
                }),
                id: request.id,
            });
        }
    };
    
    // Compute the transaction hash for logging and tracking
    let tx_hash = tx.hash();
    info!("Processing transaction {:?} from {:?}", tx_hash, tx.from);
    
    // Step 2: Validate the transaction (signature, nonce, balance)
    match state.validator.validate(&tx).await {
        // Validation succeeded - process the transaction
        Ok(()) => {
            info!("Transaction {:?} validated successfully", tx_hash);
            
            // Step 3: Update state cache to reflect the new nonce
            // This prevents nonce reuse attacks and ensures sequential ordering
            state.state_cache.increment_nonce(&tx.from).await;
            
            // Step 4: Add the transaction to the pool for batching
            state.tx_pool.add(tx.clone()).await;
            info!("Transaction {:?} added to pool", tx_hash);
            
            // Step 5: Create a soft confirmation to send back to the client
            // This gives the user immediate feedback that their transaction was accepted
            let confirmation = SoftConfirmation {
                tx_hash,
                status: ConfirmationStatus::Accepted,
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            };
            
            // Return the soft confirmation as a successful result
            Json(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                result: Some(serde_json::to_value(confirmation).unwrap()),
                error: None,
                id: request.id,
            })
        }
        // Validation failed - reject the transaction
        Err(validation_error) => {
            warn!(
                "Transaction {:?} validation failed: {}",
                tx_hash, validation_error
            );
            
            // Create a rejection confirmation with the failure reason
            // This informs the user why their transaction was rejected
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
            
            // Return the rejection confirmation as a successful response
            // Note: This is still a successful JSON-RPC call, but the confirmation indicates rejection
            Json(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                result: Some(serde_json::to_value(confirmation).unwrap()),
                error: None,
                id: request.id,
            })
        }
    }
}