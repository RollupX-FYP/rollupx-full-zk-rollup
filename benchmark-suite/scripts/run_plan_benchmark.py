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

def _case(exp_id: str, stage: str, description: str, **overrides: str) -> Case:
    return Case(exp_id, stage, description, overrides)


def _workload_overrides(name: str) -> dict[str, str]:
    workloads = {
        "normal": {"TX_MIX": "balanced", "RATE_TPS": "25"},
        "low": {"TX_MIX": "balanced", "RATE_TPS": "10"},
        "medium": {"TX_MIX": "balanced", "RATE_TPS": "25"},
        "high": {"TX_MIX": "balanced", "RATE_TPS": "60", "WORKLOAD_CONCURRENCY": "2"},
        "burst": {
            "TX_MIX": "balanced",
            "RATE_TPS": "8",
            "WORKLOAD_BURST_ENABLED": "1",
            "WORKLOAD_BURST_RATE_TPS": "80",
            "WORKLOAD_BURST_PERIOD_S": "30",
            "WORKLOAD_BURST_DUTY_CYCLE": "0.25",
            "WORKLOAD_CONCURRENCY": "2",
        },
        "transfer": {"TX_MIX": "transfer", "RATE_TPS": "40", "WORKLOAD_CONCURRENCY": "2"},
        "heavy": {"TX_MIX": "heavy", "RATE_TPS": "25", "WORKLOAD_CONCURRENCY": "2"},
        "da_heavy": {"TX_MIX": "da_heavy", "RATE_TPS": "25", "WORKLOAD_CONCURRENCY": "2"},
    }
    return workloads[name]


