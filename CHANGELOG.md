# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Planned

- Per-thread persistent event loop (eliminates the per-request `asyncio.run`
  overhead — biggest perf win on the v0.2 list)
- `simd-json` for response serialization (3–10x over Python `json`)
- Tower middleware stack: CORS, rate-limit, compression, request-ID
- HTTP/2 via `hyper` + ALPN negotiation
- `--workers N` multiprocess mode with `SO_REUSEPORT`
- CLI: `rustapi new / serve / bench`
- Hot reload via `notify`

See [`docs/ROADMAP.md`](docs/ROADMAP.md) for the full plan.

## [0.1.0] — 2026-07-06

### Added — Initial release

This is the foundational MVP. It establishes the architecture thesis — a
Rust-native HTTP engine exposed through a Python decorator API with a single
FFI hop per request — and proves it end-to-end.

#### Rust engine (`rustapi-engine`)

- HTTP/1.1 server via `hyper` 1.x + `hyper-util`
- `tokio` multi-threaded runtime
- `matchit`-based radix-tree router with path parameters (`/users/{id}`)
- Per-method routing with correct 404 vs 405 distinction
- Duplicate-route conflict detection
- Bounded body collection (10 MiB default, configurable)
- `Handler` trait with blanket impl for `async fn(Request) -> Result<Response>`
- `Request` struct: method, path, query_string, query_params, headers,
  path_params, body, remote_addr
- `Response` struct: status, headers, body, content_type, with helpers
  (`ok`, `json`, `text`, `not_found`, `method_not_allowed`, `internal_error`)
- Query string parsing with percent-decoding and `+`→space conversion
- Multi-valued query params merged with `,`
- `Server` header (configurable)
- Graceful error responses for 404, 405, 500

#### Python bindings (`rustapi-py`)

- PyO3 0.29 + `pyo3-async-runtimes` 0.29
- `RustApi` class with `@app.get / @app.post / @app.put / @app.delete /
  @app.patch / @app.head / @app.options` decorators
- `Request` Python class: `method`, `path`, `query_params`, `path_params`,
  `headers`, `body`, `remote_addr`, `.json()`, `.text()`
- `Response` Python class with custom status / headers / content-type
- `JsonResponse(data)` and `TextResponse(text)` factory functions
- Sync **and** async Python handlers
- Async bridge via `asyncio.run(coro)` on a tokio blocking-pool thread
- `maturin` build with `abi3-py38` (one wheel for CPython 3.8–3.13+)

#### Python package (`rustapi`)

- Pure-Python thin wrapper, `py.typed` marker (PEP 561)
- Decorator API ergonomic enough for one-screen examples

#### Tests

- 9 Rust unit tests (request parsing, router matching, 404/405, conflicts)
- 23 end-to-end integration assertions covering the full request lifecycle
- Performance smoke test (sync handlers ~4k RPS, async ~2k RPS with aiohttp)

#### Documentation

- `README.md` with quickstart and architecture overview
- `docs/ARCHITECTURE.md` explaining the layered design, request lifecycle,
  GIL semantics, and the v0.2 async strategy
- `docs/ROADMAP.md` with versioned feature plan from v0.2 through v1.0

#### Packaging

- Dual MIT/Apache-2.0 license
- `pyproject.toml` with full PyPI metadata, classifiers, project URLs
- CycloneDX SBOM generated per build
- Wheel: `rustapi-0.1.0-cp38-abi3-manylinux_2_34_x86_64.whl` (~750 KB)

### Known limitations

These are deliberate scoping decisions for v0.1, not bugs:

1. One event loop per async request via `asyncio.run` (~0.5ms overhead).
2. JSON serialization via Python's `json` module (not `simd-json`).
3. HTTP/1.1 only — no HTTP/2 or HTTP/3.
4. No TLS — terminate at a reverse proxy.
5. No middleware pipeline.
6. No ORM.
7. Single-process — no `--workers N`.
8. No WebSocket / SSE.
9. No OpenAPI generation.
10. No CLI.

Each is tracked in `docs/ROADMAP.md` with a target version.

[Unreleased]: https://github.com/rustapi/rustapi/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/rustapi/rustapi/releases/tag/v0.1.0
