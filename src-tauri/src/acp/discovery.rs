use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;
use tokio::sync::Mutex as AsyncMutex;

use crate::error::AppResult;
use crate::models::agent::DiscoveredAgent;

// ---------------------------------------------------------------------------
// User-defined agents.json
// ---------------------------------------------------------------------------

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
    env: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// Dynamic registry types — matches CDN registry.json schema
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryFile {
    pub version: String,
    pub agents: Vec<RegistryEntry>,
    #[serde(default)]
    pub extensions: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEntry {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(default)]
    pub repository: Option<String>,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    pub distribution: Distribution,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Distribution {
    Npx(NpxDistribution),
    Binary(HashMap<String, BinaryTarget>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpxDistribution {
    pub package: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryTarget {
    pub archive: String,
    pub cmd: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// Registry cache
// ---------------------------------------------------------------------------

const REGISTRY_URL: &str =
    "https://cdn.agentclientprotocol.com/registry/v1/latest/registry.json";

/// In-memory cache of the fetched registry.
static REGISTRY_CACHE: OnceLock<AsyncMutex<Option<RegistryFile>>> = OnceLock::new();

fn registry_mutex() -> &'static AsyncMutex<Option<RegistryFile>> {
    REGISTRY_CACHE.get_or_init(|| AsyncMutex::new(None))
}

/// Local file cache path: `~/.iaagenthub/registry.json`
fn local_cache_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".iaagenthub")
        .join("registry.json")
}

/// Path to the installed-agents manifest: `~/.iaagenthub/installed.json`
/// Contains a JSON array of registry IDs that have been explicitly installed.
fn installed_manifest_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".iaagenthub")
        .join("installed.json")
}

/// Read the set of explicitly-installed registry IDs.
pub fn load_installed_set() -> std::collections::HashSet<String> {
    let path = installed_manifest_path();
    if let Ok(content) = std::fs::read_to_string(&path) {
        if let Ok(ids) = serde_json::from_str::<Vec<String>>(&content) {
            return ids.into_iter().collect();
        }
    }
    std::collections::HashSet::new()
}

/// Persist the set of installed registry IDs.
fn save_installed_set(set: &std::collections::HashSet<String>) -> Result<(), String> {
    let path = installed_manifest_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }
    let ids: Vec<&String> = set.iter().collect();
    let json = serde_json::to_string_pretty(&ids).map_err(|e| format!("json: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("write: {e}"))?;
    Ok(())
}

/// Mark a registry ID as installed.
pub fn mark_installed(registry_id: &str) {
    let mut set = load_installed_set();
    set.insert(registry_id.to_string());
    if let Err(e) = save_installed_set(&set) {
        log::warn!("Failed to save installed manifest: {}", e);
    }
}

/// Mark a registry ID as uninstalled.
pub fn mark_uninstalled(registry_id: &str) {
    let mut set = load_installed_set();
    set.remove(registry_id);
    if let Err(e) = save_installed_set(&set) {
        log::warn!("Failed to save installed manifest: {}", e);
    }
}

/// Fetch the registry from CDN, falling back to local file cache.
/// The result is cached in memory for subsequent calls.
pub async fn fetch_registry() -> AppResult<RegistryFile> {
    // Return in-memory cache if available
    {
        let guard = registry_mutex().lock().await;
        if let Some(ref cached) = *guard {
            return Ok(cached.clone());
        }
    }

    let registry = match fetch_registry_from_cdn().await {
        Ok(reg) => {
            // Persist to local cache
            if let Err(e) = save_local_cache(&reg).await {
                log::warn!("Failed to save registry cache: {}", e);
            }
            reg
        }
        Err(e) => {
            log::warn!("Failed to fetch registry from CDN: {}, trying local cache", e);
            load_local_cache().await.map_err(|cache_err| {
                crate::error::AppError::Internal(format!(
                    "Failed to fetch registry from CDN ({}) and local cache ({})",
                    e, cache_err
                ))
            })?
        }
    };

    // Store in memory cache
    {
        let mut guard = registry_mutex().lock().await;
        *guard = Some(registry.clone());
    }

    log::info!(
        "Registry loaded: {} agents, version {}",
        registry.agents.len(),
        registry.version
    );
    Ok(registry)
}

async fn fetch_registry_from_cdn() -> Result<RegistryFile, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let resp = client
        .get(REGISTRY_URL)
        .send()
        .await
        .map_err(|e| format!("HTTP request error: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }

    let body = resp
        .text()
        .await
        .map_err(|e| format!("Read body error: {e}"))?;

    serde_json::from_str::<RegistryFile>(&body)
        .map_err(|e| format!("JSON parse error: {e}"))
}

async fn save_local_cache(registry: &RegistryFile) -> Result<(), String> {
    let path = local_cache_path();
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("mkdir error: {e}"))?;
    }
    let json = serde_json::to_string_pretty(registry)
        .map_err(|e| format!("JSON serialize error: {e}"))?;
    tokio::fs::write(&path, json)
        .await
        .map_err(|e| format!("Write cache error: {e}"))?;
    log::debug!("Saved registry cache to {:?}", path);
    Ok(())
}

