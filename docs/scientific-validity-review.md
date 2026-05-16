# Scientific Validity Review

This review evaluates the current RollupX implementation as a research tool. The short version: it is useful for controlled engineering experiments and internal comparative studies, but its results should be framed as local prototype measurements, not as production rollup or Ethereum mainnet performance claims.

## Overall Judgment

| Validity dimension | Rating | Reason |
|---|---|---|
| Internal validity | Moderate | The harness resets state, uses seeds, records config, and waits for component metric parity. However, warmup contamination and some schema mismatches weaken causal interpretation. |
| Construct validity | Mixed | Batch size, local execution time, proof wall time, and local gas receipts are meaningful. Fairness, MEV, blob utilization, and mainnet cost constructs are only approximations. |
| External validity | Low to moderate | The system is single-node, local, transfer-centric, and not public-network representative. |
| Reproducibility | Moderate | Seeds, run metadata, git commit, and Docker recreation help. Full reproducibility still depends on local hardware, Docker images, RISC0 build artifacts, and Hardhat behavior. |
| Statistical validity | Currently limited | Repeats exist, but the default matrix is narrow and analysis scripts need schema reconciliation before publication-quality inference. |

## What Is Scientifically Credible

The implementation can credibly support claims like:

- Under this local single-node setup, fixed batch-size changes alter observed batch counts, per-batch wait distributions, and local proof/submitter load.
- Under identical synthetic traffic, FCFS, FeePriority, and BlobPacking produce different batch composition and fee/blob-size proxy patterns.
- The executor/prover path has measurable wall-clock phases for this simplified STF and RISC0 proof artifact path.
- Calldata, blob-like, and offchain DA modes produce different local settlement/cost metric rows under the repository's configured model.
- Component-level instrumentation can identify bottlenecks in the prototype pipeline.

These are valid engineering research findings when clearly scoped to the repository's implementation and local environment.

## Claims That Are Not Yet Valid

Avoid or heavily qualify claims like:

- "This is the cost of RollupX on Ethereum mainnet."
- "Blob mode measures real EIP-4844 economics" unless `real_eip4844_blob = true` and receipt blob fields are present.
- "The system is EVM-equivalent."
- "The proof proves full rollup state-transition validity."
- "FairBFT demonstrates decentralized sequencing fairness."
- "The benchmark predicts production throughput."
- "MEV resistance is measured" based only on the current fee/order proxies.

## Key Validity Risks

### 1. Warmup Contamination

The workload generator sends warmup traffic but discards it from workload metrics. The live sequencer/executor/submitter do not tag warmup transactions. If warmup traffic remains queued or creates batches during the measured phase, component metrics include warmup effects while workload metrics exclude them.

Impact: latency, batch count, cost per tx, and throughput comparisons can be biased.

Fix: tag every transaction with phase/run id, discard warmup-tagged component rows, or wait until the pipeline drains before starting measurement.

### 2. Local L1 Is Not Mainnet

Hardhat/local mining, local base fees, local receipt timing, and deterministic deployment behavior are not public Ethereum. The cost model partly compensates with reference gas/ETH prices, but it cannot reproduce public mempool conditions, EIP-1559 dynamics, blob base fee dynamics, reorgs, or validator timing.

Impact: local L1 latency and cost results are comparative only.

Fix: clearly label local results; run a separate public testnet/mainnet-shadow study for market-facing claims.

### 3. Blob Mode May Be Hybrid

Submitter metrics distinguish real and estimated blob cost using:

- `real_eip4844_blob`,
- `cost_source`,
- `blob_cost_source`,
- `measured_blob_gas_used`,
- `estimated_blob_gas_used`.

If blob receipt fields are absent, blob cost is estimated. That is acceptable for model comparison, but not for claims about actual EIP-4844 fee-market outcomes.

Fix: separate measured blob rows from hybrid rows in all plots/tables.

### 4. Simplified Execution Semantics

The executor is a transfer-centric STF with lightweight state diffs and hash-derived roots. It validates signatures, nonce, balance, and transfer updates, but it does not execute arbitrary EVM bytecode.

