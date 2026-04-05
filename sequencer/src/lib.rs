//! This crate implements the sequencer component of a zk-rollup system.
//!
//! The sequencer is responsible for:
//! - Receiving user transactions via a JSON-RPC API
//! - Validating transactions (signature, nonce, balance)
//! - Ordering transactions using configurable scheduling policies
//! - Batching transactions for execution and L1 submission
//! - Monitoring L1 for forced transactions (deposits, forced exits)
//! - Persisting batch metadata for auditing and monitoring
//!
//! # Architecture Overview
//! ```text
//! User → API Server → Validator → Transaction Pool →
//!   Scheduler (Policy Engine) → Batch Engine → Batch Registry
//!                                                    ↓
//!                                            [Executor Component]
//! ```

pub mod types;       // Defines common data structures and types used throughout the system.
pub mod api;         // Handles external API definitions and interfaces.
pub mod validation;  // Contains logic for validating transactions.
pub mod state;       // Manages the in-memory state cache for fast validation.
pub mod pool;        // Implements transaction pools for pending normal and forced transactions.
pub mod l1;          // Provides utilities for monitoring L1 blockchain events.
pub mod scheduler;   // Manages configurable transaction ordering policies.
pub mod batch;       // Handles batch creation, triggering, and orchestration.
pub mod registry;    // Manages persistent batch metadata storage.
pub mod config;      // Defines and loads system configuration.

// Re-export commonly used types and configurations for easier access.
pub use types::*;
pub use config::Config;
pub use batch::BatchOrchestrator;