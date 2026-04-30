# Autonomous Full-Stack Containerization Runbook (RollupX)

This document is an execution-grade plan. An agent should be able to implement containerization end-to-end by following phases in order, and **must verify each phase before moving on**.

## Execution Protocol (Mandatory)

1. Work strictly phase-by-phase; do not start phase N+1 until phase N passes verification.
2. If a verification step fails, fix within the same phase and rerun verification.
3. Keep changes minimal and repo-aligned (no unrelated refactors).
4. Use reproducible commands from repository root.
5. After each phase, capture evidence (command + key output) in commit message or notes.
6. Final success requires all phase gates + final acceptance checks.

## Goals

1. `docker compose` can bring up the core rollup runtime without manual file editing.
2. Benchmark workload can run against containerized services and produce metrics.
3. Trace/proof lifecycle and submitter-onchain behavior are observable and validated.
4. Docs and scripts match the real containerized behavior.

## Known Issues to Resolve

1. Missing root `docker-compose.yml`.
2. Missing/partial Dockerfiles (notably `sequencer`, `executor`).
3. Benchmark script drift:
   - legacy sequencer binary (`rollup_sequencer` vs current `sequencer`)
   - `/health` assumption drift
   - `ROLLUPX_CONFIG` not consumed by sequencer startup code
4. Inconsistent runtime config/address handoff between deploy and services.

---

## Phase 0 - Preflight and Baseline

## Objectives

1. Confirm environment/tooling readiness.
2. Confirm current repo state and Docker assets.

## Tasks

1. Confirm Docker/Compose availability.
2. Inventory existing Dockerfiles and scripts.
3. Confirm key runtime ports and service expectations from code/docs.

## Verification Commands

```bash
docker --version
docker compose version

git status --short

find . -maxdepth 3 -name "Dockerfile" -o -name "docker-compose.yml" -o -name ".dockerignore"
```

## Exit Criteria

1. Docker + Compose commands are available.
2. Baseline inventory is known and no blockers remain for build work.

---

## Phase 1 - Standardize Container Build Assets

## Objectives

Create/fix service images for:
- contracts
- sequencer
- executor
- submitter
- benchmark runner

## Required Deliverables

1. `sequencer/Dockerfile` (multi-stage, builds release binary `sequencer`)
2. `executor/Dockerfile` (toolchain aligned with repo; supports real RISC0 path)
3. Updated `submitter/Dockerfile` (runtime-ready)
4. Updated `benchmark-suite/Dockerfile` (Python 3.11+, `eth-account`, benchmark deps)
5. Optional root `.dockerignore` for smaller/faster builds

## Implementation Notes

1. Keep builder stages full, runtime stages slim.
2. Include required system dependencies per service (`cmake`, `nasm`, `protobuf` where needed).
3. Expose expected ports:
   - sequencer: `3000`
   - executor: `50051`
4. Use deterministic image tags (for local dev):
   - `rollupx/contracts:dev`
   - `rollupx/sequencer:dev`
   - `rollupx/executor:dev`
   - `rollupx/submitter:dev`
   - `rollupx/benchmark:dev`

## Verification Commands

```bash
docker build -f contracts/Dockerfile -t rollupx/contracts:dev contracts
docker build -f sequencer/Dockerfile -t rollupx/sequencer:dev sequencer
docker build -f executor/Dockerfile -t rollupx/executor:dev executor
docker build -f submitter/Dockerfile -t rollupx/submitter:dev .
docker build -f benchmark-suite/Dockerfile -t rollupx/benchmark:dev benchmark-suite
```

## Phase Gate (must pass)

1. All images build successfully.
2. `docker run --rm <image> --help` (or equivalent) works for sequencer/executor/submitter containers.

---

## Phase 2 - Compose Orchestration

## Objectives

Wire all services with startup order, shared networking, and persistent volumes.

## Required Deliverables

1. Root `docker-compose.yml`
2. `compose/.env.example` (all configurable values)
3. Profiles:
   - `core` (hardhat, deployer, executor, submitter, sequencer)
   - `bench` (benchmark runner)
   - `report` (data tools/report)
4. Named volumes:
   - `sequencer_db`
   - `executor_state`
   - `executor_traces`
   - `executor_risc0`
   - `submitter_data`
   - `metrics_data`
   - `runtime_config`

## Implementation Notes

1. Service order should be:
   1. hardhat
   2. contracts-deployer (one-shot)
   3. executor
   4. submitter
   5. sequencer
2. Use explicit healthchecks where possible.
3. Put all services on one internal compose network.
4. Use shared runtime volume for deployed contract addresses and generated config.

## Verification Commands

```bash
docker compose config
docker compose --profile core up -d --build
docker compose ps
docker compose logs --no-color --tail 200
```

## Phase Gate (must pass)

