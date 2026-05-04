#!/usr/bin/env python3
"""
Estimate batch cost curves from RollupX component metrics.

This is intentionally a metrics-only analysis. It joins sequencer, executor,
and submitter per-batch JSONL rows, computes an empirical RollupX cost proxy,
and also computes an explicit calibrated prover-sensitivity curve. The second
curve is useful when the local prover has mostly fixed/mock overhead and cannot
by itself demonstrate zkSync Era-style proving growth.
"""

from __future__ import annotations

import argparse
import json
import math
import re
from pathlib import Path

import matplotlib.pyplot as plt
import numpy as np
import pandas as pd


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Create estimated full-cost vs batch-size plots from benchmark metrics."
    )
    parser.add_argument("metrics_root", type=Path, help="benchmark-suite/metrics folder")
    parser.add_argument(
        "--out",
        type=Path,
        default=None,
        help="output directory; default: <metrics_root>/cost_curve_analysis",
    )
    parser.add_argument("--eth-usd", type=float, default=3000.0)
    parser.add_argument("--l1-gas-gwei", type=float, default=25.0)
    parser.add_argument("--prover-hour-usd", type=float, default=3.0)
    parser.add_argument("--sequencer-hour-usd", type=float, default=0.25)
    parser.add_argument("--compressor-hour-usd", type=float, default=0.25)
    parser.add_argument(
        "--calibrated-prover-ms-per-tx",
        type=float,
        default=45.0,
        help="explicit sensitivity assumption added to measured prover time",
    )
    parser.add_argument(
        "--calibrated-prover-quadratic-ms",
        type=float,
        default=0.04,
        help="explicit sensitivity assumption: ms * tx_count^2",
    )
    parser.add_argument(
        "--min-complete-coverage",
        type=float,
        default=0.70,
        help="drop a run if submitter/executor coverage is below this fraction of sequencer rows",
    )
    parser.add_argument(
        "--experiment-prefix",
        default="exp_",
        help="only include experiment directories with this prefix; pass empty string to include legacy dirs",
    )
    return parser.parse_args()


