# rustapi Architecture

This document explains how `rustapi` is structured and why. For the what
(features), see `README.md`. For the when (timeline), see `ROADMAP.md`.

## The single rule

**A request crosses the Python boundary exactly once — to invoke the user's
handler.**

Everything else — HTTP parsing, routing, header munging, byte serialization,
response framing — happens in the Rust process. This is the defining
architectural decision and the source of every performance claim.

## Layered view

```
┌─────────────────────────────────────────────────────────────────┐
│                        Clients                                   │
│        (browser, mobile, IoT, curl, urllib)                      │
└───────────────────────────┬─────────────────────────────────────┘
                            │ HTTP/1.1 (v0.1)
                            │ HTTP/2, HTTP/3 (roadmap)
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│  rustapi-engine  (pure Rust, no Python)                          │
│                                                                  │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐        │
│  │  Network     │ -> │  Router      │ -> │  Dispatch    │        │
│  │  hyper +     │    │  matchit     │    │  handler     │        │
│  │  tokio       │    │  radix tree  │    │  invocation  │        │
│  └──────────────┘    └──────────────┘    └──────┬───────┘        │
│                                                   │               │
│                              ┌────────────────────┘               │
│                              ▼                                    │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │  Handler trait  (dyn Handler)                             │   │
│  │  - pure-Rust handlers (static, health, metrics)           │   │
│  │  - PyHandler (wraps a Python callable)                    │   │
│  └────────────────────────────┬─────────────────────────────┘   │
└───────────────────────────────┼──────────────────────────────────┘
                                │  PyO3 FFI (single hop)
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│  rustapi-py  (PyO3 bindings, cdylib `_rustapi.so`)               │
│                                                                  │
│  - PyRequest: holds Rust Request, exposes fields to Python       │
│  - PyResponse: built by user, converted to Rust Response         │
│  - PyApp: holds Router, exposes _register / run                  │
│  - PyHandler: impl Handler, calls Python callable                │
│                                                                  │
│  Async strategy (v0.1):                                          │
│    Python coroutine -> asyncio.run(coro) on a tokio              │
│    blocking-pool thread. The reactor stays free; the GIL         │
│    is held by the blocking thread for the duration.              │
│                                                                  │
│  Async strategy (v0.2, planned):                                 │
│    Per-thread persistent event loop. One loop per tokio          │
│    worker thread, reused across requests. Eliminates the         │
│    ~0.5ms asyncio.run overhead.                                  │
└───────────────────────────────┬──────────────────────────────────┘
                                │  Python import
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│  rustapi  (pure Python, the user-facing package)                 │
│                                                                  │
│  - RustApi(_RustApi): adds decorator ergonomics                  │
│    @app.get @app.post @app.put @app.delete ...                   │
│  - Request, Response: re-exported from native                    │
│  - JsonResponse, TextResponse: factory functions                 │
└─────────────────────────────────────────────────────────────────┘
```

## Crate layout

```
rustapi/
├── Cargo.toml                 # workspace root
├── pyproject.toml             # maturin build backend
├── crates/
│   ├── rustapi-engine/        # pure-Rust HTTP engine (no PyO3)
│   │   └── src/
│   │       ├── lib.rs         # public exports
│   │       ├── handler.rs     # Handler trait, HandlerFuture
│   │       ├── request.rs     # EngineRequest + query parsing
│   │       ├── response.rs    # EngineResponse + helpers
│   │       ├── router.rs      # matchit-based router, 404/405 logic
│   │       └── server.rs      # tokio runtime, hyper accept loop
│   └── rustapi-py/            # PyO3 bindings (cdylib _rustapi)
│       └── src/
│           └── lib.rs         # PyRequest, PyResponse, PyApp, PyHandler
├── python/
│   └── rustapi/               # pure-Python package
│       ├── __init__.py        # RustApi class, decorators, factories
│       └── py.typed           # PEP 561 marker
├── examples/
│   └── demo.py                # runnable demo app
├── tests/
│   ├── test_e2e.py            # 23 end-to-end assertions
│   └── test_perf.py           # smoke-test RPS
└── docs/
    ├── ARCHITECTURE.md        # this file
    └── ROADMAP.md             # versioned feature plan
```

## Request lifecycle (v0.1)

1. **Accept** — tokio `TcpListener::accept()` returns a stream.
2. **Hyper serve_connection** — `hyper-util` auto-detects HTTP/1.1 (HTTP/2
   on roadmap) and spawns a `service_fn` per request.
