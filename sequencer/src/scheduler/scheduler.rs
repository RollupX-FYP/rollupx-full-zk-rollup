//! Transaction Scheduler Module
//! 
//! This module implements transaction scheduling policies that determine
//! the order of transactions within a batch.
//! 
//! # Supported Policies
//! - **FCFS** (First-Come-First-Served): Transactions ordered by arrival time
//! - **FeePriority**: Transactions ordered by gas price (highest first)
//! 
//! # Important Rule
//! Forced transactions from L1 ALWAYS come first, regardless of policy.
//! Only normal transactions are reordered based on the selected policy.

use crate::{UserTransaction, ForcedTransaction, Transaction};

/// Transaction scheduler
/// 
/// Determines the order of transactions within a batch based on a configured policy.
/// Forced transactions always have priority regardless of the policy.
pub struct Scheduler {
    /// Scheduling policy: "FCFS" or "FeePriority"
    policy: String,
}

impl Scheduler {
    /// Creates a new scheduler with the specified policy
    /// 
    /// # Arguments
    /// * `policy` - Scheduling policy ("FCFS" or "FeePriority")
    pub fn new(policy: String) -> Self {
        Self { policy }
    }
    
    /// Schedule transactions for a batch
    /// 
    /// Combines forced and normal transactions into a single ordered list.
    /// 
    /// # Ordering Rules
    /// 1. ALL forced transactions come first (maintain L1 order)
    /// 2. Normal transactions follow, ordered by the selected policy:
    ///    - **FCFS**: Maintain arrival order (no sorting)
    ///    - **FeePriority**: Sort by gas_price (highest to lowest)
    /// 
    /// # Arguments
    /// * `forced` - Forced transactions from L1
    /// * `normal` - Normal user transactions from the pool
    /// 
    /// # Returns
    /// An ordered list of transactions ready for batching
    pub fn schedule(
        &self,
        forced: Vec<ForcedTransaction>,
        normal: Vec<UserTransaction>,
    ) -> Vec<Transaction> {
        let mut result = Vec::new();
        
        // Step 1: Add ALL forced transactions first
        // This ensures censorship resistance - L1 transactions cannot be reordered
        for tx in forced {
            result.push(Transaction::Forced(tx));
        }
        
        // Step 2: Add normal transactions according to the policy
        let mut sorted = normal;
        if self.policy == "FeePriority" {
            // Sort by gas price in descending order (highest fee first)
            // This incentivizes users to pay higher fees for faster inclusion
            sorted.sort_by(|a, b| b.gas_price.cmp(&a.gas_price));
        }
        // If policy is "FCFS", we keep the original order (no sorting needed)
        
        // Add all normal transactions to the result
        for tx in sorted {
            result.push(Transaction::Normal(tx));
        }
        
        result
    }
}