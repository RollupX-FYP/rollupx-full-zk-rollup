// Comprehensive test module for executor trace persistence
// Tests trace I/O, SHA256 hashing, lifecycle tracking

#[cfg(test)]
mod executor_trace_tests {
    use crate::trace::{persist_trace, verify_trace_hash, append_lifecycle, TraceLifecycleStatus};
    use crate::types::{ExecutionTraceV1, TracePublicInputs, ProverContext};
    use std::fs;
    use tempfile::TempDir;

    // ============ Fixtures & Helpers ============

    fn dummy_trace(batch_id: &str) -> ExecutionTraceV1 {
        ExecutionTraceV1 {
            trace_id: format!("trace_{}", batch_id),
            schema_version: 1,
            batch_id: batch_id.to_string(),
            created_at: 1234567890,
            executor_build_id: "dev".to_string(),
            public_inputs: TracePublicInputs {
                initial_root: [1u8; 32],
                final_root: [2u8; 32],
                tx_commitment: [3u8; 32],
                state_diff_commitment: [4u8; 32],
            },
            executed_transactions: vec![],
            tx_outcomes: vec![],
            state_diffs: vec![],
            prover_context: ProverContext {
                guest_method_id: "method_1".to_string(),
                expected_journal_hash: [5u8; 32],
                backend_config_fingerprint: [6u8; 32],
            },
        }
    }

    // ============ Trace Persistence Tests ============

    #[test]
    fn test_persist_trace_creates_files() {
        let temp = TempDir::new().unwrap();
        let trace = dummy_trace("b1");

        let result = persist_trace(temp.path(), &trace);
        assert!(result.is_ok());

        let meta = result.unwrap();
        assert!(meta.trace_path.exists());
        assert!(!meta.sha256_hex.is_empty());
        assert_eq!(meta.sha256_hex.len(), 64); // SHA256 hex is 64 chars
    }

    #[test]
    fn test_persisted_trace_readable() {
        let temp = TempDir::new().unwrap();
        let trace = dummy_trace("b2");

        let result = persist_trace(temp.path(), &trace);
        assert!(result.is_ok());

        let meta = result.unwrap();
        let contents = fs::read_to_string(&meta.trace_path).unwrap();
        assert!(contents.contains(&trace.batch_id));
        assert!(contents.contains(&trace.trace_id));
    }

