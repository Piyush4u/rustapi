//! rustapi-py: PyO3 bindings exposing the Rust engine to Python.
//!
//! The module is exposed as `_rustapi` (the leading underscore signals "internal
//! native module"). The pure-Python `rustapi` package wraps this and adds the
//! ergonomic decorator API.
//!
//! Design rule (from the blueprint): a request crosses the Python boundary
//! exactly once — to invoke the user's handler. Everything else (HTTP parsing,
//! routing, header munging, byte serialization) happens in `rustapi-engine`.

use pyo3::exceptions::{PyIOError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use rustapi_engine::{
    Handler, HandlerFuture, Request as EngineRequest, Response as EngineResponse, Router, Server,
    ServerConfig,
};
use std::collections::HashMap;
use std::sync::Arc;

use bytes::Bytes;
use http::Method;

// ===========================================================================
// Python-visible Request
// ===========================================================================

/// A Python-visible HTTP request, constructed in one shot from the Rust
/// `EngineRequest`. Field accessors go through PyO3 without per-field FFI.
#[pyclass(name = "Request", subclass)]
pub struct PyRequest {
    method: String,
    path: String,
    query_string: String,
    query_params: HashMap<String, String>,
    path_params: HashMap<String, String>,
    headers: HashMap<String, String>,
    body: Vec<u8>,
    remote_addr: Option<String>,
}

#[pymethods]
impl PyRequest {
    #[getter]
    fn method(&self) -> &str {
        &self.method
    }

    #[getter]
    fn path(&self) -> &str {
        &self.path
    }

    #[getter]
    fn query_string(&self) -> &str {
        &self.query_string
    }

    #[getter]
    fn query_params(&self) -> &HashMap<String, String> {
        &self.query_params
    }

    #[getter]
    fn path_params(&self) -> &HashMap<String, String> {
        &self.path_params
    }

    #[getter]
    fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }

    #[getter]
    fn body(&self) -> &[u8] {
        &self.body
    }

    #[getter]
    fn remote_addr(&self) -> Option<&str> {
        self.remote_addr.as_deref()
    }

    /// Decode the body as UTF-8 text.
    fn text(&self) -> PyResult<String> {
        String::from_utf8(self.body.clone())
            .map_err(|e| PyValueError::new_err(format!("body is not valid UTF-8: {e}")))
    }

    /// Parse the body as JSON using Python's `json.loads` (the roadmap moves
    /// this to a Rust `simd-json` path for an additional speedup).
    fn json(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let json_module = py.import("json")?;
        let loads = json_module.getattr("loads")?;
        let bytes = PyBytes::new(py, &self.body);
        Ok(loads.call1((bytes,))?.into())
    }
}

impl PyRequest {
    /// Build a Python `Request` from the Rust engine request. Single allocation.
    fn from_engine(req: EngineRequest) -> Self {
        let headers: HashMap<String, String> = req
            .headers
            .iter()
            .filter_map(|(k, v)| {
                v.to_str()
                    .ok()
                    .map(|s| (k.as_str().to_string(), s.to_string()))
            })
            .collect();
        PyRequest {
            method: req.method,
            path: req.path,
            query_string: req.query_string,
            query_params: req.query_params,
            path_params: req.path_params,
            headers,
            body: req.body.to_vec(),
            remote_addr: req.remote_addr.map(|a| a.to_string()),
        }
    }
}

// ===========================================================================
// Python-visible Response
// ===========================================================================

/// A Python-visible HTTP response. Users construct this when they need
/// custom status codes, headers, or content types. For the common case
/// (return a dict, get JSON), no `Response` is needed — the handler can
/// just `return {"foo": "bar"}`.
#[pyclass(name = "Response", subclass)]
pub struct PyResponse {
    status_code: u16,
    headers: HashMap<String, String>,
    body: Vec<u8>,
    content_type: String,
}

#[pymethods]
impl PyResponse {
    #[new]
    #[pyo3(signature = (body, status_code=200, content_type="application/json", headers=None))]
    fn new(
        body: Vec<u8>,
        status_code: u16,
        content_type: &str,
        headers: Option<HashMap<String, String>>,
    ) -> Self {
        PyResponse {
            status_code,
            headers: headers.unwrap_or_default(),
            body,
            content_type: content_type.to_string(),
        }
    }

