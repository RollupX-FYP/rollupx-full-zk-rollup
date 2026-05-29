use crate::types::{sha256_hash, ExecutionTraceV1};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::process::Command;

#[derive(Clone)]
pub struct ProverBackend {
    pub kind: ProverBackendKind,
}

#[derive(Clone)]
pub enum ProverBackendKind {
    Risc0 {
        host_binary: PathBuf,
        guest_elf: Option<PathBuf>,
        work_dir: PathBuf,
    },
    Mock,
}

pub struct ProofArtifacts {
    pub proof: Vec<u8>,
    pub da_commitment: Vec<u8>,
    pub journal_bytes: usize,
    pub proof_bytes: usize,
    pub metadata: ProofMetadataMetrics,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProofMetadataMetrics {
    pub witness_generation_ms: u64,
    pub zkvm_execution_ms: u64,
    pub proof_compression_ms: u64,
    pub total_prover_wall_ms: u64,
    pub trace_read_ms: u64,
    pub output_write_ms: u64,
    pub total_cycles: u64,
    pub total_segments: usize,
    pub proof_mode: String,
}

#[derive(Debug, Deserialize)]
struct ProofRunMetadata {
    status: String,
    #[serde(default)]
    proof_mode: Option<String>,
    trace_sha256: String,
    public_inputs_hash: String,
    journal_sha256: String,
    proof_sha256: String,
    journal_bytes: usize,
    proof_bytes: usize,

    // Timing breakdown (ms)
    witness_generation_ms: u64,
    zkvm_execution_ms: u64,
    proof_compression_ms: u64,
    total_prover_wall_ms: u64,
    trace_read_ms: u64,
    output_write_ms: u64,

    // RISC0-specific metrics
    total_cycles: u64,
    total_segments: usize,
}

pub fn backend_from_env() -> anyhow::Result<ProverBackend> {
    let backend_kind = std::env::var("PROVER_BACKEND").unwrap_or_else(|_| "risc0".to_string());
    if backend_kind.eq_ignore_ascii_case("mock") {
        return Ok(ProverBackend {
            kind: ProverBackendKind::Mock,
        });
    }
    if !backend_kind.eq_ignore_ascii_case("risc0") {
        anyhow::bail!(
            "unsupported PROVER_BACKEND '{}': supported values are 'risc0' and 'mock'",
            backend_kind
        );
    }

    let host_binary = PathBuf::from(
        std::env::var("RISC0_HOST_BIN")
            .map_err(|_| anyhow::anyhow!("RISC0_HOST_BIN is required"))?,
    );
    let guest_elf = std::env::var("RISC0_GUEST_ELF")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .map(PathBuf::from);
    let work_dir =
        PathBuf::from(std::env::var("RISC0_WORK_DIR").unwrap_or_else(|_| "tmp/risc0".to_string()));

    Ok(ProverBackend {
        kind: ProverBackendKind::Risc0 {
            host_binary,
            guest_elf,
            work_dir,
        },
    })
}

pub fn generate_artifacts(
    trace: &ExecutionTraceV1,
    backend: &ProverBackend,
) -> anyhow::Result<ProofArtifacts> {
    match &backend.kind {
        ProverBackendKind::Risc0 {
            host_binary,
            guest_elf,
            work_dir,
        } => generate_risc0_artifacts(trace, host_binary, guest_elf, work_dir),
        ProverBackendKind::Mock => generate_mock_artifacts(trace),
    }
}

pub fn backend_label(backend: &ProverBackend) -> &'static str {
    match &backend.kind {
        ProverBackendKind::Risc0 { .. } => "risc0",
        ProverBackendKind::Mock => "mock",
    }
}

fn generate_mock_artifacts(trace: &ExecutionTraceV1) -> anyhow::Result<ProofArtifacts> {
    let core_trace = to_rollup_core_trace(trace);
    let core_trace_bytes = serde_json::to_vec(&core_trace)?;
    let proof =
        Sha256::digest([b"rollupx-mock-proof".as_slice(), &core_trace_bytes].concat()).to_vec();
    let journal = [
        trace.public_inputs.initial_root.as_slice(),
        trace.public_inputs.final_root.as_slice(),
    ]
    .concat();
    let da_commitment = sha256_hash(&journal).to_vec();

    Ok(ProofArtifacts {
        proof_bytes: proof.len(),
        journal_bytes: journal.len(),
        proof,
        da_commitment,
        metadata: ProofMetadataMetrics {
            witness_generation_ms: 0,
            zkvm_execution_ms: 0,
            proof_compression_ms: 0,
            total_prover_wall_ms: 0,
            trace_read_ms: 0,
            output_write_ms: 0,
            total_cycles: 0,
            total_segments: 0,
            proof_mode: "mock".to_string(),
        },
    })
}

fn generate_risc0_artifacts(
    trace: &ExecutionTraceV1,
    host_binary: &PathBuf,
    guest_elf: &Option<PathBuf>,
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

    let mut cmd = Command::new(host_binary);
    cmd.arg(&trace_path);
    if let Some(path) = guest_elf {
        cmd.arg(path);
    }
    let output = cmd
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
    let proof_mode = meta
        .proof_mode
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    let allow_fallback = std::env::var("ALLOW_PROOF_FALLBACK")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if proof_mode != "groth16" && !allow_fallback {
        anyhow::bail!(
            "unsupported proof mode '{}' from host metadata; set ALLOW_PROOF_FALLBACK=1 to override",
            proof_mode
        );
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

    Ok(ProofArtifacts {
        proof,
        da_commitment,
        journal_bytes: meta.journal_bytes,
        proof_bytes: meta.proof_bytes,
        metadata: ProofMetadataMetrics {
            witness_generation_ms: meta.witness_generation_ms,
            zkvm_execution_ms: meta.zkvm_execution_ms,
            proof_compression_ms: meta.proof_compression_ms,
            total_prover_wall_ms: meta.total_prover_wall_ms,
            trace_read_ms: meta.trace_read_ms,
            output_write_ms: meta.output_write_ms,
            total_cycles: meta.total_cycles,
            total_segments: meta.total_segments,
            proof_mode,
        },
    })
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
