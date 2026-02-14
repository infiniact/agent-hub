use std::path::PathBuf;

use crate::db::migrations::{get_agents_dir, get_base_dir};
use crate::error::{AppError, AppResult};
use crate::models::agent::{AgentConfig, AgentSkill};

/// Write an agent's configuration to a markdown file with YAML frontmatter.
pub fn write_agent_md(agent: &AgentConfig) -> AppResult<PathBuf> {
    let agents_dir = get_agents_dir();
    std::fs::create_dir_all(&agents_dir)
        .map_err(|e| AppError::Io(e))?;

    let file_path = agents_dir.join(format!("{}.md", agent.id));

    // Parse capabilities from JSON
    let capabilities: Vec<String> = serde_json::from_str(&agent.capabilities_json)
        .unwrap_or_default();
    let caps_str = capabilities
        .iter()
        .map(|c| format!("\"{}\"", c))
        .collect::<Vec<_>>()
        .join(", ");

    // Parse acp_args from JSON
    let acp_args: Vec<String> = agent
        .acp_args_json
        .as_ref()
        .and_then(|j| serde_json::from_str(j).ok())
        .unwrap_or_default();
    let args_str = acp_args
        .iter()
        .map(|a| format!("\"{}\"", a))
        .collect::<Vec<_>>()
        .join(", ");

    let content = format!(
        r#"---
id: "{id}"
name: "{name}"
icon: "{icon}"
description: "{description}"
model: "{model}"
temperature: {temperature}
max_tokens: {max_tokens}
max_concurrency: {max_concurrency}
capabilities: [{capabilities}]
skills_json: {skills_json}
acp_command: "{acp_command}"
acp_args: [{acp_args}]
is_control_hub: {is_control_hub}
is_enabled: {is_enabled}
---

{system_prompt}
"#,
        id = agent.id,
        name = agent.name.replace('"', "\\\""),
        icon = agent.icon,
        description = agent.description.replace('"', "\\\""),
        model = agent.model,
        temperature = agent.temperature,
        max_tokens = agent.max_tokens,
        max_concurrency = agent.max_concurrency,
        capabilities = caps_str,
        skills_json = agent.skills_json,
        acp_command = agent.acp_command.as_deref().unwrap_or(""),
        acp_args = args_str,
        is_control_hub = agent.is_control_hub,
        is_enabled = agent.is_enabled,
        system_prompt = agent.system_prompt,
    );

    std::fs::write(&file_path, content)
        .map_err(|e| AppError::Io(e))?;

    Ok(file_path)
}

/// Read an agent configuration from a markdown file with YAML frontmatter.
pub fn read_agent_md(path: &str) -> AppResult<AgentConfig> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| AppError::Io(e))?;

    let (frontmatter, body) = parse_frontmatter(&content)
        .ok_or_else(|| AppError::Internal("Invalid markdown frontmatter".into()))?;

    // Parse frontmatter fields manually
    let id = extract_field(&frontmatter, "id").unwrap_or_default();
    let name = extract_field(&frontmatter, "name").unwrap_or_default();
    let icon = extract_field(&frontmatter, "icon").unwrap_or_else(|| "code".into());
    let description = extract_field(&frontmatter, "description").unwrap_or_default();
    let model = extract_field(&frontmatter, "model").unwrap_or_else(|| "gpt-4-turbo".into());
    let temperature: f64 = extract_field(&frontmatter, "temperature")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.7);
    let max_tokens: i64 = extract_field(&frontmatter, "max_tokens")
        .and_then(|v| v.parse().ok())
        .unwrap_or(4096);
    let max_concurrency: i64 = extract_field(&frontmatter, "max_concurrency")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);
    let is_control_hub: bool = extract_field(&frontmatter, "is_control_hub")
        .and_then(|v| v.parse().ok())
        .unwrap_or(false);
    let is_enabled: bool = extract_field(&frontmatter, "is_enabled")
        .and_then(|v| v.parse().ok())
        .unwrap_or(true);
    let acp_command = extract_field(&frontmatter, "acp_command")
        .filter(|s| !s.is_empty());

    // Parse array fields
    let capabilities_str = extract_array_field(&frontmatter, "capabilities");
    let capabilities_json = serde_json::to_string(&capabilities_str).unwrap_or_else(|_| "[]".into());

    let skills_json = extract_field(&frontmatter, "skills_json").unwrap_or_else(|| "[]".into());

    let acp_args = extract_array_field(&frontmatter, "acp_args");
    let acp_args_json = if acp_args.is_empty() {
        None
    } else {
        Some(serde_json::to_string(&acp_args).unwrap_or_else(|_| "[]".into()))
    };

    Ok(AgentConfig {
        id,
        name,
        icon,
        description,
        status: "Idle".into(),
        execution_mode: "RunNow".into(),
        model,
        temperature,
        max_tokens,
        system_prompt: body.to_string(),
        capabilities_json,
        skills_json,
        acp_command,
        acp_args_json,
        is_control_hub,
        md_file_path: Some(path.to_string()),
        max_concurrency,
        available_models_json: None,
        is_enabled,
        disabled_reason: None,
        created_at: String::new(),
        updated_at: String::new(),
    })
}

