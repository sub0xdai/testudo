//! β005: Universal response patterns that eliminate error handling duplication
//!
//! This module implements semantic compression by creating standardized response
//! builders that make error handling omissions impossible.

use actix_web::{HttpResponse, Result};
use serde_json::Value;

use crate::types::auth::ErrorResponse;

/// Universal response builder that enforces consistent error handling
pub struct ResponseBuilder;

impl ResponseBuilder {
    /// Create a standardized success response with optional data
    pub fn success<T: serde::Serialize>(data: T) -> Result<HttpResponse> {
        Ok(HttpResponse::Ok().json(data))
    }

    /// Create a standardized created response for POST endpoints
    pub fn created<T: serde::Serialize>(data: T) -> Result<HttpResponse> {
        Ok(HttpResponse::Created().json(data))
    }

    /// Create a standardized no content response for DELETE endpoints
    pub fn no_content() -> Result<HttpResponse> {
        Ok(HttpResponse::NoContent().finish())
    }

    /// Create a standardized validation error response
    pub fn validation_error(errors: Value) -> Result<HttpResponse> {
        Ok(HttpResponse::BadRequest().json(ErrorResponse::validation_error(errors)))
    }

    /// Create a standardized forbidden response
    pub fn forbidden(message: &str) -> Result<HttpResponse> {
        Ok(HttpResponse::Forbidden().json(ErrorResponse::new("forbidden", message)))
    }

    /// Create a standardized not found response
    pub fn not_found(resource: &str) -> Result<HttpResponse> {
        Ok(HttpResponse::NotFound().json(ErrorResponse::new(
            "not_found",
            &format!("{} not found", resource),
        )))
    }

    /// Create a standardized bad request response
    pub fn bad_request(message: &str) -> Result<HttpResponse> {
        Ok(HttpResponse::BadRequest().json(ErrorResponse::new("bad_request", message)))
    }

    /// Create a standardized unauthorized response
    pub fn unauthorized(message: &str) -> Result<HttpResponse> {
        Ok(HttpResponse::Unauthorized().json(ErrorResponse::new("unauthorized", message)))
    }

    /// Create a standardized internal server error response
    pub fn internal_error() -> Result<HttpResponse> {
        Ok(HttpResponse::InternalServerError().json(ErrorResponse::internal_error()))
    }

    /// Create a standardized mock response for Phase 1 implementations
    pub fn mock_success<T: serde::Serialize>(data: T, message: &str) -> Result<HttpResponse> {
        let mut response = serde_json::to_value(data).unwrap_or(Value::Null);
        if let Value::Object(ref mut map) = response {
            map.insert(
                "_mock_message".to_string(),
                Value::String(format!("{} (Phase 1 simulation)", message)),
            );
        }
        Ok(HttpResponse::Ok().json(response))
    }
}

/// Macro for eliminating boilerplate in route handler error handling
#[macro_export]
macro_rules! handle_validation {
    ($request:expr) => {
        if let Err(validation_errors) = $request.validate() {
            let errors = serde_json::to_value(validation_errors).unwrap_or_default();
            return $crate::utils::responses::ResponseBuilder::validation_error(errors);
        }
    };
}

/// Macro for consistent logging patterns
#[macro_export]
macro_rules! log_route_action {
    ($action:expr, $user_id:expr, $resource:expr) => {
        tracing::info!(
            "{} for user {} on {} - Phase 1 simulation",
            $action,
            $user_id,
            $resource
        );
    };
    ($action:expr, $user_id:expr, $resource:expr, $id:expr) => {
        tracing::info!(
            "{} {} for user {} on {} - Phase 1 simulation",
            $action,
            $id,
            $user_id,
            $resource
        );
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_response_builder_success() {
        let data = serde_json::json!({"test": "data"});
        let response = ResponseBuilder::success(data).unwrap();
        assert_eq!(response.status(), 200);
    }

    #[test]
    fn test_response_builder_created() {
        let data = serde_json::json!({"id": 123});
        let response = ResponseBuilder::created(data).unwrap();
        assert_eq!(response.status(), 201);
    }

    #[test]
    fn test_response_builder_no_content() {
        let response = ResponseBuilder::no_content().unwrap();
        assert_eq!(response.status(), 204);
    }

    #[test]
    fn test_response_builder_forbidden() {
        let response = ResponseBuilder::forbidden("Access denied").unwrap();
        assert_eq!(response.status(), 403);
    }

    #[test]
    fn test_mock_success_response() {
        let data = serde_json::json!({"result": "test"});
        let response = ResponseBuilder::mock_success(data, "Test completed").unwrap();
        assert_eq!(response.status(), 200);
    }
}
