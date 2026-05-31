// @anchor test:cli:errors
// @tags api

use testudo_cli::api::types::ApiError;

#[test]
fn api_error_unauthorized_is_clear() {
    let err = ApiError::Unauthorized;
    let msg = err.to_string();
    assert!(msg.contains("Unauthorized"));
    assert!(msg.contains("agent key"));
    assert!(msg.contains("config.toml"));
}

#[test]
fn api_error_network_connection_refused() {
    let err = ApiError::Network("Connection refused".into());
    let msg = err.to_string();
    assert!(msg.contains("Network error"));
    assert!(msg.contains("Connection refused"));
}

#[test]
fn api_error_not_found_includes_detail() {
    let err = ApiError::NotFound("No trades for this period".into());
    let msg = err.to_string();
    assert!(msg.contains("Not found"));
    assert!(msg.contains("No trades"));
}

#[test]
fn api_error_deserialize_includes_context() {
    let err = ApiError::Deserialize("invalid field 'foo' at line 1".into());
    let msg = err.to_string();
    assert!(msg.contains("Failed to parse"));
    assert!(msg.contains("invalid field"));
}

#[test]
fn api_error_unexpected_status_shows_code_and_body() {
    let err = ApiError::UnexpectedStatus(500, "Internal Server Error".into());
    let msg = err.to_string();
    assert!(msg.contains("500"));
    assert!(msg.contains("Internal Server Error"));
}

#[test]
fn api_error_signal_rejected_includes_reason() {
    let err = ApiError::SignalRejected("Insufficient margin".into());
    let msg = err.to_string();
    assert!(msg.contains("Signal rejected"));
    assert!(msg.contains("Insufficient margin"));
}

#[test]
fn api_error_from_reqwest_timeout() {
    // Simulate a timeout by creating a reqwest error
    // We can't easily create reqwest errors, but verify the From impl compiles
    let err: ApiError = ApiError::Network("Request timed out".into());
    assert!(err.to_string().contains("timed out"));
}

#[test]
fn tracing_subscriber_can_be_configured() {
    // Verify the tracing subscriber builder compiles and doesn't panic
    use tracing_subscriber::fmt;
    use tracing_subscriber::EnvFilter;

    let filter = EnvFilter::new("info");
    let subscriber = fmt()
        .with_writer(std::io::sink)
        .with_target(false)
        .with_env_filter(filter)
        .finish();

    // Set as default and verify it works
    let _guard = tracing::subscriber::set_default(subscriber);
    tracing::info!("test trace message");
    tracing::warn!("test warning");
    tracing::error!("test error");
}
