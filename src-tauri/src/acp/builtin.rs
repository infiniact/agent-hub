use std::path::PathBuf;

use crate::models::agent::DiscoveredAgent;

use super::discovery::{get_adapters_dir, get_enriched_path};

// ---------------------------------------------------------------------------
// Built-in agent constants
// ---------------------------------------------------------------------------

const BUILTIN_PACKAGE_JSON: &str = r#"{
  "name": "claude-code-acp-builtin",
  "version": "0.16.1",
  "private": true,
  "dependencies": {
    "@agentclientprotocol/sdk": "0.14.1",
    "@anthropic-ai/claude-agent-sdk": "latest",
    "@modelcontextprotocol/sdk": "1.26.0",
    "diff": "8.0.3",
    "minimatch": "10.1.2"
  },
  "overrides": {
    "@anthropic-ai/claude-agent-sdk": "latest"
  }
}"#;

const BUILTIN_AGENT_ID: &str = "claude-code-acp";
const BUILTIN_AGENT_NAME: &str = "Claude Code";
const BUILTIN_BIN_NAME: &str = "claude-code-acp";
const BUILTIN_DESCRIPTION: &str = "Claude Code via ACP (built-in)";

// ---------------------------------------------------------------------------
// Embedded adapter JS files (from @zed-industries/claude-code-acp@0.16.1)
// ---------------------------------------------------------------------------

const ADAPTER_INDEX_JS: &str = include_str!("../../resources/claude-code-acp/index.js");
const ADAPTER_ACP_AGENT_JS: &str = include_str!("../../resources/claude-code-acp/acp-agent.js");
const ADAPTER_MCP_SERVER_JS: &str = include_str!("../../resources/claude-code-acp/mcp-server.js");
const ADAPTER_SETTINGS_JS: &str = include_str!("../../resources/claude-code-acp/settings.js");
const ADAPTER_TOOLS_JS: &str = include_str!("../../resources/claude-code-acp/tools.js");
const ADAPTER_UTILS_JS: &str = include_str!("../../resources/claude-code-acp/utils.js");
const ADAPTER_LIB_JS: &str = include_str!("../../resources/claude-code-acp/lib.js");

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Deploy the built-in agent to `~/.iaagenthub/adapters/claude-code-acp/`.
///
/// Writes embedded JS files to `dist/`, a template `package.json` (without the
/// adapter package itself), and runs `npm install` to fetch only the runtime
/// dependencies. This eliminates nested `node_modules` version conflicts.
///
/// Failures are logged as warnings and do **not** block application startup.
pub async fn ensure_builtin_deployed() {
    let adapter_dir = get_builtin_adapter_dir();

    if let Err(e) = deploy_builtin(&adapter_dir).await {
        log::warn!("Built-in agent deployment failed (non-fatal): {}", e);
    }
}

/// Return a `DiscoveredAgent` for the built-in agent.
///
/// `available` is `true` when `node_modules/.bin/claude-code-acp` exists on
/// disk; otherwise the agent is returned with `available = false` (the
/// frontend can show it as installable).
pub fn get_builtin_agent() -> DiscoveredAgent {
    let adapter_dir = get_builtin_adapter_dir();
    let local_bin = adapter_dir
        .join("node_modules")
        .join(".bin")
        .join(BUILTIN_BIN_NAME);
    let available = local_bin.exists();

    let source_path = if available {
        local_bin.to_string_lossy().to_string()
    } else {
        format!("installable:builtin:{}", BUILTIN_AGENT_ID)
    };

    // Parse adapter version from the embedded package.json
    let adapter_version = serde_json::from_str::<serde_json::Value>(BUILTIN_PACKAGE_JSON)
        .ok()
        .and_then(|v| v.get("version")?.as_str().map(|s| s.to_string()));

    let cli_version = get_cli_version();

    DiscoveredAgent {
        id: uuid::Uuid::new_v4().to_string(),
        name: BUILTIN_AGENT_NAME.to_string(),
        command: BUILTIN_BIN_NAME.to_string(),
        args_json: "[]".to_string(),
        env_json: "{}".to_string(),
        source_path,
        last_seen_at: chrono::Utc::now()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string(),
        available,
        models: Vec::new(),
        registry_id: Some(BUILTIN_AGENT_ID.to_string()),
        icon_url: None,
        description: BUILTIN_DESCRIPTION.to_string(),
        adapter_version,
        cli_version,
    }
}

/// Check whether a registry entry ID corresponds to the built-in agent.
pub fn is_builtin_agent(registry_id: &str) -> bool {
    registry_id == BUILTIN_AGENT_ID
}

