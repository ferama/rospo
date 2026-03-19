mod common;

use std::path::PathBuf;

use rospo::tunnel::{run_forward, run_reverse};
use rospo::utils::new_endpoint;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use common::{authorized_keys_path, client_options_for, reserve_local_addr, start_echo_service, start_sshd, wait_for_tcp};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn tunnel_forward_echoes_data() {
    let server = start_sshd(vec![authorized_keys_path("authorized_keys")], "", false, false).await;
    let known_hosts = PathBuf::from(common::unique_path("rospo-known-hosts-tun-forward"));
    let options = client_options_for(&server.addr, "client", None, true, &known_hosts, Vec::new()).await;

    let (echo_addr, echo_task) = start_echo_service().await;
    let local_addr = reserve_local_addr();
    let local = new_endpoint(&local_addr).expect("parse local endpoint");
    let remote = new_endpoint(&echo_addr).expect("parse remote endpoint");

    let tunnel_task = tokio::spawn(async move { run_forward(options, local, remote).await });
    wait_for_tcp(&local_addr).await;

    let mut conn = tokio::net::TcpStream::connect(&local_addr).await.expect("connect forward tunnel");
    conn.write_all(b"test\n").await.expect("write to forward tunnel");
    let mut buf = [0u8; 5];
    conn.read_exact(&mut buf).await.expect("read from forward tunnel");
    assert_eq!(&buf, b"test\n");

    tunnel_task.abort();
    echo_task.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn tunnel_reverse_echoes_data() {
    let server = start_sshd(vec![authorized_keys_path("authorized_keys")], "", false, false).await;
    let known_hosts = PathBuf::from(common::unique_path("rospo-known-hosts-tun-reverse"));
    let options = client_options_for(&server.addr, "client", None, true, &known_hosts, Vec::new()).await;

    let (echo_addr, echo_task) = start_echo_service().await;
    let local = new_endpoint(&echo_addr).expect("parse local endpoint");
    let remote_addr = reserve_local_addr();
    let remote = new_endpoint(&remote_addr).expect("parse remote endpoint");

    let tunnel_task = tokio::spawn(async move { run_reverse(options, local, remote).await });
    wait_for_tcp(&remote_addr).await;

    let mut conn = tokio::net::TcpStream::connect(&remote_addr).await.expect("connect reverse tunnel");
    conn.write_all(b"test\n").await.expect("write to reverse tunnel");
    let mut buf = [0u8; 5];
    conn.read_exact(&mut buf).await.expect("read from reverse tunnel");
    assert_eq!(&buf, b"test\n");

    tunnel_task.abort();
    echo_task.abort();
}