def read_jsonl(path: Path) -> list[dict]:
    rows: list[dict] = []
    if not path.exists():
        return rows
    with path.open("r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if line:
                rows.append(json.loads(line))
    return rows


def configured_size_from_name(text: str) -> int | None:
    patterns = [
        r"bs(\d{3,4})",
        r"baseline_bs(\d{3,4})",
        r"bs_(\d{3,4})",
    ]
    for pattern in patterns:
        match = re.search(pattern, text)
        if match:
            return int(match.group(1))
    return None


def load_batches(metrics_root: Path, min_complete_coverage: float, experiment_prefix: str) -> pd.DataFrame:
    rows: list[dict] = []
    for run_dir in sorted(p for p in metrics_root.rglob("*") if p.is_dir()):
        seq_rows = read_jsonl(run_dir / "sequencer_batch_metrics.jsonl")
        exe_rows = read_jsonl(run_dir / "executor_batch_metrics.jsonl")
        sub_rows = read_jsonl(run_dir / "submitter_metrics.json")
        if not seq_rows:
            continue

        coverage = min(len(exe_rows), len(sub_rows)) / len(seq_rows) if seq_rows else 0
        if coverage < min_complete_coverage:
            continue

        exe_by_id = {str(r.get("batch_id")): r for r in exe_rows}
        sub_by_id = {str(r.get("batch_id")): r for r in sub_rows}
        run_id = run_dir.name
        exp_id = run_dir.parent.name
        if experiment_prefix and not exp_id.startswith(experiment_prefix):
            continue
        fallback_size = configured_size_from_name(run_id) or configured_size_from_name(exp_id)

        for seq in seq_rows:
            batch_id = str(seq.get("batch_id"))
            exe = exe_by_id.get(batch_id, {})
            sub = sub_by_id.get(batch_id, {})
            tx_count = int(seq.get("tx_count") or 0)
            if tx_count <= 0:
                continue
            rows.append(
                {
                    "experiment_id": exp_id,
                    "run_id": run_id,
                    "batch_id": batch_id,
                    "configured_batch_size": int(seq.get("configured_max_batch_size") or fallback_size or 0),
                    "actual_tx_count": tx_count,
                    "seal_reason": seq.get("seal_reason"),
                    "oldest_tx_wait_ms": float(seq.get("oldest_tx_wait_ms") or 0),
                    "batch_data_bytes": float(seq.get("batch_data_bytes") or exe.get("batch_data_bytes") or 0),
                    "execution_time_ms": float(exe.get("execution_time_ms") or 0),
                    "proof_time_ms": float(exe.get("proof_time_ms") or 0),
                    "proof_bytes": float(exe.get("proof_bytes") or sub.get("proof_bytes") or 0),
                    "compression_time_ms": float(sub.get("compression_time_ms") or 0),
                    "compressed_bytes": float(sub.get("compressed_bytes") or 0),
                    "blob_count": float(sub.get("blob_count") or 0),
                    "blob_utilization": float(sub.get("blob_utilization") or 0),
                    "l1_gas_used": float(sub.get("l1_gas_used") or 0),
                    "submission_status": sub.get("submission_status"),
                }
            )

    if not rows:
        raise SystemExit(f"No usable joined batch metrics found below {metrics_root}")
    return pd.DataFrame(rows)


def add_costs(df: pd.DataFrame, args: argparse.Namespace) -> pd.DataFrame:
    out = df.copy()
    gas_usd = args.l1_gas_gwei * 1e-9 * args.eth_usd
    out["settlement_cost_usd"] = out["l1_gas_used"] * gas_usd
    out["exec_cost_usd"] = out["execution_time_ms"] / 3_600_000 * args.sequencer_hour_usd
    out["empirical_prover_cost_usd"] = out["proof_time_ms"] / 3_600_000 * args.prover_hour_usd
    out["compress_cost_usd"] = out["compression_time_ms"] / 3_600_000 * args.compressor_hour_usd
    out["empirical_total_cost_usd"] = (
        out["settlement_cost_usd"]
        + out["exec_cost_usd"]
        + out["empirical_prover_cost_usd"]
        + out["compress_cost_usd"]
    )
    out["empirical_cost_per_tx_usd"] = out["empirical_total_cost_usd"] / out["actual_tx_count"]

    calibrated_extra_ms = (
        args.calibrated_prover_ms_per_tx * out["actual_tx_count"]
        + args.calibrated_prover_quadratic_ms * out["actual_tx_count"] ** 2
    )
    out["calibrated_proof_time_ms"] = out["proof_time_ms"] + calibrated_extra_ms
    out["calibrated_prover_cost_usd"] = out["calibrated_proof_time_ms"] / 3_600_000 * args.prover_hour_usd
    out["calibrated_total_cost_usd"] = (
        out["settlement_cost_usd"]
        + out["exec_cost_usd"]
        + out["calibrated_prover_cost_usd"]
        + out["compress_cost_usd"]
    )
    out["calibrated_cost_per_tx_usd"] = out["calibrated_total_cost_usd"] / out["actual_tx_count"]
    return out


def summarize(df: pd.DataFrame) -> pd.DataFrame:
    grouped = df.groupby("configured_batch_size", dropna=False)
    return grouped.agg(
        batches=("batch_id", "count"),
        mean_actual_tx=("actual_tx_count", "mean"),
        p95_actual_tx=("actual_tx_count", lambda s: float(np.percentile(s, 95))),
        size_seals=("seal_reason", lambda s: int((s == "SizeThreshold").sum())),
        timeout_seals=("seal_reason", lambda s: int((s == "Timeout").sum())),
        mean_proof_time_ms=("proof_time_ms", "mean"),
        p95_proof_time_ms=("proof_time_ms", lambda s: float(np.percentile(s, 95))),
        mean_l1_gas=("l1_gas_used", "mean"),
        mean_empirical_total_cost_usd=("empirical_total_cost_usd", "mean"),
        mean_empirical_cost_per_tx_usd=("empirical_cost_per_tx_usd", "mean"),
        mean_calibrated_total_cost_usd=("calibrated_total_cost_usd", "mean"),
        mean_calibrated_cost_per_tx_usd=("calibrated_cost_per_tx_usd", "mean"),
        mean_settlement_cost_usd=("settlement_cost_usd", "mean"),
        mean_empirical_prover_cost_usd=("empirical_prover_cost_usd", "mean"),
        mean_calibrated_prover_cost_usd=("calibrated_prover_cost_usd", "mean"),
    ).reset_index().sort_values("configured_batch_size")


def mark_minimum(summary: pd.DataFrame, col: str) -> str:
    idx = summary[col].idxmin()
    row = summary.loc[idx]
    return f"minimum {col}: configured_batch_size={int(row['configured_batch_size'])}, value={row[col]:.8f}"


def plot_curves(summary: pd.DataFrame, out_dir: Path, args: argparse.Namespace) -> None:
    fig_dir = out_dir / "figures"
    fig_dir.mkdir(parents=True, exist_ok=True)

    x = summary["configured_batch_size"]

    plt.figure(figsize=(9, 5))
    plt.plot(x, summary["mean_empirical_total_cost_usd"], marker="o", label="Measured RollupX total")
    plt.plot(x, summary["mean_calibrated_total_cost_usd"], marker="o", label="With calibrated prover growth")
    plt.xlabel("Configured batch size")
    plt.ylabel("Estimated full batch cost (USD)")
    plt.title("Batch Size vs Estimated Full Batch Cost")
    plt.grid(True, alpha=0.3)
    plt.legend()
    plt.tight_layout()
    plt.savefig(fig_dir / "batch_size_vs_estimated_full_cost.png", dpi=160)
    plt.close()

    plt.figure(figsize=(9, 5))
    plt.plot(x, summary["mean_empirical_cost_per_tx_usd"], marker="o", label="Measured RollupX cost/tx")
    plt.plot(x, summary["mean_calibrated_cost_per_tx_usd"], marker="o", label="With calibrated prover growth cost/tx")
    plt.xlabel("Configured batch size")
    plt.ylabel("Estimated cost per tx (USD)")
    plt.title("Batch Size vs Estimated Cost Per Transaction")
    plt.grid(True, alpha=0.3)
    plt.legend()
    plt.tight_layout()
    plt.savefig(fig_dir / "batch_size_vs_estimated_cost_per_tx.png", dpi=160)
    plt.close()

    plt.figure(figsize=(9, 5))
    plt.stackplot(
        x,
        summary["mean_settlement_cost_usd"],
        summary["mean_calibrated_prover_cost_usd"],
        labels=["L1 settlement", "Prover"],
        alpha=0.85,
    )
    plt.xlabel("Configured batch size")
    plt.ylabel("Mean cost component (USD)")
    plt.title("Cost Components With Calibrated Prover Growth")
    plt.grid(True, alpha=0.3)
    plt.legend(loc="upper left")
    plt.tight_layout()
    plt.savefig(fig_dir / "cost_components_calibrated.png", dpi=160)
    plt.close()

    plt.figure(figsize=(9, 5))
    plt.plot(x, summary["mean_proof_time_ms"], marker="o", label="Measured proof time")
    calibrated_mean = (
        summary["mean_proof_time_ms"]
        + args.calibrated_prover_ms_per_tx * summary["mean_actual_tx"]
        + args.calibrated_prover_quadratic_ms * summary["mean_actual_tx"] ** 2
    )
    plt.plot(x, calibrated_mean, marker="o", label="Calibrated proof-time sensitivity")
    plt.xlabel("Configured batch size")
    plt.ylabel("Proof time (ms)")
    plt.title("Proof Time Used In Cost Model")
    plt.grid(True, alpha=0.3)
    plt.legend()
    plt.tight_layout()
    plt.savefig(fig_dir / "proof_time_cost_input.png", dpi=160)
    plt.close()


def write_report(df: pd.DataFrame, summary: pd.DataFrame, args: argparse.Namespace, out_dir: Path) -> None:
    usable_runs = df["run_id"].nunique()
    usable_batches = len(df)
    report = [
        "# Batch Size vs Estimated Full Cost",
        "",
        f"Input: `{args.metrics_root}`",
        f"Usable runs: {usable_runs}",
        f"Usable joined batches: {usable_batches}",
        "",
        "## Cost Model",
        "",
        f"- L1 gas price: `{args.l1_gas_gwei}` gwei",
        f"- ETH/USD: `${args.eth_usd}`",
        f"- Prover price: `${args.prover_hour_usd}` per prover-hour",
        f"- Sequencer price: `${args.sequencer_hour_usd}` per hour",
        f"- Compressor price: `${args.compressor_hour_usd}` per hour",
        "- Empirical total cost = L1 settlement + measured execution time + measured proof time + measured compression time.",
        (
            "- Calibrated total cost uses measured proof time plus explicit sensitivity: "
            f"`{args.calibrated_prover_ms_per_tx}ms * tx_count + "
            f"{args.calibrated_prover_quadratic_ms}ms * tx_count^2`."
        ),
        "",
        "## Key Results",
        "",
        f"- {mark_minimum(summary, 'mean_empirical_cost_per_tx_usd')}",
        f"- {mark_minimum(summary, 'mean_calibrated_cost_per_tx_usd')}",
        "- Use the calibrated curve only as a stated prover-scaling sensitivity, not as direct RollupX measurement.",
        "- If the calibrated total batch cost rises at larger batch sizes while cost/tx bottoms out, that is the economic-sealer turning point: waiting longer adds prover cost faster than it saves fixed L1 cost.",
        "",
        "## Outputs",
        "",
        "- `joined_batch_costs.csv`: one row per sealed batch with cost fields.",
        "- `batch_size_cost_summary.csv`: grouped summary by configured batch size.",
        "- `figures/batch_size_vs_estimated_full_cost.png`",
        "- `figures/batch_size_vs_estimated_cost_per_tx.png`",
        "- `figures/cost_components_calibrated.png`",
        "- `figures/proof_time_cost_input.png`",
    ]
    (out_dir / "cost_curve_report.md").write_text("\n".join(report) + "\n", encoding="utf-8")


def main() -> None:
    args = parse_args()
    out_dir = args.out or (args.metrics_root / "cost_curve_analysis")
    out_dir.mkdir(parents=True, exist_ok=True)
    df = add_costs(load_batches(args.metrics_root, args.min_complete_coverage, args.experiment_prefix), args)
    summary = summarize(df)
    df.to_csv(out_dir / "joined_batch_costs.csv", index=False)
    summary.to_csv(out_dir / "batch_size_cost_summary.csv", index=False)
    plot_curves(summary, out_dir, args)
    write_report(df, summary, args, out_dir)
    print(f"Wrote cost curve analysis to {out_dir}")
    print(mark_minimum(summary, "mean_empirical_cost_per_tx_usd"))
    print(mark_minimum(summary, "mean_calibrated_cost_per_tx_usd"))


if __name__ == "__main__":
    main()