3. **Parse** — hyper parses the request line and headers into
   `http::Request<Incoming>`. The body is collected into `bytes::Bytes`
   with a configurable cap (10 MiB default).
4. **Route** — `Router::match_route(method, path)` returns the matched
   `Arc<dyn Handler>` plus extracted path params, or a `NotFound` /
   `MethodNotAllowed` error.
5. **Dispatch** — `handler.handle(req).await` is called. For Python routes,
   this is `PyHandler::handle`.
6. **PyO3 hop #1** — `Python::attach` acquires the GIL, builds `PyRequest`
   from the `EngineRequest` (one allocation, no per-field FFI), and calls
   the user's Python callable.
7. **Async await** (if handler is `async def`) — the coroutine is moved
   to a `tokio::task::spawn_blocking` thread, where `asyncio.run(coro)`
   runs it to completion. The tokio reactor stays free.
8. **PyO3 hop #2** — `pyobj_to_response` converts the return value
   (`dict` / `str` / `bytes` / `Response`) into an `EngineResponse`.
   JSON serialization uses Python's `json.dumps` (roadmap: `simd-json`).
9. **Render** — `EngineResponse` is converted to `http::Response<Full<Bytes>>`
   with the `Server` header added.
10. **Send** — hyper writes the response bytes to the socket.

Steps 1–5, 9–10 are pure Rust. Steps 6–8 cross into Python. The single-hop
thesis is approximately honored: most requests cross the boundary exactly
twice (call + result conversion), not counting the GIL acquisitions needed
for refcount management. Optimizations to fold steps 6–8 into one hop are
on the v0.2 roadmap.

## Why asyncio.run per request (and why we'll change it)

**v0.1** uses `asyncio.run(coro)` because:

- It works correctly from any thread, including tokio worker threads.
- It requires no global state (no pre-existing event loop).
- It's the simplest path to a correct async story.

The cost is ~0.5ms per request for loop creation/teardown, plus the GIL is
held by the blocking thread for the duration of the coroutine.

**v0.2** will move to a per-thread persistent event loop:

- Each tokio worker thread gets its own `asyncio.EventLoop`, created lazily
  on first use and cached in a thread-local.
- Coroutines are scheduled via `loop.run_until_complete(coro)`, which reuses
  the existing loop.
- Estimated speedup: 2-3x on async-heavy workloads, more on tiny handlers
  where loop creation dominates.

The longer-term v0.5+ design uses a single dedicated Python event loop
thread that all workers schedule onto, with `asyncio.run_coroutine_threadsafe`.
This fully decouples the GIL from the tokio reactor.

## GIL semantics

The GIL is acquired:

1. Once at handler registration (to clone the `Py<PyAny>` callback ref).
2. Once per request to call the Python handler (step 6).
3. Once per request to convert the result (step 8).
4. If async: once on the blocking thread to invoke `asyncio.run`.

For sync handlers, the GIL is held continuously from step 6 through step 8
(a single `Python::attach` would suffice — v0.2 will fuse these).

For async handlers, the GIL is released between the initial call (which
returns a coroutine) and the blocking-thread execution. During the
`asyncio.run` call, the GIL is held by the blocking thread; other tokio
workers can still process pure-Rust requests (static files, health checks)
without contending for it.

## Extension points

The `Handler` trait is the primary extension point:

```rust
pub trait Handler: Send + Sync {
    fn handle(&self, req: Request) -> Pin<Box<dyn Future<Output = HandlerResult> + Send>>;
}
```

A pure-Rust handler (no Python) implements this directly and never touches
the GIL. This is how v0.2 will implement:

- `/healthz`, `/readyz`, `/livez` (no Python)
- Static asset serving (with `tokio-uring` on Linux)
- `/metrics` (Prometheus exporter)
- OpenAPI schema endpoint

Python handlers go through `PyHandler`, which is just one implementation
of `Handler`. The router doesn't know or care which kind it's dispatching to.

## What's deliberately NOT in v0.1

These are roadmap items, not oversights:

- **No middleware** — handlers run with no pre/post hooks. The `tower::Service`
  stack is the v0.2 fix.
- **No TLS** — terminate at a reverse proxy. `rustls` integration is v0.3.
- **No ORM** — bring your own. The Rust async ORM is v0.4.
- **No OpenAPI generation** — v0.4.
- **No WebSocket / SSE** — v0.5.
- **No `--workers N`** — single process. `SO_REUSEPORT` multi-worker is v0.2.
- **No hot reload** — restart the process. `notify`-based hot reload is v0.2.

Each of these is a deliberate scope cut to ship a correct foundation first.
