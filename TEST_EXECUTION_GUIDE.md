# Executor & Prover Component Verification Test Guide

## Overview

This guide walks you through running and interpreting the comprehensive test suite for the executor and prover components. These tests must pass before running the factorial experiment (batch size × sequencing policy × DA mode).

## Test Structure

### A. Executor Tests

Located in: `executor/tests/`

#### `test_tx_engine.rs` — Transaction Engine Verification
- **30+ tests** covering:
  - Signature validation (valid, invalid, unsigned, malformed)
  - Nonce verification (mismatch rejection)
  - Balance verification (insufficient balance rejection)
  - State transitions (single tx, multiple txs, root changes)
  - Batch determinism (same batch → identical traces)
  - Mixed valid/invalid transactions
  - Batch scaling (10, 50 transactions)
  - Dust transfers (1 wei)

**Success Criteria:**
- All signature tests pass (including recovery validation)
- Nonce mismatch → rejected with "invalid_nonce"
- Insufficient balance → rejected with "insufficient_balance"
- State diffs present and merkle proofs included
- Trace determinism verified across runs
- Large batches execute correctly

#### `test_state.rs` — State Manager Verification
- **25+ tests** covering:
  - InMemoryStateManager (seed, get, set, root computation)
  - RocksDbStateManager (persistence, recovery, multiple instances)
  - Account state tracking (balance, nonce)
  - Merkle root changes on state updates
  - Large state (100+ accounts)
  - StateManager trait implementation

**Success Criteria:**
- In-memory state reads/writes correctly
- Persistent state survives process restart
- Root computation reflects state changes
- Merkle proofs included in all diffs
- Large state handles efficiently

#### `test_trace.rs` — Trace Persistence Verification
- **25+ tests** covering:
  - Trace JSON persistence
  - SHA256 hashing and verification
  - File integrity checking
  - Lifecycle tracking (generated → persisted → proved → published)
  - index.jsonl structure and parsing
  - Batch subdirectory organization

**Success Criteria:**
- Traces persist to disk with correct SHA256 hash
- Hash verification passes for uncorrupted traces
- Hash verification fails for corrupted traces
- Lifecycle index tracks state progression
- Multiple traces in same directory organized correctly

#### `test_integration.rs` — End-to-End Flow Verification
- **10+ integration tests** covering:
  - Single tx: execute → persist → verify
  - Multiple txs with determinism verification
  - Mixed valid/invalid txs with rejection tracking
  - Large batches (10, 50 transactions)
  - Lifecycle tracking (all 4 stages)
  - Empty batches
  - Concurrent batch processing
  - State transition verification
  - Commitment consistency

**Success Criteria:**
- E2E pipeline produces valid outputs at each stage
- Same batch produces identical traces and hashes
- Lifecycle index tracks progression correctly
- State transitions verified mathematically
- Commitments (tx and state_diff) deterministic

### B. Prover Tests

Located in: `risc0_prover/tests/`

#### `test_guest_logic.rs` — RISC0 Guest Program Verification
- **40+ tests** covering:
  - LightweightSMT initialization and root tracking
  - Single/multiple diff application
  - Nonce validation (reject decreases)
  - Balance validation (reject simultaneous increase with nonce increase)
  - Merkle proof validation (reject mismatches, missing proofs)
  - Root progression across multiple diffs
  - Deterministic final root computation
  - Edge cases (zero balance, large values, 100 accounts, nonce sequences)

**Success Criteria:**
- SMT correctly applies diffs and updates root
- Invalid nonce transitions rejected
- Invalid balance transitions rejected
- Merkle proofs validated correctly
- Root progression deterministic
- All edge cases handled correctly

## Running the Tests

### Quick Start (All Tests)

```bash
cd /path/to/rollupx-full-zk-rollup

# Option 1: Using provided script
bash run_verification_tests.sh

# Option 2: Manual execution
cd executor
CXXFLAGS='-include cstdint' cargo +nightly-2025-03-19 test \
  --manifest-path executor/src/Cargo.toml \
  -p zksync_state_machine \
  --ignore-rust-version \
  --test-threads=1

cd ../risc0_prover
cargo +nightly test \
  --manifest-path risc0_prover/rollup_core/Cargo.toml \
  -p rollup_core \
  --test-threads=1
```

### Running Specific Test Categories

```bash
# Executor TX Engine tests only
cd executor
cargo +nightly-2025-03-19 test executor_tx_engine --ignore-rust-version

# Executor State tests only
cargo +nightly-2025-03-19 test executor_state --ignore-rust-version

# Executor Trace tests only
cargo +nightly-2025-03-19 test executor_trace --ignore-rust-version

# Executor Integration tests only
cargo +nightly-2025-03-19 test executor_integration --ignore-rust-version

# Prover Guest Logic tests only
cd ../risc0_prover
cargo +nightly test risc0_guest_logic
```

### Running Individual Tests

```bash
# Example: Single test
cd executor
cargo +nightly-2025-03-19 test \
  executor_tx_engine_tests::test_nonce_mismatch_rejected \
  --ignore-rust-version -- --nocapture
```

### Running with Output

```bash
# Show println! output from tests
cargo +nightly-2025-03-19 test -- --nocapture

# Run with backtrace on failure
RUST_BACKTRACE=1 cargo +nightly-2025-03-19 test
```

