//! Configuration Module
//! 
//! This module defines all configuration structures for the sequencer.
//! Configuration is loaded from TOML files and parsed using serde.

use serde::Deserialize;
use std::fs;

/// Main configuration structure
/// 
/// Contains all configuration sections for the sequencer.
/// Loaded from a TOML file (e.g., config/default.toml).
/// 
/// # Example TOML
/// ```toml
/// [batch]
/// max_batch_size = 100
/// timeout_interval_ms = 5000
/// min_batch_size = 10
/// 
/// [scheduling]
/// policy_type = "FCFS"
/// 
/// [api]
/// host = "127.0.0.1"
/// port = 8545
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub batch: BatchConfig,
    pub scheduling: SchedulingConfig,
    pub api: ApiConfig,
    pub l1: L1Config,
    pub database: DatabaseConfig,
}

/// Batch creation configuration
/// 
/// Controls when and how batches are created.
/// 
/// # Fields
/// - `max_batch_size`: Maximum number of transactions per batch
/// - `timeout_interval_ms`: How long to wait before sealing a partial batch (in milliseconds)
/// - `min_batch_size`: Minimum transactions before considering a timeout seal
#[derive(Debug, Clone, Deserialize)]
pub struct BatchConfig {
    pub max_batch_size: usize,
    pub timeout_interval_ms: u64,
    pub min_batch_size: usize,
}

/// Transaction scheduling configuration
/// 
/// Determines which scheduling policy to use when creating batches.
/// 
/// # Supported Policies
/// - `"FCFS"`: First-Come-First-Served (transactions ordered by arrival time)
/// - `"FeePriority"`: Fee-based priority (highest gas price first)
#[derive(Debug, Clone, Deserialize)]
pub struct SchedulingConfig {
    /// Policy type: "FCFS" or "FeePriority"
    pub policy_type: String,
}

/// API server configuration
/// 
/// Controls the JSON-RPC API endpoint settings.
/// 
/// # Fields
/// - `host`: IP address to bind to (e.g., "127.0.0.1" or "0.0.0.0")
/// - `port`: TCP port to listen on (e.g., 8545)
#[derive(Debug, Clone, Deserialize)]
pub struct ApiConfig {
    pub host: String,
    pub port: u16,
}

/// Layer 1 connection configuration
/// 
/// Settings for monitoring the L1 blockchain for forced transactions.
/// 
/// # Fields
/// - `rpc_url`: Ethereum L1 RPC endpoint (e.g., "https://eth-mainnet.g.alchemy.com/v2/...")
/// - `bridge_address`: Address of the L1 bridge contract to monitor
/// - `start_block`: L1 block number to start monitoring from
#[derive(Debug, Clone, Deserialize)]
pub struct L1Config {
    pub rpc_url: String,
    pub bridge_address: String,
    pub start_block: u64,
}

/// Database configuration
/// 
/// Settings for the batch metadata registry database.
/// 
/// # Fields
/// - `url`: Database connection URL (e.g., "sqlite://registry.db")
#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
}

impl Config {
    /// Load configuration from a TOML file
    /// 
    /// # Arguments
    /// * `path` - Path to the TOML configuration file
    /// 
    /// # Returns
    /// * `Ok(Config)` if the file was successfully loaded and parsed
    /// * `Err` if the file couldn't be read or the TOML is invalid
    /// 
    /// # Example
    /// ```no_run
    /// let config = Config::load("config/default.toml")?;
    /// ```
    pub fn load(path: &str) -> anyhow::Result<Self> {
        // Read the file contents as a string
        let content = fs::read_to_string(path)?;
        
        // Parse the TOML into our Config structure
        let config: Config = toml::from_str(&content)?;
        
        Ok(config)
    }
}