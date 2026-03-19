use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use internal_russh_forked_ssh_key::PublicKey as ParsedPublicKey;
use internal_russh_forked_ssh_key::{Algorithm, EcdsaCurve, LineEnding};
use p521::elliptic_curve::rand_core::OsRng;
use russh::server::{self, Auth, Msg, Server as _};
use russh::{Channel, ChannelId};
use russh_sftp::protocol::{Attrs, Data, File, FileAttributes, Handle, Name, OpenFlags, Status, StatusCode, Version};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
#[cfg(unix)]
use tokio::process::Child;
use tokio::process::{ChildStderr, ChildStdout, Command};
use tokio::sync::{mpsc, Mutex};

use crate::config::SshdConf;
use crate::utils::{current_home_dir, current_username, expand_user_home, write_file_0600};

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

pub async fn run(options: ServerOptions) -> Result<(), String> {
    if options.server_key.is_empty() {
        return Err("server_key is not set".to_string());
    }
    if options.listen_address.is_empty() {
        return Err("listen port can't be empty".to_string());
    }

    ensure_server_key(Path::new(&expand_user_home(&options.server_key))).await?;

    if !options.disable_auth {
        let authorized = load_authorized_keys(&options.authorized_keys).await?;
        if authorized.is_empty() && options.authorized_password.is_empty() {
            return Err(
                "failed to load authorized_keys\n\nyou need an authorized_keys source or an authorized password\n"
                    .to_string(),
            );
        }
    }

    let private_key = russh::keys::load_secret_key(expand_user_home(&options.server_key), None)
        .map_err(|err| err.to_string())?;
    let config = Arc::new(server::Config {
        auth_rejection_time: Duration::from_secs(1),
        auth_rejection_time_initial: Some(Duration::from_secs(0)),
        inactivity_timeout: None,
        keepalive_interval: Some(Duration::from_secs(5)),
        keepalive_max: 3,
        nodelay: true,
        keys: vec![private_key],
        ..Default::default()
    });

    let state = SharedState {
        options,
        channels: Arc::new(Mutex::new(HashMap::new())),
        forwards: Arc::new(Mutex::new(HashMap::new())),
    };
    let mut server = Server { state };
    let listener = TcpListener::bind(&server.state.options.listen_address)
        .await
        .map_err(|err| err.to_string())?;

    server
        .run_on_socket(config, &listener)
        .await
        .map_err(|err| err.to_string())
}

impl server::Server for Server {
    type Handler = Handler;

    fn new_client(&mut self, _peer_addr: Option<std::net::SocketAddr>) -> Self::Handler {
        Handler {
            state: self.state.clone(),
        }
    }

    fn handle_session_error(&mut self, _error: <Self::Handler as server::Handler>::Error) {}
}

impl server::Handler for Handler {
    type Error = russh::Error;

    async fn auth_none(&mut self, _user: &str) -> Result<Auth, Self::Error> {
        if self.state.options.disable_auth {
            Ok(Auth::Accept)
        } else {
            Ok(Auth::reject())
        }
    }

    async fn auth_password(&mut self, _user: &str, password: &str) -> Result<Auth, Self::Error> {
        if self.state.options.disable_auth {
            return Ok(Auth::Accept);
        }
        if !self.state.options.authorized_password.is_empty()
            && password == self.state.options.authorized_password
        {
            return Ok(Auth::Accept);
        }
        Ok(Auth::reject())
    }

