//! The HTTP server.
//!
//! Owns the tokio runtime, the listener, and the per-connection task. Each
//! accepted connection is parsed by hyper, matched against the [`Router`],
//! and the matched handler is invoked. Response bytes are written back by
//! hyper.
//!
//! This is where the "single FFI hop" thesis lives: every step before the
//! handler call and every step after it runs in pure Rust.

use crate::{request::Request, response::Response, router::Router};
use bytes::Bytes;
use http::{HeaderMap, Method, Request as HttpRequest, Response as HttpResponse};
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper_util::rt::{TokioExecutor, TokioIo};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{debug, error, info, warn};

/// Server configuration.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    /// Max request body size in bytes. Defaults to 10 MiB.
    pub max_body_size: usize,
    /// Server string emitted in the `Server` header. Empty disables the header.
    pub server_name: String,
    /// Whether to emit per-request `tracing` spans (info level).
    pub request_logging: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            host: "127.0.0.1".to_string(),
            port: 8080,
            max_body_size: 10 * 1024 * 1024,
            server_name: "rustapi/0.1".to_string(),
            request_logging: false,
        }
    }
}

/// A ready-to-run HTTP server.
///
/// Built by registering routes on a [`Router`], then calling [`Server::run`]
/// (blocking) or [`Server::serve`] (async, on a caller-provided runtime).
pub struct Server {
    router: Arc<Router>,
    config: ServerConfig,
}

impl Server {
    pub fn new(router: Router, config: ServerConfig) -> Self {
        Server {
            router: Arc::new(router),
            config,
        }
    }

    /// Run on a fresh current-thread tokio runtime. Blocks the calling thread
    /// until the server is shut down (Ctrl-C / SIGTERM).
    pub fn run(self) -> std::io::Result<()> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        rt.block_on(self.serve())?;
        Ok(())
    }

    /// Async entry point. Resolves when the listener fails irrecoverably.
    pub async fn serve(self) -> std::io::Result<()> {
        let addr: SocketAddr = format!("{}:{}", self.config.host, self.config.port)
            .parse()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
        let listener = TcpListener::bind(addr).await?;
        info!(%addr, "rustapi listening");

        let router = self.router.clone();
        let config = Arc::new(self.config.clone());

        loop {
            let (stream, remote) = match listener.accept().await {
                Ok(s) => s,
                Err(e) => {
                    warn!(error = %e, "accept failed");
                    continue;
                }
            };
            let io = TokioIo::new(stream);
            let router = router.clone();
            let config = config.clone();
            tokio::task::spawn(async move {
                if let Err(e) = hyper_util::server::conn::auto::Builder::new(TokioExecutor::new())
                    .serve_connection(
                        io,
                        hyper::service::service_fn(move |req| {
                            let router = router.clone();
                            let config = config.clone();
                            async move { dispatch(req, router, config, remote).await }
                        }),
                    )
                    .await
                {
                    error!(error = %e, "connection error");
                }
            });
        }
    }
}

async fn dispatch(
    req: HttpRequest<Incoming>,
    router: Arc<Router>,
    config: Arc<ServerConfig>,
    remote: SocketAddr,
) -> Result<HttpResponse<Full<Bytes>>, Infallible> {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let query_string = req.uri().query().unwrap_or("").to_string();
    let headers = req.headers().clone();

    if config.request_logging {
        info!(%method, %path, "request");
    }

    // Collect body, capped at max_body_size.
    let body = match collect_body(req.into_body(), config.max_body_size).await {
        Ok(b) => b,
        Err(e) => {
            warn!(error = %e, "body read failed");
            return Ok(render(
                Response::internal_error("body too large or unreadable"),
                &config,
            ));
        }
    };

    let query_params = Request::parse_query(&query_string);

    // Match route.
    let (handler, path_params) = match router.match_route(&method, &path) {
        Ok(m) => m,
        Err(crate::router::RouterError::NotFound) => {
            debug!(%path, "404");
            return Ok(render(Response::not_found(), &config));
        }
        Err(crate::router::RouterError::MethodNotAllowed) => {
            debug!(%method, %path, "405");
            return Ok(render(Response::method_not_allowed(), &config));
        }
        Err(e) => {
            warn!(error = %e, "router error");
            return Ok(render(Response::internal_error("router error"), &config));
        }
    };

    let req = Request {
        method: method.as_str().to_string(),
        path,
        query_string,
        query_params,
        headers,
        path_params,
        body,
        remote_addr: Some(remote),
    };

    // THE single FFI hop lives inside the handler (which is, for Python
    // routes, the PyO3 adapter). Everything else has been pure Rust so far.
    let resp = match handler.handle(req).await {
        Ok(r) => r,
        Err(e) => {
            error!(error = %e, "handler error");
            Response::internal_error(&e.to_string())
        }
    };

    Ok(render(resp, &config))
}

/// Convert a [`Response`] into a hyper-compatible `HttpResponse<Full<Bytes>>`.
fn render(resp: Response, config: &ServerConfig) -> HttpResponse<Full<Bytes>> {
    let mut builder = HttpResponse::builder().status(resp.status);
    for (name, value) in resp.headers.iter() {
        builder = builder.header(name, value);
    }
    if !config.server_name.is_empty() {
        builder = builder.header("server", &config.server_name);
    }
    builder.body(Full::new(resp.body)).expect("valid response")
}

async fn collect_body(
    body: Incoming,
    max_bytes: usize,
) -> Result<Bytes, Box<dyn std::error::Error + Send + Sync>> {
    let mut collected = Vec::new();
    let mut remaining = max_bytes;
    let mut body = body;
    while let Some(frame) = body.frame().await {
        let frame = frame?;
        if let Ok(data) = frame.into_data() {
            if data.len() > remaining {
                return Err(format!("request body exceeds max size of {} bytes", max_bytes).into());
            }
            remaining -= data.len();
            collected.extend_from_slice(&data);
        }
    }
    Ok(Bytes::from(collected))
}

/// Convenience for building a request struct in tests / pure-Rust embeds.
#[allow(dead_code)]
pub(crate) fn make_test_request(
    method: &str,
    path: &str,
    body: Bytes,
) -> (Method, String, String, HeaderMap, Bytes) {
    let (path, qs) = match path.split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None => (path.to_string(), String::new()),
    };
    (
        Method::from_bytes(method.as_bytes()).unwrap(),
        path,
        qs,
        HeaderMap::new(),
        body,
    )
}
