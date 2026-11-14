#![cfg(unix)]

use nook_core::config::Config;
use nook_core::providers::mcp::{check_server_settings, valid_server_name};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn find_mcp_servers_doc() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let p3 = manifest.join("../../../docs/mcp-servers.md");
    if p3.exists() {
        return p3;
    }
    let p2 = manifest.join("../../docs/mcp-servers.md");
    if p2.exists() {
        return p2;
    }
    panic!(
        "could not find docs/mcp-servers.md from CARGO_MANIFEST_DIR: {}",
        manifest.display()
    );
}

fn extract_google_docs_toml_block(doc_content: &str) -> String {
    let heading = "## Example: Google Docs";
    let heading_pos = doc_content
        .find(heading)
        .unwrap_or_else(|| panic!("heading '{heading}' not found in docs/mcp-servers.md"));
    let after_heading = &doc_content[heading_pos + heading.len()..];
    let fence_start = "```toml";
    let fence_start_pos = after_heading
        .find(fence_start)
        .unwrap_or_else(|| panic!("fenced toml block not found under '{heading}'"));
    let content_start = fence_start_pos + fence_start.len();
    let fence_end = "```";
    let fence_end_pos = after_heading[content_start..]
        .find(fence_end)
        .unwrap_or_else(|| panic!("closing fence for toml block not found"));
    after_heading[content_start..content_start + fence_end_pos]
        .trim()
        .to_string()
}

#[test]
fn the_google_docs_sample_block_loads_as_a_valid_server() {
    let doc_path = find_mcp_servers_doc();
    let content = std::fs::read_to_string(&doc_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", doc_path.display()));

    let toml_block = extract_google_docs_toml_block(&content);
    let wrapped = format!("base_url = \"u\"\nmodel = \"m\"\n{toml_block}\n");

    let config: Config = toml::from_str(&wrapped)
        .unwrap_or_else(|e| panic!("failed to parse sample block as Config: {e}"));

    assert_eq!(
        config.tools.mcp_servers.len(),
        1,
        "expected exactly one MCP server in the sample block"
    );
    let server = &config.tools.mcp_servers[0];

    assert!(
        valid_server_name(&server.name),
        "server name {:?} must pass valid_server_name",
        server.name
    );
    assert!(server.network, "network must be true");

    for secret_key in server.env_secrets.keys() {
        assert!(
            !server.env.contains_key(secret_key),
            "secret key {secret_key} must not appear in env"
        );
    }

    for key in server.env.keys().chain(server.env_secrets.keys()) {
        assert!(
            !key.starts_with("LD_") && !key.starts_with("DYLD_"),
            "key {key} must not start with LD_ or DYLD_"
        );
    }

    check_server_settings(server).expect("check_server_settings must accept the server config");

    let expected_env_keys: BTreeSet<&str> =
        ["GOOGLE_DRIVE_MCP_CLIENT_ID", "GOOGLE_DRIVE_MCP_SCOPES"]
            .into_iter()
            .collect();
    let actual_env_keys: BTreeSet<&str> = server.env.keys().map(String::as_str).collect();
    assert_eq!(
        actual_env_keys, expected_env_keys,
        "env keys must match expected sample keys"
    );

    let expected_secret_keys: BTreeSet<&str> = [
        "GOOGLE_DRIVE_MCP_CLIENT_SECRET",
        "GOOGLE_DRIVE_MCP_REFRESH_TOKEN",
    ]
    .into_iter()
    .collect();
    let actual_secret_keys: BTreeSet<&str> =
        server.env_secrets.keys().map(String::as_str).collect();
    assert_eq!(
        actual_secret_keys, expected_secret_keys,
        "env_secrets keys must match expected sample keys"
    );

    assert!(
        Path::new(&server.command).is_absolute(),
        "command path {:?} must be absolute",
        server.command
    );
    assert!(
        !server.args.is_empty(),
        "args must have at least one element for script path"
    );
    assert!(
        Path::new(&server.args[0]).is_absolute(),
        "args[0] path {:?} must be absolute",
        server.args[0]
    );

    for path in &server.read_paths {
        assert!(
            Path::new(path).is_absolute(),
            "read_path {:?} must be absolute",
            path
        );
    }
    for path in &server.write_paths {
        assert!(
            Path::new(path).is_absolute(),
            "write_path {:?} must be absolute",
            path
        );
    }
}
