use std::path::Path;

use russh_sftp::client::SftpSession;
use russh_sftp::protocol::OpenFlags;
use tokio::fs;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};

use crate::ssh::{ClientOptions, Session};

pub const DEFAULT_CHUNK_SIZE: usize = 128 * 1024;

pub struct Client {
    session: Session,
    sftp: SftpSession,
}

impl Client {
    pub async fn connect(options: ClientOptions) -> Result<Self, String> {
        let mut session = Session::connect(options).await?;
        let sftp = session.open_sftp().await?;
        Ok(Self { session, sftp })
    }

    pub async fn close(&mut self) -> Result<(), String> {
        let _ = self.sftp.close().await;
        self.session.disconnect().await
    }

    pub async fn get_file(&self, remote: &str, local: &str) -> Result<(), String> {
        let remote_path = canonicalize_or(remote, ".", &self.sftp).await?;
        let remote_meta = self.sftp.metadata(remote_path.clone()).await.map_err(|err| err.to_string())?;
        let local_path = resolve_local_target(local, &remote_path).await?;

        let offset = match fs::metadata(&local_path).await {
            Ok(meta) => meta.len(),
            Err(_) => 0,
        };
        if offset >= remote_meta.len() {
            return Ok(());
        }

        let mut remote_file = self.sftp.open(remote_path).await.map_err(|err| err.to_string())?;
        let mut local_file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&local_path)
            .await
            .map_err(|err| err.to_string())?;

        if offset > 0 {
            remote_file
                .seek(std::io::SeekFrom::Start(offset))
                .await
                .map_err(|err| err.to_string())?;
            local_file
                .seek(std::io::SeekFrom::Start(offset))
                .await
                .map_err(|err| err.to_string())?;
        }

        tokio::io::copy(&mut remote_file, &mut local_file)
            .await
            .map_err(|err| err.to_string())?;
        local_file.flush().await.map_err(|err| err.to_string())?;
        remote_file.shutdown().await.map_err(|err| err.to_string())?;
        Ok(())
    }

    pub async fn put_file(&self, remote: &str, local: &str) -> Result<(), String> {
        let local_meta = fs::metadata(local).await.map_err(|err| err.to_string())?;
        if local_meta.is_dir() {
            return Err(format!("local path is not a file: {local}"));
        }

        let remote_target = resolve_remote_target(remote, local, &self.sftp).await?;
        let offset = match self.sftp.metadata(remote_target.clone()).await {
            Ok(meta) => meta.len(),
            Err(_) => 0,
        };
        if offset >= local_meta.len() {
            return Ok(());
        }

        if let Some(parent) = remote_parent(&remote_target) {
            ensure_remote_dir(&self.sftp, &parent).await?;
        }

        let mut local_file = fs::File::open(local).await.map_err(|err| err.to_string())?;
        let mut remote_file = self
            .sftp
            .open_with_flags(
                remote_target,
                OpenFlags::CREATE | OpenFlags::WRITE | OpenFlags::READ,
            )
            .await
            .map_err(|err| err.to_string())?;

        if offset > 0 {
            local_file
                .seek(std::io::SeekFrom::Start(offset))
                .await
                .map_err(|err| err.to_string())?;
            remote_file
                .seek(std::io::SeekFrom::Start(offset))
                .await
                .map_err(|err| err.to_string())?;
        }

        tokio::io::copy(&mut local_file, &mut remote_file)
            .await
            .map_err(|err| err.to_string())?;
        remote_file.flush().await.map_err(|err| err.to_string())?;
        remote_file.shutdown().await.map_err(|err| err.to_string())?;
        Ok(())
    }

    pub async fn get_recursive(&self, remote: &str, local: &str) -> Result<(), String> {
        let remote_root = canonicalize_or(remote, ".", &self.sftp).await?;
        let remote_meta = self.sftp.metadata(remote_root.clone()).await.map_err(|err| err.to_string())?;
        if !remote_meta.is_dir() {
            return Err(format!("remote path is not a directory: {remote_root}"));
        }
        let local_meta = fs::metadata(local).await.map_err(|err| err.to_string())?;
        if !local_meta.is_dir() {
            return Err(format!("local path is not a directory: {local}"));
        }
        self.get_recursive_inner(&remote_root, Path::new(local), &remote_root).await
    }

    pub async fn put_recursive(&self, remote: &str, local: &str) -> Result<(), String> {
        let local_meta = fs::metadata(local).await.map_err(|err| err.to_string())?;
        if !local_meta.is_dir() {
            return Err(format!("local path is not a directory: {local}"));
        }

        let remote_root = canonicalize_or(remote, ".", &self.sftp).await?;
        let remote_meta = self.sftp.metadata(remote_root.clone()).await.map_err(|err| err.to_string())?;
        if !remote_meta.is_dir() {
            return Err(format!("remote path is not a directory: {remote_root}"));
        }

        let base = Path::new(local)
            .file_name()
            .ok_or_else(|| format!("invalid local path: {local}"))?
            .to_string_lossy()
            .into_owned();
        let target_root = remote_join(&remote_root, &base);
        ensure_remote_dir(&self.sftp, &target_root).await?;
        self.put_recursive_inner(Path::new(local), &target_root).await
    }

    async fn get_recursive_inner(&self, remote_path: &str, local_root: &Path, root: &str) -> Result<(), String> {
        let entries = self.sftp.read_dir(remote_path).await.map_err(|err| err.to_string())?;
        for entry in entries {
            let child_remote = remote_join(remote_path, &entry.file_name());
            let relative = child_remote.trim_start_matches(root).trim_start_matches('/');
            let local_path = local_root.join(relative);
            if entry.metadata().is_dir() {
                fs::create_dir_all(&local_path).await.map_err(|err| err.to_string())?;
                Box::pin(self.get_recursive_inner(&child_remote, local_root, root)).await?;
            } else {
                if let Some(parent) = local_path.parent() {
                    fs::create_dir_all(parent).await.map_err(|err| err.to_string())?;
                }
                self.get_file(&child_remote, &local_path.to_string_lossy()).await?;
            }
        }
        Ok(())
    }

    async fn put_recursive_inner(&self, local_path: &Path, remote_root: &str) -> Result<(), String> {
        let mut dir = fs::read_dir(local_path).await.map_err(|err| err.to_string())?;
        while let Some(entry) = dir.next_entry().await.map_err(|err| err.to_string())? {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let target = remote_join(remote_root, &name);
            let metadata = entry.metadata().await.map_err(|err| err.to_string())?;
            if metadata.is_dir() {
                ensure_remote_dir(&self.sftp, &target).await?;
                Box::pin(self.put_recursive_inner(&path, &target)).await?;
            } else {
                self.put_file(&target, &path.to_string_lossy()).await?;
            }
        }
        Ok(())
    }
}

