# RollupX Sequencer

The sequencer component of the RollupX zk-rollup system. It receives user transactions, validates them, orders them using a configurable scheduling policy, and produces sealed batches for execution and L1 submission.

## Architecture

<p align="center">
  <img src="./public/images/architecture.svg" width="600" />
</p>

> The full Mermaid source is in [`architecture.mmd`](./architecture.mmd).

### Data Flow

```
User ──► API Server ──► Validator ──► State Cache (deduct balance, increment nonce)
                                             │
                                             ▼
                                        Transaction Pool
                                             │
L1 Bridge ──► L1 Listener ──► Forced Queue   │
                                   │         │
                                   ▼         ▼
                                Batch Orchestrator
                                       │
                ┌──────────────────────┼──────────────────────┐
                ▼                      ▼                      ▼
            Batch Trigger          Scheduler             Batch Engine
            (when to seal)       (how to order)         (creates batch)
                                                              │
                                                              ▼
                                                        Batch Registry
                                                       (SQLite metadata)
                                                              │
                                                              ▼
                                                      [Executor gRPC]
```

### Component Overview

| Component               | File                      | Description                                                |
| ----------------------- | ------------------------- | ---------------------------------------------------------- |
| **API Server**          | `api/server.rs`           | JSON-RPC endpoint that receives `sendTransaction` calls    |
| **Validator**           | `validation/validator.rs` | Verifies signatures (ECDSA), nonces, and balances          |
| **State Cache**         | `state/cache.rs`          | In-memory account state with pessimistic balance tracking  |
| **Transaction Pool**    | `pool/tx_pool.rs`         | FIFO queue for validated user transactions                 |
| **Forced Queue**        | `pool/forced_queue.rs`    | Priority queue for L1-originated deposits and forced exits |
| **L1 Listener**         | `l1/listener.rs`          | WebSocket listener for L1 bridge contract events           |
| **Batch Orchestrator**  | `batch/orchestrator.rs`   | Coordinates the full batch production pipeline             |
| **Batch Trigger**       | `batch/trigger.rs`        | Determines when to seal batches (forced/size/timeout)      |
| **Scheduler**           | `scheduler/scheduler.rs`  | Orders transactions using the configured policy            |
| **Scheduling Policies** | `scheduler/policies.rs`   | FCFS, Fee-Priority, Time-Boost, Fair BFT                   |
| **Batch Engine**        | `batch/engine.rs`         | Creates sealed batches with sequential IDs                 |
| **Batch Registry**      | `registry/database.rs`    | SQLite database storing batch metadata                     |
| **Config**              | `config.rs`               | TOML configuration loading                                 |

---

## Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (edition 2024)
- SQLite (bundled via `sqlx` — no separate install needed)

### Build

```bash
cargo build
```

### Run

```bash
cargo run
```

The sequencer will:

1. Load configuration from `config/default.toml`
2. Initialize the SQLite batch registry
3. Start the L1 event listener (background)
4. Start the batch orchestrator (background)
5. Start the JSON-RPC API server (foreground)

By default the API listens on `http://127.0.0.1:3000`.

### Run Tests

```bash
cargo test
```

### Shutdown

Press **Ctrl+C** for a graceful shutdown.

---

## Configuration

All settings are in [`config/default.toml`](./config/default.toml):

```toml
[batch]
max_batch_size = 100            # Maximum transactions per batch
timeout_interval_ms = 5000      # Seal partial batch after this timeout (ms)
min_batch_size = 10             # Minimum txs before timeout seal fires
max_gas_limit = 30000000        # Max cumulative gas per batch (30M)

[scheduling]
policy_type = "FCFS"            # Scheduling policy (see below)
# time_window_ms = 5000         # Only used for "TimeBoost" policy

[api]
host = "127.0.0.1"             # API bind address
port = 3000                     # API port

[l1]
rpc_url = "wss://sepolia.infura.io/ws/v3/YOUR_KEY"  # L1 WebSocket RPC
bridge_address = "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb"  # Bridge contract
start_block = 18500000          # L1 block to start monitoring from

[database]
url = "sqlite://sequencer.db"   # SQLite database path for batch registry

[executor]
grpc_url = "http://127.0.0.1:50051" # Executor PublishBatch endpoint
```

## gRPC Integration

When a batch is sealed, the sequencer publishes it to the executor using `RollupService/PublishBatch`.

Required runtime order:

1. Start executor gRPC server on `EXECUTOR_GRPC_ADDR` (default `127.0.0.1:50051`).
2. Start sequencer (uses `[executor].grpc_url`).
3. Start submitter in default gRPC mode (`COMM_MODE=grpc`) pointing to the same executor URL.

### Configuration Reference

#### `[batch]` — Batch Creation