    async fn auth_publickey_offered(
        &mut self,
        _user: &str,
        public_key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<Auth, Self::Error> {
        if self.state.options.disable_auth {
            return Ok(Auth::Accept);
        }
        if is_authorized_key(&self.state.options.authorized_keys, public_key).await {
            Ok(Auth::Accept)
        } else {
            Ok(Auth::reject())
        }
    }

    async fn auth_publickey(
        &mut self,
        _user: &str,
        public_key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<Auth, Self::Error> {
        if self.state.options.disable_auth {
            return Ok(Auth::Accept);
        }
        if is_authorized_key(&self.state.options.authorized_keys, public_key).await {
            Ok(Auth::Accept)
        } else {
            Ok(Auth::reject())
        }
    }

    async fn authentication_banner(&mut self) -> Result<Option<String>, Self::Error> {
        if cfg!(windows) || self.state.options.disable_banner {
            Ok(None)
        } else {
            Ok(Some(BANNER.to_string()))
        }
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        _session: &mut server::Session,
    ) -> Result<bool, Self::Error> {
        self.state.channels.lock().await.insert(
            channel.id(),
            SessionChannelState {
                channel: Some(channel),
                env: HashMap::new(),
                io: None,
                pty: None,
            },
        );
        Ok(true)
    }

    async fn channel_open_direct_tcpip(
        &mut self,
        channel: Channel<Msg>,
        host_to_connect: &str,
        port_to_connect: u32,
        _originator_address: &str,
        _originator_port: u32,
        _session: &mut server::Session,
    ) -> Result<bool, Self::Error> {
        if self.state.options.disable_tunnelling {
            return Ok(false);
        }

        let stream = match TcpStream::connect((host_to_connect, port_to_connect as u16)).await {
            Ok(stream) => stream,
            Err(_) => return Ok(false),
        };

        tokio::spawn(async move {
            let mut ssh_stream = channel.into_stream();
            let mut stream = stream;
            let _ = tokio::io::copy_bidirectional(&mut ssh_stream, &mut stream).await;
        });
        Ok(true)
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        _session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        if let Some(channel_io) = self
            .state
            .channels
            .lock()
            .await
            .get(&channel)
            .and_then(|state| state.io.clone())
        {
            match channel_io {
                ChannelIo::Stream(stdin_tx) => {
                    let _ = stdin_tx.send(data.to_vec());
                }
                ChannelIo::Pty(pty) => {
                    let _ = pty.stdin_tx.send(data.to_vec());
                }
            }
        }
        Ok(())
    }

    async fn channel_eof(
        &mut self,
        channel: ChannelId,
        _session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        if let Some(state) = self.state.channels.lock().await.get_mut(&channel) {
            state.io = None;
        }
        Ok(())
    }

    async fn channel_close(
        &mut self,
        channel: ChannelId,
        _session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.state.channels.lock().await.remove(&channel);
        Ok(())
    }

    async fn pty_request(
        &mut self,
        channel: ChannelId,
        _term: &str,
        col_width: u32,
        row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _modes: &[(russh::Pty, u32)],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        if self.state.options.disable_shell {
            session.channel_failure(channel)?;
        } else {
            if let Some(state) = self.state.channels.lock().await.get_mut(&channel) {
                state.pty = Some(PtyRequest {
                    cols: col_width,
                    rows: row_height,
                });
            }
            session.channel_success(channel)?;
        }
        Ok(())
    }

    async fn env_request(
        &mut self,
        channel: ChannelId,
        variable_name: &str,
        variable_value: &str,
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        if let Some(state) = self.state.channels.lock().await.get_mut(&channel) {
            state
                .env
                .insert(variable_name.to_string(), variable_value.to_string());
            session.channel_success(channel)?;
        } else {
            session.channel_failure(channel)?;
        }
        Ok(())
    }

    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        if self.state.options.disable_shell {
            session.channel_failure(channel)?;
            return Ok(());
        }
        let env = self
            .state
            .channels
            .lock()
            .await
            .get(&channel)
            .map(|state| (state.env.clone(), state.pty))
            .unwrap_or_default();
        session.channel_success(channel)?;
        spawn_shell(channel, session.handle(), self.state.clone(), env.0, env.1, None).await?;
        Ok(())
    }

    async fn exec_request(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        if self.state.options.disable_shell {
            session.channel_failure(channel)?;
            return Ok(());
        }
        let env = self
            .state
            .channels
            .lock()
            .await
            .get(&channel)
            .map(|state| (state.env.clone(), state.pty))
            .unwrap_or_default();
        session.channel_success(channel)?;
        spawn_shell(
            channel,
            session.handle(),
            self.state.clone(),
            env.0,
            env.1,
            Some(String::from_utf8_lossy(data).into_owned()),
        )
        .await?;
        Ok(())
    }

    async fn subsystem_request(
        &mut self,
        channel: ChannelId,
        name: &str,
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        if name != "sftp" || self.state.options.disable_sftp_subsystem {
            session.channel_failure(channel)?;
            return Ok(());
        }

        let session_channel = self
            .state
            .channels
            .lock()
            .await
            .get_mut(&channel)
            .and_then(|state| state.channel.take());
        let Some(session_channel) = session_channel else {
            session.channel_failure(channel)?;
            return Ok(());
        };

        session.channel_success(channel)?;
        russh_sftp::server::run(session_channel.into_stream(), SftpServer::default()).await;
        Ok(())
    }

    async fn window_change_request(
        &mut self,
        channel: ChannelId,
        col_width: u32,
        row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        if let Some(state) = self.state.channels.lock().await.get_mut(&channel) {
            state.pty = Some(PtyRequest {
                cols: col_width,
                rows: row_height,
            });
            if let Some(ChannelIo::Pty(pty)) = state.io.clone() {
                let _ = pty.resize_tx.send((col_width, row_height));
            }
        }
        session.channel_success(channel)?;
        Ok(())
    }

    async fn tcpip_forward(
        &mut self,
        address: &str,
        port: &mut u32,
        session: &mut server::Session,
    ) -> Result<bool, Self::Error> {
        if self.state.options.disable_tunnelling {
            return Ok(false);
        }

        let bind_addr = normalize_bind_address(address, *port as u16);
        let listener = match TcpListener::bind(&bind_addr).await {
            Ok(listener) => listener,
            Err(_) => return Ok(false),
        };
        let local_addr = listener.local_addr()?;
        *port = local_addr.port() as u32;
        let listen_key = forward_key(address, *port);
        let handle = session.handle();
        let connected_address = address.to_string();
        let connected_port = *port;
        let task = tokio::spawn(async move {
            while let Ok((mut inbound, origin)) = listener.accept().await {
                let origin_address = match origin.ip() {
                    std::net::IpAddr::V4(ip) => ip.to_string(),
                    std::net::IpAddr::V6(ip) => ip.to_string(),
                };
                let Ok(channel) = handle
                    .channel_open_forwarded_tcpip(
                        connected_address.clone(),
                        connected_port,
                        origin_address,
                        origin.port() as u32,
                    )
                    .await
                else {
                    break;
                };
                tokio::spawn(async move {
                    let mut ssh_stream = channel.into_stream();
                    let _ = tokio::io::copy_bidirectional(&mut ssh_stream, &mut inbound).await;
                });
            }
        });
        self.state.forwards.lock().await.insert(listen_key, task);
        Ok(true)
    }

    async fn cancel_tcpip_forward(
        &mut self,
        address: &str,
        port: u32,
        _session: &mut server::Session,
    ) -> Result<bool, Self::Error> {
        if let Some(task) = self
            .state
            .forwards
            .lock()
            .await
            .remove(&forward_key(address, port))
        {
            task.abort();
        }
        Ok(true)
    }
}

async fn spawn_shell(
    channel: ChannelId,
    handle: server::Handle,
    state: SharedState,
    env: HashMap<String, String>,
    pty: Option<PtyRequest>,
    command: Option<String>,
) -> Result<(), russh::Error> {
    #[cfg(unix)]
    if let Some(pty) = pty {
        return spawn_pty_shell(channel, handle, state, env, pty, command).await;
    }

    let mut cmd = build_command(&state.options.shell_executable, command);
    apply_default_env(&mut cmd, &env, &state.options.shell_executable);
    cmd.stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .current_dir(current_home_dir());

    let mut child = cmd.spawn()?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdin = child.stdin.take();
    let (stdin_tx, mut stdin_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    if let Some(session) = state.channels.lock().await.get_mut(&channel) {
        session.io = Some(ChannelIo::Stream(stdin_tx));
    }

    if let Some(mut stdin) = stdin {
        tokio::spawn(async move {
            while let Some(bytes) = stdin_rx.recv().await {
                if stdin.write_all(&bytes).await.is_err() {
                    break;
                }
                let _ = stdin.flush().await;
            }
        });
    }

    if let Some(stdout) = stdout {
        tokio::spawn(copy_stdout(channel, handle.clone(), stdout));
    }
    if let Some(stderr) = stderr {
        tokio::spawn(copy_stderr(channel, handle.clone(), stderr));
    }

    tokio::spawn(async move {
        let status = match child.wait().await {
            Ok(status) => status.code().unwrap_or(1) as u32,
            Err(_) => 1,
        };
        let _ = handle.exit_status_request(channel, status).await;
        let _ = handle.eof(channel).await;
        let _ = handle.close(channel).await;
    });

    Ok(())
}

#[cfg(unix)]
async fn spawn_pty_shell(
    channel: ChannelId,
    handle: server::Handle,
    state: SharedState,
    env: HashMap<String, String>,
    pty: PtyRequest,
    command: Option<String>,
) -> Result<(), russh::Error> {
    use std::fs::File as StdFile;
    use std::os::fd::AsRawFd;

    use nix::libc;
    use nix::pty::{openpty, Winsize};
    use nix::unistd::setsid;

    let pty_result = openpty(
        Some(&Winsize {
            ws_row: pty.rows as u16,
            ws_col: pty.cols as u16,
            ws_xpixel: 0,
            ws_ypixel: 0,
        }),
        None,
    )
    .map_err(|err| russh::Error::IO(io::Error::other(err.to_string())))?;

    let mut cmd = build_command(&state.options.shell_executable, command);
    apply_default_env(&mut cmd, &env, &state.options.shell_executable);
    cmd.current_dir(current_home_dir());

    let slave_fd = pty_result.slave.as_raw_fd();
    let slave_file: StdFile = pty_result.slave.into();
    let stdin_file = slave_file
        .try_clone()
        .map_err(russh::Error::IO)?;
    let stdout_file = slave_file
        .try_clone()
        .map_err(russh::Error::IO)?;
    cmd.stdin(std::process::Stdio::from(stdin_file))
        .stdout(std::process::Stdio::from(stdout_file))
        .stderr(std::process::Stdio::from(slave_file));
    unsafe {
        cmd.pre_exec(move || {
            setsid().map_err(|err| io::Error::other(err.to_string()))?;
            if libc::ioctl(slave_fd, libc::TIOCSCTTY as _, 0) == -1 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }

    let child = cmd.spawn()?;
    let master_file: StdFile = pty_result.master.into();
    let writer_std = master_file
        .try_clone()
        .map_err(russh::Error::IO)?;
    let resizer_std = master_file
        .try_clone()
        .map_err(russh::Error::IO)?;
    let reader = tokio::fs::File::from_std(master_file);
    let writer = tokio::fs::File::from_std(writer_std);

    let (stdin_tx, stdin_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let (resize_tx, resize_rx) = mpsc::unbounded_channel::<(u32, u32)>();

    if let Some(session) = state.channels.lock().await.get_mut(&channel) {
        session.io = Some(ChannelIo::Pty(PtyHandle {
            stdin_tx,
            resize_tx,
        }));
    }

    tokio::spawn(run_pty_writer(writer, stdin_rx));
    tokio::spawn(run_pty_reader(channel, handle.clone(), reader));
    tokio::spawn(run_pty_resizer(resizer_std, resize_rx));
    tokio::spawn(wait_for_child(channel, handle, child));

    Ok(())
}

async fn copy_stdout(channel: ChannelId, handle: server::Handle, mut stdout: ChildStdout) {
    let mut buf = vec![0u8; 16 * 1024];
    loop {
        match stdout.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let _ = handle.data(channel, buf[..n].to_vec().into()).await;
            }
        }
    }
}

async fn copy_stderr(channel: ChannelId, handle: server::Handle, mut stderr: ChildStderr) {
    let mut buf = vec![0u8; 16 * 1024];
    loop {
        match stderr.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let _ = handle.extended_data(channel, 1, buf[..n].to_vec().into()).await;
            }
        }
    }
}

#[cfg(unix)]
async fn run_pty_reader(
    channel: ChannelId,
    handle: server::Handle,
    mut reader: tokio::fs::File,
) {
    let mut buf = vec![0u8; 16 * 1024];
    loop {
        match reader.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let _ = handle.data(channel, buf[..n].to_vec().into()).await;
            }
        }
    }
}

