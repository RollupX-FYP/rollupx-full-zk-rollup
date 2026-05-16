#!/usr/bin/env python3
"""Compare FCFS vs BlobPacking sequencer metrics for quick scheduler studies."""

from __future__ import annotations

import argparse
import csv
import json
from collections import Counter
from pathlib import Path
from statistics import mean


def read_jsonl(path: Path) -> list[dict]:
    rows: list[dict] = []
    if not path.exists():
        return rows
    with path.open("r", encoding="utf-8") as handle:
        for line in handle:
            line = line.strip()
            if not line:
                continue
            rows.append(json.loads(line))
    return rows


def latest_run(metrics_root: Path, experiment_id: str) -> Path:
    candidates = sorted((metrics_root / experiment_id).glob("*"), key=lambda p: p.stat().st_mtime, reverse=True)
    if not candidates:
        raise SystemExit(f"No run directories found for {experiment_id} under {metrics_root}")
    return candidates[0]


def summarize_run(label: str, run_dir: Path) -> tuple[dict, list[dict]]:
    seq = read_jsonl(run_dir / "sequencer_batch_metrics.jsonl")
    if not seq:
        raise SystemExit(f"{run_dir} has no sequencer_batch_metrics.jsonl rows")

    reasons = Counter(str(row.get("blob_low_fill_reason")) for row in seq)
    seal_reasons = Counter(str(row.get("seal_reason")) for row in seq)
    tx_counts = [int(row.get("tx_count", 0)) for row in seq]
    blob_utils = [float(row.get("blob_utilization", 0.0)) for row in seq]
    wait_p95 = [float(row.get("wait_time_p95_ms", 0.0)) for row in seq]
    batch_bytes = [int(row.get("estimated_batch_bytes", 0)) for row in seq]

    summary = {
        "label": label,
        "run_dir": str(run_dir),
        "scheduling_policy": ",".join(sorted({str(row.get("scheduling_policy")) for row in seq})),
        "batch_policy": ",".join(sorted({str(row.get("batch_policy")) for row in seq})),
        "batches": len(seq),
        "total_txs_batched": sum(tx_counts),
        "avg_tx_count": mean(tx_counts),
        "avg_blob_utilization": mean(blob_utils),
        "max_blob_utilization": max(blob_utils),
        "avg_estimated_batch_bytes": mean(batch_bytes),
        "avg_wait_p95_ms": mean(wait_p95),
        "blob_selected_bytes_sum": sum(int(row.get("blob_selected_bytes", 0)) for row in seq),
        "blob_eligible_bytes_sum": sum(int(row.get("blob_eligible_bytes", 0)) for row in seq),
        "blob_eligible_tx_count_sum": sum(int(row.get("blob_eligible_tx_count", 0)) for row in seq),
        "nonce_gap_count_sum": sum(int(row.get("blob_ineligible_nonce_gap_count", 0)) for row in seq),
        "truncated_senders_sum": sum(int(row.get("blob_nonce_chain_truncated_senders", 0)) for row in seq),
        "low_fill_reasons": json.dumps(dict(sorted(reasons.items()))),
        "seal_reasons": json.dumps(dict(sorted(seal_reasons.items()))),
    }

    batch_rows: list[dict] = []
    for row in seq:
        batch_rows.append(
            {
                "label": label,
                "batch_id": row.get("batch_id"),
                "scheduling_policy": row.get("scheduling_policy"),
                "seal_reason": row.get("seal_reason"),
                "tx_count": row.get("tx_count"),
                "estimated_batch_bytes": row.get("estimated_batch_bytes"),
                "blob_utilization": row.get("blob_utilization"),
                "wait_time_p95_ms": row.get("wait_time_p95_ms"),
                "blob_selected_bytes": row.get("blob_selected_bytes"),
                "blob_eligible_bytes": row.get("blob_eligible_bytes"),
                "blob_eligible_tx_count": row.get("blob_eligible_tx_count"),
                "blob_ineligible_nonce_gap_count": row.get("blob_ineligible_nonce_gap_count"),
                "blob_low_fill_reason": row.get("blob_low_fill_reason"),
            }
        )
    return summary, batch_rows


def write_csv(path: Path, rows: list[dict]) -> None:
    if not rows:
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=list(rows[0].keys()))
        writer.writeheader()
        writer.writerows(rows)


