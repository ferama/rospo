#![allow(dead_code)]

use std::net::TcpListener as StdTcpListener;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rospo::ssh::{fetch_server_public_key, ClientOptions, JumpHostOptions};
use rospo::sshd::{self, ServerOptions};
use rospo::utils::{add_host_key_to_known_hosts, new_endpoint, parse_ssh_url};
use tempfile::TempDir;
use tokio::process::{Child, Command};
use tokio::task::JoinHandle;

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rust dir has parent")
        .to_path_buf()
}

pub fn unique_path(prefix: &str) -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{ts}-{}", std::process::id()))
}

pub fn reserve_local_addr() -> String {
    let listener = StdTcpListener::bind("127.0.0.1:0").expect("bind ephemeral tcp listener");
    let addr = listener.local_addr().expect("listener local addr").to_string();
    drop(listener);
    addr
}

pub async fn wait_for_tcp(addr: &str) {
    for _ in 0..100 {
        if tokio::net::TcpStream::connect(addr).await.is_ok() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("timed out waiting for tcp listener at {addr}");
}

pub struct StartedServer {
    pub addr: String,
    _tempdir: TempDir,
    task: JoinHandle<Result<(), String>>,
}

impl Drop for StartedServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub async fn start_sshd(
    authorized_keys: Vec<String>,
    authorized_password: &str,
    disable_shell: bool,
    disable_sftp_subsystem: bool,
) -> StartedServer {
    let tempdir = tempfile::tempdir().expect("create tempdir");
    let listen_address = reserve_local_addr();
    let options = ServerOptions {
        server_key: tempdir.path().join("server_key").display().to_string(),
        authorized_keys,
        authorized_password: authorized_password.to_string(),
        listen_address: listen_address.clone(),
        disable_shell,
        disable_banner: false,
        disable_auth: false,
        disable_sftp_subsystem,
        disable_tunnelling: false,
        shell_executable: String::new(),
    };

    let task = tokio::spawn(sshd::run(options.clone()));
    wait_for_tcp(&listen_address).await;

    StartedServer {
        addr: listen_address,
        _tempdir: tempdir,
        task,
    }
}

pub struct StartedGoServer {
    pub addr: String,
    _tempdir: TempDir,
    child: Child,
}

impl Drop for StartedGoServer {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

pub fn go_baseline_path() -> PathBuf {
    PathBuf::from("/tmp/rospo-go-baseline")
}

pub fn has_go_baseline() -> bool {
    go_baseline_path().exists()
}

pub async fn start_go_sshd(authorized_keys: &str, authorized_password: &str) -> Option<StartedGoServer> {
    if !has_go_baseline() {
        return None;
    }

    let tempdir = tempfile::tempdir().expect("create tempdir");
    let addr = reserve_local_addr();
    let mut command = Command::new(go_baseline_path());
    command
        .arg("sshd")
        .arg("-I")
        .arg(tempdir.path().join("server_key"))
        .arg("-P")
        .arg(&addr)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    if authorized_password.is_empty() {
        command.arg("-K").arg(authorized_keys);
    } else {
        command.arg("-A").arg(authorized_password);
    }

    let child = command.spawn().expect("spawn go baseline sshd");
    wait_for_tcp(&addr).await;
    Some(StartedGoServer {
        addr,
        _tempdir: tempdir,
        child,
    })
}

pub fn key_path(name: &str) -> PathBuf {
    repo_root().join("testdata").join(name)
}

pub fn authorized_keys_path(name: &str) -> String {
    key_path(name).display().to_string()
}

pub async fn client_options_for(
    server_addr: &str,
    identity_name: &str,
    password: Option<&str>,
    insecure: bool,
    known_hosts_path: &Path,
    jump_hosts: Vec<JumpHostOptions>,
) -> ClientOptions {
    if !insecure {
        let endpoint = new_endpoint(server_addr).expect("parse server endpoint");
        let key = fetch_server_public_key((endpoint.host.as_str(), endpoint.port))
            .await
            .expect("fetch server public key");
        add_host_key_to_known_hosts(server_addr, &key, known_hosts_path).expect("write known_hosts");
    }

    let server = parse_ssh_url(&format!("ferama@{server_addr}")).expect("parse ssh url");
    ClientOptions {
        username: server.username,
        host: server.host.trim_matches(&['[', ']'][..]).to_string(),
        port: server.port,
        identity: key_path(identity_name),
        known_hosts: known_hosts_path.to_path_buf(),
        password: password.map(str::to_string),
        insecure,
        quiet: true,
        jump_hosts,
    }
}

pub async fn start_echo_service() -> (String, JoinHandle<()>) {
    let addr = reserve_local_addr();
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("bind echo listener");
    let task = tokio::spawn(async move {
        loop {
            let Ok((socket, _)) = listener.accept().await else {
                break;
            };
            tokio::spawn(async move {
                let (mut reader, mut writer) = socket.into_split();
                let _ = tokio::io::copy(&mut reader, &mut writer).await;
            });
        }
    });
    (addr, task)
}

pub async fn start_http_hello_server(body: &'static str) -> (String, JoinHandle<()>) {
    let addr = reserve_local_addr();
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("bind http test listener");
    let task = tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            tokio::spawn(async move {
                let mut buf = vec![0u8; 4096];
                let _ = socket.readable().await;
                let _ = socket.try_read(&mut buf);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = tokio::io::AsyncWriteExt::write_all(&mut socket, response.as_bytes()).await;
                let _ = tokio::io::AsyncWriteExt::shutdown(&mut socket).await;
            });
        }
    });
    (addr, task)
}
