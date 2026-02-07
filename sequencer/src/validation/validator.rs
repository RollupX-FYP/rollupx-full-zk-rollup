//! Transaction Validator Module
//! 
//! This module is responsible for validating user transactions before they
//! are accepted into the transaction pool. It performs three main checks:
//! 1. Signature verification - ensures the transaction is signed by the claimed sender
//! 2. Nonce validation - ensures transactions are processed in order
//! 3. Balance verification - ensures the sender has sufficient funds

use crate::{UserTransaction, ValidationError, state::StateCache};
use anyhow::Result;
use ethers::types::U256;
use tracing::{debug, warn};

/// The transaction validator
/// 
/// Validates transactions against the current state before they enter the pool.
/// Uses the state cache to check account nonces and balances.
pub struct Validator {
    state_cache: StateCache,
}

impl Validator {
    /// Creates a new validator with access to the state cache
    /// 
    /// # Arguments
    /// * `state_cache` - The state cache for looking up account data
    pub fn new(state_cache: StateCache) -> Self {
        Self { state_cache }
    }
    
    /// Validate a user transaction
    /// 
    /// Performs a comprehensive validation of the transaction by checking:
    /// 1. Signature validity - is this transaction signed by the claimed sender?
    /// 2. Nonce correctness - is this the next expected transaction from this account?
    /// 3. Sufficient balance - does the account have enough funds for value + gas?
    /// 
    /// # Arguments
    /// * `tx` - The transaction to validate
    /// 
    /// # Returns
    /// * `Ok(())` if the transaction passes all validation checks
    /// * `Err(ValidationError)` if any validation check fails
    pub async fn validate(&self, tx: &UserTransaction) -> Result<(), ValidationError> {
        debug!("Validating transaction from {:?}", tx.from);
        
        // Step 1: Verify the cryptographic signature
        // This ensures the transaction was actually signed by the private key
        // corresponding to the 'from' address
        self.verify_signature(tx)?;
        
        // Step 2: Check the nonce (transaction sequence number)
        // This ensures transactions are processed in order and prevents replay attacks
        self.check_nonce(tx).await?;
        
        // Step 3: Check the account balance
        // This ensures the sender has enough funds to cover both the transfer value
        // and the gas costs
        self.check_balance(tx).await?;
        
        debug!("Transaction validation successful");
        Ok(())
    }
    
    /// Verify the transaction signature
    /// 
    /// Uses ECDSA signature recovery to verify that the transaction was signed
    /// by the private key corresponding to the 'from' address.
    /// 
    /// # Process
    /// 1. Compute the hash of the transaction
    /// 2. Recover the public key/address from the signature
    /// 3. Compare the recovered address with the 'from' field
    /// 
    /// # Returns
    /// * `Ok(())` if the signature is valid
    /// * `Err(ValidationError::InvalidSignature)` if signature recovery fails or doesn't match
    fn verify_signature(&self, tx: &UserTransaction) -> Result<(), ValidationError> {
        // Hash the transaction data
        let tx_hash = tx.hash();
        
        // Recover the signer's address from the signature
        // This uses ECDSA recovery which is a standard cryptographic operation
        let recovered_address = tx.signature.recover(tx_hash)
            .map_err(|_| ValidationError::InvalidSignature)?;
        
        // Verify that the recovered address matches the claimed sender
        // If they don't match, the signature is invalid (potential forgery)
        if recovered_address != tx.from {
            warn!("Signature verification failed: signer mismatch");
            return Err(ValidationError::InvalidSignature);
        }
        
        Ok(())
    }
    
    /// Check if the transaction nonce is valid
    /// 
    /// The nonce is a sequence number that ensures transactions from an account
    /// are processed in order. Each transaction must have a nonce equal to the
    /// current account nonce.
    /// 
    /// # Why nonces are important
    /// - Prevents replay attacks (reusing the same transaction)
    /// - Ensures deterministic transaction ordering
    /// - Prevents race conditions when submitting multiple transactions
    /// 
    /// # Returns
    /// * `Ok(())` if the nonce matches the expected value
    /// * `Err(ValidationError::InvalidNonce)` if the nonce is incorrect
    async fn check_nonce(&self, tx: &UserTransaction) -> Result<(), ValidationError> {
        // Get the current account state from the cache
        let account = self.state_cache.get_or_init_account(&tx.from).await;
        let expected_nonce = account.nonce;
        
        // Nonce must be exactly equal to the current account nonce
        // This enforces sequential processing: nonce 0, then 1, then 2, etc.
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
    /// 
    /// Ensures the sender has enough funds to cover both:
    /// 1. The transfer value (amount being sent)
    /// 2. The gas costs (fees paid to execute the transaction)
    /// 
    /// # Gas Cost Calculation
    /// For simplicity, this implementation assumes a fixed gas limit of 21,000
    /// (the standard cost for a basic ETH transfer). In production, this would
    /// be computed based on the actual transaction complexity.
    /// 
    /// # Returns
    /// * `Ok(())` if the account has sufficient balance
    /// * `Err(ValidationError::InsufficientBalance)` if funds are insufficient
    async fn check_balance(&self, tx: &UserTransaction) -> Result<(), ValidationError> {
        // Fetch the current account state
        let account = self.state_cache.get_or_init_account(&tx.from).await;
        
        // Calculate gas cost: gas_price * gas_limit
        // In production, gas_limit would be estimated based on transaction complexity
        let gas_limit = U256::from(21000); // Standard gas for basic transfer
        let gas_cost = tx.gas_price * gas_limit;
        
        // Calculate total funds required: transfer value + gas fees
        let required = tx.value + gas_cost;
        
        // Check if the account has sufficient balance
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