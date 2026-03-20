mod common;

use std::path::PathBuf;

use rospo::sftp::{Client as SftpClient, TransferOptions};
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sftp_chunked_put_and_get_roundtrip_large_file() {
    let server = start_sshd(vec![authorized_keys_path("authorized_keys")], "", false, false).await;
    let known_hosts = PathBuf::from(common::unique_path("rospo-known-hosts-sftp-chunked"));
    let options = client_options_for(&server.addr, "client", None, true, &known_hosts, Vec::new()).await;

    let mut client = SftpClient::connect(options).await.expect("connect sftp client");
    let local_upload = std::env::temp_dir().join(format!("rospo-sftp-chunked-up-{}", std::process::id()));
    let local_download =
        std::env::temp_dir().join(format!("rospo-sftp-chunked-down-{}", std::process::id()));
    let _ = std::fs::remove_file(&local_download);

    let payload = vec![b'x'; 400 * 1024];
    std::fs::write(&local_upload, &payload).expect("write local upload fixture");
    let remote_path = format!("/tmp/rospo-sftp-chunked-{}", std::process::id());
    let transfer = TransferOptions::new(4, 1);

    client
        .put_file_with_options(&remote_path, &local_upload.to_string_lossy(), transfer)
        .await
        .expect("chunked upload");
    client
        .get_file_with_options(&remote_path, &local_download.to_string_lossy(), transfer)
        .await
        .expect("chunked download");

    assert_eq!(std::fs::read(&local_download).expect("read chunked download"), payload);

    let _ = std::fs::remove_file(local_upload);
    let _ = std::fs::remove_file(local_download);
    let _ = client.close().await;
}
