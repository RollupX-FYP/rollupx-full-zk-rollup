//! API Module
//! 
//! This module handles the JSON-RPC API for receiving user transactions.
//! It provides the HTTP endpoint that clients use to submit transactions.

mod server;
pub use server::Server;