#[cfg(unix)]
async fn run_pty_writer(
    mut writer: tokio::fs::File,
    mut stdin_rx: mpsc::UnboundedReceiver<Vec<u8>>,
) {
    while let Some(bytes) = stdin_rx.recv().await {
        if writer.write_all(&bytes).await.is_err() {
            break;
        }
        let _ = writer.flush().await;
    }
}

#[cfg(unix)]
async fn run_pty_resizer(
    pty_file: std::fs::File,
    mut resize_rx: mpsc::UnboundedReceiver<(u32, u32)>,
) {
    while let Some((cols, rows)) = resize_rx.recv().await {
        let _ = resize_pty(&pty_file, cols, rows);
    }
}

#[cfg(unix)]
async fn wait_for_child(channel: ChannelId, handle: server::Handle, mut child: Child) {
    let status = match child.wait().await {
        Ok(status) => status.code().unwrap_or(1) as u32,
        Err(_) => 1,
    };
    let _ = handle.exit_status_request(channel, status).await;
    let _ = handle.eof(channel).await;
    let _ = handle.close(channel).await;
}

#[cfg(unix)]
fn resize_pty(pty_file: &std::fs::File, cols: u32, rows: u32) -> io::Result<()> {
    use std::os::fd::AsRawFd;

    let winsize = nix::libc::winsize {
        ws_row: rows as u16,
        ws_col: cols as u16,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    let result = unsafe { nix::libc::ioctl(pty_file.as_raw_fd(), nix::libc::TIOCSWINSZ as _, &winsize) };
    if result == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn build_command(shell_executable: &str, command: Option<String>) -> Command {
    if !shell_executable.trim().is_empty() {
        let mut parts = shell_executable.split_whitespace();
        let program = parts.next().unwrap_or(shell_executable);
        let mut cmd = Command::new(program);
        for arg in parts {
            cmd.arg(arg);
        }
        if let Some(command) = command {
            cmd.arg(command);
        }
        return cmd;
    }

    if cfg!(windows) {
        let mut cmd = Command::new("powershell.exe");
        if let Some(command) = command {
            cmd.arg(command);
        }
        return cmd;
    }

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    let mut cmd = Command::new(shell);
    if let Some(command) = command {
        cmd.arg("-c").arg(command);
    }
    cmd
}

fn apply_default_env(cmd: &mut Command, env: &HashMap<String, String>, shell_executable: &str) {
    cmd.env_clear();
    for (key, value) in env {
        cmd.env(key, value);
    }

    let shell = if shell_executable.trim().is_empty() {
        if cfg!(windows) {
            "powershell.exe".to_string()
        } else {
            std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
        }
    } else {
        shell_executable
            .split_whitespace()
            .next()
            .unwrap_or(shell_executable)
            .to_string()
    };
    let home = current_home_dir();
    let user = current_username();
    let term = std::env::var("TERM").unwrap_or_else(|_| "xterm".to_string());
    let path = std::env::var("PATH").unwrap_or_else(|_| {
        if cfg!(windows) {
            r"C:\Windows\system32;C:\Windows;C:\Windows\System32\Wbem".to_string()
        } else {
            "/usr/local/bin:/usr/bin:/bin:/usr/local/sbin:/usr/sbin:/sbin".to_string()
        }
    });
    cmd.env("TERM", term)
        .env("HOME", home)
        .env("USER", &user)
        .env("LOGNAME", user)
        .env("PATH", path)
        .env("SHELL", shell);
}

async fn ensure_server_key(path: &Path) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).await.map_err(|err| err.to_string())?;

    let secret = russh::keys::PrivateKey::random(
        &mut OsRng,
        Algorithm::Ecdsa {
            curve: EcdsaCurve::NistP521,
        },
    )
    .map_err(|err| err.to_string())?;
    let private_pem = secret
        .to_openssh(LineEnding::LF)
        .map_err(|err| err.to_string())?
        .to_string();
    let public = secret
        .public_key()
        .to_openssh()
        .map_err(|err| err.to_string())?;

    write_file_0600(path, private_pem.as_bytes())?;
    write_file_0600(&path.with_extension("pub"), format!("{public}\n").as_bytes())?;
    Ok(())
}