def _stage_cases() -> dict[str, list[Case]]:
    stage1: list[Case] = []
    for size in (25, 50, 100, 200, 500, 1000):
        stage1.append(
            _case(f"s1_bs_{size:04d}", "stage1", f"Fixed batch size {size}", MAX_BATCH_SIZE=str(size))
        )
    for timeout in (500, 1000, 2000, 5000, 10000):
        stage1.append(
            _case(f"s1_to_{timeout:05d}", "stage1", f"Fixed timeout {timeout}ms", TIMEOUT_MS=str(timeout))
        )
    for workload in ("normal", "transfer", "heavy"):
        stage1.append(
            _case(
                f"s1_wl_{workload}",
                "stage1",
                f"Fixed batching under {workload} workload",
                **_workload_overrides(workload),
            )
        )

    stage2: list[Case] = []
    for load in ("low", "medium", "high", "burst"):
        for policy in ("fixed", "adaptive"):
            stage2.append(
                _case(
                    f"s2_{policy}_{load}",
                    "stage2",
                    f"{policy.title()} batching under {load} load",
                    BATCH_POLICY=policy,
                    **_workload_overrides(load),
                )
            )
    for low, medium, small, mid, large in (
        (10, 50, 25, 100, 500),
        (25, 100, 50, 200, 500),
        (50, 150, 50, 200, 1000),
    ):
        stage2.append(
            _case(
                f"s2_adapt_l{low}_m{medium}",
                "stage2",
                f"Adaptive thresholds low={low} medium={medium}",
                BATCH_POLICY="adaptive",
                ADAPTIVE_LOW_LOAD_THRESHOLD=str(low),
                ADAPTIVE_MEDIUM_LOAD_THRESHOLD=str(medium),
                ADAPTIVE_SMALL_BATCH_SIZE=str(small),
                ADAPTIVE_MEDIUM_BATCH_SIZE=str(mid),
                ADAPTIVE_LARGE_BATCH_SIZE=str(large),
            )
        )

    stage3: list[Case] = []
    for policy in ("FCFS", "FeePriority", "TimeBoost", "FairBFT", "BlobPacking"):
        overrides = {"POLICY": policy, "TX_MIX": "balanced"}
        if policy == "BlobPacking":
            overrides.update({"DA_MODE": "blob", "TX_MIX": "da_heavy"})
        stage3.append(
            _case(f"s3_pol_{policy.lower()}", "stage3", f"{policy} scheduling policy", **overrides)
        )
    for policy in ("FeePriority", "TimeBoost", "FairBFT"):
        stage3.append(
            _case(
                f"s3_burst_{policy.lower()}",
                "stage3",
                f"{policy} scheduling under burst load",
                POLICY=policy,
                **_workload_overrides("burst"),
            )
        )

    stage4: list[Case] = [
        _case("s4_da_calldata", "stage4", "Calldata DA mode", DA_MODE="calldata"),
        _case("s4_da_blob", "stage4", "Blob DA mode", DA_MODE="blob"),
        _case("s4_da_offchain", "stage4", "Offchain DA mode", DA_MODE="offchain"),
        _case("s4_da_blobpacking", "stage4", "Blob mode with BlobPacking", DA_MODE="blob", POLICY="BlobPacking", TX_MIX="da_heavy"),
    ]
    for target in (32768, 65536, 98304, 120000):
        stage4.append(
            _case(
                f"s4_blob_target_{target}",
                "stage4",
                f"Blob target bytes {target}",
                DA_MODE="blob",
                BLOB_TARGET_BYTES=str(target),
                TX_MIX="da_heavy",
            )
        )
    for fill in ("0.50", "0.70", "0.80", "0.90", "0.95"):
        stage4.append(
            _case(
                f"s4_blob_fill_{fill.replace('.', '')}",
                "stage4",
                f"Blob fill target {fill}",
                DA_MODE="blob",
                BLOB_FILL_TARGET=fill,
                TX_MIX="da_heavy",
            )
        )

    stage5: list[Case] = []
    for size in (50, 100, 200, 500):
        stage5.append(
            _case(
                f"s5_real_bs_{size:04d}",
                "stage5",
                f"Real RISC0 proof batch size {size}",
                MAX_BATCH_SIZE=str(size),
                REQUIRE_REAL_PROOFS="true",
                ALLOW_PROOF_FALLBACK="1",
            )
        )
    stage5.extend(
        [
            _case("s5_proof_mock", "stage5", "Mock/fallback proof mode", REQUIRE_REAL_PROOFS="false", ALLOW_PROOF_FALLBACK="1"),
            _case("s5_proof_real", "stage5", "Real proofs with fallback allowed", REQUIRE_REAL_PROOFS="true", ALLOW_PROOF_FALLBACK="1"),
            _case("s5_proof_strict", "stage5", "Strict real proofs without fallback", REQUIRE_REAL_PROOFS="true", ALLOW_PROOF_FALLBACK="0"),
            _case("s5_heavy_real", "stage5", "Real proofs under heavy-state workload", REQUIRE_REAL_PROOFS="true", ALLOW_PROOF_FALLBACK="1", **_workload_overrides("heavy")),
        ]
    )

    stage6: list[Case] = []
    for interval in (1000, 3000, 12000, 30000):
        stage6.append(
            _case(
                f"s6_l1_interval_{interval}",
                "stage6",
                f"L1 mining interval {interval}ms",
                HARDHAT_MINING_INTERVAL=str(interval),
            )
        )
    for regular, blob in ((5, "0.1"), (10, "1"), (30, "5"), (100, "20")):
        stage6.append(
            _case(
                f"s6_gas_regular_{regular}_blob_{blob.replace('.', '')}",
                "stage6",
                f"Gas price regular={regular}gwei blob={blob}gwei",
                REGULAR_GAS_PRICE_GWEI=str(regular),
                BLOB_GAS_PRICE_GWEI=blob,
                DA_MODE="blob",
            )
        )

    stage7: list[Case] = []
    for retries in (0, 1, 3, 5):
        stage7.append(
            _case(
                f"s7_retry_{retries}",
                "stage7",
                f"Publish retries {retries}",
                SEQUENCER_EXECUTOR_PUBLISH_RETRIES=str(retries),
                **_workload_overrides("burst"),
            )
        )
    for timeout in (1000, 3000, 5000, 10000):
        stage7.append(
            _case(
                f"s7_timeout_{timeout}",
                "stage7",
                f"Publish timeout {timeout}ms",
                SEQUENCER_EXECUTOR_PUBLISH_TIMEOUT_MS=str(timeout),
                **_workload_overrides("burst"),
            )
        )
    for comm in ("grpc", "file"):
        stage7.append(
            _case(
                f"s7_comm_{comm}",
                "stage7",
                f"{comm.upper()} communication mode",
                COMM_MODE=comm,
                **_workload_overrides("burst"),
            )
        )

    stage8: list[Case] = []
    final_configs = {
        "baseline": {},
        "best_fixed": {"MAX_BATCH_SIZE": "200", "TIMEOUT_MS": "2000", "BATCH_POLICY": "fixed", "POLICY": "FCFS", "DA_MODE": "calldata"},
        "best_adaptive": {
            "BATCH_POLICY": "adaptive",
            "ADAPTIVE_LOW_LOAD_THRESHOLD": "25",
            "ADAPTIVE_MEDIUM_LOAD_THRESHOLD": "100",
            "ADAPTIVE_SMALL_BATCH_SIZE": "50",
            "ADAPTIVE_MEDIUM_BATCH_SIZE": "200",
            "ADAPTIVE_LARGE_BATCH_SIZE": "500",
            "TIMEOUT_MS": "2000",
        },
        "best_fairness": {"POLICY": "FairBFT", "MAX_BATCH_SIZE": "100", "TIMEOUT_MS": "2000"},
        "best_cost": {"POLICY": "BlobPacking", "DA_MODE": "blob", "BLOB_TARGET_BYTES": "120000", "BLOB_FILL_TARGET": "0.90"},
        "best_realproof": {"REQUIRE_REAL_PROOFS": "true", "ALLOW_PROOF_FALLBACK": "0", "MAX_BATCH_SIZE": "100"},
    }
    for config_name, config_overrides in final_configs.items():
        for workload in ("normal", "burst", "heavy", "da_heavy"):
            stage8.append(
                _case(
                    f"s8_{config_name}_{workload}",
                    "stage8",
                    f"Final comparison {config_name} on {workload} workload",
                    **{**_workload_overrides(workload), **config_overrides},
                )
            )

    return {
        "stage0": [
            _case(
                "s0_validation",
                "stage0",
                "Instrumentation validation at 5 TPS transfer-only workload",
                RATE_TPS="5",
                DURATION_S="60",
                WARMUP_S="0",
                TX_MIX="transfer",
                REQUIRE_REAL_PROOFS="false",
                ALLOW_PROOF_FALLBACK="1",
            )
        ],
        "baseline": [_case("baseline", "baseline", "Baseline configuration")],
        "stage1": stage1,
        "stage2": stage2,
        "stage3": stage3,
        "stage4": stage4,
        "stage5": stage5,
        "stage6": stage6,
        "stage7": stage7,
        "stage8": stage8,
    }


