#!/usr/bin/env python3
"""
run_tps_adjusted_sweep.py: Run DA mode gas sweep with rate_tps adjusted to batch_size.
This avoids hitting the 2000ms batch timeout bottleneck for large batch sizes.
"""

import argparse
import csv
import os
import subprocess
import sys
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path

@dataclass
class Case:
    exp_id: str
    group: str
    description: str
    overrides: dict[str, str]

PROJECT_ROOT = Path(__file__).resolve().parents[2]
BENCH_DIR = PROJECT_ROOT / "benchmark-suite"

PROFILE_DEFAULTS = {
    "smoke": {
        "RATE_TPS": "1",
        "DURATION_S": "5",
        "WARMUP_S": "0",
        "WORKLOAD_TARGET_TXS": "1",
        "WORKLOAD_CONCURRENCY": "1",
        "SEED": "42",
        "DOCKER_UP_BUILD": "0",
    },
    "pilot": {
        "DURATION_S": "60",
        "WARMUP_S": "5",
        "WORKLOAD_CONCURRENCY": "1",
        "SEED": "42",
        "DOCKER_UP_BUILD": "0",
    },
    "final": {
        "DURATION_S": "180",
        "WARMUP_S": "15",
        "WORKLOAD_CONCURRENCY": "1",
        "SEED": "42",
        "DOCKER_UP_BUILD": "0",
    },
}

BASE_ENV = {
    "MAX_BATCH_SIZE": "100",
    "MIN_BATCH_SIZE": "10",
    "TIMEOUT_MS": "2000",
    "BATCH_POLICY": "fixed",
    "POLICY": "FCFS",
    "BLOB_TARGET_BYTES": "120000",
    "BLOB_FILL_TARGET": "0.80",
    "PROVER": "groth16",
    "PROVER_BACKEND": "risc0",
    "REQUIRE_REAL_PROOFS": "true",
    "ALLOW_PROOF_FALLBACK": "0",
    "ALLOW_UNSIGNED_USER_TXS": "0",
    "VALIDITY_PROOF_MODE_POLICY": "groth16_only",
    "ETH_PRICE_USD": "3000",
    "REGULAR_GAS_PRICE_GWEI": "10",
    "BLOB_GAS_PRICE_GWEI": "1",
    "TX_MIX": "balanced",
    "HARDHAT_MINING_INTERVAL": "12000",
    "SEQUENCER_EXECUTOR_PUBLISH_RETRIES": "3",
    "SEQUENCER_EXECUTOR_PUBLISH_TIMEOUT_MS": "5000",
    "COMM_MODE": "grpc",
    "USE_DOCKER_STACK": "1",
}

def _build_cases() -> list[Case]:
    cases: list[Case] = []
    # 3 DA modes x 5 batch sizes = 15 runs
    for da_mode in ("calldata", "blob", "offchain"):
        for batch_size in (25, 50, 100, 200, 500):
            # Scale TPS rate with batch size to ensure the batch fills within 2 seconds
            # e.g., 500 TPS for batch_size=500 fills in 1 second.
            rate_tps = batch_size
            
            # Limit total transactions to prevent generating too many txs (e.g. 500 TPS for 60s is 30k txs!)
            # We send exactly 5 full batches (batch_size * 5)
            target_txs = batch_size * 5

            overrides = {
                "MAX_BATCH_SIZE": str(batch_size),
                "DA_MODE": da_mode,
                "BATCH_POLICY": "fixed",
                "POLICY": "FCFS",
                "RATE_TPS": str(rate_tps),
                "WORKLOAD_TARGET_TXS": str(target_txs),
            }
            cases.append(Case(
                exp_id=f"cmp_da_{da_mode}_bs{batch_size:04d}",
                group="da_sweep",
                description=f"TPS-adjusted DA sweep: {da_mode} batch_size={batch_size}",
                overrides=overrides,
            ))
    return cases

ALL_CASES = _build_cases()

def _session_dir(session_name: str | None, profile: str) -> Path:
    stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    name = session_name or f"adjusted_sweep_{profile}_{stamp}"
    return BENCH_DIR / "metrics" / name

