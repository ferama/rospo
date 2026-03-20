use std::fs;
use std::path::Path;

use internal_russh_forked_ssh_key::PublicKey;

use super::endpoint::split_host_port_with_default;
use super::helpers::write_file_0600;
use super::types::DEFAULT_PORT;

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
