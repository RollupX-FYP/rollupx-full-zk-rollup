use zksync_types::{L1BatchNumber, Transaction, H256};
pub use zksync_vm_interface::{L1BatchEnv, SystemEnv, FinishedL1Batch};
pub use zksync_merkle_tree::domain::TreeMetadata;

#[derive(Debug, Clone)]
pub struct BatchInput {
    pub l1_batch_env: L1BatchEnv,
    pub system_env: SystemEnv,
    pub transactions: Vec<Transaction>,
}

#[derive(Debug, Clone)]
pub struct BatchOutput {
    pub batch_number: L1BatchNumber,
    pub root_hash: H256,
    pub witness: Option<zksync_prover_interface::inputs::WitnessInputMerklePaths>,
    pub pubdata: Vec<u8>,
    pub finished_batch: FinishedL1Batch,
    pub tree_metadata: TreeMetadata,
}
