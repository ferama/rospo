use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use internal_russh_forked_ssh_key::PublicKey;
use russh::client;
use russh::{client::Handle, ChannelMsg, Disconnect, Pty};
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
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
struct KeyGrabber {
    server_key: Arc<Mutex<Option<PublicKey>>>,
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

pub async fn fetch_server_public_key(server: (&str, u16)) -> Result<PublicKey, String> {
    let config = Arc::new(client::Config {
        inactivity_timeout: Some(Duration::from_secs(5)),
        ..Default::default()
    });
    let handler = KeyGrabber::default();
    let captured = handler.server_key.clone();
    let session = client::connect(config, server, handler)
        .await
        .map_err(|err| err.to_string())?;
    session
        .disconnect(russh::Disconnect::ByApplication, "", "English")
        .await
        .map_err(|err| err.to_string())?;

    captured
        .lock()
        .map_err(|_| "failed to acquire server key lock".to_string())?
        .clone()
        .ok_or_else(|| "server did not present a public key".to_string())
}

pub fn load_secret_key(path: &Path, password: Option<&str>) -> Result<Arc<russh::keys::PrivateKey>, String> {
    russh::keys::load_secret_key(path, password)
        .map(Arc::new)
        .map_err(|err| err.to_string())
}

#[derive(Clone)]
struct ClientHandler {
    options: ClientOptions,
    forwarded_sender: Option<mpsc::UnboundedSender<ForwardedTcpIp>>,
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

pub struct Session {
    handle: Handle<ClientHandler>,
    forwarded_receiver: mpsc::UnboundedReceiver<ForwardedTcpIp>,
}

impl Session {
    pub async fn connect(options: ClientOptions) -> Result<Self, String> {
        let (forwarded_sender, forwarded_receiver) = mpsc::unbounded_channel();
        let mut previous_handle = None::<Handle<ClientHandler>>;

        for hop in &options.jump_hosts {
            let handler = ClientHandler {
                options: ClientOptions {
                    username: hop.username.clone(),
                    host: hop.host.clone(),
                    port: hop.port,
                    identity: hop.identity.clone(),
                    known_hosts: options.known_hosts.clone(),
                    password: hop.password.clone(),
                    insecure: options.insecure,
                    quiet: true,
                    jump_hosts: Vec::new(),
                },
                forwarded_sender: None,
            };

            let mut handle = if let Some(previous) = previous_handle.take() {
                let channel = previous
                    .channel_open_direct_tcpip(
                        hop.host.clone(),
                        u32::from(hop.port),
                        "127.0.0.1",
                        0,
                    )
                    .await
                    .map_err(|err| err.to_string())?;
                client::connect_stream(build_client_config(), channel.into_stream(), handler)
                    .await
                    .map_err(|err| err.to_string())?
            } else {
                client::connect(build_client_config(), (hop.host.as_str(), hop.port), handler)
                    .await
                    .map_err(|err| err.to_string())?
            };

            authenticate_handle(
                &mut handle,
                &hop.username,
                &hop.identity,
                hop.password.as_deref(),
            )
            .await?;

            previous_handle = Some(handle);
        }

        let handler = ClientHandler {
            options: options.clone(),
            forwarded_sender: Some(forwarded_sender),
        };

        let mut handle = if let Some(previous) = previous_handle {
            let channel = previous
                .channel_open_direct_tcpip(
                    options.host.clone(),
                    u32::from(options.port),
                    "127.0.0.1",
                    0,
                )
                .await
                .map_err(|err| err.to_string())?;
            client::connect_stream(build_client_config(), channel.into_stream(), handler)
                .await
                .map_err(|err| err.to_string())?
        } else {
            client::connect(build_client_config(), (options.host.as_str(), options.port), handler)
                .await
                .map_err(|err| err.to_string())?
        };

        authenticate_handle(
            &mut handle,
            &options.username,
            &options.identity,
            options.password.as_deref(),
        )
        .await?;

        Ok(Self {
            handle,
            forwarded_receiver,
        })
    }

    pub async fn run_command(&mut self, command: &str) -> Result<u32, String> {
        let mut channel = self
            .handle
            .channel_open_session()
            .await
            .map_err(|err| err.to_string())?;
        channel.exec(true, command).await.map_err(|err| err.to_string())?;
        drain_channel(&mut channel, false).await
    }

    pub async fn run_shell(&mut self) -> Result<u32, String> {
        let mut channel = self
            .handle
            .channel_open_session()
            .await
            .map_err(|err| err.to_string())?;

        for env_name in [
            "LANG",
            "LANGUAGE",
            "LC_CTYPE",
            "LC_NUMERIC",
            "LC_TIME",
            "LC_COLLATE",
            "LC_MONETARY",
            "LC_MESSAGES",
            "LC_PAPER",
            "LC_NAME",
            "LC_ADDRESS",
            "LC_TELEPHONE",
            "LC_MEASUREMENT",
            "LC_IDENTIFICATION",
            "LC_ALL",
        ] {
            if let Ok(val) = std::env::var(env_name) {
                if !val.is_empty() {
                    let _ = channel.set_env(false, env_name, val).await;
                }
            }
        }

        let term = std::env::var("TERM").unwrap_or_else(|_| "xterm-256color".to_string());
        let _ = channel
            .request_pty(
                true,
                &term,
                80,
                24,
                0,
                0,
                &[(Pty::ECHO, 1), (Pty::TTY_OP_ISPEED, 14400), (Pty::TTY_OP_OSPEED, 14400)],
            )
            .await;
        channel.request_shell(true).await.map_err(|err| err.to_string())?;
        drain_channel(&mut channel, true).await
    }

