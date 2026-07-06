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
//! ## 5. Blob Packing
//! - Orders transactions to favor tighter DA blob utilization
//! - Sorts by estimated encoded size first so larger transactions are packed early
//! - Falls back to gas price and FCFS tie-breaking
//! - **Advantage**: Better blob fill ratio and lower DA waste on mixed payloads
//! - **Disadvantage**: Slightly more scheduling complexity
//! 
//! # Important Rule
//! All policies only affect **normal user transactions**. Forced transactions
//! from L1 ALWAYS come first, regardless of the selected policy.

use crate::PooledTransaction;
use std::collections::HashMap;

/// Scheduling policy trait (Strategy pattern)
/// Defines the interface for all transaction ordering policies.
/// Each policy implements its own `order_transactions()` logic.
pub trait SchedulingPolicy: Send + Sync {
    /// Order transactions according to this policy's rules
    fn order_transactions(&self, transactions: Vec<PooledTransaction>) -> Vec<PooledTransaction>;
    
    /// Get the policy name for logging and metadata
    fn name(&self) -> &str;

    /// Get policy configuration parameters for metrics recording
    fn config_params(&self) -> serde_json::Value {
        serde_json::json!({})
    }
}

/// FCFS (First-Come-First-Served) Policy
/// 
/// Maintains the original submission order. No reordering is performed.
/// This is the simplest and most predictable policy.
pub struct FcfsPolicy;

