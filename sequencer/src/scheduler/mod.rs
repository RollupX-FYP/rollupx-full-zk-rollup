//! Transaction Scheduling Module
//! 
//! This module implements scheduling policies that determine transaction ordering
//! using the Strategy design pattern:
//! - FCFS (First-Come-First-Served): Transactions ordered by arrival time
//! - FeePriority: Transactions ordered by gas price (highest first)
//! - TimeBoost: Time-windowed ordering with premium bids for faster confirmation
//! - FairBFT: Timestamp-based fair ordering (Byzantine Fault Tolerant)
//! 
//! Forced transactions from L1 always have priority regardless of policy.

mod scheduler;
mod policies;

#[cfg(test)]
mod tests;

pub use scheduler::Scheduler;
pub use policies::{
    SchedulingPolicy,
    SchedulingPolicyType,
    FcfsPolicy,
    FeePriorityPolicy,
    TimeBoostPolicy,
    FairBftPolicy,
    create_policy,
};