| Parameter             | Type    | Default  | Description                                  |
| --------------------- | ------- | -------- | -------------------------------------------- |
| `max_batch_size`      | `usize` | 100      | Maximum number of transactions per batch     |
| `timeout_interval_ms` | `u64`   | 5000     | Milliseconds before sealing a partial batch  |
| `min_batch_size`      | `usize` | 10       | Minimum transactions before timeout can fire |
| `max_gas_limit`       | `u64`   | 30000000 | Maximum cumulative gas per batch             |

#### `[scheduling]` — Transaction Ordering

| Parameter        | Type     | Default  | Description                               |
| ---------------- | -------- | -------- | ----------------------------------------- |
| `policy_type`    | `String` | `"FCFS"` | Scheduling policy (see table below)       |
| `time_window_ms` | `u64`    | 5000     | Time window size (only for `"TimeBoost"`) |

#### `[api]` — JSON-RPC Server

| Parameter | Type     | Default       | Description           |
| --------- | -------- | ------------- | --------------------- |
| `host`    | `String` | `"127.0.0.1"` | IP address to bind to |
| `port`    | `u16`    | 3000          | TCP port to listen on |

#### `[l1]` — Layer 1 Integration

| Parameter        | Type     | Default | Description                              |
| ---------------- | -------- | ------- | ---------------------------------------- |
| `rpc_url`        | `String` | —       | Ethereum L1 WebSocket RPC endpoint       |
| `bridge_address` | `String` | —       | Address of the RollupBridge contract     |
| `start_block`    | `u64`    | —       | L1 block number to start monitoring from |

#### `[database]` — Batch Registry

| Parameter | Type     | Default                   | Description           |
| --------- | -------- | ------------------------- | --------------------- |
| `url`     | `String` | `"sqlite://sequencer.db"` | SQLite connection URL |

---

## Scheduling Policies

The sequencer supports four configurable scheduling policies that determine how **normal** transactions are ordered within a batch. Set `policy_type` in `[scheduling]`:

| Policy           | Config Value    | Ordering Rule                            | Best For             |
| ---------------- | --------------- | ---------------------------------------- | -------------------- |
| **FCFS**         | `"FCFS"`        | Arrival order (no reordering)            | Simplicity, fairness |
| **Fee Priority** | `"FeePriority"` | Highest `gas_price` first                | Revenue maximization |
| **Time-Boost**   | `"TimeBoost"`   | Time windows + `boost_bid` premium       | SLA guarantees       |
| **Fair BFT**     | `"FairBFT"`     | Strictly by `timestamp` (earliest first) | MEV resistance       |

> **Important:** Forced transactions from L1 (deposits and forced exits) **always** come first in every batch, regardless of the selected policy. This guarantees censorship resistance.

### FCFS (First-Come-First-Served)

Maintains the original submission order. No reordering. Simple and predictable.

### Fee Priority

Sorts transactions by `gas_price` in descending order. Users willing to pay higher fees get priority. Maximizes sequencer revenue.

### Time-Boost

Divides time into configurable windows (default 5 seconds). Within each window, transactions are sorted by:

1. `boost_bid` (descending) — optional premium bid field
2. `gas_price` (descending) — fallback
3. FCFS — final tie-breaker

Requires the `time_window_ms` parameter:

```toml
[scheduling]
policy_type = "TimeBoost"
time_window_ms = 5000
```

### Fair BFT Ordering

Orders transactions strictly by their `timestamp` field (earliest first). Provides time-based fairness and MEV resistance. Current implementation is for a single-node sequencer; a multi-node version would use BFT consensus for timestamp agreement.

---

## Batch Trigger Conditions

The batch orchestrator evaluates three trigger conditions in priority order:

| Priority | Trigger                 | Condition                                    | Rationale               |
| -------- | ----------------------- | -------------------------------------------- | ----------------------- |
| 1        | **Forced Transactions** | Any L1 tx in forced queue                    | Censorship resistance   |
| 2        | **Size Threshold**      | Pool size ≥ `max_batch_size`                 | Throughput optimization |
| 3        | **Timeout**             | Elapsed ≥ `timeout_interval_ms` AND pool > 0 | Latency guarantee       |

---

## API Reference

The sequencer exposes a single JSON-RPC 2.0 endpoint at `POST /`.

### `sendTransaction`

Submit a signed transaction to the sequencer.

**Request:**

```json
{
  "jsonrpc": "2.0",
  "method": "sendTransaction",
  "params": {
    "from": "0x...",
    "to": "0x...",
    "value": "0x...",
    "nonce": 0,
    "gas_price": "0x...",
    "gas_limit": 21000,
    "signature": { "r": "0x...", "s": "0x...", "v": 27 },
    "timestamp": 1712345678,
    "boost_bid": null
  },
  "id": 1
}
```

**Success Response:**

```json
{
  "jsonrpc": "2.0",
  "result": {
    "tx_hash": "0x...",
    "status": "Accepted",
    "timestamp": 1712345679
  },
  "id": 1
}
```

**Rejection Response:**

