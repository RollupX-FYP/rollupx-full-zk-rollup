use ethers::types::{Address as EthAddress, Signature as EthSignature, U256};
use ethers::utils::keccak256;
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};

pub type Hash = [u8; 32];
pub type Address = [u8; 20];

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Account {
    pub balance: u64,
    pub nonce: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub from: Address,
    pub to: Address,
    pub amount: u64,
    pub nonce: u64,
    pub signature: Vec<u8>,
    pub gas_price: u64,
    pub gas_limit: u64,
    pub timestamp: u64,
    pub boost_bid: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitnessPathElement {
    pub sibling_hash: Hash,
    pub sibling_is_left: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDiff {
    pub account: Address,
    pub old_balance: u64,
    pub new_balance: u64,
    pub old_nonce: u64,
    pub new_nonce: u64,
    pub merkle_proof: Vec<Hash>,
    pub witness_path: Vec<WitnessPathElement>,
    pub leaf_encoding: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountSnapshot {
    pub address: Address,
    pub balance: u64,
    pub nonce: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxExecutionOutcome {
    pub tx_hash: Hash,
    pub included: bool,
    pub rejection_reason: Option<String>,
    pub sender_pre: AccountSnapshot,
    pub sender_post: AccountSnapshot,
    pub receiver_pre: AccountSnapshot,
    pub receiver_post: AccountSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracePublicInputs {
    pub initial_root: Hash,
    pub final_root: Hash,
    pub tx_commitment: Hash,
    pub state_diff_commitment: Hash,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProverContext {
    pub guest_method_id: String,
    pub expected_journal_hash: Hash,
    pub backend_config_fingerprint: Hash,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTraceV1 {
    pub trace_id: String,
    pub schema_version: u16,
    pub batch_id: String,
    pub created_at: u64,
    pub executor_build_id: String,
    pub public_inputs: TracePublicInputs,
    pub executed_transactions: Vec<Transaction>,
    pub tx_outcomes: Vec<TxExecutionOutcome>,
    pub state_diffs: Vec<StateDiff>,
    pub prover_context: ProverContext,
}

#[derive(Debug, thiserror::Error)]
pub enum ExecutorError {
    #[error("invalid payload: {0}")]
    InvalidPayload(String),
    #[error("state error: {0}")]
    State(String),
}

#[derive(Debug, Clone, Deserialize)]
pub enum SequencerTransaction {
    Normal(SequencerUserTransaction),
    Forced(SequencerForcedTransaction),
}

#[derive(Debug, Clone, Deserialize)]
pub struct SequencerUserTransaction {
    pub from: EthAddress,
    pub to: EthAddress,
    pub value: U256,
    pub nonce: u64,
    pub gas_price: U256,
    pub gas_limit: u64,
    #[serde(deserialize_with = "deserialize_signature")]
    pub signature: EthSignature,
    pub timestamp: u64,
    #[serde(default)]
    pub boost_bid: Option<U256>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SequencerForcedTransaction {
    pub from: EthAddress,
    pub to: EthAddress,
    pub value: U256,
    pub nonce: u64,
    pub gas_limit: u64,
    pub timestamp: u64,
}

fn deserialize_signature<'de, D>(deserializer: D) -> Result<EthSignature, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    s.parse().map_err(serde::de::Error::custom)
}

impl SequencerUserTransaction {
    pub fn hash(&self) -> Hash {
        let mut data = Vec::new();
        data.extend_from_slice(self.from.as_bytes());
        data.extend_from_slice(self.to.as_bytes());

        let mut value_bytes = [0u8; 32];
        self.value.to_big_endian(&mut value_bytes);
        data.extend_from_slice(&value_bytes);

        data.extend_from_slice(&self.nonce.to_be_bytes());

        let mut gas_price_bytes = [0u8; 32];
        self.gas_price.to_big_endian(&mut gas_price_bytes);
        data.extend_from_slice(&gas_price_bytes);

        data.extend_from_slice(&self.timestamp.to_be_bytes());

        let mut boost_bid_bytes = [0u8; 32];
        if let Some(boost_bid) = self.boost_bid {
            boost_bid.to_big_endian(&mut boost_bid_bytes);
        }
        data.extend_from_slice(&boost_bid_bytes);

        keccak256(data)
    }
}

pub fn sha256_hash(data: &[u8]) -> Hash {
    Sha256::digest(data).into()
}

pub fn tx_commitment(outcomes: &[TxExecutionOutcome]) -> Hash {
    let encoded = bincode::serialize(outcomes).unwrap_or_default();
    sha256_hash(&encoded)
}

pub fn state_diff_commitment(diffs: &[StateDiff]) -> Hash {
    let encoded = bincode::serialize(diffs).unwrap_or_default();
    sha256_hash(&encoded)
}

fn u256_to_u64_checked(value: U256, label: &str) -> Result<u64, ExecutorError> {
    if value > U256::from(u64::MAX) {
        return Err(ExecutorError::InvalidPayload(format!("{label} exceeds u64")));
    }
    Ok(value.as_u64())
}

pub fn normalize_transactions(input: Vec<SequencerTransaction>) -> Result<Vec<Transaction>, ExecutorError> {
    input
        .into_iter()
        .map(|tx| match tx {
            SequencerTransaction::Normal(n) => Ok(Transaction {
                from: n.from.0,
                to: n.to.0,
                amount: u256_to_u64_checked(n.value, "value")?,
                nonce: n.nonce,
                signature: n.signature.to_vec(),
                gas_price: u256_to_u64_checked(n.gas_price, "gas_price")?,
                gas_limit: n.gas_limit,
                timestamp: n.timestamp,
                boost_bid: n.boost_bid.map(|v| v.as_u64()).unwrap_or(0),
            }),
            SequencerTransaction::Forced(f) => Ok(Transaction {
                from: f.from.0,
                to: f.to.0,
                amount: u256_to_u64_checked(f.value, "value")?,
                nonce: f.nonce,
                signature: Vec::new(),
                gas_price: 0,
                gas_limit: f.gas_limit,
                timestamp: f.timestamp,
                boost_bid: 0,
            }),
        })
        .collect()
}
