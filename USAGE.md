# RollupX End-to-End Usage Guide

Welcome to the RollupX system. This guide provides comprehensive, step-by-step instructions on how to operate the entire RollupX stack, run the benchmark workloads, and use the data-tools pipeline for analysis and generating reports.

---

## Table of Contents
1. [Starting the System Components](#1-starting-the-system-components)
2. [Generating the Workload](#2-generating-the-workload)
3. [Running Automated Benchmarks](#3-running-automated-benchmarks)
4. [Using Data Tools & Generating Reports](#4-using-data-tools--generating-reports)

---

## 1. Starting the System Components

RollupX relies on several microservices running in tandem: an L1 node, a Sequencer, an Executor, and a Submitter.

### Recommended: Docker Compose
You can run the entire system via Docker Compose:
```bash
docker compose up --build
```
This handles starting the L1 node, deploying contracts, and launching the sequencer, executor, and submitter with their configured YAML files.

### Manual Startup
Here’s the manual way to spin them up.

> **⚠️ Known Issue (Local Executor Compilation):**
> Local cargo compilation of the `executor` is currently blocked by a pre-existing upstream toolchain inconsistency. The executor's `rust-toolchain.toml` is pinned to `nightly-2024-08-01`, but its committed `Cargo.lock` resolves to newer dependencies requiring Rust 2024 features.
>
> Until this lockfile/toolchain incompatibility is resolved in a separate task, the recommended way to run the executor locally is strictly via Docker Compose (`docker compose up --build`).

### Step 1: Start the Local Ethereum (L1) Node
We use Hardhat to simulate an Ethereum L1 locally.
```bash
cd contracts
npm install
npx hardhat node
```
This starts the local node on `http://127.0.0.1:8545`.

### Step 2: Deploy the L1 Smart Contracts
In a new terminal window, deploy the ZKRollupBridge to your local network:
```bash
cd contracts
npx hardhat run scripts/deploy-local.ts --network localhost
```
*Note down the deployed Bridge address.*

### Step 3: Start the Executor (gRPC Relay Mode)
The executor processes batches from the Sequencer. In the default configuration (`grpc` mode), it acts as a pass-through layer that bridges directly to the Submitter.
```bash
cd executor
export EXECUTOR_CONFIG="executor.yaml"
python3 run_executor.py ./target/release/zksync_state_machine
```

### Step 4: Start the Submitter
The Submitter pulls completed execution batches and lands the mocked ZK proofs on L1.
```bash
cd submitter
export SUBMITTER_PRIVATE_KEY="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
cargo run --release --bin submitter -- --config submitter.yaml
```

### Step 5: Start the Sequencer
Finally, run the Sequencer which handles incoming REST API transactions, batches them, and fires them to the Executor over gRPC.
```bash
cd sequencer
# Ensure the config matches your DB paths and Bridge address in `sequencer/sequencer.yaml`.
# Then run:
export SEQUENCER_CONFIG="sequencer.yaml"
cargo run --release
```

---

## 2. Generating the Workload

Once the entire stack is running (Hardhat, Sequencer, Executor, Submitter), you can start blasting it with synthetic traffic.

We provide a Python-based Poisson process workload generator that synthesizes realistic transaction arrival times.

### Setup Workload Generator Dependencies
Make sure you have the required Python modules:
```bash
pip install eth-account
```

### Running the Generator Manually
```bash
cd benchmark-suite

# Set the metrics output directory
export METRICS_ROOT="metrics/my_manual_exp"

# Run a test for 30 seconds at 5 transactions/second
python3 workload/poisson_generator.py \
    --experiment_id "exp_manual_1" \
    --run_id "run_01" \
    --rate 5.0 \
    --duration 30 \
    --warmup 5 \
    --seed 42 \
    --tx_mix "balanced" \
    --host "localhost" \
    --port 3000
```
This blasts transactions to the Sequencer at `http://localhost:3000`. You will see `workload_exp_manual_1.json` and a `.csv` log created in the `METRICS_ROOT` folder.

---

## 3. Running Automated Benchmarks

Instead of starting all components by hand, we provide orchestration scripts to automate configuration matrix testing. These scripts manage the lifecycle of the Sequencer for you.

### Prerequisite
Ensure `metrics/` directory exists and that your L1 Node and Executor/Submitter are running in the background as defined in Section 1.

### Running a Single Experiment
You can execute an end-to-end experiment using `run_experiment.sh`. This script will launch its own sequencer instance, run the workload, wait for the submitter to flush, and capture all telemetry.

```bash
cd benchmark-suite

# Syntax: bash scripts/run_experiment.sh <EXPERIMENT_NAME> <REPEAT_INDEX>
# Make sure to provide standard ENV variable overrides if you want to test specific parameters.

export MAX_BATCH_SIZE=100
export TIMEOUT_MS=2000
export RATE_TPS=15
export DURATION_S=60

bash scripts/run_experiment.sh test_batching 1
```

**What happens?**
1. It creates an isolated directory: `benchmark-suite/metrics/test_batching/test_batching_r01/`
2. It dumps the current environment variables into `run_metadata.json`.
3. It creates a custom `seq_config.toml` and starts the Sequencer.
4. It blasts traffic via the python `poisson_generator.py`.
5. It monitors the `submitter_metrics.json` to ensure everything settled on L1.
6. It cleanly shuts down the Sequencer.

---

## 4. Using Data Tools & Generating Reports

After running your workloads (either manually or via the benchmark script), telemetry is dumped across various JSON and CSV files in your `METRICS_ROOT`. 

The `data-tools` pipeline merges these files, calculates standard statistics (P50, P95, Jain's fairness index, gas saved), and generates plotting figures for academic analysis.

### Step 1: Install Data-Tools Dependencies
```bash
pip install pandas matplotlib
```

### Step 2: Run the Orchestration Pipeline
We provide a single `run_pipeline.sh` script located in the repository root that aggregates the metrics and runs all the plotting files sequentially.

```bash
# Make sure it is executable
chmod +x run_pipeline.sh

# Run the entire pipeline pointing to your root metrics folder
export METRICS_ROOT="benchmark-suite/metrics"
./run_pipeline.sh
```

### Manual Data-Tools Steps (Optional)

If you prefer to run them individually:

**1. Aggregation**
Merge all JSON files into a single Pandas dataframe:
```bash
python3 data-tools/aggregate.py --metrics_root benchmark-suite/metrics --output benchmark-suite/metrics/all_results.csv --include_failed
```

**2. Statistics computation**
Compute the mean, standard deviation, percentiles, and compare against baselines:
```bash
python3 data-tools/stats.py --input benchmark-suite/metrics/all_results.csv --output benchmark-suite/metrics/stats_summary.csv
```

**3. Plotting**
Generate throughput bars, latency CDFs, and Pareto frontiers (saved to `benchmark-suite/metrics/figures/`):
```bash
python3 data-tools/plots/pareto_frontier.py --input benchmark-suite/metrics/all_results.csv
python3 data-tools/plots/throughput_bar.py --input benchmark-suite/metrics/all_results.csv
python3 data-tools/plots/latency_cdf.py --metrics_root benchmark-suite/metrics
# ... see run_pipeline.sh for all available plots
```

**4. Report Generation**
Generate a clean Markdown report detailing the findings:
```bash
python3 data-tools/report/generate_md.py --input benchmark-suite/metrics/all_results.csv --stats benchmark-suite/metrics/stats_summary.csv --output benchmark-suite/metrics/thesis_summary.md
```
