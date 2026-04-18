import os
import yaml

def load_config():
    config_path = os.environ.get("DATATOOLS_CONFIG", "data-tools.yaml")
    # If run from inside data-tools directory
    if not os.path.exists(config_path) and os.path.exists("data-tools/" + config_path):
        config_path = "data-tools/" + config_path
    elif not os.path.exists(config_path) and os.path.exists("../data-tools/" + config_path):
         config_path = "../data-tools/" + config_path

    if os.path.exists(config_path):
        with open(config_path, "r") as f:
            return yaml.safe_load(f)
    return {
        "metrics_root": "metrics",
        "output_dir": "figures"
    }
