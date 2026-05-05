# Executor & Prover Verification Test Plan

## Objective

Design and implement extensive tests to verify that **executor** and **prover** components work correctly and produce intended outputs before running the factorial experiment (batch size × sequencing policy × DA mode).

## Test Scope

### A. Executor Component Tests

#### A.1 Transaction Engine Tests
- **Signature Verification**
  - Valid ECDSA signatures accept transactions
  - Invalid signatures reject with reason "invalid_signature"
  - Malformed signatures (wrong length, invalid recovery id) reject
  - Unsigned transactions allowed only when `ALLOW_UNSIGNED_USER_TXS=1`

- **Transaction Validation**
  - Nonce mismatch rejects with "invalid_nonce"
  - Insufficient balance rejects with "insufficient_balance"
  - Valid tx updates sender nonce and both balances
  - Correct state transitions for batches of varying sizes

- **State Transitions**
  - Single tx: initial_root → final_root via state_diff
  - Multiple txs: deterministic root progression
  - Sender and receiver state diffs reflect balance/nonce changes
  - Merkle proof included in each diff

- **Batch Determinism**
  - Same batch → identical trace (trace_id may differ, but state_diffs identical)
  - Execution trace fields: initial_root, final_root, tx_commitment, state_diff_commitment
  - Reproducible public_inputs across runs

#### A.2 State Manager Tests
- **InMemoryStateManager**
  - Account creation (balance, nonce initialization)
  - get_account returns correct state
  - set_account updates and returns StateDiff
  - Root computation reflects all state changes

- **RocksDbStateManager**
  - Persistent state across process restarts
  - Recovery from disk
  - Large state (1000+ accounts) correctness

#### A.3 Trace Persistence Tests
- **Trace Lifecycle**
  - Trace persisted to JSON with correct structure
  - SHA256 hash computed and stored
  - index.jsonl records all traces with lifecycle states (generated, persisted, proved, published)
  - Hash verification passes for stored traces

#### A.4 Batch Processing Tests
- **Batch Sizes**
  - Empty batch (0 txs) → trace with no state_diffs
  - Small batch (1–5 txs) → traces execute correctly
  - Large batch (100–1000 txs) → performance and correctness maintained

- **Rejection Handling**
  - Mixed valid/invalid txs in batch
  - Invalid txs recorded in tx_outcomes with rejection_reason
  - Invalid txs do not update state
  - Valid txs in same batch still execute

- **Transaction Mix**
  - All successful txs
  - All failing txs
  - Mixed success/failure
  - Ensure final_root correct for all scenarios

### B. Prover Component Tests

#### B.1 RISC0 Host Prover Tests
- **Artifact Generation**
  - Host invoked with valid BlockTrace JSON
  - Proof file written and not empty
  - Journal file written and not empty
  - Metadata JSON valid and complete

- **Metadata Integrity**
  - trace_sha256 matches input trace hash
  - public_inputs_hash matches (initial_root, final_root) hash
  - journal_sha256 matches generated journal
  - proof_sha256 matches generated proof

- **Output Validation**
  - Proof bytes > 0
  - Journal bytes > 0
  - Proof in valid format (Groth16 or journal_fallback mode)

#### B.2 RISC0 Guest Program Tests
- **State Diff Application**
  - Single diff applied correctly (root update)
  - Multiple diffs applied in order
  - Final root matches expected after all diffs
  - Invalid diffs reject (nonce decrease, balance+nonce increases)

- **Witness Validation**
  - Merkle proof in diff matches current root
  - Diff missing proof rejects
  - Out-of-order proofs detected and rejected

#### B.3 Proof Artifact Tests
- **File Output**
  - All 3 artifact files exist (proof, journal, metadata)
  - No file corruption or truncation
  - Files readable and parseable

- **Artifact Sizes**
  - Journal size reasonable (< 1MB for typical batch)
  - Proof size reasonable (< 10MB for Groth16)
  - Metadata JSON < 1KB

