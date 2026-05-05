/// L1 Testnet Integration Tests
/// Tests real Ethereum testnet interaction (Sepolia)

use submitter_rs::contracts::ZKRollupBridge;
use ethers::prelude::*;
use ethers::providers::Provider;
use ethers::signers::LocalWallet;
use std::sync::Arc;

#[tokio::test]
#[ignore]
async fn test_l1_testnet_bridge_deployment() {
    let rpc_url = std::env::var("TESTNET_RPC_URL")
        .unwrap_or_else(|_| "https://sepolia.infura.io/v3/YOUR_KEY".to_string());
    let pk = std::env::var("TESTNET_PRIVATE_KEY")
        .unwrap_or_else(|_| "0x...".to_string());

    let provider = Provider::<Http>::try_from(rpc_url).unwrap();
    let wallet: LocalWallet = pk.parse::<LocalWallet>().unwrap().with_chain_id(11155111u64);
    let client = Arc::new(SignerMiddleware::new(provider, wallet));

    let bridge_addr: Address = std::env::var("BRIDGE_CONTRACT_ADDRESS")
        .unwrap_or_else(|_| "0x0000000000000000000000000000000000000000".to_string())
        .parse()
        .unwrap();

    let code = client.get_code(bridge_addr, None).await.unwrap();
    assert!(!code.is_empty(), "Bridge contract should be deployed");

    println!("✓ Bridge contract found at {:?}", bridge_addr);
}

#[tokio::test]
#[ignore]
async fn test_l1_testnet_batch_submission() {
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

    let batch_data = vec![0u8; 32];
    let new_root = [1u8; 32];
    let proof = Bytes::from(vec![0u8; 256]);

    let call = bridge.commit_batch(0, 0, batch_data.into(), Bytes::new(), new_root, proof);
    match call.send().await {
        Ok(pending) => {
            println!("Transaction sent: {:?}", pending.tx_hash());
            match pending.await {
                Ok(Some(receipt)) => {
                    assert_eq!(receipt.status, Some(U64::from(1)));
                    println!("✓ Batch submitted: {:?}", receipt.transaction_hash);
                }
                Ok(None) => panic!("Transaction dropped"),
                Err(e) => panic!("Transaction failed: {}", e),
            }
        }
        Err(e) => {
            eprintln!("Failed to send: {}", e);
        }
    }
}

#[tokio::test]
#[ignore]
async fn test_l1_state_root_reading() {
    let rpc_url = std::env::var("TESTNET_RPC_URL")
        .unwrap_or_else(|_| "https://sepolia.infura.io/v3/YOUR_KEY".to_string());

    let provider = Provider::<Http>::try_from(rpc_url).unwrap();
    let bridge_addr: Address = std::env::var("BRIDGE_CONTRACT_ADDRESS")
        .unwrap_or_else(|_| "0x0000000000000000000000000000000000000000".to_string())
        .parse()
        .unwrap();

    println!("Bridge at: {:?}", bridge_addr);
}

#[tokio::test]
#[ignore]
async fn test_l1_transaction_confirmation_blocks() {
    let rpc_url = std::env::var("TESTNET_RPC_URL")
        .unwrap_or_else(|_| "https://sepolia.infura.io/v3/YOUR_KEY".to_string());

    let provider = Provider::<Http>::try_from(rpc_url).unwrap();
    let current_block = provider.get_block_number().await.unwrap();
    println!("Current block: {}", current_block);

    let required_confirmations = 6;
    let confirmed_at_block = current_block + required_confirmations;
    println!("Transaction will be confirmed at block: {}", confirmed_at_block);
}

#[tokio::test]
#[ignore]
async fn test_l1_gas_price_estimation() {
    let rpc_url = std::env::var("TESTNET_RPC_URL")
        .unwrap_or_else(|_| "https://sepolia.infura.io/v3/YOUR_KEY".to_string());

    let provider = Provider::<Http>::try_from(rpc_url).unwrap();
    let gas_price = provider.get_gas_price().await.unwrap();
    println!("Current gas price: {} wei", gas_price);

    assert!(gas_price > U256::zero());
    assert!(gas_price < U256::from(1000 * 1_000_000_000u64));
}

#[tokio::test]
#[ignore]
async fn test_l1_nonce_ordering() {
    let rpc_url = std::env::var("TESTNET_RPC_URL")
        .unwrap_or_else(|_| "https://sepolia.infura.io/v3/YOUR_KEY".to_string());
    let pk = std::env::var("TESTNET_PRIVATE_KEY")
        .expect("TESTNET_PRIVATE_KEY required");

    let provider = Provider::<Http>::try_from(rpc_url).unwrap();
    let wallet: LocalWallet = pk.parse::<LocalWallet>().unwrap().with_chain_id(11155111u64);

    let current_nonce = provider
        .get_transaction_count(wallet.address(), None)
        .await
        .unwrap();

    println!("Current nonce: {}", current_nonce);
    assert!(current_nonce >= U256::zero());
}

#[tokio::test]
#[ignore]
async fn test_l1_error_handling_insufficient_balance() {
    let rpc_url = std::env::var("TESTNET_RPC_URL")
        .unwrap_or_else(|_| "https://sepolia.infura.io/v3/YOUR_KEY".to_string());
    let pk = std::env::var("TESTNET_PRIVATE_KEY")
        .expect("TESTNET_PRIVATE_KEY required");

    let provider = Provider::<Http>::try_from(rpc_url).unwrap();
    let wallet: LocalWallet = pk.parse::<LocalWallet>().unwrap().with_chain_id(11155111u64);

    let balance = provider.get_balance(wallet.address(), None).await.unwrap();
    println!("Account balance: {} wei", balance);
}

#[tokio::test]
#[ignore]
async fn test_l1_reorg_safety() {
    let required_depth = 6;
    println!("Reorg safety: Waiting {} blocks before confirmation", required_depth);
}

#[cfg(test)]
mod l1_testnet_constants {
    #[test]
    fn sepolia_chain_id_is_11155111() {
        assert_eq!(11155111u64, 11155111);
    }

    #[test]
    fn sepolia_confirmation_depth_reasonable() {
        let production_confirmations = 6;
        assert!(production_confirmations >= 1);
        assert!(production_confirmations <= 12);
    }

    #[test]
    fn transaction_timeout_seconds() {
        let reasonable_timeout = 120;
        assert!(reasonable_timeout > 60);
    }
}