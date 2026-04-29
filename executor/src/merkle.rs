use crate::types::{Account, Address, Hash, StateDiff, WitnessPathElement};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub trait MerkleTree: Send + Sync {
    fn root(&self, accounts: &BTreeMap<Address, Account>) -> Hash;
    fn prove_account(&self, accounts: &BTreeMap<Address, Account>, address: &Address) -> Vec<Hash>;
}

#[derive(Default)]
pub struct Sha256SparseMerkle;

impl MerkleTree for Sha256SparseMerkle {
    fn root(&self, accounts: &BTreeMap<Address, Account>) -> Hash {
        compute_root(accounts)
    }

    fn prove_account(&self, accounts: &BTreeMap<Address, Account>, address: &Address) -> Vec<Hash> {
        let mut hasher = Sha256::new();
        hasher.update(compute_root(accounts));
        hasher.update(address);
        [hasher.finalize().into()]
    }
}

pub fn compute_root(accounts: &BTreeMap<Address, Account>) -> Hash {
    let mut leaf_hashes: Vec<Hash> = accounts
        .iter()
        .map(|(addr, account)| {
            let mut hasher = Sha256::new();
            hasher.update(addr);
            hasher.update(account.balance.to_be_bytes());
            hasher.update(account.nonce.to_be_bytes());
            hasher.finalize().into()
        })
        .collect();

    if leaf_hashes.is_empty() {
        return [0u8; 32];
    }

    while leaf_hashes.len() > 1 {
        let mut next = Vec::with_capacity((leaf_hashes.len() + 1) / 2);
        let mut i = 0usize;
        while i < leaf_hashes.len() {
            let left = leaf_hashes[i];
            let right = if i + 1 < leaf_hashes.len() { leaf_hashes[i + 1] } else { left };
            let mut hasher = Sha256::new();
            hasher.update(left);
            hasher.update(right);
            next.push(hasher.finalize().into());
            i += 2;
        }
        leaf_hashes = next;
    }

    leaf_hashes[0]
}

pub fn attach_proof(diff: &mut StateDiff, root_before: Hash) {
    diff.merkle_proof = vec![root_before];
    diff.witness_path = vec![WitnessPathElement {
        sibling_hash: root_before,
        sibling_is_left: false,
    }];
    diff.leaf_encoding = "account_v1(balance_u64,nonce_u64)".to_string();
}
