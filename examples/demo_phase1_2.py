"""Example demonstrating Phase 1 & 2 features: Dependency Injection, OpenAPI, and ORM."""

from typing import Optional

from rustapi import Body, Depends, Model, Path, Query, RustApi

# =============================================================================
# Initialize app with automatic OpenAPI documentation
# =============================================================================
app = RustApi(
    title="My Awesome API",
    version="1.0.0",
    description="A demonstration of rustapi's Django/FastAPI compatibility",
)

# =============================================================================
# Phase 1: Dependency Injection (FastAPI-compatible)
# =============================================================================


# Simulated database connection pool
class Database:
    def __init__(self):
        self.connected = False

    async def connect(self):
        self.connected = True
        print("Database connected")

    async def disconnect(self):
        self.connected = False
        print("Database disconnected")

    async def get_user(self, user_id: int) -> dict:
        return {"id": user_id, "name": f"User {user_id}", "email": f"user{user_id}@example.com"}

    async def create_user(self, name: str, email: str) -> dict:
        return {"id": 42, "name": name, "email": email}


# Dependency: Create/get database connection per request
async def get_db() -> Database:
    """Dependency that provides a database connection."""
    db = Database()
    await db.connect()
    try:
        yield db  # Context manager style (like FastAPI)
    finally:
        await db.disconnect()


# Another dependency: Authentication
async def get_current_user(token: str = Header(None)) -> Optional[dict]:
    """Dependency that validates authentication token."""
    if not token:
        return None
    # In real app, validate token and fetch user
    return {"id": 1, "username": "authenticated_user"}


# =============================================================================
# Phase 1: Routes with Dependency Injection
# =============================================================================


@app.get("/")
async def root():
    """Root endpoint - returns welcome message."""
    return {"message": "Welcome to rustapi!", "docs": "/docs"}


@app.get("/users/{user_id}")
async def get_user(
    user_id: int = Path(..., description="The ID of the user to retrieve"),
    db: Database = Depends(get_db),
):
    """Get a specific user by ID.

    - **user_id**: The unique identifier of the user
    - Returns user data or 404
    """
    user = await db.get_user(user_id)
    return user


@app.get("/users/")
async def list_users(
    skip: int = Query(0, ge=0, description="Number of users to skip"),
    limit: int = Query(10, ge=1, le=100, description="Maximum number of users to return"),
    db: Database = Depends(get_db),
):
    """List all users with pagination.

    - **skip**: Number of records to skip (pagination)
    - **limit**: Maximum records to return (max 100)
    """
    # In real app: query database with skip/limit
    return {
        "items": [],
        "total": 0,
        "skip": skip,
        "limit": limit,
    }


@app.post("/users/")
async def create_user(
    user_data: dict = Body(..., description="User data to create"),
    db: Database = Depends(get_db),
):
    """Create a new user.

    - **name**: User's full name (required)
    - **email**: User's email address (required)
    """
    name = user_data.get("name")
    email = user_data.get("email")

    if not name or not email:
        return {"error": "name and email are required"}, 400

    user = await db.create_user(name, email)
    return user, 201


# =============================================================================
# Phase 1: Automatic OpenAPI Documentation Endpoints
# =============================================================================


@app.get(app.openapi_url)
async def openapi_json():
    """Return OpenAPI 3.1 specification as JSON."""
    return app.openapi_json_response()


@app.get(app.docs_url)
async def swagger_ui():
    """Interactive Swagger UI documentation."""
    return app.swagger_ui_response()


@app.get(app.redoc_url)
async def redoc():
    """Alternative ReDoc documentation."""
    return app.redoc_response()


# =============================================================================
# Phase 2: Django-style ORM Models
# =============================================================================


class User(Model):
    """User model with Django-style ORM."""

    id: int
    username: str
    email: str
    is_active: bool = True
    created_at: str  # datetime in real app


# Example ORM usage (would work once database is configured)
@app.get("/orm/users/")
async def list_users_orm():
    """Demonstrate Django-style ORM queries."""
    # These would execute against real database:

    # Get all active users ordered by creation date
    # users = await User.objects.filter(is_active=True).order_by("-created_at").all()

    # Get first user matching criteria
    # user = await User.objects.filter(email__endswith="@example.com").first()

    # Count users
    # count = await User.objects.filter(is_active=True).count()

    # Pagination
    # page = await User.objects.all().offset(20).limit(10).all()

    return {
        "message": "ORM queries demonstrated in code comments",
        "features": [
            ".filter(**kwargs) - WHERE conditions",
            ".exclude(**kwargs) - NOT conditions",
            ".order_by(*fields) - ORDER BY (prefix with - for DESC)",
            ".limit(n) - LIMIT clause",
            ".offset(n) - OFFSET clause",
            ".all() - Execute and return all results",
            ".first() - Execute and return first result",
            ".count() - Count matching records",
            ".delete() - Delete matching records",
        ],
    }


# =============================================================================
# Phase 2: Migrations
# =============================================================================


class CreateUserTable(Migration):
    """Migration to create users table."""

    name = "001_create_users"

    async def apply(self, database):
        """Create the users table."""
        await database.execute("""
            CREATE TABLE IF NOT EXISTS users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                username TEXT NOT NULL UNIQUE,
                email TEXT NOT NULL UNIQUE,
                is_active BOOLEAN DEFAULT 1,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
        """)

    async def rollback(self, database):
        """Drop the users table."""
        await database.execute("DROP TABLE IF EXISTS users")


class AddUserIndex(Migration):
    """Migration to add index on email."""

    name = "002_add_email_index"
    dependencies = ["001_create_users"]

    async def apply(self, database):
        await database.execute("CREATE INDEX idx_users_email ON users(email)")

    async def rollback(self, database):
        await database.execute("DROP INDEX IF EXISTS idx_users_email")


# Setup migrations
migrator = Migrator("sqlite:///mydb.sqlite3")
migrator.add_migration(CreateUserTable())
migrator.add_migration(AddUserIndex())


# Migration management endpoints
@app.post("/migrations/run")
async def run_migrations():
    """Run all pending database migrations."""
    # await migrator.run()
    return {"message": "Migrations would be applied here"}


@app.post("/migrations/rollback")
async def rollback_migrations(steps: int = Body(1)):
    """Rollback specified number of migrations."""
    # await migrator.rollback(steps)
    return {"message": f"Would rollback {steps} migration(s)"}


# =============================================================================
# Run the server
# =============================================================================

if __name__ == "__main__":
    print("=" * 60)
    print("rustapi - Django/FastAPI compatible with Rust performance")
    print("=" * 60)
    print("\n📖 API Documentation:")
    print("   Swagger UI: http://127.0.0.1:8080/docs")
    print("   ReDoc:      http://127.0.0.1:8080/redoc")
    print("   OpenAPI:    http://127.0.0.1:8080/openapi.json")
    print("\n🚀 Features demonstrated:")
    print("   ✅ FastAPI-compatible Depends() injection")
    print("   ✅ Automatic OpenAPI 3.1 schema generation")
    print("   ✅ Swagger UI & ReDoc documentation")
    print("   ✅ Django-style ORM with QuerySet")
    print("   ✅ Migration system")
    print("   ✅ Path/Query/Body/Header parameter validation")
    print(f"\n{'=' * 60}\n")

    run(app, host="127.0.0.1", port=8080)
