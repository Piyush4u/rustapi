"""End-to-end integration test for rustapi.

Boots the demo server in a subprocess, fires HTTP requests at it with
urllib, and asserts the responses. Proves the full request lifecycle works:
hyper → matchit → PyO3 → Python handler → serde/JSON → hyper response.

Run with:
    python tests/test_e2e.py
"""

import json
import os
import signal
import socket
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEMO = ROOT / "examples" / "demo.py"


def pick_port() -> int:
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    return port


def wait_for(port: int, timeout: float = 10.0) -> None:
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            with socket.create_connection(("127.0.0.1", port), timeout=0.5):
                return
        except OSError:
            time.sleep(0.05)
    raise TimeoutError(f"server on :{port} didn't come up in {timeout}s")


def get(port: int, path: str, method: str = "GET", body: bytes | None = None, headers=None):
    url = f"http://127.0.0.1:{port}{path}"
    req = urllib.request.Request(url, method=method, data=body, headers=headers or {})
    try:
        with urllib.request.urlopen(req, timeout=5) as r:
            # `r.headers` is an http.client.HTTPMessage (case-insensitive);
            # convert to a plain dict but keep a case-insensitive lookup helper.
            raw_headers = dict(r.headers)
            return r.status, r.read(), raw_headers
    except urllib.error.HTTPError as e:
        return e.code, e.read(), dict(e.headers)


def hget(headers: dict, name: str) -> str:
    """Case-insensitive header lookup (urllib preserves wire case)."""
    name_lower = name.lower()
    for k, v in headers.items():
        if k.lower() == name_lower:
            return v
    return ""


def main() -> int:
    port = pick_port()
    env = {**os.environ, "PYTHONUNBUFFERED": "1"}
    proc = subprocess.Popen(
        [sys.executable, str(DEMO)],
        cwd=str(ROOT),
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    # Patch the demo to listen on our port.
    # Simpler: pass port via env and have the demo read it. For this test we
    # just use the default 8080 by making sure to override the demo's argv.
    # Instead, restart with port override.
    proc.send_signal(signal.SIGTERM)
    proc.wait(timeout=5)

    # Write a tiny launcher that imports the demo's `app` and runs on our port.
    launcher = ROOT / "tests" / "_launcher.py"
    launcher.write_text(
        f"import sys; sys.path.insert(0, '{(ROOT / 'examples')}'); "
        "import demo; demo.app.run(host='127.0.0.1', port=" + str(port) + ")"
    )

    proc = subprocess.Popen(
        [sys.executable, str(launcher)],
        cwd=str(ROOT),
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    try:
        wait_for(port)
        print(f"[ok] server up on :{port}")

        failures = 0

        def check(name, cond, detail=""):
            nonlocal failures
            mark = "ok" if cond else "FAIL"
            if not cond:
                failures += 1
            print(f"  [{mark}] {name} {detail}")

        # 1. JSON root
        status, body, _ = get(port, "/")
        check("GET / -> 200", status == 200, f"(got {status})")
        check("GET / body has hello", b"hello" in body, f"(body={body[:80]!r})")

        # 2. Health
        status, body, _ = get(port, "/health")
        check("GET /health -> 200", status == 200)
        check("GET /health body", b'"status": "ok"' in body, f"(body={body!r})")

        # 3. Path params
        status, body, _ = get(port, "/users/42")
        check("GET /users/42 -> 200", status == 200)
        payload = json.loads(body)
        check("path param id=42", payload.get("id") == 42, f"(payload={payload})")

        # 4. Path param validation
        status, body, _ = get(port, "/users/abc")
        check("GET /users/abc -> 400", status == 400, f"(got {status})")

        # 5. Sync handler
        status, body, _ = get(port, "/sync")
        check("GET /sync -> 200 (sync handler)", status == 200)
        check("sync handler returns sync=True", b'"sync": true' in body.lower(), f"(body={body!r})")

        # 6. Text response
        status, body, hdrs = get(port, "/text")
        check("GET /text -> 200", status == 200)
        check("text body", body == b"hello, world\n", f"(body={body!r})")
        check(
            "text content-type",
            "text/plain" in hget(hdrs, "Content-Type"),
            f"(ct={hget(hdrs, 'Content-Type')!r})",
        )

        # 7. Async with await
        status, body, _ = get(port, "/slow")
        check("GET /slow (async await) -> 200", status == 200)
        check("async handler completed", b'"slept": "50ms"' in body, f"(body={body!r})")

        # 8. POST echo
        payload = {"msg": "hi", "n": 7}
        status, body, _ = get(
            port,
            "/echo",
            method="POST",
            body=json.dumps(payload).encode(),
            headers={"Content-Type": "application/json"},
        )
        check("POST /echo -> 200", status == 200)
        echoed = json.loads(body)
        check("echoed body matches", echoed.get("echoed") == payload, f"(echoed={echoed})")

        # 9. Headers endpoint
        status, body, _ = get(port, "/headers", headers={"X-Custom": "abc"})
        check("GET /headers -> 200", status == 200)
        hdrs_payload = json.loads(body)
        check(
            "custom header seen",
            hdrs_payload.get("headers", {}).get("x-custom") == "abc",
            f"(hdrs={hdrs_payload})",
        )

        # 10. Query params
        status, body, _ = get(port, "/query?a=1&b=two")
        check("GET /query?a=1&b=two -> 200", status == 200)
        qp = json.loads(body)
        check("query params parsed", qp.get("query") == {"a": "1", "b": "two"}, f"(qp={qp})")

        # 11. 404
        status, _, _ = get(port, "/definitely-not-a-route")
        check("GET /missing -> 404", status == 404, f"(got {status})")

        # 12. 405
        status, _, _ = get(port, "/users/42", method="DELETE")
        check("DELETE /users/42 -> 405", status == 405, f"(got {status})")

        # 13. Server header
        status, _, hdrs = get(port, "/")
        check(
            "server header set",
            "rustapi" in hget(hdrs, "Server").lower(),
            f"(server={hget(hdrs, 'Server')!r})",
        )

        print()
        if failures == 0:
            print(f"ALL PASS — full request lifecycle works on port {port}")
            return 0
        else:
            print(f"{failures} FAILURE(S)")
            return 1
    finally:
        proc.send_signal(signal.SIGTERM)
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()
        launcher.unlink(missing_ok=True)


if __name__ == "__main__":
    sys.exit(main())
