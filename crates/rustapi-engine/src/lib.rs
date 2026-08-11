//! rustapi-engine: the Rust-core HTTP engine with Django/FastAPI compatibility.
//!
//! This crate owns every hot path: HTTP parsing (hyper), routing (matchit),
//! and the per-connection tokio task. It is *Python-agnostic* at this layer —
//! the PyO3 bindings in `rustapi-py` register handlers as trait objects
//! implementing [`Handler`].
//!
//! Design rule: a request crosses the Python boundary exactly once (to invoke
//! the user's handler). Everything else — parsing, routing, header munging,
//! serialization of bytes — happens here.
//!
//! # Features
//! - **Dependency Injection**: FastAPI-compatible `Depends()` pattern
//! - **OpenAPI 3.1**: Automatic schema generation with Swagger UI / ReDoc
//! - **Async ORM**: Django-style query builder with sqlx backend
//! - **Migrations**: Automatic schema detection and application

pub mod handler;
pub mod request;
pub mod response;
pub mod router;
pub mod server;
pub mod di;
pub mod openapi;
pub mod orm;

pub use handler::{Handler, HandlerFuture, HandlerResult};
pub use request::Request;
pub use response::Response;
pub use router::{MatchResult, Router, RouterError};
pub use server::{Server, ServerConfig};
pub use di::{Depends, Injector, RequestScope, Scope, Context};
pub use openapi::{OpenApiSpec, Info, OpenApiBuilder};
pub use orm::{Model, QuerySet, Database, DatabasePool, Migrator, Migration};
