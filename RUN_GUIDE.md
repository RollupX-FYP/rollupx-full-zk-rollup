# RollupX — Full Stack Run Guide

Step-by-step instructions to bring up the entire ZK-Rollup pipeline, run the benchmark suite, and analyze the results.

> **Docker Compose (Recommended):** The fully containerized execution flow is the primary and easiest method to run the stack.
>
> **Platform note:** If you are running natively without Docker, all shell commands are written for **Linux / WSL2**. If you are on Windows natively, use **WSL2** or **Git Bash** — the benchmark orchestration scripts (`run_experiment.sh`, `run_matrix.sh`, `reset_state.sh`, `run_pipeline.sh`) are pure Bash. Docker commands can be run natively from **PowerShell** or **WSL**.

---

## Table of Contents

0. [Server Access & Preparation](#0-server-access--preparation)
1. [Containerized Execution (Recommended)](#1-containerized-execution-recommended)
2. [Continuous Integration (GitHub Actions)](#2-continuous-integration-github-actions)
3. [Architecture Overview](#3-architecture-overview)
4. [Native Execution: Prerequisites](#4-native-execution-prerequisites)
5. [Native Execution: One-Time Setup](#5-native-execution-one-time-setup)
6. [Native Execution: Start the L1 Node (Terminal A)](#6-native-execution-start-the-l1-node-terminal-a)
7. [Native Execution: Deploy Contracts (Terminal E)](#7-native-execution-deploy-contracts-terminal-e)
8. [Native Execution: Start the Executor (Terminal B)](#8-native-execution-start-the-executor-terminal-b)
9. [Native Execution: Start the Submitter (Terminal C)](#9-native-execution-start-the-submitter-terminal-c)
10. [Native Execution: Start the Sequencer (Terminal D)](#10-native-execution-start-the-sequencer-terminal-d)
11. [Validate End-to-End Pipeline](#11-validate-end-to-end-pipeline)
12. [Running Benchmark Experiments](#12-running-benchmark-experiments)
13. [Configuration Reference](#13-configuration-reference)
14. [Troubleshooting](#14-troubleshooting)

---

## 0. Server Access & Preparation

1. **Check the Connection After Setting Up the VPN**

   From your local terminal:

   ```bash
   ping 10.15.94.170
   ```

   What successful output looks like:

   ```bash
   Reply from 10.15.94.170: bytes=32 time=15ms TTL=64
   ```

2. **Connect to Server**

   From your local terminal:

   ```bash
   ssh cseroot@10.15.94.170
   ```

   Say yes to “Are you sure you want to continue connecting (yes/no/[fingerprint])?” and enter the password.

3. **Start Docker**

   ```bash
   sudo systemctl start docker
   ```

   Verify:

   ```bash
   systemctl status docker
   ```

4. **Navigate to Project Directory**

   ```bash
   cd rollupx-full-zk-rollup
   ```

5. **Switch Branch (Switch to the branch "connected" and pull for latest changes)**

   View branches:

   ```bash
   git branch -a
   ```

   Checkout:

   ```bash
   git checkout <branch-name>
   ```

   Pull the latest changes:

   ```bash
   git pull origin <branch-name>
   ```

## 1. Containerized Execution (Recommended)

The entire RollupX stack is fully containerized. You do not need to install Rust, Node, or set up multiple terminals. You can run these primary docker commands from **PowerShell** or **WSL**.

### 1.1 Cleanup and Start the Core Stack

Always ensure a clean state before starting, especially if you are re-running an experiment.

1. **Tear down existing containers and volumes:**

```bash
docker compose down -v --remove-orphans
```

2. **(Optional) Force Rebuild Images:**
   If you modified Dockerfiles or want a completely fresh image build without using cached layers, run:

```bash
docker compose --profile core build --no-cache
```

3. **Start the core stack:**

```bash
docker compose --profile core up -d --build
```

This single command spins up:

- `hardhat`: Local L1 node
- `contracts-deployer`: Automatically deploys contracts and outputs runtime config to a shared volume
- `executor`: The RISC0 executor
- `submitter`: Submits batches to L1
- `sequencer`: The transaction sequencer

### 1.2 Verify Stack Health

Wait a few seconds for services to become healthy, then run the verification script (requires **WSL** or **Git Bash**):

```bash
bash scripts/verify_stack.sh
```

### 1.3 Run the Full Benchmark Suite

All commands below are run **from the project root** (`rollupx-full-zk-rollup/`) on the **host machine**(normal terminal) (not inside a container).

To run a pre-defined experiment phase (builds images, recreates the stack for each run, and generates reports), use the matrix entry point:

```bash
# Run a quick smoke test (recommended for first-time validation)
bash benchmark-suite/scripts/run_matrix.sh --phase smoke

# Run the standard feasibility study
bash benchmark-suite/scripts/run_matrix.sh --phase feasibility
```

> [!NOTE]
> Every time you run an experiment, the results are stored in `benchmark-suite/metrics/`. If you use the `run_full_suite.sh` wrapper, it automatically creates a new timestamped folder (e.g., `benchmark-suite/metrics/run_20260503_150000/`) to prevent overwriting previous data.

**Output:** All raw metrics, plots, and markdown reports are saved directly to the host inside the new timestamped folder. Inside this folder you will find:

- `metrics/<experiment_id>/<run_id>/`: Raw parameters per-run (JSON + CSV)
- `all_results.csv` & `stats_summary.csv`: Aggregated flat lists and statistical summaries.
- `figures/`: Visual plots (throughput charts, latency CDFs, Pareto frontiers, etc.)
- `thesis_summary.md`: Auto-generated markdown report summarizing the findings.

---

<details>
<summary><strong>Advanced: Running Individual Components</strong></summary>

If you want to run specific parts instead of the full suite, you can run the individual scripts directly:

**Step 1 — Build the benchmark image** (once, or after Dockerfile changes):

```bash
docker compose --profile bench build benchmark --no-cache
```

**Step 2 — Run workload experiments** (rate, tx mix — no stack restart needed):

```bash
bash scripts/run_workload_matrix.sh
```

**Step 3 — Run infrastructure experiments** (batch size, timeout, policy, DA mode, prover):

```bash
bash scripts/run_infra_matrix.sh
```

**Step 4 — Generate Analytics Reports**:

```bash
bash scripts/generate_reports.sh
```

</details>

<details>
<summary><strong>Filtering to specific factors</strong></summary>

**Method 1: Using run_matrix.sh** (Recommended)

```bash
# Run only batch size experiments
bash benchmark-suite/scripts/run_matrix.sh --filter batch_size --phase smoke

# Run only DA mode experiments
bash benchmark-suite/scripts/run_matrix.sh --filter da_mode --phase smoke

# Preview what will run without executing
bash benchmark-suite/scripts/run_matrix.sh --filter batch_size --list
```

**Method 2: Workload vs. Infrastructure Experiments**

- **Infrastructure Factors** (batch size, timeout, policy, DA mode, prover): These require the Docker stack to be restarted for each run. Use `run_matrix.sh` on the host.
- **Workload Factors** (rate, tx mix): These can be run against a stable stack. You can run them via `run_matrix.sh --filter workload`.

</details>

<details>
<summary><strong>Running a quick smoke test</strong></summary>

A short, pre-configured single-pass benchmark to validate end-to-end functionality:

```bash
bash scripts/smoke_benchmark.sh
```

</details>

<details>
<summary><strong>Manual ad-hoc infrastructure runs</strong></summary>

For a one-off infrastructure test without the automated script, manually restart the stack:

```bash
# 1. Tear down
docker compose down -v --remove-orphans

# 2. Restart with the new config (from the project root)
SEQUENCER_BATCH_MAX_SIZE=25 docker compose --profile core up -d --build

# 3. Verify health
bash scripts/verify_stack.sh

# 4. Run the benchmark
docker compose --profile bench run --rm benchmark bash scripts/run_experiment.sh bs_025 1
```

**Environment variable → factor mapping:**

| Factor       | Environment Variable         | Default    |
| ------------ | ---------------------------- | ---------- |
| `batch_size` | `SEQUENCER_BATCH_MAX_SIZE`   | `100`      |
| `timeout_ms` | `SEQUENCER_BATCH_TIMEOUT_MS` | `5000`     |
| `policy`     | `SEQUENCER_POLICY`           | `FCFS`     |
| `da_mode`    | `SUBMITTER_DA_MODE`          | `offchain` |
| `prover`     | `SUBMITTER_PROOF_BACKEND`    | `groth16`  |

</details>

<details>
<summary><strong>Configurable parameters reference</strong></summary>

All experiments are configured in `benchmark-suite/config/experiments.toml`. The following parameters can be set:

- `rate_tps`: Target transaction input rate (e.g., 5, 10, 50).
- `duration_s`: Test duration in seconds.
- `warmup_s`: Warmup time before recording metrics.
- `tx_mix`: Workload complexity (`light`, `balanced`, `heavy`).
- `batch_size`: Max transactions per batch.
- `timeout_ms`: Time limit (ms) to seal a batch.
- `policy`: Sequencer scheduling policy (`FCFS`, `FeePriority`, `TimeBoost`, `FairBFT`).
- `da_mode`: Data availability mode (`calldata`, `blob`, `offchain`).
- `prover`: Prover backend (`groth16`, `plonk`).
- `eth_price_usd`: Fixed ETH/USD reference price used only for reproducible USD conversion.
- `regular_gas_price_gwei`: Fixed EIP-1559 gas price reference if a receipt gas price is unavailable.
- `blob_gas_price_gwei`: Fixed blob gas price reference for local mock-blob fee modeling.
- `seeds`: Random seeds for reproducible workloads.
- `repeats`: Number of iterations to run each experiment.

</details>

<details>
<summary><strong>Scientific cost interpretation</strong></summary>

The benchmark records deterministic contract execution gas separately from modeled price assumptions:

- Calldata runs use measured receipt `l1_gas_used` for EVM execution gas.
- Local blob runs use `cost_source=hybrid`: measured regular receipt gas plus `estimated_blob_gas_used = blob_count * 131072`.
- A blob-capable network can report `cost_source=measured` when receipt-level `blobGasUsed` and blob gas price fields are available.
- USD values use the recorded `eth_price_usd`, `regular_gas_price_gwei`, and `blob_gas_price_gwei` assumptions. Defaults are `$2,500/ETH`, `2 gwei` regular gas, and `0.001 gwei` blob gas.

For paper wording, use: “Hardhat EVM gas measurements are deterministic for contract execution. USD values are derived from fixed reference prices. Local blob DA uses modeled blob fees unless receipt-level EIP-4844 blob gas fields are available.”

</details>

### 1.4 View the Results

The metrics are saved in `benchmark-suite/metrics/run_<timestamp>/`.

**Method 1: Through University VPN**

The pipeline generates `.png` graphs and a `thesis_summary.md`. Since the VM is hosted on a university server behind a firewall, ports like `8080` might be blocked even on the VPN. The most reliable way to view the graphs is using **SSH Local Port Forwarding**.

1. **On your local machine** (your laptop/desktop), open a new terminal and create an SSH tunnel:

   ```bash
   ssh -N -L 8080:localhost:8080 cseroot@10.15.94.170
   ```

   _(Keep this terminal open as long as you want to view the files)_

2. **On the VM terminal** (where you run your project), start a simple Python web server serving your most recent run:

   ```bash
   cd benchmark-suite/metrics
   python3 -m http.server 8080 --directory "$(ls -td run_* | head -n 1)"
   ```

   If you want to view the **entire** metrics folder containing all previous runs, omit the `--directory` argument:

   ```bash
   cd benchmark-suite/metrics
   python3 -m http.server 8080
   ```

3. **On your local machine**, open a web browser and navigate to:
   ```text
   http://localhost:8080
   ```

Because of the SSH tunnel, your local browser will securely connect to the VM's server as if it were running on your own machine, completely bypassing any university firewall restrictions. Press `Ctrl+C` on the VM terminal to stop the server when done.

**Method 2: Download Results to Your Local Machine**

From a terminal on your **local machine** (your laptop/desktop):

```bash
scp -r cseroot@10.15.94.170:~/rollupx-full-zk-rollup/benchmark-suite/metrics <save_path>
```

_(Replace `<save_path>` with the directory on your computer where you want to save the files. For example: `C:\Users\Downloads`)_

---

## 2. Continuous Integration (GitHub Actions)

The containerized flow is fully automated in CI.

A GitHub Action is defined at `.github/workflows/docker-smoke.yml` which triggers on `push` and `pull_request` to the `main` branch.

**Workflow Steps:**

1. Checks out the repository.
2. Builds and starts the core stack using Docker Compose.
3. Verifies the stack health using `verify_stack.sh`.
4. Executes the smoke benchmark using `smoke_benchmark.sh`.
5. Analyzes the metrics using the `data-tools` container.
6. Automatically extracts the generated artifacts (CSVs, Plots, Markdown Reports) and uploads them as a downloadable artifact attached to the GitHub Action run.

---

## 3. Architecture Overview

```text
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

If running **Natively**, you need 5 terminal windows (or `tmux` panes) in **WSL**:

| Terminal | Component          | Listens on |
| -------- | ------------------ | ---------- |
| A        | Hardhat L1 node    | `:8545`    |
| B        | Executor + prover  | `:50051`   |
| C        | Submitter          | —          |
| D        | Sequencer          | `:3000`    |
| E        | Deploy / Benchmark | —          |

**Startup order matters:** A → (deploy) → B → C → D → (benchmark from E)

---

## 4. Native Execution: Prerequisites

Run these in **WSL** or **Linux**.

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

## 5. Native Execution: One-Time Setup

Run these once on a fresh machine / environment in **WSL**.

### 5.1 System Packages

```bash
sudo apt update
sudo apt install -y \
  build-essential clang libclang-dev cmake pkg-config \
  libssl-dev sqlite3 python3-venv nasm jq curl gettext-base
```

### 5.2 Rust Toolchains

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustup toolchain install stable
rustup toolchain install nightly-2024-08-01
```

### 5.3 Node.js & Python Dependencies

```bash
cd contracts && npm install && cd ..
cd benchmark-suite && pip install eth-account && cd ..
```

### 5.4 Build All Rust Binaries

```bash
# 1. RISC0 Prover Host
cd risc0_prover && cargo +stable build -p rollup_host && cd ..

# 2. Executor
cd executor && cargo build --ignore-rust-version && cd ..

# 3. Sequencer
cd sequencer && cargo build --release && cd ..

# 4. Submitter
cargo build --manifest-path submitter/Cargo.toml
```

---

## 6. Native Execution: Start the L1 Node (Terminal A)

```bash
cd contracts
npx hardhat node
```

Leave this terminal running.

---

## 7. Native Execution: Deploy Contracts (Terminal E)

```bash
cd contracts
npx hardhat run scripts/deploy-local.ts --network localhost
```

> **Important:** If your deployed address is different, update `sequencer/config/default.toml` and `submitter/config/local-offchain-risc0.yaml` to match.

---

## 8. Native Execution: Start the Executor (Terminal B)

Replace `<PROJECT_ROOT>` with your absolute path.

```bash
export PROJECT_ROOT="/absolute/path/to/rollupx-full-zk-rollup"

PROVER_BACKEND=risc0 \
RISC0_HOST_BIN="$PROJECT_ROOT/risc0_prover/target/debug/rollup_host" \
EXECUTOR_GRPC_ADDR=127.0.0.1:50051 \
TRACE_ROOT="$PROJECT_ROOT/executor/tmp/traces" \
STATE_DB_PATH="$PROJECT_ROOT/executor/tmp/state_db" \
"$PROJECT_ROOT/executor/target/debug/zksync_state_machine"
```

Leave this terminal running.

---

## 9. Native Execution: Start the Submitter (Terminal C)

```bash
SUBMITTER_PRIVATE_KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 \
EXECUTOR_URL=http://127.0.0.1:50051 \
DATABASE_URL=sqlite://data/submitter.db \
RUST_LOG=info \
cargo run --manifest-path submitter/Cargo.toml --bin submitter \
  -- --config submitter/config/local-offchain-risc0.yaml
```

Leave this terminal running.

---

## 10. Native Execution: Start the Sequencer (Terminal D)

**You must `cd sequencer` first.**

```bash
cd sequencer
RUST_LOG=info cargo run --release
```

Leave this terminal running.

---

## 11. Validate End-to-End Pipeline

Regardless of Docker or Native execution, you can check output artifacts.

### Check trace lifecycle

```bash
tail -n 50 executor/tmp/traces/index.jsonl
```

### Check proof artifacts

```bash
ls -lt executor/tmp/risc0/
```

### Check benchmark outputs

```bash
ls -la benchmark-suite/metrics/
```

---

## 12. Running Benchmark Experiments

The benchmark suite orchestrates end-to-end experiments to measure RollupX performance across various configurations. Below is a detailed explanation of what happens when you run a benchmark command.

### 12.1 Example Command (Smoke Test)

```bash
cd ~/rollupx-full-zk-rollup/benchmark-suite

python3 -m venv .venv
source .venv/bin/activate
pip install -U pip
pip install eth-account requests

WORKLOAD_TARGET_TXS=1 \
RATE_TPS=1 \
DURATION_S=5 \
WARMUP_S=0 \
MAX_BATCH_SIZE=1 \
TIMEOUT_MS=1000 \
DA_MODE=calldata \
POLICY=FCFS \
bash scripts/run_experiment.sh smoke_one_tx_calldata 1
```

### 12.2 What Happens During Execution

The `run_experiment.sh` script orchestrates a complete benchmark run through these phases:

#### Phase 1: Environment Setup

- Creates output directories in `metrics/<exp_id>/<run_id>/`
- Creates a shared metrics directory (`metrics/latest/`)
- Sets up RISC0 prover working directory
- Initializes a main log file at `metrics/<exp_id>/<run_id>/run.log`

#### Phase 2: Stack Initialization

- If Docker is available and `USE_DOCKER_STACK=1`, recreates the entire RollupX core stack:
  - Sequencer container (batch production)
  - Executor container (proof generation)
  - Submitter container (batch finalization)
  - Contracts deployer (L1 state management)
- Waits for sequencer readiness on port 3000
- Collects diagnostics from all containers

#### Phase 3: Workload Generation

- Runs `poisson_generator.py` with specified parameters:
  - `RATE_TPS=1`: Send 1 transaction per second
  - `DURATION_S=5`: Run for 5 seconds
  - `WARMUP_S=0`: No warm-up period
  - `WORKLOAD_TARGET_TXS=1`: Target of 1 total transaction (optional upper bound)
  - Generates transactions with specified concurrency and tx mix
  - Submits them to the sequencer via HTTP JSON-RPC

#### Phase 4: Pipeline Execution

- **Sequencer**: Receives transactions, batches them according to:
  - `MAX_BATCH_SIZE=1`: Max 1 transaction per batch
  - `TIMEOUT_MS=1000`: Seal batch after 1 second if not full
  - `POLICY=FCFS`: First-come-first-serve transaction ordering
  - `DA_MODE=calldata`: Publish batch data as calldata to L1
- **Executor**: Generates zero-knowledge proofs for each batch
- **Submitter**: Submits batches to Ethereum L1 with receipt confirmation

#### Phase 5: Metrics Collection

- Waits for all three components to stabilize (no new metrics for 3+ seconds)
- Copies component metric files from shared directory to run-specific directory:
  - `sequencer_batch_metrics.jsonl` (per-batch timing, gas usage)
  - `executor_batch_metrics.jsonl` (proof generation metrics)
  - `submitter_metrics.json` (submission confirmations, costs)

#### Phase 6: Validation

- Validates that metrics from all three components are present
- Verifies executor metrics count ≥ sequencer count
- Verifies submitter metrics count ≥ executor count
- Validates L1 bridge state advanced (if Docker available)
- Checks workload status is "pass"

### 12.3 Parameters Explained

| Parameter             | Value      | Meaning                                                        |
| --------------------- | ---------- | -------------------------------------------------------------- |
| `WORKLOAD_TARGET_TXS` | `1`        | Target total transactions to send (0 = unlimited)              |
| `RATE_TPS`            | `1`        | Poisson arrival rate: 1 transaction per second                 |
| `DURATION_S`          | `5`        | Measurement duration: 5 seconds                                |
| `WARMUP_S`            | `0`        | Warm-up period before measurement starts                       |
| `MAX_BATCH_SIZE`      | `1`        | Maximum transactions per batch                                 |
| `TIMEOUT_MS`          | `1000`     | Batch seal timeout: 1000ms (1 second)                          |
| `DA_MODE`             | `calldata` | Data availability mode: `calldata`, `blob`, or `offchain`      |
| `POLICY`              | `FCFS`     | Batch ordering policy: `FCFS`, `FeePriority`, or `BlobPacking` |

### 12.4 Output Artifacts

All outputs are saved to: `benchmark-suite/metrics/<exp_id>/<run_id>/`

#### Main Metrics Files

- **`run.log`** — Full execution transcript (all stdout/stderr)
- **`run_metadata.json`** — Experiment metadata (start/end timestamps, config snapshot)
- **`run_status.json`** — Overall run status (`pass`, `fail`, error details)
- **`workload_<exp_id>.json`** — Workload generation results (transaction arrival times, success/failure)
- **`tx_log_<run_id>.csv`** — Per-transaction log (timestamps, gas, status)

#### Component Metrics

- **`sequencer_batch_metrics.jsonl`** — One JSON object per line:
  - Batch ID, transaction count, batching duration, timestamp
  - Gas estimates for L1 verification
- **`executor_batch_metrics.jsonl`** — One JSON object per line:
  - Proof generation time, CPU/memory usage, prover backend
- **`submitter_metrics.json`** — JSON object containing:
  - All batch submission records
  - L1 receipts, gas used, costs in USD
  - EIP-4844 blob costs (if applicable)

#### Deployment & Validation

- **`l1_deployment.json`** — L1 bridge contract addresses and deployment state
- **`l1_state_validation.json`** — Hardhat verification output (on-chain state post-run)

#### Diagnostics (if Docker used)

- **`diagnostics/after_start/`** — Container logs immediately after startup
- **`diagnostics/final/`** — Container logs at end of run
  - `sequencer.log`, `executor.log`, `submitter.log`
  - `contracts-deployer.log`
  - `compose_ps.txt` — Docker Compose process status

### 12.5 What Gets Tested

This smoke test validates:

1. **Transaction Flow**: 1 TX successfully traverses sequencer → executor → submitter
2. **Batching**: Single transaction batched with 1-second timeout
3. **Proof Generation**: ZK proof successfully generated (assumes Docker with RISC0)
4. **L1 Settlement**: Batch successfully submitted to Ethereum L1
5. **Data Availability**: Calldata mode correctly encodes batch on-chain
6. **Cost Accounting**: Gas and USD costs correctly calculated

### 12.6 Pre-requisites

- Docker and Docker Compose (for containerized execution)
- Python 3.8+ with virtualenv
- `eth-account` and `requests` Python packages
- Active Sequencer (if not using Docker)

### 12.7 Longer Benchmark Runs

For more comprehensive benchmarking, use preset phases:

```bash
# 1 repeat, 30s measured, batch-size sweep (fastest)
bash scripts/run_matrix.sh --phase smoke

# 3 repeats, 90s measured (feasibility check)
bash scripts/run_matrix.sh --phase feasibility-lite

# 5 repeats, 120s measured (production-ready)
bash scripts/run_matrix.sh --phase feasibility

# 30 repeats, 120s measured (statistical quality)
bash scripts/run_matrix.sh --phase model-quality
```

Each phase sweeps multiple configurations (DA modes, batch policies, sizes) and generates aggregated results in `metrics/`.

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
```

### Benchmark Matrix — `benchmark-suite/config/experiments.toml`

Defines parameters for matrix runs like DA mode, provers, rates.

---

## 14. Troubleshooting

| Symptom                                                               | Cause                                   | Fix                                                                  |
| --------------------------------------------------------------------- | --------------------------------------- | -------------------------------------------------------------------- |
| `[WARN] Sequencer binary not found`                                   | Expected in Docker — no local binary    | Harmless; the script falls back to HTTP (`http://sequencer:3000`)    |
| Infrastructure experiments (batch_size, policy, da_mode) don't change | Matrix script can't restart containers  | Set env vars on `docker compose up` and restart the stack per config |
| `No such file or directory` from sequencer                            | Not running from `sequencer/` directory | `cd sequencer` before `cargo run`                                    |
| `UNIQUE constraint failed: batches.batch_id`                          | Stale DB from previous run              | Delete `sequencer/sequencer.db` or use `reset_state.sh`              |
| Submitter can't reach executor gRPC                                   | Executor not running on `:50051`        | Start executor first; verify `EXECUTOR_URL` matches                  |
| `envsubst: command not found`                                         | Missing `gettext-base`                  | `sudo apt install gettext-base`                                      |
| `wait_for_sequencer.sh` fails on `/health`                            | Probes legacy endpoint                  | Keep sequencer manual; use JSON-RPC probe on `/`                     |

### Clean Restart (Native)

```bash
cd benchmark-suite
bash scripts/reset_state.sh manual_clean
```
