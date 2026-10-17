use crate::policy_assets::ToolCatalog;
use crate::providers::{ProviderError, ProviderRegistry};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use thiserror::Error;
use url::{Host, Url};
use uuid::Uuid;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AuthorizationError {
    #[error("Unknown tool: {0}")]
    UnknownTool(String),
    #[error("Tool arguments are not valid JSON: {0}")]
    InvalidArguments(String),
    #[error("Tool arguments must be a JSON object")]
    ArgumentsNotObject,
    #[error("web_fetch requires a string url argument")]
    MissingUrl,
    #[error("Invalid url: {0}")]
    InvalidUrl(String),
    #[error("Unsupported url scheme: {0}")]
    UnsupportedScheme(String),
    #[error("The url has no host")]
    MissingHost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationRequest {
    pub chat_id: Uuid,
    pub call_id: String,
    pub tool_name: String,
    pub arguments: Value,
    pub target_host: Option<String>,
    pub target_path: Option<String>,
    pub target_dir: Option<String>,
    pub facts: BTreeMap<String, String>,
    pub summary: Option<String>,
    pub approval_tier: String,
    pub argument_digest: String,
}

const WEB_FETCH_TOOL: &str = "nook:web_fetch";

/// Tools the Worker runs itself. Every other tool needs a Daemon provider.
pub const WORKER_TOOLS: &[&str] = &["nook:web_search", "nook:web_fetch"];

impl AuthorizationRequest {
    pub fn normalize(
        catalog: &ToolCatalog,
        chat_id: Uuid,
        call_id: &str,
        tool_name: &str,
        arguments_json: &str,
    ) -> Result<Self, AuthorizationError> {
        Self::normalize_with(catalog, None, chat_id, call_id, tool_name, arguments_json)
    }

    pub fn normalize_with(
        catalog: &ToolCatalog,
        providers: Option<&ProviderRegistry>,
        chat_id: Uuid,
        call_id: &str,
        tool_name: &str,
        arguments_json: &str,
    ) -> Result<Self, AuthorizationError> {
        let spec = catalog
            .resolve(tool_name)
            .ok_or_else(|| AuthorizationError::UnknownTool(tool_name.to_string()))?;
        let arguments: Value = serde_json::from_str(arguments_json)
            .map_err(|e| AuthorizationError::InvalidArguments(e.to_string()))?;
        if !arguments.is_object() {
            return Err(AuthorizationError::ArgumentsNotObject);
        }
        let argument_digest = digest_arguments(&arguments);
        let mut facts = BTreeMap::new();
        let mut target_host = None;
        let mut target_path = None;
        let mut target_dir = None;
        let mut summary = None;
        if let Some(registry) = providers.filter(|registry| registry.contains(tool_name)) {
            let tool_facts = registry.facts(tool_name, &arguments).map_err(|e| match e {
                ProviderError::InvalidArguments(message) => {
                    AuthorizationError::InvalidArguments(message)
                }
                ProviderError::UnknownTool => {
                    AuthorizationError::UnknownTool(tool_name.to_string())
                }
            })?;
            facts = tool_facts.context;
            target_host = tool_facts.target_host;
            target_path = tool_facts.target_path;
            target_dir = tool_facts.target_dir;
            summary = Some(tool_facts.summary);
        } else if tool_name == WEB_FETCH_TOOL {
            let host = fetch_target_host(&arguments)?;
            facts.insert("host".to_string(), host.clone());
            facts.insert(
                "url".to_string(),
                arguments
                    .get("url")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            );
            target_host = Some(host);
        } else if !WORKER_TOOLS.contains(&tool_name) {
            return Err(AuthorizationError::UnknownTool(tool_name.to_string()));
        }
        if tool_name == WEB_FETCH_TOOL || target_host.is_some() || !facts.is_empty() {
            facts.insert("argument_digest".to_string(), argument_digest.clone());
        }
        Ok(Self {
            chat_id,
            call_id: call_id.to_string(),
            tool_name: tool_name.to_string(),
            arguments,
            target_host,
            target_path,
            target_dir,
            facts,
            summary,
            approval_tier: spec.approval.tier.clone(),
            argument_digest,
        })
    }

    pub fn url(&self) -> Option<&str> {
        self.arguments.get("url").and_then(Value::as_str)
    }

    pub fn resource_summary(&self) -> String {
        if let Some(summary) = &self.summary {
            return summary.clone();
        }
        if let (Some(url), Some(host)) = (self.url(), self.target_host.as_deref()) {
            return format!("{url} ({host})");
        }
        if let Some(query) = self.arguments.get("query").and_then(Value::as_str) {
            return query.to_string();
        }
        self.arguments.to_string()
    }
}

fn fetch_target_host(arguments: &Value) -> Result<String, AuthorizationError> {
    let raw = arguments
        .get("url")
        .and_then(Value::as_str)
        .ok_or(AuthorizationError::MissingUrl)?;
    if raw
        .chars()
        .any(|c| c == '\\' || c.is_control() || c.is_whitespace())
    {
        return Err(AuthorizationError::InvalidUrl(
            "url contains a backslash, space or control character".to_string(),
        ));
    }
    let url = Url::parse(raw).map_err(|e| AuthorizationError::InvalidUrl(e.to_string()))?;
    if !url.username().is_empty() || url.password().is_some() {
        return Err(AuthorizationError::InvalidUrl(
            "url must not contain a username or password".to_string(),
        ));
    }
    if !matches!(url.scheme(), "http" | "https") {
        return Err(AuthorizationError::UnsupportedScheme(
            url.scheme().to_string(),
        ));
    }
    match url.host().ok_or(AuthorizationError::MissingHost)? {
        Host::Domain(domain) => normalize_domain(domain),
        Host::Ipv4(ip) => {
            if authority_host(raw) != ip.to_string() {
                return Err(AuthorizationError::InvalidUrl(
                    "an IP address must be written as four decimal numbers".to_string(),
                ));
            }
            Ok(ip.to_string())
        }
        Host::Ipv6(ip) => Ok(format!("[{ip}]")),
    }
}

fn authority_host(raw: &str) -> &str {
    let rest = raw.split_once("://").map_or(raw, |(_, rest)| rest);
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..end];
    authority
        .rsplit_once(':')
        .filter(|(_, port)| port.chars().all(|c| c.is_ascii_digit()))
        .map_or(authority, |(host, _)| host)
}