1. `docker compose config` succeeds.
2. Core services stay up (no crash loop).
3. Hardhat reachable and deployer completes successfully.

---

## Phase 3 - Runtime and Script Alignment

## Objectives

Fix drift so scripts and runtime behavior are consistent in containerized mode.

## Required Deliverables

1. Sequencer config loader supports:
   - `ROLLUPX_CONFIG` env var
   - fallback to `config/default.toml`
2. Benchmark scripts updated:
   - correct sequencer binary default/name
   - probe strategy aligned with actual sequencer behavior
3. Deployer writes runtime addresses to shared config used by sequencer/submitter.
4. Docs updated (`RUN_GUIDE.md`, `USAGE.md`) to match container flow.

## Implementation Notes

1. Prefer one consistent readiness check:
   - either add real `/health`, or
   - standard JSON-RPC probe on `/`
2. Avoid hardcoded addresses when runtime-generated values are available.

## Verification Commands

```bash
# static checks
bash -n benchmark-suite/scripts/run_experiment.sh
bash -n benchmark-suite/scripts/wait_for_sequencer.sh

# runtime check
docker compose --profile core restart sequencer
docker compose logs --no-color --tail 200 sequencer
```

If JSON-RPC probe is used:

```bash
curl -sf http://127.0.0.1:3000/ -X POST \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"rollup_health","params":{},"id":1}'
```

## Phase Gate (must pass)

1. Scripts are syntactically valid.
2. Sequencer starts with env-supplied config path.
3. Probe strategy used by scripts matches actual endpoint behavior.

---

## Phase 4 - End-to-End Validation Harness

## Objectives

Add deterministic verification entrypoints and prove full integration.

## Required Deliverables

1. `scripts/verify_stack.sh`
2. `scripts/smoke_benchmark.sh` (short run)
3. Optional `scripts/verify_artifacts.sh`

## Minimum Checks to Implement

1. Hardhat responds to `eth_blockNumber`.
2. Contracts deployed and addresses present in shared runtime config.
3. Sequencer accepts workload traffic on `:3000`.
4. Executor lifecycle shows `generated -> persisted -> proved -> published`.
5. Submitter shows batch submission activity (tx hash/receipt path).
6. Metrics files are written by benchmark.

## Verification Commands

```bash
docker compose --profile core up -d
bash scripts/verify_stack.sh
bash scripts/smoke_benchmark.sh
```

Expected benchmark artifacts (under mounted metrics path):

1. `workload_*.json`
2. `tx_log_*.csv`
3. `run_status.json`

Expected executor trace artifacts:

1. `trace_<batch>.json`
2. `proof_<batch>.bin`
3. `journal_<batch>.bin`
4. `proof_meta_<batch>.json`

## Phase Gate (must pass)

1. Verification script exits 0.
2. Smoke benchmark exits 0 and writes artifacts.
3. Lifecycle and submitter checks pass.

---

## Phase 5 - Data Pipeline and CI Smoke Automation

## Objectives

Ensure repeatable validation in CI and prevent future drift.

## Required Deliverables

1. CI workflow (for example: `.github/workflows/docker-smoke.yml`) that:
   - builds images
   - starts core compose stack
   - runs smoke benchmark
   - runs data pipeline
   - uploads logs/artifacts
2. Compose-compatible report step (`run_pipeline.sh` equivalent in containerized mode).

## Verification Commands

```bash
# local CI-equivalent smoke
docker compose --profile core up -d --build
bash scripts/smoke_benchmark.sh
docker compose --profile report run --rm data-tools
```

Expected report outputs:

1. `all_results.csv`
2. `stats_summary.csv`
3. plot images
4. summary markdown report

## Phase Gate (must pass)

1. Local CI-equivalent flow succeeds without manual intervention.
2. Workflow file is present and runnable in CI.

---

## Final Acceptance Criteria

All must be true:

1. One command starts core stack:
   - `docker compose --profile core up -d`
2. One command runs smoke benchmark:
   - `bash scripts/smoke_benchmark.sh`
3. One command verifies stack health:
   - `bash scripts/verify_stack.sh`
4. One command generates analytics artifacts:
   - `docker compose --profile report run --rm data-tools`
5. No manual edits to service configs are required between runs.

---

## Rollback/Cleanup

Use for clean reruns:

```bash
docker compose down -v --remove-orphans
docker image prune -f
```

If preserving metrics is required, do not remove named metrics volumes.

---

## Suggested Commit Sequence

1. `chore(docker): add/fix service Dockerfiles`
2. `chore(compose): add root docker-compose orchestration`
3. `fix(runtime): align sequencer config and benchmark scripts`
4. `test(stack): add verify/smoke scripts`
5. `ci(docker): add smoke workflow`
6. `docs: align RUN_GUIDE and USAGE with docker flow`