def write_plots(out_dir: Path, summaries: list[dict], batch_rows: list[dict]) -> None:
    try:
        import matplotlib.pyplot as plt
    except ImportError:
        (out_dir / "PLOTS_NOT_GENERATED.txt").write_text(
            "Install matplotlib to generate plots: python3 -m pip install matplotlib\n",
            encoding="utf-8",
        )
        return

    labels = [row["label"] for row in summaries]

    def bar(metric: str, title: str, ylabel: str, filename: str) -> None:
        values = [float(row[metric]) for row in summaries]
        plt.figure(figsize=(7, 4))
        plt.bar(labels, values, color=["#4c78a8", "#f58518"][: len(labels)])
        plt.title(title)
        plt.ylabel(ylabel)
        plt.tight_layout()
        plt.savefig(out_dir / filename, dpi=160)
        plt.close()

    bar("avg_blob_utilization", "Average Blob Utilization Proxy", "utilization", "avg_blob_utilization.png")
    bar("avg_estimated_batch_bytes", "Average Estimated Batch Bytes", "bytes", "avg_batch_bytes.png")
    bar("avg_wait_p95_ms", "Average Batch p95 Wait", "milliseconds", "avg_wait_p95_ms.png")

    plt.figure(figsize=(8, 4))
    for label in labels:
        ys = [float(row["blob_utilization"]) for row in batch_rows if row["label"] == label]
        xs = list(range(1, len(ys) + 1))
        plt.plot(xs, ys, marker="o", linewidth=1, label=label)
    plt.title("Blob Utilization by Batch")
    plt.xlabel("batch index")
    plt.ylabel("utilization")
    plt.legend()
    plt.tight_layout()
    plt.savefig(out_dir / "blob_utilization_by_batch.png", dpi=160)
    plt.close()


def write_markdown(path: Path, summaries: list[dict]) -> None:
    lines = [
        "# Blob Scheduler Effectiveness Report",
        "",
        "| Metric | " + " | ".join(row["label"] for row in summaries) + " |",
        "|---|" + "|".join("---" for _ in summaries) + "|",
    ]
    metrics = [
        ("Policy", "scheduling_policy"),
        ("Batches", "batches"),
        ("Total txs batched", "total_txs_batched"),
        ("Average txs/batch", "avg_tx_count"),
        ("Average blob utilization", "avg_blob_utilization"),
        ("Max blob utilization", "max_blob_utilization"),
        ("Average batch bytes", "avg_estimated_batch_bytes"),
        ("Average p95 wait ms", "avg_wait_p95_ms"),
        ("Blob selected bytes sum", "blob_selected_bytes_sum"),
        ("Eligible tx count sum", "blob_eligible_tx_count_sum"),
        ("Nonce gap count sum", "nonce_gap_count_sum"),
        ("Low fill reasons", "low_fill_reasons"),
        ("Seal reasons", "seal_reasons"),
    ]
    for label, key in metrics:
        values = [str(row[key]) for row in summaries]
        lines.append("| " + label + " | " + " | ".join(values) + " |")
    lines.extend(
        [
            "",
            "## Interpretation Guide",
            "",
            "- BlobPacking is effective if it raises average/max blob utilization or batch bytes without increasing nonce gaps, failures, or wait time too much.",
            "- If utilization does not improve, inspect whether the transaction size model actually varies enough for packing to matter.",
            "- If `nonce_gap_count_sum` is high, nonce constraints are limiting packing freedom.",
            "- Use strict executor/submitter runs only after the sequencer-level scheduler behavior looks promising.",
        ]
    )
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--metrics-root", default="metrics", type=Path)
    parser.add_argument("--fcfs-exp", required=True)
    parser.add_argument("--blob-exp", required=True)
    parser.add_argument("--out-dir", default="metrics/blob_scheduler_effectiveness", type=Path)
    args = parser.parse_args()

    args.out_dir.mkdir(parents=True, exist_ok=True)
    specs = [
        ("FCFS", latest_run(args.metrics_root, args.fcfs_exp)),
        ("BlobPacking", latest_run(args.metrics_root, args.blob_exp)),
    ]

    summaries: list[dict] = []
    all_batches: list[dict] = []
    for label, run_dir in specs:
        summary, batch_rows = summarize_run(label, run_dir)
        summaries.append(summary)
        all_batches.extend(batch_rows)

    write_csv(args.out_dir / "summary.csv", summaries)
    write_csv(args.out_dir / "batch_metrics.csv", all_batches)
    write_markdown(args.out_dir / "report.md", summaries)
    write_plots(args.out_dir, summaries, all_batches)

    print(f"Wrote {args.out_dir / 'summary.csv'}")
    print(f"Wrote {args.out_dir / 'batch_metrics.csv'}")
    print(f"Wrote {args.out_dir / 'report.md'}")


if __name__ == "__main__":
    main()
