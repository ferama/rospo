use std::time::Duration;

use crate::logging::{Logger, MAGENTA};
use crate::ssh::LOG as SSH_LOG;
use tokio::io::{copy_bidirectional, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time;

use crate::ssh::{ClientOptions, Session};
use crate::utils::Endpoint;

pub const RECONNECTION_INTERVAL_SECS: u64 = 5;
const LOG: Logger = Logger::new("[TUN]  ", MAGENTA);

pub async fn run_forward(options: ClientOptions, local: Endpoint, remote: Endpoint) -> Result<(), String> {
    loop {
        let mut session = match Session::connect(options.clone()).await {
            Ok(session) => session,
            Err(_) => {
                time::sleep(Duration::from_secs(RECONNECTION_INTERVAL_SECS)).await;
                continue;
            }
        };
        SSH_LOG.log(format_args!("starting client keep alive"));

        let listener = match TcpListener::bind(local.to_string()).await {
            Ok(listener) => listener,
            Err(err) => {
                LOG.log(format_args!("dial INTO remote service error. {}", err));
                return Err(err.to_string());
            }
        };
        LOG.log(format_args!(
            "forward connected. Local: {} <- Remote: {}",
            listener.local_addr().map_err(|err| err.to_string())?,
            remote
        ));

        // Go uses the same 5-second cadence for reconnect backoff and tunnel liveness checks, so
        // the Rust loop keeps both decisions on the same timer.
        let mut ping = time::interval(Duration::from_secs(RECONNECTION_INTERVAL_SECS));
        let mut should_reconnect = false;

        while !should_reconnect {
            tokio::select! {
                accepted = listener.accept() => {
                    let (socket, origin) = match accepted {
                        Ok(pair) => pair,
                        Err(_) => {
                            should_reconnect = true;
                            continue;
                        }
                    };
                    let channel = match session.open_direct_tcpip(
                        remote.host.trim_matches(&['[', ']'][..]),
                        remote.port,
                        &origin.ip().to_string(),
                        u32::from(origin.port()),
                    ).await {
                        Ok(channel) => channel,
                        Err(err) => {
                            LOG.log(format_args!("listen open port ON local server error. {}", err));
                            should_reconnect = true;
                            continue;
                        }
                    };
                    tokio::spawn(proxy_streams(socket, channel.into_stream()));
                }
                _ = ping.tick() => {
                    if let Err(err) = session.send_keepalive_request().await {
                        SSH_LOG.log(format_args!("error while sending keep alive {}", err));
                        should_reconnect = true;
                    }
                }
            }
        }

        let _ = session.disconnect().await;
        time::sleep(Duration::from_secs(RECONNECTION_INTERVAL_SECS)).await;
    }
}

pub async fn run_reverse(options: ClientOptions, local: Endpoint, remote: Endpoint) -> Result<(), String> {
    loop {
        let mut session = match Session::connect(options.clone()).await {
            Ok(session) => session,
            Err(_) => {
                time::sleep(Duration::from_secs(RECONNECTION_INTERVAL_SECS)).await;
                continue;
            }
        };
        SSH_LOG.log(format_args!("starting client keep alive"));

        let remote_host = remote.host.trim_matches(&['[', ']'][..]).to_string();
        LOG.log(format_args!("starting remote listener"));
        let assigned_port = match session.tcpip_forward(&remote_host, remote.port).await {
            Ok(port) => port,
            Err(err) => {
                LOG.log(format_args!("listen open port ON remote server error. {}", err));
                let _ = session.disconnect().await;
                time::sleep(Duration::from_secs(RECONNECTION_INTERVAL_SECS)).await;
                continue;
            }
        };
        LOG.log(format_args!(
            "reverse connected. Local: {} -> Remote: {}:{}",
            local, remote_host, assigned_port
        ));

        let mut ping = time::interval(Duration::from_secs(RECONNECTION_INTERVAL_SECS));
        let mut should_reconnect = false;
        while !should_reconnect {
            tokio::select! {
                maybe_forwarded = session.next_forwarded() => {
                    let Some(forwarded) = maybe_forwarded else {
                        should_reconnect = true;
                        continue;
                    };
                    let local_addr = local.to_string();
                    tokio::spawn(async move {
                        // Reverse tunnels terminate locally: each forwarded-tcpip channel from the
                        // server is bridged into a fresh TCP connection on the local machine.
                        match TcpStream::connect(&local_addr).await {
                            Ok(socket) => proxy_streams(socket, forwarded.channel.into_stream()).await,
                            Err(err) => LOG.log(format_args!("dial INTO local service error. {}", err)),
                        }
                    });
                }
                _ = ping.tick() => {
                    if let Err(err) = session.send_keepalive_request().await {
                        SSH_LOG.log(format_args!("error while sending keep alive {}", err));
                        LOG.log(format_args!("disconnected"));
                        should_reconnect = true;
                    }
                }
            }
        }

        let _ = session.cancel_tcpip_forward(&remote_host, assigned_port as u16).await;
        let _ = session.disconnect().await;
        time::sleep(Duration::from_secs(RECONNECTION_INTERVAL_SECS)).await;
    }
}

async fn proxy_streams<S>(mut socket: TcpStream, mut stream: S)
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let _ = copy_bidirectional(&mut socket, &mut stream).await;
    let _ = socket.shutdown().await;
    let _ = stream.shutdown().await;
}

#[cfg(test)]
mod tests {
    use super::RECONNECTION_INTERVAL_SECS;

    #[test]
    fn keepalive_interval_matches_go_default() {
        assert_eq!(RECONNECTION_INTERVAL_SECS, 5);
    }
}
