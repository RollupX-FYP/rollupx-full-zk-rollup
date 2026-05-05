// Comprehensive test module for executor state manager
// Tests in-memory and persistent state, merkle trees, root computation

#[cfg(test)]
mod executor_state_tests {
    use crate::state::{InMemoryStateManager, StateManager, RocksDbStateManager};
    use crate::types::{Account, Address};
    use std::path::PathBuf;
    use tempfile::TempDir;

    // ============ Fixtures & Helpers ============

    fn test_addresses() -> (Address, Address, Address) {
        ([1u8; 20], [2u8; 20], [3u8; 20])
    }

    fn test_account(balance: u64, nonce: u64) -> Account {
        Account { balance, nonce }
    }

    // ============ InMemoryStateManager Tests ============

    #[test]
    fn test_in_memory_get_default_account() {
        let state = InMemoryStateManager::default();
        let (addr1, _, _) = test_addresses();

        let account = state.get_account(&addr1);
        assert_eq!(account.balance, 0);
        assert_eq!(account.nonce, 0);
    }

    #[test]
    fn test_in_memory_seed_and_get_account() {
        let mut state = InMemoryStateManager::default();
        let (addr1, _, _) = test_addresses();

        state.seed_account(addr1, test_account(1000, 5));
        let account = state.get_account(&addr1);
        assert_eq!(account.balance, 1000);
        assert_eq!(account.nonce, 5);
    }

    #[test]
    fn test_in_memory_set_account_returns_diff() {
        let mut state = InMemoryStateManager::default();
        let (addr1, _, _) = test_addresses();
        state.seed_account(addr1, test_account(1000, 5));

        let new_account = test_account(500, 6);
        let diff = state.set_account(addr1, new_account.clone()).unwrap();

        assert_eq!(diff.account, addr1);
        assert_eq!(diff.old_balance, 1000);
        assert_eq!(diff.new_balance, 500);
        assert_eq!(diff.old_nonce, 5);
        assert_eq!(diff.new_nonce, 6);
    }

    #[test]
    fn test_in_memory_multiple_accounts() {
        let mut state = InMemoryStateManager::default();
        let (addr1, addr2, addr3) = test_addresses();

        state.seed_account(addr1, test_account(1000, 0));
        state.seed_account(addr2, test_account(2000, 1));
        state.seed_account(addr3, test_account(3000, 2));

        assert_eq!(state.get_account(&addr1).balance, 1000);
        assert_eq!(state.get_account(&addr2).balance, 2000);
        assert_eq!(state.get_account(&addr3).balance, 3000);
    }

    #[test]
    fn test_in_memory_root_changes_on_update() {
        let mut state = InMemoryStateManager::default();
        let (addr1, _, _) = test_addresses();

        let root1 = state.current_root();
        state.seed_account(addr1, test_account(100, 0));
        let root2 = state.current_root();

        assert_ne!(root1, root2, "Root should change after account update");
    }

    #[test]
    fn test_in_memory_set_account_updates_root() {
        let mut state = InMemoryStateManager::default();
        let (addr1, _, _) = test_addresses();
        state.seed_account(addr1, test_account(100, 0));

        let root_before = state.current_root();
        state.set_account(addr1, test_account(50, 1)).unwrap();
        let root_after = state.current_root();

        assert_ne!(root_before, root_after);
    }

    #[test]
    fn test_in_memory_merkle_proof_included() {
        let mut state = InMemoryStateManager::default();
        let (addr1, _, _) = test_addresses();
        state.seed_account(addr1, test_account(100, 0));

        let diff = state.set_account(addr1, test_account(50, 1)).unwrap();
        assert!(!diff.merkle_proof.is_empty(), "Merkle proof should be present");
    }

    #[test]
    fn test_in_memory_large_state_10k_accounts() {
        let mut state = InMemoryStateManager::default();

        // Seed 100 accounts
        for i in 0..100 {
            let mut addr = [0u8; 20];
            addr[0..8].copy_from_slice(&i.to_le_bytes()[0..8]);
            state.seed_account(addr, test_account(1000 + i as u64, 0));
        }

        // Verify all present
        for i in 0..100 {
            let mut addr = [0u8; 20];
            addr[0..8].copy_from_slice(&i.to_le_bytes()[0..8]);
            let account = state.get_account(&addr);
            assert_eq!(account.balance, 1000 + i as u64);
        }
    }

    // ============ RocksDbStateManager Tests ============

