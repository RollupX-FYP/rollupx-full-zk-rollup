use std::rc::Rc;
use std::cell::RefCell;
use std::str::FromStr;

use zksync_types::{
    Address, L1BatchNumber, Transaction, U256,
    fee_model::BatchFeeInput, AccountTreeId,
    utils::storage_key_for_eth_balance,
    h256_to_u256, settlement::SettlementLayer, SLChainId,
    get_code_key, get_known_code_key, H256, StorageKey,
};
use zksync_multivm::{
    interface::{
        VmInterface, L1BatchEnv, SystemEnv, L2BlockEnv, VmExecutionResultAndLogs,
        InspectExecutionMode, TxExecutionMode,
    },
    LegacyVmInstance,
    tracers::TracerDispatcher,
    vm_latest,
    pubdata_builders::FullPubdataBuilder,
};
use zksync_vm_interface::storage::{ReadStorage, StorageView, StoragePtr, WriteStorage};
use zksync_vm_interface::FinishedL1Batch;

pub mod types;
pub mod tree;
pub mod executor;

pub struct StateMachine<S: ReadStorage> {
    vm: LegacyVmInstance<S, vm_latest::HistoryEnabled>,
    storage: StoragePtr<StorageView<S>>,
    #[allow(dead_code)]
    l1_batch_env: L1BatchEnv,
    #[allow(dead_code)]
    system_env: SystemEnv,
}

impl<S: ReadStorage> StateMachine<S> {
    pub fn new(
        storage: S,
        l1_batch_env: L1BatchEnv,
        system_env: SystemEnv,
    ) -> Self {
        let storage_view = Rc::new(RefCell::new(StorageView::new(storage)));
        let vm = LegacyVmInstance::new_with_specific_version(
            l1_batch_env.clone(),
            system_env.clone(),
            storage_view.clone(),
            system_env.version.into(),
        );

        Self {
            vm,
            storage: storage_view,
            l1_batch_env,
            system_env,
        }
    }

    pub fn execute_transaction(&mut self, tx: Transaction) -> anyhow::Result<VmExecutionResultAndLogs> {
        self.vm.push_transaction(tx);

        let mut dispatcher = TracerDispatcher::default();

        let result = self.vm.inspect(&mut dispatcher, InspectExecutionMode::OneTx);
        Ok(result)
    }

    pub fn start_new_l2_block(&mut self, l2_block_env: L2BlockEnv) {
        self.vm.start_new_l2_block(l2_block_env);
    }

    pub fn get_base_balance(&self, address: Address) -> U256 {
        let key = storage_key_for_eth_balance(&address);
        let value = self.storage.borrow_mut().read_value(&key);
        h256_to_u256(value)
    }

    pub fn set_base_balance(&mut self, address: Address, balance: U256) {
        let l2_base_token = Address::from_low_u64_be(0x800a);
        let key = StorageKey::new(AccountTreeId::new(l2_base_token), zksync_types::utils::key_for_eth_balance(&address));
        let value = zksync_types::u256_to_h256(balance);
        self.storage.borrow_mut().set_value(key, value);

        let addr_01 = Address::from_low_u64_be(1);
        if l2_base_token != addr_01 {
             let key_01 = StorageKey::new(AccountTreeId::new(addr_01), zksync_types::utils::key_for_eth_balance(&address));
             self.storage.borrow_mut().set_value(key_01, value);
        }
    }

    pub fn set_account_bytecode(&mut self, address: Address, bytecode_hash: zksync_types::H256) {
        let key = get_code_key(&address);
        self.storage.borrow_mut().set_value(key, bytecode_hash);

        let known_code_key = get_known_code_key(&bytecode_hash);
        self.storage.borrow_mut().set_value(known_code_key, H256::from_low_u64_be(1));

        let is_account_key = zksync_types::get_is_account_key(&address);
        self.storage.borrow_mut().set_value(is_account_key, H256::from_low_u64_be(1));
    }

    pub fn storage(&self) -> StoragePtr<StorageView<S>> {
        self.storage.clone()
    }