    pub async fn open_direct_tcpip(
        &mut self,
        host: &str,
        port: u16,
        originator_address: &str,
        originator_port: u32,
    ) -> Result<russh::Channel<russh::client::Msg>, String> {
        self.handle
            .channel_open_direct_tcpip(host.to_string(), u32::from(port), originator_address.to_string(), originator_port)
            .await
            .map_err(|err| err.to_string())
    }

    pub async fn tcpip_forward(&mut self, host: &str, port: u16) -> Result<u32, String> {
        self.handle
            .tcpip_forward(host.to_string(), u32::from(port))
            .await
            .map_err(|err| err.to_string())
    }

    pub async fn cancel_tcpip_forward(&self, host: &str, port: u16) -> Result<(), String> {
        self.handle
            .cancel_tcpip_forward(host.to_string(), u32::from(port))
            .await
            .map_err(|err| err.to_string())
    }

    pub async fn send_ping(&self) -> Result<(), String> {
        self.handle.send_ping().await.map_err(|err| err.to_string())
    }

    pub async fn next_forwarded(&mut self) -> Option<ForwardedTcpIp> {
        self.forwarded_receiver.recv().await
    }

    pub async fn disconnect(&mut self) -> Result<(), String> {
        self.handle
            .disconnect(Disconnect::ByApplication, "", "English")
            .await
            .map_err(|err| err.to_string())
    }
}

fn build_client_config() -> Arc<client::Config> {
    Arc::new(client::Config {
        inactivity_timeout: None,
        keepalive_interval: Some(Duration::from_secs(5)),
        keepalive_max: 3,
        nodelay: true,
        ..Default::default()
    })
}

async fn authenticate_handle(
    handle: &mut Handle<ClientHandler>,
    username: &str,
    identity: &Path,
    password: Option<&str>,
) -> Result<(), String> {
    let mut authenticated = false;
    if let Ok(key) = load_secret_key(identity, None) {
        let auth = handle
            .authenticate_publickey(
                username.to_string(),
                russh::keys::PrivateKeyWithHashAlg::new(
                    key,
                    handle.best_supported_rsa_hash().await.map_err(|err| err.to_string())?.flatten(),
                ),
            )
            .await
            .map_err(|err| err.to_string())?;
        authenticated = auth.success();
    }

    if !authenticated {
        if let Some(password) = password {
            let auth = handle
                .authenticate_password(username.to_string(), password.to_string())
                .await
                .map_err(|err| err.to_string())?;
            authenticated = auth.success();
        }
    }

    if !authenticated {
        let auth = handle
            .authenticate_none(username.to_string())
            .await
            .map_err(|err| err.to_string())?;
        authenticated = auth.success();
    }

    if authenticated {
        Ok(())
    } else {
        Err("authentication failed".to_string())
    }
}

async fn drain_channel(channel: &mut russh::Channel<russh::client::Msg>, interactive: bool) -> Result<u32, String> {
    let mut stdout = io::stdout();
    let mut stderr = io::stderr();
    let mut stdin = io::stdin();
    let mut in_buf = [0u8; 1024];
    let mut stdin_closed = false;
    let mut exit_status = None::<u32>;

    loop {
        tokio::select! {
            result = stdin.read(&mut in_buf), if interactive && !stdin_closed => {
                match result {
                    Ok(0) => {
                        stdin_closed = true;
                        let _ = channel.eof().await;
                    }
                    Ok(n) => channel.data(&in_buf[..n]).await.map_err(|err| err.to_string())?,
                    Err(err) => return Err(err.to_string()),
                }
            }
            msg = channel.wait() => {
                let Some(msg) = msg else {
                    break;
                };
                match msg {
                    ChannelMsg::Data { data } => {
                        stdout.write_all(&data).await.map_err(|err| err.to_string())?;
                        stdout.flush().await.map_err(|err| err.to_string())?;
                    }
                    ChannelMsg::ExtendedData { data, .. } => {
                        stderr.write_all(&data).await.map_err(|err| err.to_string())?;
                        stderr.flush().await.map_err(|err| err.to_string())?;
                    }
                    ChannelMsg::ExitStatus { exit_status: code } => {
                        exit_status = Some(code);
                        if interactive && !stdin_closed {
                            let _ = channel.eof().await;
                        }
                    }
                    ChannelMsg::Eof => {}
                    ChannelMsg::Close => break,
                    _ => {}
                }
            }
        }
    }

    Ok(exit_status.unwrap_or(0))
}
