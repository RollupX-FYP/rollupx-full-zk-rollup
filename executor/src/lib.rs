use zksync_types::{Transaction, H256};
use zksync_multivm::interface::{L1BatchEnv, SystemEnv, FinishedL1Batch};
use std::path::PathBuf;
use zksync_prover_interface::inputs::WitnessInputMerklePaths;

pub mod bridge;
pub mod executor;

/// Structured input for processing a full batch.
pub struct BatchInput {
    pub l1_batch_env: L1BatchEnv,
    pub system_env: SystemEnv,
    pub transactions: Vec<Transaction>,
    pub db_path: PathBuf,
}

/// Results produced after batch execution and tree update.
pub struct BatchOutput {
    pub root_hash: H256,
    pub pubdata: Vec<u8>,
    pub witness: Option<WitnessInputMerklePaths>,
    pub finished_batch: FinishedL1Batch,
}
