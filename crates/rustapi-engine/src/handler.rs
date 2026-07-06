//! Handler abstraction.
//!
//! A [`Handler`] is what the router ultimately dispatches to. In the full
//! design, this is a trait object so we can have pure-Rust handlers (e.g.
//! static file serving, health checks) that never touch Python. The default
//! implementation in `rustapi-py` wraps a Python callable.

use crate::{request::Request, response::Response};
use std::future::Future;
use std::pin::Pin;

/// Boxed future returned by a handler. Lives for the duration of one request.
pub type HandlerFuture = Pin<Box<dyn Future<Output = HandlerResult> + Send>>;

/// Result of running a handler: a [`Response`], or a boxed error.
pub type HandlerResult = Result<Response, Box<dyn std::error::Error + Send + Sync>>;

/// Trait implemented by anything that can serve a request.
///
/// Pure-Rust handlers (static assets, `/healthz`, metrics) implement this
/// directly; Python handlers go through the PyO3 adapter in `rustapi-py`.
pub trait Handler: Send + Sync {
    fn handle(&self, req: Request) -> HandlerFuture;
}

/// Blanket implementation so any `async fn(Request) -> HandlerResult` can be
/// turned into a [`Handler`] without ceremony.
impl<F, Fut> Handler for F
where
    F: Fn(Request) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = HandlerResult> + Send + 'static,
{
    fn handle(&self, req: Request) -> HandlerFuture {
        let fut = (self)(req);
        Box::pin(fut)
    }
}
