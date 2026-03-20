use super::helpers::current_username;
use super::types::{Endpoint, SshUrl, DEFAULT_HOST, DEFAULT_PORT};

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

pub(crate) fn split_host_port_with_default(input: &str) -> Result<(String, String), String> {
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
