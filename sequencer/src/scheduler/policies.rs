//! Scheduling Policies Module
//!
//! This module implements the Strategy design pattern for transaction scheduling.
//! Each policy determines how normal transactions are ordered within a batch.
//!
//! # Available Policies
//!
//! ## 1. FCFS (First-Come-First-Served)
//! - Orders transactions by sequencer-observed arrival time
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
//! - Orders strictly by sequencer-observed arrival time (earliest first)
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
/// Maintains the pool submission order. No reordering is performed.
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
    fn order_transactions(
        &self,
        mut transactions: Vec<PooledTransaction>,
    ) -> Vec<PooledTransaction> {
        // Sort by gas_price in descending order (highest fee first)
        transactions.sort_by(|a, b| b.tx.gas_price.cmp(&a.tx.gas_price));
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
    fn order_transactions(
        &self,
        mut transactions: Vec<PooledTransaction>,
    ) -> Vec<PooledTransaction> {
        // Group transactions by sequencer-observed arrival time window.
        // The user-supplied transaction timestamp is signed data and may be
        // second-granularity or client-controlled, so it is not suitable for
        // local scheduling fairness.
        let window_ms = self.time_window_ms.max(1);

        // Sort by multiple criteria:
        // 1. Time window (ascending - earlier windows first)
        // 2. Within same window: boost_bid (descending)
        // 3. Within same boost_bid: gas_price (descending)
        // 4. Maintain stable sort for FCFS tie-breaking

        transactions.sort_by(|a, b| {
            let window_a = a.arrived_at / window_ms;
            let window_b = b.arrived_at / window_ms;

            // First, compare by time window
            match window_a.cmp(&window_b) {
                std::cmp::Ordering::Equal => {
                    // Same window: compare by boost_bid
                    let boost_a = a.tx.boost_bid.unwrap_or_default();
                    let boost_b = b.tx.boost_bid.unwrap_or_default();

                    match boost_b.cmp(&boost_a) {
                        // Descending (b vs a)
                        std::cmp::Ordering::Equal => {
                            // Same boost: compare by gas_price
                            b.tx.gas_price.cmp(&a.tx.gas_price) // Descending
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

    fn config_params(&self) -> serde_json::Value {
        serde_json::json!({
            "time_window_ms": self.time_window_ms
        })
    }
}

/// Fair BFT Ordering Policy
///
/// Orders transactions strictly by sequencer-observed arrival time to provide
/// local time-based fairness.
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
/// Orders by local sequencer arrival time (single-node, no consensus).
pub struct FairBftPolicy;

impl SchedulingPolicy for FairBftPolicy {
    fn order_transactions(
        &self,
        mut transactions: Vec<PooledTransaction>,
    ) -> Vec<PooledTransaction> {
        transactions.sort_by(|a, b| match a.arrived_at.cmp(&b.arrived_at) {
            std::cmp::Ordering::Equal => a.pool_entry_at.cmp(&b.pool_entry_at),
            other => other,
        });
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
    fn order_transactions(
        &self,
        mut transactions: Vec<PooledTransaction>,
    ) -> Vec<PooledTransaction> {
        transactions.sort_by(|a, b| {
            let size_a = a.estimated_encoded_bytes();
            let size_b = b.estimated_encoded_bytes();
            match size_b.cmp(&size_a) {
                std::cmp::Ordering::Equal => match b.tx.gas_price.cmp(&a.tx.gas_price) {
                    std::cmp::Ordering::Equal => a.arrived_at.cmp(&b.arrived_at),
                    other => other,
                },
                other => other,
            }
        });
        transactions
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

/// Blob Packing Best-Fit Policy
///
/// The orchestrator uses a nonce-safe best-fit selector for this policy. The
/// trait implementation remains useful for tests and non-orchestrator callers.
pub struct BlobPackingBestFitPolicy;

impl SchedulingPolicy for BlobPackingBestFitPolicy {
    fn order_transactions(
        &self,
        mut transactions: Vec<PooledTransaction>,
    ) -> Vec<PooledTransaction> {
        transactions.sort_by(|a, b| match a.arrived_at.cmp(&b.arrived_at) {
            std::cmp::Ordering::Equal => b.tx.gas_price.cmp(&a.tx.gas_price),
            other => other,
        });
        transactions
    }

    fn name(&self) -> &str {
        "BlobPackingBestFit"
    }

    fn config_params(&self) -> serde_json::Value {
        serde_json::json!({
            "objective": "blob_best_fit_with_age_guard",
            "age_guard_ms": 15000
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
        time_window_ms: u64,
    },
    /// Fair BFT Ordering (timestamp-based)
    FairBft,
    /// Blob Packing (size-aware blob fill optimization)
    BlobPacking,
    /// Blob Packing Best-Fit with age guard
    BlobPackingBestFit,
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
        SchedulingPolicyType::BlobPackingBestFit => Box::new(BlobPackingBestFitPolicy),
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
            signature: Signature {
                r: U256::zero(),
                s: U256::zero(),
                v: 27,
            },
            timestamp,
            boost_bid: None,
            calldata: None,
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
}
