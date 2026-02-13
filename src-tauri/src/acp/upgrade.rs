use crate::acp::discovery;
use crate::error::{AppError, AppResult};

/// Information extracted from a version-upgrade error message.
#[derive(Debug, Clone)]
pub struct UpgradeInfo {
    /// Full package specifier, e.g. `@anthropic-ai/claude-code@2.1.39`
    pub package: String,
    /// Agent type / binary name, e.g. `claude-code`
    pub agent_type: String,
}

/// Detect a version-upgrade error in an error message string.
///
/// Looks for `npm install -g <package>@<version>` anywhere in the message
/// (including nested JSON). Returns parsed upgrade info if found.
pub fn detect_upgrade_error(error_msg: &str) -> Option<UpgradeInfo> {
    let marker = "npm install -g ";
    let pos = error_msg.find(marker)?;
    let after = &error_msg[pos + marker.len()..];

    // Extract the package specifier: everything up to the next whitespace, quote, or end
    let end = after
        .find(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == '}')
        .unwrap_or(after.len());
    let package = after[..end].trim().to_string();

    if package.is_empty() {
        return None;
    }

    // Must contain a version separator '@' (not just a scoped prefix)
    // For scoped packages like @scope/name@ver, there are at least 2 '@' chars
    // For unscoped like name@ver, there is exactly 1 '@' char
    let has_version = if package.starts_with('@') {
        // Scoped: need a second '@' for version
        package[1..].contains('@')
    } else {
        package.contains('@')
    };

    if !has_version {
        return None;
    }

    // Extract agent_type (binary name) from the package specifier
    let agent_type = extract_agent_type(&package);

    Some(UpgradeInfo {
        package,
        agent_type,
    })
}

/// Extract the agent type / binary name from a package specifier.
///
/// `@anthropic-ai/claude-code@2.1.39` → `claude-code`
/// `some-tool@1.0.0` → `some-tool`
fn extract_agent_type(package: &str) -> String {
    // Strip version suffix
    let without_version = if package.starts_with('@') {
        // Scoped: @scope/name@version — find the second '@'
        if let Some(pos) = package[1..].find('@') {
            &package[..pos + 1]
        } else {
            package
        }
    } else {
        // Unscoped: name@version
        package.split('@').next().unwrap_or(package)
    };

    // Take the part after the last '/' (strip scope)
    without_version
        .rsplit('/')
        .next()
        .unwrap_or(without_version)
        .to_string()
}

/// Run `npm install -g <package>@<version>` to upgrade the agent.
///
/// Uses the enriched PATH from discovery to find npm.
pub async fn run_npm_upgrade(info: &UpgradeInfo) -> AppResult<()> {
    let enriched_path = discovery::get_enriched_path();
    let npm_path = resolve_in_path("npm", &enriched_path).ok_or_else(|| {
        AppError::Internal("npm not found on PATH — cannot perform automatic upgrade".into())
    })?;

    log::info!(
        "Running upgrade: {} install -g {}",
        npm_path,
        info.package
    );

    let output = tokio::process::Command::new(&npm_path)
        .args(["install", "-g", &info.package])
        .env("PATH", &enriched_path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to run npm upgrade: {e}")))?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        log::info!("npm upgrade succeeded: {}", stdout.trim());
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        Err(AppError::Internal(format!(
            "npm install -g {} failed (exit {}): stdout={}, stderr={}",
            info.package,
            output.status,
            stdout.trim(),
            stderr.trim(),
        )))
    }
}

