//! Shared HTTP helpers: client construction and incremental line splitting for NDJSON/SSE streams.

use std::time::Duration;

use reqwest::{Client, Response};

use crate::error::ProviderError;

/// Build an HTTP client. Loopback servers never go through a proxy.
pub fn build_client(base_url: &str) -> Client {
    let mut b = Client::builder().connect_timeout(Duration::from_secs(3)).user_agent(concat!("PulseBench/", env!("CARGO_PKG_VERSION")));
    if is_loopback(base_url) {
        b = b.no_proxy();
    }
    b.build().expect("reqwest client builds")
}

pub fn is_loopback(url: &str) -> bool {
    let rest = url.split("://").nth(1).unwrap_or(url);
    let host = rest.split(['/', '?']).next().unwrap_or("");
    let host = if let Some(stripped) = host.strip_prefix('[') {
        stripped.split(']').next().unwrap_or("")
    } else {
        host.rsplit_once(':').map(|(h, _)| h).unwrap_or(host)
    };
    matches!(host, "localhost" | "127.0.0.1" | "::1")
}

pub fn join(base: &str, path: &str) -> String {
    format!("{}/{}", base.trim_end_matches('/'), path.trim_start_matches('/'))
}

pub fn map_reqwest(base_url: &str, e: reqwest::Error) -> ProviderError {
    if e.is_connect() {
        ProviderError::Unreachable { url: base_url.to_string(), detail: root_cause(&e) }
    } else if e.is_timeout() {
        ProviderError::Unreachable { url: base_url.to_string(), detail: "request timed out".into() }
    } else {
        ProviderError::Stream(root_cause(&e))
    }
}

fn root_cause(e: &dyn std::error::Error) -> String {
    let mut cur: &dyn std::error::Error = e;
    while let Some(next) = cur.source() {
        cur = next;
    }
    cur.to_string()
}

/// Turn a non-success response into a [`ProviderError`].
pub async fn error_from_response(model: &str, resp: Response) -> ProviderError {
    let status = resp.status().as_u16();
    let body = resp.text().await.unwrap_or_default();
    let lower = body.to_lowercase();
    if status == 404 || lower.contains("not found") && lower.contains("model") {
        return ProviderError::ModelNotFound(model.to_string());
    }
    if lower.contains("requires more system memory")
        || lower.contains("out of memory")
        || lower.contains("insufficient")
        || lower.contains("failed to load model")
    {
        return ProviderError::InsufficientMemory(extract_message(&body));
    }
    ProviderError::Http { status, body: extract_message(&body) }
}

fn extract_message(body: &str) -> String {
    let parsed: Option<serde_json::Value> = serde_json::from_str(body).ok();
    let msg = parsed
        .as_ref()
        .and_then(|v| {
            v.get("error")
                .and_then(|e| e.as_str().map(str::to_string).or_else(|| e.get("message").and_then(|m| m.as_str()).map(str::to_string)))
        })
        .unwrap_or_else(|| body.trim().to_string());
    msg.chars().take(500).collect()
}

/// Splits a byte stream into complete UTF-8 lines.
#[derive(Default)]
pub struct LineSplitter {
    buf: Vec<u8>,
}

impl LineSplitter {
    pub fn push(&mut self, chunk: &[u8]) -> Vec<String> {
        self.buf.extend_from_slice(chunk);
        let mut lines = Vec::new();
        while let Some(pos) = self.buf.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = self.buf.drain(..=pos).collect();
            let s = String::from_utf8_lossy(&line[..line.len() - 1]).trim_end_matches('\r').to_string();
            lines.push(s);
        }
        lines
    }

    /// Remaining partial line at end of stream.
    pub fn finish(&mut self) -> Option<String> {
        if self.buf.is_empty() {
            return None;
        }
        let s = String::from_utf8_lossy(&self.buf).trim().to_string();
        self.buf.clear();
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_lines_across_chunks_and_utf8_boundaries() {
        let mut s = LineSplitter::default();
        let bytes = "héllo\nwörld\n".as_bytes();
        let (a, b) = bytes.split_at(2); // splits inside 'é'
        assert!(s.push(a).is_empty());
        assert_eq!(s.push(b), vec!["héllo".to_string(), "wörld".to_string()]);
        assert_eq!(s.finish(), None);
    }

    #[test]
    fn keeps_trailing_partial_line() {
        let mut s = LineSplitter::default();
        assert_eq!(s.push(b"a\r\nb"), vec!["a".to_string()]);
        assert_eq!(s.finish(), Some("b".to_string()));
    }

    #[test]
    fn loopback_detection() {
        assert!(is_loopback("http://localhost:11434"));
        assert!(is_loopback("http://127.0.0.1:1234/v1"));
        assert!(is_loopback("http://[::1]:8080"));
        assert!(!is_loopback("https://openrouter.ai/api/v1"));
    }

    #[test]
    fn join_handles_slashes() {
        assert_eq!(join("http://x/v1/", "/models"), "http://x/v1/models");
    }
}
