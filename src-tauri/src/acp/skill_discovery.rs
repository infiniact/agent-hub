use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::models::agent::AgentSkill;

/// A discovered skill directory entry with its source path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDirEntry {
    pub skill: AgentSkill,
    /// The directory that contains this skill definition.
    pub dir_path: String,
    /// The SKILL.md file that was parsed.
    pub md_file: String,
    /// "project" or "user"
    pub location: String,
    /// SKILL.md body content, loaded on activation (progressive loading).
    #[serde(default)]
    pub body_content: Option<String>,
    /// Whether the skill directory contains a `scripts/` subdirectory.
    #[serde(default)]
    pub has_scripts: bool,
    /// Whether the skill directory contains a `references/` subdirectory.
    #[serde(default)]
    pub has_references: bool,
    /// Whether the skill directory contains an `assets/` subdirectory.
    #[serde(default)]
    pub has_assets: bool,
}

/// Result of a skill discovery scan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDiscoveryResult {
    pub skills: Vec<SkillDirEntry>,
    pub scanned_directories: Vec<String>,
    pub last_scanned_at: String,
}

/// Discover skills from skills directories.
///
/// Scans two locations:
/// 1. `{cwd}/skills/` — working directory (project-level) skills
/// 2. `~/.iaagenthub/skills/` — config directory (global) skills
///
/// Each immediate subdirectory of `skills/` is treated as a skill.
/// Inside each subdirectory, a `SKILL.md` file is parsed for YAML
/// frontmatter fields (`name`, `description`, `allowed-tools`).
///
/// The directory name is used as the skill ID and as a fallback for
/// the `name` field. Project skills take priority over global skills
/// when IDs collide.
pub fn discover_skills(cwd: &str) -> SkillDiscoveryResult {
    let mut skills: Vec<SkillDirEntry> = Vec::new();
    let mut scanned_directories: Vec<String> = Vec::new();
    let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    // 1. Working directory: {cwd}/skills/
    let project_skills_dir = Path::new(cwd).join("skills");
    if project_skills_dir.is_dir() {
        scanned_directories.push(project_skills_dir.to_string_lossy().to_string());
        let entries = scan_skills_directory(&project_skills_dir, "project");
        for entry in entries {
            if seen_ids.insert(entry.skill.id.clone()) {
                skills.push(entry);
            }
        }
    }

    // 2. Config directory: ~/.iaagenthub/skills/
    if let Some(home) = dirs::home_dir() {
        let global_skills_dir = home.join(".iaagenthub").join("skills");
        if global_skills_dir.is_dir() {
            scanned_directories.push(global_skills_dir.to_string_lossy().to_string());
            let entries = scan_skills_directory(&global_skills_dir, "user");
            for entry in entries {
                // Project skills take priority (dedup by ID)
                if seen_ids.insert(entry.skill.id.clone()) {
                    skills.push(entry);
                }
            }
        }
    }

    log::info!(
        "Skill discovery: found {} skills from {} directories",
        skills.len(),
        scanned_directories.len(),
    );

    SkillDiscoveryResult {
        skills,
        scanned_directories,
        last_scanned_at: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    }
}

/// Scan a `skills/` directory. Each immediate subdirectory is a skill.
fn scan_skills_directory(skills_dir: &Path, location: &str) -> Vec<SkillDirEntry> {
    let mut entries = Vec::new();

    let read_dir = match std::fs::read_dir(skills_dir) {
        Ok(rd) => rd,
        Err(e) => {
            log::warn!("Failed to read skills directory {}: {}", skills_dir.display(), e);
            return entries;
        }
    };

    for dir_entry in read_dir {
        let dir_entry = match dir_entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = dir_entry.path();
        if !path.is_dir() {
            continue;
        }

        let dir_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        // Skip hidden directories
        if dir_name.starts_with('.') {
            continue;
        }

        if let Some(skill_entry) = parse_skill_dir(&path, &dir_name, location) {
            entries.push(skill_entry);
        }
    }

    // Sort by ID for stable ordering
    entries.sort_by(|a, b| a.skill.id.cmp(&b.skill.id));
    entries
}

