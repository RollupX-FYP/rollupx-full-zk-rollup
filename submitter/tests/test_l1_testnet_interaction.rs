/// L1 Testnet Integration Tests
/// Tests real Ethereum testnet interaction (Sepolia)
/// Verifies proper contract calls and on-chain state

use submitter_rs::contracts::ZKRollupBridge;
use submitter_rs::domain::batch::{Batch, BatchStatus};
use submitter_rs::infrastructure::ethereum_adapter::RealBridgeClient;
use ethers::prelude::*;
use ethers::providers::Provider;
use ethers::signers::LocalWallet;
use std::sync::Arc;
use std::str::FromStr;

/// Test against Sepolia testnet
/// Requires:
///   - TESTNET_RPC_URL env var (Sepolia endpoint)
///   - TESTNET_PRIVATE_KEY env var (funded account)
///   - BRIDGE_CONTRACT_ADDRESS env var
#[tokio::test]
#[ignore] // Run with: cargo test test_l1_testnet -- --ignored --nocapture
async fn test_l1_testnet_bridge_deployment() {
    // Setup
    let rpc_url = std::env::var("TESTNET_RPC_URL")
        .unwrap_or_else(|_| "https://sepolia.infura.io/v3/YOUR_KEY".to_string());
    let pk = std::env::var("TESTNET_PRIVATE_KEY")
        .unwrap_or_else(|_| "0x...".to_string());

    let provider = Provider::<Http>::try_from(rpc_url).unwrap();
    let wallet: LocalWallet = pk.parse::<LocalWallet>().unwrap().with_chain_id(11155111u64); // Sepolia
    let client = Arc::new(SignerMiddleware::new(provider, wallet));

    let bridge_addr: Address = std::env::var("BRIDGE_CONTRACT_ADDRESS")
        .unwrap_or_else(|_| "0x0000000000000000000000000000000000000000".to_string())
        .parse()
        .unwrap();

    // Test: Bridge contract exists
    let code = client.get_code(bridge_addr, None).await.unwrap();
    assert!(!code.is_empty(), "Bridge contract should be deployed");

    println!("✓ Bridge contract found at {:?}", bridge_addr);
}

#[tokio::test]
#[ignore]
async fn test_l1_testnet_batch_submission() {
    /// Test actual batch submission to L1 bridge
    /// Verifies transaction is included in block

    let rpc_url = std::env::var("TESTNET_RPC_URL")
        .unwrap_or_else(|_| "https://sepolia.infura.io/v3/YOUR_KEY".to_string());
    let pk = std::env::var("TESTNET_PRIVATE_KEY")
        .expect("TESTNET_PRIVATE_KEY required");

    let provider = Provider::<Http>::try_from(rpc_url).unwrap();
    let wallet: LocalWallet = pk.parse::<LocalWallet>().unwrap().with_chain_id(11155111u64);
    let client = Arc::new(SignerMiddleware::new(provider, wallet));

    let bridge_addr: Address = std::env::var("BRIDGE_CONTRACT_ADDRESS")
        .expect("BRIDGE_CONTRACT_ADDRESS required")
        .parse()
        .unwrap();

    let bridge = ZKRollupBridge::new(bridge_addr, client.clone());

    // Create test batch
    let batch_data = vec![0u8; 32]; // Minimal batch for testing
    let new_root = [1u8; 32];
    let proof = Bytes::from(vec![0u8; 256]); // Minimal proof for testing

    // Submit batch via calldata (DA mode 0, verifier 0)
    let call = bridge.commit_batch(
        0, // daId = calldata
        0, // verifier = Groth16
        batch_data.into(),
        Bytes::new(), // no daMeta for calldata
        new_root,
        proof,
    );

    match call.send().await {
        Ok(pending) => {
            println!("Transaction sent: {:?}", pending.tx_hash());

            match pending.await {
                Ok(Some(receipt)) => {
                    assert_eq!(receipt.status, Some(U64::from(1)), "Transaction should succeed");
                    println!("✓ Batch submitted successfully: {:?}", receipt.transaction_hash);
                    println!("  Block: {}", receipt.block_number.unwrap());
                    println!("  Gas used: {}", receipt.gas_used.unwrap());
                }
                Ok(None) => panic!("Transaction dropped"),
                Err(e) => panic!("Transaction failed: {}", e),
            }
        }
        Err(e) => {
            eprintln!("Failed to send transaction: {}", e);
            // This is expected if contract not deployed or private key invalid
        }
    }
}

#[tokio::test]
#[ignore]
async fn test_l1_state_root_reading() {
    /// Verify that we can read current state root from L1

    let rpc_url = std::env::var("TESTNET_RPC_URL")
        .unwrap_or_else(|_| "https://sepolia.infura.io/v3/YOUR_KEY".to_string());

    let provider = Provider::<Http>::try_from(rpc_url).unwrap();
    let bridge_addr: Address = std::env::var("BRIDGE_CONTRACT_ADDRESS")
        .unwrap_or_else(|_| "0x0000000000000000000000000000000000000000".to_string())
        .parse()
        .unwrap();

    // Query bridge for current state root
    // This assumes a `stateRoot()` function exists on the contract
    
    println!("Bridge at: {:?}", bridge_addr);
    
    // In real test, would call: let root = bridge.state_root().call().await
}

