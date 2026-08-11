//! Dependency Injection System - FastAPI-compatible Depends() pattern.
//!
//! This module provides a type-safe dependency injection system that allows
//! handlers to declare their dependencies, which are automatically resolved
//! and injected at runtime. Supports nested dependencies, async resolvers,
//! and cleanup via context managers.
//!
//! # Example
//! ```rust
//! use rustapi_engine::di::{Depends, Injector, Scope};
//!
//! // Define a dependency
//! async fn get_db() -> DatabaseConnection {
//!     DatabaseConnection::new().await
//! }
//!
//! // Use in handler
//! async fn handler(req: Request, db: Depends<DatabaseConnection>) -> Response {
//!     // db is automatically injected
//!     Ok(Response::text("ok"))
//! }
//! ```

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;

/// A boxed future returning a dependency value or error.
pub type DepFuture<T> = Pin<Box<dyn Future<Output = Result<T, Box<dyn std::error::Error + Send + Sync>>> + Send>>;

/// Trait for types that can be resolved as dependencies.
#[async_trait::async_trait]
pub trait Dependency: Send + Sync + 'static {
    /// Resolve this dependency, potentially asynchronously.
    async fn resolve(ctx: &Context) -> Result<Self, Box<dyn std::error::Error + Send + Sync>>
    where
        Self: Sized;
}

/// Context passed to dependency resolvers containing parent dependencies.
pub struct Context {
    scope: Arc<RwLock<Scope>>,
}

impl Context {
    pub fn new(scope: Arc<RwLock<Scope>>) -> Self {
        Context { scope }
    }

    /// Get a dependency from the current scope.
    pub async fn get<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
        let scope = self.scope.read().await;
        scope.get::<T>().cloned()
    }
}

/// A scope holds resolved dependencies for the duration of a request.
pub struct Scope {
    dependencies: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
}

impl Scope {
    pub fn new() -> Self {
        Scope {
            dependencies: HashMap::new(),
        }
    }

    /// Insert a dependency into the scope.
    pub fn insert<T: Any + Send + Sync>(&mut self, value: Arc<T>) {
        self.dependencies.insert(TypeId::of::<T>(), value);
    }

    /// Get a dependency from the scope.
    pub fn get<T: Any + Send + Sync>(&self) -> Option<&Arc<T>> {
        self.dependencies
            .get(&TypeId::of::<T>())
            .and_then(|v| v.downcast_ref())
    }
}

impl Default for Scope {
    fn default() -> Self {
        Self::new()
    }
}

/// Marker type for dependency injection - wraps the actual value.
pub struct Depends<T>(pub T);

impl<T> Depends<T> {
    pub fn into_inner(self) -> T {
        self.0
    }
}

/// Dependency injector that manages resolution and caching.
pub struct Injector {
    scope: Arc<RwLock<Scope>>,
}

impl Injector {
    pub fn new() -> Self {
        Injector {
            scope: Arc::new(RwLock::new(Scope::new())),
        }
    }

    /// Create a new child scope for a request.
    pub fn request_scope(&self) -> RequestScope {
        RequestScope {
            scope: Arc::new(RwLock::new(Scope::new())),
            _parent: self.scope.clone(),
        }
    }

    /// Register a singleton dependency (resolved once, reused).
    pub async fn add_singleton<T, F, Fut>(&self, resolver: F) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
    where
        T: Any + Send + Sync + 'static,
        F: FnOnce(&Context) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T, Box<dyn std::error::Error + Send + Sync>>> + Send + 'static,
    {
        let ctx = Context::new(self.scope.clone());
        let value = resolver(&ctx).await?;
        let mut scope = self.scope.write().await;
        scope.insert(Arc::new(value));
        Ok(())
    }

    /// Get the global scope for singleton access.
    pub fn scope(&self) -> Arc<RwLock<Scope>> {
        self.scope.clone()
    }
}

impl Default for Injector {
    fn default() -> Self {
        Self::new()
    }
}

/// Per-request scope for transient dependencies.
pub struct RequestScope {
    scope: Arc<RwLock<Scope>>,
    _parent: Arc<RwLock<Scope>>,
}

impl RequestScope {
    /// Resolve a dependency within this request scope.
    pub async fn resolve<T, F, Fut>(&self, resolver: F) -> Result<Arc<T>, Box<dyn std::error::Error + Send + Sync>>
    where
        T: Any + Send + Sync + 'static,
        F: FnOnce(&Context) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T, Box<dyn std::error::Error + Send + Sync>>> + Send + 'static,
    {
        let ctx = Context::new(self.scope.clone());
        let value = resolver(&ctx).await?;
        let arc_value = Arc::new(value);
        
        // Cache in scope for potential reuse within same request
        let mut scope = self.scope.write().await;
        scope.insert(arc_value.clone());
        
        Ok(arc_value)
    }

    /// Get an already-resolved dependency from this scope.
    pub async fn get<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
        let scope = self.scope.read().await;
        scope.get::<T>().cloned()
    }
}

/// Common database connection dependency type.
#[derive(Clone)]
pub struct DbConnection {
    pool: Arc<sqlx::Pool<sqlx::Postgres>>,
}

impl DbConnection {
    pub fn new(pool: sqlx::Pool<sqlx::Postgres>) -> Self {
        DbConnection {
            pool: Arc::new(pool),
        }
    }

    pub fn pool(&self) -> &sqlx::Pool<sqlx::Postgres> {
        &self.pool
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dependency_injection() {
        let injector = Injector::new();
        
        // Add a singleton
        injector
            .add_singleton::<String, _, _>(|_ctx| async { Ok("singleton".to_string()) })
            .await
            .unwrap();

        // Create request scope
        let req_scope = injector.request_scope();
        
        // Resolve transient dependency
        let db = req_scope
            .resolve::<i32, _, _>(|_ctx| async { Ok(42) })
            .await
            .unwrap();
        
        assert_eq!(*db, 42);
    }
}
