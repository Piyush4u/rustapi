//! Response produced by a handler.
//!
//! Bytes-level: the Python bridge is responsible for serializing Python
//! objects (dicts, dataclasses, Pydantic models) into the final `body` bytes
//! before constructing a [`Response`]. In v0.1 we delegate JSON serialization
//! to Python's `json` module (already fast, C-implemented); the roadmap moves
//! this to `simd-json` for an additional 3-10x on large payloads.

use bytes::Bytes;
use http::{HeaderMap, StatusCode};

#[derive(Debug, Clone)]
pub struct Response {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
}

impl Response {
    pub fn new(status: StatusCode, body: impl Into<Bytes>, content_type: &str) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(
            http::header::CONTENT_TYPE,
            http::HeaderValue::from_str(content_type).unwrap(),
        );
        Response {
            status,
            headers,
            body: body.into(),
        }
    }

    pub fn ok(body: impl Into<Bytes>, content_type: &str) -> Self {
        Self::new(StatusCode::OK, body, content_type)
    }

    /// 200 OK with `application/json` content type.
    pub fn json(body: impl Into<Bytes>) -> Self {
        Self::ok(body, "application/json; charset=utf-8")
    }

    /// Convenience: 200 OK with `text/plain; charset=utf-8`.
    pub fn text(body: impl Into<Bytes>) -> Self {
        Self::ok(body, "text/plain; charset=utf-8")
    }

    pub fn not_found() -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            Bytes::from_static(b"{\"error\":\"not found\"}"),
            "application/json; charset=utf-8",
        )
    }

    pub fn method_not_allowed() -> Self {
        Self::new(
            StatusCode::METHOD_NOT_ALLOWED,
            Bytes::from_static(b"{\"error\":\"method not allowed\"}"),
            "application/json; charset=utf-8",
        )
    }

    pub fn internal_error(msg: &str) -> Self {
        let body = serde_json::json!({"error": "internal server error", "detail": msg})
            .to_string()
            .into_bytes();
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            Bytes::from(body),
            "application/json; charset=utf-8",
        )
    }

    /// Append a header. Silently drops malformed values (logging is on the roadmap).
    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        if let (Ok(name), Ok(value)) = (
            http::HeaderName::from_bytes(name.as_bytes()),
            http::HeaderValue::from_str(value),
        ) {
            self.headers.insert(name, value);
        }
        self
    }
}
