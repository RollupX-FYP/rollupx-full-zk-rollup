//! Tests for scheduling policies
//! 
//! Comprehensive test suite verifying the behavior of all scheduling policies

#[cfg(test)]
mod tests {
    use crate::{
        scheduler::{
            SchedulingPolicy, FcfsPolicy, FeePriorityPolicy, TimeBoostPolicy, FairBftPolicy,
            SchedulingPolicyType, create_policy, Scheduler,
        },
        UserTransaction, ForcedTransaction, Transaction, ForcedEventType,
    };
    use ethers::types::{Address, U256, Signature, H256};

    /// Helper function to create a test user transaction
    fn create_test_tx(
        nonce: u64,
        gas_price: u64,
        timestamp: u64,
        boost_bid: Option<u64>,
    ) -> UserTransaction {
        UserTransaction {
            from: Address::zero(),
            to: Address::zero(),
            value: U256::from(1000),
            nonce,
            gas_price: U256::from(gas_price),
            signature: Signature::default(),
            timestamp,
            boost_bid: boost_bid.map(U256::from),
        }
    }

    /// Helper function to create a test forced transaction
    fn create_forced_tx(nonce: u64) -> ForcedTransaction {
        ForcedTransaction {
            tx_hash: H256::zero(),
            from: Address::zero(),
            to: Address::zero(),
            value: U256::from(1000),
            nonce,
            l1_tx_hash: H256::zero(),
            l1_block_number: 1,
            event_type: ForcedEventType::Deposit,
            timestamp: 0,
        }
    }

    #[test]
    fn test_fcfs_policy_maintains_order() {
        let policy = FcfsPolicy;
        
        // Create transactions with different gas prices but sequential timestamps
        let txs = vec![
            create_test_tx(1, 100, 1000, None),
            create_test_tx(2, 500, 2000, None),  // Higher gas price
            create_test_tx(3, 50, 3000, None),   // Lower gas price
        ];
        
        let ordered = policy.order_transactions(txs.clone());
        
        // FCFS should maintain original order
        assert_eq!(ordered.len(), 3);
        assert_eq!(ordered[0].nonce, 1);
        assert_eq!(ordered[1].nonce, 2);
        assert_eq!(ordered[2].nonce, 3);
    }

    #[test]
    fn test_fee_priority_orders_by_gas_price() {
        let policy = FeePriorityPolicy;
        
        // Create transactions with different gas prices
        let txs = vec![
            create_test_tx(1, 100, 1000, None),
            create_test_tx(2, 500, 2000, None),  // Highest gas price
            create_test_tx(3, 50, 3000, None),   // Lowest gas price
            create_test_tx(4, 300, 4000, None),  // Medium gas price
        ];
        
        let ordered = policy.order_transactions(txs);
        
        // Should be ordered by gas price (highest first)
        assert_eq!(ordered.len(), 4);
        assert_eq!(ordered[0].gas_price, U256::from(500)); // nonce 2
        assert_eq!(ordered[1].gas_price, U256::from(300)); // nonce 4
        assert_eq!(ordered[2].gas_price, U256::from(100)); // nonce 1
        assert_eq!(ordered[3].gas_price, U256::from(50));  // nonce 3
    }

    #[test]
    fn test_time_boost_groups_by_window() {
        let policy = TimeBoostPolicy {
            time_window_ms: 5000, // 5-second windows
        };
        
        // Create transactions in different time windows
        let txs = vec![
            create_test_tx(1, 100, 1000, None),  // Window 0 (0-4999ms)
            create_test_tx(2, 200, 6000, None),  // Window 1 (5000-9999ms)
            create_test_tx(3, 300, 12000, None), // Window 2 (10000-14999ms)
            create_test_tx(4, 150, 2000, None),  // Window 0 (0-4999ms)
        ];
        
        let ordered = policy.order_transactions(txs);
        
        // Should process window 0 first, then window 1, then window 2
        assert_eq!(ordered.len(), 4);
        assert_eq!(ordered[0].timestamp / 5000, 0); // Window 0
        assert_eq!(ordered[1].timestamp / 5000, 0); // Window 0
        assert_eq!(ordered[2].timestamp / 5000, 1); // Window 1
        assert_eq!(ordered[3].timestamp / 5000, 2); // Window 2
    }

    #[test]
    fn test_time_boost_prioritizes_boost_bids() {
        let policy = TimeBoostPolicy {
            time_window_ms: 5000,
        };
        
        // Create transactions in same time window with different boost bids
        let txs = vec![
            create_test_tx(1, 100, 1000, None),       // No boost
            create_test_tx(2, 100, 2000, Some(500)),  // High boost
            create_test_tx(3, 100, 3000, Some(200)),  // Medium boost
            create_test_tx(4, 100, 4000, Some(800)),  // Highest boost
        ];
        
        let ordered = policy.order_transactions(txs);
        
        // Within same window, should order by boost_bid (highest first)
        assert_eq!(ordered.len(), 4);
        assert_eq!(ordered[
0].boost_bid, Some(U256::from(800))); // nonce 4
        assert_eq!(ordered[1].boost_bid, Some(U256::from(500))); // nonce 2
        assert_eq!(ordered[2].boost_bid, Some(U256::from(200))); // nonce 3
        assert_eq!(ordered[3].boost_bid, None);                  // nonce 1
    }

