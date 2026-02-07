//! This crate defines the core components and logic for a blockchain-like system or a distributed application.
//! It includes modules for data types, API definitions, state management, transaction pooling,
//! L1 interaction, scheduling, batch processing, and configuration.

pub mod types; // Defines common data structures and types used throughout the system.
pub mod api; // Handles external API definitions and interfaces.
pub mod validation; // Contains logic for validating transactions, blocks, or state transitions.
pub mod state; // Manages the current state of the system.
pub mod pool; // Implements a mempool or transaction pool for pending items.
pub mod l1; // Provides utilities for interacting with a Layer 1 blockchain or base layer.
pub mod scheduler; // Manages task scheduling and execution.
pub mod batch; // Handles batch processing of transactions or operations.
pub mod registry; // Manages registration and lookup of components or entities.
pub mod config; // Defines and loads system configuration.

// Re-export commonly used types and configurations for easier access.
pub use types::*;
pub use config::Config;
pub use batch::BatchOrchestrator;