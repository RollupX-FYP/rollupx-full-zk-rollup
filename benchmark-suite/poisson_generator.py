import argparse
import time
import random
import sys
import os
import json
import csv
import urllib.request
import urllib.error
from datetime import datetime
try:
    from eth_account import Account
    from eth_account.messages import encode_defunct
    import eth_utils
except ImportError:
    print("Error: eth-account is not installed. Please run `pip install eth-account`")
    sys.exit(1)

class PoissonWorkloadGenerator:
    def __init__(self, rate, duration, seed, experiment_id, prover_backend, host='localhost', port=3000):
        self.rate = rate
        self.duration = duration
        self.seed = seed
        self.experiment_id = experiment_id
        self.prover_backend = prover_backend
        self.base_url = f"http://{host}:{port}"
        self.stats = []
        
        if seed is not None:
            random.seed(seed)
            
    def run(self):
        print(f"Starting Poisson Workload Generator")
        print(f"Rate: {self.rate} tx/s, Duration: {self.duration}s, Seed: {self.seed}")
        print(f"Target: {self.base_url}")
        
        start_time = time.time()
        end_time = start_time + self.duration
        
        tx_count = 0
        
        try:
            while time.time() < end_time:
                wait_time = random.expovariate(self.rate)
                time.sleep(wait_time)
                
                if time.time() >= end_time:
                    break
                
                # Create a transaction
                # Use Hardhat default account #0 to pass dev_mode whitelist
                private_key = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
                acct = Account.from_key(private_key)
                from_addr = acct.address
                to_addr = "0x" + "02" * 20
                value = random.randint(1, 1000)
                
                # We need to sign a representation of the tx. The Rust Sequencer's UserTransaction::hash() expects 
                # a specific serialization, but since dev_mode bypasses signature content verification as long as the 
                # signature structure is valid, we can provide a syntactically valid ECDSA signature.
                # Here we just sign a dummy message to get valid r, s, v values.
                msg = encode_defunct(text=f"Dummy signature for tx {tx_count}")
                signed_message = acct.sign_message(msg)

                # Flatten the signature to a single 65-byte hex string (r ++ s ++ v)
                r_hex = hex(signed_message.r)[2:].zfill(64)
                s_hex = hex(signed_message.s)[2:].zfill(64)
                v_hex = hex(signed_message.v)[2:].zfill(2)
                signature_flat = "0x" + r_hex + s_hex + v_hex

                tx = {
                    "from": from_addr,
                    "to": to_addr,
                    "value": hex(value),
                    "nonce": tx_count,
                    "gas_price": "0x3b9aca00", # 1 gwei
                    "gas_limit": 21000,
                    "signature": signature_flat,
                    "timestamp": int(time.time())
                }
                
                send_time = datetime.now().isoformat()
                ts_start = time.time()
                try:
                    # Using urllib instead of requests
                    url = f"{self.base_url}/tx"
                    data = json.dumps(tx).encode('utf-8')
                    req = urllib.request.Request(url, data=data, headers={'Content-Type': 'application/json'})
                    
                    with urllib.request.urlopen(req) as response:
                         res_data = response.read()
                         print(res_data.decode('utf-8'), file=sys.stdout)

                    print(json.dumps({"event": "tx_submitted", "tx_hash": tx_count, "timestamp_ms": int(time.time()*1000), "experiment_id": self.experiment_id}), file=sys.stderr)
                    
                    latency = time.time() - ts_start
                    status = "success"
                except Exception as e:
                    print(f"Request Error: {e}", file=sys.stderr)
                    latency = 0
                    status = "error"

                self.stats.append({
                    "tx_id": tx_count,
                    "timestamp": send_time,
                    "latency": latency,
                    "status": status
                })
                
                tx_count += 1
                if tx_count % 10 == 0:
                    print(f"Sent {tx_count} transactions...")

        except KeyboardInterrupt:
            print("Interrupted by user")
            
        print(f"Finished. Total sent: {tx_count}")
        self.save_stats()

    def save_stats(self):
        # Calculate avg latency
        latencies = [s["latency"] for s in self.stats if s["status"] == "success"]
        avg_latency_ms = (sum(latencies) / len(latencies)) * 1000 if latencies else 0

        metrics = {
            "experiment_id": self.experiment_id,
            "source": "workload",
            "prover_backend": self.prover_backend,
            "da_mode": "n/a",
            "latency_metrics": {
                "user_action_latency_ms": avg_latency_ms,
                "l2_l1_latency_ms": 0
            },
            "witness_info": {
                "constraints": 0,
                "witness_size_bytes": 0
            },
            "details": {
                "total_txs": len(self.stats),
                "successful_txs": len(latencies),
                "duration": self.duration,
                "rate": self.rate
            }
        }
        
        # Ensure directory exists
        metrics_root = os.environ.get("METRICS_ROOT", "metrics")
        os.makedirs(metrics_root, exist_ok=True)
        
        filename = os.path.join(metrics_root, f"workload_{self.experiment_id}.json")
        print(f"Saving metrics to {filename}")
        with open(filename, 'w') as f:
            json.dump(metrics, f, indent=2)

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description='Poisson Workload Generator')
    parser.add_argument('--rate', type=float, default=1.0, help='Arrival rate (tx/sec)')
    parser.add_argument('--duration', type=int, default=10, help='Duration in seconds')
    parser.add_argument('--seed', type=int, default=None, help='RNG seed')
    parser.add_argument('--host', type=str, default='localhost', help='Sequencer Host')
    parser.add_argument('--port', type=int, default=3000, help='Sequencer Port')
    parser.add_argument('--experiment_id', type=str, default=f"exp_{int(time.time())}", help='Experiment ID')
    parser.add_argument('--prover_backend', type=str, default="unknown", help='Prover Backend')
    
    args = parser.parse_args()
    
    generator = PoissonWorkloadGenerator(
        args.rate, args.duration, args.seed, 
        args.experiment_id, args.prover_backend,
        args.host, args.port
    )
    generator.run()