fn remote_join(base: &str, child: &str) -> String {
    if base.ends_with('/') {
        format!("{base}{child}")
    } else if base.is_empty() {
        child.to_string()
    } else {
        format!("{base}/{child}")
    }
}

fn remote_parent(path: &str) -> Option<String> {
    path.rsplit_once('/').map(|(parent, _)| {
        if parent.is_empty() {
            "/".to_string()
        } else {
            parent.to_string()
        }
    })
}

async fn canonicalize_or(path: &str, fallback: &str, sftp: &SftpSession) -> Result<String, String> {
    let target = if path.is_empty() { fallback } else { path };
    sftp.canonicalize(target).await.map_err(|err| err.to_string())
}

async fn resolve_local_target(local: &str, remote_path: &str) -> Result<String, String> {
    let local_path = if local.is_empty() { "." } else { local };
    match fs::metadata(local_path).await {
        Ok(meta) if meta.is_dir() => {
            let file_name = Path::new(remote_path)
                .file_name()
                .ok_or_else(|| format!("invalid remote path: {remote_path}"))?;
            Ok(Path::new(local_path).join(file_name).to_string_lossy().into_owned())
        }
        _ => Ok(local_path.to_string()),
    }
}

async fn resolve_remote_target(remote: &str, local: &str, sftp: &SftpSession) -> Result<String, String> {
    let remote_path = if remote.is_empty() { "." } else { remote };
    if let Ok(meta) = sftp.metadata(remote_path).await {
        if meta.is_dir() {
            let file_name = Path::new(local)
                .file_name()
                .ok_or_else(|| format!("invalid local path: {local}"))?;
            return Ok(remote_join(remote_path, &file_name.to_string_lossy()));
        }
        return Ok(remote_path.to_string());
    }

    if remote.is_empty() {
        let file_name = Path::new(local)
            .file_name()
            .ok_or_else(|| format!("invalid local path: {local}"))?;
        Ok(file_name.to_string_lossy().into_owned())
    } else {
        Ok(remote.to_string())
    }
}

async fn ensure_remote_dir(sftp: &SftpSession, path: &str) -> Result<(), String> {
    if path.is_empty() || path == "." || path == "/" {
        return Ok(());
    }
    if sftp.try_exists(path).await.map_err(|err| err.to_string())? {
        return Ok(());
    }
    if let Some(parent) = remote_parent(path) {
        Box::pin(ensure_remote_dir(sftp, &parent)).await?;
    }
    match sftp.create_dir(path).await {
        Ok(()) => Ok(()),
        Err(err) => {
            if sftp.try_exists(path).await.map_err(|inner| inner.to_string())? {
                Ok(())
            } else {
                Err(err.to_string())
            }
        }
    }
}