Impact: execution/proof timing does not generalize to full smart-contract workloads.

Fix: describe it as a synthetic STF; add EVM-equivalent execution or a calibrated trace corpus before broader claims.

### 5. Proof Construct Is Narrow

The RISC0 guest verifies replay of simplified state diffs from initial root to final root. This is valuable, but it is not a full rollup validity circuit. It proves consistency of the trace model, not end-to-end correctness of arbitrary L2 execution.

Impact: proof timing and proof-size measurements are valid for this circuit, not for a production rollup circuit.

Fix: specify the circuit statement precisely in papers/reports.

### 6. Aggregation Schema Drift

`data-tools/aggregate.py` contains historical field names that do not fully match current emitters. Examples:

- executor currently emits `total_execution_ms` and `total_proof_ms`, while aggregation references `execution_time_ms` and `proof_time_ms` in batch rows;
- sequencer emits `wait_time_*`, while aggregation references `oldest_tx_wait_ms`;
- sequencer does not emit `batch_data_bytes`, while aggregation references it from the sequencer row.

Impact: CSV outputs may contain missing/empty columns even when raw JSONL is correct.

Fix: update aggregation mappings and add schema tests using real fixture rows.

### 7. One-Factor Matrix Limits

The configured experiments mostly vary one factor at 10 TPS with balanced traffic. This is good for first-order debugging but weak for interaction effects.

Impact: conclusions may not hold at higher load, different mixes, or combined DA/scheduler/prover settings.

Fix: add factorial or fractional-factorial runs across rate, mix, DA, scheduler, and batch policy.

### 8. Fairness and MEV Metrics Are Proxies

Jain's fairness index is computed over in-batch wait times. That can describe wait-time equality among included transactions, but it does not prove user-level fairness or censorship resistance. The MEV/order metrics are also simplified fee-order proxies.

Impact: fairness/MEV conclusions should be descriptive, not normative.

Fix: define adversarial workloads, measure starvation, include rejected/delayed transactions, and compute fairness over the whole run.

### 9. Single Sender and Nonce Structure

The default workload uses one Hardhat account/private key with sequential nonces. That is reproducible, but it is not representative of multi-user mempool behavior, account contention, or heterogeneous balances.

Impact: validation/cache contention and fairness behavior are underexplored.

Fix: add multi-account workloads with controlled account distributions.

### 10. Resource Metrics Are Weak

`resource_metrics.json` currently captures a Docker stats snapshot/proxy, not a rigorous peak memory profile over the whole run.

Impact: memory claims should be treated as diagnostic, not scientific.

Fix: sample resource metrics continuously and record time series per component.

## Validity Envelope for Current Results

A defensible report statement would be:

> These results measure a local, single-node RollupX prototype with a transfer-centric STF, RISC0 state-diff proof path, synthetic Poisson workloads, and local Hardhat settlement. Cost metrics are comparative and may combine measured local gas with configured or estimated fee references. Results should be interpreted as implementation-level evidence about relative behavior under controlled conditions, not as production or mainnet performance.

## Recommendations Before Publication

1. Patch aggregation schema drift and add fixture tests.
2. Add phase/run identifiers to transactions and component metrics.
3. Drain or reset after warmup before measured collection.
4. Add multi-account workload generation.
5. Add rate and tx-mix sweeps, not only batch-size sweeps.
6. Separate real, hybrid, estimated, and simulated cost rows in all analysis.
7. Report confidence intervals and raw sample counts.
8. Archive `run_metadata.json`, diagnostics, git commit, and raw JSONL with every figure.
9. Explicitly state the STF and proof statement.
10. Validate at least one end-to-end run against a public testnet if making external settlement claims.

## Bottom Line

RollupX is a valid research harness for studying relative behavior of this prototype under controlled local workloads. It is not yet a valid standalone source for broad scientific claims about production ZK-rollup performance, Ethereum mainnet economics, decentralized sequencing fairness, or full EVM validity. The right use is careful comparative experimentation with clear caveats and raw-metric transparency.

