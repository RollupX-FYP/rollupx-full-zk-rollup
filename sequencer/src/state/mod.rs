//! State Management Module
//! 
//! This module provides in-memory caching of account state for fast transaction validation.
//! The state cache stores account balances and nonces.

mod cache;
pub use cache::StateCache;