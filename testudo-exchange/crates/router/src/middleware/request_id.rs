//! AUD-05 FR-3: X-Request-Id middleware.
//!
//! Extracts `X-Request-Id` from incoming request headers or generates a new UUID v4.
//! Inserts into request extensions for downstream extraction and adds to response headers.

// @anchor exchange:router:request_id
// @tags api

use actix_web::dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::http::header::{HeaderName, HeaderValue};
use actix_web::HttpMessage;
use futures_util::future::LocalBoxFuture;
use std::future::{ready, Ready};
use uuid::Uuid;

/// The request ID value, accessible via `req.extensions()` or `web::ReqData<RequestId>`.
#[derive(Debug, Clone)]
pub struct RequestId(pub String);

static REQUEST_ID_HEADER: &str = "x-request-id";

/// Middleware that assigns/extracts an X-Request-Id for every request.
pub struct RequestIdMiddleware;

impl<S, B> Transform<S, ServiceRequest> for RequestIdMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = actix_web::Error;
    type InitError = ();
    type Transform = RequestIdMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RequestIdMiddlewareService { service }))
    }
}

pub struct RequestIdMiddlewareService<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for RequestIdMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = actix_web::Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let request_id = req
            .headers()
            .get(REQUEST_ID_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(String::from)
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        req.extensions_mut()
            .insert(RequestId(request_id.clone()));

        let fut = self.service.call(req);
        Box::pin(async move {
            let mut resp = fut.await?;
            resp.headers_mut().insert(
                HeaderName::from_static(REQUEST_ID_HEADER),
                HeaderValue::from_str(&request_id).unwrap_or_else(|_| HeaderValue::from_static("")),
            );
            Ok(resp)
        })
    }
}
