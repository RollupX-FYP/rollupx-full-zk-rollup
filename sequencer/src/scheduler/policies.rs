//! Scheduling Policies Module
//! 
//! This module implements the Strategy design pattern for transaction scheduling.
//! Each policy determines how normal transactions are ordered within a batch.
//! 
//! # Available Policies
//! 
//! ## 1. FCFS (First-Come-First-Served)
//! - Orders transactions by arrival time
//! - Maintains submission order (no reordering)
//! - **Advantage**: Simple, fair, predictable
//! - **Disadvantage**: No incentive for higher fees
//! - **Best for**: Systems prioritizing simplicity and time-based fairness
//! 
//! ## 2. Fee Priority
//! - Orders transactions by gas price (highest first)
//! - Incentivizes users to pay higher fees
//! - **Advantage**: Revenue maximization, faster confirmation for willing payers
//! - **Disadvantage**: Unfair to low-fee transactions, prone to fee wars
//! - **Best for**: Systems prioritizing throughput and revenue
//! 
//! ## 3. Time-Boost
//! - Divides time into discrete windows (e.g., 5-second slots)
//! - Users bid for priority within their submission window via `boost_bid`
//! - Within each window: sorts by boost_bid, then gas_price, then FCFS
//! - **Advantage**: Predictable latency guarantees, granular fairness
//! - **Disadvantage**: Complex, still favors wealthy users, strategic gaming
//! - **Best for**: Systems needing SLA guarantees with balanced fairness
//! 
//! ## 4. Fair BFT Ordering
//! - Emphasizes timestamp fairness using distributed agreement
//! - Orders strictly by transaction timestamp (earliest first)
//! - **Note**: Current implementation is simplified for single-node sequencer
//! - **Advantage**: MEV-resistant, decentralized, time-fair
//! - **Disadvantage**: Higher overhead, increased latency (in multi-node setup)
//! - **Best for**: Decentralized sequencers prioritizing censorship resistance
//! 
//! # Important Rule
//! All policies only affect **normal user transactions**. Forced transactions
//! from L1 ALWAYS come first, regardless of the selected policy.

use crate::UserTransaction;

/// Scheduling policy trait (Strategy pattern)
/// Defines the interface for all transaction ordering policies.
/// Each policy implements its own `order_transactions()` logic.
pub trait SchedulingPolicy: Send + Sync {
    /// Order transactions according to this policy's rules
    fn order_transactions(&self, transactions: Vec<UserTransaction>) -> Vec<UserTransaction>;
    
    /// Get the policy name for logging and metadata
    fn name(&self) -> &str;
}

/// FCFS (First-Come-First-Served) Policy
/// 
/// Maintains the original submission order. No reordering is performed.
/// This is the simplest and most predictable policy.
pub struct FcfsPolicy;

impl SchedulingPolicy for FcfsPolicy {
    fn order_transactions(&self, transactions: Vec<UserTransaction>) -> Vec<UserTransaction> {
        // FCFS: maintain original order, no sorting needed
        transactions
    }
    
    fn name(&self) -> &str {
        "FCFS"
    }
}

/// Fee Priority Policy
/// 
/// Orders transactions by gas price in descending order (highest fee first).
/// This maximizes sequencer revenue and gives priority to users willing to pay more.
pub struct FeePriorityPolicy;

impl SchedulingPolicy for FeePriorityPolicy {
    fn order_transactions(&self, mut transactions: Vec<UserTransaction>) -> Vec<UserTransaction> {
        // Sort by gas_price in descending order (highest fee first)
        transactions.sort_by(|a, b| b.gas_price.cmp(&a.gas_price));
        transactions
    }
    
    fn name(&self) -> &str {
        "FeePriority"
    }
}

/// Time-Boost Policy
/// 
/// Divides time into discrete windows and allows users to bid for priority
/// within their submission window. Provides more granular fairness than pure
/// fee-priority while still allowing premium payments for faster confirmation.
/// 
/// # Ordering Rules (within each time window)
/// 1. Sort by `boost_bid` (if present) - descending
/// 2. If no boost_bid or tied, sort by `gas_price` - descending  
/// 3. If tied on both, maintain FCFS order
pub struct TimeBoostPolicy {
    /// Time window size in milliseconds (e.g., 5000 for 5-second windows)
    pub time_window_ms: u64,
}

