//! Batch Creation Module
//!
//! This module handles batch creation, sealing, and trigger logic:
//! - `BatchEngine`: Creates sealed batches from ordered transactions
//! - `BatchTrigger`: Determines when batches should be sealed
//!   (time-based, size-based, forced-tx triggers)
//! - `BatchOrchestrator`: Coordinates the full batch production pipeline

mod engine;
pub mod orchestrator;
pub mod trigger;

pub use engine::BatchEngine;
pub use orchestrator::BatchOrchestrator;
pub use trigger::BatchTrigger;
