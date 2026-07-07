//! Pi Agent MCP sync and import module.
//!
//! Pi stores MCP servers in `~/.pi/agent/mcp.json` with a top-level
//! `mcpServers` object, using the same server spec shape as Claude.

use serde_json::Value;
use std::collections::HashMap;
use std::fs;

use crate::app_config::{McpApps, McpServer, MultiAppConfig};
use crate::error::AppError;
use crate::pi_config;

use super::validation::validate_server_spec;

fn should_sync_pi_mcp() -> bool {
    pi_config::get_pi_dir().exists() || pi_config::get_pi_mcp_path().exists()
}

fn read_pi_mcp_root() -> Result<serde_json::Map<String, Value>, AppError> {
    let path = pi_config::get_pi_mcp_path();
    if !path.exists() {
        return Ok(serde_json::Map::new());
    }

    let content = fs::read_to_string(&path).map_err(|e| AppError::io(&path, e))?;
    if content.trim().is_empty() {
        return Ok(serde_json::Map::new());
    }

    let value: Value = serde_json::from_str(&content)
        .map_err(|e| AppError::McpValidation(format!("Failed to parse Pi mcp.json: {e}")))?;
    Ok(value.as_object().cloned().unwrap_or_default())
}

fn read_pi_mcp_servers_map() -> Result<HashMap<String, Value>, AppError> {
    let root = read_pi_mcp_root()?;
    let Some(servers) = root.get("mcpServers").and_then(Value::as_object) else {
        return Ok(HashMap::new());
    };

    Ok(servers
        .iter()
        .map(|(id, spec)| (id.clone(), spec.clone()))
        .collect())
}

fn write_pi_mcp_servers_map(servers: &HashMap<String, Value>) -> Result<(), AppError> {
    let mut root = read_pi_mcp_root()?;
    let mut server_obj = serde_json::Map::new();

    let mut ids: Vec<_> = servers.keys().cloned().collect();
    ids.sort();
    for id in ids {
        if let Some(spec) = servers.get(&id) {
            server_obj.insert(id, spec.clone());
        }
    }

    root.insert("mcpServers".to_string(), Value::Object(server_obj));

    let path = pi_config::get_pi_mcp_path();
    let text = serde_json::to_string_pretty(&Value::Object(root))
        .map_err(|e| AppError::Config(format!("Failed to serialize Pi mcp.json: {e}")))?;
    crate::config::write_text_file(&path, &text)
}

pub fn sync_single_server_to_pi(
    _config: &MultiAppConfig,
    id: &str,
    server_spec: &Value,
) -> Result<(), AppError> {
    if !should_sync_pi_mcp() {
        return Ok(());
    }

    validate_server_spec(server_spec)?;
    let mut servers = read_pi_mcp_servers_map()?;
    servers.insert(id.to_string(), server_spec.clone());
    write_pi_mcp_servers_map(&servers)
}

pub fn remove_server_from_pi(id: &str) -> Result<(), AppError> {
    if !should_sync_pi_mcp() {
        return Ok(());
    }

    let mut servers = read_pi_mcp_servers_map()?;
    servers.remove(id);
    write_pi_mcp_servers_map(&servers)
}

pub fn import_from_pi(config: &mut MultiAppConfig) -> Result<usize, AppError> {
    let mcp_map = read_pi_mcp_servers_map()?;
    if mcp_map.is_empty() {
        return Ok(0);
    }

    let servers = config.mcp.servers.get_or_insert_with(HashMap::new);
    let mut changed = 0;
    let mut errors = Vec::new();

    for (id, spec) in mcp_map {
        if let Err(e) = validate_server_spec(&spec) {
            log::warn!("Skip invalid Pi MCP server '{id}': {e}");
            errors.push(format!("{id}: {e}"));
            continue;
        }

        if let Some(existing) = servers.get_mut(&id) {
            if !existing.apps.pi {
                existing.apps.pi = true;
                changed += 1;
                log::info!("MCP server '{id}' enabled for Pi Agent");
            }
        } else {
            servers.insert(
                id.clone(),
                McpServer {
                    id: id.clone(),
                    name: id.clone(),
                    server: spec,
                    apps: McpApps {
                        pi: true,
                        ..Default::default()
                    },
                    description: None,
                    homepage: None,
                    docs: None,
                    tags: Vec::new(),
                },
            );
            changed += 1;
            log::info!("Imported new Pi MCP server '{id}'");
        }
    }

    if !errors.is_empty() {
        log::warn!(
            "Pi MCP import completed with {} failures: {:?}",
            errors.len(),
            errors
        );
    }

    Ok(changed)
}