async fn is_authorized_key(
    sources: &[String],
    public_key: &russh::keys::ssh_key::PublicKey,
) -> bool {
    match load_authorized_keys(sources).await {
        Ok(keys) => keys.contains(public_key),
        Err(_) => false,
    }
}

async fn load_authorized_keys(
    sources: &[String],
) -> Result<HashSet<russh::keys::ssh_key::PublicKey>, String> {
    let mut keys = HashSet::new();
    for source in sources {
        let path = expand_user_home(source);
        let content = match fs::read_to_string(&path).await {
            Ok(content) => content,
            Err(_) => continue,
        };
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if let Ok(key) = ParsedPublicKey::from_openssh(trimmed) {
                let openssh = key.to_openssh().map_err(|err| err.to_string())?;
                let parsed =
                    russh::keys::ssh_key::PublicKey::from_openssh(&openssh).map_err(|err| err.to_string())?;
                keys.insert(parsed);
            }
        }
    }
    Ok(keys)
}

fn normalize_bind_address(address: &str, port: u16) -> String {
    if address.is_empty() {
        return format!("0.0.0.0:{port}");
    }
    if address.contains(':') && !address.starts_with('[') {
        return format!("[{address}]:{port}");
    }
    format!("{address}:{port}")
}

fn forward_key(address: &str, port: u32) -> String {
    format!("{address}:{port}")
}