    #[test]
    fn test_time_boost_falls_back_to_gas_price() {
        let policy = TimeBoostPolicy {
            time_window_ms: 5000,
        };
        
        // Create transactions with same boost bid but different gas prices
        let txs = vec![
            create_test_tx(1, 100, 1000, Some(500)),
            create_test_tx(2, 300, 2000, Some(500)), // Same boost, higher gas
            create_test_tx(3, 200, 3000, Some(500)), // Same boost, medium gas
        ];
        
        let ordered = policy.order_transactions(txs);
        
        // Should fall back to gas_price when boost_bid is equal
        assert_eq!(ordered.len(), 3);
        assert_eq!(ordered[0].gas_price, U256::from(300)); // nonce 2
        assert_eq!(ordered[1].gas_price, U256::from(200)); // nonce 3
        assert_eq!(ordered[2].gas_price, U256::from(100)); // nonce 1
    }

    #[test]
    fn test_fair_bft_orders_by_timestamp() {
        let policy = FairBftPolicy;
        
        // Create transactions with different timestamps
        let txs = vec![
            create_test_tx(1, 500, 5000, None),  // Later timestamp
            create_test_tx(2, 100, 1000, None),  // Earliest
            create_test_tx(3, 300, 3000, None),  // Middle
        ];
        
        let ordered = policy.order_transactions(txs);
        
        // Should be ordered by timestamp (earliest first)
        assert_eq!(ordered.len(), 3);
        assert_eq!(ordered[0].timestamp, 1000); // nonce 2
        assert_eq!(ordered[1].timestamp, 3000); // nonce 3
        assert_eq!(ordered[2].timestamp, 5000); // nonce 1
    }

    #[test]
    fn test_scheduler_forced_transactions_always_first() {
        let policy = create_policy(SchedulingPolicyType::FeePriority);
        let scheduler = Scheduler::new(policy);
        
        // Create forced and normal transactions
        let forced = vec![
            create_forced_tx(100),
            create_forced_tx(101),
        ];
        
        let normal = vec![
            create_test_tx(1, 1000, 1000, None), // Very high gas price
            create_test_tx(2, 500, 2000, None),
        ];
        
        let ordered = scheduler.schedule(forced, normal);
        
        // Verify forced transactions come first
        assert_eq!(ordered.len(), 4);
        match &ordered[0] {
            Transaction::Forced(tx) => assert_eq!(tx.nonce, 100),
            _ => panic!("Expected forced transaction first"),
        }
        match &ordered[1] {
            Transaction::Forced(tx) => assert_eq!(tx.nonce, 101),
            _ => panic!("Expected forced transaction second"),
        }
        
        // Normal transactions should follow, ordered by gas price
        match &ordered[2] {
            Transaction::Normal(tx) => assert_eq!(tx.gas_price, U256::from(1000)),
            _ => panic!("Expected normal transaction third"),
        }
        match &ordered[3] {
            Transaction::Normal(tx) => assert_eq!(tx.gas_price, U256::from(500)),
            _ => panic!("Expected normal transaction fourth"),
        }
    }

    #[test]
    fn test_policy_factory_creates_correct_instances() {
        // Test FCFS creation
        let fcfs = create_policy(SchedulingPolicyType::Fcfs);
        assert_eq!(fcfs.name(), "FCFS");
        
        // Test FeePriority creation
        let fee = create_policy(SchedulingPolicyType::FeePriority);
        assert_eq!(fee.name(), "FeePriority");
        
        // Test TimeBoost creation
        let time_boost = create_policy(SchedulingPolicyType::TimeBoost { time_window_ms: 3000 });
        assert_eq!(time_boost.name(), "TimeBoost");
        
        // Test FairBFT creation
        let fair_bft = create_policy(SchedulingPolicyType::FairBft);
        assert_eq!(fair_bft.name(), "FairBFT");
    }

    #[test]
    fn test_policy_switching() {
        // Create transactions
        let txs = vec![
            create_test_tx(1, 100, 1000, None),
            create_test_tx(2, 500, 2000, None),
            create_test_tx(3, 50, 3000, None),
        ];
        
        // Test with FCFS policy
        let fcfs_policy = create_policy(SchedulingPolicyType::Fcfs);
        let fcfs_ordered = fcfs_policy.order_transactions(txs.clone());
        assert_eq!(fcfs_ordered[0].nonce, 1); // Original order
        
        // Test with FeePriority policy
        let fee_policy = create_policy(SchedulingPolicyType::FeePriority);
        let fee_ordered = fee_policy.order_transactions(txs.clone());
        assert_eq!(fee_ordered[0].gas_price, U256::from(500)); // Highest fee first
        
        // Test with FairBFT policy
        let bft_policy = create_policy(SchedulingPolicyType::FairBft);
        let bft_ordered = bft_policy.order_transactions(txs.clone());
        assert_eq!(bft_ordered[0].timestamp, 1000); // Earliest timestamp first
    }

    #[test]
    fn test_empty_transaction_list() {
        let policy = FeePriorityPolicy;
        let txs = vec![];
        let ordered = policy.order_transactions(txs);
        assert_eq!(ordered.len(), 0);
    }

    #[test]
    fn test_single_transaction() {
        let policy = TimeBoostPolicy { time_window_ms: 5000 };
        let txs = vec![create_test_tx(1, 100, 1000, None)];
        let ordered = policy.order_transactions(txs);
        assert_eq!(ordered.len(), 1);
        assert_eq!(ordered[0].nonce, 1);
    }
}
