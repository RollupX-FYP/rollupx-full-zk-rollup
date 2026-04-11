use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let mode = std::env::var("EXECUTOR_MODE").unwrap_or_else(|_| "grpc".to_string());

    if mode.eq_ignore_ascii_case("bridge") {
        zksync_state_machine::bridge::run_from_env().await
    } else {
        zksync_state_machine::grpc::run_server_from_env().await
    }
}
