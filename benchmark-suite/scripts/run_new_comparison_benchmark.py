#!/usr/bin/env python3
"""
Comparison benchmark runner for RollupX.

Runs two groups of experiments:
  Group A — DA Mode Gas Sweep:  batch sizes [25, 50, 100, 200, 500] × DA modes [calldata, blob, offchain]
  Group B — Batch Policy Comparison:  load levels [low, medium, high, burst] × policies [fixed, adaptive, dpdab]

Usage:
  # Dry-run to see what would run:
  python scripts/run_new_comparison_benchmark.py --dry-run

  # Run all cases (pilot profile, 1 repeat):
  python scripts/run_new_comparison_benchmark.py --profile pilot --repeats 1

  # Run only the DA sweep:
  python scripts/run_new_comparison_benchmark.py --group da_sweep --profile pilot

  # Run only the batch policy comparison:
  python scripts/run_new_comparison_benchmark.py --group policy_compare --profile pilot

  # With mock proofs (faster, no real RISC0):
  python scripts/run_new_comparison_benchmark.py --mock-proofs --profile pilot
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
        "RATE_TPS": "25",
        "DURATION_S": "60",
        "WARMUP_S": "5",
        "WORKLOAD_TARGET_TXS": "0",
        "WORKLOAD_CONCURRENCY": "1",
        "SEED": "42",
        "DOCKER_UP_BUILD": "0",
    },
    "final": {
        "RATE_TPS": "25",
        "DURATION_S": "180",
        "WARMUP_S": "15",
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
    "ADAPTIVE_SMALL_TIMEOUT_MS": "500",
    "ADAPTIVE_MEDIUM_TIMEOUT_MS": "1000",
    "ADAPTIVE_LARGE_TIMEOUT_MS": "2000",
    "BLOB_TARGET_BYTES": "120000",
    "BLOB_FILL_TARGET": "0.80",
    "POLICY": "FCFS",
    "DA_MODE": "calldata",
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

WORKLOADS = {
    "low":    {"TX_MIX": "balanced", "RATE_TPS": "5"},
    "medium": {"TX_MIX": "balanced", "RATE_TPS": "25"},
    "high":   {"TX_MIX": "balanced", "RATE_TPS": "60", "WORKLOAD_CONCURRENCY": "2"},
    "burst":  {
        "TX_MIX": "balanced",
        "RATE_TPS": "8",
        "WORKLOAD_BURST_ENABLED": "1",
        "WORKLOAD_BURST_RATE_TPS": "80",
        "WORKLOAD_BURST_PERIOD_S": "30",
        "WORKLOAD_BURST_DUTY_CYCLE": "0.25",
        "WORKLOAD_CONCURRENCY": "2",
    },
}


def _build_cases() -> dict[str, list[Case]]:
    # ── Group A: DA mode gas sweep ──────────────────────────────────────
    da_sweep: list[Case] = []
    for da_mode in ("calldata", "blob", "offchain"):
        for batch_size in (25, 50, 100, 200, 500):
            overrides = {
                "MAX_BATCH_SIZE": str(batch_size),
                "DA_MODE": da_mode,
                "BATCH_POLICY": "fixed",
                "POLICY": "FCFS",
            }
            da_sweep.append(Case(
                exp_id=f"cmp_da_{da_mode}_bs{batch_size:04d}",
                group="da_sweep",
                description=f"DA sweep: {da_mode} batch_size={batch_size}",
                overrides=overrides,
            ))

    # ── Group B: Batch policy comparison ────────────────────────────────
    policy_compare: list[Case] = []
    policy_configs = {
        "fixed": {
            "BATCH_POLICY": "fixed",
        },
        "adaptive": {
            "BATCH_POLICY": "adaptive",
            # Use default high timeout (same as timeout_interval_ms) to show
            # the OLD adaptive behavior where timeout doesn't scale down
            "ADAPTIVE_SMALL_TIMEOUT_MS": "2000",
            "ADAPTIVE_MEDIUM_TIMEOUT_MS": "2000",
            "ADAPTIVE_LARGE_TIMEOUT_MS": "2000",
        },
        "dpdab": {
            "BATCH_POLICY": "adaptive",
            # Dynamic timeouts: low-load gets 500ms, mid gets 1000ms, high gets 2000ms
            "ADAPTIVE_SMALL_TIMEOUT_MS": "500",
            "ADAPTIVE_MEDIUM_TIMEOUT_MS": "1000",
            "ADAPTIVE_LARGE_TIMEOUT_MS": "2000",
        },
    }

    for load_name, load_overrides in WORKLOADS.items():
        for policy_name, policy_overrides in policy_configs.items():
            combined = {**load_overrides, **policy_overrides}
            policy_compare.append(Case(
                exp_id=f"cmp_pol_{policy_name}_{load_name}",
                group="policy_compare",
                description=f"Policy compare: {policy_name} under {load_name} load",
                overrides=combined,
            ))

    return {
        "da_sweep": da_sweep,
        "policy_compare": policy_compare,
    }


ALL_CASES = _build_cases()


def _session_dir(session_name: str | None, profile: str) -> Path:
    stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    name = session_name or f"comparison_{profile}_{stamp}"
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
        f"[cmp] group={case.group} exp={case.exp_id} repeat={repeat} "
        f"batch={env.get('MAX_BATCH_SIZE')} timeout={env.get('TIMEOUT_MS')} "
        f"batch_policy={env.get('BATCH_POLICY')} policy={env.get('POLICY')} "
        f"da={env.get('DA_MODE')} mix={env.get('TX_MIX')} rate={env.get('RATE_TPS')} "
        f"adaptive_small_timeout={env.get('ADAPTIVE_SMALL_TIMEOUT_MS')} "
        f"burst={env.get('WORKLOAD_BURST_ENABLED', '0')}"
    )
    subprocess.run(cmd, cwd=BENCH_DIR, env=env, check=True)


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Run comparison benchmark: DA mode gas sweep + batch policy comparison"
    )
    parser.add_argument("--profile", choices=sorted(PROFILE_DEFAULTS), default="pilot")
    parser.add_argument(
        "--group",
        choices=["all", "da_sweep", "policy_compare"],
        default="all",
        help="Which experiment group to run.",
    )
    parser.add_argument("--repeats", type=int, default=1)
    parser.add_argument("--session-name", default=None)
    parser.add_argument(
        "--mock-proofs",
        action="store_true",
        default=False,
        help="Force all cases into mock/fallback proof mode.",
    )
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    # Select groups
    if args.group == "all":
        groups = ["da_sweep", "policy_compare"]
    else:
        groups = [args.group]

    cases: list[Case] = []
    for g in groups:
        cases.extend(ALL_CASES.get(g, []))

    session_dir = _session_dir(args.session_name, args.profile)
    session_dir.mkdir(parents=True, exist_ok=True)

    manifest_path = session_dir / "comparison_manifest.csv"
    _write_manifest(manifest_path, cases, args.repeats, args.profile)

    print(f"[cmp] profile={args.profile}")
    print(f"[cmp] groups={', '.join(groups)}")
    print(f"[cmp] cases={len(cases)} repeats={args.repeats}")
    print(f"[cmp] mock_proofs={args.mock_proofs}")
    print(f"[cmp] session_dir={session_dir}")
    print(f"[cmp] manifest={manifest_path}")

    if args.dry_run:
        print("\n[cmp] Dry-run — listing all cases:\n")
        for i, case in enumerate(cases, 1):
            overrides_str = " ".join(f"{k}={v}" for k, v in sorted(case.overrides.items()))
            print(f"  {i:02d}. [{case.group}] {case.exp_id}")
            print(f"      {case.description}")
            print(f"      {overrides_str}")
        print(f"\n[cmp] Total: {len(cases)} cases × {args.repeats} repeats = {len(cases) * args.repeats} runs")
        return

    seeds = [42, 43, 44, 45, 46]
    for case in cases:
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

    print(f"\n[cmp] All {len(cases) * args.repeats} runs complete.")
    print(f"[cmp] Results in: {session_dir}")
    print(f"\n[cmp] To aggregate and plot:")
    print(f"  python data-tools/aggregate.py {session_dir}")
    print(f"  python scripts/generate_new_comparison_plots.py {session_dir / 'consolidated_results.csv'}")


if __name__ == "__main__":
    main()
