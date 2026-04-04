# RollupX — Benchmark Suite

Orchestrates controlled experiments for the RollupX ZK-Rollup prototype.  
See `PLAN.md` for the full research methodology.

## Quick start

```bash
# 1. Install Python deps
pip install eth-account

# 2. Smoke test (workload generator only, no sequencer needed)
METRICS_ROOT=metrics/smoke \
python workload/poisson_generator.py \
  --experiment_id smoke --rate 5 --duration 30 \
  --tx_mix balanced --seed 42 --warmup 0

# 3. Single full experiment (sequencer must be running)
bash scripts/run_experiment.sh baseline 1

# 4. Full matrix (dry run first)
bash scripts/run_matrix.sh --dry-run
bash scripts/run_matrix.sh

# 5. Filter to one factor
bash scripts/run_matrix.sh --filter batch_size
```

## Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `METRICS_ROOT` | `metrics` | Output directory for JSON/CSV |
| `SEQ_HOST` | `localhost` | Sequencer host |
| `SEQ_PORT` | `3000` | Sequencer port |
| `SEQUENCER_BIN` | `./target/release/rollup_sequencer` | Path to sequencer binary |
| `L1_RPC_URL` | (required) | Sepolia RPC endpoint |
| `BRIDGE_ADDRESS` | (required) | Deployed RollupBridge address |

## Output layout

```
metrics/
└── <exp_id>/
    └── <run_id>/
        ├── workload_<exp_id>.json    # generator metrics
        ├── executor_<exp_id>.json    # executor metrics (written by executor)
        ├── submitter_metrics.json    # JSONL, one entry per batch
        ├── tx_log_<run_id>.csv       # per-tx latency log
        ├── run_metadata.json         # hw/sw/config snapshot
        ├── run_status.json           # pass / fail / partial
        └── run.log                   # full stdout/stderr
```
