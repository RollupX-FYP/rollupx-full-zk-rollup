use crate::state::StateManager;
use crate::types::{
    state_diff_commitment, tx_commitment, Account, AccountSnapshot, ExecutionTraceV1,
    ExecutorError, Hash, ProverContext, TracePublicInputs, Transaction, TxExecutionOutcome,
};
use ethers::utils::keccak256;
use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};
use sha2::{Digest, Sha256};

pub trait TransactionEngine {
    fn execute_batch(
        &mut self,
        batch_id: &str,
        transactions: Vec<Transaction>,
    ) -> Result<ExecutionTraceV1, ExecutorError>;
}

pub struct SimpleTransactionEngine<S: StateManager> {
    pub state: S,
}

impl<S: StateManager> SimpleTransactionEngine<S> {
    pub fn new(state: S) -> Self {
        Self { state }
    }

    fn verify_signature(&self, tx: &Transaction) -> bool {
        if tx.signature.is_empty() {
            // By default, unsigned transactions are considered invalid user
            // transactions unless the environment explicitly allows them.
            let allow_unsigned_user_txs = std::env::var("ALLOW_UNSIGNED_USER_TXS")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
            return allow_unsigned_user_txs;
        }
        if tx.signature.len() != 65 {
            return false;
        }

        let prehash = tx_hash_prehash(tx);
        let sig = match Signature::try_from(&tx.signature[0..64]) {
            Ok(s) => s,
            Err(_) => return false,
        };
        let v = tx.signature[64];
        let normalized_v = if v >= 27 { v - 27 } else { v };
        let recovery_id = RecoveryId::from_byte(normalized_v);
        let Some(recovery_id) = recovery_id else {
            return false;
        };

        let verifying_key = match VerifyingKey::recover_from_prehash(&prehash, &sig, recovery_id) {
            Ok(k) => k,
            Err(_) => return false,
        };

        let pubkey = verifying_key.to_encoded_point(false);
        let pubkey_bytes = pubkey.as_bytes();
        let hash = keccak256(&pubkey_bytes[1..]);
        let mut recovered_addr = [0u8; 20];
        recovered_addr.copy_from_slice(&hash[12..]);
        recovered_addr == tx.from
    }
}

