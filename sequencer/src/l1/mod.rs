//! Layer 1 Integration Module
//! 
//! This module handles integration with the Ethereum L1 blockchain:
//! - Monitors the bridge contract for forced transaction events
//! - Detects deposits and forced exits from L1
//! - Ensures censorship resistance

mod listener;
pub use listener::L1Listener;