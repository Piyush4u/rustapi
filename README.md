# rustapi

[![PyPI version](https://img.shields.io/pypi/v/rustapi.svg?logo=pypi&logoColor=white)](https://pypi.org/project/rustapi/)
[![PyPI pyversions](https://img.shields.io/pypi/pyversions/rustapi.svg?logo=python&logoColor=white)](https://pypi.org/project/rustapi/)
[![PyPI platforms](https://img.shields.io/pypi/wheel/rustapi.svg)](https://pypi.org/project/rustapi/#files)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/rustapi/rustapi#license)
[![CI](https://github.com/rustapi/rustapi/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/rustapi/rustapi/actions/workflows/ci.yml)
[![Release](https://github.com/rustapi/rustapi/actions/workflows/release.yml/badge.svg)](https://github.com/rustapi/rustapi/actions/workflows/release.yml)
[![Documentation](https://img.shields.io/badge/docs-README-blue.svg)](https://github.com/rustapi/rustapi#readme)
[![Changelog](https://img.shields.io/badge/changelog-Keep%20a%20Changelog-blue.svg)](CHANGELOG.md)

A Rust-core Python web framework. Hot paths in Rust, business logic in Python.

> **Status:** Alpha. This is a foundational MVP demonstrating the architecture
> thesis — a Rust-native HTTP engine exposed through a Python decorator API
> with a single FFI hop per request. The full feature set (HTTP/3, async ORM,
> GraphQL, gRPC, native OpenTelemetry) is on the roadmap; see `docs/ROADMAP.md`.

## Install

```bash
pip install rustapi
```

For local development:

```bash
git clone https://github.com/rustapi/rustapi
cd rustapi
maturin develop --release
```

## Quick start

```python
from rustapi import RustApi, Request, JsonResponse

app = RustApi()

@app.get("/")
async def index(request: Request):
    return {"hello": "world"}

@app.get("/users/{id}")
async def get_user(request: Request):
    user_id = request.path_params["id"]
    return {"id": int(user_id), "name": "Alice"}

@app.post("/echo")
async def echo(request: Request):
    body = await request.json()
    return JsonResponse({"echoed": body})

if __name__ == "__main__":
    app.run(host="127.0.0.1", port=8080)
```

Run it:

```bash
python app.py
# In another shell:
curl http://127.0.0.1:8080/
curl http://127.0.0.1:8080/users/42
curl -X POST http://127.0.0.1:8080/echo -d '{"msg":"hi"}'
```

## Architecture

```
┌──────────────────────────────────────────────────────────┐
│  Rust engine (single binary, tokio runtime)              │
│  ┌──────────┐  ┌─────────┐  ┌──────────┐  ┌──────────┐  │
│  │ hyper +  │→ │ matchit │→ │  serde   │→ │ hyper    │  │
│  │ tokio    │  │ router  │  │ serialize│  │ response │  │
│  └──────────┘  └─────────┘  └──────────┘  └──────────┘  │
│                       │                                  │
│                       ▼                                  │
│            ┌────────────────────┐                        │
│            │  PyO3 FFI bridge   │  ← single hop per req  │
│            └────────────────────┘                        │
└──────────────────────────────────────────────────────────┘
                       │
                       ▼
┌──────────────────────────────────────────────────────────┐
│  Python layer (thin)                                     │
│  @app.get @app.post ... decorator API                    │
│  user handlers (async or sync)                           │
└──────────────────────────────────────────────────────────┘
```

## What's implemented (v0.1)

- HTTP/1.1 server via `hyper` + `tokio`
- Radix-tree router via `matchit` (path params, 404 vs 405)
- PyO3 bridge with sync **and** async Python handlers
- JSON serialization via Python's `json` module
- `Response` / `JsonResponse` / `TextResponse` types
- `Request` with `method`, `path`, `query_params`, `path_params`, `headers`, `body`,
  `.json()`, `.text()`
- Single-FFI-hop request dispatch

## Roadmap

See `docs/ROADMAP.md` for the full plan. Highlights:

- HTTP/2 + HTTP/3 (h3/quinn)
- Tower middleware stack (CORS, CSRF, rate-limit, compression)
- Async ORM (sqlx-based, Django-style API)
- Native JWT/OAuth2/OIDC
- WebSocket + SSE
- OpenTelemetry-native tracing/metrics/logs
- FastAPI/Flask/Django compatibility adapters
- `simd-json` for response serialization
- `forge`/`rustapi` CLI (new, serve, migrate, bench)

## License

Licensed under the [MIT License](LICENSE-MIT).

Contributions are accepted under the same license unless explicitly
stated otherwise.

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md). The short version: open an issue
first, fork the repo, run `cargo fmt && cargo clippy && cargo test && python
tests/test_e2e.py` before submitting a PR.

## Security

Found a vulnerability? See [`SECURITY.md`](SECURITY.md). **Do not open a
public issue for security problems.**

## Changelog

See [`CHANGELOG.md`](CHANGELOG.md).

## Publishing

Releases are built by GitHub Actions and published to PyPI via Trusted
Publishing (OIDC). Maintainers: see [`docs/PUBLISHING.md`](docs/PUBLISHING.md)
for setup and the release runbook.
