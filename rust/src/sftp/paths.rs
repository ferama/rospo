use std::path::{Path, PathBuf};

use russh_sftp::client::SftpSession;
use tokio::fs;

use super::types::Chunk;

pub(crate) fn build_chunks(offset: u64, size: u64) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut cursor = offset;
    while cursor < size {
        let remaining = size - cursor;
        let len = remaining.min(super::DEFAULT_CHUNK_SIZE as u64);
        chunks.push(Chunk {
            offset: cursor,
            len,
        });
        cursor += len;
    }
    chunks
}

pub(crate) fn remote_join(base: &str, child: &str) -> String {
    if base == "/" {
        format!("/{child}")
    } else if base.is_empty() {
        child.to_string()
    } else {
        format!("{}/{}", base.trim_end_matches('/'), child)
    }
}

pub(crate) fn remote_parent(path: &str) -> Option<String> {
    let path = path.trim_end_matches('/');
    if path.is_empty() || path == "." || path == "/" {
        return None;
    }
    path.rsplit_once('/').map(|(parent, _)| {
        if parent.is_empty() {
            "/".to_string()
        } else {
            parent.to_string()
        }
    })
}

pub(crate) async fn canonicalize_or(path: &str, fallback: &str, sftp: &SftpSession) -> Result<String, String> {
    let target = if path.is_empty() { fallback } else { path };
    match sftp.canonicalize(target.to_string()).await {
        Ok(path) => Ok(path),
        Err(_) => Ok(target.to_string()),
    }
}

pub(crate) async fn resolve_local_target(local: &str, remote_path: &str) -> Result<String, String> {
    let local_path = if local.is_empty() { "." } else { local };
    let metadata = fs::metadata(local_path).await.ok();
    if metadata.as_ref().is_some_and(|meta| meta.is_dir()) {
        let name = Path::new(remote_path)
            .file_name()
            .ok_or_else(|| format!("invalid remote path: {remote_path}"))?;
        Ok(Path::new(local_path).join(name).display().to_string())
    } else {
        Ok(local_path.to_string())
    }
}

pub(crate) async fn resolve_remote_target(remote: &str, local: &str, sftp: &SftpSession) -> Result<String, String> {
    let remote_path = if remote.is_empty() { "." } else { remote };
    let local_name = Path::new(local)
        .file_name()
        .ok_or_else(|| format!("invalid local path: {local}"))?
        .to_string_lossy()
        .into_owned();
    let metadata = sftp.metadata(remote_path.to_string()).await.ok();
    if metadata.as_ref().is_some_and(|meta| meta.is_dir()) {
        Ok(remote_join(remote_path, &local_name))
    } else {
        Ok(remote_path.to_string())
    }
}

pub(crate) async fn ensure_remote_dir(sftp: &SftpSession, path: &str) -> Result<(), String> {
    if path.is_empty() || path == "." || path == "/" {
        return Ok(());
    }

    let mut current = String::new();
    for part in PathBuf::from(path).components() {
        let segment = part.as_os_str().to_string_lossy();
        current = remote_join(&current, &segment);
        if sftp.metadata(current.clone()).await.is_err() {
            sftp.create_dir(current.clone()).await.map_err(|err| err.to_string())?;
        }
    }
    Ok(())
}