/// Parse a single skill directory by reading its SKILL.md file.
fn parse_skill_dir(skill_dir: &Path, dir_name: &str, location: &str) -> Option<SkillDirEntry> {
    // Validate directory name per Agent Skills spec
    if !is_valid_skill_name(dir_name) {
        log::warn!(
            "Skipping skill directory '{}': name does not conform to Agent Skills spec (lowercase, digits, hyphens, 1-64 chars, no leading/trailing/consecutive hyphens)",
            dir_name,
        );
        return None;
    }

    // Agent Skills convention: SKILL.md (uppercase)
    let skill_md = skill_dir.join("SKILL.md");
    if !skill_md.is_file() {
        log::debug!("No SKILL.md in {}", skill_dir.display());
        return None;
    }

    let content = match std::fs::read_to_string(&skill_md) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("Failed to read {}: {}", skill_md.display(), e);
            return None;
        }
    };

    let (frontmatter, body) = match parse_frontmatter(&content) {
        Some(pair) => pair,
        None => {
            log::warn!("No valid YAML frontmatter in {}", skill_md.display());
            return None;
        }
    };

    // Parse frontmatter `name` — must match directory name per spec
    let fm_name = extract_field(&frontmatter, "name");
    let name = match fm_name {
        Some(ref n) if n != dir_name => {
            log::warn!(
                "Skill '{}': frontmatter name '{}' does not match directory name, using directory name",
                dir_name, n,
            );
            dir_name.to_string()
        }
        Some(n) => n,
        None => dir_name.to_string(),
    };

    // `description` is required by spec; fall back to first body line
    let description = extract_field(&frontmatter, "description")
        .unwrap_or_else(|| {
            body.lines()
                .find(|l| !l.trim().is_empty() && !l.trim().starts_with('#'))
                .unwrap_or("")
                .trim()
                .to_string()
        });

    // `allowed-tools`: spec requires space-separated (not comma)
    let allowed_tools = extract_field(&frontmatter, "allowed-tools")
        .unwrap_or_default();

    // New spec fields
    let license = extract_field(&frontmatter, "license");
    let compatibility = extract_field(&frontmatter, "compatibility");
    let metadata = extract_metadata_block(&frontmatter);

    // Build task_keywords from name by splitting on `-` and `_`
    let keywords: Vec<String> = name
        .split(|c: char| c == '-' || c == '_' || c == ' ')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase())
        .collect();

    // Convert allowed-tools to constraints (space-separated per spec)
    let constraints: Vec<String> = if allowed_tools.is_empty() {
        Vec::new()
    } else {
        allowed_tools
            .split_whitespace()
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
            .collect()
    };

    // Detect optional subdirectories per Agent Skills spec
    let has_scripts = skill_dir.join("scripts").is_dir();
    let has_references = skill_dir.join("references").is_dir();
    let has_assets = skill_dir.join("assets").is_dir();

    // Store body for progressive loading (activated on demand)
    let body_content = if body.is_empty() { None } else { Some(body) };

    let skill = AgentSkill {
        id: dir_name.to_string(),
        name,
        skill_type: "skill".into(),
        description,
        task_keywords: keywords,
        constraints,
        skill_source: format!("discovered:{}", location),
        license,
        compatibility,
        metadata,
    };

    Some(SkillDirEntry {
        skill,
        dir_path: skill_dir.to_string_lossy().to_string(),
        md_file: skill_md.to_string_lossy().to_string(),
        location: location.to_string(),
        body_content,
        has_scripts,
        has_references,
        has_assets,
    })
}

// ---------------------------------------------------------------------------
// Frontmatter parsing
// ---------------------------------------------------------------------------

fn parse_frontmatter(content: &str) -> Option<(String, String)> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }

    let after_first = &trimmed[3..];
    let end_idx = after_first.find("\n---")?;
    let frontmatter = after_first[..end_idx].trim().to_string();
    let body = after_first[end_idx + 4..].trim().to_string();

    Some((frontmatter, body))
}

fn extract_field(frontmatter: &str, key: &str) -> Option<String> {
    for line in frontmatter.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix(&format!("{key}:")) {
            let val = rest.trim();
            if val.is_empty() {
                return None;
            }
            // Remove surrounding quotes
            if (val.starts_with('"') && val.ends_with('"'))
                || (val.starts_with('\'') && val.ends_with('\''))
            {
                return Some(val[1..val.len() - 1].to_string());
            }
            return Some(val.to_string());
        }
    }
    None
}

/// Validate a skill name per Agent Skills spec:
/// - 1-64 characters
/// - Only lowercase ASCII letters, digits, and hyphens
/// - Must not start or end with `-`
/// - Must not contain consecutive `--`
fn is_valid_skill_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains("--")
}

/// Extract the `metadata:` block from YAML frontmatter.
///
/// Parses indented `key: value` lines following the `metadata:` line.
fn extract_metadata_block(frontmatter: &str) -> HashMap<String, String> {
    let mut result = HashMap::new();
    let mut in_metadata = false;

    for line in frontmatter.lines() {
        let trimmed = line.trim();
        if trimmed == "metadata:" || trimmed == "metadata: " {
            in_metadata = true;
            continue;
        }

        if in_metadata {
            // Check if this line is indented (part of metadata block)
            if line.starts_with(' ') || line.starts_with('\t') {
                if let Some(colon_pos) = trimmed.find(':') {
                    let key = trimmed[..colon_pos].trim().to_string();
                    let val = trimmed[colon_pos + 1..].trim().to_string();
                    // Remove surrounding quotes from value
                    let val = if (val.starts_with('"') && val.ends_with('"'))
                        || (val.starts_with('\'') && val.ends_with('\''))
                    {
                        val[1..val.len() - 1].to_string()
                    } else {
                        val
                    };
                    if !key.is_empty() {
                        result.insert(key, val);
                    }
                }
            } else {
                // Non-indented line means we left the metadata block
                in_metadata = false;
            }
        }
    }

    result
}
