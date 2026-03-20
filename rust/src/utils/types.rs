use std::fmt;

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
