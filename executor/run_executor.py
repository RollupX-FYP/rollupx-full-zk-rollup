#!/usr/bin/env python3
import os
import sys

try:
    import yaml
except ImportError:
    print("[run_executor.py] ERROR: pyyaml not installed. Run `pip install pyyaml`.")
    sys.exit(1)

def load_yaml_to_env(yaml_path):
    if not os.path.exists(yaml_path):
        print(f"[run_executor.py] WARNING: {yaml_path} not found. Proceeding with existing env vars.")
        return

    print(f"[run_executor.py] Loading config from {yaml_path}...")
    with open(yaml_path, 'r') as f:
        config = yaml.safe_load(f)

    if not config:
        return

    mapping = {
        "mode": "EXECUTOR_MODE",
        "grpc_addr": "EXECUTOR_GRPC_ADDR",
        "metrics_root": "METRICS_ROOT",
        "batch_path": "EXECUTOR_BATCH_PATH",
        "prover_path": "EXECUTOR_PROVER_PATH",
        "db_path": "EXECUTOR_DB_PATH",
    }

    for key, env_var in mapping.items():
        if key in config:
            val = str(config[key])
            os.environ[env_var] = val
            print(f"[run_executor.py] Exported {env_var}={val}")

if __name__ == "__main__":
    yaml_config = os.environ.get("EXECUTOR_CONFIG", "executor.yaml")
    load_yaml_to_env(yaml_config)

    binary = sys.argv[1] if len(sys.argv) > 1 else "executor"
    args = sys.argv[1:] if len(sys.argv) > 1 else ["executor"]

    print(f"[run_executor.py] Executing: {' '.join(args)}")
    os.execvp(binary, args)