/// If a local npm adapter exists at `~/.iaagenthub/adapters/<agent_type>/`,
/// update its package.json to use the new version and run `npm install`.
/// Non-fatal on failure.
pub async fn update_local_adapter(agent_type: &str) -> AppResult<()> {
    let adapters_base = discovery::get_adapters_dir();
    let adapter_dir = adapters_base.join(agent_type);

    // If the exact directory doesn't exist, try the "-acp" variant.
    // The built-in adapter directory is "claude-code-acp" but the package name
    // yields agent_type "claude-code".
    let adapter_dir = if adapter_dir.exists() {
        adapter_dir
    } else {
        let acp_variant = adapters_base.join(format!("{}-acp", agent_type));
        if acp_variant.exists() {
            log::info!(
                "Adapter dir for '{}' not found, using '-acp' variant: {:?}",
                agent_type,
                acp_variant
            );
            acp_variant
        } else {
            log::debug!(
                "No local adapter directory for {} (tried {} and {}-acp), skipping update",
                agent_type,
                agent_type,
                agent_type
            );
            return Ok(());
        }
    };

    // Try to update the version in package.json to use ^latest from registry
    // and remove stale overrides that may pin transitive dependencies to old versions
    let pkg_path = adapter_dir.join("package.json");
    if let Ok(content) = tokio::fs::read_to_string(&pkg_path).await {
        if let Ok(mut pkg) = serde_json::from_str::<serde_json::Value>(&content) {
            // We always update: add overrides + fix pinned versions
            let mut updated = true;

            // Ensure overrides force the SDK to latest across all nested deps.
            // Without this, the adapter's pinned SDK dependency (e.g. 0.2.38) may
            // be installed in a nested node_modules and take precedence over our
            // top-level latest version due to Node.js module resolution rules.
            {
                let obj = pkg.as_object_mut().unwrap();
                obj.insert(
                    "overrides".to_string(),
                    serde_json::json!({
                        "@anthropic-ai/claude-agent-sdk": "latest"
                    }),
                );
                log::info!("Set SDK override in package.json for {}", agent_type);
            }

            if let Some(deps) = pkg.get_mut("dependencies").and_then(|d| d.as_object_mut()) {
                // Find the dependency and update its version to a caret range
                // so npm install will pull the latest compatible version
                for (_key, value) in deps.iter_mut() {
                    if let Some(ver) = value.as_str() {
                        // If it's a pinned version (no ^, ~, or range operators), make it a caret range
                        let trimmed = ver.trim();
                        if !trimmed.is_empty()
                            && !trimmed.starts_with('^')
                            && !trimmed.starts_with('~')
                            && !trimmed.starts_with('>')
                            && !trimmed.starts_with('<')
                            && !trimmed.starts_with('=')
                            && trimmed != "latest"
                        {
                            *value = serde_json::Value::String(format!("^{}", trimmed));
                            updated = true;
                        }
                    }
                }
            }

            if updated {
                if let Ok(new_content) = serde_json::to_string_pretty(&pkg) {
                    let _ = tokio::fs::write(&pkg_path, new_content).await;
                    log::info!("Updated package.json for {} to use caret version range", agent_type);
                }
            }
        }
    }

    // Remove package-lock.json to force fresh resolution
    let lock_path = adapter_dir.join("package-lock.json");
    if lock_path.exists() {
        let _ = tokio::fs::remove_file(&lock_path).await;
        log::info!("Removed package-lock.json for {} to force fresh install", agent_type);
    }

    let enriched_path = discovery::get_enriched_path();
    let npm_path = resolve_in_path("npm", &enriched_path).ok_or_else(|| {
        AppError::Internal("npm not found on PATH".into())
    })?;

    log::info!(
        "Updating local adapter at {:?}: {} install",
        adapter_dir,
        npm_path
    );

    let output = tokio::process::Command::new(&npm_path)
        .arg("install")
        .current_dir(&adapter_dir)
        .env("PATH", &enriched_path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to run npm install in adapter dir: {e}")))?;

    if output.status.success() {
        log::info!("Local adapter update succeeded for {}", agent_type);
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        log::warn!(
            "Local adapter update failed for {} (non-fatal): {}",
            agent_type,
            stderr.trim()
        );
    }

    // Also force-upgrade the embedded claude-agent-sdk to latest.
    // The adapter may pin an older SDK whose bundled cli.js is rejected
    // by the remote API when it mandates a newer client version.
    upgrade_embedded_sdk(&adapter_dir, &enriched_path, &npm_path).await;

    Ok(())
}

/// Force-install the latest `@anthropic-ai/claude-agent-sdk` in an adapter directory.
/// The SDK bundles a complete Claude Code `cli.js`; if the remote API requires a newer
/// client version than what the adapter pins, this ensures we have it. Non-fatal.
async fn upgrade_embedded_sdk(
    adapter_dir: &std::path::Path,
    enriched_path: &str,
    npm_path: &str,
) {
    log::info!("upgrade_embedded_sdk: ensuring latest claude-agent-sdk in {:?}", adapter_dir);
    let result = tokio::process::Command::new(npm_path)
        .args(["install", "@anthropic-ai/claude-agent-sdk@latest"])
        .current_dir(adapter_dir)
        .env("PATH", enriched_path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await;

    match result {
        Ok(o) if o.status.success() => {
            let sdk_pkg = adapter_dir
                .join("node_modules/@anthropic-ai/claude-agent-sdk/package.json");
            if let Ok(content) = std::fs::read_to_string(&sdk_pkg) {
                if let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&content) {
                    let ver = pkg.get("version").and_then(|v| v.as_str()).unwrap_or("?");
                    log::info!("upgrade_embedded_sdk: claude-agent-sdk now at {}", ver);
                }
            }
        }
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            log::warn!("upgrade_embedded_sdk failed (non-fatal): {}", stderr.trim());
        }
        Err(e) => {
            log::warn!("upgrade_embedded_sdk spawn failed (non-fatal): {}", e);
        }
    }
}