    #[test]
    fn test_persisted_trace_deserializable() {
        let temp = TempDir::new().unwrap();
        let trace = dummy_trace("b3");

        let result = persist_trace(temp.path(), &trace);
        assert!(result.is_ok());

        let meta = result.unwrap();
        let json = fs::read_to_string(&meta.trace_path).unwrap();
        let deserialized: ExecutionTraceV1 = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.batch_id, trace.batch_id);
        assert_eq!(deserialized.trace_id, trace.trace_id);
        assert_eq!(deserialized.schema_version, trace.schema_version);
    }

    #[test]
    fn test_trace_sha256_computed_correctly() {
        let temp = TempDir::new().unwrap();
        let trace = dummy_trace("b4");

        let result = persist_trace(temp.path(), &trace);
        assert!(result.is_ok());

        let meta = result.unwrap();
        let contents = fs::read(&meta.trace_path).unwrap();
        
        // Verify SHA256 is computed correctly
        let computed_hash = format!("{:x}", sha2::Sha256::digest(&contents));
        assert_eq!(computed_hash, meta.sha256_hex);
    }

    #[test]
    fn test_trace_hash_verification_passes() {
        let temp = TempDir::new().unwrap();
        let trace = dummy_trace("b5");

        let result = persist_trace(temp.path(), &trace);
        assert!(result.is_ok());

        let meta = result.unwrap();
        let verify_result = verify_trace_hash(&meta.trace_path, &meta.sha256_hex);
        assert!(verify_result.is_ok());
    }

    #[test]
    fn test_trace_hash_verification_fails_on_mismatch() {
        let temp = TempDir::new().unwrap();
        let trace = dummy_trace("b6");

        let result = persist_trace(temp.path(), &trace);
        assert!(result.is_ok());

        let meta = result.unwrap();
        let wrong_hash = "0000000000000000000000000000000000000000000000000000000000000000";
        let verify_result = verify_trace_hash(&meta.trace_path, wrong_hash);
        assert!(verify_result.is_err());
    }

    #[test]
    fn test_trace_verification_detects_file_corruption() {
        let temp = TempDir::new().unwrap();
        let trace = dummy_trace("b7");

        let result = persist_trace(temp.path(), &trace);
        assert!(result.is_ok());

        let meta = result.unwrap();

        // Corrupt the file
        fs::write(&meta.trace_path, b"corrupted data").unwrap();

        let verify_result = verify_trace_hash(&meta.trace_path, &meta.sha256_hex);
        assert!(verify_result.is_err());
    }

    #[test]
    fn test_multiple_traces_in_same_directory() {
        let temp = TempDir::new().unwrap();

        for i in 0..5 {
            let trace = dummy_trace(&format!("b{}", i));
            let result = persist_trace(temp.path(), &trace);
            assert!(result.is_ok());
        }

        // Verify batch directories created
        for i in 0..5 {
            let batch_dir = temp.path().join(format!("b{}", i));
            assert!(batch_dir.exists());
        }
    }

    #[test]
    fn test_trace_in_batch_subdirectory() {
        let temp = TempDir::new().unwrap();
        let trace = dummy_trace("batch_001");

        let result = persist_trace(temp.path(), &trace);
        assert!(result.is_ok());

        let meta = result.unwrap();
        let parent = meta.trace_path.parent().unwrap();
        assert_eq!(parent.file_name().unwrap(), "batch_001");
    }

    // ============ Trace Lifecycle Tests ============

    #[test]
    fn test_append_lifecycle_creates_index() {
        let temp = TempDir::new().unwrap();
        let trace = dummy_trace("b8");

        let result = append_lifecycle(
            temp.path(),
            &trace,
            TraceLifecycleStatus::Generated,
            None,
            None,
        );
        assert!(result.is_ok());

        let index_path = temp.path().join("index.jsonl");
        assert!(index_path.exists());
    }

    #[test]
    fn test_append_lifecycle_writes_entry() {
        let temp = TempDir::new().unwrap();
        let trace = dummy_trace("b9");

        let result = append_lifecycle(
            temp.path(),
            &trace,
            TraceLifecycleStatus::Generated,
            None,
            None,
        );
        assert!(result.is_ok());

        let index_path = temp.path().join("index.jsonl");
        let contents = fs::read_to_string(&index_path).unwrap();
        assert!(contents.contains(&trace.batch_id));
        assert!(contents.contains(&trace.trace_id));
    }

    #[test]
    fn test_append_lifecycle_multiple_statuses() {
        let temp = TempDir::new().unwrap();
        let trace = dummy_trace("b10");

        append_lifecycle(
            temp.path(),
            &trace,
            TraceLifecycleStatus::Generated,
            None,
            None,
        ).unwrap();

        append_lifecycle(
            temp.path(),
            &trace,
            TraceLifecycleStatus::Persisted,
            None,
            Some("abc123"),
        ).unwrap();

        append_lifecycle(
            temp.path(),
            &trace,
            TraceLifecycleStatus::Proved,
            None,
            None,
        ).unwrap();

        let index_path = temp.path().join("index.jsonl");
        let contents = fs::read_to_string(&index_path).unwrap();
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn test_lifecycle_entry_parseable() {
        let temp = TempDir::new().unwrap();
        let trace = dummy_trace("b11");

        append_lifecycle(
            temp.path(),
            &trace,
            TraceLifecycleStatus::Generated,
            None,
            None,
        ).unwrap();

        let index_path = temp.path().join("index.jsonl");
        let contents = fs::read_to_string(&index_path).unwrap();
        let line = contents.lines().next().unwrap();
        
        let entry: serde_json::Value = serde_json::from_str(line).unwrap();
        assert_eq!(entry["batch_id"].as_str().unwrap(), trace.batch_id);
        assert_eq!(entry["trace_id"].as_str().unwrap(), trace.trace_id);
    }

    #[test]
    fn test_lifecycle_with_path_and_sha256() {
        let temp = TempDir::new().unwrap();
        let trace = dummy_trace("b12");

        let trace_path = temp.path().join("test_trace.json");
        fs::write(&trace_path, b"test content").unwrap();

        let result = append_lifecycle(
            temp.path(),
            &trace,
            TraceLifecycleStatus::Persisted,
            Some(&trace_path),
            Some("abc123def456"),
        );
        assert!(result.is_ok());

        let index_path = temp.path().join("index.jsonl");
        let contents = fs::read_to_string(&index_path).unwrap();
        assert!(contents.contains("abc123def456"));
    }

    #[test]
    fn test_lifecycle_idempotent_creation() {
        let temp = TempDir::new().unwrap();
        let trace = dummy_trace("b13");

        append_lifecycle(
            temp.path(),
            &trace,
            TraceLifecycleStatus::Generated,
            None,
            None,
        ).unwrap();

        // Should not fail if index already exists
        let result = append_lifecycle(
            temp.path(),
            &trace,
            TraceLifecycleStatus::Persisted,
            None,
            None,
        );
        assert!(result.is_ok());
    }

    // ============ Integration Tests ============

    #[test]
    fn test_full_trace_lifecycle() {
        let temp = TempDir::new().unwrap();
        let trace = dummy_trace("b14");

        // Step 1: Generate
        append_lifecycle(
            temp.path(),
            &trace,
            TraceLifecycleStatus::Generated,
            None,
            None,
        ).unwrap();

        // Step 2: Persist
        let meta = persist_trace(temp.path(), &trace).unwrap();
        append_lifecycle(
            temp.path(),
            &trace,
            TraceLifecycleStatus::Persisted,
            Some(&meta.trace_path),
            Some(&meta.sha256_hex),
        ).unwrap();

        // Step 3: Prove
        append_lifecycle(
            temp.path(),
            &trace,
            TraceLifecycleStatus::Proved,
            None,
            None,
        ).unwrap();

        // Step 4: Publish
        append_lifecycle(
            temp.path(),
            &trace,
            TraceLifecycleStatus::Published,
            None,
            None,
        ).unwrap();

        // Verify index has all 4 entries
        let index_path = temp.path().join("index.jsonl");
        let contents = fs::read_to_string(&index_path).unwrap();
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 4);
    }

    #[test]
    fn test_trace_determinism_persists_identically() {
        let temp = TempDir::new().unwrap();
        let trace = dummy_trace("b15");

        let meta1 = persist_trace(temp.path(), &trace).unwrap();

        // Remove the trace file and persist again
        fs::remove_file(&meta1.trace_path).unwrap();

        let meta2 = persist_trace(temp.path(), &trace).unwrap();

        assert_eq!(meta1.sha256_hex, meta2.sha256_hex);
    }
}
