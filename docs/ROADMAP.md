# rustapi Roadmap

This document tracks what's implemented vs. what's planned. The v0.1 release
establishes the architecture; subsequent versions fill in the feature matrix
described in the original blueprint.

## Status legend

- ✅ **Implemented** — shipped in v0.1
- 🚧 **In progress** — being built right now
- 📋 **Planned** — designed, not yet started
- 🔬 **Experimental** — prototyped behind a feature flag

---

## v0.1 (current) — Foundation

The goal of v0.1 is to prove the architecture thesis: a Rust-native HTTP
engine exposed through a Python decorator API with a single FFI hop per
request.

### ✅ Core runtime

- HTTP/1.1 server via `hyper` 1.x + `hyper-util`
- `tokio` multi-threaded runtime
- Per-connection task with bounded body collection (10 MiB default)
- `Server` header, configurable host/port
- Graceful error responses (404, 405, 500)

### ✅ Routing

- `matchit`-based radix-tree router
- Path parameters: `/users/{id}`
- Per-method routing with proper 405 detection
- Duplicate-route conflict detection

### ✅ Python bindings (PyO3 0.29)

- `RustApi` class with `@app.get / @app.post / @app.put / @app.delete /
  @app.patch / @app.head / @app.options` decorators
- `Request` Python class with `method`, `path`, `query_params`, `path_params`,
  `headers`, `body`, `remote_addr`, `.json()`, `.text()`
- `Response` class with custom status / headers / content-type
- `JsonResponse(data)` and `TextResponse(text)` convenience factories
- Sync **and** async Python handlers
- `pyo3-async-runtimes` bridge via `asyncio.run` on a tokio blocking pool
- `maturin` build with `abi3-py38` (one wheel for CPython 3.8–3.13+)

### ✅ Serialization

- JSON via Python's `json` module (response side)
- `request.json()` parses body via `json.loads`

### ✅ Developer experience

- `pip install rustapi` (after `maturin build`)
- `py.typed` marker for type-checker support
- Demo app at `examples/demo.py`
- End-to-end test suite at `tests/test_e2e.py` (23 assertions, all passing)

---

## v0.2 — Performance & middleware

### 📋 Performance

- `simd-json` for request body parsing (3–10x over `serde_json`)
- Per-thread persistent event loop (replaces `asyncio.run` per request)
- `moka` LRU cache for hot responses
- `mimalloc` / `jemalloc` global allocator
- `bytes::Bytes` pooling to eliminate body allocations on the hot path
- TechEmpower-style benchmark harness (`rustapi bench`)

### 📋 Middleware pipeline

- `tower::Service` + `tower::Layer` trait pair
- Built-in Rust middleware:
  - CORS
  - request-ID
  - body-size limit
  - compression (gzip / brotli / zstd)
  - ETag
  - IP allow/deny
  - structured request logging
- Python middleware as opt-in escape hatch (one extra FFI hop)

### 📋 HTTP/2

- `hyper` HTTP/2 server (already supported by hyper, just needs wiring)
- ALPN negotiation via `rustls`

---

## v0.3 — Security & auth

### 📋 TLS

- `rustls` for TLS 1.2 / 1.3
- ACME / Let's Encrypt auto-provisioning
- OCSP stapling
- mTLS client-cert verification

### 📋 Auth primitives (all in Rust)

- JWT (RS256 / ES256 / EdDSA) via `jsonwebtoken`
- OAuth2 / OIDC via `openidconnect`, PKCE enforced
- CSRF: origin/referer check + signed-token for cookie auth
- Rate limiting: token-bucket via `tower::limit` + Redis distributed
- Security headers middleware: CSP, HSTS, X-Frame-Options,
  X-Content-Type-Options, Referrer-Policy, Permissions-Policy
- Secrets management: encrypted-at-rest env vault, Vault / AWS Secrets Manager
- `rustapi scan` subcommand: `cargo-audit` + `pip-audit` + Semgrep

---

## v0.4 — Database & ORM

### 📋 Async ORM

- `sqlx` (compile-time checked SQL) as the foundation
- Django-style query builder exposed via PyO3:
  `.objects.filter().exclude().prefetch().join()`
- Migrations CLI (`rustapi migrate`)
- Connection pooling via `sqlx::Pool` + `deadpool-postgres`
- Read/write split with database routers
- No lazy loading by default (avoids N+1)
- Pydantic/msgspec models **are** the database models — one class, no
  duplication
- Adapters: PostgreSQL, MySQL, SQLite (roadmap: ClickHouse, ScyllaDB)

---

## v0.5 — Real-time

### 📋 WebSocket

- `tokio-tungstenite` transport
- Built-in pub/sub hub with typed channels
- `app.websocket("/path")` decorator
- `WebSocket` Python object with `send` / `recv` bridged to Rust

### 📋 Server-Sent Events

- Backpressure-aware SSE broadcaster
- `app.sse("/path")` decorator

### 📋 HTTP/3

- `h3` + `quinn` for QUIC transport
- Feature-flagged; default off until h3 stabilizes in hyper

---

## v0.6 — Observability

### 📋 OpenTelemetry (native Rust)

