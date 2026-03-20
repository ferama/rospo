use russh::{client, client::Handle, Disconnect, Pty};
use russh_sftp::client::SftpSession;
use tokio::sync::mpsc;

use super::interactive::{drain_channel, terminal_size};
use super::transport::{authenticate_handle, build_client_config};
use super::types::{ClientHandler, ClientOptions, ForwardedTcpIp, CHECKALIVE_REQUEST, KEEPALIVE_REQUEST};
use super::LOG;

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
            let hop_addr = format_endpoint(&hop.host, hop.port);
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
            LOG.log(format_args!("connecting to hop {}@{}", hop.username, hop_addr));

            let mut handle = if let Some(previous) = previous_handle.take() {
                let channel = previous
                    .channel_open_direct_tcpip(hop.host.clone(), u32::from(hop.port), "127.0.0.1", 0)
                    .await
                    .map_err(|err| err.to_string())?;
                client::connect_stream(build_client_config(), channel.into_stream(), handler)
                    .await
                    .map_err(|err| err.to_string())?
            } else {
                match client::connect(build_client_config(), (hop.host.as_str(), hop.port), handler).await {
                    Ok(handle) => handle,
                    Err(err) => {
                        LOG.log(format_args!("dial INTO remote server error. {}", err));
                        return Err(err.to_string());
                    }
                }
            };

            authenticate_handle(&mut handle, &hop.username, &hop.identity, hop.password.as_deref()).await?;

            LOG.log(format_args!("reached the jump host {}@{}", hop.username, hop_addr));

            previous_handle = Some(handle);
        }

        let handler = ClientHandler {
            options: options.clone(),
            forwarded_sender: Some(forwarded_sender),
        };
        let server_addr = format_endpoint(&options.host, options.port);

        let mut handle = if let Some(previous) = previous_handle {
            LOG.log(format_args!("connecting to {}@{}", options.username, server_addr));
            let channel = previous
                .channel_open_direct_tcpip(options.host.clone(), u32::from(options.port), "127.0.0.1", 0)
                .await
                .map_err(|err| err.to_string())?;
            client::connect_stream(build_client_config(), channel.into_stream(), handler)
                .await
                .map_err(|err| err.to_string())?
        } else {
            LOG.log(format_args!("connecting to {}", server_addr));
            match client::connect(build_client_config(), (options.host.as_str(), options.port), handler).await {
                Ok(handle) => handle,
                Err(err) => {
                    LOG.log(format_args!("dial INTO remote server error. {}", err));
                    return Err(err.to_string());
                }
            }
        };

        authenticate_handle(
            &mut handle,
            &options.username,
            &options.identity,
            options.password.as_deref(),
        )
        .await?;

        LOG.log(format_args!("connected to remote server at {}", server_addr));

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
            if let Ok(val) = std::env::var(env_name)
                && !val.is_empty()
            {
                let _ = channel.set_env(false, env_name, val).await;
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

    pub async fn send_keepalive_request(&self) -> Result<(), String> {
        self.send_alive_request(KEEPALIVE_REQUEST).await
    }

    pub async fn send_checkalive_request(&self) -> Result<(), String> {
        self.send_alive_request(CHECKALIVE_REQUEST).await
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

    async fn send_alive_request(&self, request_name: &str) -> Result<(), String> {
        let _ = request_name;
        self.handle
            .send_ping()
            .await
            .map_err(|err| err.to_string())
    }
}

fn format_endpoint(host: &str, port: u16) -> String {
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

#[cfg(test)]
mod tests {
    use super::format_endpoint;

    #[test]
    fn format_endpoint_preserves_ipv4() {
        assert_eq!(format_endpoint("127.0.0.1", 2222), "127.0.0.1:2222");
    }

    #[test]
    fn format_endpoint_wraps_ipv6() {
        assert_eq!(format_endpoint("::1", 2222), "[::1]:2222");
    }
}
