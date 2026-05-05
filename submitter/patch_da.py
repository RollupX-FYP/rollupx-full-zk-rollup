import sys

def patch_file(path, old, new):
    with open(path, 'r', encoding='utf-8') as f:
        data = f.read()
    if old in data:
        data = data.replace(old, new)
        with open(path, 'w', encoding='utf-8') as f:
            f.write(data)
        print(f'Patched {path}')
    else:
        print(f'Failed to patch {path}')

old_call = """        Ok(SubmissionResult {
            tx_hash: format!("{:?}", tx_hash),
            block_number: receipt.block_number.unwrap_or_default().as_u64(),
            latency_ms: latency,
            compression_ratio: None,
            compressed_bytes: Some(compressed_len),
            gas_saved: None,
            gas_used,
        })"""
new_call = """        Ok(SubmissionResult {
            tx_hash: format!("{:?}", tx_hash),
            block_number: receipt.block_number.unwrap_or_default().as_u64(),
            latency_ms: latency,
            compression_ratio: None,
            compressed_bytes: Some(compressed_len),
            gas_saved: None,
            gas_used,
            blob_gas_used: None,
            blob_base_fee_wei: None,
            da_mode_is_simulated: false,
        })"""
patch_file('src/infrastructure/da_calldata.rs', old_call, new_call)

old_blob = """        Ok(SubmissionResult {
            tx_hash: format!("{:?}", tx_hash),
            block_number: receipt.block_number.unwrap_or_default().as_u64(),
            latency_ms: latency,
            compression_ratio: None,
            compressed_bytes: Some(compressed_len),
            gas_saved: None,
            gas_used,
        })"""
new_blob = """        Ok(SubmissionResult {
            tx_hash: format!("{:?}", tx_hash),
            block_number: receipt.block_number.unwrap_or_default().as_u64(),
            latency_ms: latency,
            compression_ratio: None,
            compressed_bytes: Some(compressed_len),
            gas_saved: None,
            gas_used,
            blob_gas_used: receipt.blob_gas_used.map(|g| g.as_u64()),
            blob_base_fee_wei: receipt.blob_gas_price.map(|p| p.as_u64()),
            da_mode_is_simulated: false,
        })"""
patch_file('src/infrastructure/da_blob.rs', old_blob, new_blob)

old_off = """        Ok(SubmissionResult {
            tx_hash: "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            block_number: 0,
            latency_ms: 0,
            compression_ratio: None,
            compressed_bytes: Some(batch.data_file.len()), // rough approx
            gas_saved: None,
            gas_used: Some(0),
        })"""
new_off = """        Ok(SubmissionResult {
            tx_hash: "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            block_number: 0,
            latency_ms: 0,
            compression_ratio: None,
            compressed_bytes: Some(batch.data_file.len()), // rough approx
            gas_saved: None,
            gas_used: Some(0),
            blob_gas_used: None,
            blob_base_fee_wei: None,
            da_mode_is_simulated: true,
        })"""
patch_file('src/infrastructure/da_offchain.rs', old_off, new_off)
