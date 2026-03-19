mod common;

use std::path::PathBuf;

use rospo::ssh::Session;

use common::{authorized_keys_path, client_options_for, start_sshd};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sftp_enabled_accepts_subsystem() {
    let server = start_sshd(vec![authorized_keys_path("authorized_keys")], "", false, false).await;
    let known_hosts = PathBuf::from(common::unique_path("rospo-known-hosts-sftp-enabled"));
    let options = client_options_for(&server.addr, "client", None, true, &known_hosts, Vec::new()).await;

    let mut session = Session::connect(options).await.expect("connect");
    let _sftp = session.open_sftp().await.expect("open sftp subsystem");
    session.disconnect().await.expect("disconnect");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sftp_disabled_rejects_subsystem() {
    let server = start_sshd(vec![authorized_keys_path("authorized_keys")], "", false, true).await;
    let known_hosts = PathBuf::from(common::unique_path("rospo-known-hosts-sftp-disabled"));
    let options = client_options_for(&server.addr, "client", None, true, &known_hosts, Vec::new()).await;

    let mut session = Session::connect(options).await.expect("connect");
    assert!(session.open_sftp().await.is_err());
    session.disconnect().await.expect("disconnect");
}
