//! Error types for the sheaf engine.

/// Top-level error type.
#[derive(Debug, thiserror::Error)]
pub enum SheafError {
    #[error("alignment error: {0}")]
    Alignment(String),

    #[error("graph error: {0}")]
    Graph(String),

    #[error("laplacian error: {0}")]
    Laplacian(String),

    #[error("signal extraction error: {0}")]
    SignalExtraction(String),

    #[error("tick source error: {0}")]
    TickSource(#[from] crate::source::TickSourceError),

    #[error("gRPC error: {0}")]
    Grpc(#[from] tonic::Status),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("configuration error: {0}")]
    Config(String),
}