    /// Convenience: build a JSON response from any JSON-serializable Python object.
    #[staticmethod]
    #[pyo3(signature = (data, status_code=200, headers=None))]
    fn json(
        py: Python<'_>,
        data: &Bound<'_, PyAny>,
        status_code: u16,
        headers: Option<HashMap<String, String>>,
    ) -> PyResult<Self> {
        let json_module = py.import("json")?;
        let dumps = json_module.getattr("dumps")?;
        let json_str: String = dumps.call1((data,))?.extract()?;
        Ok(PyResponse {
            status_code,
            headers: headers.unwrap_or_default(),
            body: json_str.into_bytes(),
            content_type: "application/json; charset=utf-8".to_string(),
        })
    }

    /// Convenience: build a text response.
    #[staticmethod]
    #[pyo3(signature = (text, status_code=200, headers=None))]
    fn text(text: &str, status_code: u16, headers: Option<HashMap<String, String>>) -> Self {
        PyResponse {
            status_code,
            headers: headers.unwrap_or_default(),
            body: text.as_bytes().to_vec(),
            content_type: "text/plain; charset=utf-8".to_string(),
        }
    }

    #[getter]
    fn status_code(&self) -> u16 {
        self.status_code
    }

    #[getter]
    fn body(&self) -> &[u8] {
        &self.body
    }

    #[getter]
    fn content_type(&self) -> &str {
        &self.content_type
    }

    #[getter]
    fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }
}

// ===========================================================================
// PyHandler — wraps a Python callable as a Rust Handler
// ===========================================================================

/// Adapter that lets a Python callable satisfy the Rust `Handler` trait.
///
/// This is where the "single FFI hop per request" thesis meets reality:
/// each request triggers exactly one `handle()` call, which acquires the
/// GIL, invokes the Python function (sync or async), and converts the
/// result back to a Rust `Response`.
pub struct PyHandler {
    callback: Py<PyAny>,
}

impl PyHandler {
    pub fn new(callback: Py<PyAny>) -> Self {
        PyHandler { callback }
    }
}

impl Handler for PyHandler {
    fn handle(&self, req: EngineRequest) -> HandlerFuture {
        // Clone the callback Py<PyAny> under the GIL (refcount bump). This is
        // the only GIL acquisition that happens before the actual handler
        // call; subsequent steps acquire the GIL only when crossing back into
        // Python.
        let cb = Python::attach(|py| self.callback.clone_ref(py));
        Box::pin(async move {
            // ---- THE single FFI hop ----------------------------------
            // Step 1: acquire GIL, build PyRequest, call the Python handler.
            let awaitable_or_value: Py<PyAny> = Python::attach(|py| {
                let py_req = Py::new(py, PyRequest::from_engine(req))?;
                cb.call1(py, (py_req,))
            })
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.to_string().into() })?;

            // Step 2: if it's awaitable, run it to completion. We use
            // `asyncio.run(coro)` on a tokio blocking-pool thread so we don't
            // stall the reactor. This creates a fresh event loop per request —
            // correct but not optimal; the roadmap moves to a per-thread
            // persistent loop or a single shared loop on a dedicated thread.
            let final_value: Py<PyAny> = {
                let is_awaitable: bool = Python::attach(|py: Python<'_>| -> PyResult<bool> {
                    let inspect = py.import("inspect")?;
                    inspect
                        .getattr("isawaitable")?
                        .call1((awaitable_or_value.bind(py),))?
                        .extract()
                })
                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                    e.to_string().into()
                })?;

                if is_awaitable {
                    // Move the awaitable into the blocking thread. `asyncio.run`
                    // creates a fresh loop, runs the coroutine to completion,
                    // and closes the loop. The GIL is held by this thread for
                    // the duration; tokio's reactor stays free because we're
                    // on a blocking-pool thread, not an async worker.
                    let join = tokio::task::spawn_blocking(move || {
                        Python::attach(|py| -> PyResult<Py<PyAny>> {
                            let asyncio = py.import("asyncio")?;
                            let result = asyncio.call_method1("run", (awaitable_or_value,))?;
                            Ok(result.into())
                        })
                    })
                    .await
                    .map_err(
                        |e| -> Box<dyn std::error::Error + Send + Sync> { e.to_string().into() },
                    )?;
                    join.map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                        e.to_string().into()
                    })?
                } else {
                    awaitable_or_value
                }
            };

            // Step 3: convert the PyObject to a Rust Response (one more GIL hop).
            Python::attach(|py| pyobj_to_response(py, &final_value))
                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.to_string().into() })
        })
    }
}

