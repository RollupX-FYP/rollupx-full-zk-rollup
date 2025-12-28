use crate::{UserTransaction, state::StateCache};
use anyhow::Result;

pub struct Validator {
    state_cache: StateCache,
}

impl Validator {
    pub fn new(state_cache: StateCache) -> Self {
        Self { state_cache }
    }
    
    pub async fn validate(&self, tx: &UserTransaction) -> Result<bool> {
        // TODO: Implement validation logic
        // 1. Verify signature
        // 2. Check nonce
        // 3. Check balance
        Ok(true)
    }
}