    pub fn seal_batch(&mut self) -> FinishedL1Batch {
        let pubdata_builder = FullPubdataBuilder::new(zksync_types::commitment::PubdataParams::genesis().pubdata_validator());
        self.vm.finish_batch(Rc::new(pubdata_builder))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zksync_vm_interface::storage::InMemoryStorage;
    use zksync_types::{ProtocolVersionId, Nonce, fee::Fee, L2ChainId};
    use zksync_contracts::BaseSystemContracts;
    use zksync_types::l2::L2Tx;
    use zksync_crypto_primitives::K256PrivateKey;

    fn mock_signed_tx(recipient: Address, nonce: u32, chain_id: L2ChainId) -> Transaction {
        let pk = K256PrivateKey::from_bytes(H256::repeat_byte(0x01)).unwrap();
        let mut fee = Fee::default();
        fee.gas_per_pubdata_limit = U256::from(50000);
        fee.gas_limit = U256::from(1000000);
        fee.max_fee_per_gas = U256::from(1_000_000_000); // 1 Gwei
        fee.max_priority_fee_per_gas = U256::zero();

        let l2_tx = L2Tx::new_signed(
            Some(recipient), // Correct recipient
            vec![], // Calldata
            Nonce(nonce),
            fee,
            U256::from(100), // Value
            chain_id,
            &pk,
            vec![], // Factory deps
            Default::default(), // Paymaster params
        ).unwrap();
        l2_tx.into()
    }

    #[test]
    fn test_sequential_transactions() {
        let chain_id = L2ChainId::from(270);

        // 1. Storage with system contracts pre-loaded
        let storage = InMemoryStorage::with_system_contracts_and_chain_id(chain_id);

        // The expected prev block hash for block 1 is Keccak256(uint32(0))
        let prev_block_hash = H256::from_str("0xe8e77626586f73b955364c7b4bbf0bb7f7685ebd40e852b164633a4acbd3244c").unwrap();

        let l1_batch_env = L1BatchEnv {
            number: L1BatchNumber(1),
            timestamp: 100,
            fee_account: Address::repeat_byte(0x11),
            enforced_base_fee: None,
            first_l2_block: L2BlockEnv {
                number: 1,
                timestamp: 100,
                prev_block_hash,
                max_virtual_blocks_to_create: 3,
                interop_roots: vec![],
            },
            previous_batch_hash: Some(H256::zero()),
            fee_input: BatchFeeInput::pubdata_independent(1000, 1000, 1000),
            interop_fee: U256::zero(),
            settlement_layer: SettlementLayer::L1(SLChainId(9)),
        };

        let system_contracts = BaseSystemContracts::load_from_disk();
        let default_aa_hash = system_contracts.default_aa.hash;

        let system_env = SystemEnv {
            zk_porter_available: false,
            version: ProtocolVersionId::latest(),
            base_system_smart_contracts: system_contracts,
            bootloader_gas_limit: 100000000,
            execution_mode: TxExecutionMode::VerifyExecute,
            default_validation_computational_gas_limit: 10000000,
            chain_id,
        };

        let mut sm = StateMachine::new(storage, l1_batch_env, system_env);

        let pk = K256PrivateKey::from_bytes(H256::repeat_byte(0x01)).unwrap();
        let initiator = pk.address();
        let bob = Address::repeat_byte(0xbb);

        sm.set_base_balance(initiator, U256::from(10u128.pow(19))); // 10 ETH
        sm.set_account_bytecode(initiator, default_aa_hash);
        sm.set_base_balance(bob, U256::zero());

        let initial_initiator_balance = sm.get_base_balance(initiator);
        println!("Initial Initiator balance: {}", initial_initiator_balance);

        // 1. First transaction: Transfer to Bob
        let tx1 = mock_signed_tx(bob, 0, chain_id);
        println!("Executing tx1 via EraVM...");
        let result1 = sm.execute_transaction(tx1).unwrap();
        println!("Tx1 execution status: {:?}", result1.result);

        // Verify state transition
        let after_tx1_initiator_balance = sm.get_base_balance(initiator);
        let after_tx1_bob_balance = sm.get_base_balance(bob);

        println!("Initiator balance after tx1: {}", after_tx1_initiator_balance);
        println!("Bob balance after tx1: {}", after_tx1_bob_balance);

        // 2. Second transaction: Another transfer
        let tx2 = mock_signed_tx(bob, 1, chain_id);
        println!("Executing tx2 via EraVM...");
        let result2 = sm.execute_transaction(tx2).unwrap();
        println!("Tx2 execution status: {:?}", result2.result);

        let final_initiator_balance = sm.get_base_balance(initiator);
        let final_bob_balance = sm.get_base_balance(bob);

        println!("Final Initiator balance: {}", final_initiator_balance);
        println!("Final Bob balance: {}", final_bob_balance);

        // Final assertions
        assert!(!result1.result.is_failed(), "Transaction 1 failed: {:?}", result1.result);
        assert!(!result2.result.is_failed(), "Transaction 2 failed: {:?}", result2.result);
        assert!(final_initiator_balance < initial_initiator_balance, "Balance did not decrease!");
        assert!(final_bob_balance == U256::from(200), "Bob should have received 200 wei total!");
    }

    #[test]
    fn test_batch_processor() {
        let chain_id = L2ChainId::from(270);
        let storage = InMemoryStorage::with_system_contracts_and_chain_id(chain_id);
        let prev_block_hash = H256::from_str("0xe8e77626586f73b955364c7b4bbf0bb7f7685ebd40e852b164633a4acbd3244c").unwrap();

        let l1_batch_env = L1BatchEnv {
            number: L1BatchNumber(1),
            timestamp: 100,
            fee_account: Address::repeat_byte(0x11),
            enforced_base_fee: None,
            first_l2_block: L2BlockEnv {
                number: 1,
                timestamp: 100,
                prev_block_hash,
                max_virtual_blocks_to_create: 3,
                interop_roots: vec![],
            },
            previous_batch_hash: Some(H256::zero()),
            fee_input: BatchFeeInput::pubdata_independent(1000, 1000, 1000),
            interop_fee: U256::zero(),
            settlement_layer: SettlementLayer::L1(SLChainId(9)),
        };

        let system_contracts = BaseSystemContracts::load_from_disk();
        let default_aa_hash = system_contracts.default_aa.hash;

        let system_env = SystemEnv {
            zk_porter_available: false,
            version: ProtocolVersionId::latest(),
            base_system_smart_contracts: system_contracts,
            bootloader_gas_limit: 100000000,
            execution_mode: TxExecutionMode::VerifyExecute,
            default_validation_computational_gas_limit: 10000000,
            chain_id,
        };

        let temp_dir = tempfile::tempdir().unwrap();
        let mut processor = executor::BatchProcessor::new(
            storage,
            l1_batch_env.clone(),
            system_env.clone(),
            temp_dir.path(),
        ).unwrap();

        let pk = K256PrivateKey::from_bytes(H256::repeat_byte(0x01)).unwrap();
        let initiator = pk.address();
        let bob = Address::repeat_byte(0xbb);

        {
            let sm = processor.state_machine_mut();
            sm.set_base_balance(initiator, U256::from(10u128.pow(19))); // 10 ETH
            sm.set_account_bytecode(initiator, default_aa_hash);
            sm.set_base_balance(bob, U256::zero());
        }

        let tx1 = mock_signed_tx(bob, 0, chain_id);
        let tx2 = mock_signed_tx(bob, 1, chain_id);

        let input = types::BatchInput {
            l1_batch_env,
            system_env,
            transactions: vec![tx1, tx2],
        };

        println!("Running full batch processor...");
        let output = processor.process_batch(input).unwrap();

        println!("Batch processed!");
        println!("Root hash: {:?}", output.root_hash);
        println!("Pubdata size: {}", output.pubdata.len());
        println!("Witness present: {}", output.witness.is_some());

        assert!(output.root_hash != H256::zero());
        assert!(!output.pubdata.is_empty());
        assert!(output.witness.is_some());

        // Final state verification via the state machine
        let final_bob_balance = output.finished_batch.final_execution_state.deduplicated_storage_logs
            .iter()
            .find(|log| log.key == zksync_types::utils::storage_key_for_eth_balance(&bob))
            .map(|log| zksync_types::h256_to_u256(log.value))
            .unwrap_or(U256::zero());

        println!("Final Bob balance from logs: {}", final_bob_balance);
        assert_eq!(final_bob_balance, U256::from(200));
    }
}
