"""
poisson_generator.py — Poisson workload generator for RollupX benchmark suite.

Extended from original to support:
  --tx_mix      preset (balanced / light / heavy) or custom fractions
  --mix_a/b/c   custom type fractions (used when --tx_mix custom)
  --seed        RNG seed for reproducibility
  --warmup      warm-up duration in seconds (traffic sent but not recorded)
  --run_id      unique run identifier for output file naming

Metrics file written to $METRICS_ROOT/workload_<experiment_id>.json
Run metadata appended to $METRICS_ROOT/run_status.json
"""

import argparse
import csv
import json
import os
import random
import sys
import time
import urllib.error
import urllib.request
from concurrent.futures import ThreadPoolExecutor, as_completed
from datetime import datetime, timezone

try:
    from eth_account import Account
    from eth_account.messages import encode_defunct
except ImportError:
    print("Error: run `pip install eth-account`")
    sys.exit(1)

# local module — must be in same directory
sys.path.insert(0, os.path.dirname(__file__))
from tx_types import TxFactory, resolve_mix, MIX_PRESETS


# ── constants ─────────────────────────────────────────────────────────────────

_DEFAULT_PRIVATE_KEY = (
    "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
)


# ── generator class ───────────────────────────────────────────────────────────

class PoissonWorkloadGenerator:
    def __init__(
        self,
        rate: float,
        duration: int,
        warmup: int,
        seed: int | None,
        experiment_id: str,
        run_id: str,
        prover_backend: str,
        tx_mix: tuple[float, float, float],
        host: str = "localhost",
        port: int = 3000,
        concurrency: int = 1,
        target_txs: int = 0,
        account_count: int = 1,
        phase: str = "measured",
        start_nonce: int = 0,
    ):
        self.rate          = rate
        self.duration      = duration
        self.warmup        = warmup
        self.seed          = seed
        self.experiment_id = experiment_id
        self.run_id        = run_id
        self.prover_backend= prover_backend
        self.tx_mix        = tx_mix
        self.base_url      = f"http://{host}:{port}"
        self.concurrency   = max(1, concurrency)
        self.target_txs    = max(0, target_txs)
        self.account_count = max(1, account_count)
        self.phase         = phase
        self.start_nonce   = max(0, start_nonce)

        # seeded RNG — separate instance for type sampling vs inter-arrival
        self.rng_arrival = random.Random(seed)
        self.rng_factory = random.Random(seed + 1000 if seed is not None else None)

        self.factory = TxFactory(
            private_key=_DEFAULT_PRIVATE_KEY,
            account_count=self.account_count,
            seed=seed + 2000 if seed is not None else None,  # independent stream
        )
        self.sender_nonce: dict[int, int] = {
            i: self.start_nonce for i in range(self.account_count)
        }

        # stats storage
        self.stats: list[dict] = []          # timed run only
        self.warmup_stats: list[dict] = []   # warm-up run (discarded from metrics)

    # ── run ───────────────────────────────────────────────────────────────────

    def run(self):
        print(f"[{self.run_id}] PoissonWorkloadGenerator starting")
        print(f"  rate={self.rate} tx/s  warmup={self.warmup}s  duration={self.duration}s")
        print(f"  seed={self.seed}  mix=A{self.tx_mix[0]:.0%}/B{self.tx_mix[1]:.0%}/C{self.tx_mix[2]:.0%}")
        print(f"  target={self.base_url}")
        print(f"  sender_concurrency={self.concurrency}")
        if self.target_txs > 0:
            print(f"  fixed_target_txs={self.target_txs}")

        nonce = self.start_nonce

        # ── warm-up phase ─────────────────────────────────────────────────────
        if self.warmup > 0:
            print(f"\n[WARMUP] {self.warmup}s (not recorded)")
            nonce = self._send_phase(
                phase_duration=self.warmup,
                start_nonce=nonce,
                record_to=self.warmup_stats,
                label="WARMUP",
            )
            print(f"[WARMUP] complete — {len(self.warmup_stats)} txs sent (discarded)")

        # ── timed measurement phase ───────────────────────────────────────────
        if self.target_txs > 0:
            print(f"\n[RUN] fixed-count burst measurement ({self.target_txs} txs)")
            nonce = self._send_count_concurrent(
                count=self.target_txs,
                start_nonce=nonce,
                record_to=self.stats,
                label="RUN",
            )
        else:
            print(f"\n[RUN] {self.duration}s timed measurement")
            nonce = self._send_phase(
                phase_duration=self.duration,
                start_nonce=nonce,
                record_to=self.stats,
                label="RUN",
            )

        total = len(self.stats)
        success = sum(1 for s in self.stats if s["status"] == "success")
        print(f"\n[DONE] total={total}  success={success}  failed={total - success}")

        self._save_metrics()
        self._save_status(success=total > 0 and success == total)

    # ── internal send loop ────────────────────────────────────────────────────

    def _send_phase(
        self,
        phase_duration: int,
        start_nonce: int,
        record_to: list,
        label: str,
    ) -> int:
        if self.concurrency > 1:
            return self._send_phase_concurrent(
                phase_duration=phase_duration,
                start_nonce=start_nonce,
                record_to=record_to,
                label=label,
            )

        end_time = time.time() + phase_duration
        nonce = start_nonce
        tx_count = 0

        try:
            while time.time() < end_time:
                wait = self.rng_arrival.expovariate(self.rate)
                time.sleep(wait)

                if time.time() >= end_time:
                    break

                tx_type = self.rng_factory.choices(["A", "B", "C"], weights=self.tx_mix, k=1)[0]
                sender_idx = self.rng_factory.randrange(self.account_count)
                sender_nonce = self.sender_nonce[sender_idx]
                tx = self.factory.make(
                    tx_type,
                    sender_nonce,
                    phase=self.phase,
                    run_id=self.run_id,
                    experiment_id=self.experiment_id,
                )
                tx["benchmark"]["sender_index"] = sender_idx
                self.sender_nonce[sender_idx] = sender_nonce + 1

                ts_start = time.time()
                status, err = self._post_tx(tx)
                latency = time.time() - ts_start

                record_to.append({
                    "tx_id":    nonce,
                    "tx_type":  tx_type,
                    "sender_index": sender_idx,
                    "sender_nonce": sender_nonce,
                    "from": tx["from"],
                    "phase": self.phase,
                    "timestamp": datetime.now(timezone.utc).isoformat(),
                    "latency":  latency,
                    "status":   status,
                    "error":    err,
                })

                if tx_count % 20 == 0:
                    print(f"  [{label}] sent {tx_count} txs ...")

                nonce    += 1
                tx_count += 1

        except KeyboardInterrupt:
            print(f"\n[{label}] interrupted at tx #{tx_count}")

        return nonce

    def _send_phase_concurrent(
        self,
        phase_duration: int,
        start_nonce: int,
        record_to: list,
        label: str,
    ) -> int:
        end_time = time.time() + phase_duration
        nonce = start_nonce
        tx_count = 0
        futures = []

        with ThreadPoolExecutor(max_workers=self.concurrency) as executor:
            try:
                next_send = time.time()
                while time.time() < end_time:
                    wait = self.rng_arrival.expovariate(self.rate)
                    next_send += wait
                    sleep_for = next_send - time.time()
                    if sleep_for > 0:
                        time.sleep(sleep_for)

                    if time.time() >= end_time:
                        break

                    tx_type = self.rng_factory.choices(["A", "B", "C"], weights=self.tx_mix, k=1)[0]
                    sender_idx = self.rng_factory.randrange(self.account_count)
                    sender_nonce = self.sender_nonce[sender_idx]
                    tx = self.factory.make(
                        tx_type,
                        sender_nonce,
                        phase=self.phase,
                        run_id=self.run_id,
                        experiment_id=self.experiment_id,
                    )
                    tx["benchmark"]["sender_index"] = sender_idx
                    self.sender_nonce[sender_idx] = sender_nonce + 1
                    futures.append(
                        executor.submit(
                            self._post_tx_record, tx, nonce, tx_type, sender_idx, sender_nonce
                        )
                    )

                    if tx_count % 100 == 0:
                        print(f"  [{label}] scheduled {tx_count} txs ...")

                    nonce += 1
                    tx_count += 1

            except KeyboardInterrupt:
                print(f"\n[{label}] interrupted at tx #{tx_count}")

            for future in as_completed(futures):
                record_to.append(future.result())

        record_to.sort(key=lambda item: item["tx_id"])
        return nonce

    def _post_tx_record(
        self,
        tx: dict,
        nonce: int,
        tx_type: str,
        sender_idx: int | None = None,
        sender_nonce: int | None = None,
    ) -> dict:
        ts_start = time.time()
        status, err = self._post_tx(tx)
        latency = time.time() - ts_start
        return {
            "tx_id":    nonce,
            "tx_type":  tx_type,
            "sender_index": sender_idx if sender_idx is not None else tx.get("benchmark", {}).get("sender_index"),
            "sender_nonce": sender_nonce if sender_nonce is not None else tx.get("nonce"),
            "from": tx.get("from"),
            "phase": self.phase,
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "latency":  latency,
            "status":   status,
            "error":    err,
        }

    def _send_count_concurrent(
        self,
        count: int,
        start_nonce: int,
        record_to: list,
        label: str,
    ) -> int:
        nonce = start_nonce
        prepared = []
        for i in range(count):
            tx_type = self.rng_factory.choices(["A", "B", "C"], weights=self.tx_mix, k=1)[0]
            sender_idx = self.rng_factory.randrange(self.account_count)
            sender_nonce = self.sender_nonce[sender_idx]
            tx = self.factory.make(
                tx_type,
                sender_nonce,
                phase=self.phase,
                run_id=self.run_id,
                experiment_id=self.experiment_id,
            )
            tx["benchmark"]["sender_index"] = sender_idx
            self.sender_nonce[sender_idx] = sender_nonce + 1
            prepared.append((tx, nonce, tx_type, sender_idx, sender_nonce))
            nonce += 1
            if i % 250 == 0:
                print(f"  [{label}] prepared {i} txs ...")

        with ThreadPoolExecutor(max_workers=self.concurrency) as executor:
            futures = [
                executor.submit(self._post_tx_record, tx, tx_nonce, tx_type, sender_idx, sender_nonce)
                for tx, tx_nonce, tx_type, sender_idx, sender_nonce in prepared
            ]
            for index, future in enumerate(as_completed(futures), start=1):
                record_to.append(future.result())
                if index % 250 == 0:
                    print(f"  [{label}] completed {index} txs ...")

        record_to.sort(key=lambda item: item["tx_id"])
        return nonce

    def _post_tx(self, tx: dict) -> tuple[str, str | None]:
        url  = f"{self.base_url}/tx"
        data = json.dumps(tx).encode("utf-8")
        req  = urllib.request.Request(
            url, data=data, headers={"Content-Type": "application/json"}
        )
        try:
            with urllib.request.urlopen(req, timeout=5) as resp:
                body = resp.read()
                try:
                    decoded = json.loads(body.decode("utf-8"))
                    status = decoded.get("status")
                    if status == "Accepted":
                        return "success", None
                    if isinstance(status, dict) and "Rejected" in status:
                        return "error", f"Rejected: {status['Rejected']}"
                    if status not in (None, "Accepted"):
                        return "error", f"Unexpected confirmation status: {status}"
                except json.JSONDecodeError:
                    pass
            return "success", None
        except urllib.error.HTTPError as e:
            return "error", f"HTTP {e.code}: {e.reason}"
        except Exception as e:
            return "error", str(e)

    # ── persistence ───────────────────────────────────────────────────────────

    def _save_metrics(self):
        successes = [s for s in self.stats if s["status"] == "success"]
        latencies = [s["latency"] for s in successes]
        avg_latency_ms = (sum(latencies) / len(latencies)) * 1000 if latencies else 0

        # per-type breakdown
        type_counts: dict[str, int] = {"A": 0, "B": 0, "C": 0}
        for s in self.stats:
            type_counts[s["tx_type"]] += 1

        metrics = {
            "experiment_id": self.experiment_id,
            "run_id":        self.run_id,
            "source":        "workload",
            "prover_backend": self.prover_backend,
            "da_mode":       "n/a",
            "seed":          self.seed,
            "phase":         self.phase,
            "account_count": self.account_count,
            "start_nonce":   self.start_nonce,
            "tx_mix":        {
                "A": self.tx_mix[0],
                "B": self.tx_mix[1],
                "C": self.tx_mix[2],
            },
            "latency_metrics": {
                "user_action_latency_ms": avg_latency_ms,
                "l2_l1_latency_ms":       0,
            },
            "witness_info": {
                "constraints":        0,
                "witness_size_bytes": 0,
            },
            "details": {
                "total_txs":      len(self.stats),
                "successful_txs": len(successes),
                "failed_txs":     len(self.stats) - len(successes),
                "duration":       self.duration,
                "rate":           self.rate,
                "type_counts":    type_counts,
                "sender_counts":  self._sender_counts(),
            },
        }

        metrics_root = os.environ.get("METRICS_ROOT", "metrics")
        os.makedirs(metrics_root, exist_ok=True)

        # main metrics file (matches pareto.py contract)
        path = os.path.join(metrics_root, f"workload_{self.experiment_id}.json")
        with open(path, "w") as f:
            json.dump(metrics, f, indent=2)
        print(f"Saved metrics → {path}")

        # per-tx CSV for fairness / CDF analysis
        csv_path = os.path.join(metrics_root, f"tx_log_{self.run_id}.csv")
        with open(csv_path, "w", newline="") as f:
            writer = csv.DictWriter(
                f,
                fieldnames=[
                    "tx_id",
                    "tx_type",
                    "sender_index",
                    "sender_nonce",
                    "from",
                    "phase",
                    "timestamp",
                    "latency",
                    "status",
                    "error",
                ],
            )
            writer.writeheader()
            writer.writerows(self.stats)
        print(f"Saved tx log  → {csv_path}")

    def _save_status(self, success: bool):
        metrics_root = os.environ.get("METRICS_ROOT", "metrics")
        success_txs = sum(1 for s in self.stats if s["status"] == "success")
        total_txs = len(self.stats)
        failed_txs = total_txs - success_txs
        status = {
            "run_id":        self.run_id,
            "experiment_id": self.experiment_id,
            "status":        "pass" if success else "fail",
            "timestamp":     datetime.now(timezone.utc).isoformat(),
            "total_txs":     total_txs,
            "success_txs":   success_txs,
            "failed_txs":    failed_txs,
            "success_rate":  success_txs / total_txs if total_txs else 0,
            "phase":         self.phase,
        }
        path = os.path.join(metrics_root, "run_status.json")
        with open(path, "w") as f:
            json.dump(status, f, indent=2)
        print(f"Saved status  → {path}")

    def _sender_counts(self) -> dict[str, int]:
        counts: dict[str, int] = {}
        for stat in self.stats:
            idx = stat.get("sender_index")
            key = str(idx if idx is not None else "unknown")
            counts[key] = counts.get(key, 0) + 1
        return counts


