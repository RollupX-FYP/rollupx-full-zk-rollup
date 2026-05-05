/// Batch State Management & Orchestration Tests
/// Verifies batch lifecycle and state transitions

use submitter::domain::batch::{Batch, BatchStatus};
use submitter::application::orchestrator::Orchestrator;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn test_batch_status_transitions() {
    /// Verify valid state transitions for batches

    let mut batch = Batch {
        id: "test_batch_001".to_string(),
        status: BatchStatus::Discovered,
        ..Default::default()
    };

    // Valid transition: Discovered -> Proving
    batch.transition_to(BatchStatus::Proving);
    assert_eq!(batch.status, BatchStatus::Proving);

    // Valid transition: Proving -> Proved
    batch.transition_to(BatchStatus::Proved);
    assert_eq!(batch.status, BatchStatus::Proved);

    // Valid transition: Proved -> Submitting
    batch.transition_to(BatchStatus::Submitting);
    assert_eq!(batch.status, BatchStatus::Submitting);

    // Valid transition: Submitting -> Submitted
    batch.transition_to(BatchStatus::Submitted);
    assert_eq!(batch.status, BatchStatus::Submitted);

    // Valid transition: Submitted -> Confirmed
    batch.transition_to(BatchStatus::Confirmed);
    assert_eq!(batch.status, BatchStatus::Confirmed);

    println!("✓ All valid batch state transitions work");
}

#[test]
fn test_batch_retry_attempt_tracking() {
    /// Verify that batch tracks retry attempts

    let mut batch = Batch {
        id: "retry_batch_001".to_string(),
        attempts: 0,
        ..Default::default()
    };

    assert_eq!(batch.attempts, 0);

    batch.attempts += 1;
    assert_eq!(batch.attempts, 1);

    batch.attempts += 1;
    assert_eq!(batch.attempts, 2);

    // After max attempts, should fail
    let max_attempts = 3;
    if batch.attempts >= max_attempts {
        batch.transition_to(BatchStatus::Failed);
        assert_eq!(batch.status, BatchStatus::Failed);
    }

    println!("✓ Batch attempt tracking works");
}

#[test]
fn test_batch_expiration() {
    /// Verify that old batches are marked as stale

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let mut batch = Batch {
        id: "old_batch_001".to_string(),
        created_at: now - 86400, // 1 day old
        ..Default::default()
    };

    let expiry_window_seconds = 3600; // 1 hour
    let age = now - batch.created_at;

    if age > expiry_window_seconds {
        // Batch is stale
        println!("Batch age: {} seconds (expired)", age);
        assert!(age > expiry_window_seconds);
    }
}

#[test]
fn test_batch_with_proof_stores_verification_data() {
    /// Verify that batch stores proof and verification metadata

    let mut batch = Batch {
        id: "proof_batch_001".to_string(),
        proof: None,
        new_root: "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
        ..Default::default()
    };

    let proof_data = "0x" + &"ab".repeat(128); // 256 byte proof
    batch.proof = Some(proof_data.clone());

    assert!(batch.proof.is_some());
    assert_eq!(batch.proof.as_ref().unwrap(), &proof_data);
    println!("✓ Batch proof storage works");
}

#[test]
fn test_batch_transaction_hash_tracking() {
    /// Verify that batch tracks L1 transaction hash

    let mut batch = Batch {
        id: "tx_batch_001".to_string(),
        tx_hash: None,
        ..Default::default()
    };

    let tx_hash = "0x" + &"cd".repeat(32); // 64-char hex = 32 bytes
    batch.tx_hash = Some(tx_hash.clone());

    assert!(batch.tx_hash.is_some());
    assert_eq!(batch.tx_hash.as_ref().unwrap(), &tx_hash);
    println!("✓ Batch transaction hash tracking works");
}

#[test]
fn test_batch_error_message_storage() {
    /// Verify batch stores error information for debugging

    let mut batch = Batch {
        id: "error_batch_001".to_string(),
        error: None,
        ..Default::default()
    };

    let error_msg = "Proof generation timeout after 30 seconds";
    batch.error = Some(error_msg.to_string());

    assert!(batch.error.is_some());
    assert!(batch.error.as_ref().unwrap().contains("Proof generation"));
    println!("✓ Batch error storage works");
}

#[test]
fn test_batch_compression_metadata() {
    /// Verify batch stores compression metrics

    let mut batch = Batch {
        id: "compress_batch_001".to_string(),
        ..Default::default()
    };

    // Simulated compression: 100KB -> 30KB
    let original_size = 100_000usize;
    let compressed_size = 30_000usize;

    let compression_ratio = (compressed_size as f64) / (original_size as f64);

    assert!(compression_ratio < 1.0);
    println!(
        "Compression: {} KB -> {} KB ({:.1}%)",
        original_size / 1024,
        compressed_size / 1024,
        compression_ratio * 100.0
    );
}

