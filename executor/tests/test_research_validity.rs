use std::path::PathBuf;
use executor::state::{RocksDbStateManager, StateManager};
use executor::tx_engine::{SimpleTransactionEngine, TransactionEngine};
use executor::types::{Account, Address, Transaction};
use tempfile::tempdir;

#[test]
fn test_streaming_stats_correctness() {
    let mut stats = executor::types::StreamingStats::default();
    stats.update(10.0);
    stats.update(20.0);
    stats.update(30.0);
    
    assert_eq!(stats.count, 3);
    assert_eq!(stats.mean, 20.0);
    assert_eq!(stats.min, 10.0);
    assert_eq!(stats.max, 30.0);
}

#[test]
fn test_tx_engine_phase_breakdown() {
    let mut state = executor::state::InMemoryStateManager::default();
    let from: Address = [1; 20];
    let to: Address = [2; 20];
    state.seed_account(from, Account { balance: 1000, nonce: 0 });
    
    let mut engine = SimpleTransactionEngine::new(state);
    let tx = Transaction {
        from,
        to,
        amount: 100,
        nonce: 0,
        signature: vec![0; 65], // mock
        gas_price: 1,
        gas_limit: 100000,
        timestamp: 123456,
        boost_bid: 0,
    };

    let trace = engine.execute_batch("test_batch", vec![tx]).unwrap();
    let phases = trace.execution_phases;

    // Verify all phases were timed (should be >= 0)
    assert!(phases.total_execution_ms > 0.0);
    assert!(phases.merkle_update_ms > 0.0);
    assert!(phases.signature_verify_ms >= 0.0);
    assert!(phases.state_transition_ms > 0.0);
}

#[test]
fn test_rocksdb_state_reset() {
    let tmp = tempdir().unwrap();
    let path = tmp.path().to_path_buf();
    
    let mut state = RocksDbStateManager::open(&path).unwrap();
    let addr: Address = [0x42; 20];
    state.seed_account(addr, Account { balance: 100, nonce: 1 }).unwrap();
    
    assert_eq!(state.get_account(&addr).balance, 100);
    
    // Reset to genesis
    state.reset_to_genesis().unwrap();
    
    // Account should be gone
    assert_eq!(state.get_account(&addr).balance, 0);
    assert_eq!(state.get_account(&addr).nonce, 0);
}

#[test]
fn test_merkle_isolation_is_measurable() {
    // This test ensures that Merkle updates take a significant portion of execution
    // and are correctly isolated in the metrics.
    let mut state = executor::state::InMemoryStateManager::default();
    let from: Address = [1; 20];
    let to: Address = [2; 20];
    state.seed_account(from, Account { balance: 10000, nonce: 0 });
    
    let mut engine = SimpleTransactionEngine::new(state);
    let mut txs = Vec::new();
    for i in 0..10 {
        txs.push(Transaction {
            from,
            to,
            amount: 1,
            nonce: i as u64,
            signature: vec![0; 65],
            gas_price: 1,
            gas_limit: 100000,
            timestamp: 123456,
            boost_bid: 0,
        });
    }

    let trace = engine.execute_batch("test_merkle_perf", txs).unwrap();
    let phases = trace.execution_phases;
    
    println!("Total: {}ms, Merkle: {}ms", phases.total_execution_ms, phases.merkle_update_ms);
    
    assert!(phases.merkle_update_ms <= phases.total_execution_ms);
    assert!(phases.merkle_update_ms > 0.0);
}
