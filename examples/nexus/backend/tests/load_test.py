import asyncio
import time

import httpx

BASE_URL = "http://127.0.0.1:8080"
NUM_REQUESTS = 1000
CONCURRENCY = 50


async def fetch_feed(client):
    try:
        res = await client.get("/api/posts", timeout=10.0)
        return res.status_code
    except Exception:
        return 500


async def worker(queue, client, results):
    while True:
        try:
            _ = queue.get_nowait()
        except asyncio.QueueEmpty:
            break

        start = time.time()
        status = await fetch_feed(client)
        end = time.time()
        results.append((status, end - start))
        queue.task_done()


async def main():
    print(f"Starting load test against {BASE_URL}")
    print(f"Concurrency: {CONCURRENCY}, Total Requests: {NUM_REQUESTS}")

    queue = asyncio.Queue()
    for i in range(NUM_REQUESTS):
        queue.put_nowait(i)

    results = []

    # We use httpx.AsyncClient with high limits for load testing
    limits = httpx.Limits(max_connections=CONCURRENCY, max_keepalive_connections=CONCURRENCY)

    start_time = time.time()
    async with httpx.AsyncClient(base_url=BASE_URL, limits=limits) as client:
        tasks = []
        for _ in range(CONCURRENCY):
            task = asyncio.create_task(worker(queue, client, results))
            tasks.append(task)

        await asyncio.gather(*tasks)

    total_time = time.time() - start_time

    successes = sum(1 for r in results if r[0] == 200)
    avg_latency = sum(r[1] for r in results) / len(results) if results else 0
    req_per_sec = NUM_REQUESTS / total_time

    print("\n--- Load Test Results ---")
    print(f"Total Time: {total_time:.2f}s")
    print(f"Requests per second: {req_per_sec:.2f} req/s")
    print(f"Success Rate: {(successes / NUM_REQUESTS) * 100:.1f}% ({successes}/{NUM_REQUESTS})")
    print(f"Average Latency: {avg_latency * 1000:.2f}ms")


if __name__ == "__main__":
    asyncio.run(main())
