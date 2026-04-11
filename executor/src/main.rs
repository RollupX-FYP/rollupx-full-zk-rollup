use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    zksync_state_machine::bridge::run_from_env().await
}
