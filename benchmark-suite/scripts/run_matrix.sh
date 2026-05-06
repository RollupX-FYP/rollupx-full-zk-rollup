#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BENCH_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "$BENCH_DIR"

usage() {
    cat <<'EOF'
Usage:
  bash scripts/run_matrix.sh [options]

Common presets:
  --phase smoke           1 repeat, 30s run, 5s warmup, batch-size sweep
  --phase feasibility-lite 3 repeats, 90s run, 5s warmup, batch-size sweep
  --phase feasibility     5 repeats, 120s run, 15s warmup, batch-size sweep
  --phase model-quality   30 repeats, 120s run, 15s warmup, batch-size sweep

Options:
  --dry-run               Print planned runs without executing
  --list                  List selected experiments and exit
  --filter <factor>       baseline, batch_size, da_mode, tx_mix, rate, workload, all
  --only <experiment_id>  Run/list only one experiment id
  --repeats <n>           Override repeat count
  --duration <seconds>    Override measured run duration
  --warmup <seconds>      Override warmup duration
  --docker / --no-docker  Force Docker stack on/off for run_experiment.sh
  --no-build              Do not pass --build to docker compose up
  --help                  Show this help

Examples:
  bash scripts/run_matrix.sh --phase smoke --dry-run
  bash scripts/run_matrix.sh --phase smoke
  bash scripts/run_matrix.sh --filter batch_size --repeats 1 --duration 20 --warmup 5
  bash scripts/run_matrix.sh --only exp_002_batch_size_bs010_calldata_balanced_10tps --repeats 1
EOF
}

DRY_RUN=false
LIST_ONLY=false
FILTER=""
ONLY_ID=""
REPEATS=""
DURATION=""
WARMUP=""
PHASE=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run) DRY_RUN=true; shift ;;
        --list) LIST_ONLY=true; shift ;;
        --filter) FILTER="${2:?--filter requires a value}"; shift 2 ;;
        --only) ONLY_ID="${2:?--only requires an experiment id}"; shift 2 ;;
        --repeats) REPEATS="${2:?--repeats requires a number}"; shift 2 ;;
        --duration) DURATION="${2:?--duration requires seconds}"; shift 2 ;;
        --warmup) WARMUP="${2:?--warmup requires seconds}"; shift 2 ;;
        --phase) PHASE="${2:?--phase requires smoke, feasibility, or model-quality}"; shift 2 ;;
        --workload) FILTER="workload"; shift ;;
        --docker) export USE_DOCKER_STACK=1; shift ;;
        --no-docker) export USE_DOCKER_STACK=0; shift ;;
        --no-build) export DOCKER_UP_BUILD=0; shift ;;
        --help|-h) usage; exit 0 ;;
        *) echo "Unknown flag: $1" >&2; usage; exit 1 ;;
    esac
done

case "$PHASE" in
    "")
        ;;
    smoke)
        FILTER="${FILTER:-batch_size}"
        REPEATS="${REPEATS:-1}"
        DURATION="${DURATION:-30}"
        WARMUP="${WARMUP:-5}"
        export HARDHAT_MINING_INTERVAL=0
        ;;
    feasibility-lite)
        FILTER="${FILTER:-batch_size}"
        REPEATS="${REPEATS:-3}"
        DURATION="${DURATION:-90}"
        WARMUP="${WARMUP:-5}"
        ;;
    feasibility)
        FILTER="${FILTER:-batch_size}"
        REPEATS="${REPEATS:-5}"
        DURATION="${DURATION:-120}"
        WARMUP="${WARMUP:-15}"
        ;;
    model-quality)
        FILTER="${FILTER:-batch_size}"
        REPEATS="${REPEATS:-30}"
        DURATION="${DURATION:-120}"
        WARMUP="${WARMUP:-15}"
        ;;
    *)
        echo "Unknown phase: $PHASE" >&2
        usage
        exit 1
        ;;
esac

export REPEATS_OVERRIDE="${REPEATS:-${REPEATS_OVERRIDE:-}}"
export DURATION_S_OVERRIDE="${DURATION:-${DURATION_S_OVERRIDE:-}}"
export WARMUP_S_OVERRIDE="${WARMUP:-${WARMUP_S_OVERRIDE:-}}"

python3 - "$FILTER" "$DRY_RUN" "$LIST_ONLY" "$ONLY_ID" <<'PYEOF'
import os
import subprocess
import sys
import tomllib

filter_factor = sys.argv[1] or None
dry_run = sys.argv[2].lower() == "true"
list_only = sys.argv[3].lower() == "true"
only_id = sys.argv[4] or None

def optional_int_env(name):
    value = os.environ.get(name)
    if value in (None, ""):
        return None
    return int(value)

d_override = optional_int_env("DURATION_S_OVERRIDE")
w_override = optional_int_env("WARMUP_S_OVERRIDE")
r_override = optional_int_env("REPEATS_OVERRIDE")

with open("config/experiments.toml", "rb") as f:
    cfg = tomllib.load(f)

baseline = cfg["baseline"]
seeds = baseline["seeds"]
repeats = r_override if r_override is not None else baseline["repeats"]

experiments_to_run = [{"factor": "baseline", **baseline}]
for exp in cfg["experiments"]:
    factor = exp.get("factor")
    if filter_factor in (None, "", "all"):
        pass
    elif filter_factor == "workload" and factor not in ("rate", "tx_mix"):
        continue
    elif filter_factor != "workload" and factor != filter_factor:
        continue
    experiments_to_run.append({**baseline, **exp})

if filter_factor and filter_factor not in ("all", "baseline") and not only_id:
    experiments_to_run = [
        exp for exp in experiments_to_run
        if exp.get("factor") == "baseline" or exp.get("factor") == filter_factor or (
            filter_factor == "workload" and exp.get("factor") in ("rate", "tx_mix")
        )
    ]

