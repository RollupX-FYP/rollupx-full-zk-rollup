// Comprehensive test module for executor transaction engine
// Tests signature validation, state transitions, batch determinism, and edge cases

#[cfg(test)]
mod executor_tx_engine_tests {
    use zksync_state_machine::state::InMemoryStateManager;
    use zksync_state_machine::state::StateManager;
    use zksync_state_machine::tx_engine::{SimpleTransactionEngine, TransactionEngine};
    use zksync_state_machine::types::{Account, Transaction};
    use ethers::signers::{LocalWallet, Signer};
    use ethers::types::H256;
    use rand::thread_rng;
    use std::str::FromStr;

    // ============ Fixtures & Helpers ============

    fn make_unsigned_tx(
        from: [u8; 20],
        to: [u8; 20],
        amount: u64,
        nonce: u64,
    ) -> Transaction {
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

    fn make_signed_tx(
        from: [u8; 20],
        to: [u8; 20],
        amount: u64,
        nonce: u64,
        sig: Vec<u8>,
    ) -> Transaction {
        Transaction {
            from,
            to,
            amount,
            nonce,
            signature: sig,
            gas_price: 1,
            gas_limit: 21_000,
            timestamp: 1,
            boost_bid: 0,
        }
    }

    fn make_invalid_signature_tx(from: [u8; 20], to: [u8; 20], amount: u64, nonce: u64) -> Transaction {
        let mut tx = make_signed_tx(from, to, amount, nonce, vec![0u8; 65]);
        tx.signature[0] = 0xFF;
        tx
    }

    fn create_test_engine() -> SimpleTransactionEngine<InMemoryStateManager> {
        SimpleTransactionEngine::new(InMemoryStateManager::default())
    }

    fn seed_account(engine: &mut SimpleTransactionEngine<InMemoryStateManager>, addr: [u8; 20], balance: u64, nonce: u64) {
        engine.state.seed_account(
            addr,
            Account {
                balance,
                nonce,
            },
        );
    }

    // ============ A.1 Signature Verification Tests ============

    #[test]
    fn test_unsigned_tx_accepted_with_env_flag() {
        std::env::set_var("ALLOW_UNSIGNED_USER_TXS", "1");
        let mut engine = create_test_engine();
        let from = [1u8; 20];
        let to = [2u8; 20];
        seed_account(&mut engine, from, 100, 0);

        let tx = make_unsigned_tx(from, to, 10, 0);
        let result = engine.execute_batch("b1", vec![tx]);

        assert!(result.is_ok());
        let trace = result.unwrap();
        assert_eq!(trace.executed_transactions.len(), 1);
    }

    #[test]
    fn test_unsigned_tx_rejected_by_default() {
        std::env::remove_var("ALLOW_UNSIGNED_USER_TXS");
        let mut engine = create_test_engine();
        let from = [1u8; 20];
        let to = [2u8; 20];
        seed_account(&mut engine, from, 100, 0);

        let tx = make_unsigned_tx(from, to, 10, 0);
        let result = engine.execute_batch("b1", vec![tx]);

        assert!(result.is_ok());
        let trace = result.unwrap();
        // Unsigned tx should be rejected
        assert_eq!(trace.executed_transactions.len(), 0);
        assert_eq!(trace.tx_outcomes.len(), 1);
        assert_eq!(trace.tx_outcomes[0].rejection_reason, Some("invalid_signature".to_string()));
    }

    #[test]
    fn test_malformed_signature_rejected() {
        std::env::set_var("ALLOW_UNSIGNED_USER_TXS", "1");
        let mut engine = create_test_engine();
        let from = [1u8; 20];
        let to = [2u8; 20];
        seed_account(&mut engine, from, 100, 0);

        // Create tx with wrong-length signature
        let mut tx = make_unsigned_tx(from, to, 10, 0);
        tx.signature = vec![0u8; 64]; // Wrong length (needs 65)
        tx.gas_price = 1; // Make it look like a user tx

        let result = engine.execute_batch("b1", vec![tx]);
        assert!(result.is_ok());
        let trace = result.unwrap();
        // Should be rejected for invalid signature
        assert_eq!(trace.executed_transactions.len(), 0);
    }

    // ============ A.2 Transaction Validation Tests ============

    #[test]
    fn test_nonce_mismatch_rejected() {
        std::env::set_var("ALLOW_UNSIGNED_USER_TXS", "1");
        let mut engine = create_test_engine();
        let from = [3u8; 20];
        let to = [4u8; 20];
        seed_account(&mut engine, from, 100, 0);

        // Account nonce is 0, but tx claims nonce 1
        let tx = make_unsigned_tx(from, to, 10, 1);
        let result = engine.execute_batch("b1", vec![tx]);

        assert!(result.is_ok());
        let trace = result.unwrap();
        assert_eq!(trace.executed_transactions.len(), 0);
        assert_eq!(trace.tx_outcomes[0].rejection_reason, Some("invalid_nonce".to_string()));
    }

    #[test]
    fn test_insufficient_balance_rejected() {
        std::env::set_var("ALLOW_UNSIGNED_USER_TXS", "1");
        let mut engine = create_test_engine();
        let from = [5u8; 20];
        let to = [6u8; 20];
        seed_account(&mut engine, from, 5, 0); // Only 5 wei balance

        let tx = make_unsigned_tx(from, to, 10, 0); // Try to send 10
        let result = engine.execute_batch("b1", vec![tx]);

        assert!(result.is_ok());
        let trace = result.unwrap();
        assert_eq!(trace.executed_transactions.len(), 0);
        assert_eq!(trace.tx_outcomes[0].rejection_reason, Some("insufficient_balance".to_string()));
    }

    #[test]
    fn test_valid_tx_updates_balances_and_nonce() {
        std::env::set_var("ALLOW_UNSIGNED_USER_TXS", "1");
        let mut engine = create_test_engine();
        let from = [7u8; 20];
        let to = [8u8; 20];
        seed_account(&mut engine, from, 100, 0);
        seed_account(&mut engine, to, 50, 0);

        let tx = make_unsigned_tx(from, to, 25, 0);
        let result = engine.execute_batch("b1", vec![tx]);

        assert!(result.is_ok());
        let trace = result.unwrap();
        assert_eq!(trace.executed_transactions.len(), 1);
        assert_eq!(trace.tx_outcomes.len(), 1);

        let outcome = &trace.tx_outcomes[0];
        assert!(outcome.included);
        assert_eq!(outcome.sender_pre.balance, 100);
        assert_eq!(outcome.sender_post.balance, 75); // 100 - 25
        assert_eq!(outcome.sender_pre.nonce, 0);
        assert_eq!(outcome.sender_post.nonce, 1);
        assert_eq!(outcome.receiver_pre.balance, 50);
        assert_eq!(outcome.receiver_post.balance, 75); // 50 + 25
        assert_eq!(outcome.receiver_pre.nonce, 0);
        assert_eq!(outcome.receiver_post.nonce, 0); // nonce doesn't change for receiver
    }

    // ============ A.3 State Transitions & Merkle Roots ============

    #[test]
    fn test_single_tx_produces_state_diffs() {
        std::env::set_var("ALLOW_UNSIGNED_USER_TXS", "1");
        let mut engine = create_test_engine();
        let from = [9u8; 20];
        let to = [10u8; 20];
        seed_account(&mut engine, from, 100, 0);

        let initial_root = engine.state.current_root();
        let tx = make_unsigned_tx(from, to, 50, 0);
        let result = engine.execute_batch("b1", vec![tx]);

        assert!(result.is_ok());
        let trace = result.unwrap();
        assert_eq!(trace.public_inputs.initial_root, initial_root);
        assert_ne!(trace.public_inputs.initial_root, trace.public_inputs.final_root);
        assert_eq!(trace.state_diffs.len(), 2); // sender + receiver
        assert_eq!(trace.state_diffs[0].account, from);
        assert_eq!(trace.state_diffs[1].account, to);
    }

    #[test]
    fn test_multiple_txs_root_progression() {
        std::env::set_var("ALLOW_UNSIGNED_USER_TXS", "1");
        let mut engine = create_test_engine();
        let from = [11u8; 20];
        let to = [12u8; 20];
        seed_account(&mut engine, from, 1000, 0);

        let txs = vec![
            make_unsigned_tx(from, to, 100, 0),
            make_unsigned_tx(from, to, 100, 1),
            make_unsigned_tx(from, to, 100, 2),
        ];

        let result = engine.execute_batch("b1", txs);
        assert!(result.is_ok());

        let trace = result.unwrap();
        assert_eq!(trace.executed_transactions.len(), 3);
        assert_eq!(trace.state_diffs.len(), 6); // 3 txs * 2 diffs
        assert_ne!(trace.public_inputs.initial_root, trace.public_inputs.final_root);
    }

    #[test]
    fn test_merkle_proof_present_in_diffs() {
        std::env::set_var("ALLOW_UNSIGNED_USER_TXS", "1");
        let mut engine = create_test_engine();
        let from = [13u8; 20];
        let to = [14u8; 20];
        seed_account(&mut engine, from, 100, 0);

        let tx = make_unsigned_tx(from, to, 50, 0);
        let result = engine.execute_batch("b1", vec![tx]);

        assert!(result.is_ok());
        let trace = result.unwrap();
        for diff in &trace.state_diffs {
            assert!(!diff.merkle_proof.is_empty(), "Each diff must have merkle_proof");
        }
    }

    // ============ A.4 Batch Determinism ============

    #[test]
    fn test_deterministic_trace_from_same_batch() {
        std::env::set_var("ALLOW_UNSIGNED_USER_TXS", "1");

        let txs = (0..5)
            .map(|i| make_unsigned_tx([i; 20], [i + 1; 20], 50, 0))
            .collect::<Vec<_>>();

        let mut traces = Vec::new();
        for _ in 0..3 {
            let mut engine = create_test_engine();
            for tx in &txs {
                engine.state.seed_account(tx.from, Account {
                    balance: 1000,
                    nonce: 0,
                });
            }

            let result = engine.execute_batch("batch1", txs.clone());
            assert!(result.is_ok());
            traces.push(result.unwrap());
        }

        // Check all traces have same initial_root, final_root, commitments
        assert_eq!(traces[0].public_inputs.initial_root, traces[1].public_inputs.initial_root);
        assert_eq!(traces[0].public_inputs.final_root, traces[1].public_inputs.final_root);
        assert_eq!(traces[1].public_inputs.final_root, traces[2].public_inputs.final_root);
        assert_eq!(traces[0].public_inputs.tx_commitment, traces[1].public_inputs.tx_commitment);
        assert_eq!(traces[0].public_inputs.state_diff_commitment, traces[1].public_inputs.state_diff_commitment);
    }

    #[test]
    fn test_empty_batch_generates_trace() {
        std::env::set_var("ALLOW_UNSIGNED_USER_TXS", "1");
        let mut engine = create_test_engine();
        let initial_root = engine.state.current_root();

        let result = engine.execute_batch("empty", vec![]);
        assert!(result.is_ok());

        let trace = result.unwrap();
        assert_eq!(trace.executed_transactions.len(), 0);
        assert_eq!(trace.state_diffs.len(), 0);
        assert_eq!(trace.public_inputs.initial_root, initial_root);
        assert_eq!(trace.public_inputs.initial_root, trace.public_inputs.final_root);
    }

    // ============ A.5 Rejection Handling ============

    #[test]
    fn test_mixed_valid_and_invalid_txs() {
        std::env::set_var("ALLOW_UNSIGNED_USER_TXS", "1");
        let mut engine = create_test_engine();
        let from = [15u8; 20];
        let to1 = [16u8; 20];
        let to2 = [17u8; 20];
        seed_account(&mut engine, from, 100, 0);

        let txs = vec![
            make_unsigned_tx(from, to1, 50, 0), // valid
            make_unsigned_tx(from, to2, 100, 0), // insufficient balance (only 100 left after first)
            make_unsigned_tx(from, to1, 25, 0), // insufficient nonce
        ];

        let result = engine.execute_batch("b1", txs);
        assert!(result.is_ok());

        let trace = result.unwrap();
        assert_eq!(trace.executed_transactions.len(), 1); // Only first succeeds
        assert_eq!(trace.tx_outcomes.len(), 3);
        assert!(trace.tx_outcomes[0].included);
        assert!(!trace.tx_outcomes[1].included);
        assert!(!trace.tx_outcomes[2].included);
    }

    #[test]
    fn test_rejected_txs_dont_update_state() {
        std::env::set_var("ALLOW_UNSIGNED_USER_TXS", "1");
        let mut engine = create_test_engine();
        let from = [18u8; 20];
        let to = [19u8; 20];
        seed_account(&mut engine, from, 50, 0);

        let txs = vec![
            make_unsigned_tx(from, to, 100, 0), // insufficient balance -> rejected
        ];

        let result = engine.execute_batch("b1", txs);
        assert!(result.is_ok());

        let trace = result.unwrap();
        assert_eq!(trace.state_diffs.len(), 0); // No state changes
        assert_eq!(trace.public_inputs.initial_root, trace.public_inputs.final_root);
    }

    // ============ A.6 Batch Scaling ============

    #[test]
    fn test_large_batch_10_transactions() {
        std::env::set_var("ALLOW_UNSIGNED_USER_TXS", "1");
        let mut engine = create_test_engine();

        let from = [20u8; 20];
        seed_account(&mut engine, from, 10000, 0);

        let mut txs = Vec::new();
        for i in 0u64..100u64 {
            let mut to = [21u8; 20];
            to[0] = to[0] + i as u8;
            txs.push(make_unsigned_tx(from, to, 100, i as u64));
        }

        let result = engine.execute_batch("large1", txs);
        assert!(result.is_ok());

        let trace = result.unwrap();
        assert_eq!(trace.executed_transactions.len(), 10);
        assert_eq!(trace.state_diffs.len(), 20); // 10 txs * 2 diffs each
    }

    #[test]
    fn test_large_batch_50_transactions() {
        std::env::set_var("ALLOW_UNSIGNED_USER_TXS", "1");
        let mut engine = create_test_engine();

        let from = [22u8; 20];
        seed_account(&mut engine, from, 100000, 0);

        let mut txs = Vec::new();
        for i in 0u64..100u64 {
            let mut to = [23u8; 20];
            to[0] = to[0].wrapping_add(i as u8);
            txs.push(make_unsigned_tx(from, to, 100, i as u64));
        }

        let result = engine.execute_batch("large2", txs);
        assert!(result.is_ok());

        let trace = result.unwrap();
        assert_eq!(trace.executed_transactions.len(), 50);
        assert_eq!(trace.state_diffs.len(), 100);
    }

    #[test]
    fn test_batch_with_dust_transfers() {
        std::env::set_var("ALLOW_UNSIGNED_USER_TXS", "1");
        let mut engine = create_test_engine();

        let from = [24u8; 20];
        seed_account(&mut engine, from, 1000, 0);

        let mut txs = Vec::new();
        for i in 0..10 {
            let mut to = [25u8; 20];
            to[0] = to[0] + i as u8;
            txs.push(make_unsigned_tx(from, to, 1, i as u64)); // 1 wei transfers
        }

        let result = engine.execute_batch("dust", txs);
        assert!(result.is_ok());

        let trace = result.unwrap();
        assert_eq!(trace.executed_transactions.len(), 10);
    }
}
