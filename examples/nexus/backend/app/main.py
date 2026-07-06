import asyncio
from rustapi import RustApi, Request, JsonResponse
from sqlalchemy.future import select
from sqlalchemy.orm import selectinload

from app.database import init_db, AsyncSessionLocal
from app.models import User, Post, Comment, Follow
from app.schemas import UserCreate, UserResponse, PostCreate, PostResponse, CommentCreate, CommentResponse
from app.auth import hash_password, verify_password, create_access_token, require_auth

app = RustApi()

# --- Auth Routes ---

@app.post("/api/auth/register")
async def register(request: Request):
    try:
        body = request.json()
        user_data = UserCreate(**body)
    except Exception as e:
        return JsonResponse({"detail": str(e)}, status_code=422)
    
    async with AsyncSessionLocal() as session:
        result = await session.execute(select(User).filter(User.email == user_data.email))
        if result.scalars().first():
            return JsonResponse({"detail": "Email already registered"}, status_code=400)
            
        new_user = User(
            username=user_data.username,
            email=user_data.email,
            password_hash=hash_password(user_data.password)
        )
        session.add(new_user)
        await session.commit()
        await session.refresh(new_user)
        
        return JsonResponse(UserResponse.model_validate(new_user).model_dump(mode="json"), status_code=201)

@app.post("/api/auth/login")
async def login(request: Request):
    try:
        body = request.json()
    except Exception as e:
        return JsonResponse({"detail": "Invalid JSON"}, status_code=400)
        
    email = body.get("email")
    password = body.get("password")
    
    if not email or not password:
        return JsonResponse({"detail": "Missing email or password"}, status_code=400)
        
    async with AsyncSessionLocal() as session:
        result = await session.execute(select(User).filter(User.email == email))
        user = result.scalars().first()
        
        if not user or not verify_password(password, user.password_hash):
            return JsonResponse({"detail": "Invalid email or password"}, status_code=401)
            
        access_token = create_access_token(data={"sub": str(user.id)})
        return JsonResponse({"access_token": access_token, "token_type": "bearer"})

@app.get("/api/users/me")
@require_auth
async def get_me(request: Request, user: User = None):
    return JsonResponse(UserResponse.model_validate(user).model_dump(mode="json"))

# --- Post Routes ---

@app.get("/api/posts")
async def list_posts(request: Request):
    async with AsyncSessionLocal() as session:
        result = await session.execute(
            select(Post).options(selectinload(Post.author), selectinload(Post.comments).selectinload(Comment.author)).order_by(Post.created_at.desc()).limit(50)
        )
        posts = result.scalars().all()
        return JsonResponse([PostResponse.model_validate(p).model_dump(mode="json") for p in posts])

@app.post("/api/posts")
@require_auth
async def create_post(request: Request, user: User = None):
    try:
        body = request.json()
        post_data = PostCreate(**body)
        
        async with AsyncSessionLocal() as session:
            new_post = Post(content=post_data.content, author_id=user.id)
            session.add(new_post)
            await session.commit()
            await session.refresh(new_post)
            
            # Load author and comments for response
            result = await session.execute(
                select(Post)
                .options(selectinload(Post.author), selectinload(Post.comments))
                .filter(Post.id == new_post.id)
            )
            new_post = result.scalars().first()
            
            return JsonResponse(PostResponse.model_validate(new_post).model_dump(mode="json"), status_code=201)
    except Exception as e:
        import traceback
        traceback.print_exc()
        return JsonResponse({"detail": str(e)}, status_code=422)

# --- Comment Routes ---

@app.post("/api/posts/{post_id}/comments")
@require_auth
async def create_comment(request: Request, user: User = None):
    try:
        post_id = request.path_params["post_id"]
        body = request.json()
        comment_data = CommentCreate(**body)
        
        async with AsyncSessionLocal() as session:
            # Check if post exists
            post = await session.get(Post, int(post_id))
            if not post:
                return JsonResponse({"detail": "Post not found"}, status_code=404)
                
            new_comment = Comment(content=comment_data.content, author_id=user.id, post_id=int(post_id))
            session.add(new_comment)
            await session.commit()
            await session.refresh(new_comment)
            
            result = await session.execute(select(Comment).options(selectinload(Comment.author)).filter(Comment.id == new_comment.id))
            new_comment = result.scalars().first()
            
            return JsonResponse(CommentResponse.model_validate(new_comment).model_dump(mode="json"), status_code=201)
    except Exception as e:
        import traceback
        traceback.print_exc()
        return JsonResponse({"detail": str(e)}, status_code=422)

if __name__ == "__main__":
    # Initialize DB synchronously for simplicity, but wait it's async
    loop = asyncio.get_event_loop()
    loop.run_until_complete(init_db())
    
    print("Starting Nexus API on http://127.0.0.1:8080")
    app.run(host="127.0.0.1", port=8080)
