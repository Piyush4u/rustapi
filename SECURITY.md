# Security Policy

## Supported versions

rustapi is pre-1.0. We support the latest minor release with security patches.

| Version | Supported          |
|---------|--------------------|
| 0.1.x   | ✅ active          |
| < 0.1   | ❌ not supported   |

## Reporting a vulnerability

**Please do not open public GitHub issues for security vulnerabilities.**

Instead, email **security@rustapi.dev** (or open a private security advisory
on GitHub via the "Security" tab → "Report a vulnerability") with:

1. A description of the vulnerability and its impact.
2. Steps to reproduce, or a proof-of-concept.
3. Affected versions (if known).
4. Any suggested mitigations.

You should receive an acknowledgment within 48 hours. We'll coordinate a fix
and disclosure timeline with you. Please do not publicly disclose the issue
until we've shipped a patch.

## Disclosure policy

- We acknowledge receipt within 48 hours.
- We investigate and confirm the vulnerability within 7 days.
- We ship a patch within 30 days for high-severity issues, 90 days for low.
- We publish a security advisory on GitHub and PyPI after the patch is released.
- We credit reporters (unless they prefer to remain anonymous).

## Threat model (v0.1)

This is a v0.1 framework. Treat it as **not yet production-hardened**:

- No TLS — terminate at a reverse proxy (nginx, Caddy, etc.).
- No built-in auth — implement JWT/OAuth2 in Python handlers.
- No rate limiting — protect endpoints at the edge.
- No CSRF / CORS middleware yet.
- No request smuggling hardening beyond what `hyper` provides by default.
- Body size is capped at 10 MiB by default; configure via `ServerConfig`.

The v0.3 release will add the security primitives listed in
[`docs/ROADMAP.md`](docs/ROADMAP.md) — JWT, OAuth2, CSRF, rate-limit,
TLS, security headers — all implemented in Rust.

Until then, **do not deploy rustapi directly to the internet without a
reverse proxy and proper edge protection**.

## Dependency policy

- We run `cargo audit` on every CI build.
- We run `pip-audit` on the Python dependencies.
- Critical vulnerabilities in dependencies trigger an immediate patch release.
- We prefer dependencies with active maintenance and a permissive license.
