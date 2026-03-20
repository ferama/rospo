use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use internal_russh_forked_ssh_key::PublicKey;
use russh::client;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Connecting,
    Connected,
    Closed,
}

pub const KEEPALIVE_REQUEST: &str = "keepalive@rospo";
pub const CHECKALIVE_REQUEST: &str = "checkalive@rospo";

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

        match russh::keys::check_known_hosts_path(
            &self.options.host,
            self.options.port,
            server_public_key,
            &self.options.known_hosts,
        ) {
            Ok(found) => Ok(found),
            Err(_) => Ok(false),
        }
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
