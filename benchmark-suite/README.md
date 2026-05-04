# RollupX Benchmark Suite

This benchmark suite runs metrics-only experiments for the RollupX batch-size
feasibility study. It is meant to show how batch inputs such as transaction
count, serialized batch bytes, gas proxies, execution/proof timing, and L1
submission metrics change as batch size changes.

The default path is Docker-based. Each experiment run recreates the RollupX
`core` stack with that run's config and writes metrics into that run's output
folder.

## Quick Start

Run from the repository root:

```bash
cd ~/rollupx-full-zk-rollup

# Show available CLI options.
bash benchmark-suite/scripts/run_matrix.sh --help

# Preview the smoke run without starting Docker or sending transactions.
bash benchmark-suite/scripts/run_matrix.sh --phase smoke --dry-run

# Run the smoke batch-size experiment.
bash benchmark-suite/scripts/run_matrix.sh --phase smoke
```

You can also run from inside `benchmark-suite`:

```bash
cd ~/rollupx-full-zk-rollup/benchmark-suite
bash scripts/run_matrix.sh --phase smoke
```

## CLI

Main entrypoint:

```bash
bash benchmark-suite/scripts/run_matrix.sh [options]
```

Common presets:

```bash
# 1 repeat, 30s measured run, 5s warmup, batch-size sweep.
bash benchmark-suite/scripts/run_matrix.sh --phase smoke

# 5 repeats, 120s measured run, 15s warmup, batch-size sweep.
bash benchmark-suite/scripts/run_matrix.sh --phase feasibility

# 30 repeats, 120s measured run, 15s warmup, batch-size sweep.
bash benchmark-suite/scripts/run_matrix.sh --phase model-quality
```

Useful options:

```bash
# Print selected experiments without running them.
bash benchmark-suite/scripts/run_matrix.sh --phase smoke --list

# Print exact commands/env without running Docker or workloads.
bash benchmark-suite/scripts/run_matrix.sh --phase smoke --dry-run

# Run one experiment only.
bash benchmark-suite/scripts/run_matrix.sh \
  --only exp_002_batch_size_bs010_calldata_balanced_10tps \
  --repeats 1 \
  --duration 20 \
  --warmup 5

# Run all batch-size experiments with custom timing.
bash benchmark-suite/scripts/run_matrix.sh \
  --filter batch_size \
  --repeats 1 \
  --duration 20 \
  --warmup 5

# Skip docker compose build during each stack recreation.
bash benchmark-suite/scripts/run_matrix.sh --phase smoke --no-build

# Force local non-Docker mode.
bash benchmark-suite/scripts/run_matrix.sh --phase smoke --no-docker
```

Compatibility wrappers:

```bash
bash benchmark-suite/scripts/run_smoke_batchsize.sh
bash benchmark-suite/scripts/run_feasibility_batchsize.sh
bash benchmark-suite/scripts/run_model_quality_batchsize.sh
```

The wrappers accept the same extra options:

```bash
bash benchmark-suite/scripts/run_smoke_batchsize.sh --dry-run
bash benchmark-suite/scripts/run_feasibility_batchsize.sh --list
bash benchmark-suite/scripts/run_model_quality_batchsize.sh --no-build
```

## Experiment Naming

Experiments are numbered so folders sort in execution order:

```text
exp_000_baseline_bs050_calldata_balanced_10tps
exp_001_batch_size_bs001_calldata_balanced_10tps
exp_002_batch_size_bs010_calldata_balanced_10tps
exp_003_batch_size_bs025_calldata_balanced_10tps
exp_004_batch_size_bs050_calldata_balanced_10tps
exp_005_batch_size_bs100_calldata_balanced_10tps
exp_006_batch_size_bs250_calldata_balanced_10tps
exp_007_batch_size_bs500_calldata_balanced_10tps
exp_008_batch_size_bs1000_calldata_balanced_10tps
```

Run folders append the repeat number:

```text
metrics/exp_002_batch_size_bs010_calldata_balanced_10tps/
  exp_002_batch_size_bs010_calldata_balanced_10tps_r01/
```

## Docker Behavior

Docker is the default.

For each run, `scripts/run_experiment.sh`:

