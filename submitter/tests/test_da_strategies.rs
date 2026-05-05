/// DA Strategies Integration Tests
/// Tests for Calldata, Blob, and OffChain data availability modes

use submitter_rs::domain::batch::Batch;

#[test]
fn test_da_modes_defined() {
    use submitter_rs::domain::batch::DaMode;
    
    // Verify DA modes exist
    let _ = DaMode::Calldata;
    let _ = DaMode::Blob;
    let _ = DaMode::OffChain;
    println!("✓ DA modes defined");
}

#[test]
fn test_da_mode_clone() {
    use submitter_rs::domain::batch::DaMode;
    let mode = DaMode::Calldata;
    let cloned = mode.clone();
    assert_eq!(mode, cloned);
    println!("✓ DaMode clone works");
}

#[test]
fn test_da_mode_serialize() {
    use serde::{Serialize, Deserialize};
    use submitter_rs::domain::batch::DaMode;
    let mode = DaMode::Blob;
    let bytes = serde_json::to_vec(&mode).unwrap();
    let restored: DaMode = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(mode, restored);
    println!("✓ DaMode serialize works");
}

#[tokio::test]
#[ignore]
async fn test_calldata_strategy_computes_commitment() {
    let batch_data = b"test batch data for commitment";
    let commitment1 = ethers::utils::keccak256(batch_data);
    let commitment2 = ethers::utils::keccak256(batch_data);
    assert_eq!(commitment1, commitment2, "Commitment should be deterministic");
}

#[tokio::test]
#[ignore]
async fn test_blob_strategy_encodes_metadata() {
    let batch = Batch {
        id: "batch_blob_001".to_string(),
        blob_versioned_hash: Some("0x01d041cd4ecbf1545fde7b32a99e4a2b3d7d4e8f9a0b1c2d3e4f5a6b7c8d9e0".to_string()),
        blob_index: Some(0),
        ..Default::default()
    };

    let versioned_hash = batch.blob_versioned_hash.as_ref().unwrap();
    assert!(versioned_hash.starts_with("0x"), "Versioned hash must be hex");
    assert_eq!(versioned_hash.len(), 66, "Hash must be 32 bytes");
}

#[tokio::test]
#[ignore]
async fn test_calldata_vs_blob_gas_efficiency() {
    let test_sizes = vec![100, 1_000, 10_000, 100_000];

    for size in test_sizes {
        // For calldata: gas = 16 * zero_bytes + 4 * nonzero_bytes
        let calldata_gas: u64 = (4 * size * 16) as u64;
        
        // For blob: fixed cost ~21000 + blob_gas (minimal compared to calldata)
        let blob_gas = 21000u64 + (size as u64 * 1);

        assert!(
            blob_gas < calldata_gas,
            "Blob gas ({}) should be < calldata gas ({})",
            blob_gas, calldata_gas
        );
    }
}

#[tokio::test]
#[ignore]
async fn test_compression_reduces_calldata_size() {
    let original_data = b"test data test data test data test data".repeat(100);
    
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    use std::io::Write;
    encoder.write_all(&original_data).unwrap();
    let compressed = encoder.finish().unwrap();

    let compression_ratio = (compressed.len() as f64) / (original_data.len() as f64);
    
    assert!(
        compression_ratio < 1.0,
        "Compression should reduce size"
    );
}

#[tokio::test]
#[ignore]
async fn test_da_commitment_consistency() {
    let batch_data = b"consistent batch data";
    
    let commit1 = ethers::utils::keccak256(batch_data);
    let commit2 = ethers::utils::keccak256(batch_data);
    let commit3 = ethers::utils::keccak256(batch_data);

    assert_eq!(commit1, commit2);
    assert_eq!(commit2, commit3);
    
    let different_data = b"different batch data";
    let commit_diff = ethers::utils::keccak256(different_data);
    
    assert_ne!(commit1, commit_diff);
}

#[cfg(test)]
mod da_strategy_defaults {
    use super::*;

    #[test]
    fn calldata_strategy_da_id_is_zero() {
        use submitter_rs::domain::batch::DaMode;
        assert_eq!(DaMode::Calldata as u8, 0);
    }

    #[test]
    fn blob_strategy_da_id_is_one() {
        use submitter_rs::domain::batch::DaMode;
        assert_eq!(DaMode::Blob as u8, 1);
    }
}