#### B.4 Cross-Component Consistency
- **Executor → Prover**
  - ExecutionTraceV1.public_inputs serialized to BlockTrace
  - StateDiffs converted to guest-format diffs
  - Initial/final roots match in both representations
  - tx_commitment and state_diff_commitment computed identically

### C. Integration Tests

#### C.1 End-to-End Flow
1. Generate randomized batch of transactions
2. Execute via SimpleTransactionEngine
3. Persist trace
4. Invoke RISC0 host with trace
5. Verify proof artifacts
6. Validate metadata hashes
7. Verify final_root matches prover's computed root

#### C.2 Batch Scaling
- Run E2E tests at batch sizes: 10, 50, 100, 500, 1000 txs
- Measure:
  - Execution time
  - Proof generation time
  - Artifact sizes
  - Peak memory usage

#### C.3 Determinism & Reproducibility
- Execute same batch 3 times → identical traces (state_diffs, final_root, commitments)
- Prover on same trace 2 times → identical metadata hashes, proof sizes
- No TOCTOU issues or race conditions

### D. Edge Cases & Error Handling

- **Empty batches**: execution, trace generation, proof generation
- **Dust txs** (1 wei transfers): correct trace, state diffs
- **Large transfers** (u64::MAX balance): overflow handling
- **Max accounts**: state manager performance with 10k+ accounts
- **File system errors**: trace persistence failure recovery
- **Prover timeout**: host process management, cleanup
- **Invalid input JSON**: graceful rejection with errors

---

## Test Infrastructure

### Test Framework
- **Rust**: native `#[test]` + `#[tokio::test]` for async
- **Fixtures**: reusable transaction generators, account seeders
- **Temp Directories**: ephemeral test state (cleaned up after each test)

### Key Test Fixtures
```
- make_test_transaction(from, to, amount, nonce, signature) -> Transaction
- make_test_account(balance, nonce) -> Account
- make_random_batch(size, sequencing_policy) -> Vec<Transaction>
- create_test_state_manager() -> InMemoryStateManager
- create_test_executor() -> SimpleTransactionEngine
```

### Verification Utilities
```
- assert_state_transition_valid(initial_root, final_root, diffs)
- assert_trace_deterministic(batch, num_runs)
- assert_proof_valid(trace, proof_artifacts)
- assert_metadata_integrity(metadata, trace, proof, journal)
```

---

## Test Execution

### Run All Tests
```bash
cd executor
CXXFLAGS='-include cstdint' cargo +nightly-2025-03-19 test \
  --manifest-path Cargo.toml \
  -p zksync_state_machine \
  --ignore-rust-version -- --test-threads=1 --nocapture

cd risc0_prover
cargo +nightly test --all --manifest-path Cargo.toml
```

### Run by Category
```bash
# Transaction engine tests
cargo test executor::tx_engine

# State manager tests
cargo test executor::state

# Trace tests
cargo test executor::trace

# Prover tests
cargo test risc0_prover::proof

# Integration tests
cargo test integration_tests
```

### Test Coverage Report
- Target: **≥ 80% line coverage** for executor, prover
- Focus on critical paths: signature verification, state transitions, proof artifact generation

---

## Success Criteria

✅ All executor transaction validation tests pass
✅ State transitions deterministic and reproducible
✅ Traces persist and verify correctly
✅ RISC0 host generates valid artifacts
✅ Proof metadata integrity verified
✅ Guest program correctly validates state diffs
✅ E2E flow produces consistent results
✅ Batch scaling tests show predictable performance
✅ Edge cases handled gracefully
✅ ≥ 80% code coverage

---

## Next Steps

1. **Phase 1**: Implement unit tests (A.1 – A.3)
2. **Phase 2**: Implement state/proof tests (B.1 – B.3)
3. **Phase 3**: Implement integration tests (C.1 – C.2)
4. **Phase 4**: Performance benchmarking (C.2, D)
5. **Phase 5**: Run full test suite, validate outputs
6. **Phase 6**: Design & run factorial experiment (batch size × policy × DA mode)

