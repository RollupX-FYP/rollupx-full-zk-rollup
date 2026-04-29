use anyhow::{Context, Result};
use risc0_zkvm::{default_prover, ExecutorEnv};
use rollup_core::BlockTrace;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;

#[derive(Debug, Serialize, Deserialize)]
struct ProofRunMetadata {
    status: String,
    trace_sha256: String,
    public_inputs_hash: String,
    journal_sha256: String,
    proof_sha256: String,
    journal_bytes: usize,
    proof_bytes: usize,
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        anyhow::bail!(
            "usage: rollup_host <block_trace.json> <guest_elf_path> [proof_out] [journal_out] [metadata_out]"
        );
    }

    let trace_path = &args[1];
    let elf_path = &args[2];
    let proof_out = args.get(3).cloned().unwrap_or_else(|| "snark_proof.bin".to_string());
    let journal_out = args.get(4).cloned().unwrap_or_else(|| "journal.bin".to_string());
    let metadata_out = args.get(5).cloned().unwrap_or_else(|| "proof_metadata.json".to_string());

    let trace_bytes = fs::read(trace_path).context("read trace json")?;
    let trace_sha = hex::encode(Sha256::digest(&trace_bytes));
    let trace: BlockTrace = serde_json::from_slice(&trace_bytes).context("parse trace json")?;
    let elf = fs::read(elf_path).context("read guest elf")?;

    let env = ExecutorEnv::builder()
        .write(&trace)
        .context("write trace to env")?
        .build()
        .context("build prover env")?;

    let prover = default_prover();
    println!("Generating ZK Proof... (this may take time)");

    let receipt = prover.prove(env, &elf).context("prove guest execution")?.receipt;

    let journal = receipt.journal.bytes.clone();
    fs::write(&journal_out, &journal).context("write journal")?;

    let seal = receipt.inner.groth16().map(|s| s.to_vec()).unwrap_or_default();
    fs::write(&proof_out, &seal).context("write proof")?;

    let mut inputs_buf = Vec::with_capacity(64);
    inputs_buf.extend_from_slice(&trace.initial_root);
    inputs_buf.extend_from_slice(&trace.final_root);

    let metadata = ProofRunMetadata {
        status: "ok".to_string(),
        trace_sha256: trace_sha,
        public_inputs_hash: hex::encode(Sha256::digest(&inputs_buf)),
        journal_sha256: hex::encode(Sha256::digest(&journal)),
        proof_sha256: hex::encode(Sha256::digest(&seal)),
        journal_bytes: journal.len(),
        proof_bytes: seal.len(),
    };

    fs::write(&metadata_out, serde_json::to_vec_pretty(&metadata)?).context("write metadata")?;

    println!("Proof generated successfully");
    println!(
        "journal_bytes={} proof_bytes={} metadata={} ",
        metadata.journal_bytes, metadata.proof_bytes, metadata_out
    );
    Ok(())
}
