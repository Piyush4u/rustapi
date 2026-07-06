import asyncio

import httpx
import pytest

BASE_URL = "http://127.0.0.1:8080"


@pytest.fixture(scope="session")
def event_loop():
    loop = asyncio.get_event_loop_policy().new_event_loop()
    yield loop
    loop.close()


@pytest.mark.asyncio
async def test_full_user_flow():
    async with httpx.AsyncClient(base_url=BASE_URL) as client:
        # 1. Register User
        res = await client.post(
            "/api/auth/register",
            json={"username": "testuser", "email": "test@example.com", "password": "password123"},
        )
        # If already exists from previous test, it might fail with 400. That's fine for simple test, let's use random email.
        import uuid

        uid = str(uuid.uuid4())[:8]
        email = f"user_{uid}@test.com"
        username = f"user_{uid}"

        res = await client.post(
            "/api/auth/register",
            json={"username": username, "email": email, "password": "password123"},
        )
        assert res.status_code == 201

        # 2. Login
        res = await client.post("/api/auth/login", json={"email": email, "password": "password123"})
        assert res.status_code == 200
        token = res.json()["access_token"]

        headers = {"Authorization": f"Bearer {token}"}

        # 3. Create Post
        res = await client.post(
            "/api/posts", json={"content": "Hello World from Pytest!"}, headers=headers
        )
        assert res.status_code == 201
        post_id = res.json()["id"]

        # 4. Create Comment
        res = await client.post(
            f"/api/posts/{post_id}/comments", json={"content": "Nice post!"}, headers=headers
        )
        assert res.status_code == 201

        # 5. Fetch Feed
        res = await client.get("/api/posts")
        assert res.status_code == 200
        feed = res.json()
        assert len(feed) > 0
        assert feed[0]["content"] == "Hello World from Pytest!"
        assert len(feed[0]["comments"]) == 1
        assert feed[0]["comments"][0]["content"] == "Nice post!"
