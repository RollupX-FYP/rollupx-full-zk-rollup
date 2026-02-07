//! Transaction Validation Module
//! 
//! This module validates user transactions before they enter the pool.
//! Performs signature verification, nonce checking, and balance validation.

mod validator;
pub use validator::Validator;