#[derive(Default)]
struct SftpServer {
    next_handle: usize,
    handles: HashMap<String, SftpHandle>,
}

enum SftpHandle {
    File(fs::File),
    Dir {
        entries: Vec<File>,
        index: usize,
    },
}

impl russh_sftp::server::Handler for SftpServer {
    type Error = StatusCode;

    fn unimplemented(&self) -> Self::Error {
        StatusCode::OpUnsupported
    }

    async fn init(
        &mut self,
        _version: u32,
        _extensions: HashMap<String, String>,
    ) -> Result<Version, Self::Error> {
        Ok(Version::new())
    }

    async fn open(
        &mut self,
        id: u32,
        filename: String,
        pflags: OpenFlags,
        _attrs: FileAttributes,
    ) -> Result<Handle, Self::Error> {
        let path = sftp_path(&filename);
        let file = fs::OpenOptions::from(std::fs::OpenOptions::from(pflags))
            .open(path)
            .await
            .map_err(map_io_error)?;
        let handle = self.allocate_handle(SftpHandle::File(file));
        Ok(Handle { id, handle })
    }

    async fn close(&mut self, id: u32, handle: String) -> Result<Status, Self::Error> {
        self.handles.remove(&handle);
        Ok(ok_status(id))
    }

    async fn read(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        len: u32,
    ) -> Result<Data, Self::Error> {
        let Some(SftpHandle::File(file)) = self.handles.get_mut(&handle) else {
            return Err(StatusCode::NoSuchFile);
        };
        file.seek(std::io::SeekFrom::Start(offset))
            .await
            .map_err(map_io_error)?;
        let mut buf = vec![0u8; len as usize];
        let n = file.read(&mut buf).await.map_err(map_io_error)?;
        if n == 0 {
            return Err(StatusCode::Eof);
        }
        buf.truncate(n);
        Ok(Data { id, data: buf })
    }

