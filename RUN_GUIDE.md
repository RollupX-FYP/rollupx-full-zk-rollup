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

### 1.3 Configure and Run Benchmarks
You can trigger a pre-configured smoke benchmark via the containerized runner (requires **WSL** or **Git Bash**):
```bash
docker compose --profile bench build benchmark --no-cache # Run this once or if Dockerfile changes
bash scripts/smoke_benchmark.sh
```

**Configuring Benchmarks:**
The benchmarks are controlled via Environment Variables passed to the benchmark container. If you open `scripts/smoke_benchmark.sh`, you can tweak parameters like:
- `RATE_TPS=10`: Increase or decrease load.
- `DURATION_S=30`: Run the test for a longer period.
- `TX_MIX=heavy`: Change the workload type (valid: `balanced`, `light`, `heavy`, `custom`).
- `DA_MODE=blob`: Test different Data Availability modes.
- `PROVER=groth16`: Change the prover backend.

Or, you can run specific workload scripts locally against the containerized sequencer exposed on port `3000`.

### 1.4 Generate and View Analytics Reports
After the benchmarks finish, extract metrics and generate CSV summaries, markdown reports, and plots using the `data-tools` profile:
```bash
docker compose --profile report build data-tools --no-cache # Run this once or if data-tools/Dockerfile changes
docker compose --profile report run --rm data-tools
```

### 1.5 View the Results
**Method 1: Download Results to Your Local Machine**

If you prefer to download the raw files and graphs to view them natively on your PC, you can securely copy the entire folder from the VM using `scp`:

1. Open a terminal on your local machine (your laptop/desktop).
2. Run the `scp` command to download the folder:
   ```bash
   scp -r cseroot@10.15.94.170:/home/cseroot/rollupx-full-zk-rollup/benchmark-suite/metrics <save_path>
   ```
   *(Replace `<save_path>` with the directory on your computer where you want to save the files. For example: `C:\Users\Lishan\Downloads`)*

**Method 2: Through University VPN**

The pipeline generates `.png` graphs and a `thesis_summary.md` inside the `metrics/` folder. Since the VM is hosted on a university server behind a firewall, ports like `8080` might be blocked even on the VPN. The most reliable way to view the graphs is using **SSH Local Port Forwarding**.

1. **On your local machine** (your laptop/desktop), open a new terminal and create an SSH tunnel:
   ```bash
   ssh -N -L 8080:localhost:8080 cseroot@10.15.94.170
   ```
   *(Keep this terminal open as long as you want to view the files)*

2. **On the VM terminal** (where you run your project), start a simple Python web server:
   ```bash
   cd rollupx-full-zk-rollup
   python3 -m http.server 8080 --directory metrics/
   ```

3. **On your local machine**, open a web browser and navigate to:
   ```text
   http://localhost:8080
   ```

Because of the SSH tunnel, your local browser will securely connect to the VM's server as if it were running on your own machine, completely bypassing any university firewall restrictions. Press `Ctrl+C` on the VM terminal to stop the server when done.

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
