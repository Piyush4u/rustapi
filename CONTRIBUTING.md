# Contributing to rustapi

Thanks for your interest in contributing! This document covers the basics.
For architecture context, see [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).
For the feature plan, see [`docs/ROADMAP.md`](docs/ROADMAP.md).

## Development setup

### Prerequisites

- **Rust** 1.75+ (stable): https://rustup.rs
- **Python** 3.8+ (CPython)
- **maturin** 1.4+: `pip install maturin`
- (optional) **aiohttp** for running the async benchmark: `pip install aiohttp`

### Build & install in dev mode

```bash
git clone https://github.com/rustapi/rustapi
cd rustapi
maturin develop --release
```

This builds the Rust extension in release mode and installs it into your
active Python environment. The pure-Python `rustapi/` package is picked up
via the `python-source` setting in `pyproject.toml`.

### Run the tests

```bash
# Rust unit tests (engine crate)
cargo test -p rustapi-engine

# End-to-end integration tests (boots the demo server, fires HTTP requests)
python tests/test_e2e.py

# Performance smoke test
python tests/test_perf.py
python tests/test_perf_async.py   # requires aiohttp
```

### Run the demo

```bash
python examples/demo.py
# In another shell:
curl http://127.0.0.1:8080/
curl http://127.0.0.1:8080/users/42
```

## Code style

### Rust

- Run `cargo fmt` before committing.
- Run `cargo clippy --workspace --all-targets -- -D warnings` and fix all
  warnings.
- Public items have doc comments (`///`).
- Tests live in `#[cfg(test)] mod tests { ... }` blocks in the same file.
- No `unwrap()` or `expect()` on the FFI boundary — use `PyResult` and
  propagate errors.

### Python

- Run `ruff check .` and `ruff format .` before committing.
- Run `mypy rustapi/` (will be strict once stubs are shipped in v0.2).
- Type hints on all public APIs.
- Docstrings on public functions and classes.

## Architecture notes

The defining rule: **a request crosses the Python boundary exactly once** —
to invoke the user's handler. Everything else (HTTP parsing, routing,
serialization) stays in Rust.

When adding a feature, ask: *can this run in Rust without touching Python?*
If yes, it belongs in `rustapi-engine`. If it needs to call Python, it goes
in `rustapi-py` as a `PyHandler`-style adapter. The pure-Python `rustapi/`
package should stay thin — decorators, type hints, convenience factories.

## Pull request flow

1. Open an issue first for non-trivial changes — we'll scope it together.
2. Fork the repo, create a feature branch.
3. Make your changes. Keep commits focused; squash if needed.
4. Ensure `cargo fmt && cargo clippy && cargo test && python tests/test_e2e.py`
   all pass.
5. Update `CHANGELOG.md` under `[Unreleased]`.
6. Open a PR with a clear description. Reference the issue.

## Release flow

Releases are tagged as `vX.Y.Z` and pushed by maintainers. The CI workflow
builds wheels for all supported platforms and publishes to PyPI via Trusted
Publishing (OIDC) — no API tokens needed.

See [`.github/workflows/release.yml`](.github/workflows/release.yml) for the
exact matrix.

## Areas with the highest leverage right now

These are well-scoped, high-impact contributions:

1. **Per-thread event loop** (v0.2) — biggest perf win. Replace the
   per-request `asyncio.run` with a thread-local persistent loop.
2. **`simd-json` integration** (v0.2) — straightforward, big payoff on
   large JSON payloads.
3. **Tower middleware stack** (v0.2) — unlocks CORS, rate-limit,
   compression for free.
4. **OpenAPI 3.1 generator** (v0.4) — high DX value, mostly mechanical.
5. **Type stubs (`*.pyi`)** — ship `py.typed`-compliant stubs so users get
   IDE autocomplete.

Pick one, comment on the issue, and we'll get you started.

## Code of conduct

Be kind. Disagreements are fine; personal attacks are not. We follow the
[Contributor Covenant](https://www.contributor-covenant.org/version/2/1/code_of_conduct/).
