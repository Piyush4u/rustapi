"""Demo rustapi application.

Run with:
    python examples/demo.py

Then in another shell:
    curl http://127.0.0.1:8080/
    curl http://127.0.0.1:8080/health
    curl http://127.0.0.1:8080/users/42
    curl -X POST http://127.0.0.1:8080/echo -H 'Content-Type: application/json' -d '{"msg":"hi"}'
    curl http://127.0.0.1:8080/sync      # exercises a sync handler
    curl http://127.0.0.1:8080/text      # exercises a text response
    curl http://127.0.0.1:8080/missing   # 404
    curl -X DELETE http://127.0.0.1:8080/users/42  # 405
"""

import asyncio
import time

from rustapi import JsonResponse, Request, RustApi, TextResponse

app = RustApi(server_name="rustapi-demo/0.1")


@app.get("/")
async def index(request: Request):
    """Root endpoint — returns a JSON greeting."""
    return {"hello": "world", "ts": int(time.time())}


@app.get("/health")
async def health(request: Request):
    """Health-check endpoint."""
    return {"status": "ok"}


@app.get("/users/{id}")
async def get_user(request: Request):
    """Path-param extraction."""
    user_id = request.path_params["id"]
    try:
        uid = int(user_id)
    except ValueError:
        return JsonResponse(
            {"error": "id must be an integer"},
            status_code=400,
        )
    return {"id": uid, "name": f"User {uid}", "email": f"user{uid}@example.com"}


@app.get("/slow")
async def slow(request: Request):
    """Demonstrates async handler that yields control."""
    await asyncio.sleep(0.05)
    return {"slept": "50ms"}


@app.get("/sync")
def sync_handler(request: Request):
    """Sync handlers are also supported — they just don't `await`."""
    return {"sync": True, "method": request.method}


@app.get("/text")
async def text_endpoint(request: Request):
    """Return plain text."""
    return TextResponse("hello, world\n")


@app.post("/echo")
async def echo(request: Request):
    """Echo the request body as JSON."""
    try:
        body = request.json()
    except Exception as e:
        return JsonResponse({"error": "invalid JSON", "detail": str(e)}, status_code=400)
    return JsonResponse(
        {"echoed": body, "received_content_type": request.headers.get("content-type", "")}
    )


@app.get("/headers")
async def headers(request: Request):
    """Return all request headers."""
    return {"headers": dict(request.headers)}


@app.get("/query")
async def query(request: Request):
    """Return parsed query params."""
    return {"query": dict(request.query_params)}


if __name__ == "__main__":
    print("Starting rustapi demo on http://127.0.0.1:8080")
    print("Routes registered:", app.route_count())
    app.run(host="127.0.0.1", port=8080)
