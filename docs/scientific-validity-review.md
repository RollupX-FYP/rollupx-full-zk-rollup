# Scientific Validity Review

This review evaluates the current RollupX implementation as a research tool. The short version: it is useful for controlled prototype experiments, but results must be framed as local comparative measurements, not production rollup or Ethereum mainnet performance.

## Overall Judgment

| Validity dimension | Rating | Current assessment |
|---|---|---|
| Internal validity | Moderate, improving | State reset, seeded workloads, sender/nonce fix, nonce-safe BlobPacking, and run metadata help. Smoke mode can still pass without executor/submitter metrics, so run mode must be reported. |
| Construct validity | Mixed | Batch size, wait time, local proof time, and local gas receipts are meaningful. Fairness, MEV, blob economics, and mainnet cost are proxy constructs. |
| External validity | Low to moderate | Single-node, local Hardhat, simplified STF, synthetic workload, and local proof hardware limit generalization. |
| Reproducibility | Moderate | Git commit, seeds, Docker recreation, and run directories support replay. Hardware, Docker cache, proof artifacts, and local timing still affect outcomes. |
| Statistical validity | Limited to moderate | The matrix now includes batch, policy, DA, rate, mix, and sender sweeps, but publication-grade claims still require enough repeats and schema-checked aggregation. |

## What Is Scientifically Credible

The implementation can support carefully scoped claims such as:

- In this local prototype, changing batch size changes wait-time distribution, sealed batch count, and downstream proof/submission pressure.
- Under identical synthetic workloads, scheduling policies produce different ordering and batch-composition metrics.
- The nonce-safe BlobPacking implementation avoids deliberately selecting transactions that violate per-sender nonce prefixes.
- The simplified executor/prover path has measurable wall-clock and artifact-size behavior for its state-diff circuit.
- Local calldata/blob/offchain DA modes produce different measured or model-derived cost rows, when separated by provenance.
- Component metrics can identify bottlenecks and lag in the prototype pipeline.

## Claims That Are Not Valid Without More Evidence

Avoid or heavily qualify:

- "RollupX achieves this throughput in production."
- "These costs are Ethereum mainnet costs."
- "Blob mode measures real EIP-4844 economics" unless `real_eip4844_blob = true` and receipt blob fields are present.
- "The system is EVM-equivalent."
- "The proof proves full rollup validity."
- "FairBFT proves decentralized sequencing fairness."
- "MEV resistance is measured."

## Major Validity Risks

### 1. Smoke Runs Are Partial Validation

Fast smoke mode can pass with `REQUIRE_COMPONENT_METRICS=0` and `REQUIRE_L1_VALIDATION=0`. That is useful for debugging but not sufficient for full pipeline claims.

Impact: a passing report may show workload/sequencer health while executor, submitter, and L1 bridge are missing or still lagging.

Fix: for research results, use strict mode: `STRICT_PIPELINE_CATCHUP=1`, `REQUIRE_COMPONENT_METRICS=1`, `REQUIRE_L1_VALIDATION=1`.

### 2. Executor/Submitter Lag Is Real

Sequencer can seal many batches before executor proof work and submitter settlement catch up. Short waits can produce missing executor/submitter files even when the workload succeeded.

Impact: comparing only sequencer metrics overstates end-to-end throughput.

Fix: report both front-door success and full-pipeline completion; track unique batch ids by component.

### 3. Warmup Contamination

Warmup transactions are sent into the live system, while workload summaries discard them. If component metrics are not reset/drained cleanly, warmup effects can leak into measured component rows.

Impact: latency, cost per tx, and batch count may be biased.

Fix: tag phase/run id through component metrics or wait for complete pipeline drain before measurement.

### 4. Local L1 Is Not Mainnet

Hardhat timing, local gas behavior, and deterministic deployments do not reproduce public Ethereum mempool/base-fee/blob-fee dynamics.

Impact: local L1 latency and cost are comparative only.

Fix: label local results clearly; use public testnet or mainnet-shadow experiments for market-facing claims.

