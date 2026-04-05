//! Batch Metadata Registry Module
//!
//! This module implements a persistent SQLite database registry for storing
//! batch metadata. The registry allows querying batch information without
//! loading full transaction data, which is useful for:
//! - Monitoring and dashboards
//! - Auditing batch production history
//! - Debugging scheduling policy behavior
//! - Tracking sequencer performance metrics
//!
//! # Database Schema
//! ```sql
//! CREATE TABLE IF NOT EXISTS batches (
//!     batch_id        INTEGER PRIMARY KEY,
//!     tx_count        INTEGER NOT NULL,
//!     forced_tx_count INTEGER NOT NULL,
//!     timestamp       INTEGER NOT NULL,
//!     scheduling_policy TEXT NOT NULL
//! );
//! ```

use crate::BatchMetadata;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::str::FromStr;
use tracing::info;

/// Batch metadata registry backed by SQLite
///
/// Stores lightweight metadata for each sealed batch in a persistent
/// database. This enables historical queries and operational monitoring
/// without loading full batch transaction data.
///
/// # Connection Pooling
/// Uses `sqlx::SqlitePool` for connection pooling, allowing multiple
/// concurrent database operations without connection overhead.
pub struct Registry {
    /// SQLite connection pool
    pool: SqlitePool,
}

impl Registry {
    /// Creates a new registry and initializes the database schema
    ///
    /// Opens a connection pool to the specified SQLite database and
    /// runs the migration to create the `batches` table if it doesn't
    /// already exist.
    ///
    /// # Arguments
    /// * `database_url` - SQLite connection URL (e.g., "sqlite://sequencer.db")
    ///
    /// # Returns
    /// * `Ok(Registry)` if the database was successfully opened and initialized
    /// * `Err` if the connection or migration fails
    ///
    /// # Example
    /// ```ignore
    /// let registry = Registry::new("sqlite://sequencer.db").await?;
    /// ```
    pub async fn new(database_url: &str) -> anyhow::Result<Self> {
        info!("Initializing batch registry at {}", database_url);

        // Configure options to create the database file if it doesn't exist
        let options = SqliteConnectOptions::from_str(database_url)?
            .create_if_missing(true);

        // Create a connection pool with sensible defaults for a sequencer
        // - max_connections: 5 is sufficient since we have a single writer
        //   (the orchestrator) and occasional readers (API queries)
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;

        // Run the schema migration to create the batches table
        // IF NOT EXISTS ensures this is idempotent (safe to run multiple times)
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS batches (
                batch_id         INTEGER PRIMARY KEY,
                tx_count         INTEGER NOT NULL,
                forced_tx_count  INTEGER NOT NULL,
                timestamp        INTEGER NOT NULL,
                scheduling_policy TEXT NOT NULL
            )"
        )
        .execute(&pool)
        .await?;

        info!("Batch registry initialized successfully");
        Ok(Self { pool })
    }

    /// Store batch metadata to the database
    ///
    /// Inserts a new row into the `batches` table with the metadata for
    /// a sealed batch. Called by the orchestrator after each batch is created.
    ///
    /// # Arguments
    /// * `metadata` - Batch metadata to persist
    ///
    /// # Returns
    /// * `Ok(())` if the metadata was successfully stored
    /// * `Err` if the database insert fails (e.g., duplicate batch_id)
    pub async fn store(&self, metadata: &BatchMetadata) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO batches (batch_id, tx_count, forced_tx_count, timestamp, scheduling_policy)
             VALUES (?, ?, ?, ?, ?)"
        )
        .bind(metadata.batch_id as i64)
        .bind(metadata.tx_count as i64)
        .bind(metadata.forced_tx_count as i64)
        .bind(metadata.timestamp as i64)
        .bind(&metadata.scheduling_policy)
        .execute(&self.pool)
        .await?;

        info!("Stored metadata for batch #{}", metadata.batch_id);
        Ok(())
    }

    /// Retrieve metadata for a specific batch by ID
    ///
    /// # Arguments
    /// * `batch_id` - The unique batch identifier to look up
    ///
    /// # Returns
    /// * `Ok(Some(BatchMetadata))` if the batch exists
    /// * `Ok(None)` if no batch with that ID was found
    /// * `Err` if the database query fails
    pub async fn get_batch(&self, batch_id: u64) -> anyhow::Result<Option<BatchMetadata>> {
        let row = sqlx::query_as::<_, BatchRow>(
            "SELECT batch_id, tx_count, forced_tx_count, timestamp, scheduling_policy
             FROM batches WHERE batch_id = ?"
        )
        .bind(batch_id as i64)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.into()))
    }

    /// Retrieve the most recent batch metadata entries
    ///
    /// Returns batches in descending order by batch_id (newest first).
    /// Useful for monitoring dashboards and recent activity queries.
    ///
    /// # Arguments
    /// * `limit` - Maximum number of entries to return
    ///
    /// # Returns
    /// A vector of batch metadata, newest first
    pub async fn get_latest_batches(&self, limit: u32) -> anyhow::Result<Vec<BatchMetadata>> {
        let rows = sqlx::query_as::<_, BatchRow>(
            "SELECT batch_id, tx_count, forced_tx_count, timestamp, scheduling_policy
             FROM batches ORDER BY batch_id DESC LIMIT ?"
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    /// Get the total number of batches stored in the registry
    ///
    /// # Returns
    /// The count of all batches ever produced by this sequencer
    pub async fn get_batch_count(&self) -> anyhow::Result<u64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM batches")
            .fetch_one(&self.pool)
            .await?;

        Ok(row.0 as u64)
    }
}

/// Internal row struct for SQLite query deserialization
///
/// Maps database columns to Rust fields. The `From` impl converts
/// this to the public `BatchMetadata` type.
#[derive(sqlx::FromRow)]
struct BatchRow {
    batch_id: i64,
    tx_count: i64,
    forced_tx_count: i64,
    timestamp: i64,
    scheduling_policy: String,
}

/// Convert a database row into the public BatchMetadata type
impl From<BatchRow> for BatchMetadata {
    fn from(row: BatchRow) -> Self {
        BatchMetadata {
            batch_id: row.batch_id as u64,
            tx_count: row.tx_count as usize,
            forced_tx_count: row.forced_tx_count as usize,
            timestamp: row.timestamp as u64,
            scheduling_policy: row.scheduling_policy,
        }
    }
}