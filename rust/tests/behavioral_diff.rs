mod common;

use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;

use rospo::sftp::Client as SftpClient;
use rospo::socks;
use rospo::ssh::Session;
use rospo::tunnel::{run_forward, run_reverse};
use rospo::utils::new_endpoint;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use common::{
    authorized_keys_path, client_options_for, has_go_baseline, reserve_local_addr, start_echo_service,
    start_go_sshd, start_http_hello_server, start_sshd, unique_path, wait_for_tcp,
};

#[derive(Debug, PartialEq, Eq)]
struct BinaryResult {
    status: i32,
    stdout: String,
    stderr: String,
}

#[derive(Debug, PartialEq, Eq)]
struct ServerBehavior {
    shell_status: u32,
    sftp_roundtrip: Vec<u8>,
    socks_response: String,
    forward_echo: Vec<u8>,
    reverse_echo: Vec<u8>,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rust_and_go_binaries_match_for_grabpubkey_against_rust_sshd() {
    if !has_go_baseline() {
        eprintln!("skipping: /tmp/rospo-go-baseline not available");
        return;
    }

    let server = start_sshd(vec![authorized_keys_path("authorized_keys")], "", false, false).await;

    let rust_known_hosts = unique_path("rospo-rust-grabpubkey-known-hosts");
    let go_known_hosts = unique_path("rospo-go-grabpubkey-known-hosts");
    let server_arg = format!("ferama@{}", server.addr);

    let rust = run_rospo_binary(
        &rust_binary_path(),
        &["-q", "grabpubkey", "-k", path_str(&rust_known_hosts), &server_arg],
    );
    let go = run_rospo_binary(
        &common::go_baseline_path(),
        &["-q", "grabpubkey", "-k", path_str(&go_known_hosts), &server_arg],
    );

    assert_eq!(rust, go);
    assert_eq!(
        std::fs::read_to_string(&rust_known_hosts).expect("read rust known_hosts"),
        std::fs::read_to_string(&go_known_hosts).expect("read go known_hosts"),
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rust_and_go_binaries_match_for_shell_exec_against_rust_sshd() {
    if !has_go_baseline() {
        eprintln!("skipping: /tmp/rospo-go-baseline not available");
        return;
    }

    let server = start_sshd(vec![authorized_keys_path("authorized_keys")], "", false, false).await;

    let identity = common::key_path("client");
    let server_arg = format!("ferama@{}", server.addr);
    let rust = run_rospo_binary(
        &rust_binary_path(),
        &[
            "-q",
            "shell",
            "-i",
            "-s",
            path_str(&identity),
            &server_arg,
            "printf binary-diff-ok",
        ],
    );
    let go = run_rospo_binary(
        &common::go_baseline_path(),
        &[
            "-q",
            "shell",
            "-i",
            "-s",
            path_str(&identity),
            &server_arg,
            "printf binary-diff-ok",
        ],
    );

    assert_eq!(rust, go);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rust_and_go_servers_match_for_behavioral_probes() {
    if !has_go_baseline() {
        eprintln!("skipping: /tmp/rospo-go-baseline not available");
        return;
    }

    let rust_server = start_sshd(vec![authorized_keys_path("authorized_keys")], "", false, false).await;
    let go_server = start_go_sshd(&authorized_keys_path("authorized_keys"), "")
        .await
        .expect("start go sshd");

    let rust_behavior = probe_server_behavior(&rust_server.addr).await;
    let go_behavior = probe_server_behavior(&go_server.addr).await;

    assert_eq!(rust_behavior, go_behavior);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rust_and_go_binaries_match_for_put_against_rust_sshd() {
    if !has_go_baseline() {
        eprintln!("skipping: /tmp/rospo-go-baseline not available");
        return;
    }

    let server = start_sshd(vec![authorized_keys_path("authorized_keys")], "", false, false).await;
    let identity = common::key_path("client");
    let server_arg = format!("ferama@{}", server.addr);
    let local = unique_path("rospo-binary-put-local");
    let rust_remote = unique_path("rospo-binary-put-rust-remote");
    let go_remote = unique_path("rospo-binary-put-go-remote");
    let payload = b"binary-put-parity\n";
    std::fs::write(&local, payload).expect("write local put payload");

    let rust = run_rospo_binary(
        &rust_binary_path(),
        &[
            "-q",
            "put",
            "-i",
            "-s",
            path_str(&identity),
            &server_arg,
            path_str(&local),
            path_str(&rust_remote),
        ],
    );
    let go = run_rospo_binary(
        &common::go_baseline_path(),
        &[
            "-q",
            "put",
            "-i",
            "-s",
            path_str(&identity),
            &server_arg,
            path_str(&local),
            path_str(&go_remote),
        ],
    );

    assert_eq!(rust, go);
    assert_eq!(std::fs::read(&rust_remote).expect("read rust uploaded file"), payload);
    assert_eq!(std::fs::read(&go_remote).expect("read go uploaded file"), payload);

    let _ = std::fs::remove_file(local);
    let _ = std::fs::remove_file(rust_remote);
    let _ = std::fs::remove_file(go_remote);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rust_and_go_binaries_match_for_get_against_rust_sshd() {
    if !has_go_baseline() {
        eprintln!("skipping: /tmp/rospo-go-baseline not available");
        return;
    }

    let server = start_sshd(vec![authorized_keys_path("authorized_keys")], "", false, false).await;
    let identity = common::key_path("client");
    let server_arg = format!("ferama@{}", server.addr);
    let remote = unique_path("rospo-binary-get-remote");
    let rust_local = unique_path("rospo-binary-get-rust-local");
    let go_local = unique_path("rospo-binary-get-go-local");
    let payload = b"binary-get-parity\n";
    std::fs::write(&remote, payload).expect("write remote get payload");

    let rust = run_rospo_binary(
        &rust_binary_path(),
        &[
            "-q",
            "get",
            "-i",
            "-s",
            path_str(&identity),
            &server_arg,
            path_str(&remote),
            path_str(&rust_local),
        ],
    );
    let go = run_rospo_binary(
        &common::go_baseline_path(),
        &[
            "-q",
            "get",
            "-i",
            "-s",
            path_str(&identity),
            &server_arg,
            path_str(&remote),
            path_str(&go_local),
        ],
    );

    assert_eq!(rust, go);
    assert_eq!(std::fs::read(&rust_local).expect("read rust downloaded file"), payload);
    assert_eq!(std::fs::read(&go_local).expect("read go downloaded file"), payload);

    let _ = std::fs::remove_file(remote);
    let _ = std::fs::remove_file(rust_local);
    let _ = std::fs::remove_file(go_local);
}

async fn probe_server_behavior(server_addr: &str) -> ServerBehavior {
    let known_hosts = PathBuf::from(unique_path("rospo-behavior-known-hosts"));
    let options = client_options_for(server_addr, "client", None, true, &known_hosts, Vec::new()).await;

    let mut shell_session = Session::connect(options.clone()).await.expect("connect shell session");
    let shell_status = shell_session
        .run_command("true")
        .await
        .expect("run shell probe");
    shell_session.disconnect().await.expect("disconnect shell probe");

    let mut sftp_client = SftpClient::connect(options.clone()).await.expect("connect sftp client");
    let local_upload = std::env::temp_dir().join(format!("rospo-behavior-up-{}", std::process::id()));
    let local_download = std::env::temp_dir().join(format!("rospo-behavior-down-{}", std::process::id()));
    let payload = b"side-by-side-sftp".to_vec();
    std::fs::write(&local_upload, &payload).expect("write upload payload");
    let _ = std::fs::remove_file(&local_download);
    let remote_path = format!("/tmp/rospo-behavior-{}", std::process::id());
    sftp_client
        .put_file(&remote_path, &local_upload.to_string_lossy())
        .await
        .expect("put sftp payload");
    sftp_client
        .get_file(&remote_path, &local_download.to_string_lossy())
        .await
        .expect("get sftp payload");
    let sftp_roundtrip = std::fs::read(&local_download).expect("read downloaded payload");
    let _ = std::fs::remove_file(&local_upload);
    let _ = std::fs::remove_file(&local_download);
    let _ = sftp_client.close().await;

    let proxy_addr = reserve_local_addr();
    let proxy_addr_for_task = proxy_addr.clone();
    let socks_options = options.clone();
    let socks_task = tokio::spawn(async move { socks::run(socks_options, &proxy_addr_for_task).await });
    wait_for_tcp(&proxy_addr).await;
    let (http_addr, http_task) = start_http_hello_server("behavior-socks").await;
    let remote_http = new_endpoint(&http_addr).expect("parse http endpoint");
    let socks_response = fetch_http_via_socks(&proxy_addr, &remote_http.host, remote_http.port).await;
    socks_task.abort();
    http_task.abort();

    let (echo_addr, echo_task) = start_echo_service().await;
    let local_forward_addr = reserve_local_addr();
    let forward_task = tokio::spawn({
        let options = options.clone();
        let local = new_endpoint(&local_forward_addr).expect("parse forward local");
        let remote = new_endpoint(&echo_addr).expect("parse forward remote");
        async move { run_forward(options, local, remote).await }
    });
    wait_for_tcp(&local_forward_addr).await;
    let forward_echo = echo_roundtrip(&local_forward_addr, b"forward-diff\n").await;
    forward_task.abort();

    let reverse_addr = reserve_local_addr();
    let reverse_task = tokio::spawn({
        let options = options.clone();
        let local = new_endpoint(&echo_addr).expect("parse reverse local");
        let remote = new_endpoint(&reverse_addr).expect("parse reverse remote");
        async move { run_reverse(options, local, remote).await }
    });
    wait_for_tcp(&reverse_addr).await;
    let reverse_echo = echo_roundtrip(&reverse_addr, b"reverse-diff\n").await;
    reverse_task.abort();
    echo_task.abort();

    ServerBehavior {
        shell_status,
        sftp_roundtrip,
        socks_response,
        forward_echo,
        reverse_echo,
    }
}

async fn fetch_http_via_socks(proxy_addr: &str, host: &str, port: u16) -> String {
    let mut socket = tokio::net::TcpStream::connect(proxy_addr)
        .await
        .expect("connect socks proxy");
    socket.write_all(&[0x05, 0x01, 0x00]).await.expect("write socks methods");
    let mut method_reply = [0u8; 2];
    socket
        .read_exact(&mut method_reply)
        .await
        .expect("read socks method reply");
    assert_eq!(method_reply, [0x05, 0x00]);

    let ip: std::net::Ipv4Addr = host.parse().expect("parse ipv4 host");
    let mut request = vec![0x05, 0x01, 0x00, 0x01];
    request.extend_from_slice(&ip.octets());
    request.extend_from_slice(&port.to_be_bytes());
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
        .expect("write proxied request");
    let mut response = Vec::new();
    socket
        .read_to_end(&mut response)
        .await
        .expect("read proxied response");
    String::from_utf8(response).expect("utf8 proxied response")
}

async fn echo_roundtrip(addr: &str, payload: &[u8]) -> Vec<u8> {
    let mut conn = tokio::net::TcpStream::connect(addr)
        .await
        .expect("connect echo endpoint");
    conn.write_all(payload).await.expect("write echo payload");
    let mut buf = vec![0u8; payload.len()];
    conn.read_exact(&mut buf).await.expect("read echo payload");
    buf
}

fn rust_binary_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rospo"))
}

fn run_rospo_binary(binary: &Path, args: &[&str]) -> BinaryResult {
    let output = StdCommand::new(binary)
        .args(args)
        .output()
        .expect("run rospo binary");
    BinaryResult {
        status: output.status.code().unwrap_or_default(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn path_str(path: &Path) -> &str {
    path.to_str().expect("utf8 path")
}