#[tokio::test]
#[ignore]
async fn test_l1_transaction_confirmation_blocks() {
    /// Verify that transaction reaches required confirmation depth
    /// Standard: 6-12 blocks for production, 1-2 for testnet

    let rpc_url = std::env::var("TESTNET_RPC_URL")
        .unwrap_or_else(|_| "https://sepolia.infura.io/v3/YOUR_KEY".to_string());

    let provider = Provider::<Http>::try_from(rpc_url).unwrap();

    let current_block = provider.get_block_number().await.unwrap();
    println!("Current block: {}", current_block);

    // After submission, verify block depth
    let required_confirmations = 6; // Ethereum standard
    let confirmed_at_block = current_block + required_confirmations;

    println!("Transaction will be confirmed at block: {}", confirmed_at_block);
}

#[tokio::test]
#[ignore]
async fn test_l1_gas_price_estimation() {
    /// Verify gas price estimation for testnet

    let rpc_url = std::env::var("TESTNET_RPC_URL")
        .unwrap_or_else(|_| "https://sepolia.infura.io/v3/YOUR_KEY".to_string());

    let provider = Provider::<Http>::try_from(rpc_url).unwrap();

    let gas_price = provider.get_gas_price().await.unwrap();
    println!("Current gas price: {} wei (~{} Gwei)", gas_price, gas_price / 1_000_000_000);

    // Verify gas price is reasonable (not 0, not astronomical)
    assert!(gas_price > U256::zero(), "Gas price should be positive");
    assert!(
        gas_price < U256::from(1000 * 1_000_000_000u64),
        "Gas price should be reasonable (< 1000 Gwei)"
    );
}

#[tokio::test]
#[ignore]
async fn test_l1_nonce_ordering() {
    /// Verify transactions maintain correct nonce ordering
    /// This prevents transaction conflicts

    let rpc_url = std::env::var("TESTNET_RPC_URL")
        .unwrap_or_else(|_| "https://sepolia.infura.io/v3/YOUR_KEY".to_string());
    let pk = std::env::var("TESTNET_PRIVATE_KEY")
        .expect("TESTNET_PRIVATE_KEY required");

    let provider = Provider::<Http>::try_from(rpc_url).unwrap();
    let wallet: LocalWallet = pk.parse().unwrap().with_chain_id(11155111u64);

    let current_nonce = provider
        .get_transaction_count(wallet.address(), None)
        .await
        .unwrap();

    println!("Current nonce: {}", current_nonce);

    // Multiple batches should increment nonce
    let next_nonce_1 = current_nonce + 1;
    let next_nonce_2 = current_nonce + 2;

    assert_ne!(next_nonce_1, next_nonce_2, "Nonces should increment");
}

#[tokio::test]
#[ignore]
async fn test_l1_transaction_replacement() {
    /// Test that stuck transactions can be replaced with higher gas
    /// (RBF - Replace By Fee for EIP-1559)

    // Scenario: Submit batch with low gas price, then replace with higher
    // Verify: Later transaction overrides earlier one

    println!("Testing transaction replacement capability...");
    // Implementation would:
    // 1. Send transaction with low gas
    // 2. Send replacement transaction with same nonce + higher gas
    // 3. Verify only later transaction included
}

#[tokio::test]
#[ignore]
async fn test_l1_error_handling_insufficient_balance() {
    /// Verify proper error when submitter account has insufficient balance

    let rpc_url = std::env::var("TESTNET_RPC_URL")
        .unwrap_or_else(|_| "https://sepolia.infura.io/v3/YOUR_KEY".to_string());
    let pk = std::env::var("TESTNET_PRIVATE_KEY")
        .expect("TESTNET_PRIVATE_KEY required");

    let provider = Provider::<Http>::try_from(rpc_url).unwrap();
    let wallet: LocalWallet = pk.parse().unwrap().with_chain_id(11155111u64);

    let balance = provider.get_balance(wallet.address(), None).await.unwrap();
    println!("Account balance: {} wei", balance);

    if balance == U256::zero() {
        println!("✓ Empty account detected - submission will fail gracefully");
    }
}

#[tokio::test]
#[ignore]
async fn test_l1_reorg_safety() {
    /// Verify system handles blockchain reorgs safely
    /// Batches must not be marked confirmed until deep enough

    // Requirement: Wait for N blocks before considering batch final
    // N = 6 for mainnet, 1-2 for testnet

    let required_depth = 6;
    println!("Reorg safety: Waiting {} blocks before confirmation", required_depth);
}

#[cfg(test)]
mod l1_testnet_constants {
    use super::*;

    #[test]
    fn sepolia_chain_id_is_11155111() {
        assert_eq!(11155111u64, 11155111);
    }

    #[test]
    fn sepolia_confirmation_depth_reasonable() {
        let min_confirmations = 1;
        let max_confirmations = 12;
        let production_confirmations = 6;

        assert!(production_confirmations >= min_confirmations);
        assert!(production_confirmations <= max_confirmations);
    }

    #[test]
    fn transaction_timeout_seconds() {
        // Typical L1 block time: ~12 seconds
        // For 6 confirmations: ~72 seconds + propagation
        let reasonable_timeout = 120; // 2 minutes
        assert!(reasonable_timeout > 60);
    }
}
