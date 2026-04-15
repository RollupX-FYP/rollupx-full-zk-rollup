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
    import eth_utils
    import eth_keys
except ImportError:
    print("Error: eth-account is not installed. Please run `pip install eth-account`")
    sys.exit(1)

def hash_tx(from_addr, to_addr, value, nonce, gas_price, timestamp, boost_bid=None):
    data = bytearray()
    data.extend(eth_utils.to_bytes(hexstr=from_addr))
    data.extend(eth_utils.to_bytes(hexstr=to_addr))
    data.extend(value.to_bytes(32, 'big'))
    data.extend(nonce.to_bytes(8, 'big'))
    data.extend(gas_price.to_bytes(32, 'big'))
    data.extend(timestamp.to_bytes(8, 'big'))
    
    if boost_bid is not None:
        data.extend(boost_bid.to_bytes(32, 'big'))
    else:
        data.extend(b'\x00' * 32)
        
    return eth_utils.keccak(data)

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
        
        private_key = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
        acct = Account.from_key(private_key)
        from_addr = acct.address

        try:
            req = urllib.request.Request(
                self.base_url,
                data=json.dumps({
                    "jsonrpc": "2.0",
                    "method": "eth_getTransactionCount",
                    "params": [from_addr, "latest"],
                    "id": 1
                }).encode("utf-8"),
                headers={"Content-Type": "application/json"}
            )
            with urllib.request.urlopen(req) as response:
                res_data = json.loads(response.read().decode('utf-8'))
                tx_count = int(res_data["result"], 16)
                print(f"Fetched starting nonce: {tx_count}")
        except Exception as e:
            print(f"Failed to fetch nonce, falling back to 0: {e}")
            tx_count = 0
        
        try:
            while time.time() < end_time:
                wait_time = random.expovariate(self.rate)
                time.sleep(wait_time)
                
                if time.time() >= end_time:
                    break
                
                # Create a transaction
                to_addr = "0x" + "02" * 20
                value = random.randint(1, 1000)
                
                timestamp = int(time.time())
                gas_price = int("0x3b9aca00", 16)
                h = hash_tx(from_addr, to_addr, value, tx_count, gas_price, timestamp)
                
                pk_obj = eth_keys.keys.PrivateKey(eth_utils.to_bytes(hexstr=private_key))
                signature = pk_obj.sign_msg_hash(h)

                r_hex = hex(signature.r)[2:].zfill(64)
                s_hex = hex(signature.s)[2:].zfill(64)
                v_hex = hex(signature.v + 27)[2:].zfill(2)
                signature_flat = "0x" + r_hex + s_hex + v_hex

                tx = {
                    "from": from_addr,
                    "to": to_addr,
                    "value": hex(value),
                    "nonce": tx_count,
                    "gas_price": "0x3b9aca00", # 1 gwei
                    "gas_limit": 21000,
                    "signature": signature_flat,
                    "timestamp": timestamp
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