# ── CLI ───────────────────────────────────────────────────────────────────────

def parse_args():
    p = argparse.ArgumentParser(description="RollupX Poisson Workload Generator")

    # existing flags (unchanged defaults)
    p.add_argument("--rate",           type=float, default=1.0,
                   help="Arrival rate (tx/sec)")
    p.add_argument("--duration",       type=int,   default=120,
                   help="Measurement duration in seconds")
    p.add_argument("--host",           type=str,   default="localhost")
    p.add_argument("--port",           type=int,   default=3000)
    p.add_argument("--experiment_id",  type=str,   default=f"exp_{int(time.time())}")
    p.add_argument("--prover_backend", type=str,   default="groth16")

    # new flags
    p.add_argument("--seed",   type=int,   default=None,
                   help="RNG seed for reproducibility")
    p.add_argument("--warmup", type=int,   default=15,
                   help="Warm-up seconds (traffic sent but not recorded)")
    p.add_argument("--run_id", type=str,   default=None,
                   help="Unique run identifier (default: <exp_id>_r00)")
    p.add_argument("--concurrency", type=int, default=1,
                   help="Number of concurrent HTTP sender workers")
    p.add_argument("--target_txs", type=int, default=0,
                   help="If >0, send this many txs as a fixed-count concurrent burst")
    p.add_argument("--account_count", type=int, default=1,
                   help="Number of deterministic sender accounts to use")
    p.add_argument("--phase", type=str, default="measured", choices=["warmup", "measured"],
                   help="Benchmark phase label")
    p.add_argument("--start_nonce", type=int, default=0,
                   help="Initial nonce for each sender account")

    # tx mix
    mix_group = p.add_argument_group("Transaction mix")
    mix_group.add_argument(
        "--tx_mix", type=str, default="balanced",
        choices=list(MIX_PRESETS) + ["custom"],
        help="Mix preset or 'custom' (then supply --mix_a/b/c)"
    )
    mix_group.add_argument("--mix_a", type=float, default=None,
                           help="Fraction of Type-A txs (custom only)")
    mix_group.add_argument("--mix_b", type=float, default=None,
                           help="Fraction of Type-B txs (custom only)")
    mix_group.add_argument("--mix_c", type=float, default=None,
                           help="Fraction of Type-C txs (custom only)")

    return p.parse_args()


def main():
    args = parse_args()

    preset = None if args.tx_mix == "custom" else args.tx_mix
    tx_mix = resolve_mix(preset, args.mix_a, args.mix_b, args.mix_c)

    run_id = args.run_id or f"{args.experiment_id}_r00"

    gen = PoissonWorkloadGenerator(
        rate          = args.rate,
        duration      = args.duration,
        warmup        = args.warmup,
        seed          = args.seed,
        experiment_id = args.experiment_id,
        run_id        = run_id,
        prover_backend= args.prover_backend,
        tx_mix        = tx_mix,
        host          = args.host,
        port          = args.port,
        concurrency   = args.concurrency,
        target_txs    = args.target_txs,
        account_count = args.account_count,
        phase         = args.phase,
        start_nonce   = args.start_nonce,
    )
    gen.run()


if __name__ == "__main__":
    main()
