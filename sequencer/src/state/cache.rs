use crate::AccountState;
use ethers::types::{Address, U256};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct StateCache {
    accounts: Arc<RwLock<HashMap<Address, AccountState>>>,
}

impl StateCache {
    pub fn new() -> Self {
        Self {
            accounts: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    pub async fn get_balance(&self, address: &Address) -> Option<U256> {
        let accounts = self.accounts.read().await;
        accounts.get(address).map(|acc| acc.balance)
    }
    
    pub async fn get_nonce(&self, address: &Address) -> Option<u64> {
        let accounts = self.accounts.read().await;
        accounts.get(address).map(|acc| acc.nonce)
    }
    
    /// Get account state or initialize with defaults if not found
    pub async fn get_or_init_account(&self, address: &Address) -> AccountState {
        let accounts = self.accounts.read().await;
        if let Some(account) = accounts.get(address) {
            account.clone()
        } else {
            drop(accounts); // Release read lock before acquiring write lock
            AccountState {
                address: *address,
                balance: U256::zero(),
                nonce: 0,
            }
        }
    }
    
    /// Increment nonce for an account
    pub async fn increment_nonce(&self, address: &Address) {
        let mut accounts = self.accounts.write().await;
        if let Some(account) = accounts.get_mut(address) {
            account.nonce += 1;
        } else {
            // Initialize account if not exists
            accounts.insert(*address, AccountState {
                address: *address,
                balance: U256::zero(),
                nonce: 1,
            });
        }
    }
    
    pub async fn update(&self, state: AccountState) {
        let mut accounts = self.accounts.write().await;
        accounts.insert(state.address, state);
    }
}