impl SchedulingPolicy for TimeBoostPolicy {
    fn order_transactions(&self, mut transactions: Vec<UserTransaction>) -> Vec<UserTransaction> {
        // Group transactions by time window
        // Time window = floor(timestamp / window_size)
        
        // Sort by multiple criteria:
        // 1. Time window (ascending - earlier windows first)
        // 2. Within same window: boost_bid (descending)
        // 3. Within same boost_bid: gas_price (descending)
        // 4. Maintain stable sort for FCFS tie-breaking
        
        transactions.sort_by(|a, b| {
            // Calculate time windows
            let window_a = a.timestamp / self.time_window_ms;
            let window_b = b.timestamp / self.time_window_ms;
            
            // First, compare by time window
            match window_a.cmp(&window_b) {
                std::cmp::Ordering::Equal => {
                    // Same window: compare by boost_bid
                    let boost_a = a.boost_bid.unwrap_or_default();
                    let boost_b = b.boost_bid.unwrap_or_default();
                    
                    match boost_b.cmp(&boost_a) { // Descending (b vs a)
                        std::cmp::Ordering::Equal => {
                            // Same boost: compare by gas_price
                            b.gas_price.cmp(&a.gas_price) // Descending
                        }
                        other => other,
                    }
                }
                other => other,
            }
        });
        
        transactions
    }
    
    fn name(&self) -> &str {
        "TimeBoost"
    }
}

/// Fair BFT Ordering Policy
/// 
/// Orders transactions strictly by timestamp to provide time-based fairness.
/// This is a simplified implementation for single-node sequencers.
/// 
/// # Multi-Node BFT Extension
/// For a full Byzantine Fault Tolerant implementation with multiple sequencer nodes:
/// 
/// 1. **Distributed Timestamp Agreement**:
///    - Use a BFT consensus protocol (e.g., HotStuff, Tendermint, PBFT)
///    - Validator set agrees on canonical transaction timestamps
///    - Requires 2f+1 validators to tolerate f Byzantine faults
/// 
/// 2. **Transaction Gossip**:
///    - Transactions broadcast to all validator nodes
///    - Each validator assigns local timestamp on receipt
///    - Consensus round determines canonical timestamp
/// 
/// 3. **Ordering Consensus**:
///    - Validators propose transaction batches with timestamps
///    - BFT consensus determines final ordering
///    - Threshold signatures prove agreement
/// 
/// 4. **MEV Resistance**:
///    - Time-based ordering reduces front-running opportunities
///    - No single sequencer can manipulate order
///    - Encrypted mempool can further enhance fairness
/// 
/// # Current Implementation
/// Orders by transaction timestamp field (single-node, no consensus).
pub struct FairBftPolicy;

impl SchedulingPolicy for FairBftPolicy {
    fn order_transactions(&self, mut transactions: Vec<UserTransaction>) -> Vec<UserTransaction> {
        // Sort strictly by timestamp (ascending - earliest first)
        // This provides time-based fairness
        transactions.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        transactions
    }
    
    fn name(&self) -> &str {
        "FairBFT"
    }
}

/// Policy type enum for configuration
/// 
/// Allows easy policy selection via configuration files or API.
/// Used by the factory function to create policy instances.
#[derive(Debug, Clone)]
pub enum SchedulingPolicyType {
    /// First-Come-First-Served (maintain submission order)
    Fcfs,
    /// Fee Priority (highest gas price first)
    FeePriority,
    /// Time-Boost with configurable time window
    TimeBoost { 
        /// Time window size in milliseconds
        time_window_ms: u64 
    },
    /// Fair BFT Ordering (timestamp-based)
    FairBft,
}

/// Factory function to create policy instances
/// 
/// # Arguments
/// * `policy_type` - The type of policy to create
/// 
/// # Returns
/// A boxed trait object implementing `SchedulingPolicy`
/// 
/// # Example
/// ```
/// use sequencer::scheduler::{create_policy, SchedulingPolicyType};
/// 
/// let policy = create_policy(SchedulingPolicyType::FeePriority);
/// let ordered = policy.order_transactions(transactions);
/// ```
pub fn create_policy(policy_type: SchedulingPolicyType) -> Box<dyn SchedulingPolicy> {
    match policy_type {
        SchedulingPolicyType::Fcfs => Box::new(FcfsPolicy),
        SchedulingPolicyType::FeePriority => Box::new(FeePriorityPolicy),
        SchedulingPolicyType::TimeBoost { time_window_ms } => {
            Box::new(TimeBoostPolicy { time_window_ms })
        }
        SchedulingPolicyType::FairBft => Box::new(FairBftPolicy),
    }
}