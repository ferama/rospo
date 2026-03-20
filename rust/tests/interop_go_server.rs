mod common;

use std::path::PathBuf;

use rospo::sftp::Client as SftpClient;
use rospo::ssh::Session;
use rospo::tunnel::{run_forward, run_reverse};
use rospo::utils::new_endpoint;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use common::{
    authorized_keys_path, client_options_for, has_go_baseline, reserve_local_addr, start_echo_service, start_go_sshd,
    wait_for_tcp,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rust_shell_executes_commands_against_go_sshd() {
    let Some(server) = start_go_sshd(&authorized_keys_path("authorized_keys"), "").await else {
        eprintln!("skipping: /tmp/rospo-go-baseline not available");
        return;
    };

    let known_hosts = PathBuf::from(common::unique_path("rospo-known-hosts-go-shell"));
    let options = client_options_for(&server.addr, "client", None, true, &known_hosts, Vec::new()).await;
    let mut session = Session::connect(options).await.expect("connect to go sshd");
    let status = session
        .run_command("printf go-shell-ok")
        .await
        .expect("run command against go sshd");
    assert_eq!(status, 0);
    session.disconnect().await.expect("disconnect");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rust_sftp_put_and_get_work_against_go_sshd() {
    let Some(server) = start_go_sshd(&authorized_keys_path("authorized_keys"), "").await else {
        eprintln!("skipping: /tmp/rospo-go-baseline not available");
        return;
    };

    let known_hosts = PathBuf::from(common::unique_path("rospo-known-hosts-go-sftp"));
    let options = client_options_for(&server.addr, "client", None, true, &known_hosts, Vec::new()).await;
    let mut client = SftpClient::connect(options).await.expect("connect sftp client to go sshd");

    let local_upload = std::env::temp_dir().join(format!("rospo-go-put-{}", std::process::id()));
    std::fs::write(&local_upload, b"go-sftp-ok").expect("write upload fixture");
    let remote_path = format!("/tmp/rospo-go-remote-{}", std::process::id());
    client
        .put_file(&remote_path, &local_upload.to_string_lossy())
        .await
        .expect("upload file to go sshd");

    let local_download = std::env::temp_dir().join(format!("rospo-go-get-{}", std::process::id()));
    let _ = std::fs::remove_file(&local_download);
    client
        .get_file(&remote_path, &local_download.to_string_lossy())
        .await
        .expect("download file from go sshd");

    assert_eq!(std::fs::read(&local_download).expect("read downloaded file"), b"go-sftp-ok");

    let _ = std::fs::remove_file(local_upload);
    let _ = std::fs::remove_file(local_download);
    let _ = client.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rust_forward_tunnel_works_against_go_sshd() {
    if !has_go_baseline() {
        eprintln!("skipping: /tmp/rospo-go-baseline not available");
        return;
    }
    let server = start_go_sshd(&authorized_keys_path("authorized_keys"), "")
        .await
        .expect("start go sshd");
    let known_hosts = PathBuf::from(common::unique_path("rospo-known-hosts-go-tun-fwd"));
    let options = client_options_for(&server.addr, "client", None, true, &known_hosts, Vec::new()).await;

    let (echo_addr, echo_task) = start_echo_service().await;
    let local_addr = reserve_local_addr();
    let local = new_endpoint(&local_addr).expect("parse local endpoint");
    let remote = new_endpoint(&echo_addr).expect("parse echo endpoint");

    let tunnel_task = tokio::spawn(async move { run_forward(options, local, remote).await });
    wait_for_tcp(&local_addr).await;

    let mut conn = tokio::net::TcpStream::connect(&local_addr)
        .await
        .expect("connect to forward tunnel");
    conn.write_all(b"fwd-go\n").await.expect("write tunnel payload");
    let mut buf = [0u8; 7];
    conn.read_exact(&mut buf).await.expect("read tunnel payload");
    assert_eq!(&buf, b"fwd-go\n");

    tunnel_task.abort();
    echo_task.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rust_reverse_tunnel_works_against_go_sshd() {
    if !has_go_baseline() {
        eprintln!("skipping: /tmp/rospo-go-baseline not available");
        return;
    }
    let server = start_go_sshd(&authorized_keys_path("authorized_keys"), "")
        .await
        .expect("start go sshd");
    let known_hosts = PathBuf::from(common::unique_path("rospo-known-hosts-go-tun-rev"));
    let options = client_options_for(&server.addr, "client", None, true, &known_hosts, Vec::new()).await;

    let (echo_addr, echo_task) = start_echo_service().await;
    let local = new_endpoint(&echo_addr).expect("parse local endpoint");
    let remote_addr = reserve_local_addr();
    let remote = new_endpoint(&remote_addr).expect("parse remote endpoint");

    let tunnel_task = tokio::spawn(async move { run_reverse(options, local, remote).await });
    wait_for_tcp(&remote_addr).await;

    let mut conn = tokio::net::TcpStream::connect(&remote_addr)
        .await
        .expect("connect to reverse tunnel");
    conn.write_all(b"rev-go\n").await.expect("write tunnel payload");
    let mut buf = [0u8; 7];
    conn.read_exact(&mut buf).await.expect("read tunnel payload");
    assert_eq!(&buf, b"rev-go\n");

    tunnel_task.abort();
    echo_task.abort();
}
