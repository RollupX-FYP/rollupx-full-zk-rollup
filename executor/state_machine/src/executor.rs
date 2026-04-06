use std::path::Path;
use std::rc::Rc;
use zksync_multivm::interface::{
    L1BatchEnv, SystemEnv,
};
use zksync_vm_interface::storage::ReadStorage;
use zksync_types::Transaction;

use crate::types::{BatchInput, BatchOutput};
use crate::tree::TreeProcessor;
use crate::StateMachine;

pub struct BatchProcessor<S: ReadStorage> {
    state_machine: StateMachine<S>,
    tree: TreeProcessor,
}

impl<S: ReadStorage> BatchProcessor<S> {
    pub fn new(
        storage: S,
        l1_batch_env: L1BatchEnv,
        system_env: SystemEnv,
        tree_path: &Path,
    ) -> anyhow::Result<Self> {
        let state_machine = StateMachine::new(storage, l1_batch_env, system_env);
        let tree = TreeProcessor::new(tree_path)?;
        Ok(Self { state_machine, tree })
    }

    pub fn process_batch(mut self, input: BatchInput) -> anyhow::Result<BatchOutput> {
        let batch_number = input.l1_batch_env.number;

        for tx in input.transactions {
            self.state_machine.execute_transaction(tx)?;
        }

        let finished_batch = self.state_machine.seal_batch();
        
        let storage_logs: Vec<_> = finished_batch.final_execution_state.deduplicated_storage_logs
            .iter()
            .cloned()
            .collect();
        
        let pubdata = finished_batch.pubdata_input.clone().unwrap_or_default();

        let tree_metadata = self.tree.process_batch(&storage_logs)?;
        let root_hash = tree_metadata.root_hash;
        let witness = tree_metadata.witness.clone();

        Ok(BatchOutput {
            batch_number,
            root_hash,
            witness,
            pubdata,
            finished_batch,
            tree_metadata,
        })
    }

    pub fn tree(&mut self) -> &mut TreeProcessor {
        &mut self.tree
    }

    pub fn state_machine_mut(&mut self) -> &mut StateMachine<S> {
        &mut self.state_machine
    }
}
