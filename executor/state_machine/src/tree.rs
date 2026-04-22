use std::path::Path;
use zksync_merkle_tree::{
    domain::{TreeMetadata, ZkSyncTree},
    RocksDBWrapper, TreeInstruction, TreeEntry,
};
use zksync_storage::{RocksDB, RocksDBOptions};
use zksync_types::{
    L1BatchNumber, H256, StorageLog,
};

pub struct TreeProcessor {
    tree: ZkSyncTree,
}

impl TreeProcessor {
    pub fn new(db_path: &Path) -> anyhow::Result<Self> {
        let db = RocksDB::with_options(
            db_path,
            RocksDBOptions::default(),
        )?;
        let tree = ZkSyncTree::new(RocksDBWrapper::from(db))?;
        Ok(Self { tree })
    }

    pub fn process_batch(&mut self, storage_logs: &[StorageLog]) -> anyhow::Result<TreeMetadata> {
        let l1_batch_number = self.tree.next_l1_batch_number();
        let current_leaf_count = if l1_batch_number.0 > 0 {
            self.tree.root_info(L1BatchNumber(l1_batch_number.0 - 1)).map(|(_, count)| count).unwrap_or(0)
        } else {
            0
        };
        let mut next_leaf_index = current_leaf_count + 1;

        let keys: Vec<_> = storage_logs.iter().map(|log| log.key.hashed_key_u256()).collect();
        let tree_reader = self.tree.reader();
        let prev_batch_number = L1BatchNumber(l1_batch_number.0.saturating_sub(1));

        let existing_entries = if l1_batch_number.0 > 0 {
             tree_reader.entries_with_proofs(prev_batch_number, &keys).ok()
        } else {
            None
        };

        let mut instructions = Vec::with_capacity(storage_logs.len());
        for (i, log) in storage_logs.iter().enumerate() {
            let hashed_key = keys[i];
            let existing_leaf_index = existing_entries.as_ref()
                .and_then(|entries| entries.get(i))
                .and_then(|entry| {
                    if entry.base.leaf_index > 0 {
                        Some(entry.base.leaf_index)
                    } else {
                        None
                    }
                });

            if log.is_write() {
                let leaf_index = if let Some(idx) = existing_leaf_index {
                    idx
                } else {
                    let idx = next_leaf_index;
                    next_leaf_index += 1;
                    idx
                };
                instructions.push(TreeInstruction::Write(TreeEntry {
                    key: hashed_key,
                    value: log.value,
                    leaf_index,
                }));
            } else {
                instructions.push(TreeInstruction::Read(hashed_key));
            }
        }

        let metadata = self.tree.process_l1_batch(&instructions)?;
        Ok(metadata)
    }

    pub fn save(&mut self) -> anyhow::Result<()> {
        self.tree.save()?;
        Ok(())
    }

    pub fn root_hash(&self) -> H256 {
        self.tree.root_hash()
    }

    pub fn next_l1_batch_number(&self) -> L1BatchNumber {
        self.tree.next_l1_batch_number()
    }
}
