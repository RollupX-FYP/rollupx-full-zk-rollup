"""
aggregate.py: merge run-level and batch-level benchmark outputs.
"""

import argparse
import csv
import glob
import json
import os
import re
import statistics

import pandas as pd


def _load_json(path: str) -> dict:
    with open(path, encoding="utf-8") as f:
        raw = f.read()
    try:
        return json.loads(raw)
    except json.JSONDecodeError:
        # Some older resource metric files contain numbers like `.47`.
        sanitized = re.sub(r'(:\s*)(\.\d+)', r"\g<1>0\2", raw)
        return json.loads(sanitized)


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


def _load_csv(path: str) -> list[dict]:
    with open(path, encoding="utf-8", newline="") as f:
        return list(csv.DictReader(f))


def _mean(values: list[float]) -> float:
    return statistics.mean(values) if values else 0.0


def _percentile(data: list[float], pct: float) -> float:
    if not data:
        return 0.0
    values = sorted(data)
    k = (len(values) - 1) * pct / 100
    lo = int(k)
    hi = min(lo + 1, len(values) - 1)
    return values[lo] + (values[hi] - values[lo]) * (k - lo)


def _to_float(value, default: float = 0.0) -> float:
    try:
        if value in (None, ""):
            return default
        return float(value)
    except (TypeError, ValueError):
        return default


