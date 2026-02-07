//! Batch Registry Module
//! 
//! This module provides a database registry for storing batch metadata.
//! Allows querying batch information without loading full transaction data.

mod database;
pub use database::Registry;