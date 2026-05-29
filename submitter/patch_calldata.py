import sys

with open('src/infrastructure/da_calldata.rs', 'r', encoding='utf-8') as f:
    data = f.read()

# 1. Extend SubmissionResult
old_res = """        Ok(SubmissionResult {
            tx_hash: format!("{:?}", tx_hash),
            block_number: receipt.block_number.unwrap_or_default().as_u64(),
            latency_ms: start_time.elapsed().as_millis() as u64,
            compression_ratio: None,
            compressed_bytes: Some(compressed_len),
            gas_saved: None,
            gas_used,
        })"""

new_res = """        Ok(SubmissionResult {
            tx_hash: format!("{:?}", tx_hash),
            block_number: receipt.block_number.unwrap_or_default().as_u64(),
            latency_ms: start_time.elapsed().as_millis() as u64,
            compression_ratio: None,
            compressed_bytes: Some(compressed_len),
            gas_saved: None,
            gas_used,
            blob_gas_used: None,
            blob_base_fee_wei: None,
            da_mode_is_simulated: false,
        })"""

if old_res in data:
    data = data.replace(old_res, new_res)
    print("Extended SubmissionResult")

# 2. Fix Batch in test
old_batch = """        let batch = Batch {
            id: crate::domain::batch::BatchId::new(),
            data_file: "test_data_calldata.txt".to_string(),
            new_root: format!("{:?}", H256::zero()),
            status: crate::domain::batch::BatchStatus::Proving,
            da_mode: "calldata".to_string(),
            proof: None,
            tx_hash: None,
            attempts: 0,
            tx_count: 0,
        };"""

new_batch = """        let batch = Batch {
            id: crate::domain::batch::BatchId::new(),
            data_file: "test_data_calldata.txt".to_string(),
            new_root: format!("{:?}", H256::zero()),
            status: crate::domain::batch::BatchStatus::Proving,
            da_mode: "calldata".to_string(),
            proof: None,
            tx_hash: None,
            attempts: 0,
            tx_count: 0,
            ..Default::default()
        };"""

if old_batch in data:
    data = data.replace(old_batch, new_batch)
    print("Fixed Batch in test")
else:
    # Try another variation of Batch initialization if the first one failed
    print("Could not find exact Batch initialization, skip for now")

# 3. Add timeout and interval to test
old_test_start = """    #[tokio::test]
    async fn test_submit_calldata() {
        let mock = MockClient::new();
        let provider = Provider::new(mock.clone());"""

new_test_start = """    #[tokio::test]
    async fn test_submit_calldata() {
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        let mock = MockClient::new();
        let provider = Provider::new(mock.clone()).interval(std::time::Duration::from_millis(10));"""

if old_test_start in data:
    data = data.replace(old_test_start, new_test_start)
    print("Added timeout and interval")

# 4. Fix mock queue and add closing brace
old_test_end = """        mock.push(U256::from(100_000));
        mock.push(H256::random());
        let mut receipt = TransactionReceipt::default();
        receipt.status = Some(U64::from(1));
        receipt.block_number = Some(U64::from(1));
        receipt.gas_used = Some(U256::from(21000));
        mock.push(Some(receipt));

        let proof_hex = format!("0x{}", hex::encode([0u8; 128])); // Random bytes
        let res = strategy.submit(&batch, &proof_hex, 0).await;

        let _ = std::fs::remove_file("test_data_calldata.txt");
        if let Err(e) = &res {
            println!("Submit error: {:?}", e);
        }
        assert!(res.is_ok(), "submit failed");
    }"""

new_test_end = """        mock.push(U256::from(100_000));
        mock.push(H256::random());
        // satisfy possible getTransactionByHash
        let mut tx = Transaction::default();
        tx.hash = H256::random();
        mock.push(Some(tx));
        
        let mut receipt = TransactionReceipt::default();
        receipt.status = Some(U64::from(1));
        receipt.block_number = Some(U64::from(1));
        receipt.gas_used = Some(U256::from(21000));
        mock.push(Some(receipt));

        let proof_hex = format!("0x{}", hex::encode([0u8; 128])); // Random bytes
        let res = strategy.submit(&batch, &proof_hex, 0).await;
        assert!(res.is_ok());
        }).await.expect("Test timed out");

        let _ = std::fs::remove_file("test_data_calldata.txt");
    }"""

if old_test_end in data:
    data = data.replace(old_test_end, new_test_end)
    print("Fixed mock queue and closing brace")

with open('src/infrastructure/da_calldata.rs', 'w', encoding='utf-8') as f:
    f.write(data)
