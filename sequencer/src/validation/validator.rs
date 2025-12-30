use crate::{UserTransaction, ValidationError, state::StateCache};
use anyhow::Result;
use ethers::types::U256;
use tracing::{debug, warn};

pub struct Validator {
    state_cache: StateCache,
}

impl Validator {
    pub fn new(state_cache: StateCache) -> Self {
        Self { state_cache }
    }
    
    /// Validate a user transaction
    /// Returns Ok(()) if valid, Err(ValidationError) if invalid
    pub async fn validate(&self, tx: &UserTransaction) -> Result<(), ValidationError> {
        debug!("Validating transaction from {:?}", tx.from);
        
        // 1. Verify signature
        self.verify_signature(tx)?;
        
        // 2. Check nonce
        self.check_nonce(tx).await?;
        
        // 3. Check balance
        self.check_balance(tx).await?;
        
        debug!("Transaction validation successful");
        Ok(())
    }
    
    /// Verify the transaction signature
    fn verify_signature(&self, tx: &UserTransaction) -> Result<(), ValidationError> {
        let tx_hash = tx.hash();
        
        // Recover the signer from the signature
        let recovered_address = tx.signature.recover(tx_hash)
            .map_err(|_| ValidationError::InvalidSignature)?;
        
        // Verify the recovered address matches the from field
        if recovered_address != tx.from {
            warn!("Signature verification failed: signer mismatch");
            return Err(ValidationError::InvalidSignature);
        }
        
        Ok(())
    }
    
    /// Check if the transaction nonce is valid
    async fn check_nonce(&self, tx: &UserTransaction) -> Result<(), ValidationError> {
        let account = self.state_cache.get_or_init_account(&tx.from).await;
        let expected_nonce = account.nonce;
        
        // Nonce should be exactly the current nonce (sequential)
        if tx.nonce != expected_nonce {
            warn!(
                "Nonce check failed for {:?}: expected {}, got {}",
                tx.from, expected_nonce, tx.nonce
            );
            return Err(ValidationError::InvalidNonce {
                expected: expected_nonce,
                got: tx.nonce,
            });
        }
        
        Ok(())
    }
    
    /// Check if the account has sufficient balance for the transaction
    async fn check_balance(&self, tx: &UserTransaction) -> Result<(), ValidationError> {
        let account = self.state_cache.get_or_init_account(&tx.from).await;
        
        // Estimate gas cost (simplified: assume 21000 gas for basic transfer)
        // In production, this would be more sophisticated
        let gas_limit = U256::from(21000);
        let gas_cost = tx.gas_price * gas_limit;
        
        // Total required = value + gas cost
        let required = tx.value + gas_cost;
        
        if account.balance < required {
            warn!(
                "Insufficient balance for {:?}: required {}, available {}",
                tx.from, required, account.balance
            );
            return Err(ValidationError::InsufficientBalance {
                required,
                available: account.balance,
            });
        }
        
        Ok(())
    }
}