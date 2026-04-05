//! State Cache Module
//!
//! This module provides an in-memory cache for account state (balances and nonces).
//! The cache is used for fast transaction validation without querying a database.
//! It supports concurrent access through `RwLock` for thread safety.
//!
//! # Pessimistic Balance Tracking
//! When a transaction is validated and accepted into the pool, the cache
//! immediately deducts the transaction cost (value + gas) from the sender's
//! balance. This prevents double-spend attacks where a user submits multiple
//! transactions that individually pass balance checks but collectively exceed
//! their balance.

use crate::AccountState;
use ethers::types::{Address, U256};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// In-memory state cache for account data
///
/// Stores account state (balance and nonce) in memory for fast access.
/// Uses `Arc<RwLock<..>>` for thread-safe concurrent access:
/// - Multiple readers can access simultaneously (balance/nonce lookups)
/// - Writes are exclusive (nonce increments, balance deductions)
///
/// # Cloning
/// This struct is cheaply cloneable because it uses `Arc` internally.
/// All clones share the same underlying data.
///
/// # Consistency Model
/// The cache provides eventual consistency with the executor's state.
/// State updates from the executor are applied via `update()`.
/// Between executor syncs, the cache uses pessimistic tracking
/// (nonce increments + balance deductions) to prevent invalid transactions.
#[derive(Clone)]
pub struct StateCache {
    /// Map from address to account state, protected by a read-write lock
    accounts: Arc<RwLock<HashMap<Address, AccountState>>>,
}

impl StateCache {
    /// Creates a new empty state cache
    ///
    /// # Returns
    /// A new `StateCache` instance with no accounts
    pub fn new() -> Self {
        Self {
            accounts: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get the balance of an account
    ///
    /// # Arguments
    /// * `address` - The account address to query
    ///
    /// # Returns
    /// * `Some(balance)` if the account exists in the cache
    /// * `None` if the account is not in the cache
    pub async fn get_balance(&self, address: &Address) -> Option<U256> {
        // Acquire read lock (allows concurrent reads)
        let accounts = self.accounts.read().await;
        accounts.get(address).map(|acc| acc.balance)
    }

    /// Get the nonce of an account
    ///
    /// # Arguments
    /// * `address` - The account address to query
    ///
    /// # Returns
    /// * `Some(nonce)` if the account exists in the cache
    /// * `None` if the account is not in the cache
    pub async fn get_nonce(&self, address: &Address) -> Option<u64> {
        // Acquire read lock (allows concurrent reads)
        let accounts = self.accounts.read().await;
        accounts.get(address).map(|acc| acc.nonce)
    }

    /// Get account state or initialize with defaults if not found
    ///
    /// This is the primary method used during transaction validation.
    /// If the account doesn't exist, it returns default values (zero balance, zero nonce)
    /// WITHOUT adding it to the cache. This allows validation to proceed for new accounts.
    ///
    /// # Arguments
    /// * `address` - The account address to query
    ///
    /// # Returns
    /// Account state (either from cache or default values)
    pub async fn get_or_init_account(&self, address: &Address) -> AccountState {
        // First try to read from cache
        let accounts = self.accounts.read().await;
        if let Some(account) = accounts.get(address) {
            // Account exists — return a clone
            account.clone()
        } else {
            // Account doesn't exist — release read lock and return defaults
            drop(accounts); // Explicitly drop to release lock early
            AccountState {
                address: *address,
                balance: U256::zero(), // New accounts start with zero balance
                nonce: 0,              // New accounts start with nonce 0
            }
        }
    }

    /// Increment nonce for an account
    ///
    /// Called after a transaction is validated and accepted into the pool.
    /// This prevents the next transaction from the same account from having
    /// a nonce conflict.
    ///
    /// # Arguments
    /// * `address` - The account address to update
    ///
    /// # Behavior
    /// - If account exists: increments its nonce by 1
    /// - If account doesn't exist: creates it with nonce 1 and zero balance
    pub async fn increment_nonce(&self, address: &Address) {
        // Acquire write lock (exclusive access)
        let mut accounts = self.accounts.write().await;
        if let Some(account) = accounts.get_mut(address) {
            // Account exists — increment nonce
            account.nonce += 1;
        } else {
            // Account doesn't exist — initialize with nonce 1
            // This handles the case where the first transaction from an account is processed
            accounts.insert(*address, AccountState {
                address: *address,
                balance: U256::zero(),
                nonce: 1, // First transaction processed, so nonce becomes 1
            });
        }
    }

    /// Deduct balance from an account (pessimistic tracking)
    ///
    /// Called after a transaction passes validation and is accepted into the pool.
    /// This immediately reduces the cached balance by the total transaction cost
    /// (value + gas), preventing double-spend attacks from concurrent submissions.
    ///
    /// # Arguments
    /// * `address` - The account address to deduct from
    /// * `amount` - The total amount to deduct (value + gas_price × gas_limit)
    ///
    /// # Behavior
    /// - If account exists: subtracts `amount` from its balance (saturating at zero)
    /// - If account doesn't exist: no-op (account with zero balance can't send)
    ///
    /// # Why Saturating Subtraction?
    /// We use `saturating_sub` to avoid underflow panics. In practice, the
    /// balance check during validation should prevent this from ever reaching
    /// zero, but saturating subtraction is a safety net.
    pub async fn deduct_balance(&self, address: &Address, amount: U256) {
        let mut accounts = self.accounts.write().await;
        if let Some(account) = accounts.get_mut(address) {
            // Saturating subtraction prevents underflow
            account.balance = account.balance.saturating_sub(amount);
        }
        // If account doesn't exist in cache, no balance to deduct.
        // This is safe because the validation step already verified balance.
    }

    /// Update or insert account state
    ///
    /// Completely replaces the account state in the cache.
    /// Used when syncing state from the executor after batch execution.
    ///
    /// # Arguments
    /// * `state` - The new account state to store
    pub async fn update(&self, state: AccountState) {
        // Acquire write lock (exclusive access)
        let mut accounts = self.accounts.write().await;
        accounts.insert(state.address, state);
    }

    /// Get the number of accounts tracked in the cache
    ///
    /// Useful for diagnostics and monitoring the cache size.
    ///
    /// # Returns
    /// The number of unique accounts in the cache
    pub async fn account_count(&self) -> usize {
        let accounts = self.accounts.read().await;
        accounts.len()
    }
}