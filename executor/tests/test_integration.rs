// End-to-end integration tests for executor
// Tests full pipeline: generate txs → execute → persist → verify

#[cfg(test)]
mod executor_integration_tests {
    use zksync_state_machine::state::InMemoryStateManager;
    use zksync_state_machine::state::StateManager;
    use zksync_state_machine::trace::{append_lifecycle, persist_trace, verify_trace_hash, TraceLifecycleStatus};
    use zksync_state_machine::tx_engine::{SimpleTransactionEngine, TransactionEngine};
    use zksync_state_machine::types::{Account, Transaction};
    use std::fs;
    use tempfile::TempDir;

    // ============ Fixtures ============

    fn make_test_tx(from: [u8; 20], to: [u8; 20], amount: u64, nonce: u64) -> Transaction {
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

    // ============ E2E Flow Tests ============

    #[test]
    fn test_e2e_single_tx_full_pipeline() {
        std::env::set_var("ALLOW_UNSIGNED_USER_TXS", "1");
        let temp = TempDir::new().unwrap();

        // Step 1: Create executor
        let mut engine = SimpleTransactionEngine::new(InMemoryStateManager::default());
        let from = [1u8; 20];
        let to = [2u8; 20];
        engine.state.seed_account(from, Account {
            balance: 1000,
            nonce: 0,
        });

        // Step 2: Execute batch
        let tx = make_test_tx(from, to, 100, 0);
        let trace = engine.execute_batch("batch1", vec![tx]).unwrap();

        // Verify trace properties
        assert_eq!(trace.executed_transactions.len(), 1);
        assert_eq!(trace.state_diffs.len(), 2);
        assert_ne!(trace.public_inputs.initial_root, trace.public_inputs.final_root);

        // Step 3: Persist trace
        let persist_result = persist_trace(temp.path(), &trace);
        assert!(persist_result.is_ok());
        let meta = persist_result.unwrap();

        // Step 4: Verify persisted trace
        let verify_result = verify_trace_hash(&meta.trace_path, &meta.sha256_hex);
        assert!(verify_result.is_ok());

        // Step 5: Verify trace is readable
        let json = fs::read_to_string(&meta.trace_path).unwrap();
        let deserialized: zksync_state_machine::types::ExecutionTraceV1 = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.batch_id, "batch1");

        // Step 6: Record lifecycle
        append_lifecycle(
            temp.path(),
            &trace,
            TraceLifecycleStatus::Persisted,
            Some(&meta.trace_path),
            Some(&meta.sha256_hex),
        ).unwrap();

        // Verify lifecycle index exists
        let index_path = temp.path().join("index.jsonl");
        assert!(index_path.exists());
    }

    #[test]
    fn test_e2e_multiple_txs_deterministic() {
        std::env::set_var("ALLOW_UNSIGNED_USER_TXS", "1");
        let temp1 = TempDir::new().unwrap();
        let temp2 = TempDir::new().unwrap();

        // Create identical batches
        let txs = (0..5)
            .map(|i| make_test_tx([i; 20], [i + 1; 20], 50, 0))
            .collect::<Vec<_>>();

        let mut traces = Vec::new();
        let mut persist_results = Vec::new();

        for (idx, temp) in [temp1.path(), temp2.path()].iter().enumerate() {
            let mut engine = SimpleTransactionEngine::new(InMemoryStateManager::default());
            for tx in &txs {
                engine.state.seed_account(tx.from, Account {
                    balance: 1000,
                    nonce: 0,
                });
            }

            let trace = engine.execute_batch("same_batch", txs.clone()).unwrap();
            let meta = persist_trace(temp, &trace).unwrap();

            traces.push(trace);
            persist_results.push(meta);
        }

        // Verify traces are identical
        assert_eq!(
            traces[0].public_inputs.initial_root,
            traces[1].public_inputs.initial_root
        );
        assert_eq!(
            traces[0].public_inputs.final_root,
            traces[1].public_inputs.final_root
        );
        assert_eq!(
            traces[0].public_inputs.tx_commitment,
            traces[1].public_inputs.tx_commitment
        );

        // Verify persisted traces have same hash
        assert_eq!(persist_results[0].sha256_hex, persist_results[1].sha256_hex);
    }

