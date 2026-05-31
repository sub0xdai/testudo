// @anchor infra:cli:api:client
// @tags api

//! Reqwest HTTP client with X-Agent-Key header injection.

use crate::api::types::ApiError;
use crate::config::ApiConfig;
use reqwest::{Method, RequestBuilder, StatusCode};
use serde::de::DeserializeOwned;
use std::time::Duration;

/// Typed REST client for the Testudo backend API.
pub struct ApiClient {
    http: reqwest::Client,
    base_url: String,
    agent_key: String,
}

impl ApiClient {
    pub fn new(config: &ApiConfig) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to build reqwest client");

        Self {
            http,
            base_url: config.base_url.trim_end_matches('/').to_string(),
            agent_key: config.agent_key.clone(),
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn agent_key(&self) -> &str {
        &self.agent_key
    }

    fn request(&self, method: Method, path: &str) -> RequestBuilder {
        self.http
            .request(method, format!("{}{}", self.base_url, path))
            .header("X-Agent-Key", &self.agent_key)
            .header("Content-Type", "application/json")
    }

    pub async fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T, ApiError> {
        let resp = self.request(Method::GET, path).send().await?;
        Self::handle_json_response(resp).await
    }

    pub async fn get_text(&self, path: &str) -> Result<String, ApiError> {
        let resp = self.request(Method::GET, path).send().await?;
        Self::handle_text_response(resp).await
    }

    pub async fn post_json<T: DeserializeOwned, B: serde::Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, ApiError> {
        let resp = self.request(Method::POST, path).json(body).send().await?;
        Self::handle_json_response(resp).await
    }

    async fn handle_json_response<T: DeserializeOwned>(
        resp: reqwest::Response,
    ) -> Result<T, ApiError> {
        let status = resp.status();
        match status {
            StatusCode::OK | StatusCode::CREATED => {
                let body = resp.text().await?;
                serde_json::from_str(&body)
                    .map_err(|e| ApiError::Deserialize(format!("{}: {}", e, body)))
            }
            StatusCode::UNAUTHORIZED => Err(ApiError::Unauthorized),
            StatusCode::NOT_FOUND => {
                let body = resp.text().await.unwrap_or_default();
                Err(ApiError::NotFound(body))
            }
            StatusCode::UNPROCESSABLE_ENTITY => {
                let body = resp.text().await.unwrap_or_default();
                Err(ApiError::SignalRejected(body))
            }
            _ => {
                let body = resp.text().await.unwrap_or_default();
                Err(ApiError::UnexpectedStatus(status.as_u16(), body))
            }
        }
    }

    async fn handle_text_response(resp: reqwest::Response) -> Result<String, ApiError> {
        let status = resp.status();
        match status {
            StatusCode::OK => Ok(resp.text().await?),
            StatusCode::UNAUTHORIZED => Err(ApiError::Unauthorized),
            StatusCode::NOT_FOUND => {
                let body = resp.text().await.unwrap_or_default();
                Err(ApiError::NotFound(body))
            }
            _ => {
                let body = resp.text().await.unwrap_or_default();
                Err(ApiError::UnexpectedStatus(status.as_u16(), body))
            }
        }
    }
}
