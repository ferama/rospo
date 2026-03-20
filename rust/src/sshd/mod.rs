use std::collections::HashMap;
use std::sync::Arc;

use russh::server::Msg;
use russh::{Channel, ChannelId};
use tokio::sync::{mpsc, Mutex};

use crate::config::SshdConf;

mod auth;
mod process;
mod server;
mod sftp;
#[cfg(windows)]
mod windows_pty;

pub use server::run;

const BANNER: &str = "\n .---------------.\n | 🐸 rospo sshd |\n .---------------.\n\n";

#[derive(Debug, Clone)]
pub struct ServerOptions {
    pub server_key: String,
    pub authorized_keys: Vec<String>,
    pub authorized_password: String,
    pub listen_address: String,
    pub disable_shell: bool,
    pub disable_banner: bool,
    pub disable_auth: bool,
    pub disable_sftp_subsystem: bool,
    pub disable_tunnelling: bool,
    pub shell_executable: String,
}

impl ServerOptions {
    pub fn from_conf(conf: &SshdConf) -> Self {
        Self {
            server_key: conf.server_key.clone(),
            authorized_keys: conf.authorized_keys.clone(),
            authorized_password: conf.authorized_password.clone(),
            listen_address: conf.listen_address.clone(),
            disable_shell: conf.disable_shell,
            disable_banner: conf.disable_banner,
            disable_auth: conf.disable_auth,
            disable_sftp_subsystem: conf.disable_sftp_subsystem,
            disable_tunnelling: conf.disable_tunnelling,
            shell_executable: conf.shell_executable.clone(),
        }
    }
}

#[derive(Clone)]
struct SharedState {
    options: ServerOptions,
    channels: Arc<Mutex<HashMap<ChannelId, SessionChannelState>>>,
    forwards: Arc<Mutex<HashMap<String, tokio::task::JoinHandle<()>>>>,
}

#[derive(Default)]
struct SessionChannelState {
    channel: Option<Channel<Msg>>,
    env: HashMap<String, String>,
    io: Option<ChannelIo>,
    pty: Option<PtyRequest>,
}

#[derive(Clone)]
enum ChannelIo {
    Stream(mpsc::UnboundedSender<Vec<u8>>),
    Pty(PtyHandle),
}

#[derive(Clone)]
struct PtyHandle {
    stdin_tx: mpsc::UnboundedSender<Vec<u8>>,
    resize_tx: mpsc::UnboundedSender<(u32, u32)>,
}

#[derive(Debug, Clone, Copy)]
struct PtyRequest {
    cols: u32,
    rows: u32,
}

#[derive(Clone)]
struct Server {
    state: SharedState,
}

#[derive(Clone)]
struct Handler {
    state: SharedState,
}