async fn load_local_cache() -> Result<RegistryFile, String> {
    let path = local_cache_path();
    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("Read cache error: {e}"))?;
    serde_json::from_str::<RegistryFile>(&content)
        .map_err(|e| format!("Parse cache error: {e}"))
}

/// Force-refresh the registry from CDN (clears in-memory cache first).
pub async fn refresh_registry() -> AppResult<RegistryFile> {
    {
        let mut guard = registry_mutex().lock().await;
        *guard = None;
    }
    fetch_registry().await
}

// ---------------------------------------------------------------------------
// Platform detection
// ---------------------------------------------------------------------------

/// Return the current platform identifier matching the registry format.
/// e.g. `darwin-aarch64`, `linux-x86_64`, `windows-x86_64`
pub fn get_current_platform() -> &'static str {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    { "darwin-aarch64" }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    { "darwin-x86_64" }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    { "linux-aarch64" }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    { "linux-x86_64" }
    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    { "windows-aarch64" }
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    { "windows-x86_64" }
    #[cfg(not(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "aarch64"),
        all(target_os = "windows", target_arch = "x86_64"),
    )))]
    { "unknown" }
}

// ---------------------------------------------------------------------------
// Dynamic registry lookup helpers
// ---------------------------------------------------------------------------

/// Look up a registry entry by its ID.
pub async fn get_registry_entry(id: &str) -> Option<RegistryEntry> {
    let registry = fetch_registry().await.ok()?;
    registry.agents.into_iter().find(|e| e.id == id)
}

/// Look up a registry entry by command name.
/// For npx agents, this matches the binary name in the package (e.g. "gemini" matches package containing "gemini-cli").
/// For binary agents, this matches the cmd field basename.
pub async fn get_registry_entry_by_command(command: &str) -> Option<RegistryEntry> {
    let basename = std::path::Path::new(command)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(command);

    let registry = fetch_registry().await.ok()?;
    registry.agents.into_iter().find(|entry| {
        match &entry.distribution {
            Distribution::Npx(npx) => {
                // Match against the binary name extracted from the package specifier
                let pkg_basename = extract_npx_binary_name(&npx.package);
                pkg_basename == basename || entry.id == basename
            }
            Distribution::Binary(platforms) => {
                // Match against cmd basename from any platform
                entry.id == basename
                    || platforms.values().any(|t| {
                        let cmd_base = t.cmd.trim_start_matches("./");
                        let cmd_base = cmd_base.strip_suffix(".exe").unwrap_or(cmd_base);
                        cmd_base == basename
                    })
            }
        }
    })
}

/// Get environment variables from a registry entry for the current platform.
pub fn get_env_for_entry(entry: &RegistryEntry) -> HashMap<String, String> {
    match &entry.distribution {
        Distribution::Npx(npx) => npx.env.clone(),
        Distribution::Binary(platforms) => {
            let platform = get_current_platform();
            platforms
                .get(platform)
                .map(|t| t.env.clone())
                .unwrap_or_default()
        }
    }
}

/// Get extra environment variables for a given agent command (dynamic lookup).
pub async fn get_agent_env_for_command(command: &str) -> HashMap<String, String> {
    if let Some(entry) = get_registry_entry_by_command(command).await {
        get_env_for_entry(&entry)
    } else {
        HashMap::new()
    }
}

// ---------------------------------------------------------------------------
// Path and discovery utilities (unchanged)
// ---------------------------------------------------------------------------

/// Return the directory where downloaded adapter binaries are cached.
/// `~/.iaagenthub/adapters/`
pub fn get_adapters_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".iaagenthub")
        .join("adapters")
}

/// Check if a downloaded binary exists in the adapters cache for the given agent ID.
/// Returns the full path if found.
pub fn check_downloaded_binary(agent_id: &str) -> Option<PathBuf> {
    let dir = get_adapters_dir().join(agent_id);
    if !dir.exists() {
        return None;
    }
    // Try to find any executable file in the directory
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(meta) = path.metadata() {
                        if meta.permissions().mode() & 0o111 != 0 {
                            return Some(path);
                        }
                    }
                }
                #[cfg(windows)]
                {
                    if let Some(ext) = path.extension() {
                        if ext == "exe" {
                            return Some(path);
                        }
                    }
                }
            }
        }
    }
    // Fallback: check for binary named after the agent_id
    let candidate = dir.join(agent_id);
    if candidate.exists() && candidate.is_file() {
        return Some(candidate);
    }
    #[cfg(target_os = "windows")]
    {
        let candidate_exe = dir.join(format!("{}.exe", agent_id));
        if candidate_exe.exists() && candidate_exe.is_file() {
            return Some(candidate_exe);
        }
    }
    None
}

