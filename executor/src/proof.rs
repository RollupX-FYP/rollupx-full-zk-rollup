use crate::types::{sha256_hash, ExecutionTraceV1};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::process::Command;

#[derive(Clone)]
pub struct ProverBackend {
    pub host_binary: PathBuf,
    pub guest_elf: PathBuf,
    pub work_dir: PathBuf,
}

pub struct ProofArtifacts {
    pub proof: Vec<u8>,
    pub da_commitment: Vec<u8>,
}

#[derive(Debug, Deserialize)]
struct ProofRunMetadata {
    status: String,
    trace_sha256: String,
    public_inputs_hash: String,
    journal_sha256: String,
    proof_sha256: String,
    journal_bytes: usize,
    proof_bytes: usize,
}

pub fn backend_from_env() -> anyhow::Result<ProverBackend> {
    let host_binary = PathBuf::from(
        std::env::var("RISC0_HOST_BIN")
            .map_err(|_| anyhow::anyhow!("RISC0_HOST_BIN is required"))?,
    );
    let guest_elf = PathBuf::from(
        std::env::var("RISC0_GUEST_ELF")
            .map_err(|_| anyhow::anyhow!("RISC0_GUEST_ELF is required"))?,
    );
    let work_dir = PathBuf::from(std::env::var("RISC0_WORK_DIR").unwrap_or_else(|_| "tmp/risc0".to_string()));

    Ok(ProverBackend {
        host_binary,
        guest_elf,
        work_dir,
    })
}

pub fn generate_artifacts(trace: &ExecutionTraceV1, backend: &ProverBackend) -> anyhow::Result<ProofArtifacts> {
    generate_risc0_artifacts(trace, &backend.host_binary, &backend.guest_elf, &backend.work_dir)
}

pub fn backend_label(backend: &ProverBackend) -> &'static str {
    let _ = backend;
    "risc0"
}

fn generate_risc0_artifacts(
    trace: &ExecutionTraceV1,
    host_binary: &PathBuf,
    guest_elf: &PathBuf,
    work_dir: &PathBuf,
) -> anyhow::Result<ProofArtifacts> {
    std::fs::create_dir_all(work_dir)?;

    let trace_path = work_dir.join(format!("trace_{}.json", trace.batch_id));
    let proof_path = work_dir.join(format!("proof_{}.bin", trace.batch_id));
    let journal_path = work_dir.join(format!("journal_{}.bin", trace.batch_id));
    let metadata_path = work_dir.join(format!("proof_meta_{}.json", trace.batch_id));

    let core_trace = to_rollup_core_trace(trace);
    let core_trace_bytes = serde_json::to_vec(&core_trace)?;
    std::fs::write(&trace_path, &core_trace_bytes)?;
    let expected_trace_sha = hex::encode(Sha256::digest(&core_trace_bytes));

    let output = Command::new(host_binary)
        .arg(&trace_path)
        .arg(guest_elf)
        .arg(&proof_path)
        .arg(&journal_path)
        .arg(&metadata_path)
        .output()?;

    if !output.status.success() {
        anyhow::bail!(
            "risc0 host failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let proof = std::fs::read(&proof_path)?;
    let journal = std::fs::read(&journal_path)?;
    let meta: ProofRunMetadata = serde_json::from_slice(&std::fs::read(&metadata_path)?)?;

    if meta.status != "ok" {
        anyhow::bail!("proof metadata status is not ok");
    }
    if meta.trace_sha256 != expected_trace_sha {
        anyhow::bail!("trace sha mismatch between host metadata and executor");
    }
    let expected_inputs = hex::encode(trace.prover_context.expected_journal_hash);
    if meta.public_inputs_hash != expected_inputs {
        anyhow::bail!("public input hash mismatch between host metadata and trace prover context");
    }

    let journal_sha = hex::encode(Sha256::digest(&journal));
    if meta.journal_sha256 != journal_sha || meta.journal_bytes != journal.len() {
        anyhow::bail!("journal artifact integrity mismatch");
    }

    let proof_sha = hex::encode(Sha256::digest(&proof));
    if meta.proof_sha256 != proof_sha || meta.proof_bytes != proof.len() {
        anyhow::bail!("proof artifact integrity mismatch");
    }

    if proof.is_empty() {
        anyhow::bail!("proof artifact is empty");
    }

    let da_commitment = sha256_hash(&journal).to_vec();

    Ok(ProofArtifacts { proof, da_commitment })
}

fn to_rollup_core_trace(trace: &ExecutionTraceV1) -> rollup_core::BlockTrace {
    rollup_core::BlockTrace {
        batch_id: trace.batch_id.clone(),
        initial_root: trace.public_inputs.initial_root,
        final_root: trace.public_inputs.final_root,
        state_diffs: trace
            .state_diffs
            .iter()
            .map(|d| rollup_core::StateDiff {
                account: d.account,
                old_balance: d.old_balance,
                new_balance: d.new_balance,
                old_nonce: d.old_nonce,
                new_nonce: d.new_nonce,
                merkle_proof: d.merkle_proof.clone(),
            })
            .collect(),
    }
}
