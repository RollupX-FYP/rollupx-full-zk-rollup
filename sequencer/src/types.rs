//! Type Definitions Module
//!
//! This module contains all the core data structures used throughout the sequencer:
//! - Transaction types (normal user transactions and forced L1 transactions)
//! - Account state representation
//! - Batch structures for transaction batching
//! - Validation error types
//! - Soft confirmation responses

use ethers::types::{Address, H256, Signature, U256};
use ethers::utils::keccak256;
use serde::{Deserialize, Deserializer, Serialize};
use std::str::FromStr;

/// User transaction submitted to L2
///
/// Represents a standard transaction submitted by users through the RPC API.
/// These transactions go through validation before being added to the pool.
///
/// # Fields
/// - `from`: Sender's address
/// - `to`: Recipient's address
/// - `value`: Amount to transfer (in wei)
/// - `nonce`: Transaction sequence number (prevents replay attacks)
/// - `gas_price`: Price per unit of gas (determines transaction fee)
/// - `gas_limit`: Maximum gas units this transaction can consume
/// - `signature`: ECDSA signature proving transaction authenticity
/// - `timestamp`: When the transaction was created
/// - `boost_bid`: Optional premium bid for Time-Boost scheduling policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserTransaction {
    pub from: Address,
    pub to: Address,
    pub value: U256,
    pub nonce: u64,
    pub gas_price: U256,
    pub gas_limit: u64,
    #[serde(deserialize_with = "deserialize_signature")]
    pub signature: Signature,
    pub timestamp: u64,
    /// Optional premium bid for Time-Boost policy (faster confirmation)
    #[serde(default)]
    pub boost_bid: Option<U256>,
    /// Optional benchmark/application calldata payload encoded as a hex string.
    ///
    /// This is intentionally not part of the signature hash in the current
    /// prototype because the benchmark signing helper signs the legacy fields.
    /// It is preserved so DA-size and blob-packing diagnostics reflect the
    /// workload's Type A/B/C payload sizes instead of silently dropping them.
    #[serde(default)]
    pub calldata: Option<String>,
}

fn deserialize_signature<'de, D>(deserializer: D) -> Result<Signature, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    Signature::from_str(&s).map_err(serde::de::Error::custom)
}

impl UserTransaction {
    /// Compute the hash of the transaction for signature verification
    ///
    /// This hash is used to:
    /// 1. Verify the ECDSA signature
    /// 2. Uniquely identify the transaction
    ///
    /// The hash is computed by concatenating all transaction fields and
    /// applying Keccak256 (the same hash function used in Ethereum).
    ///
    /// # Note
    /// In production, this should follow EIP-712 or similar standard for
    /// structured data hashing to improve security and user experience.
    ///
    /// # Returns
    /// A 32-byte hash (H256) uniquely identifying this transaction
    pub fn hash(&self) -> H256 {
        // Encode all transaction fields into a byte array
        let mut data = Vec::new();

        // Add sender address (20 bytes)
        data.extend_from_slice(self.from.as_bytes());

        // Add recipient address (20 bytes)
        data.extend_from_slice(self.to.as_bytes());

        // Convert value to big-endian bytes (32 bytes)
        let mut value_bytes = [0u8; 32];
        self.value.to_big_endian(&mut value_bytes);
        data.extend_from_slice(&value_bytes);

        // Add nonce as big-endian bytes (8 bytes)
        data.extend_from_slice(&self.nonce.to_be_bytes());

        // Convert gas_price to big-endian bytes (32 bytes)
        let mut gas_price_bytes = [0u8; 32];
        self.gas_price.to_big_endian(&mut gas_price_bytes);
        data.extend_from_slice(&gas_price_bytes);

        // Add timestamp as big-endian bytes (8 bytes)
        data.extend_from_slice(&self.timestamp.to_be_bytes());

        // Add boost_bid if present (32 bytes, or zeros if None)
        let mut boost_bid_bytes = [0u8; 32];
        if let Some(boost_bid) = self.boost_bid {
            boost_bid.to_big_endian(&mut boost_bid_bytes);
        }
        data.extend_from_slice(&boost_bid_bytes);

        // Apply Keccak256 hash and return as H256
        H256::from_slice(&keccak256(data))
    }
}

