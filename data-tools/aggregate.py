"""
aggregate.py — merge all per-run metrics JSON files into a single DataFrame.

Reads:
  <metrics_root>/<exp_id>/<run_id>/workload_<exp_id>.json
  <metrics_root>/<exp_id>/<run_id>/executor_<exp_id>.json
  <metrics_root>/<exp_id>/<run_id>/submitter_metrics.json  (JSONL)
  <metrics_root>/<exp_id>/<run_id>/run_metadata.json
  <metrics_root>/<exp_id>/<run_id>/run_status.json

Writes:
  <metrics_root>/all_results.csv
"""

import argparse
import glob
import json
import os
import statistics

import pandas as pd


# ── loaders (per single run directory) ───────────────────────────────────────

def _load_json(path: str) -> dict:
    with open(path) as f:
        return json.load(f)


def _load_jsonl(path: str) -> list[dict]:
    rows = []
    with open(path) as f:
        for line in f:
            line = line.strip()
            if line:
                try:
                    rows.append(json.loads(line))
                except json.JSONDecodeError:
                    pass
    return rows


def _load_run(run_dir: str) -> dict | None:
    """
    Load one run directory and return a flat dict of all metrics.
    Returns None if the run is missing required files or is marked failed.
    """
    # identify exp_id and run_id from path
    run_id = os.path.basename(run_dir)
    exp_id = os.path.basename(os.path.dirname(run_dir))

    # ── run_status ────────────────────────────────────────────────────────────
    status_file = os.path.join(run_dir, "run_status.json")
    if os.path.exists(status_file):
        status_data = _load_json(status_file)
        status = status_data.get("status", "unknown")
    else:
        status = "unknown"

    row: dict = {
        "experiment_id": exp_id,
        "run_id":        run_id,
        "run_status":    status,
    }

    # ── run_metadata ──────────────────────────────────────────────────────────
    meta_file = os.path.join(run_dir, "run_metadata.json")
    if os.path.exists(meta_file):
        meta = _load_json(meta_file)
        cfg  = meta.get("config_snapshot", {})
        row.update({
            "git_commit":    meta.get("git_commit", "unknown"),
            "timestamp":     meta.get("timestamp_start", ""),
            "batch_size":    cfg.get("batch_size", ""),
            "timeout_ms":    cfg.get("timeout_ms", ""),
            "policy":        cfg.get("policy", ""),
            "da_mode":       cfg.get("da_mode", ""),
            "prover":        cfg.get("prover", ""),
            "rate_tps":      cfg.get("rate_tps", ""),
            "tx_mix":        cfg.get("tx_mix", ""),
            "seed":          cfg.get("seed", ""),
        })

    # ── workload metrics ──────────────────────────────────────────────────────
    wl_files = glob.glob(os.path.join(run_dir, f"workload_{exp_id}.json"))
    _wl_duration = 120
    if wl_files:
        wl = _load_json(wl_files[0])
        details = wl.get("details", {})
        _wl_duration = details.get("duration", 120)
        row.update({
            "tps_offered":   wl.get("rate", 0),
            "total_txs":     details.get("total_txs", 0),
            "success_txs":   details.get("successful_txs", 0),
            "failed_txs":    details.get("failed_txs", 0),
            "tx_count_A":    details.get("type_counts", {}).get("A", 0),
            "tx_count_B":    details.get("type_counts", {}).get("B", 0),
            "tx_count_C":    details.get("type_counts", {}).get("C", 0),
            "duration_s_actual": _wl_duration,
        })
        # compute tps_accepted from success count and duration
        duration = details.get("duration", 1)
        row["tps_accepted"] = details.get("successful_txs", 0) / max(duration, 1)

    # ── executor metrics ──────────────────────────────────────────────────────
    ex_files = glob.glob(os.path.join(run_dir, f"executor_{exp_id}.json"))
    if ex_files:
        ex = _load_json(ex_files[0])
        prove_times = ex.get("proof_generation_times_ms", [])
        row.update({
            "prover_backend":    ex.get("prover_backend", "unknown"),
            "avg_prove_ms":      statistics.mean(prove_times) if prove_times else 0,
            "p50_prove_ms":      statistics.median(prove_times) if prove_times else 0,
            "p95_prove_ms":      _percentile(prove_times, 95),
            "max_prove_ms":      max(prove_times) if prove_times else 0,
            "batch_count":       ex.get("batch_count", 0),
        })
        bc = ex.get("batch_count", 1)
        # tps_committed: total proved txs / duration
        total_proved = ex.get("total_proved_txs")  # None if absent
        duration = ex.get("duration_s", 120)
        if total_proved is not None:
            row["tps_committed"] = total_proved / max(duration, 1)
        else:
            row["tps_committed"] = None  # executor did not report; do not fabricate

    # ── submitter metrics (JSONL — one line per batch) ────────────────────────
    sub_file = os.path.join(run_dir, "submitter_metrics.json")
    if os.path.exists(sub_file):
        batches = _load_jsonl(sub_file)
        if batches:
            l2_l1    = [b.get("l2_l1_latency_ms", 0) or 0  for b in batches]
            gas_s    = [b.get("gas_saved", 0) or 0          for b in batches]
            comp_r   = [b.get("compression_ratio", 0) or 0  for b in batches]
            gas_used = [b.get("gas_used_per_batch", 0) or 0 for b in batches]
            gas_tx   = [b.get("gas_used_per_tx", 0) or 0    for b in batches]
            cdata_b  = [b.get("calldata_bytes", 0) or 0      for b in batches]
            comp_b   = [b.get("compressed_bytes", 0) or 0    for b in batches]
            retries  = [b.get("retries", 0) or 0             for b in batches]
            failed   = [b.get("failed", False)               for b in batches]

            _wl_duration = row.get("duration_s_actual") or 120  # set earlier from workload JSON
            row.update({
                "da_mode_actual":       batches[0].get("da_mode", "unknown"),
                "avg_l2_l1_ms":         statistics.mean(l2_l1),
                "p50_l2_l1_ms":         statistics.median(l2_l1),
                "p95_l2_l1_ms":         _percentile(l2_l1, 95),
                "p99_l2_l1_ms":         _percentile(l2_l1, 99),
                "avg_gas_saved":        statistics.mean(gas_s),
                "avg_comp_ratio":       statistics.mean(comp_r),
                "avg_gas_per_batch":    statistics.mean(gas_used),
                "avg_gas_per_tx":       statistics.mean(gas_tx),
                "avg_calldata_bytes":   statistics.mean(cdata_b),
                "avg_compressed_bytes": statistics.mean(comp_b),
                "total_retries":        sum(retries),
                "failed_batches":       sum(1 for f in failed if f),
                "total_batches":        len(batches),
                "tps_finalized":        sum(
                    b.get("tx_count", 0) for b in batches
                ) / max(_wl_duration, 1),
            })

    # ── tx-level log for fairness metrics ────────────────────────────────────
    tx_log = os.path.join(run_dir, f"tx_log_{run_id}.csv")
    if os.path.exists(tx_log):
        try:
            import csv
            with open(tx_log) as f:
                reader = csv.DictReader(f)
                tx_rows = list(reader)

            class_means = []
            for tx_type in ["A", "B", "C"]:
                lats = [
                    float(r["latency"]) * 1000
                    for r in tx_rows
                    if r.get("tx_type") == tx_type and r.get("status") == "success"
                ]
                row[f"p95_latency_type{tx_type}_ms"] = _percentile(lats, 95)
                row[f"avg_latency_type{tx_type}_ms"] = statistics.mean(lats) if lats else 0
                if lats:
                    class_means.append(statistics.mean(lats))

            row["jains_fairness"] = _jains_index(class_means) if len(class_means) >= 2 else None

            # starvation: still computed over all transactions
            all_lats = [
                float(r["latency"]) * 1000
                for r in tx_rows
                if r.get("status") == "success"
            ]
            if all_lats:
                mean_lat = statistics.mean(all_lats)
                row["starvation_count"] = sum(1 for l in all_lats if l > 3 * mean_lat)
            else:
                row["starvation_count"] = 0

        except Exception as e:
            print(f"  [WARN] Could not load tx_log {tx_log}: {e}")

    return row