    #[test]
    fn test_e2e_batch_with_rejections() {
        std::env::set_var("ALLOW_UNSIGNED_USER_TXS", "1");
        let temp = TempDir::new().unwrap();

        let mut engine = SimpleTransactionEngine::new(InMemoryStateManager::default());
        let from = [1u8; 20];
        let to1 = [2u8; 20];
        let to2 = [3u8; 20];

        engine.state.seed_account(from, Account {
            balance: 100,
            nonce: 0,
        });

        // Mix of valid and invalid txs
        let txs = vec![
            make_test_tx(from, to1, 50, 0),  // valid
            make_test_tx(from, to2, 200, 0), // insufficient balance
            make_test_tx(from, to1, 25, 0),  // invalid nonce
        ];

        let trace = engine.execute_batch("mixed", txs).unwrap();

        // Verify execution
        assert_eq!(trace.executed_transactions.len(), 1); // Only first succeeds
        assert_eq!(trace.tx_outcomes.len(), 3);

        // Persist and verify
        let meta = persist_trace(temp.path(), &trace).unwrap();
        let verify_result = verify_trace_hash(&meta.trace_path, &meta.sha256_hex);
        assert!(verify_result.is_ok());
    }

    #[test]
    fn test_e2e_large_batch_10_txs() {
        std::env::set_var("ALLOW_UNSIGNED_USER_TXS", "1");
        let temp = TempDir::new().unwrap();

        let mut engine = SimpleTransactionEngine::new(InMemoryStateManager::default());
        let from = [1u8; 20];
        engine.state.seed_account(from, Account {
            balance: 10000,
            nonce: 0,
        });

        let mut txs = Vec::new();
        for i in 0..10 {
            let mut to = [2u8; 20];
            to[0] = to[0] + i as u8;
            txs.push(make_test_tx(from, to, 100, i as u64));
        }

        let trace = engine.execute_batch("large10", txs).unwrap();
        assert_eq!(trace.executed_transactions.len(), 10);

        let meta = persist_trace(temp.path(), &trace).unwrap();
        assert!(verify_trace_hash(&meta.trace_path, &meta.sha256_hex).is_ok());
    }

    #[test]
    fn test_e2e_lifecycle_tracking() {
        std::env::set_var("ALLOW_UNSIGNED_USER_TXS", "1");
        let temp = TempDir::new().unwrap();

        let mut engine = SimpleTransactionEngine::new(InMemoryStateManager::default());
        let from = [1u8; 20];
        let to = [2u8; 20];
        engine.state.seed_account(from, Account {
            balance: 1000,
            nonce: 0,
        });

        let trace = engine.execute_batch("b1", vec![make_test_tx(from, to, 100, 0)]).unwrap();

        // Track full lifecycle
        append_lifecycle(
            temp.path(),
            &trace,
            TraceLifecycleStatus::Generated,
            None,
            None,
        ).unwrap();

        let meta = persist_trace(temp.path(), &trace).unwrap();
        append_lifecycle(
            temp.path(),
            &trace,
            TraceLifecycleStatus::Persisted,
            Some(&meta.trace_path),
            Some(&meta.sha256_hex),
        ).unwrap();

        append_lifecycle(
            temp.path(),
            &trace,
            TraceLifecycleStatus::Proved,
            None,
            None,
        ).unwrap();

        append_lifecycle(
            temp.path(),
            &trace,
            TraceLifecycleStatus::Published,
            None,
            None,
        ).unwrap();

        // Verify 4 lifecycle entries
        let index_path = temp.path().join("index.jsonl");
        let contents = fs::read_to_string(&index_path).unwrap();
        let count = contents.lines().count();
        assert_eq!(count, 4);
    }

    #[test]
    fn test_e2e_empty_batch() {
        std::env::set_var("ALLOW_UNSIGNED_USER_TXS", "1");
        let temp = TempDir::new().unwrap();

        let mut engine = SimpleTransactionEngine::new(InMemoryStateManager::default());
        let initial_root = engine.state.current_root();

        let trace = engine.execute_batch("empty", vec![]).unwrap();

        assert_eq!(trace.executed_transactions.len(), 0);
        assert_eq!(trace.state_diffs.len(), 0);
        assert_eq!(trace.public_inputs.initial_root, initial_root);
        assert_eq!(trace.public_inputs.initial_root, trace.public_inputs.final_root);

        let meta = persist_trace(temp.path(), &trace).unwrap();
        assert!(verify_trace_hash(&meta.trace_path, &meta.sha256_hex).is_ok());
    }