    async fn write(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        data: Vec<u8>,
    ) -> Result<Status, Self::Error> {
        let Some(SftpHandle::File(file)) = self.handles.get_mut(&handle) else {
            return Err(StatusCode::NoSuchFile);
        };
        file.seek(std::io::SeekFrom::Start(offset))
            .await
            .map_err(map_io_error)?;
        file.write_all(&data).await.map_err(map_io_error)?;
        file.flush().await.map_err(map_io_error)?;
        Ok(ok_status(id))
    }

    async fn lstat(&mut self, id: u32, path: String) -> Result<Attrs, Self::Error> {
        let metadata = fs::symlink_metadata(sftp_path(&path))
            .await
            .map_err(map_io_error)?;
        Ok(Attrs {
            id,
            attrs: FileAttributes::from(&metadata),
        })
    }

    async fn fstat(&mut self, id: u32, handle: String) -> Result<Attrs, Self::Error> {
        let Some(SftpHandle::File(file)) = self.handles.get_mut(&handle) else {
            return Err(StatusCode::NoSuchFile);
        };
        let metadata = file.metadata().await.map_err(map_io_error)?;
        Ok(Attrs {
            id,
            attrs: FileAttributes::from(&metadata),
        })
    }

    async fn setstat(
        &mut self,
        id: u32,
        path: String,
        attrs: FileAttributes,
    ) -> Result<Status, Self::Error> {
        apply_attrs(Path::new(&sftp_path(&path)), &attrs).await?;
        Ok(ok_status(id))
    }

