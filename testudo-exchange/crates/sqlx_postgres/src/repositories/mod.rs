//! Repository layer for exchange account management
//!
//! This module provides the repository pattern implementation for managing
//! exchange account data with automatic encryption/decryption capabilities.
//!
//! The repository follows SOLID principles:
//! - Single Responsibility: Each repository handles one aggregate root
//! - Open/Closed: Extensible through trait implementations
//! - Liskov Substitution: All implementations are interchangeable
//! - Interface Segregation: Focused traits for specific operations
//! - Dependency Inversion: Depends on abstractions, not concretions

pub mod api_keys;
pub mod errors;
pub mod types;

// Re-export commonly used types and traits
pub use api_keys::{ExchangeAccountRepository, PostgresExchangeAccountRepository};
pub use errors::RepositoryError;
pub use types::{
    CreateExchangeAccountRequest, ExchangeAccountFilter, ExchangeAccountRow,
    ExchangeAccountSummary, ExchangeAccountWithCredentials, UpdateExchangeAccountRequest,
};
