use crate::config::Config;
use axum::{Router, routing::post, Json};
use serde_json::Value;
use tracing::info;

pub struct Server {
    config: Config,
}

impl Server {
    pub fn new(config: Config) -> Self {
        Self { config }
    }
    
    pub async fn start(self) -> anyhow::Result<()> {
        let app = Router::new()
            .route("/", post(handle_rpc));
        
        let addr = format!("{}:{}", self.config.api.host, self.config.api.port);
        info!("API server listening on {}", addr);
        
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(listener, app).await?;
        
        Ok(())
    }
}

async fn handle_rpc(Json(payload): Json<Value>) -> Json<Value> {
    // TODO: Implement JSON-RPC handling
    Json(serde_json::json!({
        "jsonrpc": "2.0",
        "result": "success",
        "id": payload["id"]
    }))
}