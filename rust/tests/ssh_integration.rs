mod common;

use std::path::PathBuf;

use rospo::socks;
use rospo::ssh::{JumpHostOptions, Session};
use rospo::utils::new_endpoint;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use common::{
    authorized_keys_path, client_options_for, key_path, reserve_local_addr, start_http_hello_server, start_sshd,
    wait_for_tcp,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn secure_connection_with_known_hosts_succeeds() {
    let server = start_sshd(vec![authorized_keys_path("authorized_keys")], "", false, false).await;
    let known_hosts = PathBuf::from(common::unique_path("rospo-known-hosts-secure"));
    let options = client_options_for(&server.addr, "client", None, false, &known_hosts, Vec::new()).await;

    let mut session = Session::connect(options).await.expect("connect with known_hosts");
    let status = session.run_command("true").await.expect("run command");
    assert_eq!(status, 0);
    session.disconnect().await.expect("disconnect");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn jump_hosts_chain_succeeds() {
    let final_server = start_sshd(vec![authorized_keys_path("authorized_keys")], "", false, false).await;
    let jump1 = start_sshd(vec![authorized_keys_path("authorized_keys")], "", false, false).await;
    let jump2 = start_sshd(vec![authorized_keys_path("authorized_keys2")], "", false, false).await;

    let known_hosts = PathBuf::from(common::unique_path("rospo-known-hosts-jumps"));
    let final_endpoint = new_endpoint(&final_server.addr).expect("parse final endpoint");
    let jump1_endpoint = new_endpoint(&jump1.addr).expect("parse jump1 endpoint");
    let jump2_endpoint = new_endpoint(&jump2.addr).expect("parse jump2 endpoint");

    let options = rospo::ssh::ClientOptions {
        username: "ferama".to_string(),
        host: final_endpoint.host,
        port: final_endpoint.port,
        identity: key_path("client"),
        known_hosts,
        password: None,
        insecure: true,
        quiet: true,
        jump_hosts: vec![
            JumpHostOptions {
                username: "ferama".to_string(),
                host: jump1_endpoint.host,
                port: jump1_endpoint.port,
                identity: key_path("client"),
                password: None,
            },
            JumpHostOptions {
                username: "ferama".to_string(),
                host: jump2_endpoint.host,
                port: jump2_endpoint.port,
                identity: key_path("client2"),
                password: None,
            },
        ],
    };

    let mut session = Session::connect(options).await.expect("connect through jump hosts");
    let status = session.run_command("true").await.expect("run command");
    assert_eq!(status, 0);
    session.disconnect().await.expect("disconnect");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn password_auth_succeeds() {
    let server = start_sshd(Vec::new(), "password", false, false).await;
    let known_hosts = PathBuf::from(common::unique_path("rospo-known-hosts-password"));
    let options =
        client_options_for(&server.addr, "client", Some("password"), true, &known_hosts, Vec::new()).await;

    let mut session = Session::connect(options).await.expect("connect with password");
    let status = session.run_command("true").await.expect("run command");
    assert_eq!(status, 0);
    session.disconnect().await.expect("disconnect");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn shell_disabled_rejects_exec() {
    let server = start_sshd(vec![authorized_keys_path("authorized_keys")], "", true, false).await;
    let known_hosts = PathBuf::from(common::unique_path("rospo-known-hosts-shell-disabled"));
    let options = client_options_for(&server.addr, "client", None, true, &known_hosts, Vec::new()).await;

    let mut session = Session::connect(options).await.expect("connect");
    assert!(session.run_command("ls").await.is_err());
    session.disconnect().await.expect("disconnect");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn socks_proxy_transports_http_requests() {
    let server = start_sshd(vec![authorized_keys_path("authorized_keys")], "", false, false).await;
    let known_hosts = PathBuf::from(common::unique_path("rospo-known-hosts-socks"));
    let options = client_options_for(&server.addr, "client", None, true, &known_hosts, Vec::new()).await;
    let proxy_addr = reserve_local_addr();
    let proxy_addr_for_task = proxy_addr.clone();
    let socks_task = tokio::spawn(async move { socks::run(options, &proxy_addr_for_task).await });
    wait_for_tcp(&proxy_addr).await;

    let (http_addr, http_task) = start_http_hello_server("socks-test").await;
    let remote = new_endpoint(&http_addr).expect("parse http endpoint");

    let mut socket = tokio::net::TcpStream::connect(&proxy_addr)
        .await
        .expect("connect socks proxy");
    socket.write_all(&[0x05, 0x01, 0x00]).await.expect("write socks methods");
    let mut method_reply = [0u8; 2];
    socket
        .read_exact(&mut method_reply)
        .await
        .expect("read socks method reply");
    assert_eq!(method_reply, [0x05, 0x00]);

    let ip: std::net::Ipv4Addr = remote.host.parse().expect("ipv4 http endpoint");
    let mut request = vec![0x05, 0x01, 0x00, 0x01];
    request.extend_from_slice(&ip.octets());
    request.extend_from_slice(&remote.port.to_be_bytes());
    socket.write_all(&request).await.expect("write socks connect");

    let mut connect_reply = [0u8; 10];
    socket
        .read_exact(&mut connect_reply)
        .await
        .expect("read socks connect reply");
    assert_eq!(connect_reply[1], 0x00);

    socket
        .write_all(b"GET / HTTP/1.1\r\nHost: test\r\nConnection: close\r\n\r\n")
        .await
        .expect("write proxied http request");
    let mut response = Vec::new();
    socket.read_to_end(&mut response).await.expect("read proxied http response");
    let response = String::from_utf8(response).expect("utf8 http response");
    assert!(response.contains("socks-test"));

    socks_task.abort();
    http_task.abort();
}
