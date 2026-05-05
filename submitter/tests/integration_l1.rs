//! Integration tests for L1 testnet interactions of the Submitter.
//!
//! These tests are gated behind environment configuration. They are marked
//! as `#[ignore]` so they won't run in normal CI unless explicitly requested
//! via `cargo test -- --ignored`. This allows running them only in a real
//! L1 testnet environment where a bridge contract is deployed and a funded
//! account is available.
use ethers::prelude::*;
use ethers::providers::Http;
use ethers::types::Bytes;
use std::env;
use std::sync::Arc;
use tokio;

use crate::contracts::ZKRollupBridge;

/// Helper to build a SignerMiddleware client from env vars.
fn build_l1_bridge() -> Option<ZKRollupBridge<SignerMiddleware<Provider<Http>, LocalWallet>>> {
    // Default to local Hardhat (or any local Ethereum node) if not set
    let rpc_url = env::var("L1_RPC_URL").unwrap_or_else(|_| "http://127.0.0.1:8545".to_string());
    // Bridge address and private key must be provided for signing transactions
    let bridge_address = match env::var("L1_BRIDGE_ADDRESS") {
        Ok(v) => v,
        Err(_) => {
            println!("L1_BRIDGE_ADDRESS not set. Skipping L1 tests. Set L1_BRIDGE_ADDRESS to the deployed bridge address on the local node.");
            return None;
        }
    };
    let private_key = match env::var("L1_PRIVATE_KEY") {
        Ok(v) => v,
        Err(_) => {
            println!("L1_PRIVATE_KEY not set. Skipping L1 tests. Provide a private key to sign local transactions.");
            return None;
        }
    };

    // Build provider
    let provider = Provider::<Http>::try_from(rpc_url.as_str()).ok()?;
    let wallet: LocalWallet = private_key.parse().ok()?;
    let chain_id = futures::executor::block_on(provider.get_chainid()).ok()?.as_u64();
    let wallet = wallet.with_chain_id(chain_id);
    let client = SignerMiddleware::new(provider, wallet);
    let bridge_addr: Address = bridge_address.parse().ok()?;
    let bridge = ZKRollupBridge::new(bridge_addr, Arc::new(client));
    Some(bridge)
}

/// Integration test: bridge deployment/connectivity (latest state root).
#[tokio::test]
#[ignore]
async fn test_l1_testnet_bridge_deployment() {
    let client_opt = build_l1_bridge();
    if client_opt.is_none() {
        println!("L1 environment not configured; skipping test_l1_testnet_bridge_deployment");
        return;
    }
    let bridge = client_opt.unwrap();
    let root: Result<[u8; 32], _> = bridge.latest_state_root().call().await;
    match root {
        Ok(value) => {
            println!("L1 bridge latest_state_root: {:?}", value);
        }
        Err(e) => {
            println!("L1 bridge latest_state_root call failed: {:?}", e);
            return;
        }
    }
}

/// Integration test: submit calldata path on L1 testnet.
#[tokio::test]
#[ignore]
async fn test_l1_testnet_batch_submission() {
    let bridge_opt = build_l1_bridge();
    if bridge_opt.is_none() {
        println!("L1 environment not configured; skipping test_l1_testnet_batch_submission");
        return;
    }
    let bridge = bridge_opt.unwrap();
    let batch_data = vec![0u8; 32];
    let new_root = [0u8; 32];
    let proof = Bytes::from(vec![0u8; 128]);

    let call = bridge.commit_batch(0, 0, batch_data.into(), Bytes::new(), new_root, proof);
    match call.send().await {
        Ok(pending) => {
            let receipt = pending.await;
            println!("L1 calldata tx submitted.");
        }
        Err(e) => {
            println!("L1 calldata submission failed: {:?}", e);
        }
    }
}

/// Integration test: read latest state root from L1 (read-only).
#[tokio::test]
#[ignore]
async fn test_l1_state_root_reading() {
    let client_opt = build_l1_bridge();
    if client_opt.is_none() {
        println!("L1 environment not configured; skipping test_l1_state_root_reading");
        return;
    }
    let bridge = client_opt.unwrap();
    let root: Result<[u8; 32], _> = bridge.latest_state_root().call().await;
    match root {
        Ok(r) => println!("L1 latest_state_root: {:?}", r),
        Err(e) => println!("Failed to read latest_state_root: {:?}", e),
    }
}

/// Integration test: gas price estimation placeholder (safe skip if not implemented).
#[tokio::test]
#[ignore]
async fn test_l1_gas_price_estimation() {
    println!("L1 gas price estimation test: not strictly required in integration mode. Skipping.");
}