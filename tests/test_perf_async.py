"""Real async benchmark for rustapi using aiohttp.

Boots the demo server, fires N concurrent requests with aiohttp's async
client, reports p50/p95/p99 latency and RPS.

Run with:
    python tests/test_perf_async.py
"""

import asyncio
import os
import socket
import subprocess
import sys
import time
from pathlib import Path

import aiohttp

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

@app.get("/plaintext")
async def plaintext(request: Request):
    return "Hello, World!"
"""


async def bench(session: aiohttp.ClientSession, url: str, n: int, concurrency: int) -> dict:
    sem = asyncio.Semaphore(concurrency)
    latencies = []
    errors = 0

    async def one():
        nonlocal errors
        async with sem:
            t0 = time.perf_counter_ns()
            try:
                async with session.get(url) as r:
                    await r.read()
                    if r.status != 200:
                        errors += 1
            except Exception:
                errors += 1
            latencies.append(time.perf_counter_ns() - t0)

    t0 = time.perf_counter()
    await asyncio.gather(*(one() for _ in range(n)))
    elapsed = time.perf_counter() - t0

    latencies.sort()
    n_ok = len(latencies)
    p50 = latencies[n_ok // 2] / 1_000_000
    p95 = latencies[int(n_ok * 0.95)] / 1_000_000
    p99 = latencies[int(n_ok * 0.99)] / 1_000_000

    return {
        "rps": n_ok / elapsed,
        "p50_ms": p50,
        "p95_ms": p95,
        "p99_ms": p99,
        "errors": errors,
        "elapsed_s": elapsed,
    }


async def amain(port: int) -> None:
    async with aiohttp.ClientSession(
        connector=aiohttp.TCPConnector(limit=200, limit_per_host=200)
    ) as session:
        # Warmup
        for _ in range(100):
            async with session.get(f"http://127.0.0.1:{port}/json") as r:
                await r.read()

        for path in ("/json", "/sync", "/plaintext"):
            for concurrency in (50, 200):
                n = 5000
                r = await bench(session, f"http://127.0.0.1:{port}{path}", n, concurrency)
                print(
                    f"  {path:12s} c={concurrency:3d}  "
                    f"{r['rps']:8.1f} RPS  "
                    f"p50={r['p50_ms']:6.2f}ms  "
                    f"p95={r['p95_ms']:6.2f}ms  "
                    f"p99={r['p99_ms']:6.2f}ms  "
                    f"errors={r['errors']}"
                )


def main() -> int:
    port = pick_port()
    app_file = ROOT / "tests" / "_perf_async_app.py"
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
        print(f"[ok] server up on :{port}\n")
        asyncio.run(amain(port))
        return 0
    finally:
        proc.send_signal(15)
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()
        app_file.unlink(missing_ok=True)


if __name__ == "__main__":
    sys.exit(main())
