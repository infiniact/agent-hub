use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::AppResult;
use crate::models::agent::DiscoveredAgent;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentsJsonFile {
    agents: Vec<AgentsJsonEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentsJsonEntry {
    name: String,
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: std::collections::HashMap<String, String>,
}

/// ACP registry agent definition.
struct RegistryAgent {
    /// Binary / command name to look up in PATH.
    command: &'static str,
    /// Human-readable display name.
    name: &'static str,
    /// Extra CLI args required for ACP mode.
    args: &'static [&'static str],
    /// True if this agent uses the built-in Zed ACP adapter instead of PATH lookup.
    builtin: bool,
}

/// Full ACP agent registry (from https://agentclientprotocol.com/get-started/registry).
/// Claude Code is handled specially via the built-in Zed ACP adapter.
const REGISTRY_AGENTS: &[RegistryAgent] = &[
    RegistryAgent {
        command: "claude",
        name: "Claude Code",
        args: &[],
        builtin: true,
    },
    RegistryAgent {
        command: "codex",
        name: "Codex CLI",
        args: &["--acp"],
        builtin: false,
    },
    RegistryAgent {
        command: "gemini",
        name: "Gemini CLI",
        args: &["--experimental-acp"],
        builtin: false,
    },
    RegistryAgent {
        command: "copilot",
        name: "GitHub Copilot",
        args: &["--acp"],
        builtin: false,
    },
    RegistryAgent {
        command: "auggie",
        name: "Auggie CLI",
        args: &["--acp"],
        builtin: false,
    },
    RegistryAgent {
        command: "factory-droid",
        name: "Factory Droid",
        args: &["--acp"],
        builtin: false,
    },
    RegistryAgent {
        command: "kimi",
        name: "Kimi CLI",
        args: &["acp"],
        builtin: false,
    },
    RegistryAgent {
        command: "mistral-vibe",
        name: "Mistral Vibe",
        args: &["--acp"],
        builtin: false,
    },
    RegistryAgent {
        command: "opencode",
        name: "OpenCode",
        args: &["acp"],
        builtin: false,
    },
    RegistryAgent {
        command: "qoder",
        name: "Qoder CLI",
        args: &["--acp"],
        builtin: false,
    },
    RegistryAgent {
        command: "qwen-code",
        name: "Qwen Code",
        args: &["--acp"],
        builtin: false,
    },
    RegistryAgent {
        command: "goose",
        name: "Goose",
        args: &["acp"],
        builtin: false,
    },
    RegistryAgent {
        command: "aider",
        name: "Aider",
        args: &["--acp"],
        builtin: false,
    },
];

/// Get platform-specific config paths to search for agents.json
fn get_config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(config_dir) = dirs::config_dir() {
        paths.push(config_dir.join("acp").join("agents.json"));
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = dirs::home_dir() {
            paths.push(
                home.join("Library")
                    .join("Application Support")
                    .join("acp")
                    .join("agents.json"),
            );
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        paths.push(cwd.join(".acp").join("agents.json"));
    }

    paths
}

/// Get the project root directory (parent of src-tauri/).
pub(crate) fn get_project_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .unwrap_or(&manifest_dir)
        .to_path_buf()
}

/// Build an enriched PATH string that includes common CLI tool install locations.
pub(crate) fn get_enriched_path() -> String {
    let system_path = std::env::var("PATH").unwrap_or_default();

    #[cfg(not(target_os = "windows"))]
    {
        let mut extra: Vec<PathBuf> = Vec::new();

        if let Some(home) = dirs::home_dir() {
            extra.push(home.join(".local").join("bin"));
            extra.push(home.join(".cargo").join("bin"));
            extra.push(home.join("bin"));
        }

        extra.push(PathBuf::from("/opt/homebrew/bin"));
        extra.push(PathBuf::from("/opt/homebrew/sbin"));
        extra.push(PathBuf::from("/usr/local/bin"));

        let extra_str: Vec<String> = extra
            .iter()
            .filter(|p| p.is_dir())
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        if extra_str.is_empty() {
            system_path
        } else {
            format!("{}:{}", extra_str.join(":"), system_path)
        }
    }

    #[cfg(target_os = "windows")]
    {
        system_path
    }
}

/// Try to resolve the full path of a command using an enriched PATH.
fn resolve_command(cmd: &str) -> Option<String> {
    let enriched_path = get_enriched_path();

    #[cfg(target_os = "windows")]
    let lookup = "where.exe";
    #[cfg(not(target_os = "windows"))]
    let lookup = "which";

    let output = std::process::Command::new(lookup)
        .arg(cmd)
        .env("PATH", &enriched_path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let first_line = stdout.lines().next().unwrap_or("").trim().to_string();
        if first_line.is_empty() { None } else { Some(first_line) }
    } else {
        None
    }
}

