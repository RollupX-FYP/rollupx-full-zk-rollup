//! Transaction Scheduling Module
//!
//! This module implements scheduling policies that determine transaction ordering
//! using the Strategy design pattern:
//! - FCFS (First-Come-First-Served): Transactions ordered by arrival time
//! - FeePriority: Transactions ordered by gas price (highest first)
//! - TimeBoost: Time-windowed ordering with premium bids for faster confirmation
//! - FairBFT: Timestamp-based fair ordering (Byzantine Fault Tolerant)
//! - BlobPacking: Size-aware ordering for DA blob utilization
//! - BlobPackingBestFit: Nonce-safe best-fit blob packing with an age guard
//!
//! Forced transactions from L1 always have priority regardless of policy.

mod policies;
mod scheduler;

#[cfg(test)]
mod tests;

pub use policies::{
    BlobPackingBestFitPolicy, BlobPackingPolicy, FairBftPolicy, FcfsPolicy, FeePriorityPolicy,
    SchedulingPolicy, SchedulingPolicyType, TimeBoostPolicy, create_policy,
};
pub use scheduler::Scheduler;