    #[test]
    fn test_e2e_concurrent_batches() {
        std::env::set_var("ALLOW_UNSIGNED_USER_TXS", "1");
        let temp = TempDir::new().unwrap();

        let mut results = Vec::new();

        for batch_num in 0..3 {
            let mut engine = SimpleTransactionEngine::new(InMemoryStateManager::default());
            let from = [batch_num as u8; 20];
            let to = [batch_num as u8 + 1; 20];

            engine.state.seed_account(from, Account {
                balance: 1000,
                nonce: 0,
            });

            let batch_id = format!("batch_{}", batch_num);
            let tx = make_test_tx(from, to, 100, 0);
            let trace = engine.execute_batch(&batch_id, vec![tx]).unwrap();
            let meta = persist_trace(temp.path(), &trace).unwrap();

            results.push((batch_id, meta.sha256_hex));
        }

        // Verify all batches persisted
        assert_eq!(results.len(), 3);
        assert!(results[0].1 != results[1].1); // Different hashes
    }

    #[test]
    fn test_e2e_state_transition_verification() {
        std::env::set_var("ALLOW_UNSIGNED_USER_TXS", "1");

        let mut engine = SimpleTransactionEngine::new(InMemoryStateManager::default());
        let from = [1u8; 20];
        let to = [2u8; 20];

        engine.state.seed_account(from, Account {
            balance: 1000,
            nonce: 0,
        });
        engine.state.seed_account(to, Account {
            balance: 500,
            nonce: 0,
        });

        let initial_from_balance = engine.state.get_account(&from).balance;
        let initial_to_balance = engine.state.get_account(&to).balance;
        let initial_root = engine.state.current_root();

        let tx = make_test_tx(from, to, 250, 0);
        let trace = engine.execute_batch("test", vec![tx]).unwrap();

        // Verify trace public inputs
        assert_eq!(trace.public_inputs.initial_root, initial_root);
        assert_ne!(trace.public_inputs.initial_root, trace.public_inputs.final_root);

        // Verify state diffs reflect changes
        assert_eq!(trace.state_diffs.len(), 2);
        assert_eq!(trace.state_diffs[0].account, from);
        assert_eq!(trace.state_diffs[0].old_balance, initial_from_balance);
        assert_eq!(trace.state_diffs[0].new_balance, initial_from_balance - 250);
        assert_eq!(trace.state_diffs[0].old_nonce, 0);
        assert_eq!(trace.state_diffs[0].new_nonce, 1);

        assert_eq!(trace.state_diffs[1].account, to);
        assert_eq!(trace.state_diffs[1].old_balance, initial_to_balance);
        assert_eq!(trace.state_diffs[1].new_balance, initial_to_balance + 250);
    }

    #[test]
    fn test_e2e_batch_scaling_50_txs() {
        std::env::set_var("ALLOW_UNSIGNED_USER_TXS", "1");
        let temp = TempDir::new().unwrap();

        let mut engine = SimpleTransactionEngine::new(InMemoryStateManager::default());
        let from = [1u8; 20];
        engine.state.seed_account(from, Account {
            balance: 100000,
            nonce: 0,
        });

        let mut txs = Vec::new();
        for i in 0..50 {
            let mut to = [2u8; 20];
            to[0] = to[0].wrapping_add(i as u8);
            txs.push(make_test_tx(from, to, 100, i as u64));
        }

        let trace = engine.execute_batch("scale50", txs).unwrap();
        assert_eq!(trace.executed_transactions.len(), 50);
        assert_eq!(trace.state_diffs.len(), 100); // 50 * 2

        let meta = persist_trace(temp.path(), &trace).unwrap();
        assert!(verify_trace_hash(&meta.trace_path, &meta.sha256_hex).is_ok());
    }

    #[test]
    fn test_e2e_commitment_consistency() {
        std::env::set_var("ALLOW_UNSIGNED_USER_TXS", "1");

        let from = [1u8; 20];
        let to = [2u8; 20];

        // Execute same batch twice
        let mut traces = Vec::new();
        for _ in 0..2 {
            let mut engine = SimpleTransactionEngine::new(InMemoryStateManager::default());
            engine.state.seed_account(from, Account {
                balance: 1000,
                nonce: 0,
            });

            let txs = vec![
                make_test_tx(from, to, 100, 0),
                make_test_tx(from, to, 200, 1),
            ];

            let trace = engine.execute_batch("consistent", txs).unwrap();
            traces.push(trace);
        }

        // Verify commitments are identical
        assert_eq!(
            traces[0].public_inputs.tx_commitment,
            traces[1].public_inputs.tx_commitment
        );
        assert_eq!(
            traces[0].public_inputs.state_diff_commitment,
            traces[1].public_inputs.state_diff_commitment
        );
    }
}
