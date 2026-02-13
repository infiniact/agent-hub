use std::path::Path;

use crate::acp::discovery::{self, BinaryTarget, Distribution};
use crate::acp::builtin;
use crate::error::AppResult;

/// The resolved command after provisioning.
#[derive(Debug, Clone)]
pub struct ResolvedCommand {
    /// The actual command to execute (full path or "npx").
    pub command: String,
    /// Arguments to pass to the command.
    pub args: Vec<String>,
    /// The registry entry ID for identity tracking (e.g. "gemini", "codex-acp").
    pub agent_type: String,
}

/// Resolve the actual command + args for a given registry command name.
///
/// Resolution priority:
/// 1. Check PATH (enriched) — use directly
/// 2. Check `~/.iaagenthub/adapters/<agent_id>/` — use previously cached binary
/// 3. For binary distribution: download + extract → use cached binary
/// 4. For npx distribution: npx available → use `npx -y <package> <args>`
/// 5. Fallback: use command as-is
pub async fn resolve_agent_command(
    command: &str,
    args: &[String],
) -> AppResult<ResolvedCommand> {
    let basename = Path::new(command)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(command);

    let entry = discovery::get_registry_entry_by_command(basename).await;
    let agent_type = entry.as_ref().map(|e| e.id.as_str()).unwrap_or(basename);

    // Priority 0: Built-in agent — always prefer the embedded adapter.
    // The built-in adapter embeds JS files directly (not npm-installed), so the
    // generic staleness check cannot read its version and would incorrectly skip
    // it, falling through to npx which downloads an older published package.
    if let Some(ref entry) = entry {
        if builtin::is_builtin_agent(&entry.id) {
            let local_bin = discovery::get_adapters_dir()
                .join(&entry.id)
                .join("node_modules")
                .join(".bin")
                .join(basename);
            if local_bin.exists() {
                log::info!(
                    "Provisioner: using built-in adapter for {} at {:?}",
                    entry.id,
                    local_bin,
                );
                let dist_args = get_distribution_args(entry);
                let mut final_args = if dist_args.is_empty() {
                    args.to_vec()
                } else {
                    dist_args
                };
                final_args.extend(args.iter().cloned());
                return Ok(ResolvedCommand {
                    command: local_bin.to_string_lossy().to_string(),
                    args: final_args,
                    agent_type: entry.id.clone(),
                });
            }
        }
    }

    // 1. Check PATH
    let enriched_path = discovery::get_enriched_path();
    if let Some(resolved) = resolve_in_path(basename, &enriched_path) {
        log::info!("Provisioner: found {} on PATH at {}", basename, resolved);
        return Ok(ResolvedCommand {
            command: resolved,
            args: args.to_vec(),
            agent_type: agent_type.to_string(),
        });
    }

    // 2. Check cached binary by agent_id
    if let Some(ref entry) = entry {
        if let Some(cached) = discovery::check_downloaded_binary(&entry.id) {
            log::info!("Provisioner: using cached binary for {} at {:?}", entry.id, cached);
            let mut final_args = get_distribution_args(entry);
            if final_args.is_empty() {
                final_args = args.to_vec();
            }
            return Ok(ResolvedCommand {
                command: cached.to_string_lossy().to_string(),
                args: final_args,
                agent_type: entry.id.clone(),
            });
        }
    }

    if let Some(ref entry) = entry {
        match &entry.distribution {
            Distribution::Binary(platforms) => {
                let platform = discovery::get_current_platform();
                if let Some(target) = platforms.get(platform) {
                    // 3. Auto-download binary
                    log::info!(
                        "Provisioner: downloading binary for {} (platform: {})",
                        entry.id,
                        platform
                    );
                    match download_and_extract_binary(target, &entry.id).await {
                        Ok(binary_path) => {
                            log::info!("Provisioner: downloaded binary to {:?}", binary_path);
                            let mut final_args = target.args.clone();
                            if final_args.is_empty() {
                                final_args = args.to_vec();
                            }
                            return Ok(ResolvedCommand {
                                command: binary_path.to_string_lossy().to_string(),
                                args: final_args,
                                agent_type: entry.id.clone(),
                            });
                        }
                        Err(e) => {
                            log::warn!(
                                "Provisioner: binary download failed for {}: {}",
                                entry.id,
                                e
                            );
                        }
                    }
                }
            }
            Distribution::Npx(npx) => {
                // 3b. Check for locally-installed npm adapter in adapters dir
                let local_bin = discovery::get_adapters_dir()
                    .join(&entry.id)
                    .join("node_modules")
                    .join(".bin")
                    .join(basename);
                if local_bin.exists() {
                    // Check if the local adapter version matches the registry version
                    let registry_version = extract_package_version(&npx.package);
                    let local_version = read_local_adapter_version(&entry.id, &npx.package);
                    let is_stale = match (&registry_version, &local_version) {
                        (Some(reg_ver), Some(loc_ver)) if reg_ver != loc_ver => {
                            log::warn!(
                                "Provisioner: local adapter {} is stale (local={}, registry={}), skipping cache",
                                entry.id, loc_ver, reg_ver
                            );
                            true
                        }
                        (Some(_), None) => {
                            log::warn!(
                                "Provisioner: cannot determine local adapter version for {}, skipping cache",
                                entry.id,
                            );
                            true
                        }
                        _ => false,
                    };

                    if !is_stale {
                        log::info!(
                            "Provisioner: using locally-installed npm adapter for {} at {:?} (version: {})",
                            entry.id,
                            local_bin,
                            local_version.as_deref().unwrap_or("unknown"),
                        );
                        let mut final_args = npx.args.clone();
                        final_args.extend(args.iter().cloned());
                        return Ok(ResolvedCommand {
                            command: local_bin.to_string_lossy().to_string(),
                            args: final_args,
                            agent_type: entry.id.clone(),
                        });
                    }
                }

                // 4. NPX fallback
                if let Some(npx_path) = resolve_in_path("npx", &enriched_path) {
                    log::info!(
                        "Provisioner: using npx for {} (package: {})",
                        basename,
                        npx.package
                    );
                    let mut npx_args = vec!["-y".to_string(), npx.package.clone()];
                    npx_args.extend(args.iter().cloned());
                    return Ok(ResolvedCommand {
                        command: npx_path,
                        args: npx_args,
                        agent_type: entry.id.clone(),
                    });
                }
            }
        }
    }

    // 5. Fallback: try to run the command directly
    log::warn!(
        "Provisioner: no resolution found for {}, using command as-is",
        basename
    );
    Ok(ResolvedCommand {
        command: command.to_string(),
        args: args.to_vec(),
        agent_type: agent_type.to_string(),
    })
}