if filter_factor == "baseline":
    experiments_to_run = [exp for exp in experiments_to_run if exp.get("factor") == "baseline"]

if only_id:
    experiments_to_run = [exp for exp in experiments_to_run if exp["id"] == only_id]
    if not experiments_to_run:
        print(f"[MATRIX] No experiment matched --only {only_id}", file=sys.stderr)
        sys.exit(2)

mode = "[DRY-RUN] " if dry_run else "[LIST] " if list_only else ""
print(
    f"\n{mode}Matrix: {len(experiments_to_run)} configs x {repeats} repeats "
    f"= {len(experiments_to_run) * repeats} timed runs\n"
)

failures = []

for index, exp in enumerate(experiments_to_run, start=1):
    exp_id = exp["id"]
    exp_name = exp.get("name", exp_id)
    factor = exp.get("factor", "baseline")
    duration_s = d_override if d_override is not None else exp["duration_s"]
    warmup_s = w_override if w_override is not None else exp.get("warmup_s", 15)

    print(f"{index:02d}. {exp_id}")
    print(f"    name={exp_name}")
    print(
        f"    factor={factor} batch={exp['batch_size']} timeout={exp['timeout_ms']}ms "
        f"min_batch={exp.get('min_batch_size', 1)} batch_policy={exp.get('batch_policy', baseline.get('batch_policy', 'fixed'))} policy={exp['policy']}"
    )
    print(
        f"    da={exp['da_mode']} prover={exp['prover']} rate={exp['rate_tps']}tps "
        f"mix={exp['tx_mix']} duration={duration_s}s warmup={warmup_s}s repeats={repeats}"
    )

    if list_only:
        continue

    for i, seed in enumerate(seeds[:repeats], start=1):
        run_id = f"{exp_id}_r{i:02d}"
        print(f"\n  -- Run {i}/{repeats} seed={seed} run_id={run_id}")

        env = os.environ.copy()
        env.update({
            "MAX_BATCH_SIZE": str(exp["batch_size"]),
            "TIMEOUT_MS": str(exp["timeout_ms"]),
            "MIN_BATCH_SIZE": str(exp.get("min_batch_size", 1)),
            "BATCH_POLICY": exp.get("batch_policy", baseline.get("batch_policy", "fixed")),
            "ADAPTIVE_LOW_LOAD_THRESHOLD": str(exp.get("adaptive_low_load_threshold", baseline.get("adaptive_low_load_threshold", 50))),
            "ADAPTIVE_MEDIUM_LOAD_THRESHOLD": str(exp.get("adaptive_medium_load_threshold", baseline.get("adaptive_medium_load_threshold", 200))),
            "ADAPTIVE_SMALL_BATCH_SIZE": str(exp.get("adaptive_small_batch_size", baseline.get("adaptive_small_batch_size", 50))),
            "ADAPTIVE_MEDIUM_BATCH_SIZE": str(exp.get("adaptive_medium_batch_size", baseline.get("adaptive_medium_batch_size", 100))),
            "ADAPTIVE_LARGE_BATCH_SIZE": str(exp.get("adaptive_large_batch_size", baseline.get("adaptive_large_batch_size", 500))),
            "BLOB_TARGET_BYTES": str(exp.get("blob_target_bytes", baseline.get("blob_target_bytes", 131072))),
            "BLOB_FILL_TARGET": str(exp.get("blob_fill_target", baseline.get("blob_fill_target", 0.90))),
            "POLICY": exp["policy"],
            "DA_MODE": exp["da_mode"],
            "PROVER": exp["prover"],
            "REQUIRE_REAL_PROOFS": "true",
            "RATE_TPS": str(exp["rate_tps"]),
            "DURATION_S": str(duration_s),
            "WARMUP_S": str(warmup_s),
            "TX_MIX": exp["tx_mix"],
            "SEED": str(seed),
            "RUN_ID": run_id,
            "EXPERIMENT_ID": exp_id,
            "EXPERIMENT_NAME": exp_name,
        })

        cmd = ["bash", "scripts/run_experiment.sh", exp_id, str(i)]
        if dry_run:
            selected_env = " ".join(
                f"{k}={env[k]}" for k in (
                    "MAX_BATCH_SIZE", "TIMEOUT_MS", "MIN_BATCH_SIZE",
                    "BATCH_POLICY", "ADAPTIVE_LOW_LOAD_THRESHOLD",
                    "ADAPTIVE_MEDIUM_LOAD_THRESHOLD", "ADAPTIVE_SMALL_BATCH_SIZE",
                    "ADAPTIVE_MEDIUM_BATCH_SIZE", "ADAPTIVE_LARGE_BATCH_SIZE",
                    "BLOB_TARGET_BYTES", "BLOB_FILL_TARGET",
                    "POLICY", "DA_MODE", "RATE_TPS", "DURATION_S",
                    "WARMUP_S", "SEED"
                )
            )
            print(f"  [DRY-RUN] {selected_env} {' '.join(cmd)}")
            continue

        try:
            subprocess.run(cmd, env=env, check=True)
        except subprocess.CalledProcessError as e:
            print(f"  [FAIL] {run_id} exited with code {e.returncode}")
            failures.append(run_id)

print("\n" + "=" * 70)
if list_only or dry_run:
    print("[MATRIX] No runs executed.")
elif failures:
    print(f"[MATRIX] Completed with {len(failures)} failure(s):")
    for failure in failures:
        print(f"  - {failure}")
    sys.exit(1)
else:
    print("[MATRIX] All runs completed successfully.")
PYEOF