fn normalize_domain(domain: &str) -> Result<String, AuthorizationError> {
    let cleaned = domain.trim().to_lowercase();
    let cleaned = cleaned.trim_end_matches('.');
    idna::domain_to_ascii(cleaned)
        .map_err(|e| AuthorizationError::InvalidUrl(format!("host is not valid: {e}")))
}

fn digest_arguments(arguments: &Value) -> String {
    let mut canonical = String::new();
    write_canonical(arguments, &mut canonical);
    format!("{:x}", Sha256::digest(canonical.as_bytes()))
}

fn write_canonical(value: &Value, out: &mut String) {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<_> = map.iter().collect();
            entries.sort_by(|a, b| a.0.cmp(b.0));
            out.push('{');
            for (i, (key, item)) in entries.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String(key.clone()).to_string());
                out.push(':');
                write_canonical(item, out);
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_canonical(item, out);
            }
            out.push(']');
        }
        other => out.push_str(&other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOOLS: &str = include_str!("../../../../policy/tools.yaml");

    fn catalog() -> ToolCatalog {
        ToolCatalog::parse(TOOLS).unwrap()
    }

    fn fetch(args: &str) -> Result<AuthorizationRequest, AuthorizationError> {
        AuthorizationRequest::normalize(
            &catalog(),
            Uuid::new_v4(),
            "call-1",
            "nook:web_fetch",
            args,
        )
    }

    fn host_of(url: &str) -> Option<String> {
        let args = serde_json::json!({ "url": url }).to_string();
        fetch(&args).unwrap().target_host
    }

    #[test]
    fn fetch_request_carries_catalog_tier_and_fields() {
        let chat_id = Uuid::new_v4();
        let req = AuthorizationRequest::normalize(
            &catalog(),
            chat_id,
            "call-9",
            "nook:web_fetch",
            r#"{"url":"https://example.com/a"}"#,
        )
        .unwrap();
        assert_eq!(req.chat_id, chat_id);
        assert_eq!(req.call_id, "call-9");
        assert_eq!(req.tool_name, "nook:web_fetch");
        assert_eq!(req.approval_tier, "ask_once_per_host");
        assert_eq!(req.arguments["url"], "https://example.com/a");
        assert_eq!(req.target_host.as_deref(), Some("example.com"));
    }

    #[test]
    fn non_fetch_tool_has_no_target_host() {
        let req = AuthorizationRequest::normalize(
            &catalog(),
            Uuid::new_v4(),
            "c",
            "nook:web_search",
            r#"{"query":"rust"}"#,
        )
        .unwrap();
        assert_eq!(req.target_host, None);
        assert_eq!(req.approval_tier, "allow");
    }

    #[test]
    fn host_is_lowercased() {
        assert_eq!(
            host_of("https://DOCS.Example.COM/x").as_deref(),
            Some("docs.example.com")
        );
    }

    #[test]
    fn host_trailing_dot_is_stripped() {
        assert_eq!(
            host_of("https://example.com./x").as_deref(),
            Some("example.com")
        );
    }

    #[test]
    fn host_is_idna_ascii() {
        assert_eq!(
            host_of("https://Bücher.example./x").as_deref(),
            Some("xn--bcher-kva.example")
        );
    }

    #[test]
    fn domain_with_an_invalid_label_is_rejected() {
        for domain in ["xn--a-.example", "xn--zz.example", "xn--.example"] {
            assert!(
                matches!(
                    normalize_domain(domain),
                    Err(AuthorizationError::InvalidUrl(_))
                ),
                "{domain}"
            );
        }
    }

    #[test]
    fn url_with_an_invalid_label_is_rejected() {
        let args = serde_json::json!({ "url": "https://xn--a-.example/" }).to_string();
        assert!(matches!(
            fetch(&args),
            Err(AuthorizationError::InvalidUrl(_))
        ));
    }

    #[test]
    fn urls_that_parsers_read_differently_are_rejected() {
        for url in [
            "http://good.com\\@evil.com/",
            "http://good.com:80@evil.com/",
            "http://user@good.com/",
            "http://user:secret@good.com/",
            "http://good.com\t.evil.com/",
            "http://good.com\n.evil.com/",
            "http://good.com /",
        ] {
            let args = serde_json::json!({ "url": url }).to_string();
            assert!(
                matches!(fetch(&args), Err(AuthorizationError::InvalidUrl(_))),
                "{url:?}"
            );
        }
    }

    #[test]
    fn hosts_match_the_vectors_shared_with_the_worker() {
        let vectors: Value =
            serde_json::from_str(include_str!("../../../../contracts/host-vectors.json")).unwrap();
        for case in vectors["accepted"].as_array().unwrap() {
            let url = case["url"].as_str().unwrap();
            assert_eq!(host_of(url).as_deref(), case["host"].as_str(), "{url:?}");
        }
        for url in vectors["rejected"].as_array().unwrap() {
            let args = serde_json::json!({ "url": url }).to_string();
            assert!(fetch(&args).is_err(), "{url:?}");
        }
    }

    #[test]
    fn port_is_ignored() {
        assert_eq!(
            host_of("http://example.com:8080/x").as_deref(),
            Some("example.com")
        );
    }

    #[test]
    fn ip_hosts_are_kept() {
        assert_eq!(
            host_of("http://127.0.0.1:3000/").as_deref(),
            Some("127.0.0.1")
        );
        assert_eq!(host_of("http://[::1]:3000/").as_deref(), Some("[::1]"));
    }

    #[test]
    fn non_http_scheme_is_rejected() {
        assert_eq!(
            fetch(r#"{"url":"file:///etc/passwd"}"#).unwrap_err(),
            AuthorizationError::UnsupportedScheme("file".to_string())
        );
        assert!(matches!(
            fetch(r#"{"url":"ftp://example.com/"}"#),
            Err(AuthorizationError::UnsupportedScheme(_))
        ));
    }

    #[test]
    fn missing_url_is_rejected() {
        assert_eq!(fetch("{}").unwrap_err(), AuthorizationError::MissingUrl);
    }

    #[test]
    fn non_string_url_is_rejected() {
        assert_eq!(
            fetch(r#"{"url":5}"#).unwrap_err(),
            AuthorizationError::MissingUrl
        );
    }

    #[test]
    fn unparsable_url_is_rejected() {
        assert!(matches!(
            fetch(r#"{"url":"not a url"}"#),
            Err(AuthorizationError::InvalidUrl(_))
        ));
    }

    #[test]
    fn non_object_arguments_are_rejected() {
        assert_eq!(
            fetch("[1,2]").unwrap_err(),
            AuthorizationError::ArgumentsNotObject
        );
        assert_eq!(
            fetch("\"x\"").unwrap_err(),
            AuthorizationError::ArgumentsNotObject
        );
        assert_eq!(
            fetch("null").unwrap_err(),
            AuthorizationError::ArgumentsNotObject
        );
    }

    #[test]
    fn invalid_json_arguments_are_rejected() {
        assert!(matches!(
            fetch("{"),
            Err(AuthorizationError::InvalidArguments(_))
        ));
    }

    #[test]
    fn unknown_tool_is_rejected() {
        let err =
            AuthorizationRequest::normalize(&catalog(), Uuid::new_v4(), "c", "nook:nope", "{}")
                .unwrap_err();
        assert_eq!(
            err,
            AuthorizationError::UnknownTool("nook:nope".to_string())
        );
    }

    #[test]
    fn digest_is_stable_across_key_order() {
        let a =
            fetch(r#"{"url":"https://example.com","x":{"b":1,"a":[2,{"d":1,"c":2}]}}"#).unwrap();
        let b =
            fetch(r#"{"x":{"a":[2,{"c":2,"d":1}],"b":1},"url":"https://example.com"}"#).unwrap();
        assert_eq!(a.argument_digest, b.argument_digest);
    }

    #[test]
    fn digest_differs_for_different_values() {
        let a = fetch(r#"{"url":"https://example.com/a"}"#).unwrap();
        let b = fetch(r#"{"url":"https://example.com/b"}"#).unwrap();
        assert_ne!(a.argument_digest, b.argument_digest);
    }

    #[test]
    fn digest_is_sha256_of_canonical_json() {
        let req = fetch(r#"{ "url" : "https://example.com" }"#).unwrap();
        assert_eq!(
            req.argument_digest,
            "5dc5c505a79bfc2eb22d0e45eff415c6ecf0c965c3d53d6e3e02c1bda74b0927"
        );
    }

    #[test]
    fn fetch_resource_summary_shows_url_and_host() {
        let req = fetch(r#"{"url":"https://Docs.Example.com/a?b=1"}"#).unwrap();
        assert_eq!(
            req.resource_summary(),
            "https://Docs.Example.com/a?b=1 (docs.example.com)"
        );
    }

    #[test]
    fn search_resource_summary_is_the_query() {
        let req = AuthorizationRequest::normalize(
            &catalog(),
            Uuid::new_v4(),
            "c",
            "nook:web_search",
            r#"{"query":"rust cedar"}"#,
        )
        .unwrap();
        assert_eq!(req.resource_summary(), "rust cedar");
    }

    #[test]
    fn search_without_query_summarizes_the_arguments() {
        let req = AuthorizationRequest::normalize(
            &catalog(),
            Uuid::new_v4(),
            "c",
            "nook:web_search",
            r#"{"q":1}"#,
        )
        .unwrap();
        assert_eq!(req.resource_summary(), r#"{"q":1}"#);
    }
}