### 5. Blob Economics Are Often Hybrid

Blob mode records provenance fields. If receipt-backed blob gas is absent, blob cost is estimated or hybrid.

Impact: blob rows may validate a cost model, not real EIP-4844 behavior.

Fix: separate rows by `real_eip4844_blob`, `cost_source`, and `blob_cost_source`.

### 6. Simplified Execution Semantics

The executor implements a transfer-centric STF, not arbitrary EVM bytecode execution.

Impact: proof/execution results do not generalize to smart-contract-heavy rollup workloads.

Fix: describe the STF exactly; add EVM-equivalent execution or calibrated traces before broader claims.

### 7. Narrow Proof Statement

The RISC0 path proves replay consistency for simplified state diffs. It does not prove a production rollup VM.

Impact: proof size/time are valid for this circuit only.

Fix: state the public input and proof statement explicitly in research text.

### 8. Fairness and MEV Proxies Are Weak

Jain's fairness index is computed from included transaction wait times, and ordering efficiency is a simplified fee-proxy comparison.

Impact: these do not prove user-level fairness, censorship resistance, or MEV resistance.

Fix: add adversarial workloads, starvation metrics, rejected/delayed transaction accounting, and whole-run fairness metrics.

### 9. Workload Realism Is Limited

The workload is synthetic. The new multi-account sweeps improve nonce/fairness coverage, but sender distribution, balances, transaction semantics, and calldata are still simplified.

Impact: results are workload-model dependent.

Fix: report workload parameters fully and add calibrated real-trace or trace-inspired workloads.

### 10. Aggregation Schema Drift

Raw JSONL emitters evolve faster than aggregation scripts. Historical field names can silently produce missing aggregate columns.

Impact: CSV-derived plots may be wrong even when raw metrics are correct.

Fix: add schema fixture tests and prefer raw JSONL for final checks.

## Validity Improvements Already Implemented

- Workload sender selection now passes explicit `sender_index` to transaction signing, aligning sender metadata, private key, `from`, and nonce.
- BlobPacking is nonce-safe and only selects contiguous per-sender nonce prefixes from state-cache expected nonce.
- BlobPacking emits selection diagnostics: selected bytes, eligible bytes, eligible count, nonce gaps, truncated senders, and low-fill reason.
- Harness supports strict vs smoke validation, making partial runs explicit.
- Experiment matrix includes rate, transaction mix, and sender/concurrency sweeps in addition to batch, policy, DA, and batch-policy sweeps.
- Resource sampling produces a time series rather than relying only on a final memory snapshot.

## Validity Envelope for Current Results

A defensible statement is:

> These results measure a local, single-node RollupX prototype using a transfer-centric STF, RISC0 state-diff proof path, synthetic seeded workloads, and local Hardhat settlement. Metrics are suitable for comparing configurations within this implementation when run mode, cost provenance, DA provenance, and raw schemas are reported. They should not be interpreted as production rollup, mainnet Ethereum, decentralized sequencing, or full EVM validity results.

## Recommended Publication Checklist

Before using results in a report or paper:

1. Run strict mode for any end-to-end claim.
2. Verify `scheduling_policy`, `da_mode`, git commit, and run environment in `run_metadata.json`.
3. Require nonzero sequencer/executor/submitter batch ids for full-pipeline results.
4. Separate smoke runs from strict runs.
5. Separate measured/hybrid/estimated/simulated cost rows.
6. Use confidence intervals across repeats.
7. Validate aggregation schema against current raw JSONL.
8. Archive raw metrics and diagnostics with every figure.
9. State the simplified STF and proof statement.
10. Avoid mainnet/economic/decentralization claims without separate external validation.

## Bottom Line

RollupX is scientifically valid as a controlled prototype benchmark for relative, implementation-level comparisons. It is not yet scientifically valid as a standalone source for broad claims about production ZK-rollup performance, Ethereum mainnet economics, decentralized fairness, or full EVM validity.