/// Forced transaction from L1
///
/// Represents a transaction that was submitted on Layer 1 (Ethereum mainnet)
/// and must be included in the sequencer. These transactions bypass the normal
/// validation and scheduling process.
///
/// # Use Cases
/// - **Deposits**: Users deposit funds from L1 to L2
/// - **Forced Exits**: Users withdraw funds if the sequencer is censoring them
///
/// # Fields
/// - `tx_hash`: Hash of this forced transaction
/// - `from`: Sender's address
/// - `to`: Recipient's address
/// - `value`: Amount to transfer
/// - `nonce`: Transaction sequence number
/// - `gas_limit`: Maximum gas units this transaction can consume
/// - `l1_tx_hash`: Hash of the originating L1 transaction
/// - `l1_block_number`: L1 block where the event was emitted
/// - `event_type`: Type of forced transaction (Deposit or ForcedExit)
/// - `timestamp`: When the L1 event was detected
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForcedTransaction {
    pub tx_hash: H256,
    pub from: Address,
    pub to: Address,
    pub value: U256,
    pub nonce: u64,
    pub gas_limit: u64,
    pub l1_tx_hash: H256,
    pub l1_block_number: u64,
    pub event_type: ForcedEventType,
    pub timestamp: u64,
}

/// Type of forced transaction event from L1
///
/// Distinguishes between different types of L1-originated transactions:
/// - `Deposit`: User is depositing funds from L1 to L2
/// - `ForcedExit`: User is forcing a withdrawal (censorship resistance)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ForcedEventType {
    /// User depositing funds from L1 to their L2 account
    Deposit,
    /// User forcing a withdrawal from L2 to L1 (anti-censorship mechanism)
    ForcedExit,
}

/// Normal transaction with pooling metadata
///
/// Wraps a UserTransaction with additional timestamps and latency info
/// recorded during the ingestion process. This is used for precise
/// performance and fairness measurements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PooledTransaction {
    pub tx: UserTransaction,
    /// Unix epoch ms when the transaction hit the API server
    pub arrived_at: u64,
    /// Unix epoch ms when the transaction was added to the pool after validation
    pub pool_entry_at: u64,
    /// Time spent in validation logic (ms)
    pub validation_latency_ms: u64,
}

impl PooledTransaction {
    pub fn new(tx: UserTransaction, arrived_at: u64, pool_entry_at: u64) -> Self {
        Self {
            tx,
            arrived_at,
            pool_entry_at,
            validation_latency_ms: pool_entry_at.saturating_sub(arrived_at),
        }
    }

    /// Approximate serialized size used by blob-aware packing heuristics.
    pub fn estimated_encoded_bytes(&self) -> usize {
        fn u256_digits(value: &U256) -> usize {
            value.to_string().len()
        }
        fn hex_payload_bytes(value: &str) -> usize {
            let hex = value.strip_prefix("0x").unwrap_or(value);
            if hex.len() % 2 == 0 && hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                hex.len() / 2
            } else {
                value.len()
            }
        }

        let mut size = 20 + 20 + 32 + 8 + 32 + 8 + 65;
        size += u256_digits(&self.tx.value);
        size += u256_digits(&self.tx.gas_price);
        size += self.tx.nonce.to_string().len();
        size += self.tx.gas_limit.to_string().len();
        size += self.tx.timestamp.to_string().len();
        size += self.tx.boost_bid.as_ref().map(u256_digits).unwrap_or(4);
        size += self
            .tx
            .calldata
            .as_deref()
            .map(hex_payload_bytes)
            .unwrap_or(0);
        size
    }
}

/// Generic transaction (can be normal or forced)
///
/// A unified type that can represent either:
/// - Normal user transactions submitted via the RPC API (with metadata)
/// - Forced transactions originating from L1
///
/// This enum allows batches to contain a mix of both transaction types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Transaction {
    /// Standard user transaction from the RPC API with pooling metadata
    Normal(PooledTransaction),
    /// Forced transaction from L1 (deposit or forced exit)
    Forced(ForcedTransaction),
}

impl Transaction {
    /// Get the gas limit for this transaction
    ///
    /// Returns the gas limit regardless of whether this is a normal or forced transaction.
    /// Used for cumulative gas tracking in batch creation.
    pub fn gas_limit(&self) -> u64 {
        match self {
            Transaction::Normal(ptx) => ptx.tx.gas_limit,
            Transaction::Forced(tx) => tx.gas_limit,
        }
    }

