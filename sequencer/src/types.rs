use ethers::types::{Address, U256, Signature, H256};
use serde::{Deserialize, Serialize};

/// User transaction submitted to L2
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserTransaction {
    pub from: Address,
    pub to: Address,
    pub value: U256,
    pub nonce: u64,
    pub gas_price: U256,
    pub signature: Signature,
    pub timestamp: u64,
}

/// Forced transaction from L1
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForcedTransaction {
    pub tx_hash: H256,
    pub from: Address,
    pub to: Address,
    pub value: U256,
    pub nonce: u64,
    pub l1_tx_hash: H256,
    pub l1_block_number: u64,
    pub event_type: ForcedEventType,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ForcedEventType {
    Deposit,
    ForcedExit,
}

/// Generic transaction (can be normal or forced)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Transaction {
    Normal(UserTransaction),
    Forced(ForcedTransaction),
}

/// Account state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountState {
    pub address: Address,
    pub balance: U256,
    pub nonce: u64,
}

/// Sealed batch ready for execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Batch {
    pub batch_id: u64,
    pub transactions: Vec<Transaction>,
    pub prev_state_root: H256,
    pub timestamp: u64,
}

/// Batch metadata for registry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchMetadata {
    pub batch_id: u64,
    pub tx_count: usize,
    pub forced_tx_count: usize,
    pub timestamp: u64,
    pub scheduling_policy: String,
}