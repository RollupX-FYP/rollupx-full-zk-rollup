use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::types::{ExecutionTraceV1, Hash};

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceLifecycleStatus {
    Generated,
    Persisted,
    Proved,
    Published,
}

#[derive(Debug, Clone, Serialize)]
pub struct TraceIndexEntry {
    pub trace_id: String,
    pub batch_id: String,
    pub schema_version: u16,
    pub status: TraceLifecycleStatus,
    pub created_at: u64,
    pub trace_path: Option<String>,
    pub sha256: Option<String>,
    pub initial_root: Hash,
    pub final_root: Hash,
}

#[derive(Debug, Clone)]
pub struct PersistedTraceMeta {
    pub trace_path: PathBuf,
    pub sha256_hex: String,
}

pub fn persist_trace(trace_root: &Path, trace: &ExecutionTraceV1) -> anyhow::Result<PersistedTraceMeta> {
    let batch_dir = trace_root.join(&trace.batch_id);
    fs::create_dir_all(&batch_dir)?;

    let final_path = batch_dir.join(format!("{}.json", trace.trace_id));
    let tmp_path = batch_dir.join(format!("{}.tmp", trace.trace_id));
    let sha_path = batch_dir.join(format!("{}.sha256", trace.trace_id));

    let bytes = serde_json::to_vec_pretty(trace)?;
    let sha = Sha256::digest(&bytes);
    let sha_hex = hex::encode(sha);

    {
        let mut file = OpenOptions::new().create(true).write(true).truncate(true).open(&tmp_path)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
    }

    fs::rename(&tmp_path, &final_path)?;

    {
        let mut sha_file = OpenOptions::new().create(true).write(true).truncate(true).open(&sha_path)?;
        sha_file.write_all(sha_hex.as_bytes())?;
        sha_file.write_all(b"\n")?;
        sha_file.sync_all()?;
    }

    if let Some(parent) = final_path.parent() {
        if let Ok(dir_file) = OpenOptions::new().read(true).open(parent) {
            let _ = dir_file.sync_all();
        }
    }

    Ok(PersistedTraceMeta {
        trace_path: final_path,
        sha256_hex: sha_hex,
    })
}

pub fn verify_trace_hash(path: &Path, expected_sha_hex: &str) -> anyhow::Result<()> {
    let bytes = fs::read(path)?;
    let got = hex::encode(Sha256::digest(&bytes));
    if got != expected_sha_hex {
        anyhow::bail!("trace hash mismatch: expected {}, got {}", expected_sha_hex, got);
    }
    Ok(())
}

pub fn append_lifecycle(
    trace_root: &Path,
    trace: &ExecutionTraceV1,
    status: TraceLifecycleStatus,
    trace_path: Option<&Path>,
    sha256: Option<&str>,
) -> anyhow::Result<()> {
    fs::create_dir_all(trace_root)?;
    let index_path = trace_root.join("index.jsonl");
    let entry = TraceIndexEntry {
        trace_id: trace.trace_id.clone(),
        batch_id: trace.batch_id.clone(),
        schema_version: trace.schema_version,
        status,
        created_at: trace.created_at,
        trace_path: trace_path.map(|p| p.display().to_string()),
        sha256: sha256.map(|s| s.to_string()),
        initial_root: trace.public_inputs.initial_root,
        final_root: trace.public_inputs.final_root,
    };

    let line = serde_json::to_string(&entry)?;
    let mut file = OpenOptions::new().create(true).append(true).open(index_path)?;
    file.write_all(line.as_bytes())?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ExecutionTraceV1, ProverContext, TracePublicInputs};

    fn dummy_trace() -> ExecutionTraceV1 {
        ExecutionTraceV1 {
            trace_id: "t1".to_string(),
            schema_version: 1,
            batch_id: "b1".to_string(),
            created_at: 1,
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
                guest_method_id: "m".to_string(),
                expected_journal_hash: [5u8; 32],
                backend_config_fingerprint: [6u8; 32],
            },
        }
    }

    #[test]
    fn persists_and_verifies_hash() {
        let root = tempfile::tempdir().unwrap();
        let trace = dummy_trace();
        let meta = persist_trace(root.path(), &trace).unwrap();
        verify_trace_hash(&meta.trace_path, &meta.sha256_hex).unwrap();
    }
}
