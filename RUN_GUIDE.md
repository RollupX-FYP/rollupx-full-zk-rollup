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

1) **Connect to Server**
   From your local terminal:
   ```bash
   ssh cseroot@10.15.94.170
   ```
   Say yes to “Are you sure you want to continue connecting (yes/no/[fingerprint])?” and enter the password.

2) **Start Docker**
   ```bash
   sudo systemctl start docker
   sudo systemctl enable docker
   ```
   Verify:
   ```bash
   systemctl status docker
   ```

3) **Navigate to Project Directory**
   ```bash
   cd rollupx-full-zk-rollup
   ```

4) **Switch Branch (Optional)**

   ```bash
   git checkout <branch-name>
   ```
   If remote only:
   ```bash
   git checkout -b <branch-name> origin/<branch-name>
   ```

## 1. Containerized Execution (Recommended)

The entire RollupX stack is fully containerized. You do not need to install Rust, Node, or set up multiple terminals. You can run these primary docker commands from **PowerShell** or **WSL**.

### 1.1 Start the Core Stack
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
You can trigger a pre-configured smoke benchmark via the containerized runner (requires **WSL** or **Git Bash**):
```bash
bash scripts/smoke_benchmark.sh
```
Or, you can run specific workload scripts locally against the containerized sequencer exposed on port `3000`.

### 1.4 Generate Data Tools / Analytics Report
Extract metrics and generate CSV summaries, markdown reports, and plots using the `data-tools` profile:
```bash
docker compose --profile report run --rm data-tools
```

### 1.5 Cleanup
To tear down the containers and reset volumes:
```bash
docker compose down -v --remove-orphans
```
*(Note: Remove named volumes with `docker compose down -v` only if you don't wish to keep existing metrics or database state.)*

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
