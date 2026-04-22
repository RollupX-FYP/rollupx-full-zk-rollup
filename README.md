
## Docker E2E Stack
The local Docker setup is split into `core` and `full` profiles. The `core` profile is verified and includes the local L1 node, setup node, and sequencer. The `full` profile additionally includes executor-dependent services, but is currently blocked by a pre-existing executor toolchain/lockfile incompatibility. This PR intentionally does not modify the executor dependency graph or pinned Rust toolchain, so that Docker/configuration changes remain isolated from upstream executor build issues.

### Running the Core Stack
To launch the verified core stack, run:
```bash
docker compose --profile core up --build -d
```
This will start the local Ethereum node, deploy the contracts, and start the Sequencer. The project can still be used in **core Docker mode**.

### Running the Full Stack
If you wish to attempt running the full stack, run:
```bash
docker compose --profile full up --build
```
*Note: This will currently fail on the `executor` build step. This is a known, pre-existing issue.*

## Known Blockers
This PR **does not fix executor source-build compatibility**. It ensures Docker/core stack usability and clean separation of blocked services.
* **Service:** `executor`
* **Reason:** Pre-existing toolchain / lockfile / dependency incompatibility. The executor's `ruint` dependency requires `edition2024` on a newer nightly compiler, conflicting with its locked `nightly-2024-08-01` toolchain. This is **not introduced by this PR**.
* **Untouched Files:** `executor/Cargo.toml`, `executor/Cargo.lock`, and `executor/rust-toolchain.toml` were intentionally left untouched in this PR to avoid mixing Docker/config work with dependency surgery.

A follow-up task is required to repair executor lockfile/toolchain compatibility for source and Docker builds.

## Sequencer Configuration
The Sequencer accepts YAML or TOML configurations. By default, it loads `config/default.toml`.
You can override the configuration file by setting the `SEQUENCER_CONFIG` environment variable. The environment variable takes precedence over the default path.
For example, in Docker, it mounts `sequencer.docker.yaml` and sets `SEQUENCER_CONFIG=/app/sequencer.yaml`.
