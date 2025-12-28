use crate::UserTransaction;
use std::collections::VecDeque;
use tokio::sync::RwLock;

pub struct TransactionPool {
    transactions: RwLock<VecDeque<UserTransaction>>,
}

impl TransactionPool {
    pub fn new() -> Self {
        Self {
            transactions: RwLock::new(VecDeque::new()),
        }
    }
    
    pub async fn add(&self, tx: UserTransaction) {
        let mut txs = self.transactions.write().await;
        txs.push_back(tx);
    }
    
    pub async fn get_pending(&self, max: usize) -> Vec<UserTransaction> {
        let mut txs = self.transactions.write().await;
        let len = txs.len();
        txs.drain(..max.min(len)).collect()
    }
}