//! Batch Creation Module
//! 
//! This module handles batch creation and sealing:
//! - BatchEngine: Creates sealed batches from ordered transactions
//! - Trigger: Determines when batches should be sealed (planned)

mod engine;
mod trigger;

pub use engine::BatchEngine;