use std::fs;
use std::path::Path;

use super::helpers::{current_username, expand_user_home};
use super::types::{NodeConfig, DEFAULT_PORT};

pub fn parse_ssh_config_content(content: &str) -> Result<Vec<NodeConfig>, String> {
    let mut nodes = Vec::new();
    let mut current_host: Option<String> = None;
    let mut current = default_node_config();

    for raw_line in content.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let mut parts = trimmed.split_whitespace();
        let key = match parts.next() {
            Some(key) => key,
            None => continue,
        };
        let value = parts.collect::<Vec<_>>().join(" ");
        if value.is_empty() {
            continue;
        }

        if key.eq_ignore_ascii_case("Host") {
            // Keep the parser intentionally small: rospo only consumes exact Host sections and a
            // handful of fields that affect its connection model.
            if let Some(host) = current_host.take()
                && host != "*"
            {
                current.host = host;
                nodes.push(current.clone());
            }
            current = default_node_config();
            current_host = Some(value);
            continue;
        }

        if current_host.is_none() {
            continue;
        }

        match key.to_ascii_lowercase().as_str() {
            "hostname" => current.host_name = value,
            "port" => {
                current.port = value.parse::<u16>().map_err(|_| format!("invalid value for Port: {value}"))?
            }
            "user" => current.user = value,
            "identityfile" => current.identity_file = value,
            "userknownhostsfile" => current.user_known_hosts_file = value,
            "stricthostkeychecking" => {
                current.strict_host_key_checking = match value.to_ascii_lowercase().as_str() {
                    "no" | "false" => false,
                    "yes" | "true" => true,
                    _ => return Err(format!("invalid value for StrictHostKeyChecking: {value}")),
                };
            }
            "proxyjump" => current.proxy_jump = value,
            _ => {}
        }
    }

    if let Some(host) = current_host.take()
        && host != "*"
    {
        current.host = host;
        nodes.push(current);
    }

    Ok(nodes)
}

pub fn parse_ssh_config_file(path: &Path) -> Result<Vec<NodeConfig>, String> {
    let content = fs::read_to_string(path).map_err(|err| err.to_string())?;
    parse_ssh_config_content(&content)
}

pub fn parse_default_ssh_config() -> Result<Vec<NodeConfig>, String> {
    let path = expand_user_home("~/.ssh/config");
    parse_ssh_config_file(Path::new(&path))
}

pub fn get_ssh_config_host(path: &Path, host: &str) -> Result<Option<NodeConfig>, String> {
    let nodes = parse_ssh_config_file(path)?;
    Ok(get_host_conf(&nodes, host))
}

pub fn get_default_ssh_config_host(host: &str) -> Option<NodeConfig> {
    parse_default_ssh_config()
        .ok()
        .and_then(|nodes| get_host_conf(&nodes, host))
}

pub fn get_host_conf(nodes: &[NodeConfig], host: &str) -> Option<NodeConfig> {
    // Match Go's current behavior: only exact Host matches are considered, not wildcard
    // expansion or pattern matching.
    nodes.iter().find(|node| node.host == host).cloned()
}

fn default_node_config() -> NodeConfig {
    NodeConfig {
        host: String::new(),
        port: DEFAULT_PORT,
        host_name: String::new(),
        user: current_username(),
        identity_file: "~/.ssh/id_rsa".to_string(),
        strict_host_key_checking: true,
        user_known_hosts_file: "~/.ssh/known_hosts".to_string(),
        proxy_jump: String::new(),
    }
}