/// Read the CLI version from the on-disk `cli.js` header.
///
/// The SDK bundles a `cli.js` whose fourth line looks like `// Version: 2.1.39`.
/// Returns `None` if the file doesn't exist or the header can't be parsed.
pub fn get_cli_version() -> Option<String> {
    let cli_js = get_builtin_adapter_dir()
        .join("node_modules/@anthropic-ai/claude-agent-sdk/cli.js");
    let content = std::fs::read_to_string(&cli_js).ok()?;
    for line in content.lines().take(10) {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("// Version:") {
            let ver = rest.trim();
            if !ver.is_empty() {
                return Some(ver.to_string());
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn get_builtin_adapter_dir() -> PathBuf {
    get_adapters_dir().join(BUILTIN_AGENT_ID)
}

/// All embedded JS files with their filenames.
fn embedded_js_files() -> Vec<(&'static str, &'static str)> {
    vec![
        ("index.js", ADAPTER_INDEX_JS),
        ("acp-agent.js", ADAPTER_ACP_AGENT_JS),
        ("mcp-server.js", ADAPTER_MCP_SERVER_JS),
        ("settings.js", ADAPTER_SETTINGS_JS),
        ("tools.js", ADAPTER_TOOLS_JS),
        ("utils.js", ADAPTER_UTILS_JS),
        ("lib.js", ADAPTER_LIB_JS),
    ]
}

async fn deploy_builtin(adapter_dir: &PathBuf) -> Result<(), String> {
    // 1. Create dist/ directory
    let dist_dir = adapter_dir.join("dist");
    tokio::fs::create_dir_all(&dist_dir)
        .await
        .map_err(|e| format!("Failed to create dist dir: {e}"))?;

    // 2. Write all embedded JS files into dist/
    for (filename, content) in embedded_js_files() {
        let path = dist_dir.join(filename);
        tokio::fs::write(&path, content)
            .await
            .map_err(|e| format!("Failed to write {filename}: {e}"))?;
    }

    // 3. chmod +x dist/index.js (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let index_path = dist_dir.join("index.js");
        let perms = std::fs::Permissions::from_mode(0o755);
        tokio::fs::set_permissions(&index_path, perms)
            .await
            .map_err(|e| format!("Failed to chmod index.js: {e}"))?;
    }

    // 4. Write package.json
    let pkg_path = adapter_dir.join("package.json");
    tokio::fs::write(&pkg_path, BUILTIN_PACKAGE_JSON)
        .await
        .map_err(|e| format!("Failed to write package.json: {e}"))?;

    // 5. Delete package-lock.json and node_modules to force a clean install.
    //    This ensures no stale packages (e.g. a previously npm-installed
    //    @zed-industries/claude-code-acp with a nested old SDK) survive.
    let lock_path = adapter_dir.join("package-lock.json");
    if lock_path.exists() {
        let _ = tokio::fs::remove_file(&lock_path).await;
    }
    let nm_dir = adapter_dir.join("node_modules");
    if nm_dir.exists() {
        let _ = tokio::fs::remove_dir_all(&nm_dir).await;
        log::info!("Removed old node_modules for clean install");
    }

    // 6. Run npm install
    let enriched_path = get_enriched_path();
    let npm_path = resolve_npm(&enriched_path)?;

    log::info!(
        "Built-in agent: running npm install in {:?}",
        adapter_dir
    );

    let output = tokio::process::Command::new(&npm_path)
        .arg("install")
        .current_dir(adapter_dir)
        .env("PATH", &enriched_path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await
        .map_err(|e| format!("npm install spawn error: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("npm install failed: {}", stderr.trim()));
    }

    // Log installed SDK version
    let sdk_pkg = adapter_dir.join("node_modules/@anthropic-ai/claude-agent-sdk/package.json");
    if let Ok(content) = tokio::fs::read_to_string(&sdk_pkg).await {
        if let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&content) {
            let ver = pkg.get("version").and_then(|v| v.as_str()).unwrap_or("?");
            log::info!("Built-in agent: claude-agent-sdk version = {}", ver);
        }
    }

    // 7. Create symlink: node_modules/.bin/claude-code-acp -> ../../dist/index.js
    let bin_dir = adapter_dir.join("node_modules/.bin");
    tokio::fs::create_dir_all(&bin_dir)
        .await
        .map_err(|e| format!("Failed to create .bin dir: {e}"))?;

    let symlink_path = bin_dir.join(BUILTIN_BIN_NAME);
    // Remove existing symlink/file if present
    let _ = tokio::fs::remove_file(&symlink_path).await;

    #[cfg(unix)]
    {
        tokio::fs::symlink("../../dist/index.js", &symlink_path)
            .await
            .map_err(|e| format!("Failed to create symlink: {e}"))?;
    }
    #[cfg(windows)]
    {
        tokio::fs::symlink_file("../../dist/index.js", &symlink_path)
            .await
            .map_err(|e| format!("Failed to create symlink: {e}"))?;
    }

    log::info!("Built-in agent deployed successfully (embedded adapter)");
    Ok(())
}

fn resolve_npm(enriched_path: &str) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    let lookup = "where.exe";
    #[cfg(not(target_os = "windows"))]
    let lookup = "which";

    let output = std::process::Command::new(lookup)
        .arg("npm")
        .env("PATH", enriched_path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("Failed to locate npm: {e}"))?;

    if output.status.success() {
        let s = String::from_utf8_lossy(&output.stdout);
        let first = s.lines().next().unwrap_or("").trim().to_string();
        if first.is_empty() {
            Err("npm not found on PATH".to_string())
        } else {
            Ok(first)
        }
    } else {
        Err("npm not found on PATH".to_string())
    }
}
