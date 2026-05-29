//! Error handling module for the exchange system
//!
//! This module provides centralized error handling types that unify error handling
//! across all exchange components while maintaining user safety and system observability.
//!
//! # Design Principles
//!
//! - **Single Responsibility**: Each error type has one clear purpose
//! - **Open/Closed**: Extensible via enum variants and trait implementations
//! - **Liskov Substitution**: All error variants behave consistently
//! - **Interface Segregation**: Clean, focused trait implementations
//! - **Dependency Inversion**: Error handling depends on abstractions
//!
//! # Error Categories
//!
//! - **Connection**: Network and connectivity issues
//! - **Authentication**: Security and credential failures
//! - **Balance**: Insufficient funds or balance checks
//! - **Order**: Order processing and validation failures
//! - **Rate Limit**: API throttling and quota management
//! - **Availability**: Service and exchange availability issues
//!
//! # Migration Guide
//!
//! Replace existing error types with `ExchangeError`:
//!
//! ```rust,ignore
//! // Before
//! Result<OrderResponse, RoutingError>
//! Result<(), EncryptionError>
//! Result<User, RepositoryError>
//!
//! // After
//! Result<OrderResponse, ExchangeError>
//! Result<(), ExchangeError>
//! Result<User, ExchangeError>
//! ```
//!
//! All existing error types automatically convert via `From` traits.

// @anchor exchange:common_utils:mod
// @tags infra

pub mod exchange;

pub use exchange::ExchangeError;
