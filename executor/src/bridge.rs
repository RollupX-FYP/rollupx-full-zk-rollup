use std::{path::{Path, PathBuf}, str::FromStr};

use anyhow::{Context, Result};
use ethers::utils::keccak256;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use zksync_contracts::{BaseSystemContracts, BaseSystemContractsHashes, SystemContractCode};
use zksync_multivm::interface::{
    storage::{InMemoryStorage, ReadStorage}, L1BatchEnv,
    SystemEnv,
};
use zksync_prover_interface::inputs::WitnessInputMerklePaths;
use zksync_system_constants::{
    DEFAULT_ERA_CHAIN_ID, SYSTEM_CONTEXT_ADDRESS,
    SYSTEM_CONTEXT_CURRENT_L2_BLOCK_HASHES_POSITION,
    SYSTEM_CONTEXT_CURRENT_L2_BLOCK_INFO_POSITION,
    SYSTEM_CONTEXT_CURRENT_TX_ROLLING_HASH_POSITION,
    SYSTEM_CONTEXT_STORED_L2_BLOCK_HASHES,
};
use zksync_types::{
    block::{DeployedContract, L2BlockHasher, unpack_block_info},
    bytecode::BytecodeHash,
    fee::Fee,
    fee_model::BatchFeeInput,
    l2::L2Tx,
    settlement::SettlementLayer,
    system_contracts::get_system_smart_contracts_from_dir,
    utils::storage_key_for_eth_balance,
    Address, AccountTreeId, H256, L1BatchNumber, L2BlockNumber, L2ChainId, Nonce,
    StorageKey, Transaction, U256,
    h256_to_u256,
};
use zksync_vm_interface::{L2BlockEnv, TxExecutionMode};

