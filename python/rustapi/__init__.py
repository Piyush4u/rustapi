"""rustapi: a Rust-core Python web framework.

The native extension `_rustapi` (built by maturin from the Rust workspace)
provides the HTTP engine, router, and the `_RustApi` app class. This pure-Python
package wraps it with the ergonomic decorator API and convenience types.

Design rule: every hot path — HTTP parsing, routing, header munging, byte
serialization — lives in Rust. Python is reduced to a thin decorator shell
plus the user's business logic.
"""

from typing import Any, Callable, Dict, Optional, TypeVar

from ._rustapi import (
    Request,
    Response,
    _RustApi,
)
from ._rustapi import __version__ as __version__

__all__ = [
    "RustApi",
    "Request",
    "Response",
    "JsonResponse",
    "TextResponse",
    "run",
    "__version__",
]

F = TypeVar("F", bound=Callable[..., Any])


def JsonResponse(
    data: Any,
    status_code: int = 200,
    headers: Optional[Dict[str, str]] = None,
) -> Response:
    """Build a 200 JSON response from any JSON-serializable Python object.

    Uses the native `Response.json` staticmethod which calls Python's
    `json.dumps`. The roadmap moves this to a Rust `simd-json` path for
    an additional 3-10x on large payloads.
    """
    return Response.json(data, status_code=status_code, headers=headers)


def TextResponse(
    text: str,
    status_code: int = 200,
    headers: Optional[Dict[str, str]] = None,
) -> Response:
    """Build a 200 `text/plain; charset=utf-8` response."""
    return Response.text(text, status_code=status_code, headers=headers)


# ---------------------------------------------------------------------------
# Decorator factory
# ---------------------------------------------------------------------------

_HTTP_METHODS = ("get", "post", "put", "delete", "patch", "head", "options")


def _make_method_decorator(method_name: str) -> Callable[..., Any]:
    """Build a decorator factory for `method_name` (e.g. "get" -> "GET")."""

    method_upper = method_name.upper()

    def decorator(self: "RustApi", path: str) -> Callable[[F], F]:
        def wrapper(fn: F) -> F:
            self._register(method_upper, path, fn)
            return fn

        return wrapper

    decorator.__name__ = method_name
    decorator.__doc__ = (
        f"Register a handler for HTTP {method_upper} requests matching `path`.\n\n"
        f"Usage:\n\n"
        f'    @app.{method_name}("/path/{{id}}")\n'
        f"    async def handler(request: Request) -> Response:\n"
        f"        ...\n\n"
        f"The handler may be sync or async. Path params are accessible via\n"
        f"`request.path_params`."
    )
    return decorator


class RustApi(_RustApi):
    """The main application class.

    Example:
        ```python
        from rustapi import RustApi

        app = RustApi()

        @app.get("/")
        async def index(request):
            return {"hello": "world"}

        if __name__ == "__main__":
            app.run(host="127.0.0.1", port=8080)
        ```
    """


# Attach the HTTP method decorators as class methods.
for _method in _HTTP_METHODS:
    setattr(RustApi, _method, _make_method_decorator(_method))
del _method


def run(app: RustApi, host: str = "127.0.0.1", port: int = 8080) -> None:
    """Run a `RustApi` app on the given host/port. Blocks the calling thread."""
    app.run(host=host, port=port)