impl SchedulingPolicy for FcfsPolicy {
    fn order_transactions(&self, transactions: Vec<PooledTransaction>) -> Vec<PooledTransaction> {
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
    fn order_transactions(&self, transactions: Vec<PooledTransaction>) -> Vec<PooledTransaction> {
        let mut grouped: HashMap<ethers::types::Address, Vec<PooledTransaction>> = HashMap::new();
        for tx in transactions {
            grouped.entry(tx.tx.from).or_default().push(tx);
        }
        for queue in grouped.values_mut() {
            queue.sort_by_key(|t| t.tx.nonce);
        }

        let mut ordered = Vec::new();
        while !grouped.is_empty() {
            let mut best_sender = None;
            for (sender, queue) in &grouped {
                if let Some(next_tx) = queue.first() {
                    match best_sender {
                        None => {
                            best_sender = Some((*sender, next_tx));
                        }
                        Some((_, best_tx)) => {
                            match next_tx.tx.gas_price.cmp(&best_tx.tx.gas_price) {
                                std::cmp::Ordering::Greater => {
                                    best_sender = Some((*sender, next_tx));
                                }
                                std::cmp::Ordering::Equal => {
                                    if next_tx.tx.timestamp < best_tx.tx.timestamp {
                                        best_sender = Some((*sender, next_tx));
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            if let Some((sender, _)) = best_sender {
                let queue = grouped.get_mut(&sender).unwrap();
                ordered.push(queue.remove(0));
                if queue.is_empty() {
                    grouped.remove(&sender);
                }
            } else {
                break;
            }
        }
        ordered
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
    fn order_transactions(&self, transactions: Vec<PooledTransaction>) -> Vec<PooledTransaction> {
        let mut grouped: HashMap<ethers::types::Address, Vec<PooledTransaction>> = HashMap::new();
        for tx in transactions {
            grouped.entry(tx.tx.from).or_default().push(tx);
        }
        for queue in grouped.values_mut() {
            queue.sort_by_key(|t| t.tx.nonce);
        }

        let mut ordered = Vec::new();
        while !grouped.is_empty() {
            let mut best_sender = None;
            for (sender, queue) in &grouped {
                if let Some(next_tx) = queue.first() {
                    match best_sender {
                        None => {
                            best_sender = Some((*sender, next_tx));
                        }
                        Some((_, best_tx)) => {
                            let window_next = next_tx.tx.timestamp / self.time_window_ms;
                            let window_best = best_tx.tx.timestamp / self.time_window_ms;
                            
                            let next_is_better = match window_next.cmp(&window_best) {
                                std::cmp::Ordering::Less => true,
                                std::cmp::Ordering::Greater => false,
                                std::cmp::Ordering::Equal => {
                                    let boost_next = next_tx.tx.boost_bid.unwrap_or_default();
                                    let boost_best = best_tx.tx.boost_bid.unwrap_or_default();
                                    match boost_next.cmp(&boost_best) {
                                        std::cmp::Ordering::Greater => true,
                                        std::cmp::Ordering::Less => false,
                                        std::cmp::Ordering::Equal => {
                                            match next_tx.tx.gas_price.cmp(&best_tx.tx.gas_price) {
                                                std::cmp::Ordering::Greater => true,
                                                std::cmp::Ordering::Less => false,
                                                std::cmp::Ordering::Equal => {
                                                    next_tx.tx.timestamp < best_tx.tx.timestamp
                                                }
                                            }
                                        }
                                    }
                                }
                            };
                            if next_is_better {
                                best_sender = Some((*sender, next_tx));
                            }
                        }
                    }
                }
            }
            if let Some((sender, _)) = best_sender {
                let queue = grouped.get_mut(&sender).unwrap();
                ordered.push(queue.remove(0));
                if queue.is_empty() {
                    grouped.remove(&sender);
                }
            } else {
                break;
            }
        }
        ordered
    }
    
    fn name(&self) -> &str {
        "TimeBoost"
    }

    fn config_params(&self) -> serde_json::Value {
        serde_json::json!({
            "time_window_ms": self.time_window_ms
        })
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
    fn order_transactions(&self, mut transactions: Vec<PooledTransaction>) -> Vec<PooledTransaction> {
        // Sort strictly by timestamp (ascending - earliest first)
        // This provides time-based fairness
        transactions.sort_by(|a, b| a.tx.timestamp.cmp(&b.tx.timestamp));
        transactions
    }
    
    fn name(&self) -> &str {
        "FairBFT"
    }
}

/// Blob Packing Policy
///
/// Prefers larger encoded payloads first so that blob-capacity batching can
/// reach a higher fill ratio before timeout. This policy is most useful when
/// transaction sizes vary materially across the mempool.
pub struct BlobPackingPolicy;

impl SchedulingPolicy for BlobPackingPolicy {
    fn order_transactions(&self, transactions: Vec<PooledTransaction>) -> Vec<PooledTransaction> {
        let mut grouped: HashMap<ethers::types::Address, Vec<PooledTransaction>> = HashMap::new();
        for tx in transactions {
            grouped.entry(tx.tx.from).or_default().push(tx);
        }
        for queue in grouped.values_mut() {
            queue.sort_by_key(|t| t.tx.nonce);
        }

        let mut ordered = Vec::new();
        while !grouped.is_empty() {
            let mut best_sender = None;
            for (sender, queue) in &grouped {
                if let Some(next_tx) = queue.first() {
                    match best_sender {
                        None => {
                            best_sender = Some((*sender, next_tx));
                        }
                        Some((_, best_tx)) => {
                            let size_next = next_tx.estimated_encoded_bytes();
                            let size_best = best_tx.estimated_encoded_bytes();
                            
                            let next_is_better = match size_next.cmp(&size_best) {
                                std::cmp::Ordering::Greater => true,
                                std::cmp::Ordering::Less => false,
                                std::cmp::Ordering::Equal => {
                                    match next_tx.tx.gas_price.cmp(&best_tx.tx.gas_price) {
                                        std::cmp::Ordering::Greater => true,
                                        std::cmp::Ordering::Less => false,
                                        std::cmp::Ordering::Equal => {
                                            next_tx.tx.timestamp < best_tx.tx.timestamp
                                        }
                                    }
                                }
                            };
                            if next_is_better {
                                best_sender = Some((*sender, next_tx));
                            }
                        }
                    }
                }
            }
            if let Some((sender, _)) = best_sender {
                let queue = grouped.get_mut(&sender).unwrap();
                ordered.push(queue.remove(0));
                if queue.is_empty() {
                    grouped.remove(&sender);
                }
            } else {
                break;
            }
        }
        ordered
    }

    fn name(&self) -> &str {
        "BlobPacking"
    }

    fn config_params(&self) -> serde_json::Value {
        serde_json::json!({
            "objective": "blob_fill"
        })
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
    /// Blob Packing (size-aware blob fill optimization)
    BlobPacking,
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
/// ```ignore
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
        SchedulingPolicyType::BlobPacking => Box::new(BlobPackingPolicy),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PooledTransaction, UserTransaction};
    use ethers::types::{Address, Signature, U256};

    fn tx_with_size(gas_price: u64, value: &str, timestamp: u64) -> PooledTransaction {
        let tx = UserTransaction {
            from: Address::random(),
            to: Address::random(),
            value: U256::from_dec_str(value).unwrap(),
            nonce: 1,
            gas_limit: 21_000,
            gas_price: U256::from(gas_price),
            signature: Signature { r: U256::zero(), s: U256::zero(), v: 27 },
            timestamp,
            boost_bid: None,
        };
        PooledTransaction {
            tx,
            arrived_at: timestamp,
            pool_entry_at: timestamp + 1,
            validation_latency_ms: 1,
        }
    }

    #[test]
    fn blob_packing_orders_larger_payloads_first() {
        let policy = BlobPackingPolicy;
        let small = tx_with_size(2, "1", 1);
        let large = tx_with_size(1, "1000000000000000000000000000000", 2);

        let ordered = policy.order_transactions(vec![small.clone(), large.clone()]);

        assert_eq!(ordered.len(), 2);
        assert_eq!(ordered[0].tx.gas_price, large.tx.gas_price);
        assert_eq!(ordered[1].tx.gas_price, small.tx.gas_price);
    }

    #[test]
    fn fee_priority_preserves_single_sender_nonce_order() {
        let policy = FeePriorityPolicy;
        let sender = Address::random();
        let mut tx_low = tx_with_size(1, "1", 1);
        tx_low.tx.from = sender;
        tx_low.tx.nonce = 1;
        
        let mut tx_high = tx_with_size(10, "1", 2);
        tx_high.tx.from = sender;
        tx_high.tx.nonce = 2;

        // The naive sorting would put tx_high first.
        // The nonce-aware sorting must put tx_low first.
        let ordered = policy.order_transactions(vec![tx_high.clone(), tx_low.clone()]);

        assert_eq!(ordered.len(), 2);
        assert_eq!(ordered[0].tx.nonce, 1);
        assert_eq!(ordered[1].tx.nonce, 2);
    }

    #[test]
    fn blob_packing_preserves_single_sender_nonce_order() {
        let policy = BlobPackingPolicy;
        let sender = Address::random();
        let mut tx_small = tx_with_size(2, "1", 1); // small value
        tx_small.tx.from = sender;
        tx_small.tx.nonce = 1;
        
        let mut tx_large = tx_with_size(1, "1000000000000000000000000000000", 2); // large value
        tx_large.tx.from = sender;
        tx_large.tx.nonce = 2;

        // The naive sorting would put tx_large first because of size.
        // The nonce-aware sorting must put tx_small first.
        let ordered = policy.order_transactions(vec![tx_large.clone(), tx_small.clone()]);

        assert_eq!(ordered.len(), 2);
        assert_eq!(ordered[0].tx.nonce, 1);
        assert_eq!(ordered[1].tx.nonce, 2);
    }
}
