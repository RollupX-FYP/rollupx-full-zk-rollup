use crate::domain::{
    batch::{Batch, BatchId},
    errors::DomainError,
};
use async_trait::async_trait;
use ethers::types::H256;
use serde::{Deserialize, Serialize};

/// Full result returned by every DA strategy after a successful L1 submission.
#[derive(Debug, Serialize, Deserialize)]
pub struct SubmissionResult {
    pub tx_hash: String,
    pub block_number: u64,
    pub latency_ms: u64,
    pub compression_ratio: Option<f64>,
    pub compressed_bytes: Option<usize>,
    pub gas_saved: Option<u64>,
    /// EIP-1559 gas used (from receipt). Does NOT include EIP-4844 blob gas.
    pub gas_used: Option<u64>,
    /// EIP-4844 blob gas units consumed (field `blobGasUsed` in receipt).
    /// Only set for Blob DA mode; None for Calldata and Offchain.
    pub blob_gas_used: Option<u64>,
    /// Blob base fee per gas unit in wei at time of inclusion.
    /// Extracted from the block header field `blobBaseFee` (EIP-4844).
    pub blob_base_fee_wei: Option<u64>,
    /// Whether this DA strategy is a simulation/bypass (true = Mode C offchain).
    /// Must be disclosed in every result row so analysis can filter correctly.
    pub da_mode_is_simulated: bool,
}

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait BridgeReader: Send + Sync {
    /// Fetches the current state root from the L1 ZKRollupBridge contract.
    async fn state_root(&self) -> Result<H256, DomainError>;
}

#[async_trait]
pub trait Storage: Send + Sync {
    async fn save_batch(&self, batch: &Batch) -> Result<(), DomainError>;
    async fn get_batch(&self, id: BatchId) -> Result<Option<Batch>, DomainError>;
    async fn get_pending_batches(&self) -> Result<Vec<Batch>, DomainError>;
}

/// Response from the prover service.
#[derive(Debug, Serialize, Deserialize)]
pub struct ProofResponse {
    /// Serialized proof hex string.
    pub proof: String,
    /// Proof generation time reported by the prover service itself, in ms.
    /// `None` if the prover does not report this (current default).
    /// When None, the caller falls back to measuring total HTTP RTT.
    #[serde(default)]
    pub proof_generation_ms: Option<u64>,
    /// Number of constraints in the circuit witness, if reported by the prover.
    /// Useful for understanding proof time variance across batch sizes.
    #[serde(default)]
    pub witness_size: Option<usize>,
}

#[async_trait]
pub trait ProofProvider: Send + Sync {
    async fn get_proof(
        &self,
        batch_id: &BatchId,
        public_inputs: &[u8],
    ) -> Result<ProofResponse, DomainError>;
}

#[async_trait]
pub trait DaStrategy: Send + Sync {
    /// Returns the DA ID required by the contract (0 = Calldata, 1 = Blob, 2 = OffChain).
    fn da_id(&self) -> u8;

    /// Computes the commitment to be used as a Public Input.
    /// Calldata: keccak256(batch.data)
    /// Blob: batch.blob_versioned_hash
    fn compute_commitment(&self, batch: &Batch) -> Result<H256, DomainError>;

    /// Encodes the 'daMeta' bytes for the transaction.
    /// Calldata: empty bytes
    /// Blob: abi.encode(versioned_hash, blob_index)
    fn encode_da_meta(&self, batch: &Batch) -> Result<Vec<u8>, DomainError>;

    /// Broadcasts the transaction and returns the full submission result.
    async fn submit(&self, batch: &Batch, proof: &str, verifier_id: u8) -> Result<SubmissionResult, DomainError>;

    /// Checks if a transaction has been confirmed.
    async fn check_confirmation(&self, tx_hash: &str) -> Result<bool, DomainError>;
}
