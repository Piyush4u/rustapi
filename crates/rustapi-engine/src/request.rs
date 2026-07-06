//! Request representation passed to handlers.
//!
//! Built once per HTTP request by the server, then handed off to the matched
//! handler. Designed so the Python bridge can construct Python objects from
//! it in a single FFI call — no per-header Python crossings.

use bytes::Bytes;
use http::HeaderMap;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub query_string: String,
    /// Parsed query parameters: `?a=1&b=2` -> `{"a": "1", "b": "2"}`.
    /// Multi-valued keys are joined by comma for simplicity in v0.1;
    /// a proper multi-map is on the roadmap.
    pub query_params: HashMap<String, String>,
    pub headers: HeaderMap,
    /// Path parameters extracted by the router, e.g. `/users/{id}` -> `{"id": "42"}`.
    pub path_params: HashMap<String, String>,
    pub body: Bytes,
    pub remote_addr: Option<std::net::SocketAddr>,
}

impl Request {
    /// Parse a query string into a flat map. Multi-valued keys are joined by `,`.
    pub fn parse_query(qs: &str) -> HashMap<String, String> {
        let mut out = HashMap::new();
        if qs.is_empty() {
            return out;
        }
        for pair in qs.split('&') {
            if pair.is_empty() {
                continue;
            }
            let (k, v) = match pair.split_once('=') {
                Some((k, v)) => (k, v),
                None => (pair, ""),
            };
            let k = urldecode(k);
            let v = urldecode(v);
            out.entry(k)
                .and_modify(|existing: &mut String| {
                    existing.push(',');
                    existing.push_str(&v);
                })
                .or_insert(v);
        }
        out
    }
}

/// Minimal percent-decoder. Allocates; fine for query strings (small).
fn urldecode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'+' {
            out.push(b' ');
            i += 1;
        } else if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
            out.push(bytes[i]);
            i += 1;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_query() {
        let q = Request::parse_query("a=1&b=2");
        assert_eq!(q.get("a").unwrap(), "1");
        assert_eq!(q.get("b").unwrap(), "2");
    }

    #[test]
    fn parses_urlencoded() {
        let q = Request::parse_query("name=Alice%20Smith&age=30");
        assert_eq!(q.get("name").unwrap(), "Alice Smith");
        assert_eq!(q.get("age").unwrap(), "30");
    }

    #[test]
    fn parses_plus_as_space() {
        let q = Request::parse_query("q=hello+world");
        assert_eq!(q.get("q").unwrap(), "hello world");
    }

    #[test]
    fn merges_multi_values() {
        let q = Request::parse_query("tag=a&tag=b&tag=c");
        assert_eq!(q.get("tag").unwrap(), "a,b,c");
    }
}
