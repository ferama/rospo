use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, UdpSocket};
use tokio::sync::Mutex;

use crate::ssh::{ClientOptions, Session};
use crate::ssh::LOG;
use crate::utils::new_endpoint;

pub async fn run(options: ClientOptions, listen_address: &str, remote_dns: &str) -> Result<(), String> {
    let remote = new_endpoint(remote_dns)?;
    let session = Session::connect(options).await?;
    let session = Arc::new(Mutex::new(session));
    let listen_address = normalize_listen_address(listen_address);

    // Go accepts bare `:53`-style listen addresses and treats them as "all interfaces on this
    // port", so normalize before handing the address to Tokio.
    let udp_socket = Arc::new(UdpSocket::bind(&listen_address).await.map_err(|err| err.to_string())?);
    let tcp_listener = TcpListener::bind(&listen_address).await.map_err(|err| err.to_string())?;
    LOG.log(format_args!(
        "dns-proxy listening on: {}. Using remote dns: {}",
        listen_address, remote_dns
    ));

    let udp_session = Arc::clone(&session);
    let udp_remote = remote.clone();
    let udp_socket_clone = Arc::clone(&udp_socket);
    tokio::spawn(async move {
        let _ = run_udp(udp_session, udp_socket_clone, udp_remote).await;
    });

    run_tcp(session, tcp_listener, remote).await
}

async fn run_udp(
    session: Arc<Mutex<Session>>,
    socket: Arc<UdpSocket>,
    remote: crate::utils::Endpoint,
) -> Result<(), String> {
    let mut buf = vec![0u8; 65535];
    loop {
        let (size, peer) = socket.recv_from(&mut buf).await.map_err(|err| err.to_string())?;
        let query = buf[..size].to_vec();
        let response = resolve_dns(session.clone(), remote.clone(), &query).await?;
        socket
            .send_to(&response, peer)
            .await
            .map_err(|err| err.to_string())?;
    }
}

async fn run_tcp(
    session: Arc<Mutex<Session>>,
    listener: TcpListener,
    remote: crate::utils::Endpoint,
) -> Result<(), String> {
    loop {
        let (mut client, peer) = listener.accept().await.map_err(|err| err.to_string())?;
        let session = Arc::clone(&session);
        let remote = remote.clone();
        tokio::spawn(async move {
            let channel = {
                let mut guard = session.lock().await;
                guard
                    .open_direct_tcpip(
                        remote.host.trim_matches(&['[', ']'][..]),
                        remote.port,
                        &peer.ip().to_string(),
                        u32::from(peer.port()),
                    )
                    .await
            };
            if let Ok(channel) = channel {
                let mut stream = channel.into_stream();
                let _ = tokio::io::copy_bidirectional(&mut client, &mut stream).await;
                let _ = client.shutdown().await;
                let _ = stream.shutdown().await;
            }
        });
    }
}

async fn resolve_dns(
    session: Arc<Mutex<Session>>,
    remote: crate::utils::Endpoint,
    query: &[u8],
) -> Result<Vec<u8>, String> {
    let channel = {
        let mut guard = session.lock().await;
        guard
            .open_direct_tcpip(remote.host.trim_matches(&['[', ']'][..]), remote.port, "127.0.0.1", 0)
            .await
    }?;
    let mut stream = channel.into_stream();
    let len = (query.len() as u16).to_be_bytes();
    stream.write_all(&len).await.map_err(|err| err.to_string())?;
    stream.write_all(query).await.map_err(|err| err.to_string())?;
    stream.flush().await.map_err(|err| err.to_string())?;

    let mut response_len = [0u8; 2];
    stream
        .read_exact(&mut response_len)
        .await
        .map_err(|err| err.to_string())?;
    let size = u16::from_be_bytes(response_len) as usize;
    let mut response = vec![0u8; size];
    stream
        .read_exact(&mut response)
        .await
        .map_err(|err| err.to_string())?;
    let _ = stream.shutdown().await;
    Ok(response)
}

fn normalize_listen_address(address: &str) -> String {
    if address.starts_with(':') {
        format!("0.0.0.0{address}")
    } else {
        address.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_listen_address;

    #[test]
    fn normalize_listen_address_supports_go_style_port_binding() {
        assert_eq!(normalize_listen_address(":53"), "0.0.0.0:53");
        assert_eq!(normalize_listen_address("127.0.0.1:5300"), "127.0.0.1:5300");
    }
}
