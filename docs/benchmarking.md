# Benchmarking in RollupX

RollupX provides a comprehensive, automated benchmarking pipeline designed to evaluate the performance, cost, and fairness of a modular ZK-Rollup under various configurations and workloads.

## Overview

The benchmarking infrastructure is centered around the `benchmark-suite/` directory and is designed for:
- **Reproducibility**: Using fixed seeds and a containerized stack to ensure consistent results.
- **Heterogeneity**: Testing with different transaction classes (Light, Medium, Heavy).
- **Automation**: One-command execution of complex experiment matrices.
- **Observability**: Collecting granular metrics from the Sequencer, Executor, and Submitter.

## Key Components

- **Workload Generator**: A Python-based Poisson generator that simulates realistic transaction arrival patterns. See [docs/workload-generator.md](workload-generator.md).
- **Orchestration Scripts**: Bash scripts that manage the Docker stack lifecycle and experiment execution.
- **Data Tools**: Python scripts for aggregating raw JSON/CSV metrics into analyzed reports and plots. See [docs/data-tools.md](data-tools.md).

## Running Benchmarks

### Primary Entry Point

The most common way to run benchmarks is using the `run_matrix.sh` script in the `benchmark-suite/` directory.

```bash
# Run a quick smoke test
bash benchmark-suite/scripts/run_matrix.sh --phase smoke

# Run the full feasibility study (takes several hours)
bash benchmark-suite/scripts/run_matrix.sh --phase feasibility
```

### Infrastructure vs. Workload Experiments

1. **Infrastructure Experiments**: Vary factors like batch size, timeout, scheduling policy, DA mode, and prover backend. These require a **stack restart** between runs to inject new environment variables.
2. **Workload Experiments**: Vary factors like input rate and transaction mix. These can often be run against a stable, already-running stack.

## Economic Modeling (Cost Curves)

A specialized tool is available for generating "cost curves" that help determine the optimal batch size for a given L1 gas price and prover cost.

```bash
bash benchmark-suite/scripts/run_cost_curve_quick.sh
```

## Detailed Documentation

For deep dives into specific areas, see:
- [benchmark-suite/README.md](../benchmark-suite/README.md): Detailed CLI and orchestration workings.
- [benchmark-suite/PLAN.md](../benchmark-suite/PLAN.md): Research questions, hypotheses, and experimental design.
- [benchmark-suite/rollupx_workload_generation_methodology.md](../benchmark-suite/rollupx_workload_generation_methodology.md): The mathematical basis for traffic generation.