def _percentile(data: list[float], pct: float) -> float:
    if not data:
        return 0.0
    data_sorted = sorted(data)
    k = (len(data_sorted) - 1) * pct / 100
    lo, hi = int(k), min(int(k) + 1, len(data_sorted) - 1)
    return data_sorted[lo] + (data_sorted[hi] - data_sorted[lo]) * (k - lo)


def _jains_index(values: list[float]) -> float:
    """Jain's Fairness Index: (Σx)² / (n · Σx²)"""
    if not values:
        return 0.0
    n   = len(values)
    s   = sum(values)
    sq  = sum(v * v for v in values)
    return (s * s) / (n * sq) if sq > 0 else 1.0


# ── main aggregation ──────────────────────────────────────────────────────────

def aggregate(metrics_root: str, output: str, include_failed: bool = False) -> pd.DataFrame:
    """
    Walk metrics_root, load every run directory, and merge into a DataFrame.

    Parameters
    ----------
    metrics_root   : root metrics directory (contains exp_id subdirs)
    output         : path for output CSV
    include_failed : whether to include runs with status != 'pass'
    """
    pattern = os.path.join(metrics_root, "*", "*", "run_status.json")
    run_dirs = [os.path.dirname(p) for p in sorted(glob.glob(pattern))]

    if not run_dirs:
        # also accept flat layout: metrics/<exp_id>/run_status.json
        pattern2 = os.path.join(metrics_root, "*", "run_status.json")
        run_dirs = [os.path.dirname(p) for p in sorted(glob.glob(pattern2))]

    print(f"[aggregate] found {len(run_dirs)} run directories under {metrics_root}")

    rows = []
    n_failed = 0

    for run_dir in run_dirs:
        row = _load_run(run_dir)
        if row is None:
            continue

        if row.get("run_status") != "pass" and not include_failed:
            n_failed += 1
            print(f"  [SKIP] {row['run_id']} — status={row.get('run_status')}")
            continue

        rows.append(row)

    if not rows:
        print("[aggregate] No valid runs found. Exiting.")
        return pd.DataFrame()

    df = pd.DataFrame(rows)

    os.makedirs(os.path.dirname(output) if os.path.dirname(output) else ".", exist_ok=True)
    df.to_csv(output, index=False)

    print(f"[aggregate] {len(df)} valid runs  |  {n_failed} skipped")
    print(f"[aggregate] written → {output}")
    return df


# ── CLI ───────────────────────────────────────────────────────────────────────

def main():
    p = argparse.ArgumentParser(description="Aggregate RollupX benchmark metrics")
    p.add_argument("--metrics_root", default=os.environ.get("METRICS_ROOT", "metrics"),
                   help="Root metrics directory")
    p.add_argument("--output",  default="metrics/all_results.csv",
                   help="Output CSV path")
    p.add_argument("--include_failed", action="store_true",
                   help="Include failed/partial runs in output")
    args = p.parse_args()

    df = aggregate(args.metrics_root, args.output, args.include_failed)
    if not df.empty:
        print("\nColumn summary:")
        print(df.dtypes.to_string())
        print(f"\nShape: {df.shape}")


if __name__ == "__main__":
    main()
