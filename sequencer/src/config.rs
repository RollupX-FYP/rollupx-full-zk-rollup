use serde::Deserialize;
use std::fs;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub batch: BatchConfig,
    pub scheduling: SchedulingConfig,
    pub api: ApiConfig,
    pub l1: L1Config,
    pub database: DatabaseConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BatchConfig {
    pub max_batch_size: usize,
    pub timeout_interval_ms: u64,
    pub min_batch_size: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SchedulingConfig {
    pub policy_type: String, // "FCFS" or "FeePriority"
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct L1Config {
    pub rpc_url: String,
    pub bridge_address: String,
    pub start_block: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
}

impl Config {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
}