    async fn fsetstat(
        &mut self,
        id: u32,
        handle: String,
        attrs: FileAttributes,
    ) -> Result<Status, Self::Error> {
        let Some(SftpHandle::File(file)) = self.handles.get_mut(&handle) else {
            return Err(StatusCode::NoSuchFile);
        };
        if let Some(size) = attrs.size {
            file.set_len(size).await.map_err(map_io_error)?;
        }
        Ok(ok_status(id))
    }

    async fn opendir(&mut self, id: u32, path: String) -> Result<Handle, Self::Error> {
        let pathbuf = sftp_path(&path);
        let mut entries = fs::read_dir(&pathbuf).await.map_err(map_io_error)?;
        let mut files = Vec::new();
        while let Some(entry) = entries.next_entry().await.map_err(map_io_error)? {
            let metadata = entry.metadata().await.map_err(map_io_error)?;
            files.push(File::new(
                entry.file_name().to_string_lossy().into_owned(),
                FileAttributes::from(&metadata),
            ));
        }
        let handle = self.allocate_handle(SftpHandle::Dir { entries: files, index: 0 });
        Ok(Handle { id, handle })
    }

    async fn readdir(&mut self, id: u32, handle: String) -> Result<Name, Self::Error> {
        let Some(SftpHandle::Dir { entries, index }) = self.handles.get_mut(&handle) else {
            return Err(StatusCode::NoSuchFile);
        };
        if *index >= entries.len() {
            return Err(StatusCode::Eof);
        }
        let batch = entries[*index..].to_vec();
        *index = entries.len();
        Ok(Name { id, files: batch })
    }

    async fn remove(&mut self, id: u32, filename: String) -> Result<Status, Self::Error> {
        fs::remove_file(sftp_path(&filename))
            .await
            .map_err(map_io_error)?;
        Ok(ok_status(id))
    }

    async fn mkdir(
        &mut self,
        id: u32,
        path: String,
        _attrs: FileAttributes,
    ) -> Result<Status, Self::Error> {
        fs::create_dir_all(sftp_path(&path))
            .await
            .map_err(map_io_error)?;
        Ok(ok_status(id))
    }