/// Resolve a command in the given PATH (same pattern as provisioner.rs).
fn resolve_in_path(cmd: &str, path_env: &str) -> Option<String> {
    #[cfg(target_os = "windows")]
    let lookup = "where.exe";
    #[cfg(not(target_os = "windows"))]
    let lookup = "which";

    let output = std::process::Command::new(lookup)
        .arg(cmd)
        .env("PATH", path_env)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let first = stdout.lines().next().unwrap_or("").trim().to_string();
        if first.is_empty() {
            None
        } else {
            Some(first)
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_chinese_error() {
        let msg = "请升级客户端: npm install -g @anthropic-ai/claude-code@2.1.39";
        let info = detect_upgrade_error(msg).expect("should detect");
        assert_eq!(info.package, "@anthropic-ai/claude-code@2.1.39");
        assert_eq!(info.agent_type, "claude-code");
    }

    #[test]
    fn test_detect_english_error() {
        let msg = "Please upgrade: npm install -g @anthropic-ai/claude-code@2.1.39";
        let info = detect_upgrade_error(msg).expect("should detect");
        assert_eq!(info.package, "@anthropic-ai/claude-code@2.1.39");
        assert_eq!(info.agent_type, "claude-code");
    }

    #[test]
    fn test_detect_nested_json_error() {
        let msg = r#"Internal error: API Error: 400 {"error":{"message":"npm install -g @anthropic-ai/claude-code@2.1.39"}}"#;
        let info = detect_upgrade_error(msg).expect("should detect");
        assert_eq!(info.package, "@anthropic-ai/claude-code@2.1.39");
        assert_eq!(info.agent_type, "claude-code");
    }

    #[test]
    fn test_detect_unscoped_package() {
        let msg = "npm install -g some-tool@1.0.0";
        let info = detect_upgrade_error(msg).expect("should detect");
        assert_eq!(info.package, "some-tool@1.0.0");
        assert_eq!(info.agent_type, "some-tool");
    }

    #[test]
    fn test_no_version_no_detect() {
        let msg = "npm install -g some-tool";
        assert!(detect_upgrade_error(msg).is_none());
    }

    #[test]
    fn test_no_npm_marker() {
        let msg = "Some random error message without the marker";
        assert!(detect_upgrade_error(msg).is_none());
    }

    #[test]
    fn test_jsonrpc_wrapped_error() {
        let msg = "Agent error (code -32603): Internal error: API Error: 400 {\"error\":{\"message\":\"请升级客户端: npm install -g @anthropic-ai/claude-code@2.1.39\"}}";
        let info = detect_upgrade_error(msg).expect("should detect");
        assert_eq!(info.package, "@anthropic-ai/claude-code@2.1.39");
        assert_eq!(info.agent_type, "claude-code");
    }
}
