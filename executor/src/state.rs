use crate::merkle::{MerkleTree, Sha256SparseMerkle};
use crate::types::{Account, Address, ExecutorError, Hash, StateDiff};
use rocksdb::{Options, DB};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use zksync_merkle_tree::{MerkleTree as JmtTree, RocksDBWrapper, TreeEntry, TreeInstruction};
use zksync_storage::RocksDB as ZkRocksDB;
use zksync_types::{H256, U256};

const ACCOUNT_PREFIX: &[u8] = b"acct:";
const LEAF_INDEX_PREFIX: &[u8] = b"leafidx:";
const NEXT_LEAF_INDEX_KEY: &[u8] = b"meta:next_leaf_index";

pub trait StateManager: Send + Sync {
    fn get_account(&self, address: &Address) -> Account;
    fn set_account(&mut self, address: Address, account: Account) -> Result<StateDiff, ExecutorError>;
    fn current_root(&self) -> Hash;
}

pub struct InMemoryStateManager {
    accounts: BTreeMap<Address, Account>,
    tree: Box<dyn MerkleTree>,
}

impl Default for InMemoryStateManager {
    fn default() -> Self {
        Self {
            accounts: BTreeMap::new(),
            tree: Box::<Sha256SparseMerkle>::default(),
        }
    }
}

impl InMemoryStateManager {
    pub fn seed_account(&mut self, address: Address, account: Account) {
        self.accounts.insert(address, account);
    }

    pub fn accounts(&self) -> &BTreeMap<Address, Account> {
        &self.accounts
    }
}

impl StateManager for InMemoryStateManager {
    fn get_account(&self, address: &Address) -> Account {
        self.accounts.get(address).cloned().unwrap_or_default()
    }

    fn set_account(&mut self, address: Address, account: Account) -> Result<StateDiff, ExecutorError> {
        let old = self.get_account(&address);
        self.accounts.insert(address, account.clone());
        let proof = self.tree.prove_account(&self.accounts, &address);

        Ok(StateDiff {
            account: address,
            old_balance: old.balance,
            new_balance: account.balance,
            old_nonce: old.nonce,
            new_nonce: account.nonce,
            merkle_proof: proof,
            witness_path: vec![],
            leaf_encoding: String::new(),
        })
    }

    fn current_root(&self) -> Hash {
        self.tree.root(&self.accounts)
    }
}

pub struct RocksDbStateManager {
    jmt: JmtTree<RocksDBWrapper>,
    account_db: DB,
}

impl RocksDbStateManager {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, ExecutorError> {
        let base = path.as_ref();
        std::fs::create_dir_all(base).map_err(|e| ExecutorError::State(format!("mkdir state base: {e}")))?;

        let jmt_path = base.join("jmt");
        let account_path = base.join("accounts");

        std::fs::create_dir_all(&jmt_path).map_err(|e| ExecutorError::State(format!("mkdir jmt db: {e}")))?;
        std::fs::create_dir_all(&account_path).map_err(|e| ExecutorError::State(format!("mkdir account db: {e}")))?;

        let jmt_db = ZkRocksDB::new(&PathBuf::from(&jmt_path))
            .map_err(|e| ExecutorError::State(format!("open jmt rocksdb: {e}")))?;
        let jmt = JmtTree::new(RocksDBWrapper::from(jmt_db))
            .map_err(|e| ExecutorError::State(format!("init jmt: {e}")))?;

        let mut opts = Options::default();
        opts.create_if_missing(true);
        let account_db = DB::open(&opts, account_path)
            .map_err(|e| ExecutorError::State(format!("open account rocksdb: {e}")))?;

        Ok(Self { jmt, account_db })
    }

    pub fn seed_account(&mut self, address: Address, account: Account) -> Result<(), ExecutorError> {
        self.set_account(address, account).map(|_| ())
    }

    fn account_key(address: &Address) -> Vec<u8> {
        let mut key = Vec::with_capacity(ACCOUNT_PREFIX.len() + 20);
        key.extend_from_slice(ACCOUNT_PREFIX);
        key.extend_from_slice(address);
        key
    }

    fn leaf_index_key(address: &Address) -> Vec<u8> {
        let mut key = Vec::with_capacity(LEAF_INDEX_PREFIX.len() + 20);
        key.extend_from_slice(LEAF_INDEX_PREFIX);
        key.extend_from_slice(address);
        key
    }

