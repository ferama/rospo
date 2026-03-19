use std::env;
use std::fmt;
use std::fs;
use std::path::Path;

use internal_russh_forked_ssh_key::PublicKey;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshUrl {
    pub username: String,
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    pub host: String,
    pub port: u16,
}

impl fmt::Display for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.host, self.port)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeConfig {
    pub host: String,
    pub port: u16,
    pub host_name: String,
    pub user: String,
    pub identity_file: String,
    pub strict_host_key_checking: bool,
    pub user_known_hosts_file: String,
    pub proxy_jump: String,
}

pub const DEFAULT_PORT: u16 = 22;
pub const DEFAULT_HOST: &str = "127.0.0.1";

pub fn current_username() -> String {
    env::var("USER")
        .or_else(|_| env::var("USERNAME"))
        .unwrap_or_else(|_| "root".to_string())
}

pub fn current_home_dir() -> String {
    env::var("HOME")
        .or_else(|_| env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string())
}

pub fn expand_user_home(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        format!("{}/{}", current_home_dir(), rest)
    } else {
        path.to_string()
    }
}

pub fn parse_ssh_url(input: &str) -> Result<SshUrl, String> {
    let (username, host_port) = match input.split_once('@') {
        Some((user, rest)) => (user.to_string(), rest.to_string()),
        None => (current_username(), input.to_string()),
    };

    let (host_raw, port_raw) = split_host_port_with_default(&host_port)?;
    let host = normalize_host(host_raw);
    let port = port_raw.parse::<u16>().map_err(|err| err.to_string())?;

    Ok(SshUrl {
        username,
        host,
        port,
    })
}

pub fn new_endpoint(input: &str) -> Result<Endpoint, String> {
    let parsed = parse_ssh_url(input)?;
    Ok(Endpoint {
        host: parsed.host,
        port: parsed.port,
    })
}

fn split_host_port_with_default(input: &str) -> Result<(String, String), String> {
    match split_host_port(input) {
        Some(parts) => Ok(parts),
        None => {
            let with_default = format!("{input}:{DEFAULT_PORT}");
            split_host_port(&with_default).ok_or_else(|| format!("missing port in address: {input}"))
        }
    }
}

fn split_host_port(input: &str) -> Option<(String, String)> {
    if let Some(rest) = input.strip_prefix('[') {
        let end = rest.find(']')?;
        let host = &rest[..end];
        let remainder = &rest[end + 1..];
        let port = remainder.strip_prefix(':')?;
        return Some((host.to_string(), port.to_string()));
    }

    let idx = input.rfind(':')?;
    let host = &input[..idx];
    let port = &input[idx + 1..];
    Some((host.to_string(), port.to_string()))
}

fn normalize_host(host: String) -> String {
    if host.is_empty() {
        return DEFAULT_HOST.to_string();
    }
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host
    }
}

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
            if let Some(host) = current_host.take() {
                if host != "*" {
                    current.host = host;
                    nodes.push(current.clone());
                }
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

    if let Some(host) = current_host.take() {
        if host != "*" {
            current.host = host;
            nodes.push(current);
        }
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
    nodes.iter().find(|node| node.host == host).cloned()
}

pub fn write_file_0600(path: &Path, contents: &[u8]) -> Result<(), String> {
    fs::write(path, contents).map_err(|err| err.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let permissions = fs::Permissions::from_mode(0o600);
        fs::set_permissions(path, permissions).map_err(|err| err.to_string())?;
    }
    Ok(())
}

pub fn serialize_public_key(key: &PublicKey) -> Result<String, String> {
    key.to_openssh().map_err(|err| err.to_string())
}

pub fn add_host_key_to_known_hosts(host: &str, key: &PublicKey, known_hosts_path: &Path) -> Result<(), String> {
    if !known_hosts_path.exists() {
        write_file_0600(known_hosts_path, b"")?;
    }

    let (host_part, port) = split_host_port_with_default(host)?;
    let default_port = DEFAULT_PORT.to_string();
    let mut entry = format!("[{host_part}]:{port}");
    if !host_part.contains(':') && port == default_port {
        entry = host_part.clone();
    }

    let line = format!("{entry} {}\n", serialize_public_key(key)?);
    let mut existing = fs::read_to_string(known_hosts_path).unwrap_or_default();
    existing.push_str(&line);
    write_file_0600(known_hosts_path, existing.as_bytes())
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