/// Extract the CLI binary name from an npm package specifier.
///
/// Handles scoped (`@scope/name@version`) and unscoped (`name@version`) packages:
///   `@zed-industries/claude-code-acp@0.16.0` → `claude-code-acp`
///   `@google/gemini-cli@0.27.3`              → `gemini-cli`
///   `some-tool@1.0.0`                        → `some-tool`
fn extract_npx_binary_name(package: &str) -> &str {
    // Step 1: strip the version suffix
    let without_version = if package.starts_with('@') {
        // Scoped: @scope/name or @scope/name@version
        // Find the second '@' which is the version separator
        if let Some(pos) = package[1..].find('@') {
            &package[..pos + 1]
        } else {
            package
        }
    } else {
        // Unscoped: name or name@version
        package.split('@').next().unwrap_or(package)
    };

    // Step 2: take the part after the last '/' (strip scope)
    without_version.rsplit('/').next().unwrap_or(without_version)
}
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

            // Node version managers — ensure node/npx are discoverable
            let nvm_dir = std::env::var("NVM_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| home.join(".nvm"));
            let node_versions = nvm_dir.join("versions").join("node");
            if node_versions.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&node_versions) {
                    for entry in entries.flatten() {
                        let bin = entry.path().join("bin");
                        if bin.is_dir() {
                            extra.push(bin);
                        }
                    }
                }
            }

            // fnm: ~/.fnm/aliases/default/bin
            extra.push(home.join(".fnm").join("aliases").join("default").join("bin"));

            // volta: ~/.volta/bin
            extra.push(home.join(".volta").join("bin"));
        }

        // Adapters dir (for downloaded binary agents)
        let adapters_dir = get_adapters_dir();
        if adapters_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&adapters_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        extra.push(path);
                    }
                }
            }
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
    resolve_command_with_path(cmd, &get_enriched_path())
}

/// Resolve command in PATH using a specific PATH env value.
fn resolve_command_with_path(cmd: &str, path_env: &str) -> Option<String> {
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
        if first.is_empty() { None } else { Some(first) }
    } else {
        None
    }
}

/// Read the `version` field from a locally-installed NPX adapter's `package.json`.
///
/// Looks for `~/.iaagenthub/adapters/<agent_id>/node_modules/<pkg>/package.json`.
fn read_local_adapter_version(agent_id: &str, npx_package: &str) -> Option<String> {
    // npx_package is e.g. "@google/gemini-cli@0.27.3" or "some-tool@1.0.0"
    // We need the package name without version: "@google/gemini-cli" or "some-tool"
    let pkg_name = if npx_package.starts_with('@') {
        // Scoped: strip version after second '@'
        if let Some(pos) = npx_package[1..].find('@') {
            &npx_package[..pos + 1]
        } else {
            npx_package
        }
    } else {
        npx_package.split('@').next().unwrap_or(npx_package)
    };

    let pkg_json_path = get_adapters_dir()
        .join(agent_id)
        .join("node_modules")
        .join(pkg_name)
        .join("package.json");

    let content = std::fs::read_to_string(&pkg_json_path).ok()?;
    let val: serde_json::Value = serde_json::from_str(&content).ok()?;
    val.get("version")?.as_str().map(|s| s.to_string())
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
                            models: Vec::new(),
                            registry_id: None,
                            icon_url: None,
                            description: String::new(),
                            adapter_version: None,
                            cli_version: None,
                        });
                    }
                }
            }
        }
    }

    agents
}

// ---------------------------------------------------------------------------
// Main discovery entry point
// ---------------------------------------------------------------------------

