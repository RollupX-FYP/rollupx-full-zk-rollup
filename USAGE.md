# RollupX End-to-End Usage Guide (Docker Flow)

This guide provides the streamlined, containerized path for running RollupX locally with the real executor -> trace -> RISC0 proving -> submitter flow.

## 1. Start the Core Stack

Bring up the entire core architecture (Hardhat L1, Deployer, Executor, Submitter, and Sequencer) using Docker Compose:

```bash
docker compose --profile core up -d --build
```

Wait a few moments for the contracts deployer to finish and all services to become healthy. You can check the status with:

```bash
docker compose ps
```

## 2. Run E2E Workload

Activate the Python virtual environment and run the workload generator:

```bash
cd benchmark-suite
python3 -m venv .venv
source .venv/bin/activate
pip install eth-account

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

*Note: For the automated smoke benchmark and verification harness, see `scripts/smoke_benchmark.sh` and `scripts/verify_stack.sh` once implemented.*

## 3. Validate End-to-End Pipeline

### Check service logs

```bash
docker compose logs --tail 100 -f sequencer
docker compose logs --tail 100 -f executor
docker compose logs --tail 100 -f submitter
```

### Check trace lifecycle

Traces and proofs are stored in Docker volumes. You can inspect the executor traces volume:

```bash
docker compose exec executor ls -la /var/lib/rollupx/executor/traces/
```

## 4. Benchmark + Data Tools

Generate data reports via the `report` compose profile:

```bash
docker compose --profile report run --rm data-tools
```

## 5. Cleanup

To shut down the stack and remove temporary volumes (excluding metrics):

```bash
docker compose --profile core down
```

To remove all data including state and databases:

```bash
docker compose down -v --remove-orphans
docker image prune -f
```
