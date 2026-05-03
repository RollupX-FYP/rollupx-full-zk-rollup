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
12. [Configuration Reference](#12-configuration-reference)
13. [Troubleshooting](#13-troubleshooting)

---

## 0. Server Access & Preparation

1) **Check the Connection After Setting Up the VPN**
   
   From your local terminal:
   ```bash
   ping 10.15.94.170
   ```

   What successful output looks like:

   ```bash
   Reply from 10.15.94.170: bytes=32 time=15ms TTL=64
   ```

2) **Connect to Server**
   
   From your local terminal:
   ```bash
   ssh cseroot@10.15.94.170
   ```
   Say yes to “Are you sure you want to continue connecting (yes/no/[fingerprint])?” and enter the password.

3) **Start Docker**
   ```bash
   sudo systemctl start docker
   ```
   Verify:
   ```bash
   systemctl status docker
   ```

4) **Navigate to Project Directory**
   ```bash
   cd rollupx-full-zk-rollup
   ```

5) **Switch Branch (Optional)**

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

### 1.3 Run Benchmarks

All commands below are run **from the project root** (`rollupx-full-zk-rollup/`) on the **host machine** (normal terminal) (not inside a container).

**Step 1 — Build the benchmark image** (once, or after Dockerfile changes):
```bash
docker compose --profile bench build benchmark --no-cache
```

**Step 2 — Run workload experiments** (rate, tx mix — no stack restart needed):
```bash
docker compose --profile core --profile bench run --rm benchmark bash scripts/run_matrix.sh
```

**Step 3 — Run infrastructure experiments** (batch size, timeout, policy, DA mode, prover):
```bash
bash scripts/run_infra_matrix.sh
```

> [!NOTE]
> Step 2 runs **inside** the benchmark container and sweeps workload factors automatically.
> Step 3 runs **on the host** and automatically restarts the Docker Compose stack with different configurations for each infrastructure experiment.
> If you see `[WARN] Sequencer binary not found` during Step 2, this is **expected and harmless** in Docker.

---

<details>
<summary><strong>Why two separate commands?</strong></summary>

The benchmark container cannot restart the external sequencer or submitter containers. This means:

| Category | Factors | How they run |
|---|---|---|
| ✅ **Workload** | `rate_tps`, `tx_mix`, `duration_s`, `warmup_s` | Controlled by the Python workload generator **inside** the benchmark container → `run_matrix.sh` |
| ⚙️ **Infrastructure** | `batch_size`, `timeout_ms`, `policy`, `da_mode`, `prover` | Require restarting the core stack with new env vars → `run_infra_matrix.sh` |

</details>

<details>
<summary><strong>Filtering to specific factors</strong></summary>

**Workload factors** (run inside benchmark container):
```bash
docker compose --profile core --profile bench run --rm benchmark bash scripts/run_matrix.sh --filter rate
docker compose --profile core --profile bench run --rm benchmark bash scripts/run_matrix.sh --filter tx_mix
```

**Infrastructure factors** (run on host):
```bash
bash scripts/run_infra_matrix.sh --filter batch_size
bash scripts/run_infra_matrix.sh --filter policy
bash scripts/run_infra_matrix.sh --filter da_mode

# Preview what will run without executing
bash scripts/run_infra_matrix.sh --dry-run

# Skip the baseline (useful when resuming)
bash scripts/run_infra_matrix.sh --filter da_mode --skip-baseline
```

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

| Factor | Environment Variable | Default |
|---|---|---|
| `batch_size` | `SEQUENCER_BATCH_MAX_SIZE` | `100` |
| `timeout_ms` | `SEQUENCER_BATCH_TIMEOUT_MS` | `5000` |
| `policy` | `SEQUENCER_POLICY` | `FCFS` |
| `da_mode` | `SUBMITTER_DA_MODE` | `offchain` |
| `prover` | `SUBMITTER_PROOF_BACKEND` | `groth16` |

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
- `seeds`: Random seeds for reproducible workloads.
- `repeats`: Number of iterations to run each experiment.

</details>

**Output:** The raw metrics (per-run) are stored inside the Docker volume at `metrics/<experiment_id>/<run_id>/`:
- `workload_<exp_id>.json`: Details of the generated workload.
- `run_metadata.json`: Start/end timestamps, configuration snapshots.
- `tx_log_<run_id>.csv`: Transaction-level metrics (submission time, batching time, proof time, L1 finalization time).
- `submitter_metrics.json`: Submitter lifecycle and cost tracking.
- `run_status.json`: Execution status.