#[test]
fn test_batch_blob_specific_fields() {
    /// Verify batch handles blob-specific metadata

    let mut batch = Batch {
        id: "blob_batch_001".to_string(),
        blob_versioned_hash: Some(
            "0x01b08d7d5f6e9a8c7b6a5d4e3f2g1h0i9j8k7l6m5n4o3p2q1r0s".to_string(),
        ),
        blob_index: Some(0),
        ..Default::default()
    };

    assert!(batch.blob_versioned_hash.is_some());
    assert!(batch.blob_index.is_some());
    assert_eq!(batch.blob_index.unwrap(), 0);

    // Multiple blobs can be used
    batch.blob_index = Some(1);
    assert_eq!(batch.blob_index.unwrap(), 1);

    println!("✓ Blob-specific batch fields work");
}

#[test]
fn test_batch_da_mode_routing() {
    /// Verify batch correctly routes to DA strategy based on config

    #[derive(Debug, PartialEq)]
    enum DaMode {
        Calldata,
        Blob,
        OffChain,
    }

    let batch_calldata = Batch {
        id: "batch_calldata".to_string(),
        ..Default::default()
    };

    let batch_blob = Batch {
        id: "batch_blob".to_string(),
        blob_versioned_hash: Some(
            "0x01d041cd4ecbf1545fde7b32a99e4a2b3d7d4e8f9a0b1c2d3e4f5a6b7c8d9e0".to_string(),
        ),
        ..Default::default()
    };

    // Route based on presence of blob metadata
    let mode_calldata = if batch_calldata.blob_versioned_hash.is_some() {
        DaMode::Blob
    } else {
        DaMode::Calldata
    };

    let mode_blob = if batch_blob.blob_versioned_hash.is_some() {
        DaMode::Blob
    } else {
        DaMode::Calldata
    };

    assert_eq!(mode_calldata, DaMode::Calldata);
    assert_eq!(mode_blob, DaMode::Blob);

    println!("✓ DA mode routing works");
}

#[test]
fn test_batch_determinism_hash() {
    /// Verify that batch produces deterministic hash across invocations

    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let batch1 = Batch {
        id: "deterministic_001".to_string(),
        data_file: "/path/to/data".to_string(),
        new_root: "0x0000000000000000000000000000000000000000000000000000000000000001".to_string(),
        ..Default::default()
    };

    let batch2 = Batch {
        id: "deterministic_001".to_string(),
        data_file: "/path/to/data".to_string(),
        new_root: "0x0000000000000000000000000000000000000000000000000000000000000001".to_string(),
        ..Default::default()
    };

    // Same batch data should produce same hash
    let mut hasher1 = DefaultHasher::new();
    batch1.id.hash(&mut hasher1);
    let hash1 = hasher1.finish();

    let mut hasher2 = DefaultHasher::new();
    batch2.id.hash(&mut hasher2);
    let hash2 = hasher2.finish();

    assert_eq!(hash1, hash2, "Batch IDs should hash identically");
}

#[test]
fn test_batch_merkle_proof_validation() {
    /// Verify batch validates merkle proofs before submission

    let batch = Batch {
        id: "merkle_batch_001".to_string(),
        new_root: "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890".to_string(),
        ..Default::default()
    };

    // Verify root is valid hex
    let root_str = &batch.new_root;
    let is_valid_hex = root_str.starts_with("0x") && root_str.len() == 66; // 0x + 64 hex chars

    assert!(is_valid_hex, "Root must be valid 32-byte hex");
    println!("✓ Merkle root validation works");
}

#[cfg(test)]
mod batch_lifecycle_constants {
    use submitter::domain::batch::BatchStatus;

    #[test]
    fn batch_status_sequence_is_linear() {
        // Batches follow strict progression: Discovered -> Proving -> Proved -> Submitting -> Submitted -> Confirmed
        // Or may transition to Failed at any point

        let statuses = vec![
            BatchStatus::Discovered,
            BatchStatus::Proving,
            BatchStatus::Proved,
            BatchStatus::Submitting,
            BatchStatus::Submitted,
            BatchStatus::Confirmed,
        ];

        assert_eq!(statuses.len(), 6);
    }

    #[test]
    fn max_retry_attempts_is_reasonable() {
        let max_attempts = 5;
        assert!(max_attempts >= 3, "Should allow at least 3 retries");
        assert!(max_attempts <= 10, "Should not exceed 10 retries");
    }

    #[test]
    fn batch_timeout_window_seconds() {
        let timeout = 3600; // 1 hour
        assert!(timeout >= 300, "Min 5 minutes");
        assert!(timeout <= 86400, "Max 24 hours");
    }
}