```json
{
  "jsonrpc": "2.0",
  "result": {
    "tx_hash": "0x...",
    "status": {
      "Rejected": {
        "reason": "Invalid nonce: expected 5, got 3"
      }
    },
    "timestamp": 1712345679
  },
  "id": 1
}
```

### Validation Errors

| Error                 | Description                                                    |
| --------------------- | -------------------------------------------------------------- |
| `InvalidSignature`    | ECDSA signature recovery failed or signer doesn't match `from` |
| `InvalidNonce`        | Nonce doesn't match the expected sequential value              |
| `InsufficientBalance` | Account balance < `value + gas_price × gas_limit`              |

---

## Project Structure

```
sequencer/
├── src/
│   ├── main.rs                  # Entry point — initializes and starts all components
│   ├── lib.rs                   # Module exports
│   ├── types.rs                 # Shared types (Transaction, Batch, AccountState, etc.)
│   ├── config.rs                # TOML configuration structs and loader
│   │
│   ├── api/                     # ── Sequencer API ──
│   │   ├── mod.rs
│   │   └── server.rs            #   JSON-RPC server with sendTransaction handler
│   │
│   ├── validation/              # ── Validity Checker ──
│   │   ├── mod.rs
│   │   └── validator.rs         #   Signature, nonce, and balance validation
│   │
│   ├── state/                   # ── Local State Cache ──
│   │   ├── mod.rs
│   │   └── cache.rs             #   In-memory account state (pessimistic tracking)
│   │
│   ├── pool/                    # ── Transaction Pools ──
│   │   ├── mod.rs
│   │   ├── tx_pool.rs           #   Normal user transaction pool (FIFO)
│   │   └── forced_queue.rs      #   Forced L1 transaction queue (priority)
│   │
│   ├── l1/                      # ── L1 Integration ──
│   │   ├── mod.rs
│   │   └── listener.rs          #   WebSocket listener for bridge contract events
│   │
│   ├── scheduler/               # ── Scheduler (Policy Engine) ──
│   │   ├── mod.rs
│   │   ├── scheduler.rs         #   Strategy-pattern scheduler
│   │   ├── policies.rs          #   FCFS, FeePriority, TimeBoost, FairBFT policies
│   │   └── tests.rs             #   Unit tests for all policies
│   │
│   ├── batch/                   # ── Batch Production ──
│   │   ├── mod.rs
│   │   ├── engine.rs            #   Batch creation with sequential IDs
│   │   ├── trigger.rs           #   Trigger conditions (forced / size / timeout)
│   │   └── orchestrator.rs      #   Pipeline coordinator (trigger → pull → schedule → seal)
│   │
│   └── registry/                # ── Batch Registry ──
│       ├── mod.rs
│       └── database.rs          #   SQLite metadata store
│
├── config/
│   └── default.toml             # Default configuration
│
├── architecture.mmd             # Mermaid architecture diagram source
├── .env.example                 # Environment variables template
├── .gitignore
├── Cargo.lock
├── Cargo.toml                   # Dependencies
└── README.md
```

---

## Key Design Decisions

### Pessimistic Balance Tracking

When a transaction is validated and accepted, the state cache immediately deducts the full transaction cost (`value + gas_price × gas_limit`) and increments the nonce. This prevents double-spend attacks from concurrent submissions — if a user rapidly sends two transactions that individually pass balance checks, the second one will see the already-deducted balance and fail.

### Forced Transaction Priority

Forced transactions from L1 (deposits, forced exits) are **always** included first in every batch. This guarantees censorship resistance — even if the sequencer tries to censor a user, they can submit their transaction on L1 and it will be forcibly included. Forced transactions that exceed the batch gas limit are re-queued for the next batch (never dropped).

### Strategy Pattern for Policies

The scheduler uses the Strategy design pattern (`Box<dyn SchedulingPolicy>`) so policies can be swapped at startup via configuration without code changes. Adding a new policy requires only implementing the `SchedulingPolicy` trait and registering it in the factory function.

### Batch Trigger Hierarchy

Triggers are evaluated in strict priority order (forced → size → timeout) to balance between censorship resistance, throughput, and latency. The timeout trigger requires at least one transaction to avoid producing empty batches.

---

## Dependencies

| Crate                            | Purpose                                  |
| -------------------------------- | ---------------------------------------- |
| `tokio`                          | Async runtime                            |
| `axum`                           | HTTP server for JSON-RPC API             |
| `serde` / `serde_json`           | Serialization/deserialization            |
| `ethers`                         | Ethereum types, signatures, L1 WebSocket |
| `sqlx`                           | SQLite database (batch registry)         |
| `toml`                           | Configuration file parsing               |
| `tracing` / `tracing-subscriber` | Structured logging                       |
| `anyhow` / `thiserror`           | Error handling                           |
| `chrono`                         | Timestamps                               |

---

## License

See [LICENSE](./LICENSE).
