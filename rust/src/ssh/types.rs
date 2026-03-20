use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::fs;

use internal_russh_forked_ssh_key::PublicKey;
use russh::client;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Connecting,
    Connected,
    Closed,
}

#[derive(Debug, Clone)]
pub struct JumpHostOptions {
    pub username: String,
    pub host: String,
    pub port: u16,
    pub identity: PathBuf,
    pub password: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ClientOptions {
    pub username: String,
    pub host: String,
    pub port: u16,
    pub identity: PathBuf,
    pub known_hosts: PathBuf,
    pub password: Option<String>,
    pub insecure: bool,
    pub quiet: bool,
    pub jump_hosts: Vec<JumpHostOptions>,
}

pub struct ForwardedTcpIp {
    pub channel: russh::Channel<russh::client::Msg>,
    pub connected_address: String,
    pub connected_port: u32,
    pub originator_address: String,
    pub originator_port: u32,
}

#[derive(Clone, Default)]
pub(crate) struct KeyGrabber {
    pub(crate) server_key: Arc<Mutex<Option<PublicKey>>>,
}

#[derive(Clone)]
pub(crate) struct ClientHandler {
    pub(crate) options: ClientOptions,
    pub(crate) forwarded_sender: Option<mpsc::UnboundedSender<ForwardedTcpIp>>,
    pub(crate) last_error: Arc<Mutex<Option<String>>>,
}

impl client::Handler for KeyGrabber {
    type Error = russh::Error;

    async fn check_server_key(&mut self, server_public_key: &PublicKey) -> Result<bool, Self::Error> {
        if let Ok(mut slot) = self.server_key.lock() {
            *slot = Some(server_public_key.clone());
        }
        Ok(true)
    }
}

impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn auth_banner(&mut self, banner: &str, _session: &mut client::Session) -> Result<(), Self::Error> {
        if !self.options.quiet {
            print!("{banner}");
        }
        Ok(())
    }

    async fn check_server_key(&mut self, server_public_key: &PublicKey) -> Result<bool, Self::Error> {
        if self.options.insecure {
            return Ok(true);
        }

        if let Some(parent) = self.options.known_hosts.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if !self.options.known_hosts.exists() {
            let _ = std::fs::write(&self.options.known_hosts, "");
        }
        if let Err(err) = validate_known_hosts_file(&self.options.known_hosts) {
            if let Ok(mut slot) = self.last_error.lock() {
                *slot = Some(format!(
                    "error while parsing 'known_hosts' file: {}: {err}",
                    self.options.known_hosts.display()
                ));
            }
            return Ok(false);
        }

        match russh::keys::check_known_hosts_path(
            &self.options.host,
            self.options.port,
            server_public_key,
            &self.options.known_hosts,
        ) {
            Ok(true) => Ok(true),
            Ok(false) => {
                if let Ok(mut slot) = self.last_error.lock() {
                    *slot = Some(format!(
                        "ERROR: the host '{}:{}' is not trusted. If it is trusted instead,\n  please grab its pub key using the 'rospo grabpubkey' command",
                        self.options.host, self.options.port
                    ));
                }
                Ok(false)
            }
            Err(err) => {
                if let Ok(mut slot) = self.last_error.lock() {
                    *slot = Some(format!(
                        "error while parsing 'known_hosts' file: {}: {err}",
                        self.options.known_hosts.display()
                    ));
                }
                Ok(false)
            }
        }
    }

    async fn disconnected(
        &mut self,
        reason: client::DisconnectReason<Self::Error>,
    ) -> Result<(), Self::Error> {
        if let Ok(mut slot) = self.last_error.lock() {
            *slot = Some(format!("server disconnected: {reason:?}"));
        }
        Ok(())
    }

    async fn server_channel_open_forwarded_tcpip(
        &mut self,
        channel: russh::Channel<russh::client::Msg>,
        connected_address: &str,
        connected_port: u32,
        originator_address: &str,
        originator_port: u32,
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        if let Some(sender) = &self.forwarded_sender {
            let _ = sender.send(ForwardedTcpIp {
                channel,
                connected_address: connected_address.to_string(),
                connected_port,
                originator_address: originator_address.to_string(),
                originator_port,
            });
        }
        Ok(())
    }
}

fn validate_known_hosts_file(path: &PathBuf) -> Result<(), String> {
    let content = fs::read_to_string(path).map_err(|err| err.to_string())?;
    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        let Some(_host) = parts.next() else {
            continue;
        };
        let Some(key_type) = parts.next() else {
            return Err(format!("invalid entry at line {}", idx + 1));
        };
        let Some(key_body) = parts.next() else {
            return Err(format!("invalid entry at line {}", idx + 1));
        };
        let key_line = format!("{key_type} {key_body}");
        if PublicKey::from_openssh(&key_line).is_err() {
            return Err(format!("invalid entry at line {}", idx + 1));
        }
    }
    Ok(())
}
