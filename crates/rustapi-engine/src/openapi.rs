//! OpenAPI 3.1 Schema Generator with automatic documentation.
//!
//! This module provides automatic OpenAPI schema generation from route definitions,
//! compatible with FastAPI's approach. Supports Pydantic models, response schemas,
//! and interactive Swagger UI / ReDoc endpoints.
//!
//! # Example
//! ```rust
//! use rustapi_engine::openapi::{OpenApiSpec, Info, PathItem};
//!
//! let mut spec = OpenApiSpec::new(Info {
//!     title: "My API".to_string(),
//!     version: "1.0.0".to_string(),
//!     ..Default::default()
//! });
//!
//! // Routes automatically register their schemas
//! spec.add_route("/users/{id}", Method::GET, Operation {
//!     summary: Some("Get user by ID".to_string()),
//!     responses: Responses::default(),
//! });
//! ```

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use utoipa::OpenApi;

/// OpenAPI specification builder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenApiSpec {
    pub openapi: String,
    pub info: Info,
    pub servers: Vec<Server>,
    pub paths: BTreeMap<String, PathItem>,
    pub components: Components,
}

impl OpenApiSpec {
    pub fn new(info: Info) -> Self {
        OpenApiSpec {
            openapi: "3.1.0".to_string(),
            info,
            servers: vec![],
            paths: BTreeMap::new(),
            components: Components::default(),
        }
    }

    /// Add a route to the OpenAPI spec.
    pub fn add_route(&mut self, path: &str, method: http::Method, operation: Operation) {
        let path_item = self.paths.entry(path.to_string()).or_insert_with(PathItem::default);
        
        match method.as_str() {
            "GET" => path_item.get = Some(operation),
            "POST" => path_item.post = Some(operation),
            "PUT" => path_item.put = Some(operation),
            "DELETE" => path_item.delete = Some(operation),
            "PATCH" => path_item.patch = Some(operation),
            "HEAD" => path_item.head = Some(operation),
            "OPTIONS" => path_item.options = Some(operation),
            _ => {}
        }
    }

    /// Generate JSON representation of the spec.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Generate HTML for Swagger UI.
    pub fn swagger_ui_html(&self, spec_url: &str) -> String {
        format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <title>{title} - Swagger UI</title>
    <meta charset="utf-8"/>
    <link rel="stylesheet" type="text/css" href="https://unpkg.com/swagger-ui-dist@5/swagger-ui.css">
</head>
<body>
    <div id="swagger-ui"></div>
    <script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
    <script>
        window.onload = function() {{
            SwaggerUIBundle({{
                url: "{spec_url}",
                dom_id: '#swagger-ui',
                presets: [SwaggerUIBundle.presets.apis, SwaggerUIBundle.SwaggerUIStandalonePreset],
                layout: "BaseLayout"
            }});
        }};
    </script>
</body>
</html>"#,
            title = self.info.title,
            spec_url = spec_url
        )
    }

    /// Generate HTML for ReDoc.
    pub fn redoc_html(&self, spec_url: &str) -> String {
        format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <title>{title} - ReDoc</title>
    <meta charset="utf-8"/>
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <link href="https://fonts.googleapis.com/css?family=Montserrat:300,400,700|Roboto:300,400,700" rel="stylesheet">
    <style>body {{ margin: 0; padding: 0; }}</style>
</head>
<body>
    <redoc spec-url="{spec_url}"></redoc>
    <script src="https://cdn.redoc.ly/redoc/latest/bundles/redoc.standalone.js"></script>
</body>
</html>"#,
            title = self.info.title,
            spec_url = spec_url
        )
    }
}

/// OpenAPI Info object.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Info {
    pub title: String,
    pub description: Option<String>,
    pub terms_of_service: Option<String>,
    pub contact: Option<Contact>,
    pub license: Option<License>,
    pub version: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Contact {
    pub name: Option<String>,
    pub url: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct License {
    pub name: String,
    pub url: Option<String>,
}

/// Server definition.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Server {
    pub url: String,
    pub description: Option<String>,
}

/// Path Item representing all operations for a path.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PathItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub get: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub put: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delete: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patch: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Operation>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<Parameter>,
}

/// Operation (HTTP method on a path).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Operation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<Parameter>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_body: Option<RequestBody>,
    pub responses: Responses,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<bool>,
}

/// Parameter definition (path, query, header, cookie).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    #[serde(rename = "in")]
    pub location: String, // path, query, header, cookie
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<Schema>,
}

/// Request body definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub content: BTreeMap<String, MediaType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
}

/// Media type (e.g., application/json).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaType {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<Schema>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<serde_json::Value>,
}

/// Response definitions keyed by status code.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Responses {
    #[serde(flatten)]
    pub responses: BTreeMap<String, Response>,
}

/// Single response definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<BTreeMap<String, MediaType>>,
}

/// Schema definition (JSON Schema subset).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub schema_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Box<Schema>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<BTreeMap<String, Schema>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<serde_json::Value>>,
}

/// Components section for reusable schemas.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Components {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub schemas: BTreeMap<String, Schema>,
}

/// Helper to build OpenAPI spec from router metadata.
pub struct OpenApiBuilder {
    spec: OpenApiSpec,
}

impl OpenApiBuilder {
    pub fn new(title: &str, version: &str) -> Self {
        OpenApiBuilder {
            spec: OpenApiSpec::new(Info {
                title: title.to_string(),
                version: version.to_string(),
                ..Default::default()
            }),
        }
    }

    pub fn description(mut self, desc: &str) -> Self {
        self.spec.info.description = Some(desc.to_string());
        self
    }

    pub fn server(mut self, url: &str, description: Option<&str>) -> Self {
        self.spec.servers.push(Server {
            url: url.to_string(),
            description: description.map(|s| s.to_string()),
        });
        self
    }

    pub fn build(self) -> OpenApiSpec {
        self.spec
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::Method;

    #[test]
    fn test_openapi_spec_generation() {
        let mut spec = OpenApiSpec::new(Info {
            title: "Test API".to_string(),
            version: "1.0.0".to_string(),
            ..Default::default()
        });

        spec.add_route(
            "/users/{id}",
            Method::GET,
            Operation {
                summary: Some("Get user by ID".to_string()),
                responses: Responses::default(),
                ..Default::default()
            },
        );

        assert!(spec.paths.contains_key("/users/{id}"));
        assert!(spec.paths.get("/users/{id}").unwrap().get.is_some());
    }

    #[test]
    fn test_swagger_ui_html() {
        let spec = OpenApiSpec::new(Info {
            title: "Test API".to_string(),
            version: "1.0.0".to_string(),
            ..Default::default()
        });

        let html = spec.swagger_ui_html("/openapi.json");
        assert!(html.contains("Swagger UI"));
        assert!(html.contains("/openapi.json"));
    }
}
