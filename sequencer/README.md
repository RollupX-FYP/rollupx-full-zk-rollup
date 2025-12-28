# Sequencer

A Rust-based sequencer implementation for Layer 2 rollup systems.

## Architecture

<p align="center">
  <img src="./public/images/architecture.png" width="600" />
</p>

## Project Structure

```
sequencer/
├── src/
│   ├── main.rs                 # Entry point - starts all components
│   ├── lib.rs                  # Module exports
│   ├── types.rs                # All shared types (Transaction, Address, etc.)
│   ├── config.rs               # Configuration structs
│   │
│   ├── api/                    # Sequencer API
│   │   ├── mod.rs
│   │   └── server.rs           # JSON-RPC server
│   │
│   ├── validation/             # Validity Checker
│   │   ├── mod.rs
│   │   └── validator.rs        # Signature, nonce, balance checks
│   │
│   ├── state/                  # Local State Cache
│   │   ├── mod.rs
│   │   └── cache.rs            # In-memory account state
│   │
│   ├── pool/                   # Transaction Management
│   │   ├── mod.rs
│   │   ├── tx_pool.rs          # Normal transaction pool
│   │   └── forced_queue.rs     # Forced transaction queue
│   │
│   ├── l1/                     # L1 Integration
│   │   ├── mod.rs
│   │   └── listener.rs         # L1 event listener
│   │
│   ├── scheduler/              # Scheduler
│   │   ├── mod.rs
│   │   ├── scheduler.rs        # Main scheduling logic
│   │   └── policies.rs         # FCFS & Fee-Priority policies
│   │
│   ├── batch/                  # Batch Engine
│   │   ├── mod.rs
│   │   ├── engine.rs           # Batch assembly
│   │   └── trigger.rs          # Size/timeout triggers
│   │
│   └── registry/               # Batch Registry
│       ├── mod.rs
│       └── database.rs         # Store batch metadata
│
├── config/
│   └── default.toml            # Configuration file
│
├── .env.example                # Environment variables template
├── .gitignore
├── Cargo.lock
├── Cargo.toml                  # Dependencies
└── README.md
```