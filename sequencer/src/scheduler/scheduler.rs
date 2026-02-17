//! Transaction Scheduler Module
//! 
//! This module implements transaction scheduling using the Strategy design pattern.
//! The scheduler delegates transaction ordering to pluggable policy implementations.
//! 
//! # Supported Policies
//! - **FCFS** (First-Come-First-Served): Transactions ordered by arrival time
//! - **FeePriority**: Transactions ordered by gas price (highest first)
//! - **TimeBoost**: Time-windowed ordering with premium bids
//! - **FairBFT**: Timestamp-based fair ordering (Byzantine Fault Tolerant)
//! 
//! # Important Rule
//! Forced transactions from L1 ALWAYS come first, regardless of policy.
//! Only normal transactions are reordered based on the selected policy.

use crate::{UserTransaction, ForcedTransaction, Transaction};
use super::policies::SchedulingPolicy;

/// Transaction scheduler
/// 
/// Determines the order of transactions within a batch based on a pluggable
/// scheduling policy. Forced transactions always have priority regardless of the policy.
/// 
/// # Strategy Pattern
/// The scheduler uses the Strategy design pattern by holding a trait object
/// (`Box<dyn SchedulingPolicy>`) that implements transaction ordering logic.
/// This allows runtime policy selection and easy addition of new policies.
pub struct Scheduler {
    /// Scheduling policy implementation (trait object for runtime polymorphism)
    policy: Box<dyn SchedulingPolicy>,
}

impl Scheduler {
    /// Creates a new scheduler with the specified policy
    /// 
    /// # Arguments
    /// * `policy` - Boxed trait object implementing `SchedulingPolicy`
    /// 
    /// # Example
    /// ```
    /// use sequencer::scheduler::{Scheduler, create_policy, SchedulingPolicyType};
    /// 
    /// let policy = create_policy(SchedulingPolicyType::FeePriority);
    /// let scheduler = Scheduler::new(policy);
    /// ```
    pub fn new(policy: Box<dyn SchedulingPolicy>) -> Self {
        Self { policy }
    }
    
    /// Schedule transactions for a batch
    /// 
    /// Combines forced and normal transactions into a single ordered list.
    /// 
    /// # Ordering Rules
    /// 1. ALL forced transactions come first (maintain L1 order)
    /// 2. Normal transactions follow, ordered by the selected policy
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
        
        // Step 2: Delegate normal transaction ordering to the policy
        let ordered_normal = self.policy.order_transactions(normal);
        
        // Add all ordered normal transactions to the result
        for tx in ordered_normal {
            result.push(Transaction::Normal(tx));
        }
        
        result
    }
    
    /// Get the name of the current scheduling policy
    /// 
    /// # Returns
    /// Policy name string for logging and metadata
    pub fn policy_name(&self) -> &str {
        self.policy.name()
    }
}