STAGE_CASES = _stage_cases()


def _session_dir(session_name: str | None, profile: str) -> Path:
    stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    name = session_name or f"plan_{profile}_{stamp}"
    return BENCH_DIR / "metrics" / name


def _selected_cases(stage_names: list[str]) -> list[Case]:
    if "all" in stage_names:
        ordered = ["stage0", "baseline", "stage1", "stage2", "stage3", "stage4", "stage5", "stage6", "stage7", "stage8"]
    elif "minimum" in stage_names:
        ordered = ["stage0", "baseline", "stage1", "stage3", "stage4", "stage5"]
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
        f"batch_policy={env.get('BATCH_POLICY')} policy={env.get('POLICY')} "
        f"da={env.get('DA_MODE')} mix={env.get('TX_MIX')} rate={env.get('RATE_TPS')} "
        f"prover_backend={env.get('PROVER_BACKEND')} "
        f"burst={env.get('WORKLOAD_BURST_ENABLED')} real_proofs={env.get('REQUIRE_REAL_PROOFS')} "
        f"allow_fallback={env.get('ALLOW_PROOF_FALLBACK')}"
    )
    subprocess.run(cmd, cwd=BENCH_DIR, env=env, check=True)


def main() -> None:
    parser = argparse.ArgumentParser(description="Run plan-aligned RollupX benchmark cases")
    parser.add_argument("--profile", choices=sorted(PROFILE_DEFAULTS), default="pilot")
    parser.add_argument(
        "--stage",
        action="append",
        choices=[
            "minimum",
            "all",
            "stage0",
            "baseline",
            "stage1",
            "stage2",
            "stage3",
            "stage4",
            "stage5",
            "stage6",
            "stage7",
            "stage8",
        ],
        help="Stage group to run. Repeat flag to combine multiple stages.",
    )
    parser.add_argument("--repeats", type=int, default=1)
    parser.add_argument("--analytics", action="store_true", default=False)
    parser.add_argument("--analytics-mode", choices=["local", "docker"], default="local")
    parser.add_argument("--session-name", default=None)
    parser.add_argument(
        "--mock-proofs",
        action="store_true",
        default=False,
        help="Force all selected cases into mock/fallback proof mode for this session.",
    )
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
    print(f"[plan] mock_proofs={args.mock_proofs}")
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
            if args.mock_proofs:
                env["PROVER_BACKEND"] = "mock"
                env["REQUIRE_REAL_PROOFS"] = "false"
                env["ALLOW_PROOF_FALLBACK"] = "1"
                env["VALIDITY_PROOF_MODE_POLICY"] = "mock_or_fallback_allowed"
                env["DOCKER_UP_BUILD"] = "1"
                env.setdefault("SUBMITTER_WAIT_MAX", "120")
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
