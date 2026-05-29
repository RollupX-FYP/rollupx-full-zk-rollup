// Comprehensive test module for RISC0 prover guest logic
// Tests state diff verification, merkle proofs, and state transitions

#[cfg(test)]
mod risc0_guest_logic_tests {
    use rollup_core::{BlockTrace, LightweightSMT, StateDiff, VerifyError};
    use sha2::{Digest, Sha256};

    // ============ Fixtures & Helpers ============

    fn zero_hash() -> [u8; 32] {
        [0u8; 32]
    }

    fn hash_value(val: u64) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(val.to_be_bytes());
        hasher.finalize().into()
    }

    fn fold_diff(prev_root: [u8; 32], diff: &StateDiff) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(prev_root);
        hasher.update(diff.account);
        hasher.update(diff.old_balance.to_be_bytes());
        hasher.update(diff.new_balance.to_be_bytes());
        hasher.update(diff.old_nonce.to_be_bytes());
        hasher.update(diff.new_nonce.to_be_bytes());
        hasher.finalize().into()
    }

    fn make_diff(
        account: [u8; 20],
        old_balance: u64,
        new_balance: u64,
        old_nonce: u64,
        new_nonce: u64,
        merkle_proof: Vec<[u8; 32]>,
    ) -> StateDiff {
        StateDiff {
            account,
            old_balance,
            new_balance,
            old_nonce,
            new_nonce,
            merkle_proof,
        }
    }

    // ============ Lightweight SMT Tests ============

    #[test]
    fn test_smt_new_initializes_root() {
        let root = hash_value(42);
        let smt = LightweightSMT::new(root);
        assert_eq!(smt.current_root(), root);
    }

    #[test]
    fn test_smt_apply_single_diff_updates_root() {
        let initial_root = hash_value(1);
        let mut smt = LightweightSMT::new(initial_root);

        let diff = make_diff(
            [1u8; 20],
            1000,
            500,
            0,
            1,
            vec![initial_root],
        );

        let result = smt.apply_diff(&diff);
        assert!(result.is_ok());

        let new_root = smt.current_root();
        assert_ne!(new_root, initial_root);
        assert_eq!(new_root, fold_diff(initial_root, &diff));
    }

    #[test]
    fn test_smt_apply_multiple_diffs_in_sequence() {
        let mut root = hash_value(1);
        let mut smt = LightweightSMT::new(root);

        for i in 0..5 {
            let diff = make_diff(
                [i as u8; 20],
                1000,
                500,
                0,
                1,
                vec![root],
            );

            assert!(smt.apply_diff(&diff).is_ok());
            root = fold_diff(root, &diff);
            assert_eq!(smt.current_root(), root);
        }
    }

    #[test]
    fn test_smt_reject_nonce_decrease() {
        let initial_root = hash_value(1);
        let mut smt = LightweightSMT::new(initial_root);

        // Nonce goes from 5 to 3 (invalid decrease)
        let diff = make_diff(
            [1u8; 20],
            1000,
            500,
            5,
            3,
            vec![initial_root],
        );

        let result = smt.apply_diff(&diff);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "invalid old nonce");
    }

    #[test]
    fn test_smt_reject_balance_and_nonce_increase() {
        let initial_root = hash_value(1);
        let mut smt = LightweightSMT::new(initial_root);

        // Both balance and nonce increase (invalid debit)
        let diff = make_diff(
            [1u8; 20],
            1000,
            2000, // balance increase
            0,
            1,    // nonce increase
            vec![initial_root],
        );

        let result = smt.apply_diff(&diff);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "balance increased while nonce increased");
    }

    #[test]
    fn test_smt_allow_balance_decrease_with_nonce_increase() {
        let initial_root = hash_value(1);
        let mut smt = LightweightSMT::new(initial_root);

        // Balance decreases, nonce increases (valid)
        let diff = make_diff(
            [1u8; 20],
            1000,
            500, // balance decrease
            0,
            1,   // nonce increase
            vec![initial_root],
        );

        let result = smt.apply_diff(&diff);
        assert!(result.is_ok());
    }

    #[test]
    fn test_smt_allow_balance_increase_with_nonce_same() {
        let initial_root = hash_value(1);
        let mut smt = LightweightSMT::new(initial_root);

        // Balance increases, nonce stays same (valid)
        let diff = make_diff(
            [1u8; 20],
            1000,
            2000, // balance increase
            0,
            0,    // nonce same
            vec![initial_root],
        );

        let result = smt.apply_diff(&diff);
        assert!(result.is_ok());
    }

    #[test]
    fn test_smt_reject_invalid_merkle_proof() {
        let initial_root = hash_value(1);
        let mut smt = LightweightSMT::new(initial_root);

        let wrong_root = hash_value(999);
        let diff = make_diff(
            [1u8; 20],
            1000,
            500,
            0,
            1,
            vec![wrong_root], // doesn't match current root
        );

        let result = smt.apply_diff(&diff);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "invalid witness");
    }

    #[test]
    fn test_smt_reject_missing_merkle_proof() {
        let initial_root = hash_value(1);
        let mut smt = LightweightSMT::new(initial_root);

        let diff = make_diff(
            [1u8; 20],
            1000,
            500,
            0,
            1,
            vec![], // empty proof
        );

        let result = smt.apply_diff(&diff);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "invalid witness");
    }

    #[test]
    fn test_smt_proof_only_uses_first_element() {
        let initial_root = hash_value(1);
        let mut smt = LightweightSMT::new(initial_root);

        // Multiple proof elements, but only first is validated
        let diff = make_diff(
            [1u8; 20],
            1000,
            500,
            0,
            1,
            vec![initial_root, hash_value(2), hash_value(3)],
        );

        let result = smt.apply_diff(&diff);
        assert!(result.is_ok());
    }

    // ============ BlockTrace Processing Tests ============

    #[test]
    fn test_trace_single_diff_applies_correctly() {
        let initial_root = hash_value(1);
        let diff = make_diff(
            [1u8; 20],
            1000,
            500,
            0,
            1,
            vec![initial_root],
        );

        let mut smt = LightweightSMT::new(initial_root);
        assert!(smt.apply_diff(&diff).is_ok());

        let expected_final = fold_diff(initial_root, &diff);
        assert_eq!(smt.current_root(), expected_final);
    }

    #[test]
    fn test_trace_multiple_diffs_reproduce_final_root() {
        let mut root = hash_value(1);
        let mut diffs = Vec::new();

        // Create chain of diffs
        for i in 0..3 {
            let diff = make_diff(
                [i as u8; 20],
                1000,
                900 - (i * 10) as u64,
                0,
                1,
                vec![root],
            );
            diffs.push(diff.clone());
            root = fold_diff(root, &diff);
        }

        // Now verify by applying all diffs
        let mut smt = LightweightSMT::new(hash_value(1));
        for diff in diffs {
            assert!(smt.apply_diff(&diff).is_ok());
        }

        assert_eq!(smt.current_root(), root);
    }

    #[test]
    fn test_trace_empty_diffs_root_unchanged() {
        let initial_root = hash_value(1);
        let mut smt = LightweightSMT::new(initial_root);

        let trace = BlockTrace {
            batch_id: "empty".to_string(),
            initial_root,
            final_root: initial_root,
            state_diffs: vec![],
        };

        for diff in &trace.state_diffs {
            assert!(smt.apply_diff(diff).is_ok());
        }

        assert_eq!(smt.current_root(), trace.final_root);
    }

    #[test]
    fn test_trace_deterministic_final_root() {
        let mut root = hash_value(1);
        let mut diffs = Vec::new();

        for i in 0..5 {
            let diff = make_diff(
                [i as u8; 20],
                1000,
                500,
                0,
                1,
                vec![root],
            );
            diffs.push(diff.clone());
            root = fold_diff(root, &diff);
        }

        // Verify determinism by applying same diffs twice
        let mut smt1 = LightweightSMT::new(hash_value(1));
        let mut smt2 = LightweightSMT::new(hash_value(1));

        for diff in &diffs {
            smt1.apply_diff(diff).unwrap();
            smt2.apply_diff(diff).unwrap();
        }

        assert_eq!(smt1.current_root(), smt2.current_root());
        assert_eq!(smt1.current_root(), root);
    }

    // ============ Edge Cases ============

    #[test]
    fn test_nonce_zero_to_zero() {
        let initial_root = hash_value(1);
        let mut smt = LightweightSMT::new(initial_root);

        let diff = make_diff(
            [1u8; 20],
            1000,
            500,
            0,
            0, // nonce stays 0
            vec![initial_root],
        );

        let result = smt.apply_diff(&diff);
        assert!(result.is_ok());
    }

    #[test]
    fn test_large_balance_values() {
        let initial_root = hash_value(1);
        let mut smt = LightweightSMT::new(initial_root);

        let diff = make_diff(
            [1u8; 20],
            u64::MAX,
            u64::MAX / 2,
            0,
            1,
            vec![initial_root],
        );

        assert!(smt.apply_diff(&diff).is_ok());
    }

    #[test]
    fn test_many_accounts_single_batch() {
        let mut root = hash_value(1);
        let mut smt = LightweightSMT::new(root);

        // Process many account updates
        for i in 0..100 {
            let diff = make_diff(
                [i as u8; 20],
                1000,
                500,
                0,
                1,
                vec![root],
            );
            assert!(smt.apply_diff(&diff).is_ok());
            root = fold_diff(root, &diff);
        }

        assert_eq!(smt.current_root(), root);
    }

    #[test]
    fn test_nonce_increment_sequence() {
        let mut root = hash_value(1);
        let mut smt = LightweightSMT::new(root);

        // Same account, nonce increments over time
        for nonce in 0..10 {
            let diff = make_diff(
                [1u8; 20],
                1000 - (nonce * 10) as u64,
                1000 - ((nonce + 1) * 10) as u64,
                nonce as u64,
                (nonce + 1) as u64,
                vec![root],
            );
            assert!(smt.apply_diff(&diff).is_ok());
            root = fold_diff(root, &diff);
        }

        assert_eq!(smt.current_root(), root);
    }

    #[test]
    fn test_zero_balance_transfer() {
        let initial_root = hash_value(1);
        let mut smt = LightweightSMT::new(initial_root);

        // Zero-value transfer
        let diff = make_diff(
            [1u8; 20],
            1000,
            1000, // balance unchanged
            0,
            1,
            vec![initial_root],
        );

        assert!(smt.apply_diff(&diff).is_ok());
    }

    #[test]
    fn test_concurrent_account_updates() {
        let mut root = hash_value(1);
        let mut diffs = Vec::new();

        for i in 0..10 {
            let diff = make_diff(
                [i as u8; 20],
                (i as u64) * 1000,
                (i as u64) * 500,
                0,
                1,
                vec![root],
            );
            diffs.push(diff.clone());
            root = fold_diff(root, &diff);
        }

        let mut smt = LightweightSMT::new(hash_value(1));
        for diff in diffs {
            assert!(smt.apply_diff(&diff).is_ok());
        }

        assert_eq!(smt.current_root(), root);
    }
}

#[test]
fn test_guest_logic_rejects_empty_merkle_proof() {
    let initial_root = zero_hash();
    let mut smt = LightweightSMT::new(initial_root);

    // Empty merkle proof should be rejected
    let diff = make_diff(
        [1u8; 20],
        1000,
        500,
        0,
        1,
        vec![],
    );
    let result = smt.apply_diff(&diff);
    assert!(result.is_err(), "Expected error for empty merkle_proof");
}

#[test]
fn test_guest_logic_accepts_single_valid_proof() {
    let initial_root = zero_hash();
    let mut smt = LightweightSMT::new(initial_root);

    // Valid minimal merkle proof with a single hash
    let diff = make_diff(
        [2u8; 20],
        1000,
        500,
        0,
        1,
        vec![initial_root],
    );
    let result = smt.apply_diff(&diff);
    assert!(result.is_ok(), "Expected valid diff with single-proof element");
}