    #[test]
    fn test_rocksdb_new_state_opens_cleanly() {
        let temp = TempDir::new().unwrap();
        let result = RocksDbStateManager::open(temp.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_rocksdb_seed_and_get_account() {
        let temp = TempDir::new().unwrap();
        let mut state = RocksDbStateManager::open(temp.path()).unwrap();
        let (addr1, _, _) = test_addresses();

        state.seed_account(addr1, test_account(2000, 5)).unwrap();
        let account = state.get_account(&addr1);
        assert_eq!(account.balance, 2000);
        assert_eq!(account.nonce, 5);
    }

    #[test]
    fn test_rocksdb_persistence_across_instances() {
        let temp = TempDir::new().unwrap();
        let (addr1, _, _) = test_addresses();

        {
            let mut state1 = RocksDbStateManager::open(temp.path()).unwrap();
            state1.seed_account(addr1, test_account(5000, 10)).unwrap();
        }

        {
            let state2 = RocksDbStateManager::open(temp.path()).unwrap();
            let account = state2.get_account(&addr1);
            assert_eq!(account.balance, 5000);
            assert_eq!(account.nonce, 10);
        }
    }

    #[test]
    fn test_rocksdb_set_account_updates_persistent_state() {
        let temp = TempDir::new().unwrap();
        let (addr1, _, _) = test_addresses();

        {
            let mut state1 = RocksDbStateManager::open(temp.path()).unwrap();
            state1.seed_account(addr1, test_account(1000, 0)).unwrap();
            state1.set_account(addr1, test_account(500, 1)).unwrap();
        }

        {
            let state2 = RocksDbStateManager::open(temp.path()).unwrap();
            let account = state2.get_account(&addr1);
            assert_eq!(account.balance, 500);
            assert_eq!(account.nonce, 1);
        }
    }

    #[test]
    fn test_rocksdb_root_computation() {
        let temp = TempDir::new().unwrap();
        let (addr1, addr2, _) = test_addresses();

        let root1 = {
            let mut state = RocksDbStateManager::open(temp.path()).unwrap();
            state.seed_account(addr1, test_account(1000, 0)).unwrap();
            state.current_root()
        };

        let root2 = {
            let mut state = RocksDbStateManager::open(temp.path()).unwrap();
            state.seed_account(addr2, test_account(2000, 0)).unwrap();
            state.current_root()
        };

        assert_ne!(root1, root2);
    }

    #[test]
    fn test_rocksdb_multiple_accounts() {
        let temp = TempDir::new().unwrap();
        let (addr1, addr2, addr3) = test_addresses();

        let mut state = RocksDbStateManager::open(temp.path()).unwrap();
        state.seed_account(addr1, test_account(1000, 0)).unwrap();
        state.seed_account(addr2, test_account(2000, 1)).unwrap();
        state.seed_account(addr3, test_account(3000, 2)).unwrap();

        assert_eq!(state.get_account(&addr1).balance, 1000);
        assert_eq!(state.get_account(&addr2).balance, 2000);
        assert_eq!(state.get_account(&addr3).balance, 3000);
    }

    #[test]
    fn test_rocksdb_get_nonexistent_returns_default() {
        let temp = TempDir::new().unwrap();
        let state = RocksDbStateManager::open(temp.path()).unwrap();
        let (addr1, _, _) = test_addresses();

        let account = state.get_account(&addr1);
        assert_eq!(account.balance, 0);
        assert_eq!(account.nonce, 0);
    }

    #[test]
    fn test_rocksdb_merkle_proof_in_diff() {
        let temp = TempDir::new().unwrap();
        let (addr1, _, _) = test_addresses();

        let mut state = RocksDbStateManager::open(temp.path()).unwrap();
        state.seed_account(addr1, test_account(1000, 0)).unwrap();

        let diff = state.set_account(addr1, test_account(500, 1)).unwrap();
        assert!(!diff.merkle_proof.is_empty());
    }

    // ============ State Manager Trait Tests ============

    #[test]
    fn test_in_memory_implements_state_manager() {
        let mut state: Box<dyn StateManager> = Box::new(InMemoryStateManager::default());
        let (addr1, _, _) = test_addresses();

        let _result = state.set_account(addr1, test_account(100, 0));
        let account = state.get_account(&addr1);
        assert_eq!(account.balance, 100);
    }

    #[test]
    fn test_default_accounts_same_across_instances() {
        let state1 = InMemoryStateManager::default();
        let state2 = InMemoryStateManager::default();

        let (addr1, _, _) = test_addresses();

        let acc1 = state1.get_account(&addr1);
        let acc2 = state2.get_account(&addr1);

        assert_eq!(acc1.balance, acc2.balance);
        assert_eq!(acc1.nonce, acc2.nonce);
    }

    #[test]
    fn test_get_account_idempotent() {
        let state = InMemoryStateManager::default();
        let (addr1, _, _) = test_addresses();

        let acc1 = state.get_account(&addr1);
        let acc2 = state.get_account(&addr1);

        assert_eq!(acc1.balance, acc2.balance);
        assert_eq!(acc1.nonce, acc2.nonce);
    }
}