    async fn rmdir(&mut self, id: u32, path: String) -> Result<Status, Self::Error> {
        fs::remove_dir(sftp_path(&path)).await.map_err(map_io_error)?;
        Ok(ok_status(id))
    }

    async fn realpath(&mut self, id: u32, path: String) -> Result<Name, Self::Error> {
        let normalized = if path.is_empty() || path == "." {
            ".".to_string()
        } else {
            path
        };
        let absolute = fs::canonicalize(sftp_path(&normalized))
            .await
            .unwrap_or_else(|_| PathBuf::from(normalized));
        Ok(Name {
            id,
            files: vec![File::dummy(absolute.to_string_lossy().into_owned())],
        })
    }

    async fn stat(&mut self, id: u32, path: String) -> Result<Attrs, Self::Error> {
        let metadata = fs::metadata(sftp_path(&path)).await.map_err(map_io_error)?;
        Ok(Attrs {
            id,
            attrs: FileAttributes::from(&metadata),
        })
    }

    async fn rename(
        &mut self,
        id: u32,
        oldpath: String,
        newpath: String,
    ) -> Result<Status, Self::Error> {
        fs::rename(sftp_path(&oldpath), sftp_path(&newpath))
            .await
            .map_err(map_io_error)?;
        Ok(ok_status(id))
    }

    async fn readlink(&mut self, id: u32, path: String) -> Result<Name, Self::Error> {
        let target = fs::read_link(sftp_path(&path)).await.map_err(map_io_error)?;
        Ok(Name {
            id,
            files: vec![File::dummy(target.to_string_lossy().into_owned())],
        })
    }

    async fn symlink(
        &mut self,
        id: u32,
        linkpath: String,
        targetpath: String,
    ) -> Result<Status, Self::Error> {
        #[cfg(unix)]
        {
            tokio::fs::symlink(targetpath, sftp_path(&linkpath))
                .await
                .map_err(map_io_error)?;
            Ok(ok_status(id))
        }
        #[cfg(not(unix))]
        {
            let _ = (linkpath, targetpath);
            Err(StatusCode::OpUnsupported)
        }
    }
}

impl SftpServer {
    fn allocate_handle(&mut self, entry: SftpHandle) -> String {
        self.next_handle += 1;
        let handle = self.next_handle.to_string();
        self.handles.insert(handle.clone(), entry);
        handle
    }
}

fn sftp_path(path: &str) -> PathBuf {
    if path.is_empty() {
        PathBuf::from(".")
    } else {
        PathBuf::from(path)
    }
}

fn ok_status(id: u32) -> Status {
    Status {
        id,
        status_code: StatusCode::Ok,
        error_message: "Ok".to_string(),
        language_tag: "en-US".to_string(),
    }
}

fn map_io_error(err: std::io::Error) -> StatusCode {
    use std::io::ErrorKind;

    match err.kind() {
        ErrorKind::NotFound => StatusCode::NoSuchFile,
        ErrorKind::PermissionDenied => StatusCode::PermissionDenied,
        ErrorKind::AlreadyExists => StatusCode::Failure,
        ErrorKind::UnexpectedEof => StatusCode::Eof,
        _ => StatusCode::Failure,
    }
}

async fn apply_attrs(path: &Path, attrs: &FileAttributes) -> Result<(), StatusCode> {
    if let Some(size) = attrs.size {
        let file = fs::OpenOptions::new()
            .write(true)
            .open(path)
            .await
            .map_err(map_io_error)?;
        file.set_len(size).await.map_err(map_io_error)?;
    }
    #[cfg(unix)]
    if let Some(mode) = attrs.permissions {
        use std::os::unix::fs::PermissionsExt;

        fs::set_permissions(path, std::fs::Permissions::from_mode(mode & 0o777))
            .await
            .map_err(map_io_error)?;
    }
    Ok(())
}
