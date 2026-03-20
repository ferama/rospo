use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use russh::server::{self, Auth, Msg, Server as _};
use russh::{Channel, ChannelId};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

use crate::logging::{Logger, BLUE};
use crate::utils::expand_user_home;

use super::auth::{
    ensure_server_key, forward_key, is_authorized_key, load_authorized_keys, normalize_bind_address,
    normalize_server_listen_address,
};
use super::process::spawn_shell;
use super::sftp::SftpServer;
use super::{BANNER, Handler, PtyRequest, Server, ServerOptions, SessionChannelState, SharedState};

const LOG: Logger = Logger::new("[SSHD] ", BLUE);

pub async fn run(options: ServerOptions) -> Result<(), String> {
    if options.server_key.is_empty() {
        return Err("server_key is not set".to_string());
    }
    if options.listen_address.is_empty() {
        return Err("listen port can't be empty".to_string());
    }

    LOG.log(format_args!("loading server key at: '{}'", expand_user_home(&options.server_key)));
    LOG.log(format_args!("authorized_keys: {:?}", options.authorized_keys));
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
        keepalive_interval: None,
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
    let listen_address = normalize_server_listen_address(&server.state.options.listen_address);
    let listener = TcpListener::bind(&listen_address)
        .await
        .map_err(|err| err.to_string())?;
    LOG.log(format_args!(
        "listening on {}",
        listener.local_addr().map_err(|err| err.to_string())?
    ));

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
                super::ChannelIo::Stream(stdin_tx) => {
                    let _ = stdin_tx.send(data.to_vec());
                }
                super::ChannelIo::Pty(pty) => {
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
            let _ = session.eof(channel);
            let _ = session.close(channel);
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
            let _ = session.eof(channel);
            let _ = session.close(channel);
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
            let _ = session.eof(channel);
            let _ = session.close(channel);
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
            let _ = session.eof(channel);
            let _ = session.close(channel);
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
            if let Some(super::ChannelIo::Pty(pty)) = state.io.clone() {
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
