#!/usr/bin/env bash
# run_matrix.sh — sweep all experiments in config/experiments.toml.
#
# Usage:
#   bash scripts/run_matrix.sh [--dry-run] [--filter <factor>]
#
# Options:
#   --dry-run       Print commands without executing
#   --filter <f>    Only run experiments where factor == <f>
#                   e.g. --filter batch_size

set -euo pipefail

DRY_RUN=false
FILTER=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run) DRY_RUN=true; shift ;;
        --filter)  FILTER="$2"; shift 2 ;;
        *) echo "Unknown flag: $1"; exit 1 ;;
    esac
done

# ── read matrix via Python (handles TOML natively in 3.11+) ──────────────────
python3 - "$FILTER" "$DRY_RUN" <<'PYEOF'
import sys, os, json, subprocess, tomllib

filter_factor = sys.argv[1] if sys.argv[1] else None
dry_run       = sys.argv[2] == "True"

with open("config/experiments.toml", "rb") as f:
    cfg = tomllib.load(f)

baseline = cfg["baseline"]
seeds    = baseline["seeds"]
repeats  = baseline["repeats"]

experiments = cfg["experiments"]

# always include the single true baseline first
experiments_to_run = [{"id": "baseline", "factor": "baseline", **baseline}]
for exp in experiments:
    if filter_factor and exp.get("factor") != filter_factor:
        continue
    # merge with baseline: only override fields specified in the row
    merged = {**baseline, **exp}
    experiments_to_run.append(merged)

print(f"\n{'[DRY-RUN] ' if dry_run else ''}Matrix: {len(experiments_to_run)} configs × {repeats} repeats = {len(experiments_to_run)*repeats} timed runs + {len(experiments_to_run)} warm-ups\n")

failures = []

for exp in experiments_to_run:
    exp_id = exp["id"]
    factor = exp.get("factor", "baseline")

    print(f"\n{'='*70}")
    print(f"  Experiment: {exp_id}  (factor: {factor})")
    print(f"  batch={exp['batch_size']}  timeout={exp['timeout_ms']}ms  "
          f"policy={exp['policy']}  da={exp['da_mode']}  "
          f"prover={exp['prover']}  rate={exp['rate_tps']}tps  mix={exp['tx_mix']}")

    for i, seed in enumerate(seeds[:repeats], start=1):
        run_id = f"{exp_id}_r{i:02d}"
        print(f"\n  -- Run {i}/{repeats}  seed={seed}  run_id={run_id}")

        env = os.environ.copy()
        env.update({
            "MAX_BATCH_SIZE": str(exp["batch_size"]),
            "TIMEOUT_MS":     str(exp["timeout_ms"]),
            "POLICY":         exp["policy"],
            "DA_MODE":        exp["da_mode"],
            "PROVER":         exp["prover"],
            "RATE_TPS":       str(exp["rate_tps"]),
            "DURATION_S":     str(exp["duration_s"]),
            "WARMUP_S":       str(exp.get("warmup_s", 15)),
            "TX_MIX":         exp["tx_mix"],
            "SEED":           str(seed),
            "RUN_ID":         run_id,
            "EXPERIMENT_ID":  exp_id,
        })

        cmd = ["bash", "scripts/run_experiment.sh", exp_id, str(i)]

        if dry_run:
            print(f"  [DRY-RUN] Would run: {' '.join(cmd)}")
            print(f"            env: " + " ".join(f"{k}={v}" for k,v in env.items()
                                                  if k in ("MAX_BATCH_SIZE","POLICY","DA_MODE","RATE_TPS","SEED")))
        else:
            try:
                result = subprocess.run(cmd, env=env, check=True)
            except subprocess.CalledProcessError as e:
                print(f"  [FAIL] {run_id} exited with code {e.returncode}")
                failures.append(run_id)

print(f"\n{'='*70}")
if failures:
    print(f"\n[MATRIX] Completed with {len(failures)} failure(s):")
    for f in failures:
        print(f"  - {f}")
    sys.exit(1)
else:
    print(f"\n[MATRIX] All runs completed successfully.")
PYEOF
