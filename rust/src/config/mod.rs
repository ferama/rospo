use std::fs;
use std::path::Path;

use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    #[serde(rename = "sshclient")]
    pub ssh_client: Option<SshClientConf>,
    #[serde(rename = "tunnel")]
    pub tunnel: Option<Vec<TunnelConf>>,
    #[serde(rename = "sshd")]
    pub sshd: Option<SshdConf>,
    #[serde(rename = "socksproxy")]
    pub socks_proxy: Option<SocksProxyConf>,
    #[serde(rename = "dnsproxy")]
    pub dns_proxy: Option<DnsProxyConf>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct JumpHostConf {
    #[serde(rename = "uri")]
    pub uri: String,
    #[serde(rename = "identity", default)]
    pub identity: String,
    #[serde(rename = "password", default)]
    pub password: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SshClientConf {
    #[serde(rename = "identity", default)]
    pub identity: String,
    #[serde(rename = "password", default)]
    pub password: String,
    #[serde(rename = "known_hosts", default)]
    pub known_hosts: String,
    #[serde(rename = "server", default)]
    pub server: String,
    #[serde(rename = "insecure", default, deserialize_with = "deserialize_bool_compat")]
    pub insecure: bool,
    #[serde(rename = "quiet", default, deserialize_with = "deserialize_bool_compat")]
    pub quiet: bool,
    #[serde(rename = "jump_hosts", default)]
    pub jump_hosts: Vec<JumpHostConf>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TunnelConf {
    #[serde(rename = "remote", default)]
    pub remote: String,
    #[serde(rename = "local", default)]
    pub local: String,
    #[serde(rename = "forward", default, deserialize_with = "deserialize_bool_compat")]
    pub forward: bool,
    #[serde(rename = "sshclient")]
    pub ssh_client: Option<SshClientConf>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SocksProxyConf {
    #[serde(rename = "listen_address", default)]
    pub listen_address: String,
    #[serde(rename = "sshclient")]
    pub ssh_client: Option<SshClientConf>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DnsProxyConf {
    #[serde(rename = "listen_address", default)]
    pub listen_address: String,
    #[serde(rename = "remote_dns_address")]
    pub remote_dns_address: Option<String>,
    #[serde(rename = "sshclient")]
    pub ssh_client: Option<SshClientConf>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SshdConf {
    #[serde(rename = "server_key", default)]
    pub server_key: String,
    #[serde(rename = "authorized_keys", default)]
    pub authorized_keys: Vec<String>,
    #[serde(rename = "authorized_password", default)]
    pub authorized_password: String,
    #[serde(rename = "listen_address", default)]
    pub listen_address: String,
    #[serde(rename = "disable_shell", default, deserialize_with = "deserialize_bool_compat")]
    pub disable_shell: bool,
    #[serde(rename = "disable_banner", default, deserialize_with = "deserialize_bool_compat")]
    pub disable_banner: bool,
    #[serde(rename = "disable_auth", default, deserialize_with = "deserialize_bool_compat")]
    pub disable_auth: bool,
    #[serde(rename = "disable_sftp_subsystem", default, deserialize_with = "deserialize_bool_compat")]
    pub disable_sftp_subsystem: bool,
    #[serde(rename = "disable_tunnelling", default, deserialize_with = "deserialize_bool_compat")]
    pub disable_tunnelling: bool,
    #[serde(rename = "shell_executable", default)]
    pub shell_executable: String,
}

pub fn load_config(input: &str) -> Result<Config, serde_yaml::Error> {
    serde_yaml::from_str(input)
}

pub fn load_config_file(path: &Path) -> Result<Config, String> {
    let content = fs::read_to_string(path).map_err(|err| err.to_string())?;
    load_config(&content).map_err(|err| err.to_string())
}

fn deserialize_bool_compat<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum BoolCompat {
        Bool(bool),
        String(String),
    }

    // Go/YAML users often rely on YAML 1.1 spellings like "yes"/"no" even though serde_yaml
    // normalizes booleans more strictly when a field is typed as bool.
    match BoolCompat::deserialize(deserializer)? {
        BoolCompat::Bool(value) => Ok(value),
        BoolCompat::String(value) => match value.trim().to_ascii_lowercase().as_str() {
            "y" | "yes" | "true" | "on" => Ok(true),
            "n" | "no" | "false" | "off" => Ok(false),
            other => Err(serde::de::Error::custom(format!(
                "invalid boolean value {other:?}, expected one of yes/no/true/false/on/off"
            ))),
        },
    }
}
