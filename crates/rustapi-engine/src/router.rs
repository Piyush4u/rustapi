//! Radix-tree router built on `matchit`.
//!
//! Routes are keyed by `(HTTP method, path pattern)`. Static routes are
//! effectively O(1); parametrized routes are O(log n). Path params are
//! extracted by `matchit` and handed to the handler.
//!
//! Design note: we keep one `matchit::Router<Arc<dyn Handler>>` per HTTP
//! method. This avoids needing a composite key and lets us return a clean
//! 405 (method not allowed) when the path matches but the method doesn't.

use crate::handler::Handler;
use http::Method;
use matchit::Router as MatchitRouter;
use std::collections::HashMap;
use std::sync::Arc;

/// Errors raised by route registration or matching.
#[derive(Debug)]
pub enum RouterError {
    /// Invalid route pattern (e.g. malformed `{id`).
    InvalidPattern(String),
    /// Duplicate registration of the same (method, pattern).
    Conflict(String),
    /// Path matched but no route exists for this HTTP method.
    MethodNotAllowed,
    /// No route matched the path at all.
    NotFound,
}

impl std::fmt::Display for RouterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RouterError::InvalidPattern(s) => write!(f, "invalid route pattern: {s}"),
            RouterError::Conflict(s) => write!(f, "route conflict: {s}"),
            RouterError::MethodNotAllowed => write!(f, "method not allowed"),
            RouterError::NotFound => write!(f, "not found"),
        }
    }
}

impl std::error::Error for RouterError {}

#[derive(Default)]
struct MethodRoutes {
    inner: MatchitRouter<Arc<dyn Handler>>,
    /// Set of all patterns registered for this method, used for conflict detection.
    patterns: std::collections::HashSet<String>,
}

/// Per-method router. Cheap to clone via `Arc` if needed; usually one per server.
#[derive(Default)]
pub struct Router {
    routes: HashMap<Method, MethodRoutes>,
    /// Patterns registered across all methods — used to detect 405 vs 404.
    all_patterns: HashMap<String, Vec<Method>>,
}

impl Router {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a handler for `(method, pattern)`.
    pub fn add(
        &mut self,
        method: Method,
        pattern: &str,
        handler: Arc<dyn Handler>,
    ) -> Result<(), RouterError> {
        // 405-detection bookkeeping
        let methods_for_pattern = self.all_patterns.entry(pattern.to_string()).or_default();
        if !methods_for_pattern.contains(&method) {
            methods_for_pattern.push(method.clone());
        }

        let entry = self.routes.entry(method.clone()).or_default();
        if !entry.patterns.insert(pattern.to_string()) {
            return Err(RouterError::Conflict(format!("{method} {pattern}")));
        }
        entry
            .inner
            .insert(pattern, handler)
            .map_err(|e| RouterError::InvalidPattern(format!("{pattern}: {e}")))?;
        Ok(())
    }

    /// Match a request. Returns the handler plus extracted path params.
    ///
    /// Errors:
    /// - [`RouterError::NotFound`] — no route matched the path (any method).
    /// - [`RouterError::MethodNotAllowed`] — path matched, but method didn't.
    pub fn match_route(&self, method: &Method, path: &str) -> MatchResult {
        match self.routes.get(method) {
            Some(routes) => match routes.inner.at(path) {
                Ok(matched) => {
                    let params: HashMap<String, String> = matched
                        .params
                        .iter()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect();
                    Ok((matched.value.clone(), params))
                }
                Err(_) => {
                    if self
                        .all_patterns
                        .keys()
                        .any(|p| matches_any_method(p, path))
                    {
                        Err(RouterError::MethodNotAllowed)
                    } else {
                        Err(RouterError::NotFound)
                    }
                }
            },
            None => {
                if self
                    .all_patterns
                    .keys()
                    .any(|p| matches_any_method(p, path))
                {
                    Err(RouterError::MethodNotAllowed)
                } else {
                    Err(RouterError::NotFound)
                }
            }
        }
    }

    /// Number of registered routes across all methods.
    pub fn len(&self) -> usize {
        self.routes.values().map(|r| r.patterns.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Result of [`Router::match_route`]: the matched handler plus extracted
/// path params, or a [`RouterError`] explaining why no handler matched.
pub type MatchResult = Result<(Arc<dyn Handler>, HashMap<String, String>), RouterError>;

/// Best-effort structural path-match check used to distinguish 404 from 405.
/// Splits by `/`; treats `{...}` segments as wildcards. Correct for the
/// pattern syntax `matchit` accepts.
fn matches_any_method(pattern: &str, path: &str) -> bool {
    let pat_segs: Vec<&str> = pattern.split('/').collect();
    let path_segs: Vec<&str> = path.split('/').collect();
    if pat_segs.len() != path_segs.len() {
        return false;
    }
    pat_segs
        .iter()
        .zip(path_segs.iter())
        .all(|(p, s)| (p.starts_with('{') && p.ends_with('}')) || *p == *s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{handler::HandlerFuture, request::Request, response::Response};
    use std::sync::Arc;

    fn dummy_handler() -> Arc<dyn Handler> {
        Arc::new(|_req: Request| Box::pin(async { Ok(Response::text("ok")) }) as HandlerFuture)
    }

    #[test]
    fn adds_and_matches_static_route() {
        let mut r = Router::new();
        r.add(Method::GET, "/health", dummy_handler()).unwrap();
        let (_h, params) = r.match_route(&Method::GET, "/health").unwrap();
        assert!(params.is_empty());
        assert_eq!(r.len(), 1);
    }

    #[test]
    fn extracts_path_params() {
        let mut r = Router::new();
        r.add(Method::GET, "/users/{id}", dummy_handler()).unwrap();
        let (_, params) = r.match_route(&Method::GET, "/users/42").unwrap();
        assert_eq!(params.get("id").unwrap(), "42");
    }

    #[test]
    fn returns_405_when_method_wrong() {
        let mut r = Router::new();
        r.add(Method::GET, "/users/{id}", dummy_handler()).unwrap();
        match r.match_route(&Method::DELETE, "/users/42") {
            Err(RouterError::MethodNotAllowed) => (),
            other => panic!("expected 405, got {}", other.map(|_| "Ok").unwrap_err()),
        }
    }

    #[test]
    fn returns_404_when_no_match() {
        let mut r = Router::new();
        r.add(Method::GET, "/users/{id}", dummy_handler()).unwrap();
        match r.match_route(&Method::GET, "/nope") {
            Err(RouterError::NotFound) => (),
            other => panic!("expected 404, got {}", other.map(|_| "Ok").unwrap_err()),
        }
    }

    #[test]
    fn detects_duplicate_registration() {
        let mut r = Router::new();
        r.add(Method::GET, "/x", dummy_handler()).unwrap();
        match r.add(Method::GET, "/x", dummy_handler()) {
            Err(RouterError::Conflict(_)) => (),
            other => panic!("expected Conflict, got {:?}", other.map(|_| "Ok").err()),
        }
    }
}
