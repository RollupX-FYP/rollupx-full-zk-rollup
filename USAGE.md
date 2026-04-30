# RollupX End-to-End Usage Guide

This guide is the current, tested path for running RollupX locally with the real executor -> trace -> RISC0 proving -> submitter flow.

## 1. New Environment Setup

### System packages (Ubuntu/Debian)

```bash
sudo apt update
sudo apt install -y build-essential clang libclang-dev cmake pkg-config libssl-dev sqlite3 python3-venv
```

### Rust toolchains

```bash
rustup toolchain install nightly-2025-03-19
rustup toolchain install stable
```

### Node packages for contracts

```bash
cd contracts
npm install
cd ..
```

### Python venv for workload generator

```bash
cd benchmark-suite
python3 -m venv .venv
. .venv/bin/activate
pip install eth-account
cd ..
```

## 2. Start Local L1 and Deploy Contracts

### Terminal A: Hardhat node

```bash
cd contracts
npx hardhat node
```

L1 RPC: `http://127.0.0.1:8545`

### Terminal B: deploy bridge + verifier

```bash
cd contracts
npx hardhat run scripts/deploy-local.ts --network localhost
```

Expected key output includes:
- `ZKRollupBridge: 0x...`

### Optional: set OffChain DA provider for local submitter config

```bash
cd contracts
npx hardhat run scripts/tmp_setup_da.js --network localhost
```

## 3. Start Executor (Real RISC0 Path)

Build prover host once:

```bash
cd risc0_prover
cargo +stable build -p rollup_host
cd ..
```

Start executor:

```bash
PROVER_BACKEND=risc0 \
RISC0_HOST_BIN=/absolute/path/to/rollupx-full-zk-rollup/risc0_prover/target/debug/rollup_host \
EXECUTOR_GRPC_ADDR=127.0.0.1:50051 \
TRACE_ROOT=/absolute/path/to/rollupx-full-zk-rollup/executor/tmp/traces \
STATE_DB_PATH=/absolute/path/to/rollupx-full-zk-rollup/executor/tmp/state_db \
/absolute/path/to/rollupx-full-zk-rollup/executor/target/debug/zksync_state_machine
```

Notes:
- `RISC0_GUEST_ELF` is optional in this setup because `rollup_host` embeds a guest ELF.
- If you prefer cargo run:
  `cargo +nightly-2025-03-19 run --manifest-path executor/Cargo.toml -p zksync_state_machine --ignore-rust-version`

## 4. Start Submitter

```bash
SUBMITTER_PRIVATE_KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 \
EXECUTOR_URL=http://127.0.0.1:50051 \
DATABASE_URL=sqlite://data/submitter.db \
RUST_LOG=info \
cargo run --manifest-path submitter/Cargo.toml --bin submitter -- --config submitter/config/local-offchain-risc0.yaml
```

## 5. Start Sequencer

Important: run from `sequencer/` so `config/default.toml` resolves correctly.

```bash
cd sequencer
RUST_LOG=info cargo run --release
```

Sequencer JSON-RPC endpoint: `http://127.0.0.1:3000`

## 6. Run E2E Workload

```bash
cd benchmark-suite
. .venv/bin/activate
export METRICS_ROOT=metrics/e2e/e2e_rXX

python workload/poisson_generator.py \
  --experiment_id e2e_real_risc0 \
  --run_id e2e_rXX \
  --rate 3 \
  --duration 10 \
  --warmup 2 \
  --seed 19 \
  --tx_mix balanced \
  --host localhost \
  --port 3000
```

Expected outputs:
- `benchmark-suite/metrics/e2e/e2e_rXX/workload_e2e_real_risc0.json`
- `benchmark-suite/metrics/e2e/e2e_rXX/tx_log_e2e_rXX.csv`
- `benchmark-suite/metrics/e2e/e2e_rXX/run_status.json`

## 7. Validate End-to-End Pipeline

### Check service ports

```bash
ss -ltnp | rg '8545|50051|3000|9000'
```

### Check trace lifecycle

```bash
tail -n 50 executor/tmp/traces/index.jsonl
```

For completed batches you should see statuses progressing through:
- `generated`
- `persisted`
- `proved`
- `published`

### Check proof artifacts

```bash
ls -lt executor/tmp/risc0
```

Look for:
- `trace_<batch>.json`
- `proof_<batch>.bin`
- `journal_<batch>.bin`
- `proof_meta_<batch>.json`

## 8. Benchmark + Data Tools

Single experiment script:

```bash
cd benchmark-suite
bash scripts/run_experiment.sh test_batching 1
```

Aggregate/report pipeline:

```bash
export METRICS_ROOT=benchmark-suite/metrics
./run_pipeline.sh
```

## 9. Troubleshooting

- `No such file or directory (os error 2)` from sequencer:
  usually means running from wrong directory; start sequencer inside `sequencer/`.

- `UNIQUE constraint failed: batches.batch_id`:
  fixed by sequencer batch-ID recovery from registry (`MAX(batch_id)+1`).
  If you are on an older commit, update to latest.

- Workload dependency error (`pip install eth-account`):
  use `benchmark-suite/.venv` and run generator from activated venv.

- Missing executor gRPC in submitter:
  ensure executor is up on `127.0.0.1:50051` and `EXECUTOR_URL` matches.