def _write_manifest(path: Path, cases: list[Case], repeats: int, profile: str) -> None:
    with path.open("w", encoding="utf-8", newline="") as f:
        writer = csv.writer(f)
        writer.writerow(["profile", "group", "experiment_id", "description", "repeats", "overrides"])
        for case in cases:
            overrides_str = ", ".join(f"{k}={v}" for k, v in sorted(case.overrides.items()))
            writer.writerow([profile, case.group, case.exp_id, case.description, repeats, overrides_str])

def _run_case(case: Case, repeat: int, env: dict[str, str]) -> None:
    cmd = ["bash", "scripts/run_experiment.sh", case.exp_id, str(repeat)]
    print(
        f"[tps-sweep] exp={case.exp_id} repeat={repeat} "
        f"batch={env.get('MAX_BATCH_SIZE')} timeout={env.get('TIMEOUT_MS')} "
        f"da={env.get('DA_MODE')} rate={env.get('RATE_TPS')} target_txs={env.get('WORKLOAD_TARGET_TXS')}"
    )
    subprocess.run(cmd, cwd=BENCH_DIR, env=env, check=True)

def main() -> None:
    parser = argparse.ArgumentParser(
        description="Run TPS-adjusted DA mode gas sweep benchmark"
    )
    parser.add_argument("--profile", choices=sorted(PROFILE_DEFAULTS), default="pilot")
    parser.add_argument("--repeats", type=int, default=1)
    parser.add_argument("--session-name", default=None)
    parser.add_argument(
        "--mock-proofs",
        action="store_true",
        default=False,
        help="Force mock/fallback proof mode.",
    )
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    session_dir = _session_dir(args.session_name, args.profile)
    session_dir.mkdir(parents=True, exist_ok=True)

    manifest_path = session_dir / "comparison_manifest.csv"
    _write_manifest(manifest_path, ALL_CASES, args.repeats, args.profile)

    print(f"[tps-sweep] profile={args.profile}")
    print(f"[tps-sweep] cases={len(ALL_CASES)} repeats={args.repeats}")
    print(f"[tps-sweep] mock_proofs={args.mock_proofs}")
    print(f"[tps-sweep] session_dir={session_dir}")

    if args.dry_run:
        print("\n[tps-sweep] Dry-run — listing all cases:\n")
        for i, case in enumerate(ALL_CASES, 1):
            overrides_str = " ".join(f"{k}={v}" for k, v in sorted(case.overrides.items()))
            print(f"  {i:02d}. {case.exp_id}")
            print(f"      {case.description}")
            print(f"      {overrides_str}")
        return

    seeds = [42, 43, 44, 45, 46]
    for case in ALL_CASES:
        for repeat in range(1, args.repeats + 1):
            env = os.environ.copy()
            env.update(BASE_ENV)
            env.update(PROFILE_DEFAULTS[args.profile])
            env.update(case.overrides)
            if args.mock_proofs:
                env["PROVER_BACKEND"] = "mock"
                env["REQUIRE_REAL_PROOFS"] = "false"
                env["ALLOW_PROOF_FALLBACK"] = "1"
                env["VALIDITY_PROOF_MODE_POLICY"] = "mock_or_fallback_allowed"
                env["DOCKER_UP_BUILD"] = "1"
                env.setdefault("SUBMITTER_WAIT_MAX", "10000")
            env["SEED"] = str(seeds[(repeat - 1) % len(seeds)])
            env["METRICS_ROOT"] = str(session_dir)
            env["SHARED_METRICS_DIR"] = str(session_dir / "latest")
            env["EXPERIMENT_NAME"] = case.description
            _run_case(case, repeat, env)

    print(f"\n[tps-sweep] Sweep complete. Results in: {session_dir}")
    print(f"[tps-sweep] Aggregating results...")
    
    # Run aggregator
    python_bin = sys.executable
    aggregate_script = PROJECT_ROOT / "data-tools" / "aggregate.py"
    consolidated_csv = session_dir / "consolidated_results.csv"
    subprocess.run([
        python_bin, str(aggregate_script),
        "--metrics_root", str(session_dir),
        "--output", str(consolidated_csv)
    ], check=True)
    
    print(f"[tps-sweep] Generating plots...")
    # Run plotter
    plot_script = BENCH_DIR / "scripts" / "generate_new_comparison_plots.py"
    subprocess.run([
        python_bin, str(plot_script),
        str(consolidated_csv)
    ], check=True)
    
    print("\n[tps-sweep] Done! Plots have been generated in your session directory.")

if __name__ == "__main__":
    main()
