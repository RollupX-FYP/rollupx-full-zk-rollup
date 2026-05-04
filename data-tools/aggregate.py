"""
aggregate.py: merge run-level and batch-level benchmark outputs.
"""

import argparse
import glob
import json
import os
import statistics

import pandas as pd


def _load_json(path: str) -> dict:
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def _load_jsonl(path: str) -> list[dict]:
    rows = []
    with open(path, encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                rows.append(json.loads(line))
            except json.JSONDecodeError:
                pass
    return rows


def _percentile(data: list[float], pct: float) -> float:
    if not data:
        return 0.0
    values = sorted(data)
    k = (len(values) - 1) * pct / 100
    lo = int(k)
    hi = min(lo + 1, len(values) - 1)
    return values[lo] + (values[hi] - values[lo]) * (k - lo)


def _load_run(run_dir: str) -> dict | None:
    run_id = os.path.basename(run_dir)
    exp_id = os.path.basename(os.path.dirname(run_dir))
    status_file = os.path.join(run_dir, "run_status.json")
    status = "unknown"
    if os.path.exists(status_file):
        status = _load_json(status_file).get("status", "unknown")

    row = {"experiment_id": exp_id, "run_id": run_id, "run_status": status}
    meta_file = os.path.join(run_dir, "run_metadata.json")
    if os.path.exists(meta_file):
        meta = _load_json(meta_file)
        cfg = meta.get("config_snapshot", {})
        row.update(
            {
                "git_commit": meta.get("git_commit", "unknown"),
                "timestamp": meta.get("timestamp_start", ""),
                "batch_size": cfg.get("batch_size", ""),
                "timeout_ms": cfg.get("timeout_ms", ""),
                "policy": cfg.get("policy", ""),
                "da_mode": cfg.get("da_mode", ""),
                "prover": cfg.get("prover", ""),
                "rate_tps": cfg.get("rate_tps", ""),
                "tx_mix": cfg.get("tx_mix", ""),
                "seed": cfg.get("seed", ""),
            }
        )

    wl_file = os.path.join(run_dir, f"workload_{exp_id}.json")
    if os.path.exists(wl_file):
        wl = _load_json(wl_file)
        details = wl.get("details", {})
        duration = details.get("duration", 1)
        row.update(
            {
                "tps_offered": wl.get("rate", 0),
                "total_txs": details.get("total_txs", 0),
                "success_txs": details.get("successful_txs", 0),
                "failed_txs": details.get("failed_txs", 0),
                "duration_s_actual": duration,
                "tps_accepted": details.get("successful_txs", 0) / max(duration, 1),
            }
        )

    ex_file = os.path.join(run_dir, f"executor_{exp_id}.json")
    if os.path.exists(ex_file):
        ex = _load_json(ex_file)
        ex_times = ex.get("execution_times_ms", [])
        proof_times = ex.get("proof_generation_times_ms", [])
        row.update(
            {
                "batch_count": ex.get("batch_count", 0),
                "avg_exec_ms": statistics.mean(ex_times) if ex_times else 0,
                "p95_exec_ms": _percentile(ex_times, 95),
                "avg_prove_ms": statistics.mean(proof_times) if proof_times else 0,
                "p95_prove_ms": _percentile(proof_times, 95),
            }
        )

    sub_file = os.path.join(run_dir, "submitter_metrics.json")
    if os.path.exists(sub_file):
        batches = _load_jsonl(sub_file)
        if batches:
            l2_l1 = [b.get("l2_l1_latency_ms", 0) or 0 for b in batches]
            gas_used = [b.get("l1_gas_used", 0) or 0 for b in batches]
            blobs = [b.get("blob_utilization", 0) or 0 for b in batches]
            row.update(
                {
                    "avg_l2_l1_ms": statistics.mean(l2_l1),
                    "p95_l2_l1_ms": _percentile(l2_l1, 95),
                    "avg_l1_gas_used": statistics.mean(gas_used) if gas_used else 0,
                    "avg_blob_utilization": statistics.mean(blobs) if blobs else 0,
                    "total_batches": len(batches),
                }
            )
    return row


def _load_batch_rows(run_dir: str) -> list[dict]:
    run_id = os.path.basename(run_dir)
    exp_id = os.path.basename(os.path.dirname(run_dir))
    seq_file = os.path.join(run_dir, "sequencer_batch_metrics.jsonl")
    ex_file = os.path.join(run_dir, "executor_batch_metrics.jsonl")
    sub_file = os.path.join(run_dir, "submitter_metrics.json")
    seq_rows = _load_jsonl(seq_file) if os.path.exists(seq_file) else []
    ex_rows = _load_jsonl(ex_file) if os.path.exists(ex_file) else []
    sub_rows = _load_jsonl(sub_file) if os.path.exists(sub_file) else []
    ex_by_batch = {str(r.get("batch_id")): r for r in ex_rows}
    sub_by_batch = {str(r.get("batch_id")): r for r in sub_rows}
    rows = []
    for seq in seq_rows:
        bid = str(seq.get("batch_id"))
        ex = ex_by_batch.get(bid, {})
        sub = sub_by_batch.get(bid, {})
        rows.append(
            {
                "experiment_id": exp_id,
                "run_id": run_id,
                "batch_id": bid,
                "seal_reason": seq.get("seal_reason"),
                "tx_count": seq.get("tx_count"),
                "batch_data_bytes": seq.get("batch_data_bytes"),
                "oldest_tx_wait_ms": seq.get("oldest_tx_wait_ms"),
                "total_gas_limit": seq.get("total_gas_limit"),
                "fee_proxy_wei_sequencer": seq.get("fee_proxy_wei"),
                "state_diff_count": ex.get("state_diff_count"),
                "state_diff_bytes": ex.get("state_diff_bytes"),
                "unique_touched_accounts": ex.get("unique_touched_accounts"),
                "repeated_touched_accounts": ex.get("repeated_touched_accounts"),
                "execution_time_ms": ex.get("execution_time_ms"),
                "proof_time_ms": ex.get("proof_time_ms"),
                "proof_bytes": ex.get("proof_bytes"),
                "journal_bytes": ex.get("journal_bytes"),
                "da_mode": sub.get("da_mode"),
                "compressed_bytes": sub.get("compressed_bytes"),
                "compression_ratio": sub.get("compression_ratio"),
                "blob_count": sub.get("blob_count"),
                "blob_utilization": sub.get("blob_utilization"),
                "l1_gas_used": sub.get("l1_gas_used"),
                "l1_latency_ms": sub.get("l2_l1_latency_ms"),
                "fee_proxy_wei_submitter": sub.get("fee_proxy_wei"),
            }
        )
    return rows


def aggregate(metrics_root: str, output: str, include_failed: bool = False) -> pd.DataFrame:
    run_dirs = [
        os.path.dirname(p)
        for p in sorted(glob.glob(os.path.join(metrics_root, "*", "*", "run_status.json")))
    ]
    rows = []
    batch_rows = []
    for run_dir in run_dirs:
        row = _load_run(run_dir)
        if row is None:
            continue
        if row.get("run_status") != "pass" and not include_failed:
            continue
        rows.append(row)
        batch_rows.extend(_load_batch_rows(run_dir))

    if not rows:
        return pd.DataFrame()

    df = pd.DataFrame(rows)
    os.makedirs(os.path.dirname(output) if os.path.dirname(output) else ".", exist_ok=True)
    df.to_csv(output, index=False)
    if batch_rows:
        pd.DataFrame(batch_rows).to_csv(
            os.path.join(os.path.dirname(output), "all_batch_results.csv"), index=False
        )
    return df


def main() -> None:
    parser = argparse.ArgumentParser(description="Aggregate RollupX benchmark metrics")
    parser.add_argument("--metrics_root", default=os.environ.get("METRICS_ROOT", "metrics"))
    parser.add_argument("--output", default="metrics/all_results.csv")
    parser.add_argument("--include_failed", action="store_true")
    args = parser.parse_args()
    df = aggregate(args.metrics_root, args.output, args.include_failed)
    if not df.empty:
        print(f"Aggregated {len(df)} runs to {args.output}")


if __name__ == "__main__":
    main()
