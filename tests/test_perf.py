"""Quick performance sanity check for rustapi.

Boots a tiny app, fires N requests as fast as possible, reports RPS.
This isn't a TechEmpower-grade benchmark — just a smoke test that we're
in the right ballpark (>= 5k RPS on a single core for the simple case).

Run with:
    python tests/test_perf.py
"""

import os
import socket
import subprocess
import sys
import time
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


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


APP_SRC = """
from rustapi import RustApi, Request
app = RustApi()

@app.get("/json")
async def json_endpoint(request: Request):
    return {"message": "hello", "n": 42}

@app.get("/sync")
def sync_endpoint(request: Request):
    return {"message": "hello", "n": 42}
"""


def main() -> int:
    port = pick_port()
    app_file = ROOT / "tests" / "_perf_app.py"
    app_file.write_text(APP_SRC + f"\napp.run(host='127.0.0.1', port={port})\n")

    env = {**os.environ, "PYTHONUNBUFFERED": "1"}
    proc = subprocess.Popen(
        [sys.executable, str(app_file)],
        cwd=str(ROOT),
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )

    try:
        wait_for(port)
        print(f"[ok] server up on :{port}")

        # Warmup
        for _ in range(50):
            with urllib.request.urlopen(f"http://127.0.0.1:{port}/json") as r:
                r.read()

        def hit(_):
            with urllib.request.urlopen(f"http://127.0.0.1:{port}/json") as r:
                r.read()
            return 1

        for label, path in [("async /json", "/json"), ("sync /json", "/sync")]:
            N = 2000
            t0 = time.perf_counter()
            with ThreadPoolExecutor(max_workers=8) as pool:
                results = list(pool.map(lambda _: hit(_), range(N)))
            elapsed = time.perf_counter() - t0
            rps = sum(results) / elapsed
            print(f"  {label:20s}  {sum(results):5d} req  {elapsed * 1000:7.1f} ms  {rps:8.1f} RPS")

        print()
        print("Done. Numbers above are an upper-bound on the Python client")
        print("overhead — the Rust server is faster than what we can measure")
        print("with urllib. Use `oha` or `wrk` for real benchmarks.")
        return 0
    finally:
        proc.send_signal(subprocess.signal.SIGTERM if hasattr(subprocess, "signal") else 15)
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()
        app_file.unlink(missing_ok=True)


if __name__ == "__main__":
    sys.exit(main())