## Understanding Test Output

### Successful Test Run

```
running 30 tests
test executor_tx_engine_tests::test_unsigned_tx_accepted_with_env_flag ... ok
test executor_tx_engine_tests::test_nonce_mismatch_rejected ... ok
test executor_tx_engine_tests::test_deterministic_trace_from_same_batch ... ok
...

test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured

running 25 tests
test executor_state_tests::test_in_memory_get_default_account ... ok
...

test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured

Total: 120+ tests, ALL PASSED ✓
```

### Failed Test Output

```
test executor_tx_engine_tests::test_nonce_mismatch_rejected ... FAILED

failures:

---- executor_tx_engine_tests::test_nonce_mismatch_rejected stdout ----
thread 'executor_tx_engine_tests::test_nonce_mismatch_rejected' panicked at 
'assertion failed: trace.executed_transactions.len() == 0'
```

**What to check:**
1. Error message (e.g., "assertion failed", "panic")
2. Which test failed
3. Expected vs actual value
4. Stack trace for context

## Test Coverage Report

Generate test coverage report:

```bash
# Install tarpaulin (code coverage tool)
cargo install cargo-tarpaulin

# Generate coverage report
cd executor
cargo +nightly-2025-03-19 tarpaulin \
  --manifest-path executor/src/Cargo.toml \
  -p zksync_state_machine \
  --out Html \
  --ignore-rust-version

# Open coverage/tarpaulin-report.html
```

**Target:** ≥ 80% line coverage for executor and prover.

## Expected Test Results Summary

### Executor Tests (90 tests expected)

| Category | Test Count | Key Assertions |
|----------|-----------|---|
| TX Engine | 30 | Signature validation, nonce/balance checks, state transitions, determinism |
| State Manager | 25 | In-memory/persistent state, root computation, merkle proofs |
| Trace Persistence | 25 | SHA256 verification, lifecycle tracking, file integrity |
| Integration | 10 | E2E flow, mixed batches, commitment consistency |
| **TOTAL** | **90** | All critical paths covered |

### Prover Tests (40 tests expected)

| Category | Test Count | Key Assertions |
|----------|-----------|---|
| Guest Logic | 40 | SMT transitions, diff validation, nonce/balance rules, determinism |
| **TOTAL** | **40** | Core proof logic verified |

**Overall Target:** 130+ tests, **100% pass rate**

## Success Criteria Checklist

- [ ] All 90 executor tests pass
- [ ] All 40 prover tests pass
- [ ] No panics or assertion failures
- [ ] Test execution time < 5 minutes per suite
- [ ] Code coverage ≥ 80%
- [ ] All environment variables set correctly
- [ ] No race conditions detected
- [ ] File I/O operations complete without errors
- [ ] Hash computations deterministic and correct
- [ ] State transitions verified mathematically

## Troubleshooting

### Test Panics

**Problem:** `thread 'test_name' panicked at 'assertion failed'`

**Solution:**
1. Read the assertion message carefully
2. Check the expected vs actual value
3. Verify test setup (seeded accounts, initial state)
4. Run test in isolation with `--nocapture` for debug output

### Environment Variable Issues

**Problem:** `ALLOW_UNSIGNED_USER_TXS` not set

**Solution:**
```bash
export ALLOW_UNSIGNED_USER_TXS=1
cargo test
```

### File System Errors

**Problem:** `mkdir failed: Permission denied`

**Solution:**
1. Verify temp directory writeable
2. Check disk space
3. Run with elevated permissions if needed

### Nightly Toolchain Issues

**Problem:** `cargo: not installed for toolchain nightly-2025-03-19`

**Solution:**
```bash
rustup update nightly-2025-03-19
rustup component add rust-src --toolchain nightly-2025-03-19
```

### RocksDB Build Issues

**Problem:** `linking with cc failed: exit code 1`

**Solution:**
```bash
# Linux
sudo apt-get install librocksdb-dev

# macOS
brew install rocksdb

# Windows
# Ensure MSVC toolchain installed
```

## Next Steps After Verification

Once all tests pass:

1. **Benchmark Baseline**: Record test execution times
2. **State Snapshot**: Save clean state database snapshot
3. **Configuration Lock**: Freeze executor/prover configs
4. **Prepare Factorial Experiment**: Set up batch parameters (10, 50, 100, 500, 1000 txs)
5. **Run Experiment**: Execute full factorial with all policy combinations

## Performance Expectations

| Metric | Expected | Acceptable | Concerning |
|--------|----------|-----------|-----------|
| Executor 10-tx batch | < 100ms | < 200ms | > 500ms |
| Executor 100-tx batch | < 500ms | < 1s | > 2s |
| Executor 1000-tx batch | < 5s | < 10s | > 30s |
| Total test suite time | < 3 min | < 5 min | > 10 min |
| Memory per batch (100 txs) | < 100MB | < 200MB | > 500MB |

## References

- [Test Plan](./EXECUTOR_PROVER_TEST_PLAN.md)
- [Executor README](./executor/README.md)
- [Prover README](./risc0_prover/README.md)
- [System Design](./executor/SYSTEM_DESIGN.md)

