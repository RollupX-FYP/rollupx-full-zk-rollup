#!/usr/bin/env python3
"""
Plan-aware benchmark runner for rollupx_benchmarking_plan.md.

It reuses the same harness command shape as manual runs:
  <ENV OVERRIDES> bash scripts/run_experiment.sh <experiment_id> <repeat>
"""

from __future__ import annotations

import argparse
import csv
import os
import subprocess
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path


@dataclass(frozen=True)
class Case:
    exp_id: str
    stage: str
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
        "RATE_TPS": "25",
        "DURATION_S": "60",
        "WARMUP_S": "5",
        "WORKLOAD_TARGET_TXS": "0",
        "WORKLOAD_CONCURRENCY": "1",
        "SEED": "42",
        "DOCKER_UP_BUILD": "0",
    },
    "final": {
        "RATE_TPS": "50",
        "DURATION_S": "600",
        "WARMUP_S": "60",
        "WORKLOAD_TARGET_TXS": "0",
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
    "ADAPTIVE_LOW_LOAD_THRESHOLD": "25",
    "ADAPTIVE_MEDIUM_LOAD_THRESHOLD": "100",
    "ADAPTIVE_SMALL_BATCH_SIZE": "25",
    "ADAPTIVE_MEDIUM_BATCH_SIZE": "100",
    "ADAPTIVE_LARGE_BATCH_SIZE": "500",
    "BLOB_TARGET_BYTES": "120000",
    "BLOB_FILL_TARGET": "0.80",
    "POLICY": "FCFS",
    "DA_MODE": "calldata",
    "PROVER": "groth16",
    "PROVER_BACKEND": "risc0",
    "REQUIRE_REAL_PROOFS": "true",
    "ALLOW_PROOF_FALLBACK": "1",
    "ALLOW_UNSIGNED_USER_TXS": "0",
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

STAGE_CASES = {
    "baseline": [
        Case("baseline", "baseline", "Baseline configuration", {}),
    ],
    "stage1": [
        Case("bs_025", "stage1", "Batch size 25", {"MAX_BATCH_SIZE": "25"}),
        Case("bs_050", "stage1", "Batch size 50", {"MAX_BATCH_SIZE": "50"}),
        Case("bs_100", "stage1", "Batch size 100", {"MAX_BATCH_SIZE": "100"}),
        Case("bs_200", "stage1", "Batch size 200", {"MAX_BATCH_SIZE": "200"}),
        Case("bs_500", "stage1", "Batch size 500", {"MAX_BATCH_SIZE": "500"}),
        Case("to_0500", "stage1", "Timeout 500ms", {"TIMEOUT_MS": "500"}),
        Case("to_1000", "stage1", "Timeout 1000ms", {"TIMEOUT_MS": "1000"}),
        Case("to_2000", "stage1", "Timeout 2000ms", {"TIMEOUT_MS": "2000"}),
        Case("to_5000", "stage1", "Timeout 5000ms", {"TIMEOUT_MS": "5000"}),
    ],
    "stage2": [
        Case("ab_fixed_low", "stage2", "Fixed batching at low load", {"RATE_TPS": "10", "BATCH_POLICY": "fixed"}),
        Case("ab_adaptive_low", "stage2", "Adaptive batching at low load", {"RATE_TPS": "10", "BATCH_POLICY": "adaptive"}),
        Case("ab_fixed_high", "stage2", "Fixed batching at high load", {"RATE_TPS": "75", "BATCH_POLICY": "fixed"}),
        Case("ab_adaptive_high", "stage2", "Adaptive batching at high load", {"RATE_TPS": "75", "BATCH_POLICY": "adaptive"}),
    ],
    "stage3": [
        Case("pol_fcfs", "stage3", "FCFS scheduling", {"POLICY": "FCFS"}),
        Case("pol_feepriority", "stage3", "Fee-priority scheduling", {"POLICY": "FeePriority"}),
        Case("pol_blobpacking", "stage3", "Blob-aware packing policy", {"POLICY": "BlobPacking", "DA_MODE": "blob"}),
    ],
    "stage4": [
        Case("da_calldata", "stage4", "Calldata DA mode", {"DA_MODE": "calldata"}),
        Case("da_blob", "stage4", "Blob DA mode", {"DA_MODE": "blob"}),
        Case("da_offchain", "stage4", "Offchain DA mode", {"DA_MODE": "offchain"}),
        Case("da_blobpacking", "stage4", "Blob mode with BlobPacking", {"DA_MODE": "blob", "POLICY": "BlobPacking"}),
    ],
    "stage5": [
        Case("proof_real", "stage5", "Real proofs required", {"REQUIRE_REAL_PROOFS": "true", "ALLOW_PROOF_FALLBACK": "1"}),
        Case("proof_mock", "stage5", "Mock/fallback proof mode", {"REQUIRE_REAL_PROOFS": "false", "ALLOW_PROOF_FALLBACK": "1"}),
        Case("proof_strict", "stage5", "Strict real proofs without fallback", {"REQUIRE_REAL_PROOFS": "true", "ALLOW_PROOF_FALLBACK": "0"}),
    ],
    "stage6": [
        Case("l1_fast", "stage6", "Fast L1 mining", {"HARDHAT_MINING_INTERVAL": "1000"}),
        Case("l1_normal", "stage6", "Normal L1 mining", {"HARDHAT_MINING_INTERVAL": "12000"}),
        Case("l1_slow", "stage6", "Slow L1 mining", {"HARDHAT_MINING_INTERVAL": "30000"}),
    ],
    "stage7": [
        Case("rel_retry0", "stage7", "No publish retries", {"SEQUENCER_EXECUTOR_PUBLISH_RETRIES": "0"}),
        Case("rel_retry1", "stage7", "Single publish retry", {"SEQUENCER_EXECUTOR_PUBLISH_RETRIES": "1"}),
        Case("rel_retry3", "stage7", "Three publish retries", {"SEQUENCER_EXECUTOR_PUBLISH_RETRIES": "3"}),
        Case("rel_to1000", "stage7", "Publish timeout 1000ms", {"SEQUENCER_EXECUTOR_PUBLISH_TIMEOUT_MS": "1000"}),
        Case("rel_to5000", "stage7", "Publish timeout 5000ms", {"SEQUENCER_EXECUTOR_PUBLISH_TIMEOUT_MS": "5000"}),
    ],
}


def _session_dir(session_name: str | None, profile: str) -> Path:
    stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    name = session_name or f"plan_{profile}_{stamp}"
    return BENCH_DIR / "metrics" / name


def _selected_cases(stage_names: list[str]) -> list[Case]:
    if "all" in stage_names:
        ordered = ["baseline", "stage1", "stage2", "stage3", "stage4", "stage5", "stage6", "stage7"]
    elif "minimum" in stage_names:
        ordered = ["baseline", "stage1", "stage3", "stage4", "stage5"]
    else:
        ordered = ["baseline"] + [name for name in stage_names if name != "baseline"]

    cases: list[Case] = []
    for name in ordered:
        cases.extend(STAGE_CASES.get(name, []))
    seen: set[str] = set()
    unique_cases = []
    for case in cases:
        if case.exp_id in seen:
            continue
        seen.add(case.exp_id)
        unique_cases.append(case)
    return unique_cases


def _write_manifest(path: Path, cases: list[Case], repeats: int, profile: str) -> None:
    with path.open("w", encoding="utf-8", newline="") as f:
        writer = csv.writer(f)
        writer.writerow(["profile", "stage", "experiment_id", "description", "repeats", "overrides"])
        for case in cases:
            overrides = ", ".join(f"{key}={value}" for key, value in sorted(case.overrides.items()))
            writer.writerow([profile, case.stage, case.exp_id, case.description, repeats, overrides])


def _run_case(case: Case, repeat: int, env: dict[str, str]) -> None:
    cmd = ["bash", "scripts/run_experiment.sh", case.exp_id, str(repeat)]
    print(
        f"[plan] stage={case.stage} exp={case.exp_id} repeat={repeat} "
        f"batch={env.get('MAX_BATCH_SIZE')} timeout={env.get('TIMEOUT_MS')} "
        f"policy={env.get('POLICY')} da={env.get('DA_MODE')} real_proofs={env.get('REQUIRE_REAL_PROOFS')}"
    )
    subprocess.run(cmd, cwd=BENCH_DIR, env=env, check=True)


def main() -> None:
    parser = argparse.ArgumentParser(description="Run plan-aligned RollupX benchmark cases")
    parser.add_argument("--profile", choices=sorted(PROFILE_DEFAULTS), default="pilot")
    parser.add_argument(
        "--stage",
        action="append",
        choices=["minimum", "all", "baseline", "stage1", "stage2", "stage3", "stage4", "stage5", "stage6", "stage7"],
        help="Stage group to run. Repeat flag to combine multiple stages.",
    )
    parser.add_argument("--repeats", type=int, default=1)
    parser.add_argument("--analytics", action="store_true", default=False)
    parser.add_argument("--analytics-mode", choices=["local", "docker"], default="local")
    parser.add_argument("--session-name", default=None)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    stages = args.stage or ["minimum"]
    cases = _selected_cases(stages)
    session_dir = _session_dir(args.session_name, args.profile)
    session_dir.mkdir(parents=True, exist_ok=True)

    manifest_path = session_dir / "plan_manifest.csv"
    _write_manifest(manifest_path, cases, args.repeats, args.profile)

    print(f"[plan] profile={args.profile}")
    print(f"[plan] stages={', '.join(stages)}")
    print(f"[plan] cases={len(cases)} repeats={args.repeats}")
    print(f"[plan] session_dir={session_dir}")
    print(f"[plan] manifest={manifest_path}")

    if args.dry_run:
        return

    seeds = [42, 43, 44, 45, 46]
    for case in cases:
        for repeat in range(1, args.repeats + 1):
            env = os.environ.copy()
            env.update(BASE_ENV)
            env.update(PROFILE_DEFAULTS[args.profile])
            env.update(case.overrides)
            env["SEED"] = str(seeds[(repeat - 1) % len(seeds)])
            env["METRICS_ROOT"] = str(session_dir)
            env["SHARED_METRICS_DIR"] = str(session_dir / "latest")
            env["EXPERIMENT_NAME"] = case.description
            _run_case(case, repeat, env)

    if args.analytics:
        subprocess.run(
            ["bash", "scripts/generate_plan_artifacts.sh", str(session_dir), args.analytics_mode],
            cwd=BENCH_DIR,
            check=True,
        )


if __name__ == "__main__":
    main()