def _load_run(run_dir: str) -> dict | None:
    run_id = os.path.basename(run_dir)
    exp_id = os.path.basename(os.path.dirname(run_dir))
    status_file = os.path.join(run_dir, "run_status.json")
    status = "unknown"
    status_payload: dict[str, object] = {}
    if os.path.exists(status_file):
        status_payload = _load_json(status_file)
        status = status_payload.get("status", "unknown")

    row = {"experiment_id": exp_id, "run_id": run_id, "run_status": status}
    meta_file = os.path.join(run_dir, "run_metadata.json")
    duration_s = 0.0
    if os.path.exists(meta_file):
        meta = _load_json(meta_file)
        cfg = meta.get("config_snapshot", {})
        duration_s = _to_float(cfg.get("duration_s"))
        row.update(
            {
                "git_commit": meta.get("git_commit", "unknown"),
                "timestamp": meta.get("timestamp_start", ""),
                "batch_size": _to_float(cfg.get("batch_size")),
                "min_batch_size": _to_float(cfg.get("min_batch_size")),
                "timeout_ms": _to_float(cfg.get("timeout_ms")),
                "batch_policy": cfg.get("batch_policy", ""),
                "adaptive_low_load_threshold": _to_float(cfg.get("adaptive_low_load_threshold")),
                "adaptive_medium_load_threshold": _to_float(cfg.get("adaptive_medium_load_threshold")),
                "adaptive_small_batch_size": _to_float(cfg.get("adaptive_small_batch_size")),
                "adaptive_medium_batch_size": _to_float(cfg.get("adaptive_medium_batch_size")),
                "adaptive_large_batch_size": _to_float(cfg.get("adaptive_large_batch_size")),
                "blob_target_bytes": _to_float(cfg.get("blob_target_bytes")),
                "blob_fill_target": _to_float(cfg.get("blob_fill_target")),
                "policy": cfg.get("policy", ""),
                "da_mode": cfg.get("da_mode", ""),
                "prover": cfg.get("prover", ""),
                "prover_backend": cfg.get("prover_backend", ""),
                "require_real_proofs": cfg.get("require_real_proofs", ""),
                "allow_proof_fallback": cfg.get("allow_proof_fallback", ""),
                "allow_unsigned_user_txs": cfg.get("allow_unsigned_user_txs", ""),
                "comm_mode": cfg.get("comm_mode", ""),
                "eth_price_usd": _to_float(cfg.get("eth_price_usd")),
                "regular_gas_price_gwei": _to_float(cfg.get("regular_gas_price_gwei")),
                "blob_gas_price_gwei": _to_float(cfg.get("blob_gas_price_gwei")),
                "rate_tps": _to_float(cfg.get("rate_tps")),
                "workload_burst_enabled": cfg.get("workload_burst_enabled", "0"),
                "workload_burst_rate_tps": _to_float(cfg.get("workload_burst_rate_tps")),
                "hardhat_mining_interval": _to_float(cfg.get("hardhat_mining_interval")),
                "sequencer_executor_publish_retries": _to_float(cfg.get("SEQUENCER_EXECUTOR_PUBLISH_RETRIES", cfg.get("sequencer_executor_publish_retries"))),
                "sequencer_executor_publish_timeout_ms": _to_float(cfg.get("SEQUENCER_EXECUTOR_PUBLISH_TIMEOUT_MS", cfg.get("sequencer_executor_publish_timeout_ms"))),
                "tx_mix": cfg.get("tx_mix", ""),
                "seed": cfg.get("seed", ""),
            }
        )

    wl_file = os.path.join(run_dir, f"workload_{exp_id}.json")
    if os.path.exists(wl_file):
        wl = _load_json(wl_file)
        details = wl.get("details", {})
        latency_metrics = wl.get("latency_metrics", {})
        duration = _to_float(details.get("duration"), duration_s or 1)
        total_txs = int(details.get("total_txs", 0) or 0)
        success_txs = int(details.get("successful_txs", 0) or 0)
        failed_txs = int(details.get("failed_txs", 0) or 0)
        row.update(
            {
                "tps_offered": _to_float(details.get("rate"), row.get("rate_tps", 0)),
                "burst_enabled": details.get("burst_enabled", False),
                "burst_rate_tps": _to_float(details.get("burst_rate")),
                "total_txs": total_txs,
                "success_txs": success_txs,
                "failed_txs": failed_txs,
                "duration_s_actual": duration,
                "tps_accepted": success_txs / max(duration, 1),
                "avg_user_action_latency_ms": _to_float(
                    latency_metrics.get("user_action_latency_ms")
                ),
            }
        )

    seq_file = os.path.join(run_dir, "sequencer_batch_metrics.jsonl")
    seq_rows = _load_jsonl(seq_file) if os.path.exists(seq_file) else []
    if seq_rows:
        tx_counts = [int(r.get("tx_count", 0) or 0) for r in seq_rows]
        gas_limits = [_to_float(r.get("total_gas_limit")) for r in seq_rows]
        batch_bytes = [_to_float(r.get("estimated_batch_bytes")) for r in seq_rows]
        raw_bytes = [_to_float(r.get("raw_tx_bytes")) for r in seq_rows]
        blob_util = [_to_float(r.get("blob_utilization")) for r in seq_rows]
        wait_mean = [_to_float(r.get("wait_time_mean_ms")) for r in seq_rows]
        fairness = [_to_float(r.get("jains_fairness_index"), 1.0) for r in seq_rows]
        reorders = [int(r.get("reordering_events", 0) or 0) for r in seq_rows]
        row.update(
            {
                "batch_count": len(seq_rows),
                "tps_committed": sum(tx_counts) / max(duration_s or row.get("duration_s_actual", 0) or 1, 1),
                "avg_batch_tx_count": _mean(tx_counts),
                "avg_gas_per_batch": _mean(gas_limits),
                "avg_calldata_bytes": _mean(raw_bytes),
                "avg_batch_bytes": _mean(batch_bytes),
                "avg_blob_utilization": _mean(blob_util),
                "avg_queue_wait_ms": _mean(wait_mean),
                "p50_queue_wait_ms": _percentile(wait_mean, 50),
                "p95_queue_wait_ms": _percentile(wait_mean, 95),
                "p99_queue_wait_ms": _percentile(wait_mean, 99),
                "jains_fairness": _mean(fairness),
                "total_reordering_events": sum(reorders),
            }
        )

    ex_file = os.path.join(run_dir, "executor_batch_metrics.jsonl")
    ex_rows = _load_jsonl(ex_file) if os.path.exists(ex_file) else []
    if ex_rows:
        exec_times = [
            _to_float(r.get("execution_phases", {}).get("total_execution_ms"), _to_float(r.get("total_execution_ms")))
            for r in ex_rows
        ]
        proof_times = [
            _to_float(r.get("prover_metrics", {}).get("total_prover_wall_ms"), _to_float(r.get("total_proof_ms")))
            for r in ex_rows
        ]
        proof_bytes = [_to_float(r.get("proof_bytes")) for r in ex_rows]
        journal_bytes = [_to_float(r.get("journal_bytes")) for r in ex_rows]
        state_diff_bytes = [_to_float(r.get("state_diff_bytes")) for r in ex_rows]
        row.update(
            {
                "avg_exec_ms": _mean(exec_times),
                "p50_exec_ms": _percentile(exec_times, 50),
                "p95_exec_ms": _percentile(exec_times, 95),
                "p99_exec_ms": _percentile(exec_times, 99),
                "avg_prove_ms": _mean(proof_times),
                "p50_prove_ms": _percentile(proof_times, 50),
                "p95_prove_ms": _percentile(proof_times, 95),
                "p99_prove_ms": _percentile(proof_times, 99),
                "avg_proof_bytes": _mean(proof_bytes),
                "avg_journal_bytes": _mean(journal_bytes),
                "avg_state_diff_bytes": _mean(state_diff_bytes),
            }
        )

    sub_file = os.path.join(run_dir, "submitter_metrics.json")
    if os.path.exists(sub_file):
        batches = _load_jsonl(sub_file)
        if batches:
            l2_l1 = [_to_float(b.get("l2_l1_latency_ms")) for b in batches]
            gas_used = [_to_float(b.get("l1_gas_used")) for b in batches]
            regular_gas_used = [_to_float(b.get("regular_gas_used")) for b in batches]
            tx_counts = [max(int(b.get("tx_count", 0) or 0), 1) for b in batches]
            blobs = [_to_float(b.get("blob_utilization")) for b in batches]
            cost_usd = [_to_float(b.get("total_cost_usd")) for b in batches]
            cost_per_tx_usd = [_to_float(b.get("cost_per_tx_usd")) for b in batches]
            total_cost_wei = [_to_float(b.get("total_cost_wei")) for b in batches]
            cost_per_tx_wei = [_to_float(b.get("cost_per_tx_wei")) for b in batches]
            comp_ratio = [_to_float(b.get("compression_ratio")) for b in batches if b.get("compression_ratio") is not None]
            compressed_bytes = [_to_float(b.get("compressed_bytes")) for b in batches]
            batch_bytes = [_to_float(b.get("batch_data_bytes")) for b in batches]
            estimated_blob_gas = [_to_float(b.get("estimated_blob_gas_used")) for b in batches]
            measured_blob_gas = [_to_float(b.get("measured_blob_gas_used")) for b in batches]
            retries = [int(b.get("gas_bump_count", 0) or 0) for b in batches]
            failed_batches = [
                b for b in batches if str(b.get("submission_status", "")).lower() != "submitted"
            ]
            row.update(
                {
                    "avg_l2_l1_ms": _mean(l2_l1),
                    "p50_l2_l1_ms": _percentile(l2_l1, 50),
                    "p95_l2_l1_ms": _percentile(l2_l1, 95),
                    "p99_l2_l1_ms": _percentile(l2_l1, 99),
                    "avg_l1_gas_used": _mean(gas_used),
                    "avg_gas_per_batch": row.get("avg_gas_per_batch", _mean(gas_used)),
                    "avg_gas_per_tx": _mean(
                        [gas / tx for gas, tx in zip(gas_used, tx_counts, strict=False)]
                    ),
                    "avg_blob_utilization": _mean(blobs) if not row.get("avg_blob_utilization") else row["avg_blob_utilization"],
                    "avg_soft_commit_ms": _mean([_to_float(b.get("soft_commit_ms")) for b in batches]),
                    "avg_hard_finality_ms": _mean([_to_float(b.get("hard_finality_ms")) for b in batches]),
                    "avg_finality_gain_ms": _mean([_to_float(b.get("finality_gain_ms")) for b in batches]),
                    "avg_total_cost_wei": _mean(total_cost_wei),
                    "avg_cost_per_tx_wei": _mean(cost_per_tx_wei),
                    "avg_total_cost_usd": _mean(cost_usd),
                    "avg_cost_per_tx_usd": _mean(cost_per_tx_usd),
                    "avg_estimated_blob_gas_used": _mean(estimated_blob_gas),
                    "avg_measured_blob_gas_used": _mean(measured_blob_gas),
                    "avg_regular_gas_used": _mean(regular_gas_used),
                    "avg_comp_ratio": _mean(comp_ratio),
                    "avg_compressed_bytes": _mean(compressed_bytes),
                    "avg_batch_bytes_submitter": _mean(batch_bytes),
                    "cost_source": batches[-1].get("cost_source"),
                    "blob_cost_source": batches[-1].get("blob_cost_source"),
                    "real_eip4844_blob": batches[-1].get("real_eip4844_blob"),
                    "eth_usd_reference": batches[-1].get("eth_usd_reference"),
                    "regular_gas_price_reference_wei": batches[-1].get("regular_gas_price_reference_wei"),
                    "blob_gas_price_reference_wei": batches[-1].get("blob_gas_price_reference_wei"),
                    "cost_model_version": batches[-1].get("cost_model_version"),
                    "total_batches": len(batches),
                    "failed_batches": len(failed_batches),
                    "total_retries": sum(retries),
                    "goodput_tps": sum(int(b.get("tx_count", 0) or 0) for b in batches if str(b.get("submission_status", "")).lower() == "submitted")
                    / max(duration_s or row.get("duration_s_actual", 0) or 1, 1),
                }
            )
            row["avg_gas_saved"] = max(
                row.get("avg_calldata_bytes", 0.0) - row.get("avg_compressed_bytes", 0.0),
                0.0,
            )

    tx_log = os.path.join(run_dir, f"tx_log_{run_id}.csv")
    if os.path.exists(tx_log):
        tx_rows = _load_csv(tx_log)
        latencies = [_to_float(r.get("latency")) * 1000 for r in tx_rows if r.get("status") == "success"]
        row["p95_user_action_latency_ms"] = _percentile(latencies, 95)
        row["p99_user_action_latency_ms"] = _percentile(latencies, 99)
        mean_latency = _mean(latencies)
        row["starvation_count"] = sum(1 for latency in latencies if mean_latency > 0 and latency > mean_latency * 3)
        for tx_type in ("A", "B", "C"):
            per_type = [
                _to_float(r.get("latency")) * 1000
                for r in tx_rows
                if r.get("status") == "success" and r.get("tx_type") == tx_type
            ]
            row[f"p95_latency_type{tx_type}_ms"] = _percentile(per_type, 95)

    res_file = os.path.join(run_dir, "resource_metrics.json")
    if os.path.exists(res_file):
        res = _load_json(res_file)
        row["max_memory_usage_mb"] = _to_float(res.get("max_memory_usage_mb"))
        row["max_memory_usage_gb"] = _to_float(res.get("max_memory_usage_gb"))

    row["run_total_txs"] = int(status_payload.get("total_txs", row.get("total_txs", 0)) or 0)
    row["run_success_txs"] = int(status_payload.get("success_txs", row.get("success_txs", 0)) or 0)
    row["run_failed_txs"] = int(status_payload.get("failed_txs", row.get("failed_txs", 0)) or 0)
    row["run_success_rate"] = _to_float(status_payload.get("success_rate"))
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
        exec_phases = ex.get("execution_phases", {})
        prover_metrics = ex.get("prover_metrics", {})
        rows.append(
            {
                "experiment_id": exp_id,
                "run_id": run_id,
                "batch_id": bid,
                "seal_reason": seq.get("seal_reason"),
                "batch_policy": seq.get("batch_policy"),
                "scheduling_policy": seq.get("scheduling_policy"),
                "mempool_depth_at_batch": seq.get("mempool_depth_at_batch"),
                "tx_count": seq.get("tx_count"),
                "estimated_batch_bytes": seq.get("estimated_batch_bytes"),
                "blob_utilization_sequencer": seq.get("blob_utilization"),
                "wait_time_mean_ms": seq.get("wait_time_mean_ms"),
                "wait_time_p95_ms": seq.get("wait_time_p95_ms"),
                "jains_fairness_index": seq.get("jains_fairness_index"),
                "reordering_events": seq.get("reordering_events"),
                "total_gas_limit": seq.get("total_gas_limit"),
                "fee_proxy_wei_sequencer": seq.get("fee_proxy_wei"),
                "state_diff_count": ex.get("state_diff_count"),
                "state_diff_bytes": ex.get("state_diff_bytes"),
                "unique_touched_accounts": ex.get("unique_touched_accounts"),
                "repeated_touched_accounts": ex.get("repeated_touched_accounts"),
                "execution_time_ms": exec_phases.get("total_execution_ms", ex.get("total_execution_ms")),
                "proof_time_ms": prover_metrics.get("total_prover_wall_ms", ex.get("total_proof_ms")),
                "proof_bytes": ex.get("proof_bytes"),
                "journal_bytes": ex.get("journal_bytes"),
                "da_mode": sub.get("da_mode"),
                "compressed_bytes": sub.get("compressed_bytes"),
                "compression_ratio": sub.get("compression_ratio"),
                "blob_count": sub.get("blob_count"),
                "blob_utilization_submitter": sub.get("blob_utilization"),
                "l1_gas_used": sub.get("l1_gas_used"),
                "regular_gas_used": sub.get("regular_gas_used"),
                "measured_regular_gas_used": sub.get("measured_regular_gas_used"),
                "measured_blob_gas_used": sub.get("measured_blob_gas_used"),
                "estimated_blob_gas_used": sub.get("estimated_blob_gas_used"),
                "regular_gas_price_reference_wei": sub.get("regular_gas_price_reference_wei"),
                "blob_gas_price_reference_wei": sub.get("blob_gas_price_reference_wei"),
                "eth_usd_reference": sub.get("eth_usd_reference"),
                "cost_source": sub.get("cost_source"),
                "blob_cost_source": sub.get("blob_cost_source"),
                "real_eip4844_blob": sub.get("real_eip4844_blob"),
                "cost_model_version": sub.get("cost_model_version"),
                "l1_latency_ms": sub.get("l2_l1_latency_ms"),
                "fee_proxy_wei_submitter": sub.get("fee_proxy_wei"),
                "soft_commit_ms": sub.get("soft_commit_ms"),
                "hard_finality_ms": sub.get("hard_finality_ms"),
                "finality_gain_ms": sub.get("finality_gain_ms"),
                "total_cost_wei": sub.get("total_cost_wei"),
                "cost_per_tx_wei": sub.get("cost_per_tx_wei"),
                "total_cost_usd": sub.get("total_cost_usd"),
                "cost_per_tx_usd": sub.get("cost_per_tx_usd"),
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