/// Convert a Python return value into a Rust `EngineResponse`.
///
/// Supported:
/// - `Response` instance (constructed via `Response(...)` or `Response.json(...)`)
/// - `dict` -> JSON-serialized via Python's `json.dumps`
/// - `str` -> text/plain
/// - `bytes` -> application/octet-stream
/// - Anything else -> `json.dumps` is attempted (covers dataclasses, Pydantic
///   models that have a custom encoder, etc.); if that fails, error.
fn pyobj_to_response(py: Python<'_>, obj: &Py<PyAny>) -> PyResult<EngineResponse> {
    let bound = obj.bind(py);

    // 1. Response instance?
    if let Ok(r) = bound.extract::<Py<PyResponse>>() {
        let borrowed = r.borrow(py);
        let status = http::StatusCode::from_u16(borrowed.status_code)
            .unwrap_or(http::StatusCode::INTERNAL_SERVER_ERROR);
        let mut resp = EngineResponse::new(
            status,
            Bytes::from(borrowed.body.clone()),
            &borrowed.content_type,
        );
        for (k, v) in borrowed.headers.iter() {
            resp = resp.with_header(k, v);
        }
        return Ok(resp);
    }

    // 2. str?
    if let Ok(s) = bound.extract::<String>() {
        return Ok(EngineResponse::text(s));
    }

    // 3. bytes?
    if let Ok(b) = bound.extract::<Vec<u8>>() {
        return Ok(EngineResponse::ok(
            Bytes::from(b),
            "application/octet-stream",
        ));
    }

    // 4. Default: JSON-serialize.
    let json_module = py.import("json")?;
    let dumps = json_module.getattr("dumps")?;
    let json_str: String = dumps.call1((bound,))?.extract()?;
    Ok(EngineResponse::json(Bytes::from(json_str.into_bytes())))
}

// ===========================================================================
// RustApi — the main Python-facing app class
// ===========================================================================

/// The main app class. Users construct `RustApi()` and register routes.
///
/// The decorator ergonomics (`.get(path)`, `.post(path)`, etc.) are added
/// by the pure-Python `rustapi` package which subclasses this. The native
/// side exposes `_register` and `run`.
#[pyclass(name = "_RustApi", subclass)]
pub struct PyApp {
    router: Router,
    config: ServerConfig,
}

#[pymethods]
impl PyApp {
    #[new]
    #[pyo3(signature = (server_name="rustapi/0.1".to_string()))]
    fn new(server_name: String) -> Self {
        PyApp {
            router: Router::new(),
            config: ServerConfig {
                server_name,
                ..Default::default()
            },
        }
    }

    /// Register a handler. Called by the Python decorator wrappers.
    /// Returns the original function so it can be used as a decorator.
    fn _register(
        &mut self,
        py: Python<'_>,
        method: &str,
        path: &str,
        handler: Py<PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let m = Method::from_bytes(method.as_bytes())
            .map_err(|e| PyValueError::new_err(format!("bad method {method:?}: {e}")))?;
        let py_handler: Arc<dyn Handler> = Arc::new(PyHandler::new(handler.clone_ref(py)));
        self.router
            .add(m, path, py_handler)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        Ok(handler)
    }

    /// Get the number of registered routes.
    fn route_count(&self) -> usize {
        self.router.len()
    }

    /// Run the server. Blocks the calling thread.
    #[pyo3(signature = (host=None, port=None))]
    fn run(&mut self, py: Python<'_>, host: Option<&str>, port: Option<u16>) -> PyResult<()> {
        let mut config = self.config.clone();
        if let Some(h) = host {
            config.host = h.to_string();
        }
        if let Some(p) = port {
            config.port = p;
        }
        let router = std::mem::take(&mut self.router);
        let server = Server::new(router, config);
        pyo3_async_runtimes::tokio::run(py, async move {
            server
                .serve()
                .await
                .map_err(|e| PyIOError::new_err(e.to_string()))
        })
    }
}

// ===========================================================================
// Module registration
// ===========================================================================

/// The Python module entry point. Maturin discovers this via `#[pymodule]`.
#[pymodule]
fn _rustapi(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyRequest>()?;
    m.add_class::<PyResponse>()?;
    m.add_class::<PyApp>()?;

    m.setattr("__version__", env!("CARGO_PKG_VERSION"))?;
    m.setattr(
        "__doc__",
        "rustapi native module — Rust-core HTTP engine for Python.",
    )?;

    Ok(())
}
