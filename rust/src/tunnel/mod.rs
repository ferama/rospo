use std::time::Duration;

use tokio::io::{copy_bidirectional, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time;

use crate::ssh::{ClientOptions, Session};
use crate::utils::Endpoint;

pub const RECONNECTION_INTERVAL_SECS: u64 = 5;

pub async fn run_forward(options: ClientOptions, local: Endpoint, remote: Endpoint) -> Result<(), String> {
    loop {
        let mut session = match Session::connect(options.clone()).await {
            Ok(session) => session,
            Err(err) => {
                time::sleep(Duration::from_secs(RECONNECTION_INTERVAL_SECS)).await;
                if err.is_empty() {
                    continue;
                }
                continue;
            }
        };

        let listener = match TcpListener::bind(local.to_string()).await {
            Ok(listener) => listener,
            Err(err) => return Err(err.to_string()),
        };

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
                        Err(_) => {
                            should_reconnect = true;
                            continue;
                        }
                    };
                    tokio::spawn(proxy_streams(socket, channel.into_stream()));
                }
                _ = ping.tick() => {
                    if session.send_ping().await.is_err() {
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

        let remote_host = remote.host.trim_matches(&['[', ']'][..]).to_string();
        let assigned_port = match session.tcpip_forward(&remote_host, remote.port).await {
            Ok(port) => port,
            Err(_) => {
                let _ = session.disconnect().await;
                time::sleep(Duration::from_secs(RECONNECTION_INTERVAL_SECS)).await;
                continue;
            }
        };

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
                        if let Ok(socket) = TcpStream::connect(&local_addr).await {
                            proxy_streams(socket, forwarded.channel.into_stream()).await;
                        }
                    });
                }
                _ = ping.tick() => {
                    if session.send_ping().await.is_err() {
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