/// Get the args from a registry entry distribution for the current platform.
fn get_distribution_args(entry: &discovery::RegistryEntry) -> Vec<String> {
    match &entry.distribution {
        Distribution::Npx(npx) => npx.args.clone(),
        Distribution::Binary(platforms) => {
            let platform = discovery::get_current_platform();
            platforms
                .get(platform)
                .map(|t| t.args.clone())
                .unwrap_or_default()
        }
    }
}

/// Check if a resolved command is npx-based (for adjusting startup behaviour).
pub fn is_npx_command(command: &str) -> bool {
    let basename = Path::new(command)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(command);
    basename == "npx" || basename == "pnpx"
}

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

/// Extract version from a package specifier like `@scope/name@1.2.3` → `Some("1.2.3")`.
fn extract_package_version(package: &str) -> Option<String> {
    if package.starts_with('@') {
        // Scoped: @scope/name@version — find the second '@'
        package[1..].find('@').map(|pos| package[pos + 2..].to_string())
    } else {
        // Unscoped: name@version
        package.split('@').nth(1).map(|v| v.to_string())
    }
}

/// Read the installed version of an npm package from the local adapter's
/// `node_modules/<package>/package.json`.
fn read_local_adapter_version(agent_id: &str, package_specifier: &str) -> Option<String> {
    // Extract the package name (without version) from the specifier
    let pkg_name = if package_specifier.starts_with('@') {
        // Scoped: @scope/name@version → @scope/name
        match package_specifier[1..].find('@') {
            Some(pos) => &package_specifier[..pos + 1],
            None => package_specifier,
        }
    } else {
        package_specifier.split('@').next().unwrap_or(package_specifier)
    };

    let pkg_json_path = discovery::get_adapters_dir()
        .join(agent_id)
        .join("node_modules")
        .join(pkg_name)
        .join("package.json");

    let content = std::fs::read_to_string(&pkg_json_path).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&content).ok()?;
    parsed.get("version").and_then(|v| v.as_str()).map(|s| s.to_string())
}

