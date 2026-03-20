mod common;

use rospo::ssh::Session;

use common::{authorized_keys_path, client_options_for, start_sshd, unique_path};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn keepalive_request_succeeds_against_rust_sshd() {
    let known_hosts = unique_path("rospo-keepalive-known-hosts");
    let rust_server = start_sshd(vec![authorized_keys_path("authorized_keys")], "", false, false).await;
    let rust_options = client_options_for(&rust_server.addr, "client", None, true, &known_hosts, Vec::new()).await;
    let mut rust_session = Session::connect(rust_options).await.expect("connect rust sshd");
    rust_session
        .send_keepalive_request()
        .await
        .expect("send keepalive to rust sshd");
    rust_session.disconnect().await.expect("disconnect rust sshd");
}