1. Creates the run output folder.
2. Recreates the Docker `core` stack.
3. Passes the run config into Docker Compose:
   - `SEQUENCER_BATCH_MAX_SIZE`
   - `SEQUENCER_BATCH_TIMEOUT_MS`
   - `SEQUENCER_BATCH_MIN_SIZE`
   - `SEQUENCER_POLICY`
   - `SUBMITTER_DA_MODE`
   - `SUBMITTER_PROOF_BACKEND`
   - `EXPERIMENT_ID`
   - `EXPERIMENT_NAME`
   - `METRICS_DIR`
4. Waits for the sequencer.
5. Runs the workload generator.
6. Waits for component metric files to stop growing.

This is important: the batch-size sweep only changes the real running sequencer
when the stack is recreated with the new env.

## Output Layout

Each completed run writes into:

```text
benchmark-suite/metrics/<experiment_id>/<run_id>/
```

Expected files:

```text
run.log
run_metadata.json
run_status.json
workload_<experiment_id>.json
tx_log_<run_id>.csv
sequencer_batch_metrics.jsonl
executor_batch_metrics.jsonl
submitter_metrics.json
diagnostics/
```

The workload files come from the Python generator. The component JSONL files
come from the running services:

```text
sequencer_batch_metrics.jsonl   # sealed batch metrics
executor_batch_metrics.jsonl    # execution/proof/state-diff metrics
submitter_metrics.json          # DA/L1 submission metrics, JSONL rows
```

Each Docker run also writes diagnostics:

```text
diagnostics/after_start/compose_ps.txt
diagnostics/final/sequencer.log
diagnostics/final/executor.log
diagnostics/final/submitter.log
diagnostics/final/*_metrics_env.txt
```

Use these files first when a component metric is missing. They capture container
health, recent service logs, and the `METRICS_ROOT`/`EXPERIMENT_ID` seen by each
container.

## Checking Metrics

After a run:

```bash
find benchmark-suite/metrics -type f \
  \( -name "sequencer_batch_metrics.jsonl" \
     -o -name "executor_batch_metrics.jsonl" \
     -o -name "submitter_metrics.json" \)
```

Inspect one run:

```bash
RUN_DIR=benchmark-suite/metrics/exp_002_batch_size_bs010_calldata_balanced_10tps/exp_002_batch_size_bs010_calldata_balanced_10tps_r01

ls -lah "$RUN_DIR"
wc -l "$RUN_DIR"/*.jsonl "$RUN_DIR"/submitter_metrics.json 2>/dev/null
head -n 3 "$RUN_DIR/sequencer_batch_metrics.jsonl"
```

## Aggregation And Plots

After collecting runs:

```bash
python3 data-tools/aggregate.py
python3 data-tools/plots/batch_feasibility.py
```

Primary aggregate outputs:

```text
data-tools/out/all_results.csv
data-tools/out/all_batch_results.csv
```

Use `all_batch_results.csv` for feasibility analysis. Plot variables against
actual `tx_count`, not only configured batch size.

## Troubleshooting

If only `sequencer_batch_metrics.jsonl` appears:

```bash
docker compose --profile core logs --tail=300 executor submitter
docker exec rollupx-full-zk-rollup-executor-1 sh -lc 'echo $METRICS_ROOT; ls -lah /var/lib/rollupx/metrics'
docker exec rollupx-full-zk-rollup-submitter-1 sh -lc 'echo $METRICS_ROOT; ls -lah /var/lib/rollupx/metrics'
```

If Docker uses stale images, run with build enabled:

```bash
bash benchmark-suite/scripts/run_matrix.sh --phase smoke
```

or force a clean rebuild manually:

```bash
docker compose --profile core down -v
docker compose --profile core build --no-cache sequencer executor submitter
```

If executor logs show `Elf parse error: Could not read bytes in range [0x0, 0x10)`,
the executor image is using a bad RISC0 host build. Rebuild the executor image
from current sources:

```bash
docker compose --profile core down -v
docker compose --profile core build --no-cache executor
docker compose --profile core up -d
docker compose --profile core logs --tail=50 executor
```

The executor startup log should print `PROVER_BACKEND=risc0` and a non-empty
`/usr/local/bin/rollup_host`.

If Python dependency errors mention `eth-account`:

```bash
cd benchmark-suite
python3 -m venv .venv
source .venv/bin/activate
pip install -U pip
pip install eth-account requests
```

If Bash reports `$'\r'` or `pipefail` errors, normalize shell scripts on Linux:

```bash
find benchmark-suite/scripts compose/scripts -name "*.sh" -exec sed -i 's/\r$//' {} +
```
