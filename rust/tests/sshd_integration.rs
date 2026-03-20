mod common;

use std::sync::{Arc, Mutex};
use std::path::PathBuf;

use rospo::sftp::{Client as SftpClient, ProgressReporter, TransferOptions};
use rospo::ssh::Session;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sftp_recursive_get_keeps_root_dir_name_and_permissions() {
    let server = start_sshd(vec![authorized_keys_path("authorized_keys")], "", false, false).await;
    let known_hosts = PathBuf::from(common::unique_path("rospo-known-hosts-sftp-recursive"));
    let options = client_options_for(&server.addr, "client", None, true, &known_hosts, Vec::new()).await;

    let mut session = Session::connect(options.clone()).await.expect("connect");
    let sftp = session.open_sftp().await.expect("open sftp");

    let remote_root = format!("/tmp/rospo-recursive-root-{}", std::process::id());
    let remote_child_dir = format!("{remote_root}/nested");
    let remote_file = format!("{remote_child_dir}/file.txt");
    sftp.create_dir(remote_root.clone()).await.expect("create remote root");
    sftp.create_dir(remote_child_dir.clone()).await.expect("create remote nested");
    let mut file = sftp
        .open_with_flags(
            remote_file.clone(),
            russh_sftp::protocol::OpenFlags::CREATE | russh_sftp::protocol::OpenFlags::WRITE,
        )
        .await
        .expect("open remote file");
    tokio::io::AsyncWriteExt::write_all(&mut file, b"recursive").await.expect("write remote file");
    let mut attrs = russh_sftp::protocol::FileAttributes::empty();
    attrs.permissions = Some(0o640);
    sftp.set_metadata(remote_file.clone(), attrs).await.expect("chmod remote file");
    session.disconnect().await.expect("disconnect");

    let target = tempfile::tempdir().expect("tempdir");
    let client = SftpClient::connect(options).await.expect("connect sftp client");
    client
        .get_recursive_with_options(&remote_root, target.path().to_str().expect("utf8"), TransferOptions::new(2, 2))
        .await
        .expect("recursive get");

    let downloaded = target.path().join("rospo-recursive-root-".to_owned() + &std::process::id().to_string()).join("nested/file.txt");
    assert_eq!(std::fs::read_to_string(&downloaded).expect("read downloaded file"), "recursive");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        assert_eq!(std::fs::metadata(&downloaded).expect("metadata").permissions().mode() & 0o777, 0o640);
    }
}

#[derive(Clone, Default)]
struct CaptureReporter {
    captures: Arc<Mutex<Vec<(u64, u64, String, u64)>>>,
}

impl ProgressReporter for CaptureReporter {
    fn spawn(
        &self,
        file_size: u64,
        offset: u64,
        file_name: String,
        mut progress_rx: mpsc::Receiver<u64>,
    ) -> JoinHandle<()> {
        let captures = self.captures.clone();
        tokio::spawn(async move {
            let mut total = 0u64;
            while let Some(delta) = progress_rx.recv().await {
                total += delta;
            }
            captures
                .lock()
                .expect("lock captures")
                .push((file_size, offset, file_name, total));
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sftp_progress_reports_resume_offsets_for_upload_and_download() {
    let server = start_sshd(vec![authorized_keys_path("authorized_keys")], "", false, false).await;
    let known_hosts = PathBuf::from(common::unique_path("rospo-known-hosts-sftp-progress"));
    let options = client_options_for(&server.addr, "client", None, true, &known_hosts, Vec::new()).await;

    let upload_local = tempfile::NamedTempFile::new().expect("upload temp");
    let upload_payload = vec![b'u'; 300 * 1024];
    std::fs::write(upload_local.path(), &upload_payload).expect("write upload temp");

    let reporter = Arc::new(CaptureReporter::default());
    let client = SftpClient::connect(options.clone()).await.expect("connect sftp client");

    let remote_upload = format!("/tmp/rospo-progress-upload-{}", std::process::id());
    {
        let mut session = Session::connect(options.clone()).await.expect("connect upload seed");
        let sftp = session.open_sftp().await.expect("open sftp");
        let mut file = sftp
            .open_with_flags(
                remote_upload.clone(),
                russh_sftp::protocol::OpenFlags::CREATE | russh_sftp::protocol::OpenFlags::WRITE,
            )
            .await
            .expect("open remote seed");
        tokio::io::AsyncWriteExt::write_all(&mut file, &upload_payload[..128 * 1024])
            .await
            .expect("seed remote partial");
        session.disconnect().await.expect("disconnect seed");
    }

    client
        .put_file_with_options_and_progress(
            &remote_upload,
            upload_local.path().to_str().expect("utf8"),
            TransferOptions::new(4, 1),
            Some(reporter.clone()),
        )
        .await
        .expect("resume upload");

    let download_local = tempfile::NamedTempFile::new().expect("download temp");
    std::fs::write(download_local.path(), &upload_payload[..64 * 1024]).expect("seed local partial");
    client
        .get_file_with_options_and_progress(
            &remote_upload,
            download_local.path().to_str().expect("utf8"),
            TransferOptions::new(4, 1),
            Some(reporter.clone()),
        )
        .await
        .expect("resume download");

    let captures = reporter.captures.lock().expect("captures").clone();
    assert!(captures.iter().any(|(size, offset, name, progressed)| {
        *size == upload_payload.len() as u64
            && *offset == 128 * 1024
            && name.ends_with(&format!("rospo-progress-upload-{}", std::process::id()))
            && *progressed == upload_payload.len() as u64 - (128 * 1024) as u64
    }), "{captures:?}");
    assert!(captures.iter().any(|(size, offset, name, progressed)| {
        *size == upload_payload.len() as u64
            && *offset == 64 * 1024
            && name.ends_with(&format!("rospo-progress-upload-{}", std::process::id()))
            && *progressed == upload_payload.len() as u64 - (64 * 1024) as u64
    }), "{captures:?}");

    assert_eq!(std::fs::read(download_local.path()).expect("read download"), upload_payload);
}