### 1.4 Generate Analytics Reports
After the benchmarks finish, the raw per-run metrics (JSON + CSV) are stored in the Docker volume. To generate the aggregated analysis, plots, and markdown report, run the data-tools pipeline:
```bash
docker compose --profile report build data-tools --no-cache # Run this once or if data-tools/Dockerfile changes
docker compose --profile report run --rm data-tools
```

When the `data-tools` pipeline is run, it aggregates the raw metrics across all runs and generates (inside the Docker volume):
- `all_results.csv`: Combined flat list of all experiment results.
- `stats_summary.csv`: Statistical summaries across repeats (mean, standard deviation, confidence intervals).
- `figures/`: Visual plots including:
  - Throughput bar charts
  - Latency CDFs (Cumulative Distribution Functions)
  - Pareto frontiers (trade-offs between factors)
  - Fairness metrics
  - Cost heatmaps and sensitivity analyses
- `thesis_summary.md`: An auto-generated markdown report summarizing the findings.

### 1.5 View the Results

Since metrics are stored inside a Docker volume, first extract them to the host filesystem:
```bash
mkdir -p ~/rollupx-metrics
docker compose --profile report run --rm -v ~/rollupx-metrics:/out data-tools bash -c "cp -r /var/lib/rollupx/metrics/. /out/"
```

**Method 1: Through University VPN**

The pipeline generates `.png` graphs and a `thesis_summary.md` inside the metrics folder. Since the VM is hosted on a university server behind a firewall, ports like `8080` might be blocked even on the VPN. The most reliable way to view the graphs is using **SSH Local Port Forwarding**.

1. **On your local machine** (your laptop/desktop), open a new terminal and create an SSH tunnel:
   ```bash
   ssh -N -L 8080:localhost:8080 cseroot@10.15.94.170
   ```
   *(Keep this terminal open as long as you want to view the files)*

2. **On the VM terminal** (where you run your project), start a simple Python web server:
   ```bash
   python3 -m http.server 8080 --directory ~/rollupx-metrics/
   ```

3. **On your local machine**, open a web browser and navigate to:
   ```text
   http://localhost:8080
   ```

Because of the SSH tunnel, your local browser will securely connect to the VM's server as if it were running on your own machine, completely bypassing any university firewall restrictions. Press `Ctrl+C` on the VM terminal to stop the server when done.

**Method 2: Download Results to Your Local Machine**

From a terminal on your **local machine** (your laptop/desktop):
```bash
scp -r cseroot@10.15.94.170:~/rollupx-metrics <save_path>
```
*(Replace `<save_path>` with the directory on your computer where you want to save the files. For example: `C:\Users\Lishan\Downloads`)*

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

## 12. Configuration Reference

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

## 13. Troubleshooting

| Symptom                                                                   | Cause                                   | Fix                                                                     |
| ------------------------------------------------------------------------- | --------------------------------------- | ----------------------------------------------------------------------- |
| `[WARN] Sequencer binary not found`                                       | Expected in Docker — no local binary    | Harmless; the script falls back to HTTP (`http://sequencer:3000`)        |
| Infrastructure experiments (batch_size, policy, da_mode) don't change     | Matrix script can't restart containers  | Set env vars on `docker compose up` and restart the stack per config     |
| `No such file or directory` from sequencer                                | Not running from `sequencer/` directory | `cd sequencer` before `cargo run`                                       |
| `UNIQUE constraint failed: batches.batch_id`                              | Stale DB from previous run              | Delete `sequencer/sequencer.db` or use `reset_state.sh`                 |
| Submitter can't reach executor gRPC                                       | Executor not running on `:50051`        | Start executor first; verify `EXECUTOR_URL` matches                     |
| `envsubst: command not found`                                             | Missing `gettext-base`                  | `sudo apt install gettext-base`                                         |
| `wait_for_sequencer.sh` fails on `/health`                                | Probes legacy endpoint                  | Keep sequencer manual; use JSON-RPC probe on `/`                        |

### Clean Restart (Native)
```bash
cd benchmark-suite
bash scripts/reset_state.sh manual_clean
```
