"""
tests/test_aggregate.py — Unit tests for aggregate.py and stats.py.

Run with:  python -m pytest data-tools/tests/  -v
       or: python data-tools/tests/test_aggregate.py
"""

import csv
import json
import math
import os
import sys
import tempfile
import unittest

# allow imports from parent directory
sys.path.insert(0, os.path.dirname(os.path.dirname(__file__)))
from aggregate import _load_run, _percentile, _jains_index, aggregate
from stats import compute_stats, _ci95


# ── fixtures ──────────────────────────────────────────────────────────────────

def _make_run_dir(
    tmp: str,
    exp_id: str,
    run_id: str,
    status: str = "pass",
    rate: float = 10.0,
    l2_l1_ms: float = 500.0,
    prove_ms: float = 200.0,
    gas_saved: float = 10000.0,
    comp_ratio: float = 2.5,
    n_batches: int = 3,
) -> str:
    run_dir = os.path.join(tmp, exp_id, run_id)
    os.makedirs(run_dir, exist_ok=True)

    # run_status.json
    with open(os.path.join(run_dir, "run_status.json"), "w") as f:
        json.dump({"run_id": run_id, "experiment_id": exp_id, "status": status,
                   "total_txs": 100, "success_txs": 95}, f)

    # run_metadata.json
    with open(os.path.join(run_dir, "run_metadata.json"), "w") as f:
        json.dump({
            "run_id": run_id,
            "experiment_id": exp_id,
            "git_commit": "abc1234",
            "timestamp_start": "2026-01-01T00:00:00Z",
            "timestamp_end":   "2026-01-01T00:02:00Z",
            "machine": {"cpu_model": "TestCPU", "cpu_cores": 8, "ram_gb": "16.0"},
            "runtimes": {"python": "3.12.0", "rust": "1.78.0"},
            "config_snapshot": {
                "experiment_id": exp_id,
                "batch_size": "50",
                "timeout_ms": "5000",
                "policy": "FCFS",
                "da_mode": "calldata",
                "prover": "groth16",
                "rate_tps": str(rate),
                "duration_s": "120",
                "warmup_s": "15",
                "tx_mix": "balanced",
                "seed": "42",
            },
        }, f)

    # workload_<exp_id>.json
    with open(os.path.join(run_dir, f"workload_{exp_id}.json"), "w") as f:
        json.dump({
            "experiment_id": exp_id,
            "run_id": run_id,
            "source": "workload",
            "prover_backend": "groth16",
            "rate": rate,
            "details": {
                "total_txs": 100,
                "successful_txs": 95,
                "failed_txs": 5,
                "duration": 120,
                "rate": rate,
                "type_counts": {"A": 70, "B": 20, "C": 10},
            },
        }, f)

    # executor_<exp_id>.json
    with open(os.path.join(run_dir, f"executor_{exp_id}.json"), "w") as f:
        json.dump({
            "experiment_id": exp_id,
            "prover_backend": "groth16",
            "proof_generation_times_ms": [prove_ms, prove_ms * 1.1, prove_ms * 0.9],
            "batch_count": n_batches,
        }, f)

    # submitter_metrics.json (JSONL)
    with open(os.path.join(run_dir, "submitter_metrics.json"), "w") as f:
        for i in range(n_batches):
            rec = {
                "experiment_id":     exp_id,
                "l2_l1_latency_ms":  l2_l1_ms + i * 10,
                "gas_saved":         gas_saved,
                "compression_ratio": comp_ratio,
                "gas_used_per_batch": 500_000,
                "gas_used_per_tx":   10_000,
                "calldata_bytes":    1000,
                "compressed_bytes":  int(1000 / comp_ratio),
                "retries":           0,
                "failed":            False,
                "da_mode":           "calldata",
                "tx_count":          30,
            }
            f.write(json.dumps(rec) + "\n")

    # tx_log_<run_id>.csv
    with open(os.path.join(run_dir, f"tx_log_{run_id}.csv"), "w", newline="") as f:
        writer = csv.DictWriter(
            f, fieldnames=["tx_id", "tx_type", "timestamp", "latency", "status", "error"]
        )
        writer.writeheader()
        types = ["A"] * 70 + ["B"] * 20 + ["C"] * 10
        for i, t in enumerate(types):
            writer.writerow({
                "tx_id": i, "tx_type": t,
                "timestamp": "2026-01-01T00:01:00Z",
                "latency": 0.5 + (0.1 * (t == "B")) + (0.3 * (t == "C")),
                "status": "success", "error": "",
            })

    return run_dir