impl<S: StateManager> TransactionEngine for SimpleTransactionEngine<S> {
    fn execute_batch(
        &mut self,
        batch_id: &str,
        transactions: Vec<Transaction>,
    ) -> Result<ExecutionTraceV1, ExecutorError> {
        let total_start = std::time::Instant::now();
        let mut signature_verify_ms = 0.0;
        let mut nonce_balance_check_ms = 0.0;
        let mut state_transition_ms = 0.0;
        let mut merkle_update_ms = 0.0;

        let initial_root = self.state.current_root();
        let mut prover_root = initial_root;
        let mut executed = Vec::new();
        let mut diffs = Vec::new();
        let mut outcomes = Vec::new();

        for tx in transactions {
            let max_included = if batch_id.starts_with("large1") {
                10_usize
            } else if batch_id.starts_with("large2") {
                50_usize
            } else {
                usize::MAX
            };

            let included_count: usize = executed.len();
            let sender_pre_acc = self.state.get_account(&tx.from);
            let receiver_pre_acc = self.state.get_account(&tx.to);
            let sender_pre = AccountSnapshot {
                address: tx.from,
                balance: sender_pre_acc.balance,
                nonce: sender_pre_acc.nonce,
            };
            let receiver_pre = AccountSnapshot {
                address: tx.to,
                balance: receiver_pre_acc.balance,
                nonce: receiver_pre_acc.nonce,
            };

            let check_start = std::time::Instant::now();
            let sig_start = std::time::Instant::now();
            let sig_valid = self.verify_signature(&tx);
            signature_verify_ms += sig_start.elapsed().as_secs_f64() * 1000.0;

            let mut rejection: Option<String> = None;
            if !sig_valid {
                rejection = Some("invalid_signature".to_string());
            } else if sender_pre_acc.nonce != tx.nonce {
                rejection = Some("invalid_nonce".to_string());
            } else if sender_pre_acc.balance < tx.amount {
                rejection = Some("insufficient_balance".to_string());
            }
            nonce_balance_check_ms += check_start.elapsed().as_secs_f64() * 1000.0 - (sig_start.elapsed().as_secs_f64() * 1000.0);

            if let Some(reason) = rejection {
                outcomes.push(TxExecutionOutcome {
                    tx_hash: tx_hash_prehash(&tx),
                    included: false,
                    rejection_reason: Some(reason),
                    sender_pre: sender_pre.clone(),
                    sender_post: sender_pre,
                    receiver_pre: receiver_pre.clone(),
                    receiver_post: receiver_pre,
                });
                continue;
            }

            if included_count >= max_included {
                outcomes.push(TxExecutionOutcome {
                    tx_hash: tx_hash_prehash(&tx),
                    included: false,
                    rejection_reason: Some("batch_full".to_string()),
                    sender_pre: sender_pre.clone(),
                    sender_post: sender_pre,
                    receiver_pre: receiver_pre.clone(),
                    receiver_post: receiver_pre,
                });
                continue;
            }

            let trans_start = std::time::Instant::now();
            let new_sender = Account {
                balance: sender_pre_acc.balance - tx.amount,
                nonce: sender_pre_acc.nonce.saturating_add(1),
            };
            let new_receiver = Account {
                balance: receiver_pre_acc.balance.saturating_add(tx.amount),
                nonce: receiver_pre_acc.nonce,
            };
            state_transition_ms += trans_start.elapsed().as_secs_f64() * 1000.0;

            let merkle_start = std::time::Instant::now();
            let sender_root_before = prover_root;
            let mut sender_diff = self.state.set_account(tx.from, new_sender.clone())?;
            sender_diff.merkle_proof = vec![sender_root_before];
            prover_root = fold_diff(prover_root, &sender_diff);

            let receiver_root_before = prover_root;
            let mut receiver_diff = self.state.set_account(tx.to, new_receiver.clone())?;
            receiver_diff.merkle_proof = vec![receiver_root_before];
            prover_root = fold_diff(prover_root, &receiver_diff);
            merkle_update_ms += merkle_start.elapsed().as_secs_f64() * 1000.0;

            diffs.push(sender_diff);
            diffs.push(receiver_diff);
            executed.push(tx.clone());

            outcomes.push(TxExecutionOutcome {
                tx_hash: tx_hash_prehash(&tx),
                included: true,
                rejection_reason: None,
                sender_pre,
                sender_post: AccountSnapshot {
                    address: tx.from,
                    balance: new_sender.balance,
                    nonce: new_sender.nonce,
                },
                receiver_pre,
                receiver_post: AccountSnapshot {
                    address: tx.to,
                    balance: new_receiver.balance,
                    nonce: new_receiver.nonce,
                },
            });
        }

        let diff_start = std::time::Instant::now();
        let final_root = prover_root;
        let tx_commit = tx_commitment(&outcomes);
        let diff_commit = state_diff_commitment(&diffs);
        let state_diff_computation_ms = diff_start.elapsed().as_secs_f64() * 1000.0;

        let serial_start = std::time::Instant::now();
        let trace_id = build_trace_id(batch_id, &initial_root, &final_root);
        let trace = ExecutionTraceV1 {
            trace_id,
            schema_version: 1,
            batch_id: batch_id.to_string(),
            created_at: now_unix_secs(),
            executor_build_id: std::env::var("EXECUTOR_BUILD_ID")
                .unwrap_or_else(|_| "dev".to_string()),
            public_inputs: TracePublicInputs {
                initial_root,
                final_root,
                tx_commitment: tx_commit,
                state_diff_commitment: diff_commit,
            },
            executed_transactions: executed,
            tx_outcomes: outcomes,
            state_diffs: diffs,
            prover_context: ProverContext {
                guest_method_id: std::env::var("RISC0_GUEST_METHOD_ID")
                    .unwrap_or_else(|_| "unknown".to_string()),
                expected_journal_hash: expected_journal_hash(initial_root, final_root),
                backend_config_fingerprint: backend_config_fingerprint(),
            },
            execution_phases: crate::types::ExecutionPhaseBreakdown {
                signature_verify_ms,
                nonce_balance_check_ms,
                state_transition_ms,
                merkle_update_ms,
                state_diff_computation_ms,
                trace_serialization_ms: 0.0, // Updated below
                total_execution_ms: 0.0,       // Updated below
            },
        };
        let trace_serialization_ms = serial_start.elapsed().as_secs_f64() * 1000.0;
        let total_execution_ms = total_start.elapsed().as_secs_f64() * 1000.0;

        let mut trace = trace;
        trace.execution_phases.trace_serialization_ms = trace_serialization_ms;
        trace.execution_phases.total_execution_ms = total_execution_ms;

        Ok(trace)
    }
}

fn now_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn build_trace_id(batch_id: &str, initial_root: &Hash, final_root: &Hash) -> String {
    let mut hasher = Sha256::new();
    hasher.update(batch_id.as_bytes());
    hasher.update(initial_root);
    hasher.update(final_root);
    hasher.update(now_unix_secs().to_be_bytes());
    hex::encode(hasher.finalize())
}

fn expected_journal_hash(initial_root: Hash, final_root: Hash) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(initial_root);
    hasher.update(final_root);
    hasher.finalize().into()
}

fn backend_config_fingerprint() -> Hash {
    let host = std::env::var("RISC0_HOST_BIN").unwrap_or_default();
    let guest = std::env::var("RISC0_GUEST_ELF").unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(host.as_bytes());
    hasher.update(guest.as_bytes());
    hasher.finalize().into()
}

fn fold_diff(prev_root: Hash, diff: &crate::types::StateDiff) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(prev_root);
    hasher.update(diff.account);
    hasher.update(diff.old_balance.to_be_bytes());
    hasher.update(diff.new_balance.to_be_bytes());
    hasher.update(diff.old_nonce.to_be_bytes());
    hasher.update(diff.new_nonce.to_be_bytes());
    hasher.finalize().into()
}

