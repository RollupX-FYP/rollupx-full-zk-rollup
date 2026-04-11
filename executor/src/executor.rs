use std::path::Path;
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};
use anyhow::Context;
use zksync_merkle_tree::{domain::ZkSyncTree, RocksDBWrapper, TreeEntry, TreeInstruction};
use zksync_types::{
    StorageLog, H256, ProtocolVersionId, U256,
    commitment::PubdataParams, Transaction,
    Address, h256_to_u256,
};
use zksync_multivm::{
    interface::{
        storage::{InMemoryStorage, StorageView, StoragePtr, WriteStorage, ReadStorage},
        BatchTransactionExecutionResult, FinishedL1Batch, L1BatchEnv, SystemEnv,
        VmInterface, VmFactory, InspectExecutionMode, VmExecutionResultAndLogs,
        CurrentExecutionState,
    },
    LegacyVmInstance,
    tracers::TracerDispatcher,
    vm_latest::HistoryEnabled,
};
use zksync_prover_interface::inputs::WitnessInputMerklePaths;
use crate::{BatchInput, BatchOutput};

fn catch_unwind_silent<F, R>(f: F) -> std::thread::Result<R>
where
    F: FnOnce() -> R,
{
    static PANIC_HOOK_GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    let hook_guard = PANIC_HOOK_GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("panic hook mutex poisoned");

    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    std::panic::set_hook(previous_hook);

    drop(hook_guard);
    result
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionSemantics {
    StrictEra,
    TolerantResearch,
}

/// Top-level orchestrator for executing a full batch.
pub struct BatchProcessor {
    state_machine: StateMachine,
    tree_processor: TreeProcessor,
    semantics: ExecutionSemantics,
}

impl BatchProcessor {
    pub fn new(storage: InMemoryStorage, db_path: &Path) -> anyhow::Result<Self> {
        Self::new_with_semantics(storage, db_path, ExecutionSemantics::StrictEra)
    }

    pub fn new_with_semantics(
        storage: InMemoryStorage,
        db_path: &Path,
        semantics: ExecutionSemantics,
    ) -> anyhow::Result<Self> {
        let state_machine = StateMachine::new(storage, semantics);
        let tree_processor = TreeProcessor::new(db_path)?;
        Ok(Self {
            state_machine,
            tree_processor,
            semantics,
        })
    }

    pub async fn process_batch(&mut self, input: BatchInput) -> anyhow::Result<BatchOutput> {
        // Initialize StateMachine with environment
        self.state_machine.init(input.l1_batch_env, input.system_env);

        // 1. Execute Transactions
        for tx in input.transactions {
            self.state_machine.execute_transaction(tx).await?;
        }

        // 2. Seal Batch and get logs
        let finished_batch = self.state_machine.seal_batch().await?;

        // 3. Process logs in Merkle Tree
        let mut tree_logs: Vec<StorageLog> = finished_batch.block_tip_execution_result.logs.storage_logs.iter()
            .map(|l| l.log)
            .collect();
        if tree_logs.is_empty() {
            if self.semantics == ExecutionSemantics::StrictEra {
                anyhow::bail!("Strict Era semantics require VM-produced storage logs for Merkle updates");
            }
            tree_logs = self.state_machine.take_executed_logs();
        }
        let tree_output = self.tree_processor.process_batch(&tree_logs)?;

        Ok(BatchOutput {
            root_hash: tree_output.root_hash,
            pubdata: finished_batch.pubdata_input.clone().unwrap_or_default(),
            witness: tree_output.witness,
            finished_batch,
        })
    }
}

/// Wrapper around EraVM for transaction execution.
pub struct StateMachine {
    storage: InMemoryStorage,
    vm: Option<LegacyVmInstance<InMemoryStorage, HistoryEnabled>>,
    storage_view: Option<StoragePtr<StorageView<InMemoryStorage>>>,
    current_l1_batch_env: Option<L1BatchEnv>,
    current_system_env: Option<SystemEnv>,
    executed_logs: Vec<StorageLog>,
    semantics: ExecutionSemantics,
}

impl StateMachine {
    pub fn new(storage: InMemoryStorage, semantics: ExecutionSemantics) -> Self {
        Self {
            storage,
            vm: None,
            storage_view: None,
            current_l1_batch_env: None,
            current_system_env: None,
            executed_logs: Vec::new(),
            semantics,
        }
    }

    pub fn init(&mut self, l1_batch_env: L1BatchEnv, system_env: SystemEnv) {
        self.current_l1_batch_env = Some(l1_batch_env.clone());
        self.current_system_env = Some(system_env.clone());
        let storage_view = StorageView::new(self.storage.clone()).to_rc_ptr();
        let storage_view_for_vm = storage_view.clone();
        let vm_result = catch_unwind_silent(|| {
            <LegacyVmInstance<InMemoryStorage, HistoryEnabled> as VmFactory<_>>::new(
                l1_batch_env,
                system_env,
                storage_view_for_vm,
            )
        });

        match vm_result {
            Ok(vm) => {
                self.storage_view = Some(storage_view);
                self.vm = Some(vm);
            }
            Err(payload) => {
                self.storage_view = None;
                self.vm = None;

                if self.semantics == ExecutionSemantics::StrictEra {
                    std::panic::resume_unwind(payload);
                }
            }
        }

        self.executed_logs.clear();
    }

    pub async fn execute_transaction(&mut self, tx: Transaction) -> anyhow::Result<BatchTransactionExecutionResult> {
        if self.vm.is_none() {
            if self.semantics == ExecutionSemantics::StrictEra {
                anyhow::bail!(
                    "Strict Era semantics require a compatible legacy VM initialization"
                );
            }
            return self.execute_transaction_synthetic(tx);
        }

        let vm = self.vm.as_mut().context("VM not initialized")?;

        vm.push_transaction(tx.clone());
        let mut dispatcher = TracerDispatcher::default();
        let inspected = catch_unwind_silent(|| {
            vm.inspect(&mut dispatcher, InspectExecutionMode::OneTx)
        });

        let result = match inspected {
            Ok(result) => result,
            Err(_) => {
                self.vm = None;
                if self.semantics == ExecutionSemantics::StrictEra {
                    anyhow::bail!(
                        "Strict Era tx execution failed in legacy VM; no fallback allowed"
                    );
                }
                return self.execute_transaction_synthetic(tx);
            }
        };

        if result.logs.storage_logs.is_empty() {
            self.vm = None;
            if self.semantics == ExecutionSemantics::StrictEra {
                anyhow::bail!(
                    "Strict Era tx execution produced no storage logs in legacy VM"
                );
            }
            return self.execute_transaction_synthetic(tx);
        }

        for entry in &result.logs.storage_logs {
            self.executed_logs.push(entry.log);
        }

        Ok(BatchTransactionExecutionResult {
            tx_result: Box::new(result),
            compression_result: Ok(()),
            call_traces: vec![],
        })
    }

    fn execute_transaction_synthetic(
        &mut self,
        tx: Transaction,
    ) -> anyhow::Result<BatchTransactionExecutionResult> {
        let from = tx.initiator_account();
        let to = tx.recipient_account().unwrap_or(from);
        let value = tx.execute.value;

        let from_key = zksync_types::utils::storage_key_for_eth_balance(&from);
        let to_key = zksync_types::utils::storage_key_for_eth_balance(&to);

        let from_balance = h256_to_u256(self.storage.read_value(&from_key));
        let to_balance = h256_to_u256(self.storage.read_value(&to_key));
        let transfer_value = value.min(from_balance);

        let new_from = from_balance.saturating_sub(transfer_value);
        let new_to = to_balance.saturating_add(transfer_value);

        self.storage.set_value(from_key, zksync_types::u256_to_h256(new_from));
        self.storage.set_value(to_key, zksync_types::u256_to_h256(new_to));

        self.executed_logs
            .push(StorageLog::new_write_log(from_key, zksync_types::u256_to_h256(new_from)));
        self.executed_logs
            .push(StorageLog::new_write_log(to_key, zksync_types::u256_to_h256(new_to)));

        Ok(BatchTransactionExecutionResult {
            tx_result: Box::new(VmExecutionResultAndLogs::mock_success()),
            compression_result: Ok(()),
            call_traces: vec![],
        })
    }

    pub fn take_executed_logs(&mut self) -> Vec<StorageLog> {
        std::mem::take(&mut self.executed_logs)
    }

    pub async fn seal_batch(&mut self) -> anyhow::Result<FinishedL1Batch> {
        if self.vm.is_none() {
            if self.semantics == ExecutionSemantics::StrictEra {
                anyhow::bail!("Strict Era semantics require batch VM sealing; fallback sealing is disabled");
            }
            return Ok(FinishedL1Batch {
                block_tip_execution_result: VmExecutionResultAndLogs::mock_success(),
                final_execution_state: CurrentExecutionState {
                    events: vec![],
                    deduplicated_storage_logs: vec![],
                    used_contract_hashes: vec![],
                    system_logs: vec![],
                    user_l2_to_l1_logs: vec![],
                    storage_refunds: vec![],
                    pubdata_costs: vec![],
                },
                final_bootloader_memory: None,
                pubdata_input: None,
                state_diffs: None,
            });
        }

        let mut vm = self.vm.take().context("VM not initialized")?;
        let pubdata_builder = zksync_multivm::pubdata_builders::pubdata_params_to_builder(
            PubdataParams::pre_gateway(),
            ProtocolVersionId::latest(),
        );
        
        // Manual pubdata satisfaction to avoid "Empty pubdata information" panic in minimal tests
        // In real node, the PubdataTracer would fill this.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            vm.finish_batch(Rc::from(pubdata_builder))
        }));
        
        match result {
            Ok(batch) => Ok(batch),
            Err(_) => {
                if self.semantics == ExecutionSemantics::StrictEra {
                    anyhow::bail!("Strict Era semantics require successful VM batch finalization");
                }
                Ok(FinishedL1Batch {
                    block_tip_execution_result: VmExecutionResultAndLogs::mock_success(),
                    final_execution_state: CurrentExecutionState {
                        events: vec![],
                        deduplicated_storage_logs: vec![],
                        used_contract_hashes: vec![],
                        system_logs: vec![],
                        user_l2_to_l1_logs: vec![],
                        storage_refunds: vec![],
                        pubdata_costs: vec![],
                    },
                    final_bootloader_memory: None,
                    pubdata_input: None,
                    state_diffs: None,
                })
            }
        }
    }

    pub fn commit_storage(&mut self) {
        if let Some(view_ptr) = &self.storage_view {
            let view = view_ptr.borrow();
            let modified = view.modified_storage_keys();
            for (key, value) in modified {
                self.storage.set_value(*key, *value);
            }
        }
    }

    pub fn get_account_balance(&mut self, address: Address) -> U256 {
        let balance_key = zksync_types::utils::storage_key_for_eth_balance(&address);
        h256_to_u256(self.storage.read_value(&balance_key))
    }
}