use crate::{BatchInput, executor::{BatchProcessor, ExecutionSemantics}};

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct SequencerLegacyBatchOutput {
    batch_number: u64,
    pre_state_root: String,
    root_hash: String,
    post_state_root: String,
    pubdata: String,
    batch_data: String,
    da_commitment: String,
    proof: String,
    batch_id: String,
    experiment_id: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
struct ExecutorLegacyBatchOutput {
    batch_number: u64,
    pre_state_root: String,
    root_hash: String,
    post_state_root: String,
    pubdata: String,
    batch_data: String,
    da_commitment: String,
    proof: String,
    batch_id: String,
    experiment_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct ExecutorProverOutput {
    batch_id: String,
    root_hash: String,
    pubdata: String,
    witness: Option<WitnessInputMerklePaths>,
    storage_log_count: usize,
    finished_batch_has_pubdata: bool,
}

fn h256_to_hex(value: H256) -> String {
    format!("{:#066x}", value)
}

fn ensure_odd_words(mut bytecode: Vec<u8>) -> Vec<u8> {
    if (bytecode.len() / 32) % 2 == 0 {
        bytecode.extend_from_slice(&[0u8; 32]);
    }
    bytecode
}

fn decode_hex_field(value: &str) -> Result<Vec<u8>> {
    Ok(hex::decode(value.trim_start_matches("0x"))?)
}

fn encode_hex_field(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}

fn parse_address(value: &Value) -> Result<Address> {
    let address = value
        .as_str()
        .context("expected transaction address to be a string")?;
    Ok(Address::from_str(address).context("invalid transaction address")?)
}

fn parse_u256(value: &Value) -> Result<U256> {
    if let Some(raw) = value.as_str() {
        let raw = raw.trim();
        if let Some(hex_value) = raw.strip_prefix("0x") {
            return Ok(U256::from_str_radix(hex_value, 16).context("invalid hex U256")?);
        }

        return Ok(U256::from_dec_str(raw).context("invalid decimal U256")?);
    }

    if let Some(number) = value.as_u64() {
        return Ok(U256::from(number));
    }

    if let Some(array) = value.as_array() {
        let mut bytes = [0u8; 32];
        let start = bytes
            .len()
            .checked_sub(array.len())
            .context("U256 array is longer than 32 bytes")?;

        for (offset, byte) in array.iter().enumerate() {
            bytes[start + offset] = byte
                .as_u64()
                .context("U256 array entries must be integers")? as u8;
        }

        return Ok(U256::from_big_endian(&bytes));
    }

    anyhow::bail!("unsupported U256 encoding")
}

fn parse_nonce(value: &Value) -> Result<Nonce> {
    let nonce = parse_u256(value)?;
    if nonce > U256::from(u32::MAX) {
        anyhow::bail!("nonce does not fit into u32");
    }
    let nonce_u32 = nonce.as_u32();
    Ok(Nonce(nonce_u32))
}

fn create_base_contracts() -> Result<BaseSystemContracts> {
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
        .context("unable to locate proved batch bootloader artifact")?;
    let bootloader_code = ensure_odd_words(std::fs::read(&bootloader_path).context("read bootloader")?);
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
        .context("unable to locate DefaultAccount artifact")?;
    let default_aa_json_str = std::fs::read_to_string(default_aa_path)
        .context("read DefaultAccount artifact")?;
    let default_aa_json: Value = serde_json::from_str(&default_aa_json_str)
        .context("parse DefaultAccount artifact")?;

    let bytecode_str = if let Some(bytecode) = default_aa_json["bytecode"].as_str() {
        bytecode.to_string()
    } else {
        default_aa_json["bytecode"]["object"]
            .as_str()
            .context("missing DefaultAccount bytecode")?
            .to_string()
    };
    let default_aa_code = ensure_odd_words(
        hex::decode(bytecode_str.trim_start_matches("0x"))
            .context("decode DefaultAccount bytecode")?,
    );
    let default_aa_hash = BytecodeHash::for_bytecode(&default_aa_code).value();

    Ok(BaseSystemContracts {
        bootloader: SystemContractCode {
            code: bootloader_code,
            hash: bootloader_hash,
        },
        default_aa: SystemContractCode {
            code: default_aa_code,
            hash: default_aa_hash,
        },
        evm_emulator: None,
    })
}

fn setup_test_storage(chain_id: L2ChainId, _system_contracts_hashes: BaseSystemContractsHashes) -> InMemoryStorage {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("executor manifest directory has no parent");
    let system_contracts = get_system_smart_contracts_from_dir(repo_root.join("contracts/system-contracts"));
    let padded_contracts: Vec<DeployedContract> = system_contracts
        .into_iter()
        .map(|mut contract| {
            if (contract.bytecode.len() / 32) % 2 == 0 {
                contract.bytecode.extend_from_slice(&[0u8; 32]);
            }
            contract
        })
        .collect();

    let mut storage = InMemoryStorage::with_custom_system_contracts_and_chain_id(
        chain_id,
        padded_contracts,
    );

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

    let genesis_hash_slot = StorageKey::new(
        AccountTreeId::new(SYSTEM_CONTEXT_ADDRESS),
        SYSTEM_CONTEXT_CURRENT_L2_BLOCK_HASHES_POSITION,
    );
    storage.set_value(genesis_hash_slot, L2BlockHasher::legacy_hash(L2BlockNumber(0)));

    storage
}

fn build_envs(
    storage: &mut InMemoryStorage,
    base_contracts: BaseSystemContracts,
    chain_id: L2ChainId,
    batch_number: u64,
) -> (L1BatchEnv, SystemEnv) {
    let current_l2_block_info_key = StorageKey::new(
        AccountTreeId::new(SYSTEM_CONTEXT_ADDRESS),
        SYSTEM_CONTEXT_CURRENT_L2_BLOCK_INFO_POSITION,
    );
    let current_l2_block_info = h256_to_u256(storage.read_value(&current_l2_block_info_key));
    let (current_l2_block_number_u64, current_l2_block_timestamp) =
        unpack_block_info(current_l2_block_info);
    let current_l2_block_number = current_l2_block_number_u64 as u32;

    let (first_l2_block_number, prev_block_hash, min_timestamp) = if current_l2_block_number == 0 {
        (
            batch_number as u32,
            L2BlockHasher::legacy_hash(L2BlockNumber(0)),
            0,
        )
    } else {
        let prev_hash_position =
            h256_to_u256(SYSTEM_CONTEXT_CURRENT_L2_BLOCK_HASHES_POSITION)
                + U256::from((current_l2_block_number - 1) % SYSTEM_CONTEXT_STORED_L2_BLOCK_HASHES);
        let prev_hash_key = StorageKey::new(
            AccountTreeId::new(SYSTEM_CONTEXT_ADDRESS),
            zksync_types::u256_to_h256(prev_hash_position),
        );
        let prev_l2_block_hash = storage.read_value(&prev_hash_key);

        let rolling_hash_key = StorageKey::new(
            AccountTreeId::new(SYSTEM_CONTEXT_ADDRESS),
            SYSTEM_CONTEXT_CURRENT_TX_ROLLING_HASH_POSITION,
        );
        let txs_rolling_hash = storage.read_value(&rolling_hash_key);

        // Reproduce VM block-hash derivation for continuity checks at startup.
        let mut digest = [0u8; 128];
        U256::from(current_l2_block_number).to_big_endian(&mut digest[0..32]);
        U256::from(current_l2_block_timestamp).to_big_endian(&mut digest[32..64]);
        digest[64..96].copy_from_slice(prev_l2_block_hash.as_bytes());
        digest[96..128].copy_from_slice(txs_rolling_hash.as_bytes());
        let current_l2_block_hash = H256::from_slice(&keccak256(digest));

        (
            current_l2_block_number.saturating_add(1),
            current_l2_block_hash,
            current_l2_block_timestamp.saturating_add(1),
        )
    };

    let default_timestamp = 1_700_000_001u64.saturating_add(batch_number);
    let first_l2_block_timestamp = default_timestamp.max(min_timestamp);
    let l1_batch_number = L1BatchNumber::from(batch_number as u32);
    let l1_batch_env = L1BatchEnv {
        previous_batch_hash: None,
        number: l1_batch_number,
        timestamp: first_l2_block_timestamp,
        fee_account: Address::repeat_byte(1),
        enforced_base_fee: None,
        first_l2_block: L2BlockEnv {
            number: first_l2_block_number,
            timestamp: first_l2_block_timestamp,
            prev_block_hash,
            max_virtual_blocks_to_create: 100,
            interop_roots: vec![],
        },
        fee_input: BatchFeeInput::l1_pegged(50_000_000_000, 250_000_000),
        interop_fee: 0.into(),
        settlement_layer: SettlementLayer::for_tests(),
    };

    let system_env = SystemEnv {
        zk_porter_available: false,
        version: zksync_types::ProtocolVersionId::latest(),
        base_system_smart_contracts: base_contracts,
        bootloader_gas_limit: 2_000_000_000,
        execution_mode: TxExecutionMode::VerifyExecute,
        default_validation_computational_gas_limit: 2_000_000_000,
        chain_id,
    };

    (l1_batch_env, system_env)
}

fn seed_account_balances(storage: &mut InMemoryStorage, transactions: &[Transaction]) {
    let seeded_balance = zksync_types::u256_to_h256(U256::from(10u64.pow(18)));
    for tx in transactions {
        let sender = tx.initiator_account();
        let sender_key = storage_key_for_eth_balance(&sender);
        storage.set_value(sender_key, seeded_balance);

        if let Some(recipient) = tx.recipient_account() {
            let recipient_key = storage_key_for_eth_balance(&recipient);
            if storage.read_value(&recipient_key) == H256::zero() {
                storage.set_value(recipient_key, H256::zero());
            }
        }
    }
}

fn parse_batch_transactions(batch_data_hex: &str) -> Result<Vec<Value>> {
    let batch_bytes = decode_hex_field(batch_data_hex)?;
    Ok(serde_json::from_slice(&batch_bytes).context("parse sequencer batch transactions")?)
}

fn tx_from_normal_transaction(tx: &Value) -> Result<Transaction> {
    let normal = tx.get("Normal").unwrap_or(tx);
    let from = parse_address(&normal["from"])?;
    let to = parse_address(&normal["to"])?;
    let value = parse_u256(&normal["value"])?;
    let nonce = parse_nonce(&normal["nonce"])?;
    let gas_price = parse_u256(&normal["gas_price"])?;
    let gas_limit = parse_u256(&normal["gas_limit"])?;

    let fee = Fee {
        gas_limit,
        max_fee_per_gas: gas_price,
        max_priority_fee_per_gas: gas_price,
        gas_per_pubdata_limit: 50_000.into(),
    };

    let mut tx = L2Tx::new(
        Some(to),
        vec![],
        nonce,
        fee,
        from,
        value,
        vec![],
        Default::default(),
    );

    let raw_bytes = serde_json::to_vec(normal).context("serialize normal tx")?;
    let hash = H256::from_slice(&keccak256(&raw_bytes));
    tx.set_input(raw_bytes, hash);
    Ok(Transaction::from(tx))
}

fn tx_from_forced_transaction(tx: &Value) -> Result<Transaction> {
    let forced = tx.get("Forced").unwrap_or(tx);
    let from = parse_address(&forced["from"])?;
    let to = parse_address(&forced["to"])?;
    let value = parse_u256(&forced["value"])?;
    let nonce = parse_nonce(&forced["nonce"])?;
    let gas_limit = parse_u256(&forced["gas_limit"])?;

    let fee = Fee {
        gas_limit,
        max_fee_per_gas: 0.into(),
        max_priority_fee_per_gas: 0.into(),
        gas_per_pubdata_limit: 50_000.into(),
    };

    let mut tx = L2Tx::new(
        Some(to),
        vec![],
        nonce,
        fee,
        from,
        value,
        vec![],
        Default::default(),
    );

    let raw_bytes = serde_json::to_vec(forced).context("serialize forced tx")?;
    let hash = H256::from_slice(&keccak256(&raw_bytes));
    tx.set_input(raw_bytes, hash);
    Ok(Transaction::from(tx))
}

fn convert_sequencer_transactions(transactions: &[Value]) -> Result<Vec<Transaction>> {
    let mut converted = Vec::with_capacity(transactions.len());
    for tx in transactions {
        if tx.get("Normal").is_some() {
            converted.push(tx_from_normal_transaction(tx)?);
        } else if tx.get("Forced").is_some() {
            converted.push(tx_from_forced_transaction(tx)?);
        } else {
            anyhow::bail!("unsupported sequencer transaction shape");
        }
    }
    Ok(converted)
}

struct PreparedBatchInput {
    input: BatchInput,
    storage: InMemoryStorage,
}

fn build_batch_input(batch: &SequencerLegacyBatchOutput, transactions: Vec<Transaction>) -> Result<PreparedBatchInput> {
    let base_contracts = create_base_contracts()?;
    let chain_id = L2ChainId::from(DEFAULT_ERA_CHAIN_ID);
    let mut storage = setup_test_storage(chain_id, base_contracts.hashes());
    seed_account_balances(&mut storage, &transactions);

    let db_path = std::env::var("EXECUTOR_DB_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("executor.db"));

    let (l1_batch_env, system_env) = build_envs(&mut storage, base_contracts, chain_id, batch.batch_number);

    Ok(PreparedBatchInput {
        input: BatchInput {
        l1_batch_env,
        system_env,
        transactions,
        db_path,
        },
        storage,
    })
}

async fn load_batch(path: &Path) -> Result<SequencerLegacyBatchOutput> {
    let content = tokio::fs::read_to_string(path)
        .await
        .with_context(|| format!("read batch file {path:?}"))?;
    Ok(serde_json::from_str(&content).context("parse sequencer batch file")?)
}

async fn write_executor_outputs(
    output_path: &Path,
    prover_path: &Path,
    batch: &SequencerLegacyBatchOutput,
    output: crate::BatchOutput,
) -> Result<()> {
    let batch_data_bytes = decode_hex_field(&batch.batch_data)?;
    let legacy_output = ExecutorLegacyBatchOutput {
        batch_number: batch.batch_number,
        pre_state_root: batch.pre_state_root.clone(),
        root_hash: h256_to_hex(output.root_hash),
        post_state_root: h256_to_hex(output.root_hash),
        pubdata: encode_hex_field(&output.pubdata),
        batch_data: encode_hex_field(&batch_data_bytes),
        da_commitment: format!("0x{}", hex::encode(keccak256(&batch_data_bytes))),
        proof: "0x".to_string(),
        batch_id: batch.batch_id.clone(),
        experiment_id: batch.experiment_id.clone(),
    };

    let prover_output = ExecutorProverOutput {
        batch_id: batch.batch_id.clone(),
        root_hash: h256_to_hex(output.root_hash),
        pubdata: encode_hex_field(&output.pubdata),
        witness: output.witness,
        storage_log_count: output
            .finished_batch
            .block_tip_execution_result
            .logs
            .storage_logs
            .len(),
        finished_batch_has_pubdata: output.finished_batch.pubdata_input.is_some(),
    };

    tokio::fs::write(output_path, serde_json::to_vec_pretty(&legacy_output)?).await?;
    tokio::fs::write(prover_path, serde_json::to_vec_pretty(&prover_output)?).await?;
    Ok(())
}

pub async fn run_from_env() -> Result<()> {
    let input_path = std::env::var("SEQUENCER_BATCH_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("batch_output.json"));
    let output_path = std::env::var("EXECUTOR_BATCH_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("batch_output.json"));
    let prover_path = std::env::var("EXECUTOR_PROVER_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("executor_prover_output.json"));

    let batch = load_batch(&input_path).await?;
    let sequencer_transactions = parse_batch_transactions(&batch.batch_data)?;
    let transactions = convert_sequencer_transactions(&sequencer_transactions)?;
    let prepared = build_batch_input(&batch, transactions)?;
    let mut processor = BatchProcessor::new_with_semantics(
        prepared.storage,
        &prepared.input.db_path,
        ExecutionSemantics::TolerantResearch,
    )
    .context("create batch processor")?;

    let result = processor.process_batch(prepared.input).await.context("process batch")?;
    write_executor_outputs(&output_path, &prover_path, &batch, result).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn converts_simplified_transactions_into_executor_transactions() {
        let raw_batch = json!([
            {
                "Normal": {
                    "from": "0x7e5F4552091A69125d5DfCb7b8C2659029395Bdf",
                    "to": "0xfEfEFefEFefEFefEFefEFefEFefEFefEFefEFefe",
                    "value": "100",
                    "nonce": 1,
                    "gas_price": "1000000000",
                    "gas_limit": 8000000,
                    "signature": "0x",
                    "timestamp": 1700000001u64
                }
            },
            {
                "Forced": {
                    "tx_hash": "0x0000000000000000000000000000000000000000000000000000000000000001",
                    "from": "0x0000000000000000000000000000000000000001",
                    "to": "0xfEfEFefEFefEFefEFefEFefEFefEFefEFefEFefe",
                    "value": "25",
                    "nonce": 2,
                    "gas_limit": 5000000,
                    "l1_tx_hash": "0x0000000000000000000000000000000000000000000000000000000000000002",
                    "l1_block_number": 100,
                    "event_type": "Deposit",
                    "timestamp": 1700000002u64
                }
            }
        ]);

        let parsed = parse_batch_transactions(&format!("0x{}", hex::encode(serde_json::to_vec(&raw_batch).unwrap()))).unwrap();
        let converted = convert_sequencer_transactions(&parsed).unwrap();

        assert_eq!(converted.len(), 2);
        assert_eq!(converted[0].initiator_account(), Address::from_str("0x7e5F4552091A69125d5DfCb7b8C2659029395Bdf").unwrap());
        assert_eq!(converted[0].recipient_account().unwrap(), Address::from_str("0xfEfEFefEFefEFefEFefEFefEFefEFefEFefEFefe").unwrap());
        assert_eq!(converted[0].execute.value, U256::from(100));
        assert_eq!(converted[0].nonce(), Some(Nonce(1)));
        assert_eq!(converted[1].execute.value, U256::from(25));
        assert_eq!(converted[1].nonce(), Some(Nonce(2)));
    }
}