    /// Get the arrival timestamp in milliseconds
    pub fn arrival_timestamp_ms(&self) -> u64 {
        match self {
            Transaction::Normal(ptx) => ptx.arrived_at,
            Transaction::Forced(tx) => tx.timestamp.saturating_mul(1000), // L1 ts is usually seconds
        }
    }

    /// Approximate serialized size used by metrics and blob packing.
    pub fn estimated_encoded_bytes(&self) -> usize {
        match self {
            Transaction::Normal(ptx) => ptx.estimated_encoded_bytes(),
            Transaction::Forced(tx) => {
                let mut size = 20 + 20 + 32 + 8 + 32 + 8 + 8;
                size += tx.value.to_string().len();
                size += tx.nonce.to_string().len();
                size += tx.gas_limit.to_string().len();
                size += tx.l1_block_number.to_string().len();
                size += tx.timestamp.to_string().len();
                size
            }
        }
    }
}

/// Wait time distribution metrics for a batch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitTimeDistribution {
    pub p50_wait_ms: u64,
    pub p95_wait_ms: u64,
    pub p99_wait_ms: u64,
    pub max_wait_ms: u64,
    pub min_wait_ms: u64,
    pub mean_wait_ms: f64,
    pub std_dev_wait_ms: f64,
    /// Jain's fairness index across all tx wait times in the batch (0.0 to 1.0)
    pub jains_fairness_index: f64,
}

/// MEV-related metrics for a batch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MevMetrics {
    pub actual_batch_fee_wei: U256,
    pub optimal_batch_fee_wei: U256,
    /// actual / optimal - 1.0 means max revenue captured
    pub ordering_efficiency: f64,
    pub reordering_events: u32,
    pub max_reorder_distance: usize,
}

/// Mempool state snapshot at batch seal time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolSnapshot {
    pub depth_at_seal: usize,
    pub depth_after_seal: usize,
    pub forced_queue_depth: usize,
    pub pool_growth_rate_tps: f64,
    pub time_since_last_seal_ms: u64,
}

/// State cache diagnostic metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateCacheMetrics {
    pub cache_hit_rate: f64,
    pub stale_nonce_rejections: u32,
    pub balance_check_failures: u32,
    pub cache_age_ms: u64,
}

/// Account state
///
/// Represents the current state of an account in the sequencer.
/// This is cached in memory for fast validation of incoming transactions.
///
/// # Fields
/// - `address`: The account's Ethereum address
/// - `balance`: Current balance in wei
/// - `nonce`: Current nonce (number of transactions sent by this account)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountState {
    pub address: Address,
    pub balance: U256,
    pub nonce: u64,
}

/// Sealed batch ready for execution
///
/// A batch is a collection of transactions that will be executed together
/// and posted to L1 as a single unit. Batching reduces L1 costs.
///
/// # Fields
/// - `batch_id`: Unique identifier for this batch (sequential)
/// - `transactions`: All transactions in this batch (normal + forced)
/// - `prev_state_root`: State root hash before this batch (for verification)
/// - `timestamp`: When this batch was sealed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Batch {
    pub batch_id: u64,
    pub transactions: Vec<Transaction>,
    pub prev_state_root: H256,
    pub timestamp: u64,
}

/// Batch metadata for registry
///
/// Lightweight metadata about a batch, stored in the database registry.
/// This allows querying batch information without loading full transaction data.
///
/// # Fields
/// - `batch_id`: Unique identifier for this batch
/// - `tx_count`: Total number of transactions (normal + forced)
/// - `forced_tx_count`: Number of forced transactions from L1
/// - `timestamp`: When the batch was created
/// - `scheduling_policy`: Which policy was used ("FCFS" or "FeePriority")
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchMetadata {
    pub batch_id: u64,
    pub tx_count: usize,
    pub forced_tx_count: usize,
    pub timestamp: u64,
    pub scheduling_policy: String,
}

/// Validation errors
///
/// Enumeration of all possible transaction validation failures.
/// Each variant contains contextual information to help diagnose the issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationError {
    /// Signature verification failed (transaction may be forged)
    InvalidSignature,
    /// Nonce doesn't match expected value (transaction out of order)
    InvalidNonce { expected: u64, got: u64 },
    /// Account doesn't have enough funds for value + gas fees
    InsufficientBalance { required: U256, available: U256 },
}

