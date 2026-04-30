use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub type Hash = [u8; 32];
pub type Address = [u8; 20];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDiff {
    pub account: Address,
    pub old_balance: u64,
    pub new_balance: u64,
    pub old_nonce: u64,
    pub new_nonce: u64,
    pub merkle_proof: Vec<Hash>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockTrace {
    pub batch_id: String,
    pub initial_root: Hash,
    pub final_root: Hash,
    pub state_diffs: Vec<StateDiff>,
}

#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    #[error("invalid old balance")]
    InvalidOldBalance,
    #[error("invalid old nonce")]
    InvalidOldNonce,
    #[error("balance increased while nonce increased")]
    InvalidDebit,
    #[error("invalid witness")]
    InvalidWitness,
}

pub struct LightweightSMT {
    root: Hash,
}

impl LightweightSMT {
    pub fn new(root: Hash) -> Self {
        Self { root }
    }

    pub fn apply_diff(&mut self, diff: &StateDiff) -> Result<(), VerifyError> {
        if diff.new_nonce < diff.old_nonce {
            return Err(VerifyError::InvalidOldNonce);
        }
        if diff.new_nonce > diff.old_nonce && diff.new_balance > diff.old_balance {
            return Err(VerifyError::InvalidDebit);
        }

        if let Some(expected) = diff.merkle_proof.first() {
            if expected != &self.root {
                return Err(VerifyError::InvalidWitness);
            }
        } else {
            return Err(VerifyError::InvalidWitness);
        }

        self.root = fold_diff(self.root, diff);
        Ok(())
    }

    pub fn current_root(&self) -> Hash {
        self.root
    }
}

fn fold_diff(prev_root: Hash, diff: &StateDiff) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(prev_root);
    hasher.update(diff.account);
    hasher.update(diff.old_balance.to_be_bytes());
    hasher.update(diff.new_balance.to_be_bytes());
    hasher.update(diff.old_nonce.to_be_bytes());
    hasher.update(diff.new_nonce.to_be_bytes());
    hasher.finalize().into()
}