/// Check if the built-in Zed ACP adapter for Claude Code is available.
fn check_builtin_acp_adapter() -> Option<(String, String)> {
    let project_root = get_project_root();
    let adapter_path = project_root
        .join("node_modules")
        .join("@zed-industries")
        .join("claude-code-acp")
        .join("dist")
        .join("index.js");

    if !adapter_path.exists() {
        log::debug!("Built-in ACP adapter not found at: {:?}", adapter_path);
        return None;
    }

    log::info!("Found built-in ACP adapter at: {:?}", adapter_path);

    let node_path = resolve_command("node")?;
    log::info!("Found node at: {}", node_path);

    Some((node_path, adapter_path.to_string_lossy().to_string()))
}

/// Scan config file paths for user-defined agents (always marked available).
async fn scan_config_agents() -> Vec<DiscoveredAgent> {
    let mut agents = Vec::new();

    for path in get_config_paths() {
        if path.exists() {
            if let Ok(content) = tokio::fs::read_to_string(&path).await {
                if let Ok(file) = serde_json::from_str::<AgentsJsonFile>(&content) {
                    for entry in file.agents {
                        agents.push(DiscoveredAgent {
                            id: uuid::Uuid::new_v4().to_string(),
                            name: entry.name.clone(),
                            command: entry.command.clone(),
                            args_json: serde_json::to_string(&entry.args)
                                .unwrap_or_else(|_| "[]".into()),
                            env_json: serde_json::to_string(&entry.env)
                                .unwrap_or_else(|_| "{}".into()),
                            source_path: path.to_string_lossy().to_string(),
                            last_seen_at: chrono::Utc::now()
                                .format("%Y-%m-%d %H:%M:%S")
                                .to_string(),
                            available: true,
                        });
                    }
                }
            }
        }
    }

    agents
}

/// Discover all ACP agents from the registry.
/// Returns ALL registry agents; `available` indicates whether installed on the system.
/// Also includes user-defined agents from config files (always available).
pub async fn discover_agents() -> AppResult<Vec<DiscoveredAgent>> {
    let mut agents = Vec::new();

    // Check if the built-in Zed ACP adapter exists (for Claude Code)
    let builtin_adapter = check_builtin_acp_adapter();

    // Build entries for every registry agent
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    for reg in REGISTRY_AGENTS {
        if reg.builtin {
            // Claude Code — use built-in adapter if present
            if let Some((node_path, adapter_path)) = &builtin_adapter {
                let args = vec![adapter_path.clone()];
                agents.push(DiscoveredAgent {
                    id: uuid::Uuid::new_v4().to_string(),
                    name: reg.name.to_string(),
                    command: node_path.clone(),
                    args_json: serde_json::to_string(&args).unwrap_or_else(|_| "[]".into()),
                    env_json: "{}".into(),
                    source_path: adapter_path.clone(),
                    last_seen_at: now.clone(),
                    available: true,
                });
            } else {
                // Adapter not installed — still list but unavailable
                agents.push(DiscoveredAgent {
                    id: uuid::Uuid::new_v4().to_string(),
                    name: reg.name.to_string(),
                    command: reg.command.to_string(),
                    args_json: "[]".into(),
                    env_json: "{}".into(),
                    source_path: String::new(),
                    last_seen_at: now.clone(),
                    available: false,
                });
            }
        } else {
            // Non-builtin agent — check if command exists in PATH
            let resolved = resolve_command(reg.command);
            let available = resolved.is_some();
            let command = resolved.unwrap_or_else(|| reg.command.to_string());
            let args: Vec<String> = reg.args.iter().map(|s| s.to_string()).collect();

            agents.push(DiscoveredAgent {
                id: uuid::Uuid::new_v4().to_string(),
                name: reg.name.to_string(),
                command: command.clone(),
                args_json: serde_json::to_string(&args).unwrap_or_else(|_| "[]".into()),
                env_json: "{}".into(),
                source_path: if available { command } else { String::new() },
                last_seen_at: now.clone(),
                available,
            });
        }
    }

    // User-defined agents from config files (always available)
    let config_agents = scan_config_agents().await;
    for a in &config_agents {
        log::info!("Discovered config agent: {}", a.name);
    }
    agents.extend(config_agents);

    let available_count = agents.iter().filter(|a| a.available).count();
    log::info!(
        "Discovery complete: {} total agents, {} available",
        agents.len(),
        available_count
    );

    Ok(agents)
}
