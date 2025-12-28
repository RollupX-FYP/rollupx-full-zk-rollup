use crate::ForcedTransaction;
use std::collections::VecDeque;
use tokio::sync::RwLock;

pub struct ForcedQueue {
    transactions: RwLock<VecDeque<ForcedTransaction>>,
}

impl ForcedQueue {
    pub fn new() -> Self {
        Self {
            transactions: RwLock::new(VecDeque::new()),
        }
    }
    
    pub async fn add(&self, tx: ForcedTransaction) {
        let mut txs = self.transactions.write().await;
        txs.push_back(tx);
    }
    
    pub async fn get_all(&self) -> Vec<ForcedTransaction> {
        let mut txs = self.transactions.write().await;
        txs.drain(..).collect()
    }
}