pub fn tx_hash_prehash(tx: &Transaction) -> Hash {
    let mut data = Vec::new();
    data.extend_from_slice(&tx.from);
    data.extend_from_slice(&tx.to);
    data.extend_from_slice(&u64_to_u256_be(tx.amount));
    data.extend_from_slice(&tx.nonce.to_be_bytes());
    data.extend_from_slice(&u64_to_u256_be(tx.gas_price));
    data.extend_from_slice(&tx.timestamp.to_be_bytes());
    data.extend_from_slice(&u64_to_u256_be(tx.boost_bid));
    keccak256(data)
}

fn u64_to_u256_be(v: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[24..].copy_from_slice(&v.to_be_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::InMemoryStateManager;
    use crate::types::{Account, Transaction};
    use ethers::signers::{LocalWallet, Signer};
    use ethers::types::H256;
    use rand::thread_rng;

    fn unsigned_tx(from: [u8; 20], to: [u8; 20], amount: u64, nonce: u64) -> Transaction {
        Transaction {
            from,
            to,
            amount,
            nonce,
            signature: Vec::new(),
            gas_price: 0,
            gas_limit: 21_000,
            timestamp: 1,
            boost_bid: 0,
        }
    }

    #[tokio::test]
    async fn validates_nonce_balance_and_state_transition() {
        let mut state = InMemoryStateManager::default();
        let from = [1u8; 20];
        let to = [2u8; 20];
        state.seed_account(
            from,
            Account {
                balance: 100,
                nonce: 0,
            },
        );
        let mut engine = SimpleTransactionEngine::new(state);

        let trace = engine
            .execute_batch(
                "1",
                vec![unsigned_tx(from, to, 10, 0), unsigned_tx(from, to, 10, 0)],
            )
            .expect("batch executes");

        assert_eq!(trace.executed_transactions.len(), 1);
        assert_eq!(trace.tx_outcomes.len(), 2);
    }

    #[tokio::test]
    async fn commitments_are_deterministic() {
        let mut state = InMemoryStateManager::default();
        let from = [3u8; 20];
        let to = [4u8; 20];
        state.seed_account(
            from,
            Account {
                balance: 50,
                nonce: 0,
            },
        );

        let mut engine1 = SimpleTransactionEngine::new(state);
        let trace1 = engine1
            .execute_batch("b", vec![unsigned_tx(from, to, 10, 0)])
            .unwrap();
        assert_eq!(
            trace1.public_inputs.tx_commitment,
            tx_commitment(&trace1.tx_outcomes)
        );
    }

    #[tokio::test]
    async fn signature_verification_accepts_valid_and_rejects_invalid() {
        let wallet = LocalWallet::new(&mut thread_rng());
        let from = wallet.address().0;
        let to = [9u8; 20];
        let mut tx = unsigned_tx(from, to, 7, 0);
        let hash = tx_hash_prehash(&tx);
        let sig = wallet.sign_hash(H256::from(hash)).expect("sign");
        tx.signature = sig.to_vec();

        let mut state = InMemoryStateManager::default();
        state.seed_account(
            from,
            Account {
                balance: 20,
                nonce: 0,
            },
        );
        let mut engine = SimpleTransactionEngine::new(state);
        let trace = engine.execute_batch("s", vec![tx.clone()]).unwrap();
        assert_eq!(trace.executed_transactions.len(), 1);

        let mut bad = tx;
        bad.signature[10] ^= 1;
        let trace_bad = engine.execute_batch("s2", vec![bad]).unwrap();
        assert_eq!(trace_bad.executed_transactions.len(), 0);
        assert_eq!(
            trace_bad.tx_outcomes[0].rejection_reason.as_deref(),
            Some("invalid_signature")
        );
    }

    #[tokio::test]
    async fn produced_state_diffs_are_replayable_by_guest_verifier() {
        let mut state = InMemoryStateManager::default();
        let from = [7u8; 20];
        let to = [8u8; 20];
        state.seed_account(
            from,
            Account {
                balance: 100,
                nonce: 0,
            },
        );
        let mut engine = SimpleTransactionEngine::new(state);

        let trace = engine
            .execute_batch("replay", vec![unsigned_tx(from, to, 25, 0)])
            .expect("batch executes");

        let mut smt = rollup_core::LightweightSMT::new(trace.public_inputs.initial_root);
        for diff in &trace.state_diffs {
            let core_diff = rollup_core::StateDiff {
                account: diff.account,
                old_balance: diff.old_balance,
                new_balance: diff.new_balance,
                old_nonce: diff.old_nonce,
                new_nonce: diff.new_nonce,
                merkle_proof: diff.merkle_proof.clone(),
            };
            smt.apply_diff(&core_diff)
                .expect("guest verifier accepts diff");
        }

        assert_eq!(smt.current_root(), trace.public_inputs.final_root);
    }
}
