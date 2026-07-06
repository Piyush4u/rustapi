import jwt
import bcrypt
import os
from datetime import datetime, timedelta, timezone
from functools import wraps
from typing import Any, Callable

from rustapi import Request, JsonResponse
from app.database import AsyncSessionLocal
from app.models import User
from sqlalchemy.future import select

SECRET_KEY = os.getenv("SECRET_KEY", "super-secret-key-for-testing")
ALGORITHM = "HS256"
ACCESS_TOKEN_EXPIRE_MINUTES = 60 * 24 # 1 day

def hash_password(password: str) -> str:
    salt = bcrypt.gensalt()
    pwd_hash = bcrypt.hashpw(password.encode('utf-8'), salt)
    return pwd_hash.decode('utf-8')

def verify_password(plain_password: str, hashed_password: str) -> bool:
    return bcrypt.checkpw(plain_password.encode('utf-8'), hashed_password.encode('utf-8'))

def create_access_token(data: dict) -> str:
    to_encode = data.copy()
    expire = datetime.now(timezone.utc) + timedelta(minutes=ACCESS_TOKEN_EXPIRE_MINUTES)
    to_encode.update({"exp": expire})
    encoded_jwt = jwt.encode(to_encode, SECRET_KEY, algorithm=ALGORITHM)
    return encoded_jwt

def require_auth(func: Callable[..., Any]) -> Callable[..., Any]:
    """
    Custom decorator to enforce authentication.
    Intercepts the rustapi Request, verifies the Bearer token,
    and fetches the user from the database.
    Injects `request.user` into the request object (or a custom attribute).
    """
    @wraps(func)
    async def wrapper(request: Request, *args: Any, **kwargs: Any) -> Any:
        auth_header = request.headers.get("authorization")
        if not auth_header or not auth_header.startswith("Bearer "):
            return JsonResponse({"detail": "Not authenticated"}, status_code=401)
        
        token = auth_header.split(" ")[1]
        try:
            payload = jwt.decode(token, SECRET_KEY, algorithms=[ALGORITHM])
            user_id: int = payload.get("sub")
            if user_id is None:
                return JsonResponse({"detail": "Invalid authentication credentials"}, status_code=401)
        except jwt.PyJWTError:
            return JsonResponse({"detail": "Invalid authentication credentials"}, status_code=401)
            
        async with AsyncSessionLocal() as session:
            result = await session.execute(select(User).filter(User.id == int(user_id)))
            user = result.scalars().first()
            if not user:
                return JsonResponse({"detail": "User not found"}, status_code=401)
            
            # Pass user to the decorated function
            kwargs["user"] = user
            
        return await func(request, *args, **kwargs)
    return wrapper