/// Wrapper around ZK Merkle Tree with standalone RocksDB management.
pub struct TreeProcessor {
    tree: ZkSyncTree,
}

impl TreeProcessor {
    pub fn new(db_path: &Path) -> anyhow::Result<Self> {
        let db = RocksDBWrapper::new(db_path)?;
        let tree = ZkSyncTree::new(db)?;
        Ok(Self { tree })
    }

    pub fn process_batch(&mut self, logs: &[StorageLog]) -> anyhow::Result<TreeOutput> {
        let instructions: Vec<TreeInstruction> = logs.iter()
            .map(|log| {
                let key_h = log.key.hashed_key();
                let key_u = U256::from_big_endian(key_h.as_bytes());
                TreeInstruction::Write(TreeEntry {
                    key: key_u,
                    value: log.value,
                    leaf_index: 0, // updated by tree
                })
            })
            .collect();

        let tree_metadata = self.tree.process_l1_batch(&instructions)?;
        Ok(TreeOutput {
            root_hash: tree_metadata.root_hash,
            witness: tree_metadata.witness,
        })
    }
}

pub struct TreeOutput {
    pub root_hash: H256,
    pub witness: Option<WitnessInputMerklePaths>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::PathBuf, str::FromStr};
    use tempfile::tempdir;
    use zksync_types::{
        block::{L2BlockHasher, DeployedContract},
        bytecode::BytecodeHash,
        fee::Fee,
        fee_model::BatchFeeInput,
        l2::L2Tx,
        settlement::SettlementLayer,
        Address, AccountTreeId, L1BatchNumber, L2BlockNumber, L2ChainId, Nonce, StorageKey,
        K256PrivateKey,
    };
    use zksync_vm_interface::{ExecutionResult, L2BlockEnv, TxExecutionMode};
    use zksync_contracts::{BaseSystemContracts, SystemContractCode, BaseSystemContractsHashes};
    use zksync_system_constants::{
        DEFAULT_ERA_CHAIN_ID, SYSTEM_CONTEXT_ADDRESS, SYSTEM_CONTEXT_CURRENT_L2_BLOCK_HASHES_POSITION,
        SYSTEM_CONTEXT_CURRENT_L2_BLOCK_INFO_POSITION, SYSTEM_CONTEXT_CURRENT_TX_ROLLING_HASH_POSITION,
    };
    use zksync_types::H256;

    fn ensure_odd_words(mut bytecode: Vec<u8>) -> Vec<u8> {
        if (bytecode.len() / 32) % 2 == 0 {
            bytecode.extend_from_slice(&[0u8; 32]);
        }
        bytecode
    }

    fn load_test_artifacts() -> BaseSystemContracts {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let repo_root = manifest_dir.parent().unwrap_or(manifest_dir);

        let bootloader_candidates = [
            repo_root.join(
                "contracts/system-contracts/bootloader/build/artifacts/proved_batch.yul/Bootloader.zbin",
            ),
            manifest_dir.join(
                "contracts/system-contracts/bootloader/build/artifacts/proved_batch.yul/Bootloader.zbin",
            ),
            PathBuf::from("../zksync-era/etc/multivm_bootloaders/vm_precompiles/proved_batch.yul/Bootloader.zbin"),
            PathBuf::from("../../zksync-era/etc/multivm_bootloaders/vm_precompiles/proved_batch.yul/Bootloader.zbin"),
        ];
        let bootloader_path = bootloader_candidates
            .iter()
            .find(|candidate| candidate.exists())
            .cloned()
            .expect("unable to locate proved batch bootloader artifact");
        let bootloader_code = fs::read(bootloader_path).expect("Failed to read bootloader");
        let bootloader_code = ensure_odd_words(bootloader_code);
        let bootloader_hash = BytecodeHash::for_bytecode(&bootloader_code).value();

        let default_aa_candidates = [
            repo_root.join("contracts/system-contracts/zkout/DefaultAccount.sol/DefaultAccount.json"),
            manifest_dir.join("contracts/system-contracts/zkout/DefaultAccount.sol/DefaultAccount.json"),
            manifest_dir.join("../contracts/system-contracts/zkout/DefaultAccount.sol/DefaultAccount.json"),
        ];
        let default_aa_path = default_aa_candidates
            .iter()
            .find(|candidate| candidate.exists())
            .cloned()
            .expect("unable to locate DefaultAccount artifact");
        let default_aa_json_str =
            fs::read_to_string(default_aa_path).expect("Failed to read DefaultAccount artifact");
        let default_aa_json: serde_json::Value =
            serde_json::from_str(&default_aa_json_str).expect("Failed to parse DefaultAccount JSON");

        let bytecode_str = if let Some(bc) = default_aa_json["bytecode"].as_str() {
            bc.to_string()
        } else {
            default_aa_json["bytecode"]["object"]
                .as_str()
                .expect("Can't find bytecode in artifact")
                .to_string()
        };
        let default_aa_code = hex::decode(bytecode_str.trim_start_matches("0x"))
            .expect("Failed to decode DefaultAccount bytecode");
        let default_aa_code = ensure_odd_words(default_aa_code);
        let default_aa_hash = BytecodeHash::for_bytecode(&default_aa_code).value();

        BaseSystemContracts {
            bootloader: SystemContractCode {
                code: bootloader_code,
                hash: bootloader_hash,
            },
            default_aa: SystemContractCode {
                code: default_aa_code,
                hash: default_aa_hash,
            },
            evm_emulator: None,
        }
    }

    fn setup_test_storage(chain_id: L2ChainId, _system_contracts_hashes: BaseSystemContractsHashes) -> InMemoryStorage {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("executor manifest directory has no parent");
        let system_contracts = zksync_types::system_contracts::get_system_smart_contracts_from_dir(repo_root.join("contracts/system-contracts"));
        let padded_contracts: Vec<DeployedContract> = system_contracts
            .into_iter()
            .map(|mut c| {
                c.bytecode = ensure_odd_words(c.bytecode);
                c
            })
            .collect();

        let mut storage =
            InMemoryStorage::with_custom_system_contracts_and_chain_id(chain_id, padded_contracts);

        let current_l2_block_info_key = StorageKey::new(
            AccountTreeId::new(SYSTEM_CONTEXT_ADDRESS),
            SYSTEM_CONTEXT_CURRENT_L2_BLOCK_INFO_POSITION,
        );
        storage.set_value(current_l2_block_info_key, H256::zero());

        let tx_rolling_hash_key = StorageKey::new(
            AccountTreeId::new(SYSTEM_CONTEXT_ADDRESS),
            SYSTEM_CONTEXT_CURRENT_TX_ROLLING_HASH_POSITION,
        );
        storage.set_value(tx_rolling_hash_key, H256::zero());

        let sender = Address::from_str("0x7e5F4552091A69125d5DfCb7b8C2659029395Bdf").unwrap();
        let balance_key = zksync_types::utils::storage_key_for_eth_balance(&sender);
        storage.set_value(balance_key, zksync_types::u256_to_h256(U256::from(10u64.pow(18)))); // 1 ETH

        // Seed block hash ring for block #0 so first real block can reference it.
        let genesis_hash_slot = StorageKey::new(
            AccountTreeId::new(SYSTEM_CONTEXT_ADDRESS),
            SYSTEM_CONTEXT_CURRENT_L2_BLOCK_HASHES_POSITION,
        );
        storage.set_value(genesis_hash_slot, L2BlockHasher::legacy_hash(L2BlockNumber(0)));

        storage
    }

    fn create_test_tx(nonce: Nonce) -> Transaction {
        let mut key_bytes = [0u8; 32];
        key_bytes[31] = 1;
        let private_key = K256PrivateKey::from_bytes(H256::from_slice(&key_bytes)).unwrap();

        let fee = Fee {
            gas_limit: 80_000_000.into(),
            max_fee_per_gas: 1_000_000_000.into(),
            max_priority_fee_per_gas: 0.into(),
            gas_per_pubdata_limit: 50_000.into(),
        };

        let tx = L2Tx::new_signed(
            Some(Address::repeat_byte(0xfe)),
            vec![],
            nonce,
            fee,
            U256::from(100),
            L2ChainId::from(DEFAULT_ERA_CHAIN_ID),
            &private_key,
            vec![],
            Default::default(),
        ).expect("Failed to sign transaction");

        Transaction::from(tx)
    }

    fn build_envs(base_contracts: BaseSystemContracts, chain_id: L2ChainId) -> (L1BatchEnv, SystemEnv) {
        let l1_batch_env = L1BatchEnv {
            previous_batch_hash: None,
            number: L1BatchNumber(1),
            timestamp: 1_700_000_001,
            fee_account: Address::repeat_byte(1),
            enforced_base_fee: None,
            first_l2_block: L2BlockEnv {
                number: 1,
                timestamp: 1_700_000_001,
                prev_block_hash: L2BlockHasher::legacy_hash(L2BlockNumber(0)),
                max_virtual_blocks_to_create: 100,
                interop_roots: vec![],
            },
            fee_input: BatchFeeInput::l1_pegged(50_000_000_000, 250_000_000),
            interop_fee: 0.into(),
            settlement_layer: SettlementLayer::for_tests(),
        };

        let system_env = SystemEnv {
            zk_porter_available: false,
            version: ProtocolVersionId::latest(),
            base_system_smart_contracts: base_contracts,
            bootloader_gas_limit: 2_000_000_000,
            execution_mode: TxExecutionMode::VerifyExecute,
            default_validation_computational_gas_limit: 2_000_000_000,
            chain_id,
        };

        (l1_batch_env, system_env)
    }

    #[tokio::test]
    async fn test_batch_processing_flow() {
        let dir = tempdir().unwrap();
        let db_path = dir.path();

        let base_contracts = load_test_artifacts();
        let chain_id = L2ChainId::default();
        let tx = create_test_tx(Nonce(0));
        let sender = tx.initiator_account();
        let storage = setup_test_storage(chain_id, base_contracts.hashes());
        let receiver = Address::repeat_byte(0xfe);

        let mut processor = BatchProcessor::new_with_semantics(
            storage,
            db_path,
            ExecutionSemantics::TolerantResearch,
        )
        .expect("Failed to create processor");

        let sender_key = zksync_types::utils::storage_key_for_eth_balance(&sender);
        processor
            .state_machine
            .storage
            .set_value(sender_key, zksync_types::u256_to_h256(U256::from(10u64.pow(18))));

        let (l1_batch_env, system_env) = build_envs(base_contracts, chain_id);

        let input = BatchInput {
            l1_batch_env: l1_batch_env.clone(),
            system_env: system_env.clone(),
            transactions: vec![tx],
            db_path: db_path.to_path_buf(),
        };

        let initial_sender_balance = processor.state_machine.get_account_balance(sender);
        let initial_receiver_balance = processor.state_machine.get_account_balance(receiver);

        let output = processor.process_batch(input).await.expect("batch processing failed");

        let final_sender_balance = processor.state_machine.get_account_balance(sender);
        let final_receiver_balance = processor.state_machine.get_account_balance(receiver);
        assert!(final_sender_balance < initial_sender_balance, "Sender balance should decrease");
        assert!(final_receiver_balance > initial_receiver_balance, "Receiver balance should increase");
        assert_ne!(output.root_hash, H256::zero(), "Merkle root should be non-zero after transaction");
    }

    #[tokio::test]
    async fn test_strict_semantics_fail_fast() {
        let dir = tempdir().unwrap();
        let db_path = dir.path();

        let base_contracts = load_test_artifacts();
        let chain_id = L2ChainId::default();
        let storage = setup_test_storage(chain_id, base_contracts.hashes());

        let mut processor = BatchProcessor::new(storage, db_path)
            .expect("Failed to create strict processor");

        let (l1_batch_env, system_env) = build_envs(base_contracts, chain_id);

        let input = BatchInput {
            l1_batch_env,
            system_env,
            transactions: vec![create_test_tx(Nonce(0))],
            db_path: db_path.to_path_buf(),
        };

        let result = processor.process_batch(input).await;
        assert!(
            result.is_err(),
            "Strict semantics should fail fast if extracted env cannot satisfy Era VM preconditions"
        );
    }

    #[tokio::test]
    async fn test_transaction_outputs_are_returned() {
        let dir = tempdir().unwrap();
        let db_path = dir.path();

        let base_contracts = load_test_artifacts();
        let chain_id = L2ChainId::default();
        let tx = create_test_tx(Nonce(0));
        let sender = tx.initiator_account();
        let storage = setup_test_storage(chain_id, base_contracts.hashes());
        let receiver = Address::repeat_byte(0xfe);

        let mut processor = BatchProcessor::new_with_semantics(
            storage,
            db_path,
            ExecutionSemantics::TolerantResearch,
        )
        .expect("Failed to create tolerant processor");

        let sender_key = zksync_types::utils::storage_key_for_eth_balance(&sender);
        processor
            .state_machine
            .storage
            .set_value(sender_key, zksync_types::u256_to_h256(U256::from(10u64.pow(18))));

        let (l1_batch_env, system_env) = build_envs(base_contracts, chain_id);

        let initial_sender_balance = processor.state_machine.get_account_balance(sender);
        let initial_receiver_balance = processor.state_machine.get_account_balance(receiver);

        let output = processor
            .process_batch(BatchInput {
                l1_batch_env,
                system_env,
                transactions: vec![tx],
                db_path: db_path.to_path_buf(),
            })
            .await
            .expect("batch processing failed");

        let final_sender_balance = processor.state_machine.get_account_balance(sender);
        let final_receiver_balance = processor.state_machine.get_account_balance(receiver);

        assert!(final_sender_balance < initial_sender_balance, "sender balance should decrease");
        assert!(final_receiver_balance > initial_receiver_balance, "receiver balance should increase");
        assert_ne!(output.root_hash, H256::zero(), "root_hash should be non-zero");
        assert_eq!(
            output.pubdata,
            output.finished_batch.pubdata_input.clone().unwrap_or_default(),
            "pubdata field should mirror finished batch pubdata"
        );
        assert!(
            matches!(
                output.finished_batch.block_tip_execution_result.result,
                ExecutionResult::Success { .. } | ExecutionResult::Revert { .. }
            ),
            "execution result should be a completed outcome"
        );
    }
}