/// Sync all agents from DB to markdown files.
pub fn sync_all_to_md(agents: &[AgentConfig]) -> AppResult<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for agent in agents {
        let path = write_agent_md(agent)?;
        paths.push(path);
    }
    Ok(paths)
}

/// Delete the markdown file for an agent.
pub fn delete_agent_md(agent_id: &str) {
    let file_path = get_agents_dir().join(format!("{}.md", agent_id));
    let _ = std::fs::remove_file(file_path);
}

/// Write the agents registry file at ~/.iaagenthub/agents_registry.md
pub fn write_agents_registry(agents: &[AgentConfig]) -> AppResult<PathBuf> {
    let base_dir = get_base_dir();
    std::fs::create_dir_all(&base_dir)
        .map_err(|e| AppError::Io(e))?;

    let file_path = base_dir.join("agents_registry.md");

    let mut content = String::from("# Agents Registry\n\n");

    for agent in agents {
        // Skip disabled agents from the registry so the hub won't see them
        if !agent.is_enabled {
            continue;
        }

        let caps: Vec<String> = serde_json::from_str(&agent.capabilities_json)
            .unwrap_or_default();
        let caps_str = if caps.is_empty() {
            "none".to_string()
        } else {
            caps.join(", ")
        };

        content.push_str(&format!(
            "## Agent: {name}\n\
             - **ID**: {id}\n\
             - **Description**: {description}\n\
             - **Model**: {model}\n\
             - **Max Concurrency**: {max_concurrency}\n\
             - **Capabilities**: [{capabilities}]\n\
             - **Is Control Hub**: {is_control_hub}\n",
            name = agent.name,
            id = agent.id,
            description = if agent.description.is_empty() { "N/A" } else { &agent.description },
            model = agent.model,
            max_concurrency = agent.max_concurrency,
            capabilities = caps_str,
            is_control_hub = agent.is_control_hub,
        ));

        // Add Skills section
        let skills: Vec<AgentSkill> = serde_json::from_str(&agent.skills_json)
            .unwrap_or_default();
        if !skills.is_empty() {
            content.push_str("  **Skills**:\n");
            for skill in &skills {
                content.push_str(&format!(
                    "    - [{}] ({}) {}\n      Keywords: [{}]\n      Constraints: [{}]\n",
                    skill.id,
                    skill.skill_type,
                    skill.name,
                    skill.task_keywords.join(", "),
                    skill.constraints.join(", "),
                ));
            }
        }

        content.push('\n');
    }

    std::fs::write(&file_path, content)
        .map_err(|e| AppError::Io(e))?;

    Ok(file_path)
}

/// Read the agents registry file as raw text.
pub fn read_agents_registry() -> AppResult<String> {
    let file_path = get_base_dir().join("agents_registry.md");
    std::fs::read_to_string(&file_path)
        .map_err(|e| AppError::Io(e))
}

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
            // Remove surrounding quotes
            if (val.starts_with('"') && val.ends_with('"'))
                || (val.starts_with('\'') && val.ends_with('\''))
            {
                return Some(val[1..val.len() - 1].replace("\\\"", "\""));
            }
            return Some(val.to_string());
        }
    }
    None
}

fn extract_array_field(frontmatter: &str, key: &str) -> Vec<String> {
    for line in frontmatter.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix(&format!("{key}:")) {
            let val = rest.trim();
            if val.starts_with('[') && val.ends_with(']') {
                let inner = &val[1..val.len() - 1];
                return inner
                    .split(',')
                    .map(|s| {
                        let s = s.trim();
                        if (s.starts_with('"') && s.ends_with('"'))
                            || (s.starts_with('\'') && s.ends_with('\''))
                        {
                            s[1..s.len() - 1].to_string()
                        } else {
                            s.to_string()
                        }
                    })
                    .filter(|s| !s.is_empty())
                    .collect();
            }
        }
    }
    Vec::new()
}
