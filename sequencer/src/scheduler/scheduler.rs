use crate::{UserTransaction, ForcedTransaction, Transaction};

pub struct Scheduler {
    policy: String,
}

impl Scheduler {
    pub fn new(policy: String) -> Self {
        Self { policy }
    }
    
    pub fn schedule(
        &self,
        forced: Vec<ForcedTransaction>,
        normal: Vec<UserTransaction>,
    ) -> Vec<Transaction> {
        let mut result = Vec::new();
        
        // ALWAYS add forced transactions first
        for tx in forced {
            result.push(Transaction::Forced(tx));
        }
        
        // Then add normal transactions (apply policy here)
        let mut sorted = normal;
        if self.policy == "FeePriority" {
            sorted.sort_by(|a, b| b.gas_price.cmp(&a.gas_price));
        }
        
        for tx in sorted {
            result.push(Transaction::Normal(tx));
        }
        
        result
    }
}