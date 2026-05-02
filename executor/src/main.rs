use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    zksync_state_machine::service::run_server_from_env().await
}