// ---------------------------------------------------------------------------
// Binary download & extraction
// ---------------------------------------------------------------------------

/// Download and extract a binary archive to `~/.iaagenthub/adapters/<agent_id>/`.
/// Returns the path to the extracted executable.
pub async fn download_and_extract_binary(
    target: &BinaryTarget,
    agent_id: &str,
) -> Result<std::path::PathBuf, String> {
    let dest_dir = discovery::get_adapters_dir().join(agent_id);

    // Create destination directory
    tokio::fs::create_dir_all(&dest_dir)
        .await
        .map_err(|e| format!("Failed to create adapter dir: {e}"))?;

    // Download the archive
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    log::info!("Downloading binary archive: {}", target.archive);
    let resp = client
        .get(&target.archive)
        .send()
        .await
        .map_err(|e| format!("Download error: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Download HTTP {}", resp.status()));
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("Read download body error: {e}"))?;

    // Write to temp file
    let archive_name = target
        .archive
        .rsplit('/')
        .next()
        .unwrap_or("archive.tar.gz");
    let tmp_path = dest_dir.join(archive_name);
    tokio::fs::write(&tmp_path, &bytes)
        .await
        .map_err(|e| format!("Write archive error: {e}"))?;

    // Extract based on file extension
    let tmp_path_clone = tmp_path.clone();
    let dest_dir_clone = dest_dir.clone();
    let is_zip = archive_name.ends_with(".zip");

    tokio::task::spawn_blocking(move || {
        if is_zip {
            extract_zip(&tmp_path_clone, &dest_dir_clone)
        } else {
            extract_tar_gz(&tmp_path_clone, &dest_dir_clone)
        }
    })
    .await
    .map_err(|e| format!("Extract task join error: {e}"))?
    .map_err(|e| format!("Extract error: {e}"))?;

    // Clean up archive file
    let _ = tokio::fs::remove_file(&tmp_path).await;

    // Find the extracted binary
    let cmd_name = target.cmd.trim_start_matches("./");
    let binary_path = dest_dir.join(cmd_name);

    if binary_path.exists() {
        // Ensure executable permission on unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(
                &binary_path,
                std::fs::Permissions::from_mode(0o755),
            );
        }
        Ok(binary_path)
    } else {
        // Try to find any executable in the directory
        if let Some(found) = discovery::check_downloaded_binary(agent_id) {
            Ok(found)
        } else {
            Err(format!(
                "Binary '{}' not found in extracted archive at {:?}",
                cmd_name, dest_dir
            ))
        }
    }
}

/// Extract a .tar.gz archive to the destination directory.
fn extract_tar_gz(
    archive_path: &std::path::Path,
    dest: &std::path::Path,
) -> Result<(), String> {
    let file =
        std::fs::File::open(archive_path).map_err(|e| format!("Open archive error: {e}"))?;
    let gz = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(gz);
    archive
        .unpack(dest)
        .map_err(|e| format!("Unpack tar.gz error: {e}"))?;
    log::info!("Extracted tar.gz to {:?}", dest);
    Ok(())
}

/// Extract a .zip archive to the destination directory.
fn extract_zip(
    archive_path: &std::path::Path,
    dest: &std::path::Path,
) -> Result<(), String> {
    let file =
        std::fs::File::open(archive_path).map_err(|e| format!("Open archive error: {e}"))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Read zip error: {e}"))?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Zip entry error: {e}"))?;
        let outpath = match entry.enclosed_name() {
            Some(path) => dest.join(path),
            None => continue,
        };

        if entry.is_dir() {
            std::fs::create_dir_all(&outpath)
                .map_err(|e| format!("Create dir error: {e}"))?;
        } else {
            if let Some(parent) = outpath.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Create parent dir error: {e}"))?;
            }
            let mut outfile = std::fs::File::create(&outpath)
                .map_err(|e| format!("Create file error: {e}"))?;
            std::io::copy(&mut entry, &mut outfile)
                .map_err(|e| format!("Copy file error: {e}"))?;

            // Set executable permissions on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = entry.unix_mode() {
                    let _ = std::fs::set_permissions(
                        &outpath,
                        std::fs::Permissions::from_mode(mode),
                    );
                }
            }
        }
    }
    log::info!("Extracted zip to {:?}", dest);
    Ok(())
}
