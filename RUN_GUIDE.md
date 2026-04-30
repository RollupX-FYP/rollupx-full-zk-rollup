# RollupX — Full Stack Run Guide

Step-by-step instructions to bring up the entire ZK-Rollup pipeline and run the benchmark suite.

> **Platform note:** All shell commands are written for **Linux / WSL2**. If you are on
> Windows natively, use **WSL2** or **Git Bash** — the benchmark orchestration scripts
> (`run_experiment.sh`, `run_matrix.sh`, `reset_state.sh`, `run_pipeline.sh`) are pure Bash.

---

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [Prerequisites](#2-prerequisites)
3. [One-Time Setup](#3-one-time-setup)
4. [Start the L1 Node (Terminal A)](#4-start-the-l1-node-terminal-a)
5. [Deploy Contracts (Terminal E)](#5-deploy-contracts-terminal-e)
6. [Start the Executor (Terminal B)](#6-start-the-executor-terminal-b)
7. [Start the Submitter (Terminal C)](#7-start-the-submitter-terminal-c)
8. [Start the Sequencer (Terminal D)](#8-start-the-sequencer-terminal-d)
9. [Verify All Services Are Running](#9-verify-all-services-are-running)
10. [Run the Benchmark Suite](#10-run-the-benchmark-suite)
11. [Validate End-to-End Pipeline](#11-validate-end-to-end-pipeline)
12. [Analyse Results (Data Tools)](#12-analyse-results-data-tools)
13. [Configuration Reference](#13-configuration-reference)
14. [Troubleshooting](#14-troubleshooting)

---

## 1. Architecture Overview

```
                                        ┌───────────────┐
  Workload Generator ──JSON-RPC──►      │   Sequencer   │ :3000
  (Python, Poisson)                     │   (Rust)      │
                                        └──────┬────────┘
                                               │ gRPC batch
                                        ┌──────▼────────┐
                                        │   Executor    │ :50051
                                        │   (Rust)      │
                                        └──┬─────────┬──┘
                                  traces   │         │ gRPC stream
                              ┌────────────▼──┐  ┌───▼──────────┐
                              │  RISC0 Prover │  │  Submitter   │
                              │  (Rust)       │  │  (Rust)      │
                              └───────────────┘  └──────┬───────┘
                                                        │ on-chain tx
                                                 ┌──────▼───────┐
                                                 │  Hardhat L1  │ :8545
                                                 │  (Node.js)   │
                                                 └──────────────┘
```

**You need 5 terminal windows** (or `tmux` panes):

| Terminal | Component          | Listens on |
| -------- | ------------------ | ---------- |
| A        | Hardhat L1 node    | `:8545`    |
| B        | Executor + prover  | `:50051`   |
| C        | Submitter          | —          |
| D        | Sequencer          | `:3000`    |
| E        | Deploy / Benchmark | —          |

**Startup order matters:** A → (deploy) → B → C → D → (benchmark from E)

---

## 2. Prerequisites

| Tool        | Version / Notes                                               |
| ----------- | ------------------------------------------------------------- |
| **OS**      | Ubuntu 22.04+ / Debian 12+ / WSL2 on Windows                  |
| **Rust**    | `stable` + `nightly-2024-08-01` (executor)                    |
| **Node.js** | v18+ (for Hardhat)                                            |
| **Python**  | 3.11+ (for workload generator; `tomllib` is stdlib from 3.11) |
| **SQLite3** | CLI (for troubleshooting)                                     |
| **NASM**    | Required by some Rust crypto deps on Linux/WSL                |
| **jq**      | Optional, used by `collect_env.sh`                            |
| **curl**    | For health checks                                             |

---

## 3. One-Time Setup

Run these once on a fresh machine / environment.

### 3.1 System Packages (Ubuntu / Debian / WSL2)

```bash
sudo apt update
sudo apt install -y \
  build-essential clang libclang-dev cmake pkg-config \
  libssl-dev sqlite3 python3-venv nasm jq curl gettext-base
```

> `gettext-base` provides `envsubst`, needed by `run_experiment.sh` to template the sequencer config.

### 3.2 Rust Toolchains

```bash
# Install rustup if not already present
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

# Install the two required toolchains
rustup toolchain install stable
rustup toolchain install nightly-2024-08-01
```

> The executor workspace pins `nightly-2024-08-01` in `executor/rust-toolchain.toml`.
> The RISC0 prover and submitter build on `stable`.

### 3.3 Node.js Dependencies (Contracts)

```bash
cd contracts
npm install
cd ..
```

### 3.4 Python Virtual Environment (Benchmark Suite)

```bash
cd benchmark-suite
python3 -m venv .venv
source .venv/bin/activate
pip install eth-account
cd ..
```

### 3.5 Build All Rust Binaries

```bash
# 1. RISC0 Prover Host
cd risc0_prover
cargo +stable build -p rollup_host
cd ..

# 2. Executor (large workspace — takes a few minutes first time)
cd executor
cargo build --ignore-rust-version
cd ..

# 3. Sequencer (--release for benchmark-quality timing)
cd sequencer
cargo build --release
cd ..

# 4. Submitter
cargo build --manifest-path submitter/Cargo.toml
```

After this step, the key binaries are:

| Binary     | Path                                         |
| ---------- | -------------------------------------------- |
| Executor   | `executor/target/debug/zksync_state_machine` |
| RISC0 host | `risc0_prover/target/debug/rollup_host`      |
| Sequencer  | `sequencer/target/release/sequencer`         |
| Submitter  | `submitter/target/debug/submitter`           |

---

## 4. Start the L1 Node (Terminal A)

```bash
cd contracts
npx hardhat node
```

**Expected output:**

```
Started HTTP and WebSocket JSON-RPC server at http://127.0.0.1:8545/

Accounts
========
Account #0: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 (10000 ETH)
Private Key: 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
...
```

Leave this terminal running.

---

## 5. Deploy Contracts (Terminal E)

```bash
cd contracts
npx hardhat run scripts/deploy-local.ts --network localhost
```

**Expected output:**

```
MockVerifier: 0x5FbDB2315678afecb367f032d93F642f64180aa3
ZKRollupBridge: 0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512
GenesisRoot: 0x0000000000000000000000000000000000000000000000000000000000000000
```

> **Important:** The default configs already contain `0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512`.
> If your deployed address is different, update:
>
> - `sequencer/config/default.toml` → `[l1].bridge_address`
> - `submitter/config/local-offchain-risc0.yaml` → `contracts.bridge`

### Optional: Configure Off-Chain DA Provider

```bash
cd contracts
npx hardhat run scripts/tmp_setup_da.js --network localhost
```

---

## 6. Start the Executor (Terminal B)

Replace `<PROJECT_ROOT>` with your absolute path to `rollupx-full-zk-rollup`.

```bash
export PROJECT_ROOT="/absolute/path/to/rollupx-full-zk-rollup"

PROVER_BACKEND=risc0 \
RISC0_HOST_BIN="$PROJECT_ROOT/risc0_prover/target/debug/rollup_host" \
EXECUTOR_GRPC_ADDR=127.0.0.1:50051 \
TRACE_ROOT="$PROJECT_ROOT/executor/tmp/traces" \
STATE_DB_PATH="$PROJECT_ROOT/executor/tmp/state_db" \
"$PROJECT_ROOT/executor/target/debug/zksync_state_machine"
```

**Alternative (cargo run — auto-resolves paths):**

```bash
cd executor
PROVER_BACKEND=risc0 \
EXECUTOR_GRPC_ADDR=127.0.0.1:50051 \
RUST_LOG=info \
cargo run -p zksync_state_machine --ignore-rust-version
```

**Expected:** You should see gRPC server starting on port `50051`.

Leave this terminal running.

---

## 7. Start the Submitter (Terminal C)

```bash
SUBMITTER_PRIVATE_KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 \
EXECUTOR_URL=http://127.0.0.1:50051 \
DATABASE_URL=sqlite://data/submitter.db \
RUST_LOG=info \
cargo run --manifest-path submitter/Cargo.toml --bin submitter \
  -- --config submitter/config/local-offchain-risc0.yaml
```

> The private key is Hardhat's default Account #0 — safe for local dev only.

Leave this terminal running.

---

## 8. Start the Sequencer (Terminal D)

**You must `cd sequencer` first** so that `config/default.toml` resolves correctly.

```bash
cd sequencer
RUST_LOG=info cargo run --release
```

**Expected:** Sequencer starts JSON-RPC on `http://127.0.0.1:3000`.

Leave this terminal running.

---

## 9. Verify All Services Are Running

From any terminal:

```bash
# Check all ports are bound
ss -ltnp | grep -E '8545|50051|3000'

# Quick health check (JSON-RPC probe)
curl -sf http://127.0.0.1:3000/ -X POST \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"rollup_health","params":{},"id":1}' \
  > /dev/null \
  && echo "Sequencer: OK" || echo "Sequencer: DOWN"
curl -sf http://127.0.0.1:8545 -X POST \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
  && echo "L1: OK" || echo "L1: DOWN"
```

---

## 10. Run the Benchmark Suite

Make sure you activate the Python venv first:

```bash
cd benchmark-suite
source .venv/bin/activate
```

### Option A — Single Ad-Hoc Workload Run

Send transactions manually, inspect results:

```bash
export METRICS_ROOT=metrics/e2e/e2e_r01

python workload/poisson_generator.py \
  --experiment_id e2e_real_risc0 \
  --run_id e2e_r01 \
  --rate 3 \
  --duration 10 \
  --warmup 2 \
  --seed 19 \
  --tx_mix balanced \
  --host localhost \
  --port 3000
```

**Output files:**

```
metrics/e2e/e2e_r01/
├── workload_e2e_real_risc0.json   ← generator-side metrics
├── tx_log_e2e_r01.csv             ← per-transaction latency log
└── run_status.json                ← pass/fail summary
```

**Workload generator CLI flags:**

| Flag               | Type  | Default     | Description                                    |
| ------------------ | ----- | ----------- | ---------------------------------------------- |
| `--rate`           | float | `1.0`       | Transaction arrival rate (Poisson λ, tx/sec)   |
| `--duration`       | int   | `120`       | Timed measurement phase in seconds             |
| `--warmup`         | int   | `15`        | Warm-up seconds (sent but not recorded)        |
| `--seed`           | int   | `None`      | PRNG seed for reproducibility                  |
| `--tx_mix`         | str   | `balanced`  | `balanced`, `light`, `heavy`, or `custom`      |
| `--mix_a/b/c`      | float | —           | Custom type fractions (when `--tx_mix custom`) |
| `--experiment_id`  | str   | auto        | Experiment identifier                          |
| `--run_id`         | str   | auto        | Unique run identifier                          |
| `--prover_backend` | str   | `groth16`   | Proof backend tag recorded in metrics          |
| `--host`           | str   | `localhost` | Sequencer host                                 |
| `--port`           | int   | `3000`      | Sequencer port                                 |

---

### Option B — Single Named Experiment (Automated)

`run_experiment.sh` automates: state reset → environment metadata collection → sequencer config
templating → workload generation → submitter flush wait.

> **Current branch caveat (important):**
> keep the sequencer running manually from `sequencer/` (`cargo run --release`).
> The script still contains legacy assumptions for sequencer auto-restart (`rollup_sequencer`
> path and `/health` check), and the generated `ROLLUPX_CONFIG` is not consumed by the current
> sequencer binary (which loads `config/default.toml` directly).

```bash
cd benchmark-suite
source .venv/bin/activate

# Run the "baseline" experiment, repeat 1
bash scripts/run_experiment.sh baseline 1
```

**What this does behind the scenes:**

1. Clears previous runtime state (`reset_state.sh`) — removes SQLite DBs, trace files, prover artifacts
2. Creates output directory at `metrics/baseline/baseline_r01/`
3. Captures environment metadata (CPU, RAM, OS, git commit, Rust/Python versions)
4. Templates `config/sequencer.template.toml` → writes a per-run config artifact
5. Attempts sequencer restart only for legacy script paths; on this branch you should keep sequencer manual
6. Runs `poisson_generator.py` with parameters from environment variables
7. Waits for the submitter to flush all pending batches (polls for 15s of idle)
8. Stamps `timestamp_end` into `run_metadata.json`

**Environment variables (override defaults):**

| Variable                 | Default     | Description                                                       |
| ------------------------ | ----------- | ----------------------------------------------------------------- |
| `MAX_BATCH_SIZE`         | `50`        | Sequencer batch size                                              |
| `TIMEOUT_MS`             | `5000`      | Batch timeout in ms                                               |
| `POLICY`                 | `FCFS`      | Scheduling policy (`FCFS`, `FeePriority`, `TimeBoost`, `FairBFT`) |
| `DA_MODE`                | `calldata`  | DA mode (`calldata`, `blob`, `offchain`)                          |
| `PROVER`                 | `groth16`   | Proof backend                                                     |
| `RATE_TPS`               | `10`        | Workload arrival rate (tx/s)                                      |
| `DURATION_S`             | `120`       | Measurement duration                                              |
| `WARMUP_S`               | `15`        | Warm-up duration                                                  |
| `TX_MIX`                 | `balanced`  | Transaction mix preset                                            |
| `SEED`                   | `42`        | PRNG seed                                                         |
| `SEQ_HOST`               | `localhost` | Sequencer host                                                    |
| `SEQ_PORT`               | `3000`      | Sequencer port                                                    |
| `CLEAN_STATE_BEFORE_RUN` | `1`         | Reset state before each run                                       |

**Example — custom parameters:**

```bash
MAX_BATCH_SIZE=200 RATE_TPS=50 POLICY=FeePriority SEED=99 \
  bash scripts/run_experiment.sh custom_test 1
```

---

### Option C — Full Experiment Matrix

The matrix is defined in `benchmark-suite/config/experiments.toml` and sweeps 7 factors:

> Same caveat as Option B: run and keep sequencer manually in a separate terminal during matrix runs.

| Factor     | Experiment IDs                              | Values Tested                   |
| ---------- | ------------------------------------------- | ------------------------------- |
| Batch size | `bs_010`, `bs_025`, `bs_100`, `bs_200`      | 10, 25, 100, 200                |
| Timeout    | `to_0500`, `to_1000`, `to_2500`, `to_10000` | 500ms – 10s                     |
| Policy     | `pol_fee`, `pol_timeboost`, `pol_fairbft`   | FeePriority, TimeBoost, FairBFT |
| DA mode    | `da_blob`, `da_offchain`                    | blob, offchain                  |
| Prover     | `pv_plonk`                                  | plonk                           |
| Input rate | `tps_005`, `tps_020`, `tps_050`             | 5, 20, 50 tx/s                  |
| Tx mix     | `mix_light`, `mix_heavy`                    | light, heavy                    |

Each experiment is run for the number of `repeats` (default: 5) defined in the `[baseline]` block,
with different seeds for each repeat.

**Dry-run first (preview only, no execution):**

```bash
cd benchmark-suite
bash scripts/run_matrix.sh --dry-run
```

**Run the full matrix:**

```bash
bash scripts/run_matrix.sh
```

**Filter to a single factor:**

```bash
bash scripts/run_matrix.sh --filter batch_size
bash scripts/run_matrix.sh --filter timeout
bash scripts/run_matrix.sh --filter policy
bash scripts/run_matrix.sh --filter da_mode
bash scripts/run_matrix.sh --filter prover
bash scripts/run_matrix.sh --filter rate
bash scripts/run_matrix.sh --filter tx_mix
```

---

## 11. Validate End-to-End Pipeline

### Check trace lifecycle

```bash
tail -n 50 executor/tmp/traces/index.jsonl
```

Statuses should progress: `generated` → `persisted` → `proved` → `published`

### Check proof artifacts

```bash
ls -lt executor/tmp/risc0/
```

Expected files per batch:

```
trace_<batch>.json
proof_<batch>.bin
journal_<batch>.bin
proof_meta_<batch>.json
```

### Check benchmark outputs

```bash
ls -la benchmark-suite/metrics/
```

Per-run directory structure:

```
metrics/<exp_id>/<run_id>/
├── workload_<exp_id>.json      # generator-side metrics (latency, counts)
├── executor_<exp_id>.json      # executor-side metrics (if emitted)
├── submitter_metrics.json      # JSONL, one entry per batch submitted
├── tx_log_<run_id>.csv         # per-tx latency log (for CDF/fairness)
├── run_metadata.json           # hw/sw/config snapshot + timestamps
├── run_status.json             # pass / fail / partial
└── run.log                     # full stdout/stderr capture
```

---

## 12. Analyse Results (Data Tools)

After benchmarks complete, run the data-tools pipeline to aggregate, compute statistics,
generate plots, and produce a thesis-ready summary.

```bash
export METRICS_ROOT=benchmark-suite/metrics
./run_pipeline.sh
```

This runs 4 stages:

| Stage         | Script                             | Output                                                                                        |
| ------------- | ---------------------------------- | --------------------------------------------------------------------------------------------- |
| 1. Aggregate  | `data-tools/aggregate.py`          | `all_results.csv`                                                                             |
| 2. Statistics | `data-tools/stats.py`              | `stats_summary.csv`                                                                           |
| 3. Plots      | `data-tools/plots/*.py`            | PNG files (Pareto, throughput bar, latency CDF, boxplot, fairness, cost heatmap, sensitivity) |
| 4. Report     | `data-tools/report/generate_md.py` | `thesis_summary.md`                                                                           |

**Filter to one experiment:**

```bash
./run_pipeline.sh test_batching
```

---

## 13. Configuration Reference

### Sequencer — `sequencer/config/default.toml`

```toml
[batch]
max_batch_size = 100       # max txs per batch
timeout_interval_ms = 5000 # seal batch after this timeout
min_batch_size = 10        # minimum txs before sealing
max_gas_limit = 30000000   # 30M gas limit for L1 verification

[scheduling]
policy_type = "FCFS"       # FCFS | FeePriority | TimeBoost | FairBFT

[api]
host = "127.0.0.1"
port = 3000

[l1]
rpc_url = "ws://127.0.0.1:8545/"
bridge_address = "0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512"
start_block = 18500000

[database]
url = "sqlite://sequencer.db"

[executor]
grpc_url = "http://127.0.0.1:50051"
```

### Submitter — `submitter/config/local-offchain-risc0.yaml`

```yaml
network:
  rpc_url: "http://127.0.0.1:8545"
  chain_id: 31337

contracts:
  bridge: "0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512"

da:
  mode: "offchain"
  blob_binding: "mock"
  blob_index: 0
  archiver_url: "http://mock-archiver"

batch:
  data_file: "dummy.bin"
  new_root: "0x00"
  blob_versioned_hash: "0x0100000000000000000000000000000000000000000000000000000000000000"

proof:
  backend: "groth16"
  verification_mode: "onchain"
  verifier_id: 0
```

### Benchmark Matrix — `benchmark-suite/config/experiments.toml`

See the file for all experiment definitions. Key baseline settings:

```toml
[baseline]
batch_size = 50
timeout_ms = 5000
policy     = "FCFS"
da_mode    = "calldata"
prover     = "groth16"
rate_tps   = 10
duration_s = 120
warmup_s   = 15
tx_mix     = "balanced"
repeats    = 5
seeds      = [42, 43, 44, 45, 46]
```

---

## 14. Troubleshooting

| Symptom                                                                   | Cause                                   | Fix                                                                     |
| ------------------------------------------------------------------------- | --------------------------------------- | ----------------------------------------------------------------------- |
| `No such file or directory (os error 2)` from sequencer                   | Not running from `sequencer/` directory | `cd sequencer` before `cargo run`                                       |
| `UNIQUE constraint failed: batches.batch_id`                              | Stale DB from previous run              | Delete `sequencer/sequencer.db` or use `reset_state.sh`                 |
| `pip install eth-account` fails                                           | Venv not activated                      | `source benchmark-suite/.venv/bin/activate`                             |
| Submitter can't reach executor gRPC                                       | Executor not running on `:50051`        | Start executor first; verify `EXECUTOR_URL` matches                     |
| Bridge address mismatch                                                   | Re-deployment gave a different address  | Update `default.toml` and `local-offchain-risc0.yaml`                   |
| `envsubst: command not found`                                             | Missing `gettext-base`                  | `sudo apt install gettext-base`                                         |
| `NASM command not found` / `Missing dependency: cmake` during Rust builds | Host toolchain deps missing             | `sudo apt install nasm cmake`                                           |
| `reset_state.sh` permission denied                                        | Scripts not executable                  | `chmod +x benchmark-suite/scripts/*.sh`                                 |
| `wait_for_sequencer.sh` fails on `/health`                                | Script probes legacy health endpoint    | Keep sequencer running manually; use JSON-RPC probe on `/` for liveness |
| `tomllib` not found                                                       | Python < 3.11                           | Upgrade to Python 3.11+                                                 |
| Executor build fails with toolchain error                                 | Wrong nightly version                   | Ensure `nightly-2024-08-01` is installed                                |

### Clean Restart

To reset all local runtime state and start fresh:

```bash
cd benchmark-suite
bash scripts/reset_state.sh manual_clean
```

This removes:

- `sequencer/sequencer.db` (and WAL/SHM files)
- `executor/tmp/*` (traces, state DB, prover artifacts)
- `submitter/data/submitter.db`
- `submitter/offchain_store/*`