- `opentelemetry-otlp` traces exporter, W3C trace-context propagation
- Auto-instrumented HTTP / DB / cache spans
- `metrics` crate + OTLP / Prometheus exporter
- Structured JSON logs via `tracing` + `tracing-subscriber`
- `/healthz`, `/readyz`, `/livez` with dependency checks
- Panic capture: all Rust panics → structured error + trace span

---

## v0.7 — Ecosystem

### 📋 GraphQL

- `async-graphql` crate bridged to Python resolvers

### 📋 gRPC

- `tonic` for server and client
- Python stubs generated from `.proto`

### 📋 Background jobs

- Embedded job runner (Rust) with Redis / SQS adapters
- Celery protocol compatibility layer for migration
- Built-in cron-like scheduler

### 📋 Caching & sessions

- `moka` in-process + Redis / Memcached adapters
- Encrypted cookies / Redis / Memcached / DB-backed sessions

### 📋 Templating

- Askama (compile-time Rust)
- Tera (runtime)
- Jinja2 (compat)

### 📋 Integrations

- Email: `lettre`
- Storage: S3 / GCS / Azure Blob via `rust-s3`
- Auth providers: SAML, WebAuthn (passkeys), LDAP
- Feature flags: LaunchDarkly-compatible API
- i18n: `fluent` crate
- AI/LLM hooks: streaming SSE helpers for LLM token streams, MCP client/server

---

## v0.8 — Compatibility & migration

### 📋 Migration adapters

- `rustapi.compat.fastapi` — accepts `APIRouter`, `Depends`, `HTTPException`,
  Pydantic models verbatim
- `rustapi.compat.flask` — `@app.route` sync handlers on the blocking pool
- `rustapi.compat.django` — URLconf include, Django ORM session backend,
  `request.user` shim
- ASGI / WSGI hosting (Forge can host any ASGI/WSGI app as a fallback route)
- `rustapi migrate fastapi ./app` CLI: AST transforms to idiomatic rustapi

---

## v0.9 — Hardening

### 📋 Testing & QA

- `TestClient` (in-process, no socket hop)
- `pytest` fixtures: `app`, `client`, `db_transaction`, `mock_auth`, `frozen_time`
- Snapshot testing for OpenAPI schema
- Property-based testing via `proptest` (Rust) and `hypothesis` (Python)
- `rustapi bench` wraps `wrk` / `oha`, emits HDR histograms
- Fuzzing via `cargo-fuzz` (libFuzzer) on HTTP parser + router
- Mutation testing: `mutmut` (Python) + `cargo-mutants` (Rust)

### 📋 Security audit

- `cargo-audit` + `pip-audit` clean on every release
- Third-party penetration test report published
- JWT/OAuth2/OIDC conformance suites passing
- Request smuggling tests (CLTE, TECL, TE2CL variants)

---

## v1.0 — Production-ready

### 📋 Distribution

- Wheels for CPython 3.9–3.13 on manylinux x86_64/aarch64, musllinux,
  macOS universal2, Windows x86_64/aarch64
- PyPy and GraalPy wheels
- `pip install rustapi` works with no system Rust
- Docker official images (distroless + slim)
- Helm chart published to artifact hub
- SBOM (CycloneDX) per release
- SLSA Level 3 build provenance

### 📋 Documentation

- Tutorial covering REST, WebSocket, GraphQL, auth, ORM, deploy
- Reference docs for every public API
- Architecture decision records (ADRs)
- Migration guides from FastAPI, Flask, Django
- Security hardening guide
- Performance tuning guide
- Versioned docs site (mkdocs-material)

### 📋 Release process

- SemVer policy documented and enforced in CI
- Deprecation warnings 2 minor releases prior to removal
- Release notes from conventional commits
- RC phase ≥ 4 weeks before each minor
- LTS branch policy (12-month cadence, 18-month support)
- Security disclosure policy + PGP key
- On-call rotation for critical CVEs

---

## Known limitations in v0.1

These are deliberate scoping decisions, not bugs:

1. **One event loop per request** — `asyncio.run` is called per async handler,
   which adds ~0.5–1ms overhead. The fix (per-thread persistent loop) is
   planned for v0.2.
2. **JSON serialization via Python** — uses `json.dumps` / `json.loads` rather
   than `simd-json`. The roadmap moves this to Rust.
3. **No HTTP/2 or HTTP/3** — HTTP/1.1 only in v0.1.
4. **No TLS** — terminate TLS at a reverse proxy (nginx, Caddy) for now.
5. **No middleware pipeline** — handlers run directly; no CORS, rate-limit,
   compression, etc. yet.
6. **No ORM** — bring your own database library (SQLAlchemy, Tortoise, etc.).
7. **Single-process** — `--workers N` multiprocess mode is planned.
8. **No WebSocket / SSE** — planned for v0.5.
9. **No OpenAPI generation** — planned for v0.4 alongside the schema compiler.
10. **No CLI** — `rustapi new / serve / migrate / bench` planned for v0.2.

## How to contribute

The areas with the highest leverage right now:

1. **Per-thread event loop** (v0.2) — biggest perf win, well-scoped.
2. **`simd-json` integration** (v0.2) — straightforward, big payoff on large
   JSON payloads.
3. **Tower middleware stack** (v0.2) — unlocks CORS, rate-limit, compression
   for free.
4. **OpenAPI 3.1 generator** (v0.4) — high DX value, mostly mechanical.

Pick one, open an issue, and we'll scope it together.
