use std::sync::Arc;

use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

use crate::ssh::{ClientOptions, Session};
use crate::ssh::LOG;

pub const DEFAULT_LISTEN_ADDRESS: &str = "127.0.0.1:1080";

pub async fn run(options: ClientOptions, listen_address: &str) -> Result<(), String> {
    let session = Session::connect(options).await?;
    let session = Arc::new(Mutex::new(session));
    let listener = TcpListener::bind(listen_address)
        .await
        .map_err(|err| err.to_string())?;
    LOG.log(format_args!("local socks proxy listening at '{}'", listen_address));

    loop {
        let (socket, peer) = listener.accept().await.map_err(|err| err.to_string())?;
        let session = Arc::clone(&session);
        tokio::spawn(async move {
            let _ = handle_client(session, socket, peer.ip().to_string(), u32::from(peer.port())).await;
        });
    }
}

async fn handle_client(
    session: Arc<Mutex<Session>>,
    mut socket: TcpStream,
    originator_address: String,
    originator_port: u32,
) -> Result<(), String> {
    let mut version = [0u8; 1];
    socket
        .read_exact(&mut version)
        .await
        .map_err(|err| err.to_string())?;

    let (host, port, success_reply, failure_reply) = match version[0] {
        4 => handle_socks4_handshake(&mut socket).await?,
        5 => handle_socks5_handshake(&mut socket).await?,
        other => return Err(format!("unsupported socks version: {other}")),
    };

    let channel = {
        let mut guard = session.lock().await;
        guard
            .open_direct_tcpip(&host, port, &originator_address, originator_port)
            .await
    };

    let channel = match channel {
        Ok(channel) => {
            socket
                .write_all(&success_reply)
                .await
                .map_err(|err| err.to_string())?;
            channel
        }
        Err(err) => {
            let _ = socket.write_all(&failure_reply).await;
            return Err(err);
        }
    };

    let mut stream = channel.into_stream();
    let _ = io::copy_bidirectional(&mut socket, &mut stream).await;
    let _ = socket.shutdown().await;
    let _ = stream.shutdown().await;
    Ok(())
}

async fn handle_socks5_handshake(socket: &mut TcpStream) -> Result<(String, u16, Vec<u8>, Vec<u8>), String> {
    let mut methods_len = [0u8; 1];
    socket
        .read_exact(&mut methods_len)
        .await
        .map_err(|err| err.to_string())?;
    let mut methods = vec![0u8; methods_len[0] as usize];
    socket
        .read_exact(&mut methods)
        .await
        .map_err(|err| err.to_string())?;

    if !methods.contains(&0x00) {
        socket
            .write_all(&[0x05, 0xff])
            .await
            .map_err(|err| err.to_string())?;
        return Err("no supported socks5 authentication method".to_string());
    }

    socket
        .write_all(&[0x05, 0x00])
        .await
        .map_err(|err| err.to_string())?;

    let mut header = [0u8; 4];
    socket
        .read_exact(&mut header)
        .await
        .map_err(|err| err.to_string())?;
    if header[1] != 0x01 {
        return Err("unsupported socks5 command".to_string());
    }

    let host = match header[3] {
        0x01 => {
            let mut addr = [0u8; 4];
            socket.read_exact(&mut addr).await.map_err(|err| err.to_string())?;
            std::net::Ipv4Addr::from(addr).to_string()
        }
        0x03 => {
            let mut len = [0u8; 1];
            socket.read_exact(&mut len).await.map_err(|err| err.to_string())?;
            let mut addr = vec![0u8; len[0] as usize];
            socket.read_exact(&mut addr).await.map_err(|err| err.to_string())?;
            String::from_utf8(addr).map_err(|err| err.to_string())?
        }
        0x04 => {
            let mut addr = [0u8; 16];
            socket.read_exact(&mut addr).await.map_err(|err| err.to_string())?;
            std::net::Ipv6Addr::from(addr).to_string()
        }
        _ => return Err("unsupported socks5 address type".to_string()),
    };

    let mut port = [0u8; 2];
    socket.read_exact(&mut port).await.map_err(|err| err.to_string())?;
    let port = u16::from_be_bytes(port);

    Ok((
        host,
        port,
        vec![0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0],
        vec![0x05, 0x01, 0x00, 0x01, 0, 0, 0, 0, 0, 0],
    ))
}

async fn handle_socks4_handshake(socket: &mut TcpStream) -> Result<(String, u16, Vec<u8>, Vec<u8>), String> {
    let mut header = [0u8; 7];
    socket
        .read_exact(&mut header)
        .await
        .map_err(|err| err.to_string())?;
    if header[0] != 0x01 {
        return Err("unsupported socks4 command".to_string());
    }
    let port = u16::from_be_bytes([header[1], header[2]]);
    let ip = [header[3], header[4], header[5], header[6]];

    loop {
        let mut byte = [0u8; 1];
        socket.read_exact(&mut byte).await.map_err(|err| err.to_string())?;
        if byte[0] == 0 {
            break;
        }
    }

    let host = if ip[..3] == [0, 0, 0] && ip[3] != 0 {
        let mut domain = Vec::new();
        loop {
            let mut byte = [0u8; 1];
            socket.read_exact(&mut byte).await.map_err(|err| err.to_string())?;
            if byte[0] == 0 {
                break;
            }
            domain.push(byte[0]);
        }
        String::from_utf8(domain).map_err(|err| err.to_string())?
    } else {
        std::net::Ipv4Addr::from(ip).to_string()
    };

    Ok((
        host,
        port,
        vec![0x00, 0x5a, header[1], header[2], header[3], header[4], header[5], header[6]],
        vec![0x00, 0x5b, 0, 0, 0, 0, 0, 0],
    ))
}