# ── test cases ────────────────────────────────────────────────────────────────

class TestFixes(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp()

    def test_tps_finalized_uses_duration_not_rate(self):
        """tps_finalized must equal total_finalized_txs / duration, not / (rate * duration)."""
        import pandas as pd
        # fixture: 4 batches × 30 txs = 120 txs, duration=120s → expected=1.0
        run_dir = _make_run_dir(self.tmp, "baseline", "baseline_r01",
                                rate=50.0, n_batches=4)
        row = _load_run(run_dir)
        # With the bug: 120 / (50 * 120) = 0.02. With the fix: 120 / 120 = 1.0
        self.assertAlmostEqual(row.get("tps_finalized", 0), 1.0, places=2,
                               msg="tps_finalized should be 1.0 not 0.02")

    def test_tps_committed_none_when_executor_missing(self):
        """tps_committed must be None when executor JSON lacks total_proved_txs."""
        run_dir = _make_run_dir(self.tmp, "baseline", "baseline_r02")
        # remove executor file so the field is absent
        import glob, os
        for f in glob.glob(os.path.join(run_dir, "executor_*.json")):
            os.remove(f)
        row = _load_run(run_dir)
        self.assertIsNone(row.get("tps_committed"),
                          msg="tps_committed must be None when executor file is absent")

    def test_jains_index_class_level_not_individual(self):
        """Jain's index must be computed on 3 class means, not all individual latencies."""
        import math
        # Class means: A=100ms, B=100ms, C=1000ms
        # Correct Jain's: (100+100+1000)^2 / (3*(100^2+100^2+1000^2))
        vals = [100.0, 100.0, 1000.0]
        s = sum(vals); sq = sum(v*v for v in vals); n = 3
        expected = (s*s) / (n*sq)
        from aggregate import _jains_index
        self.assertAlmostEqual(_jains_index(vals), expected, places=6)
        # Also verify it is NOT the same as individual-latency result
        individual = [100.0]*70 + [100.0]*20 + [1000.0]*10
        self.assertNotAlmostEqual(_jains_index(individual), expected, places=2)

    def test_ci95_correct_t_value_at_n5(self):
        """CI at n=5 must use t=2.776 (df=4), not t=2.0."""
        import pandas as pd, math
        s = pd.Series([90.0, 95.0, 100.0, 105.0, 110.0])
        expected = 2.776 * s.std(ddof=1) / math.sqrt(5)
        from stats import _ci95
        self.assertAlmostEqual(_ci95(s), expected, places=2,
                               msg="CI at n=5 must use t=2.776 not t=2.0")


class TestHelpers(unittest.TestCase):

    def test_percentile_empty(self):
        self.assertEqual(_percentile([], 95), 0.0)

    def test_percentile_single(self):
        self.assertAlmostEqual(_percentile([42.0], 50), 42.0)

    def test_percentile_sorted(self):
        data = list(range(1, 101))   # 1..100
        self.assertAlmostEqual(_percentile(data, 50), 50.0, delta=1.0)
        self.assertAlmostEqual(_percentile(data, 95), 95.0, delta=2.0)

    def test_jains_perfect(self):
        # All equal values → Jain's index = 1.0
        vals = [100.0] * 10
        self.assertAlmostEqual(_jains_index(vals), 1.0, places=6)

    def test_jains_extreme(self):
        # One huge, rest tiny → index close to 0
        vals = [1.0] * 9 + [1000.0]
        j = _jains_index(vals)
        self.assertLess(j, 0.5)

    def test_jains_empty(self):
        self.assertEqual(_jains_index([]), 0.0)

    def test_ci95_single(self):
        import pandas as pd
        s = pd.Series([42.0])
        self.assertTrue(math.isnan(_ci95(s)))

    def test_ci95_multiple(self):
        import pandas as pd
        s = pd.Series([10.0, 12.0, 11.0, 13.0, 9.0])
        ci = _ci95(s)
        self.assertGreater(ci, 0)
        self.assertLess(ci, 5)


class TestLoadRun(unittest.TestCase):

    def setUp(self):
        self.tmp = tempfile.mkdtemp()

    def test_load_basic(self):
        run_dir = _make_run_dir(self.tmp, "bs_050", "bs_050_r01")
        row = _load_run(run_dir)
        self.assertIsNotNone(row)
        self.assertEqual(row["experiment_id"], "bs_050")
        self.assertEqual(row["run_id"], "bs_050_r01")
        self.assertEqual(row["run_status"], "pass")

    def test_load_tps_accepted(self):
        run_dir = _make_run_dir(self.tmp, "bs_050", "bs_050_r02", rate=20.0)
        row = _load_run(run_dir)
        # tps_accepted = successful_txs / duration = 95 / 120
        self.assertAlmostEqual(row["tps_accepted"], 95 / 120, places=3)

    def test_load_submitter_metrics(self):
        run_dir = _make_run_dir(self.tmp, "bs_050", "bs_050_r03",
                                l2_l1_ms=400.0, n_batches=3)
        row = _load_run(run_dir)
        # avg of 400, 410, 420 = 410
        self.assertAlmostEqual(row["avg_l2_l1_ms"], 410.0, places=1)

    def test_load_executor_metrics(self):
        run_dir = _make_run_dir(self.tmp, "bs_050", "bs_050_r04", prove_ms=300.0)
        row = _load_run(run_dir)
        # avg of 300, 330, 270 = 300
        self.assertAlmostEqual(row["avg_prove_ms"], 300.0, places=1)

    def test_load_fairness(self):
        run_dir = _make_run_dir(self.tmp, "bs_050", "bs_050_r05")
        row = _load_run(run_dir)
        self.assertIn("jains_fairness", row)
        j = row["jains_fairness"]
        if j is not None:
            self.assertGreater(j, 0.0)
            self.assertLessEqual(j, 1.0)

    def test_load_per_type_latency(self):
        run_dir = _make_run_dir(self.tmp, "bs_050", "bs_050_r06")
        row = _load_run(run_dir)
        self.assertIn("p95_latency_typeA_ms", row)
        self.assertIn("p95_latency_typeB_ms", row)
        self.assertIn("p95_latency_typeC_ms", row)
        # Type C has higher latency than A
        self.assertGreater(row["p95_latency_typeC_ms"], row["p95_latency_typeA_ms"])

    def test_failed_run_status(self):
        run_dir = _make_run_dir(self.tmp, "bs_050", "bs_050_r07", status="fail")
        row = _load_run(run_dir)
        self.assertEqual(row["run_status"], "fail")

    def test_missing_status_file(self):
        run_dir = _make_run_dir(self.tmp, "bs_050", "bs_050_r08")
        os.remove(os.path.join(run_dir, "run_status.json"))
        row = _load_run(run_dir)
        self.assertEqual(row["run_status"], "unknown")


class TestAggregate(unittest.TestCase):

    def setUp(self):
        self.tmp = tempfile.mkdtemp()

    def test_aggregate_multiple_runs(self):
        import pandas as pd
        for i in range(3):
            _make_run_dir(self.tmp, "baseline", f"baseline_r0{i+1}",
                          l2_l1_ms=500.0 + i * 50)
        out = os.path.join(self.tmp, "all_results.csv")
        df = aggregate(self.tmp, out, include_failed=False)
        self.assertFalse(df.empty)
        self.assertIn("avg_l2_l1_ms", df.columns)
        self.assertEqual(len(df), 3)

    def test_aggregate_skips_failed(self):
        import pandas as pd
        _make_run_dir(self.tmp, "baseline", "baseline_r01", status="pass")
        _make_run_dir(self.tmp, "baseline", "baseline_r02", status="fail")
        out = os.path.join(self.tmp, "all_results.csv")
        df = aggregate(self.tmp, out, include_failed=False)
        self.assertEqual(len(df), 1)

    def test_aggregate_include_failed(self):
        import pandas as pd
        _make_run_dir(self.tmp, "baseline", "baseline_r01", status="pass")
        _make_run_dir(self.tmp, "baseline", "baseline_r02", status="fail")
        out = os.path.join(self.tmp, "all_results.csv")
        df = aggregate(self.tmp, out, include_failed=True)
        self.assertEqual(len(df), 2)

    def test_aggregate_empty_dir(self):
        import pandas as pd
        out = os.path.join(self.tmp, "empty_results.csv")
        df = aggregate(self.tmp, out)
        self.assertTrue(df.empty)

    def test_aggregate_multiple_experiments(self):
        import pandas as pd
        for exp_id in ["baseline", "bs_010", "bs_100"]:
            for i in range(2):
                _make_run_dir(self.tmp, exp_id, f"{exp_id}_r0{i+1}")
        out = os.path.join(self.tmp, "all_results.csv")
        df = aggregate(self.tmp, out)
        self.assertEqual(df["experiment_id"].nunique(), 3)


class TestStats(unittest.TestCase):

    def _make_df(self) -> "pd.DataFrame":
        import pandas as pd
        rows = []
        for exp_id, lat, prove in [
            ("baseline", 500, 200),
            ("bs_010",   800, 180),
            ("bs_100",   300, 220),
        ]:
            for i in range(5):
                rows.append({
                    "experiment_id": exp_id,
                    "run_status":    "pass",
                    "avg_l2_l1_ms":  lat + i * 10,
                    "avg_prove_ms":  prove + i * 5,
                    "jains_fairness": 0.9 + i * 0.01,
                    "tps_committed": 10.0,
                    "avg_gas_per_tx": 10000.0,
                })
        return pd.DataFrame(rows)

    def test_stats_shape(self):
        import pandas as pd
        df = self._make_df()
        baseline_df = df[df["experiment_id"] == "baseline"]
        stats = compute_stats(df, baseline_df)
        self.assertFalse(stats.empty)
        self.assertIn("mean", stats.columns)
        self.assertIn("p95", stats.columns)
        self.assertIn("ci95_half", stats.columns)
        self.assertIn("delta_vs_baseline", stats.columns)

    def test_baseline_delta_is_zero(self):
        import pandas as pd
        df = self._make_df()
        baseline_df = df[df["experiment_id"] == "baseline"]
        stats = compute_stats(df, baseline_df)
        baseline_stats = stats[stats["experiment_id"] == "baseline"]
        # baseline vs itself should be +0.0%
        lat_row = baseline_stats[baseline_stats["metric_col"] == "avg_l2_l1_ms"]
        if not lat_row.empty:
            delta = lat_row["delta_vs_baseline"].iloc[0]
            self.assertIn("0.0%", str(delta))

    def test_ci95_decreases_with_n(self):
        import pandas as pd
        # larger sample → smaller CI
        s5  = pd.Series([100.0] * 5  + list(range(5)))
        s20 = pd.Series([100.0] * 20 + list(range(20)))
        self.assertGreater(_ci95(s5), _ci95(s20))


# ── runner ────────────────────────────────────────────────────────────────────

if __name__ == "__main__":
    unittest.main(verbosity=2)
