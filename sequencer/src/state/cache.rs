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
    
    pub async fn update(&self, state: AccountState) {
        let mut accounts = self.accounts.write().await;
        accounts.insert(state.address, state);
    }
}