    fn load_next_leaf_index(&self) -> Result<u64, ExecutorError> {
        match self.account_db.get(NEXT_LEAF_INDEX_KEY) {
            Ok(Some(bytes)) => {
                if bytes.len() != 8 {
                    return Err(ExecutorError::State("corrupt next leaf index bytes".to_string()));
                }
                let mut arr = [0u8; 8];
                arr.copy_from_slice(&bytes);
                Ok(u64::from_be_bytes(arr))
            }
            Ok(None) => Ok(1),
            Err(e) => Err(ExecutorError::State(format!("read next leaf index: {e}"))),
        }
    }

    fn store_next_leaf_index(&self, idx: u64) -> Result<(), ExecutorError> {
        self.account_db
            .put(NEXT_LEAF_INDEX_KEY, idx.to_be_bytes())
            .map_err(|e| ExecutorError::State(format!("write next leaf index: {e}")))
    }

    fn get_or_assign_leaf_index(&self, address: &Address) -> Result<u64, ExecutorError> {
        let key = Self::leaf_index_key(address);
        if let Some(bytes) = self
            .account_db
            .get(&key)
            .map_err(|e| ExecutorError::State(format!("read leaf index: {e}")))?
        {
            if bytes.len() != 8 {
                return Err(ExecutorError::State("corrupt leaf index bytes".to_string()));
            }
            let mut arr = [0u8; 8];
            arr.copy_from_slice(&bytes);
            return Ok(u64::from_be_bytes(arr));
        }

        let idx = self.load_next_leaf_index()?;
        self.account_db
            .put(&key, idx.to_be_bytes())
            .map_err(|e| ExecutorError::State(format!("write leaf index: {e}")))?;
        self.store_next_leaf_index(idx.saturating_add(1))?;
        Ok(idx)
    }

    fn persist_account_blob(&self, address: &Address, account: &Account) -> Result<(), ExecutorError> {
        let key = Self::account_key(address);
        let val = bincode::serialize(account)
            .map_err(|e| ExecutorError::State(format!("encode account blob: {e}")))?;
        self.account_db
            .put(key, val)
            .map_err(|e| ExecutorError::State(format!("write account blob: {e}")))
    }

    fn account_value_hash(account: &Account) -> H256 {
        let mut bytes = [0u8; 32];
        bytes[..8].copy_from_slice(&account.balance.to_be_bytes());
        bytes[8..16].copy_from_slice(&account.nonce.to_be_bytes());
        H256::from(bytes)
    }

    fn key_from_address(address: &Address) -> U256 {
        let mut bytes = [0u8; 32];
        bytes[12..].copy_from_slice(address);
        U256::from_big_endian(&bytes)
    }

    fn h256_to_array(hash: H256) -> Hash {
        hash.0
    }
}

impl StateManager for RocksDbStateManager {
    fn get_account(&self, address: &Address) -> Account {
        let key = Self::account_key(address);
        match self.account_db.get(key) {
            Ok(Some(v)) => bincode::deserialize(&v).unwrap_or_default(),
            Ok(None) => Account::default(),
            Err(_) => Account::default(),
        }
    }

    fn set_account(&mut self, address: Address, account: Account) -> Result<StateDiff, ExecutorError> {
        let old = self.get_account(&address);
        let leaf_index = self.get_or_assign_leaf_index(&address)?;
        let key = Self::key_from_address(&address);
        let value_hash = Self::account_value_hash(&account);

        let block_output = self
            .jmt
            .extend_with_proofs(vec![TreeInstruction::write(key, leaf_index, value_hash)])
            .map_err(|e| ExecutorError::State(format!("jmt extend_with_proofs: {e}")))?;

        let proof = block_output
            .logs
            .first()
            .map(|log| {
                log.merkle_path
                    .iter()
                    .copied()
                    .map(Self::h256_to_array)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        self.persist_account_blob(&address, &account)?;

        Ok(StateDiff {
            account: address,
            old_balance: old.balance,
            new_balance: account.balance,
            old_nonce: old.nonce,
            new_nonce: account.nonce,
            merkle_proof: proof,
            witness_path: vec![],
            leaf_encoding: "jmt_valuehash_v1".to_string(),
        })
    }

    fn current_root(&self) -> Hash {
        Self::h256_to_array(self.jmt.latest_root_hash())
    }
}
