"""
plots/latency_cdf.py — CDF of per-batch L2→L1 latency samples.

Reads per-batch submitter_metrics.json files directly (not the aggregated CSV)
so that the CDF reflects actual sample distribution, not experiment averages.

Writes: figures/latency_cdf_<factor>.png
"""

import argparse
import glob
import json
import os
import sys

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np


def load_batch_latencies(metrics_root: str) -> dict[str, list[float]]:
    """
    Returns {experiment_id: [l2_l1_latency_ms, ...]} from all submitter JSONL files.
    """
    result: dict[str, list[float]] = {}

    for path in sorted(glob.glob(
        os.path.join(metrics_root, "**", "submitter_metrics.json"), recursive=True
    )):
        with open(path) as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                try:
                    rec = json.loads(line)
                    exp_id = rec.get("experiment_id", "unknown")
                    lat = rec.get("l2_l1_latency_ms")
                    if lat is not None and float(lat) > 0:
                        result.setdefault(exp_id, []).append(float(lat))
                except Exception:
                    pass

    return result


def plot_cdf(latencies: dict[str, list[float]], output_dir: str, title_suffix: str = ""):
    os.makedirs(output_dir, exist_ok=True)

    if not latencies:
        print("[latency_cdf] No latency data found")
        return

    fig, ax = plt.subplots(figsize=(10, 6))
    colors = plt.cm.tab10.colors

    for idx, (exp_id, vals) in enumerate(sorted(latencies.items())):
        if not vals:
            continue
        vals_sorted = sorted(vals)
        n = len(vals_sorted)
        cdf = np.arange(1, n + 1) / n
        ax.plot(vals_sorted, cdf, label=f"{exp_id} (n={n})",
                color=colors[idx % len(colors)], linewidth=1.5, alpha=0.85)

    # reference lines
    for pct in [0.50, 0.95, 0.99]:
        ax.axhline(pct, color="grey", linestyle=":", linewidth=0.8, alpha=0.6)
        ax.text(ax.get_xlim()[0] if ax.get_xlim()[0] > 0 else 0,
                pct + 0.005, f"P{int(pct*100)}", fontsize=7, color="grey")

    ax.set_xlabel("L2→L1 Latency per Batch (ms)")
    ax.set_ylabel("CDF")
    ax.set_title(f"Latency CDF — per-batch samples{' — ' + title_suffix if title_suffix else ''}")
    ax.legend(fontsize=7, ncol=2)
    ax.grid(True, alpha=0.3)
    ax.set_ylim(0, 1.05)
    ax.set_xlim(left=0)
    fig.tight_layout()

    slug = title_suffix.lower().replace(" ", "_") or "all"
    out = os.path.join(output_dir, f"latency_cdf_{slug}.png")
    fig.savefig(out, dpi=150)
    plt.close(fig)
    print(f"[latency_cdf] saved → {out}")


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--metrics_root", default=os.environ.get("METRICS_ROOT", "metrics"))
    p.add_argument("--output_dir",   default="figures")
    p.add_argument("--filter",       default=None,
                   help="Only include experiment IDs containing this substring")
    args = p.parse_args()

    latencies = load_batch_latencies(args.metrics_root)

    if args.filter:
        latencies = {k: v for k, v in latencies.items() if args.filter in k}

    if not latencies:
        print("[latency_cdf] No data. Check --metrics_root path.")
        sys.exit(0)

    plot_cdf(latencies, args.output_dir, title_suffix=args.filter or "")


if __name__ == "__main__":
    main()