/// Implements Display trait for user-friendly error messages
impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::InvalidSignature => write!(f, "Invalid transaction signature"),
            ValidationError::InvalidNonce { expected, got } => {
                write!(f, "Invalid nonce: expected {}, got {}", expected, got)
            }
            ValidationError::InsufficientBalance {
                required,
                available,
            } => {
                write!(
                    f,
                    "Insufficient balance: required {}, available {}",
                    required, available
                )
            }
        }
    }
}

/// Implements Error trait so ValidationError can be used with anyhow and other error handling
impl std::error::Error for ValidationError {}

/// Soft confirmation sent to users after validation
///
/// Provides immediate feedback to users after they submit a transaction.
/// This is called a "soft" confirmation because the transaction hasn't been
/// executed yet - it's just been accepted into the pool.
///
/// # Fields
/// - `tx_hash`: Hash identifying the transaction
/// - `status`: Whether the transaction was accepted or rejected
/// - `timestamp`: When the confirmation was generated
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftConfirmation {
    pub tx_hash: H256,
    pub status: ConfirmationStatus,
    pub timestamp: u64,
}

/// Status of a soft confirmation
///
/// Indicates whether a transaction passed validation and was accepted,
/// or failed validation and was rejected.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfirmationStatus {
    /// Transaction passed validation and was added to the pool
    Accepted,
    /// Transaction failed validation (includes reason for rejection)
    Rejected { reason: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pooled_transaction_latency() {
        let tx = UserTransaction {
            from: Address::random(),
            to: Address::random(),
            value: U256::from(100),
            nonce: 1,
            gas_limit: 21000,
            gas_price: U256::from(10),
            signature: Signature {
                r: U256::zero(),
                s: U256::zero(),
                v: 0,
            },
            timestamp: 1000,
            boost_bid: None,
            calldata: None,
        };

        let arrived_at = 1000;
        let pool_entry_at = 1050;
        let ptx = PooledTransaction::new(tx, arrived_at, pool_entry_at);

        assert_eq!(ptx.arrived_at, 1000);
        assert_eq!(ptx.pool_entry_at, 1050);
        assert_eq!(ptx.validation_latency_ms, 50);
    }

    #[test]
    fn test_estimated_encoded_bytes_includes_calldata_payload() {
        let base_tx = UserTransaction {
            from: Address::random(),
            to: Address::random(),
            value: U256::from(100),
            nonce: 1,
            gas_limit: 21000,
            gas_price: U256::from(10),
            signature: Signature {
                r: U256::zero(),
                s: U256::zero(),
                v: 0,
            },
            timestamp: 1000,
            boost_bid: None,
            calldata: None,
        };
        let mut payload_tx = base_tx.clone();
        payload_tx.calldata = Some(format!("0x{}", "00".repeat(128)));

        let base = PooledTransaction::new(base_tx, 1000, 1001);
        let with_payload = PooledTransaction::new(payload_tx, 1000, 1001);

        assert_eq!(
            with_payload.estimated_encoded_bytes() - base.estimated_encoded_bytes(),
            128
        );
    }

    #[test]
    fn test_transaction_arrival_timestamp_ms() {
        let tx = UserTransaction {
            from: Address::random(),
            to: Address::random(),
            value: U256::from(100),
            nonce: 1,
            gas_limit: 21000,
            gas_price: U256::from(10),
            signature: Signature {
                r: U256::zero(),
                s: U256::zero(),
                v: 0,
            },
            timestamp: 1234,
            boost_bid: None,
            calldata: None,
        };
        let ptx = PooledTransaction::new(tx, 1234, 1235);
        let normal = Transaction::Normal(ptx);
        assert_eq!(normal.arrival_timestamp_ms(), 1234);

        let forced = Transaction::Forced(ForcedTransaction {
            tx_hash: H256::zero(),
            from: Address::random(),
            to: Address::random(),
            value: U256::from(500),
            nonce: 0,
            gas_limit: 100000,
            l1_tx_hash: H256::zero(),
            l1_block_number: 1,
            event_type: ForcedEventType::Deposit,
            timestamp: 1700000000, // seconds
        });
        assert_eq!(forced.arrival_timestamp_ms(), 1700000000000); // ms
    }
}
