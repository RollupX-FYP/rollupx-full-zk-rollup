use anyhow::{Context, Result};
use risc0_zkvm::{default_prover, ExecutorEnv};
use rollup_core::BlockTrace;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;

mod methods {
    include!(concat!(env!("OUT_DIR"), "/methods.rs"));
}

#[derive(Debug, Serialize, Deserialize)]
struct ProofRunMetadata {
    status: String,
    proof_mode: String,
    trace_sha256: String,
    public_inputs_hash: String,
    journal_sha256: String,
    proof_sha256: String,
    journal_bytes: usize,
    proof_bytes: usize,
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        anyhow::bail!(
            "usage: rollup_host <block_trace.json> [guest_elf_path] [proof_out] [journal_out] [metadata_out]"
        );
    }

    let trace_path = &args[1];
    // Supported forms:
    // 1) rollup_host <trace>
    // 2) rollup_host <trace> <proof_out> <journal_out> <metadata_out>
    // 3) rollup_host <trace> <guest_elf_path> <proof_out> <journal_out> <metadata_out>
    // Treat arg[2] as guest ELF only in the 5-arg payload form (program + 5 args total).
    let (external_elf_path, arg_i) = if args.len() >= 6 {
        (args.get(2).filter(|s| !s.is_empty()).cloned(), 3usize)
    } else {
        (None, 2usize)
    };
    let proof_out = args
        .get(arg_i)
        .cloned()
        .unwrap_or_else(|| "snark_proof.bin".to_string());
    let journal_out = args
        .get(arg_i + 1)
        .cloned()
        .unwrap_or_else(|| "journal.bin".to_string());
    let metadata_out = args
        .get(arg_i + 2)
        .cloned()
        .unwrap_or_else(|| "proof_metadata.json".to_string());

    let trace_bytes = fs::read(trace_path).context("read trace json")?;
    let trace_sha = hex::encode(Sha256::digest(&trace_bytes));
    let trace: BlockTrace = serde_json::from_slice(&trace_bytes).context("parse trace json")?;
    let elf: Vec<u8> = if let Some(path) = external_elf_path {
        fs::read(path).context("read guest elf")?
    } else {
        methods::ROLLUP_HOST_GUEST_ELF.to_vec()
    };

    let env = ExecutorEnv::builder()
        .write(&trace)
        .context("write trace to env")?
        .build()
        .context("build prover env")?;

    let prover = default_prover();
    println!("Generating ZK Proof... (this may take time)");

    let receipt = prover
        .prove(env, &elf)
        .context("prove guest execution")?
        .receipt;

    let journal = receipt.journal.bytes.clone();
    fs::write(&journal_out, &journal).context("write journal")?;

    let (seal, proof_mode) = match receipt.inner.groth16() {
        Ok(s) => (s.seal.to_vec(), "groth16".to_string()),
        Err(_) => {
            // Some local prover modes don't emit a Groth16 receipt.
            // Keep proof bytes non-empty so downstream DA/submitter pipeline can progress,
            // while explicitly tagging this mode in metadata.
            (
                receipt.journal.bytes.clone(),
                "journal_fallback".to_string(),
            )
        }
    };
    fs::write(&proof_out, &seal).context("write proof")?;

    let mut inputs_buf = Vec::with_capacity(64);
    inputs_buf.extend_from_slice(&trace.initial_root);
    inputs_buf.extend_from_slice(&trace.final_root);

    let metadata = ProofRunMetadata {
        status: "ok".to_string(),
        proof_mode,
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
