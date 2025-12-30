use ethers::types::{Address, U256, Signature, H256};
use ethers::utils::keccak256;
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

impl UserTransaction {
    /// Compute the hash of the transaction for signature verification
    pub fn hash(&self) -> H256 {
        // Encode transaction fields for hashing
        // Note: In production, this should follow EIP-712 or similar standard
        let mut data = Vec::new();
        data.extend_from_slice(self.from.as_bytes());
        data.extend_from_slice(self.to.as_bytes());
        
        // Convert U256 to bytes (32 bytes)
        let mut value_bytes = [0u8; 32];
        self.value.to_big_endian(&mut value_bytes);
        data.extend_from_slice(&value_bytes);
        
        data.extend_from_slice(&self.nonce.to_be_bytes());
        
        let mut gas_price_bytes = [0u8; 32];
        self.gas_price.to_big_endian(&mut gas_price_bytes);
        data.extend_from_slice(&gas_price_bytes);
        
        data.extend_from_slice(&self.timestamp.to_be_bytes());
        
        H256::from_slice(&keccak256(data))
    }
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

/// Validation errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationError {
    InvalidSignature,
    InvalidNonce { expected: u64, got: u64 },
    InsufficientBalance { required: U256, available: U256 },
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::InvalidSignature => write!(f, "Invalid transaction signature"),
            ValidationError::InvalidNonce { expected, got } => {
                write!(f, "Invalid nonce: expected {}, got {}", expected, got)
            }
            ValidationError::InsufficientBalance { required, available } => {
                write!(f, "Insufficient balance: required {}, available {}", required, available)
            }
        }
    }
}

impl std::error::Error for ValidationError {}

/// Soft confirmation sent to users after validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftConfirmation {
    pub tx_hash: H256,
    pub status: ConfirmationStatus,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfirmationStatus {
    Accepted,
    Rejected { reason: String },
}