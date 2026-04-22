
## Docker E2E Profiles

RollupX provides two Compose profiles to ease local development:

### Core Docker Stack (Verified)
The core profile launches the `l1-node` (Hardhat dev node), `setup` (contracts deployment), and `sequencer`. This profile is verified and working.

```bash
docker compose --profile core up --build -d
```

### Full Docker Stack (Blocked)
The full profile launches the `executor` and `submitter`. This profile is currently blocked by a pre-existing executor toolchain/lockfile incompatibility.

```bash
docker compose --profile full up --build
```
*Note: This will fail on the `executor` build step. This PR does not attempt to fix the source-build compatibility issue to isolate Docker/config work from dependency surgery.*

### Configurations
All services utilize `.yaml` or `.toml` configuration files. You can override configurations using environment variables. For example, `SEQUENCER_CONFIG=/app/sequencer.yaml` specifies the sequencer config path, with precedence given to the environment variable. It defaults to `config/default.toml`.
