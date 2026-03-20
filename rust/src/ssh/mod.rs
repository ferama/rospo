use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::logging::{Logger, GREEN};
use internal_russh_forked_ssh_key::PublicKey;
use russh::client;
use russh::{client::Handle, ChannelMsg, Disconnect, Pty};
use russh_sftp::client::SftpSession;
use tokio::io::{self, AsyncWriteExt};
use tokio::sync::mpsc;

#[cfg(unix)]
use nix::sys::termios::{self, SetArg, Termios};

const LOG: Logger = Logger::new("[SSHC] ", GREEN);

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
    LOG.log(format_args!("grabbing server public key from {}:{}", server.0, server.1));
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
        LOG.log(format_args!("trying to connect to remote server..."));
        if !options.identity.as_os_str().is_empty() {
            LOG.log(format_args!("using identity at {}", options.identity.display()));
        }
        if !options.insecure {
            LOG.log(format_args!("using known_hosts file at {}", options.known_hosts.display()));
        }
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

            LOG.log(format_args!("jump host connected: {}@{}:{}", hop.username, hop.host, hop.port));

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

        LOG.log(format_args!(
            "connected to {}@{}:{}",
            options.username, options.host, options.port
        ));

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

        let (cols, rows) = terminal_size().unwrap_or((80, 24));
        let term = std::env::var("TERM").unwrap_or_else(|_| "xterm-256color".to_string());
        let _ = channel
            .request_pty(
                true,
                &term,
                cols,
                rows,
                0,
                0,
                &[(Pty::ECHO, 1), (Pty::TTY_OP_ISPEED, 14400), (Pty::TTY_OP_OSPEED, 14400)],
            )
            .await;
        channel.request_shell(true).await.map_err(|err| err.to_string())?;
        drain_channel(&mut channel, true).await
    }

    pub async fn open_sftp(&mut self) -> Result<SftpSession, String> {
        let channel = self
            .handle
            .channel_open_session()
            .await
            .map_err(|err| err.to_string())?;
        channel
            .request_subsystem(true, "sftp")
            .await
            .map_err(|err| err.to_string())?;
        SftpSession::new(channel.into_stream())
            .await
            .map_err(|err| err.to_string())
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
        LOG.log(format_args!("disconnecting client"));
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
    let mut stdin_closed = false;
    let mut exit_status = None::<u32>;
    #[allow(unused_mut)]
    let mut terminal_guard = if interactive {
        TerminalModeGuard::activate().ok()
    } else {
        None
    };
    let (stdin_tx, mut stdin_rx) = mpsc::unbounded_channel::<Option<Vec<u8>>>();
    let _stdin_thread = if interactive {
        Some(spawn_stdin_reader(stdin_tx))
    } else {
        None
    };
    let mut resize_interval = tokio::time::interval(Duration::from_millis(100));
    let mut last_size = if interactive { terminal_size().ok() } else { None };

    loop {
        tokio::select! {
            stdin_msg = stdin_rx.recv(), if interactive && !stdin_closed => {
                match stdin_msg {
                    Some(Some(bytes)) => channel.data(bytes.as_slice()).await.map_err(|err| err.to_string())?,
                    Some(None) | None => {
                        stdin_closed = true;
                        let _ = channel.eof().await;
                    }
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
                        break;
                    }
                    ChannelMsg::Eof => {}
                    ChannelMsg::Close => break,
                    _ => {}
                }
            }
            _ = resize_interval.tick(), if interactive && terminal_guard.is_some() => {
                if let Ok((cols, rows)) = terminal_size() {
                    if last_size != Some((cols, rows)) {
                        last_size = Some((cols, rows));
                        let _ = channel.window_change(cols, rows, 0, 0).await;
                    }
                }
            }
        }
    }

    drop(terminal_guard.take());

    if let Some(code) = exit_status {
        Ok(code)
    } else {
        Err("channel closed without exit status".to_string())
    }
}

fn spawn_stdin_reader(stdin_tx: mpsc::UnboundedSender<Option<Vec<u8>>>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut stdin = std::io::stdin();
        let mut buf = [0u8; 1024];
        loop {
            match std::io::Read::read(&mut stdin, &mut buf) {
                Ok(0) => {
                    let _ = stdin_tx.send(None);
                    break;
                }
                Ok(n) => {
                    if stdin_tx.send(Some(buf[..n].to_vec())).is_err() {
                        break;
                    }
                }
                Err(_) => {
                    let _ = stdin_tx.send(None);
                    break;
                }
            }
        }
    })
}

#[cfg(unix)]
struct TerminalModeGuard {
    original: Termios,
}

#[cfg(unix)]
impl TerminalModeGuard {
    fn activate() -> Result<Self, String> {
        if unsafe { nix::libc::isatty(0) } != 1 {
            return Err("stdin is not a terminal".to_string());
        }
        let mut term = termios::tcgetattr(std::io::stdin()).map_err(|err| err.to_string())?;
        let original = term.clone();
        termios::cfmakeraw(&mut term);
        termios::tcsetattr(std::io::stdin(), SetArg::TCSANOW, &term).map_err(|err| err.to_string())?;
        Ok(Self { original })
    }
}

#[cfg(unix)]
impl Drop for TerminalModeGuard {
    fn drop(&mut self) {
        let _ = termios::tcsetattr(std::io::stdin(), SetArg::TCSANOW, &self.original);
    }
}

#[cfg(not(unix))]
struct TerminalModeGuard;

#[cfg(not(unix))]
impl TerminalModeGuard {
    fn activate() -> Result<Self, String> {
        Err("raw terminal mode not implemented".to_string())
    }
}

#[cfg(unix)]
fn terminal_size() -> Result<(u32, u32), String> {
    use std::mem::zeroed;

    let mut winsize: nix::libc::winsize = unsafe { zeroed() };
    let result = unsafe { nix::libc::ioctl(0, nix::libc::TIOCGWINSZ as _, &mut winsize) };
    if result == -1 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok((u32::from(winsize.ws_col), u32::from(winsize.ws_row)))
}

#[cfg(not(unix))]
fn terminal_size() -> Result<(u32, u32), String> {
    Err("terminal size not implemented".to_string())
}