/// Discover all ACP agents from the dynamic registry + user config.
/// Returns ALL registry agents; `available` indicates whether installed on the system.
pub async fn discover_agents() -> AppResult<Vec<DiscoveredAgent>> {
    let registry = fetch_registry().await?;
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let platform = get_current_platform();
    let installed_set = load_installed_set();
    let npx_available = resolve_command("npx").is_some();
    let mut agents = Vec::new();

    for entry in &registry.agents {
        // Skip entries that are handled by the built-in agent
        if super::builtin::is_builtin_agent(&entry.id) {
            log::info!("Skipping registry entry '{}' (handled by built-in)", entry.id);
            continue;
        }

        let explicitly_installed = installed_set.contains(&entry.id);

        match &entry.distribution {
            Distribution::Npx(npx) => {
                let pkg_name = extract_npx_binary_name(&npx.package);
                let direct_resolved = resolve_command(pkg_name);

                // Check for locally-installed npm package in adapters dir
                let local_bin = get_adapters_dir()
                    .join(&entry.id)
                    .join("node_modules")
                    .join(".bin")
                    .join(pkg_name);
                let has_local_install = local_bin.exists();

                // Available only if binary is on PATH, locally installed, or explicitly installed (via manifest)
                let available = direct_resolved.is_some() || has_local_install || explicitly_installed;
                // Can be installed if npx exists on PATH
                let can_install = npx_available;

                let command = if let Some(ref path) = direct_resolved {
                    path.clone()
                } else {
                    pkg_name.to_string()
                };

                let source_path = if direct_resolved.is_some() {
                    command.clone()
                } else if has_local_install {
                    local_bin.to_string_lossy().to_string()
                } else if explicitly_installed {
                    format!("npx:{}", npx.package)
                } else if can_install {
                    format!("installable:npx:{}", npx.package)
                } else {
                    String::new()
                };

                // Read adapter version from locally installed package
                let adapter_version = read_local_adapter_version(&entry.id, &npx.package);

                agents.push(DiscoveredAgent {
                    id: uuid::Uuid::new_v4().to_string(),
                    name: entry.name.clone(),
                    command,
                    args_json: serde_json::to_string(&npx.args)
                        .unwrap_or_else(|_| "[]".into()),
                    env_json: serde_json::to_string(&npx.env)
                        .unwrap_or_else(|_| "{}".into()),
                    source_path,
                    last_seen_at: now.clone(),
                    available,
                    models: Vec::new(),
                    registry_id: Some(entry.id.clone()),
                    icon_url: entry.icon.clone(),
                    description: entry.description.clone(),
                    adapter_version,
                    cli_version: None,
                });
            }
            Distribution::Binary(platforms) => {
                let target = platforms.get(platform);

                if let Some(target) = target {
                    let cmd_name = target.cmd.trim_start_matches("./");
                    let cmd_name = cmd_name.strip_suffix(".exe").unwrap_or(cmd_name);

                    let direct_resolved = resolve_command(cmd_name);
                    let cached = check_downloaded_binary(&entry.id);

                    // Available only if on PATH, in cache, or explicitly installed
                    let available = direct_resolved.is_some()
                        || cached.is_some()
                        || explicitly_installed;

                    let command = if let Some(ref path) = direct_resolved {
                        path.clone()
                    } else if let Some(ref cached_path) = cached {
                        cached_path.to_string_lossy().to_string()
                    } else {
                        cmd_name.to_string()
                    };

                    let source_path = if let Some(ref path) = direct_resolved {
                        path.clone()
                    } else if let Some(ref cached_path) = cached {
                        cached_path.to_string_lossy().to_string()
                    } else {
                        // Not installed yet — mark as installable
                        format!("installable:binary:{}", target.archive)
                    };

                    agents.push(DiscoveredAgent {
                        id: uuid::Uuid::new_v4().to_string(),
                        name: entry.name.clone(),
                        command,
                        args_json: serde_json::to_string(&target.args)
                            .unwrap_or_else(|_| "[]".into()),
                        env_json: serde_json::to_string(&target.env)
                            .unwrap_or_else(|_| "{}".into()),
                        source_path,
                        last_seen_at: now.clone(),
                        available,
                        models: Vec::new(),
                        registry_id: Some(entry.id.clone()),
                        icon_url: entry.icon.clone(),
                        description: entry.description.clone(),
                        adapter_version: Some(entry.version.clone()),
                        cli_version: None,
                    });
                } else {
                    // No binary for current platform
                    agents.push(DiscoveredAgent {
                        id: uuid::Uuid::new_v4().to_string(),
                        name: entry.name.clone(),
                        command: entry.id.clone(),
                        args_json: "[]".into(),
                        env_json: "{}".into(),
                        source_path: String::new(),
                        last_seen_at: now.clone(),
                        available: false,
                        models: Vec::new(),
                        registry_id: Some(entry.id.clone()),
                        icon_url: entry.icon.clone(),
                        description: entry.description.clone(),
                        adapter_version: Some(entry.version.clone()),
                        cli_version: None,
                    });
                }
            }
        }
    }

    // Inject built-in agent (before config agents)
    let builtin_agent = super::builtin::get_builtin_agent();
    log::info!(
        "Built-in agent '{}': available={}",
        builtin_agent.name,
        builtin_agent.available
    );
    agents.push(builtin_agent);

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
